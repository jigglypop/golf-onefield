//! 4레인 f32 묶음 커널: 샷 4개를 한 걸음씩 나란히(lockstep) 적분한다.
//!
//! 식은 스칼라 `fly` 와 완전히 같다 (`rhs`·`rk4g` 를 `Lane` 제네릭으로 공유). 다른 점은 두 가지뿐이다.
//!  1. 연산이 4레인 벡터다: wasm simd128 (`W4`, 있을 때) 또는 이식 가능한 배열 (`P4`, 어디서나 컴파일되고 네이티브 시험에도 쓴다).
//!  2. 샷마다 끝나는 걸음이 다르다: 한 레인이 착지(또는 정점)하면 그 레인만 꺼내 스칼라 f64 에르미트로 처리하고
//!     다음 샷을 같은 레인에 채운다 (AVX-512 엔진의 레인 재충전과 같은 생각, 마스크 대신 드문 사건 때 배열로 한 번 내려쓴다).
//!     걸음 h 도 레인마다 벡터라서 `Opts::min_steps` 처럼 샷마다 걸음이 달라도 된다.
//!
//! 정밀도: 걸음은 f32, 착지·정점 보간은 f64 (상태를 f32 → f64 로 올려 3차 에르미트 뉴턴). 결과는 f32 걸음의 반올림 바닥이다
//! (τ 0.05 에서 캐리 최대 1.9e-4 yd, 보상 합 `Opts::compensated` 를 켜면 2.8e-5 yd).
//! 스칼라 f32 (`batch_scalar::<f32>`) 와는 샷마다 비트 단위로 같은 답이다: 같은 연산을 같은 순서로 하기 때문이다 (시험이 확인).
//!
//! 묶음 수 G (한 고리에서 번갈아 돌리는 4레인 묶음 수): 지연 사슬을 숨기려고 G > 1 을 시도했지만 wasm (V8) 에서
//! G = 1 이 가장 빨랐다 (G = 2, 3, 4 는 레지스터 압박으로 15–40% 느림). 그래서 `GROUPS = 1`.

use crate::{apex_height, degenerate, fill_landing, rhs, rk4g, step_for, touchdown, Coef, Env, Init, Landing, Lane, Opts, Shot, Units, K, P, S};
use core::ops::{Add, Div, Mul, Neg, Sub};

/// 레인 벡터가 갖춰야 할 것: 사칙·sqrt·max (`Lane`) + 배열 변환 + 넘어감 마스크
pub trait F4: Lane {
    fn to_arr(self) -> [f32; 4];
    fn from_arr(a: [f32; 4]) -> Self;
    /// 비트 i: 레인 i 에서 y 또는 uy 가 (+ 에서 ≤ 0 으로) 넘어갔다 (착지 또는 정점 후보)
    fn crossed(y0: Self, y1: Self, u0: Self, u1: Self) -> u32;
    /// 레인 I 의 값 (사건 처리가 배열로 내려쓰지 않고 레지스터에서 바로 꺼낸다)
    fn get<const I: usize>(self) -> f32;
    /// 레인 I 만 x 로 바꾼 벡터 (새 샷 채우기)
    fn set<const I: usize>(self, x: f32) -> Self;
}

// ------------------------------------------------------------------ 이식 가능한 4레인 (배열)

/// f32 네 개 묶음. 어느 타깃에서나 컴파일된다. 컴파일러가 가능하면 SIMD 로 올린다.
#[derive(Clone, Copy, Debug)]
pub struct P4(pub [f32; 4]);

macro_rules! p4_bin {
    ($tr:ident, $f:ident, $op:tt) => {
        impl $tr for P4 {
            type Output = P4;
            #[inline(always)]
            fn $f(self, o: P4) -> P4 {
                P4([self.0[0] $op o.0[0], self.0[1] $op o.0[1], self.0[2] $op o.0[2], self.0[3] $op o.0[3]])
            }
        }
    };
}
p4_bin!(Add, add, +);
p4_bin!(Sub, sub, -);
p4_bin!(Mul, mul, *);
p4_bin!(Div, div, /);

impl Neg for P4 {
    type Output = P4;
    #[inline(always)]
    fn neg(self) -> P4 {
        P4([-self.0[0], -self.0[1], -self.0[2], -self.0[3]])
    }
}

impl Lane for P4 {
    #[inline(always)]
    fn lit(x: f64) -> P4 {
        P4([x as f32; 4])
    }
    #[inline(always)]
    fn sqrt(self) -> P4 {
        P4([self.0[0].sqrt(), self.0[1].sqrt(), self.0[2].sqrt(), self.0[3].sqrt()])
    }
    #[inline(always)]
    fn max(self, o: P4) -> P4 {
        P4([self.0[0].max(o.0[0]), self.0[1].max(o.0[1]), self.0[2].max(o.0[2]), self.0[3].max(o.0[3])])
    }
}

impl F4 for P4 {
    #[inline(always)]
    fn to_arr(self) -> [f32; 4] {
        self.0
    }
    #[inline(always)]
    fn from_arr(a: [f32; 4]) -> P4 {
        P4(a)
    }
    #[inline(always)]
    fn crossed(y0: P4, y1: P4, u0: P4, u1: P4) -> u32 {
        let mut m = 0u32;
        for i in 0..4 {
            if (y0.0[i] >= 0.0 && y1.0[i] <= 0.0) || (u0.0[i] > 0.0 && u1.0[i] <= 0.0) {
                m |= 1 << i;
            }
        }
        m
    }
    #[inline(always)]
    fn get<const I: usize>(self) -> f32 {
        self.0[I]
    }
    #[inline(always)]
    fn set<const I: usize>(mut self, x: f32) -> P4 {
        self.0[I] = x;
        self
    }
}

// ------------------------------------------------------------------ wasm simd128

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod wasm {
    use super::*;
    use core::arch::wasm32::*;

    /// v128 한 개 = f32 네 레인
    #[derive(Clone, Copy)]
    pub struct W4(pub v128);

    macro_rules! w4_bin {
        ($tr:ident, $f:ident, $intr:ident) => {
            impl $tr for W4 {
                type Output = W4;
                #[inline(always)]
                fn $f(self, o: W4) -> W4 {
                    W4($intr(self.0, o.0))
                }
            }
        };
    }
    w4_bin!(Add, add, f32x4_add);
    w4_bin!(Sub, sub, f32x4_sub);
    w4_bin!(Mul, mul, f32x4_mul);
    w4_bin!(Div, div, f32x4_div);

    impl Neg for W4 {
        type Output = W4;
        #[inline(always)]
        fn neg(self) -> W4 {
            W4(f32x4_neg(self.0))
        }
    }

    impl Lane for W4 {
        #[inline(always)]
        fn lit(x: f64) -> W4 {
            W4(f32x4_splat(x as f32))
        }
        #[inline(always)]
        fn sqrt(self) -> W4 {
            W4(f32x4_sqrt(self.0))
        }
        #[inline(always)]
        fn max(self, o: W4) -> W4 {
            W4(f32x4_max(self.0, o.0))
        }
    }

    impl F4 for W4 {
        #[inline(always)]
        fn to_arr(self) -> [f32; 4] {
            // SAFETY: v128 과 [f32; 4] 는 같은 16바이트
            unsafe { core::mem::transmute::<v128, [f32; 4]>(self.0) }
        }
        #[inline(always)]
        fn from_arr(a: [f32; 4]) -> W4 {
            // SAFETY: 위와 같다
            W4(unsafe { core::mem::transmute::<[f32; 4], v128>(a) })
        }
        #[inline(always)]
        fn crossed(y0: W4, y1: W4, u0: W4, u1: W4) -> u32 {
            let z = f32x4_splat(0.0);
            let land = v128_and(f32x4_ge(y0.0, z), f32x4_le(y1.0, z));
            let apex = v128_and(f32x4_gt(u0.0, z), f32x4_le(u1.0, z));
            i32x4_bitmask(v128_or(land, apex)) as u32
        }
        #[inline(always)]
        fn get<const I: usize>(self) -> f32 {
            f32x4_extract_lane::<I>(self.0)
        }
        #[inline(always)]
        fn set<const I: usize>(self, x: f32) -> W4 {
            W4(f32x4_replace_lane::<I>(self.0, x))
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub use wasm::W4;

/// 이 빌드의 기본 레인 벡터: simd128 이 켜진 wasm 이면 `W4`, 아니면 이식 가능한 `P4`
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub type V = W4;
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub type V = P4;

/// V 가 진짜 simd128 인가
pub const SIMD: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

// ------------------------------------------------------------------ 레인 상태 보조

type Arr8 = [[f32; 4]; 8];

#[inline(always)]
fn load_s<V4: F4>(a: &Arr8) -> S<V4> {
    S { t: V4::from_arr(a[0]), x: V4::from_arr(a[1]), y: V4::from_arr(a[2]), z: V4::from_arr(a[3]), ux: V4::from_arr(a[4]), uy: V4::from_arr(a[5]), uz: V4::from_arr(a[6]), w: V4::from_arr(a[7]) }
}
#[inline(always)]
fn load_k<V4: F4>(a: &Arr8) -> K<V4> {
    K { g: V4::from_arr(a[0]), vx: V4::from_arr(a[1]), vy: V4::from_arr(a[2]), vz: V4::from_arr(a[3]), ax: V4::from_arr(a[4]), ay: V4::from_arr(a[5]), az: V4::from_arr(a[6]), aw: V4::from_arr(a[7]) }
}
/// 레인 I 의 스칼라 상태. 배열로 내려쓰지 않고 레지스터에서 바로 꺼낸다 (사건이 난 레인만 읽는다)
#[inline(always)]
fn lane_s<V4: F4, const I: usize>(s: &S<V4>) -> S<f32> {
    S { t: s.t.get::<I>(), x: s.x.get::<I>(), y: s.y.get::<I>(), z: s.z.get::<I>(), ux: s.ux.get::<I>(), uy: s.uy.get::<I>(), uz: s.uz.get::<I>(), w: s.w.get::<I>() }
}
#[inline(always)]
fn lane_k<V4: F4, const I: usize>(k: &K<V4>) -> K<f32> {
    K { g: k.g.get::<I>(), vx: k.vx.get::<I>(), vy: k.vy.get::<I>(), vz: k.vz.get::<I>(), ax: k.ax.get::<I>(), ay: k.ay.get::<I>(), az: k.az.get::<I>(), aw: k.aw.get::<I>() }
}
/// 레인 I 에 스칼라 상태를 써 넣는다 (나머지 세 레인은 그대로: 벡터 전체를 다시 읽어 오지 않는다)
#[inline(always)]
fn set_s<V4: F4, const I: usize>(s: &mut S<V4>, v: &S<f32>) {
    *s = S { t: s.t.set::<I>(v.t), x: s.x.set::<I>(v.x), y: s.y.set::<I>(v.y), z: s.z.set::<I>(v.z), ux: s.ux.set::<I>(v.ux), uy: s.uy.set::<I>(v.uy), uz: s.uz.set::<I>(v.uz), w: s.w.set::<I>(v.w) };
}
#[inline(always)]
fn set_k<V4: F4, const I: usize>(k: &mut K<V4>, v: &K<f32>) {
    *k = K { g: k.g.set::<I>(v.g), vx: k.vx.set::<I>(v.vx), vy: k.vy.set::<I>(v.vy), vz: k.vz.set::<I>(v.vz), ax: k.ax.set::<I>(v.ax), ay: k.ay.set::<I>(v.ay), az: k.az.set::<I>(v.az), aw: k.aw.set::<I>(v.aw) };
}

const S_ZERO: S<f32> = S { t: 0.0, x: 0.0, y: 0.0, z: 0.0, ux: 0.0, uy: 0.0, uz: 0.0, w: 0.0 };

/// 사건이 난 레인 하나의 스칼라 f64 처리 (정점 높이, 착지 보간). 끝났으면 결과를 돌려준다.
/// 레인 상수 I 마다 복제되는 곳 밖으로 뺐다 (인라인하면 wasm 이 35 KB 커진다): 호출 비용은 사건당 한 번이다.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn lane_scalar_event(s0l: &S<f32>, k0l: &K<f32>, s1l: &S<f32>, k1l: &K<f32>, hl: f64, un: &Units, apex_done: &mut bool, apex: &mut f64, expired: bool) -> Option<Landing> {
    if !*apex_done && s0l.uy > 0.0 && s1l.uy <= 0.0 {
        *apex = apex_height(s0l, k0l, s1l, k1l, hl) * un.l_m;
        *apex_done = true;
    }
    if s0l.y >= 0.0 && s1l.y <= 0.0 {
        let td = touchdown(s0l, k0l, s1l, k1l, hl);
        let mut l = Landing::default();
        fill_landing(&mut l, &td, un);
        l.apex_m = *apex;
        Some(l)
    } else if s1l.y.is_nan() || expired {
        Some(Landing { carry_m: f64::NAN, lateral_m: f64::NAN, apex_m: f64::NAN, land_deg: f64::NAN, ..Landing::default() })
    } else {
        None
    }
}

/// 새 샷 하나의 레인 값: 초기 상태, 그 상태의 k1 (스칼라 rhs), 축, 걸음
struct NewLane {
    s: S<f32>,
    k: K<f32>,
    oy: f32,
    oz: f32,
    h: f32,
    hh: f32,
    h6: f32,
}

#[inline(never)]
fn new_lane<const WIND: bool>(init: &Init, h: f64, ps: &P<f32>) -> NewLane {
    let s = S { t: 0.0f32, x: 0.0, y: 0.0, z: 0.0, ux: init.u[0] as f32, uy: init.u[1] as f32, uz: init.u[2] as f32, w: init.w as f32 };
    let (oy, oz) = (init.oy as f32, init.oz as f32);
    let h_s = h as f32;
    let k = rhs::<f32, WIND>(ps, oy, oz, s.ux, s.uy, s.uz, s.w);
    NewLane { s, k, oy, oz, h: h_s, hh: h_s * 0.5, h6: h_s / 6.0 }
}

/// 한 묶음(레인 4개)의 배열 꼴 상태: 처음에 샷을 싣을 때만 쓴다 (이후 교체는 레지스터에서 레인 하나만 바꾼다).
/// 걸음은 레인마다 다를 수 있다 (`Opts::min_steps`).
struct Group {
    s: Arr8,
    k: Arr8,
    c: Arr8,
    oy: [f32; 4],
    oz: [f32; 4],
    h: [f32; 4],
    hh: [f32; 4],
    h6: [f32; 4],
}

impl Group {
    fn new() -> Group {
        Group { s: [[0.0; 4]; 8], k: [[0.0; 4]; 8], c: [[0.0; 4]; 8], oy: [0.0; 4], oz: [1.0; 4], h: [0.1; 4], hh: [0.05; 4], h6: [0.1 / 6.0; 4] }
    }
}

/// 새 샷을 레인 i 에 싣는다 (처음 채울 때). 반환: 이 레인의 걸음(f64, 보간용)
fn put_lane<const WIND: bool>(gr: &mut Group, i: usize, init: &Init, h: f64, ps: &P<f32>) -> f64 {
    let nl = new_lane::<WIND>(init, h, ps);
    gr.oy[i] = nl.oy;
    gr.oz[i] = nl.oz;
    gr.h[i] = nl.h;
    gr.hh[i] = nl.hh;
    gr.h6[i] = nl.h6;
    let sv = [nl.s.t, nl.s.x, nl.s.y, nl.s.z, nl.s.ux, nl.s.uy, nl.s.uz, nl.s.w];
    let kv = [nl.k.g, nl.k.vx, nl.k.vy, nl.k.vz, nl.k.ax, nl.k.ay, nl.k.az, nl.k.aw];
    for j in 0..8 {
        gr.s[j][i] = sv[j];
        gr.k[j][i] = kv[j];
        gr.c[j][i] = 0.0;
    }
    nl.h as f64
}

const NONE: usize = usize::MAX;

fn write_out(out: &mut [f64], id: usize, l: &Landing) {
    out[4 * id] = l.carry_m;
    out[4 * id + 1] = l.lateral_m;
    out[4 * id + 2] = l.apex_m;
    out[4 * id + 3] = l.land_deg;
}

/// 다음 유효 샷을 꺼낸다. 위로 나가지 않는 샷은 `degenerate` 결과를 바로 쓰고 건너뛴다.
fn take_next(next: &mut usize, n: usize, init_of: &dyn Fn(usize) -> Init, un: &Units, out: &mut [f64]) -> Option<(usize, Init)> {
    while *next < n {
        let i = *next;
        *next += 1;
        let init = init_of(i);
        if init.u[1] > 0.0 && init.u[0].is_finite() && init.w.is_finite() {
            return Some((i, init));
        }
        write_out(out, i, &degenerate(&init, un));
    }
    None
}

/// 한 묶음 = 레인 4개. G 묶음을 한 고리에서 단계별로 번갈아 돌린다 (총 4G 샷 동시 진행).
///
/// 드문 길(착지·정점 사건, 64 반복마다 나이 검사)은 벡터를 배열로 내려쓰지 않는다: 사건이 난 레인(`mask`)만 레인 상수 I 로
/// 필요한 값을 `get` 으로 꺼내 스칼라 f64 로 처리하고, 새 샷은 `set` 으로 그 레인 하나만 바꿔 끼운다.
/// (예전: 걸음 전후 상태 4개와 보상 항 = v128 40개를 배열로 내려쓰고 모든 레인을 훑은 뒤 29개를 다시 읽었다.)
fn run_lanes<V4: F4, const WIND: bool, const COMP: bool, const G: usize>(pv: &P<V4>, ps: &P<f32>, un: &Units, h: f64, opts: &Opts, n: usize, init_of: &dyn Fn(usize) -> Init, out: &mut [f64]) {
    let max_age = ((40.0 / h) as u32).saturating_add(2);

    let mut grp: [Group; G] = core::array::from_fn(|_| Group::new());
    let mut h_eff = [[0.0f64; 4]; G];
    let mut ids = [[NONE; 4]; G];
    let mut born = [[0u32; 4]; G];
    let mut apex = [[0.0f64; 4]; G];
    let mut apex_done = [[false; 4]; G];
    let mut act = [0u32; G];
    let mut next = 0usize;
    let mut dummy: Option<(Init, f64)> = None;
    for g in 0..G {
        for lane in 0..4 {
            match take_next(&mut next, n, init_of, un, out) {
                Some((i, init)) => {
                    let he = step_for(h, &init, opts);
                    ids[g][lane] = i;
                    act[g] |= 1 << lane;
                    h_eff[g][lane] = put_lane::<WIND>(&mut grp[g], lane, &init, he, ps);
                    dummy.get_or_insert((init, he));
                }
                None => {
                    // 빈 레인: 무해한 샷으로 채워 계속 돌게 둔다 (결과는 버린다)
                    let (init, he) = dummy.unwrap_or((Init { u: [1.0, 0.3, 0.0], w: 0.5, oy: 0.0, oz: 1.0 }, h));
                    put_lane::<WIND>(&mut grp[g], lane, &init, he, ps);
                }
            }
        }
    }
    let mut s: [S<V4>; G] = core::array::from_fn(|g| load_s::<V4>(&grp[g].s));
    let mut k: [K<V4>; G] = core::array::from_fn(|g| load_k::<V4>(&grp[g].k));
    let mut cmp: [S<V4>; G] = core::array::from_fn(|g| load_s::<V4>(&grp[g].c));
    let mut oy: [V4; G] = core::array::from_fn(|g| V4::from_arr(grp[g].oy));
    let mut oz: [V4; G] = core::array::from_fn(|g| V4::from_arr(grp[g].oz));
    let mut hv: [V4; G] = core::array::from_fn(|g| V4::from_arr(grp[g].h));
    let mut hhv: [V4; G] = core::array::from_fn(|g| V4::from_arr(grp[g].hh));
    let mut h6v: [V4; G] = core::array::from_fn(|g| V4::from_arr(grp[g].h6));
    let mut iter = 0u32;

    while act.iter().any(|&a| a != 0) {
        let (s1, c1, k1) = rk4g::<V4, WIND, COMP, G>(pv, &oy, &oz, &s, &cmp, &k, &hv, &hhv, &h6v);
        iter = iter.wrapping_add(1);
        let check_age = (iter & 63) == 0;
        for g in 0..G {
            let ev = V4::crossed(s[g].y, s1[g].y, s[g].uy, s1[g].uy) & act[g];
            let (s0, k0) = (s[g], k[g]); // 걸음 전 상태 (사건 보간에 쓴다)
            s[g] = s1[g];
            cmp[g] = c1[g];
            k[g] = k1[g];
            if ev == 0 && !check_age {
                continue;
            }
            // 드문 길: 사건이 난 레인만 (나이 검사 반복에서는 활성 레인 전부) 스칼라 f64 로 처리하고 끝난 레인에 다음 샷을 끼운다
            let mask = if check_age { act[g] } else { ev };
            macro_rules! lane_event {
                ($l:literal) => {
                    if (mask >> $l) & 1 != 0 {
                        let (s0l, k0l, s1l, k1l) = (lane_s::<V4, $l>(&s0), lane_k::<V4, $l>(&k0), lane_s::<V4, $l>(&s[g]), lane_k::<V4, $l>(&k[g]));
                        let expired = iter.wrapping_sub(born[g][$l]) > max_age;
                        if let Some(l) = lane_scalar_event(&s0l, &k0l, &s1l, &k1l, h_eff[g][$l], un, &mut apex_done[g][$l], &mut apex[g][$l], expired) {
                            write_out(out, ids[g][$l], &l);
                            match take_next(&mut next, n, init_of, un, out) {
                                Some((i, init)) => {
                                    ids[g][$l] = i;
                                    born[g][$l] = iter;
                                    apex_done[g][$l] = false;
                                    let nl = new_lane::<WIND>(&init, step_for(h, &init, opts), ps);
                                    set_s::<V4, $l>(&mut s[g], &nl.s);
                                    set_k::<V4, $l>(&mut k[g], &nl.k);
                                    set_s::<V4, $l>(&mut cmp[g], &S_ZERO);
                                    oy[g] = oy[g].set::<$l>(nl.oy);
                                    oz[g] = oz[g].set::<$l>(nl.oz);
                                    hv[g] = hv[g].set::<$l>(nl.h);
                                    hhv[g] = hhv[g].set::<$l>(nl.hh);
                                    h6v[g] = h6v[g].set::<$l>(nl.h6);
                                    h_eff[g][$l] = nl.h as f64;
                                }
                                None => act[g] &= !(1 << $l),
                            }
                        }
                    }
                };
            }
            lane_event!(0);
            lane_event!(1);
            lane_event!(2);
            lane_event!(3);
        }
    }
}

/// 기본 묶음 수 (4G 샷 동시 진행). 벤치로 고른 값: `batch_lanes_g` 를 다른 G 로 불러 비교할 수 있다.
pub const GROUPS: usize = 1;

/// 샷 여러 개를 4G 레인으로 날린다. 입출력은 `batch` 와 같다: `shots` = [mph, 발사각, rpm, 축] × n,
/// 출력 = [캐리 m, 편차 m, 최고 높이 m, 착지각 도] × n. 환경(바람·밀도·점성)을 줄 수 있다.
pub fn batch_lanes<V4: F4>(c: &Coef, env: &Env, shots: &[f64], h: f64) -> Vec<f64> {
    batch_lanes_g::<V4, GROUPS>(c, env, &Opts::default(), shots, h)
}

/// `batch_lanes` 에 선택 사항(`Opts`)을 준다
pub fn batch_lanes_opts<V4: F4>(c: &Coef, env: &Env, opts: &Opts, shots: &[f64], h: f64) -> Vec<f64> {
    batch_lanes_g::<V4, GROUPS>(c, env, opts, shots, h)
}

/// 묶음 수 G 를 고른 판본
pub fn batch_lanes_g<V4: F4, const G: usize>(c: &Coef, env: &Env, opts: &Opts, shots: &[f64], h: f64) -> Vec<f64> {
    let n = shots.len() / 4;
    let mut out = vec![f64::NAN; n * 4];
    if n == 0 || !(h > 0.0 && h.is_finite()) {
        return out;
    }
    let un = env.units();
    let ps = P::<f32>::new(c, &un);
    let pv = P::<V4>::new(c, &un);
    let init_of = |i: usize| Init::from_shot(&Shot { mph: shots[4 * i], vla_deg: shots[4 * i + 1], rpm: shots[4 * i + 2], axis_deg: shots[4 * i + 3] }, &un);
    match (env.wind != [0.0; 3], opts.compensated) {
        (false, false) => run_lanes::<V4, false, false, G>(&pv, &ps, &un, h, opts, n, &init_of, &mut out),
        (true, false) => run_lanes::<V4, true, false, G>(&pv, &ps, &un, h, opts, n, &init_of, &mut out),
        (false, true) => run_lanes::<V4, false, true, G>(&pv, &ps, &un, h, opts, n, &init_of, &mut out),
        (true, true) => run_lanes::<V4, true, true, G>(&pv, &ps, &un, h, opts, n, &init_of, &mut out),
    }
    out
}
