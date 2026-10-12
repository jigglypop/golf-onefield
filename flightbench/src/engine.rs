//! 장 하나 비행 엔진: H2(힉스 이중항 장)·B4·F3, f64 기준 적분기, f32×16 빠른 엔진, 보정, USGA 분포 평균, 모드 장.
//! 실행 파일은 src/bin/h2.rs (속도·수치 정확도) 와 src/bin/calib.rs (자료 대비 정확도).
//!
//! 입력 약속 (`run`/`fly` 와 `stream` 은 같은 샷을 서로 다른 꼴로 받는다 — 헷갈리지 않게 이름이 있다)
//!  - `RunShot`    = [v0', θ(rad), S0, α(rad)]   f64. 스핀축 기울기 α 를 **각도**로. (`run`, `fly`)
//!  - `StreamShot` = [v0', θ(rad), S0, sin α]    f32. 스핀축 기울기를 **사인**으로. (`stream`)
//!    `to_stream_shot` 이 앞의 꼴을 뒤의 꼴로 바꾼다. 두 엔진은 같은 샷에서 같은 착지를 낸다(시험: `stream_matches_run`).
//!
//! 정의역 (모든 입력에 결과가 정해져 있다; 조용히 틀린 값을 내지 않는다)
//!  - 첫 걸음에 땅 밑으로 내려가는 샷(발사각 θ ≤ 0 이고 양력이 중력을 못 이김): 뜨지 못한다. 착지는 발사점 (0, 0).
//!    (양력이 중력을 이기면 θ = 0 이어도 올라가 날아간다 — 판단은 발사각이 아니라 운동으로 한다.)
//!  - 스핀 S0 < 0: 반대 방향 회전으로 본다(축을 π 돌린 것). 공유 포화 장의 r = w·sp/(sp + l·w) 는 w < −sp/l 에 극이 있어
//!    음수 w 를 몸 안쪽에 들이지 않는다. w 는 감쇠(dw/dt = −β w |u|)만 하므로 부호가 바뀌지 않는다.
//!  - v0' ≤ 0, NaN, ±inf 가 있는 입력: (NaN, NaN). `stream` 은 |θ| > π/2 와 |sin α| > 1 도 (NaN, NaN) (sincos 다항식의 정의역).
//!  - 8 무차원 시간 안에 못 떨어지면 (NaN, NaN).

use std::cell::RefCell;
use std::hint::black_box;
use std::sync::OnceLock;
use std::time::Instant;

/// `run`/`fly` 의 샷: [v0', θ (rad), S0, α (rad)]
pub type RunShot = [f64; 4];
/// `stream` 의 샷: [v0', θ (rad), S0, sin α]
pub type StreamShot = [f32; 4];

/// `RunShot` (α 각도) → `StreamShot` (sin α)
#[inline]
pub fn to_stream_shot(s: &RunShot) -> StreamShot {
    [s[0] as f32, s[1] as f32, s[2] as f32, s[3].sin() as f32]
}

pub const BETA: f64 = 0.06;
pub const L_M: f64 = 53.53157581531039; // 2m/(rho A), rho = 1.2
pub const U_MS: f64 = 22.916037151920374; // sqrt(g L)
pub const YD: f64 = L_M / 0.9144;
pub const R_BALL: f64 = 0.04267 / 2.0;
pub const MPH: f64 = 0.44704;

// ------------------------------------------------------------------ 장

pub trait Aero: Copy + Send + Sync {
    /// (|u| C_D, |u| C_L) 를 |u| 와 w 로 돌려준다
    fn kk(&self, sp: f64, w: f64) -> (f64, f64);
    /// 스핀 감쇠 dw/dt = -beta w |u|
    #[inline(always)]
    fn beta(&self) -> f64 {
        BETA
    }
}

/// 스핀 감쇠를 자유 매개변수로 둔 감싸개
#[derive(Clone, Copy)]
pub struct WithBeta<A: Aero> {
    pub a: A,
    pub b: f64,
}
impl<A: Aero> Aero for WithBeta<A> {
    #[inline(always)]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        self.a.kk(sp, w)
    }
    #[inline(always)]
    fn beta(&self) -> f64 {
        self.b
    }
}

#[derive(Clone, Copy, Debug)]
pub struct H2 {
    pub v: f64,
    pub d: f64,
    pub c: f64,
    pub lam: f64,
    /// 달리는 배경: C_D 에 e/|u| (속도가 낮을수록 항력 계수가 커진다, 레이놀즈 수 의존의 첫 항)
    pub e: f64,
}
impl Aero for H2 {
    #[inline(always)]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let lw = self.lam * w;
        (self.v * sp + self.d * w + self.e, self.c * w * sp / (sp * sp + lw * lw).sqrt())
    }
}

/// 공유 포화 장: 스핀 항력과 양력이 같은 포화 r = S/(1 + l S) 를 쓴다.
///   C_D = v + d r,  C_L = c r,   |u| r = w |u| / (|u| + l w)  (나눗셈 하나)
///
/// 정의역: |u| > 0, w ≥ 0, l ≥ 0. 분모 |u| + l w 는 이 안에서 양수다.
/// w < −|u|/l 이면 분모가 0 이 되는 극이 있다(w = −|u|/l 에서 r → ±∞). 엔진의 비행 함수(`fly`, `stream`)는
/// 발사 때 스핀 부호를 접어(S0 < 0 → 반대 회전) w ≥ 0 만 이 함수에 들어오게 한다.
/// 직접 부르는 쪽(screen·spininv 의 스핀 벡터 크기, 시험대 식)도 크기 w ≥ 0 을 넣는다.
#[derive(Clone, Copy, Debug)]
pub struct Shared {
    pub v: f64,
    pub d: f64,
    pub c: f64,
    pub l: f64,
}
impl Aero for Shared {
    #[inline(always)]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let r = w * sp / (sp + self.l * w);
        (self.v * sp + self.d * r, self.c * r)
    }
}

/// 공유 포화 장 + 항력 위기: 배경 항력이 느린 공에서 e 만큼 오른다.
///   C_D = v + e/(1 + (|u|/u0)^n) + d r,  C_L = c r,  r = S/(1 + l S)
///   u0 = Re0 / RE_PER_U. N 은 위기 꺾임의 가파름(짝수, 형 매개변수라 곱셈으로 펼쳐진다):
///   4 = 이전 '공유위기', 12 = '공유위기12'(기본, CV 로 고름)
/// 정의역은 `Shared` 와 같다 (|u| > 0, w ≥ 0, l ≥ 0; 극은 w = −|u|/l).
#[derive(Clone, Copy, Debug)]
pub struct SharedRe<const N: i32 = 12> {
    pub v: f64,
    pub e: f64,
    pub u0: f64,
    pub d: f64,
    pub c: f64,
    pub l: f64,
}
impl<const N: i32> Aero for SharedRe<N> {
    #[inline(always)]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let r = w * sp / (sp + self.l * w);
        let q = sp * sp / (self.u0 * self.u0);
        // q^(N/2): N 이 상수라 분기 없이 곱 몇 번으로 접힌다
        let qn = match N {
            4 => q * q,
            12 => {
                let q3 = q * q * q;
                q3 * q3
            }
            _ => q.powi(N / 2),
        };
        ((self.v + self.e / (1.0 + qn)) * sp + self.d * r, self.c * r)
    }
}

/// 기본 장: '공유위기12' (공유 포화 + 가파른 항력 위기 n = 12), PGA 12행 세 양으로만 맞춘 값.
/// 고르기: 투어 23행 한 행 빼기 CV 2.675 (n = 4 는 2.823), n 단면 4·6·8·12·16 에서 12 가 최소.
/// 판정(PGA 만 훈련): LPGA 2.78/1.68/3.05, USGA 7.42 (프로 13.75, 아마 4.64), FS 캐리 3.33·높이 0.55.
pub fn default_field() -> SharedRe<12> {
    SharedRe { v: 0.1383, e: 0.2540, u0: 74003.0320 / crate::harness::RE_PER_U, d: 1.4838, c: 2.8062, l: 6.5014 }
}

/// 이전 기본 장 '공유위기' (n = 4), 비교·회귀용
pub fn field_n4() -> SharedRe<4> {
    SharedRe { v: 0.1330, e: 0.3247, u0: 62859.6581 / crate::harness::RE_PER_U, d: 1.3688, c: 2.7901, l: 5.9226 }
}

#[derive(Clone, Copy)]
pub struct B4 {
    pub d0: f64,
    pub d1: f64,
    pub l0: f64,
    pub p: f64,
    pub e: f64,
}
impl Aero for B4 {
    #[inline(always)]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        (self.d0 * sp + self.d1 * w + self.e, self.l0 * sp * (w / sp).max(0.0).powf(self.p))
    }
}

#[derive(Clone, Copy)]
pub struct F3 {
    pub vev: f64,
    pub h: f64,
}
impl Aero for F3 {
    #[inline(always)]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let k = (self.vev * sp + self.h * w) * std::f64::consts::FRAC_1_SQRT_2;
        (k, k)
    }
}

// ------------------------------------------------------------------ 적분기

/// N 샷 동시 진행 RK4. 입력 `RunShot` = (v0', theta, S0, a [rad]), 출력 착지 (x, z). 정의역은 파일 머리 참고.
///
/// 착지는 매 걸음(첫 걸음 포함) '걸음 처음 y ≥ 0 이고 끝 y < 0' 으로 잡는다. 발사점이 y = 0 이라, 첫 걸음에 곧바로 땅 밑으로
/// 내려가는 샷(θ ≤ 0 이고 양력이 중력을 못 이김)의 착지는 발사점 (0, 0) 이다. 이전 판은 첫 걸음을 건너뛰고 둘째 걸음부터
/// y0 < 0 인 채로 에르미트 보간을 해서 땅 밑 점에서 엉뚱한 값(한 걸음 거리)을 냈다.
#[inline(always)]
pub fn fly<A: Aero, const N: usize>(ae: &A, s: &[[f64; 4]; N], dt: f64) -> [(f64, f64); N] {
    let mut st = [[0.0f64; N]; 7]; // x y z ux uy uz w
    let (mut oy, mut oz) = ([0.0; N], [0.0; N]);
    let mut out = [(f64::NAN, f64::NAN); N];
    let mut done = [false; N];
    let mut nd = 0;
    for i in 0..N {
        // 정의역 밖(유한하지 않음, v0' ≤ 0)은 (NaN, NaN) 으로 바로 끝낸다. 상태는 무해한 값으로 둔다.
        if !(s[i].iter().all(|x| x.is_finite()) && s[i][0] > 0.0) {
            (st[3][i], st[4][i], oz[i]) = (1.0, 0.3, 1.0);
            done[i] = true;
            nd += 1;
            continue;
        }
        // 스핀 부호는 접는다: S0 < 0 은 같은 크기의 반대 회전 (축 π 회전 = 사인·코사인 부호 반전). S0 ≥ 0 이면 ±1.0 곱이 값을 안 바꾼다.
        let sg = if s[i][2] < 0.0 { -1.0 } else { 1.0 };
        st[3][i] = s[i][0] * s[i][1].cos();
        st[4][i] = s[i][0] * s[i][1].sin();
        st[6][i] = s[i][2].abs() * s[i][0];
        oy[i] = sg * s[i][3].sin();
        oz[i] = sg * s[i][3].cos();
    }
    #[inline(always)]
    fn d<A: Aero>(ae: &A, ux: f64, uy: f64, uz: f64, w: f64, oy: f64, oz: f64) -> [f64; 4] {
        let sp = (ux * ux + uy * uy + uz * uz).sqrt();
        let (kd, kl) = ae.kk(sp, w);
        let (cx, cy, cz) = (oy * uz - oz * uy, oz * ux, -oy * ux);
        [-kd * ux + kl * cx, -kd * uy + kl * cy - 1.0, -kd * uz + kl * cz, -ae.beta() * w * sp]
    }
    assert!(dt.is_finite() && dt > 0.0, "fly: 걸음 dt 는 0 보다 큰 유한한 값이어야 한다 (dt = {dt})");
    let (h, s6) = (0.5 * dt, dt / 6.0);
    let mut steps = 0u32;
    while nd < N && steps < ((8.0 / dt) as u32).saturating_add(2) {
        let mut nx = [[0.0f64; N]; 7];
        for i in 0..N {
            let (ux, uy, uz, w) = (st[3][i], st[4][i], st[5][i], st[6][i]);
            let k1 = d(ae, ux, uy, uz, w, oy[i], oz[i]);
            let (a2, b2, c2, w2) = (ux + h * k1[0], uy + h * k1[1], uz + h * k1[2], w + h * k1[3]);
            let k2 = d(ae, a2, b2, c2, w2, oy[i], oz[i]);
            let (a3, b3, c3, w3) = (ux + h * k2[0], uy + h * k2[1], uz + h * k2[2], w + h * k2[3]);
            let k3 = d(ae, a3, b3, c3, w3, oy[i], oz[i]);
            let (a4, b4, c4, w4) = (ux + dt * k3[0], uy + dt * k3[1], uz + dt * k3[2], w + dt * k3[3]);
            let k4 = d(ae, a4, b4, c4, w4, oy[i], oz[i]);
            nx[0][i] = st[0][i] + s6 * (ux + 2.0 * a2 + 2.0 * a3 + a4);
            nx[1][i] = st[1][i] + s6 * (uy + 2.0 * b2 + 2.0 * b3 + b4);
            nx[2][i] = st[2][i] + s6 * (uz + 2.0 * c2 + 2.0 * c3 + c4);
            nx[3][i] = ux + s6 * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]);
            nx[4][i] = uy + s6 * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]);
            nx[5][i] = uz + s6 * (k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2]);
            nx[6][i] = w + s6 * (k1[3] + 2.0 * k2[3] + 2.0 * k3[3] + k4[3]);
        }
        steps += 1;
        for i in 0..N {
            if !done[i] && st[1][i] >= 0.0 && nx[1][i] < 0.0 {
                let (y0, y1, m0, m1) = (st[1][i], nx[1][i], st[4][i] * dt, nx[4][i] * dt);
                let t = if y0 <= 0.0 && m0 <= 0.0 {
                    // 발사점(y = 0)에서 곧바로 내려감 (θ ≤ 0): 착지는 발사점. 아래 뉴턴은 f(0) = 0, f'(0) = m0 ≤ 0 이라 0/0 이 될 수 있다.
                    0.0
                } else {
                    // y0 = 0 이어도 m0 > 0 이면 걸음 안에서 올라갔다 내려오므로 t = 0 이 아닌 근을 위(t = 1)에서 찾는다
                    let mut t = if y0 > 0.0 { y0 / (y0 - y1) } else { 1.0 };
                    for _ in 0..4 {
                        let (t2, t3) = (t * t, t * t * t);
                        let f = (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * m1;
                        let df = (6.0 * t2 - 6.0 * t) * y0 + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (-6.0 * t2 + 6.0 * t) * y1 + (3.0 * t2 - 2.0 * t) * m1;
                        // clamp 는 NaN 을 그대로 돌려준다 (stream 의 `F::clamp01` 도 같다)
                        t = (t - f / df).clamp(0.0, 1.0);
                    }
                    t
                };
                let (t2, t3) = (t * t, t * t * t);
                let hh = [2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2];
                out[i] = (
                    hh[0] * st[0][i] + hh[1] * st[3][i] * dt + hh[2] * nx[0][i] + hh[3] * nx[3][i] * dt,
                    hh[0] * st[2][i] + hh[1] * st[5][i] * dt + hh[2] * nx[2][i] + hh[3] * nx[5][i] * dt,
                );
                done[i] = true;
                nd += 1;
            }
        }
        st = nx;
    }
    out
}

/// f64 기준 엔진: `RunShot` (스핀축 기울기 α 는 **각도 rad**; `stream` 은 sin α) 들의 착지 (x, z), 무차원.
pub fn run<A: Aero>(ae: &A, shots: &[RunShot], dt: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(shots.len());
    for ch in shots.chunks(8) {
        let mut a = [ch[0]; 8];
        a[..ch.len()].copy_from_slice(ch);
        out.extend_from_slice(&fly::<A, 8>(ae, &a, dt)[..ch.len()]);
    }
    out
}

pub fn run_par<A: Aero>(ae: &A, shots: &[RunShot], dt: f64, threads: usize) -> Vec<(f64, f64)> {
    let per = shots.len().div_ceil(threads.max(1)).div_ceil(8) * 8;
    let mut out = vec![(0.0, 0.0); shots.len()];
    std::thread::scope(|sc| {
        for (p, r) in shots.chunks(per.max(8)).zip(out.chunks_mut(per.max(8))) {
            sc.spawn(move || r.copy_from_slice(&run(ae, p, dt)));
        }
    });
    out
}

// ------------------------------------------------------------------ 빠른 엔진: f32 × 16 레인 (AVX-512 레지스터 직접), rsqrt, 레인 재충전
//
// 병목 네 가지를 없앤다.
//  1) sqrt·나눗셈 지연 사슬: |u| = q·rsqrt(q), |u|C_L = c w |u| · rsqrt(q + λ²w²). 나눗셈 0, sqrt 0, rsqrt 2개(+뉴턴 1회).
//  2) 레인 대기: 8샷 묶음은 가장 늦게 떨어지는 공을 모두 기다린다. 공이 떨어진 레인에 다음 샷을 바로 채운다.
//  3) 폭: zmm 한 개에 f32 16개. 착지 거리 200 yd 에서 f32 반올림은 1e-4 yd 아래라 정확도 목표를 해치지 않는다.
//  4) 컴파일러 자동 벡터화가 배열 클로저를 ymm 반쪽·memcpy·함수 호출로 쪼갰다(측정 14 ns/샷·걸음).
//     상태 9개를 __m512 값으로 레지스터에 묶어 둔다.

// 판본 셋, 같은 16레인 API(F): x86_64 + AVX-512 는 zmm 한 개, aarch64 는 NEON q 레지스터 4개,
// 그 밖(AVX-512 없는 x86 등)은 배열 16칸(컴파일러 자동 벡터화). stream 은 AVX-512 판과 이식판 둘.

pub const WL: usize = 16;

#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
#[allow(unused_unsafe)]
pub mod simd {
    use std::arch::x86_64::*;
    use std::ops::{Add, Mul, Neg, Sub};
    #[derive(Clone, Copy)]
    pub struct F(pub __m512);
    impl F {
        #[inline(always)]
        pub fn splat(x: f32) -> F {
            F(unsafe { _mm512_set1_ps(x) })
        }
        #[inline(always)]
        pub fn load(a: &[f32; 16]) -> F {
            F(unsafe { _mm512_loadu_ps(a.as_ptr()) })
        }
        #[inline(always)]
        pub fn store(self, a: &mut [f32; 16]) {
            unsafe { _mm512_storeu_ps(a.as_mut_ptr(), self.0) }
        }
        /// a*b + c
        #[inline(always)]
        pub fn fma(a: F, b: F, c: F) -> F {
            F(unsafe { _mm512_fmadd_ps(a.0, b.0, c.0) })
        }
        /// 1/sqrt(x): rsqrt14 + 뉴턴 1회 (상대오차 ~1e-8)
        #[inline(always)]
        pub fn rsqrt(self) -> F {
            unsafe {
                let r = _mm512_rsqrt14_ps(self.0);
                let t = _mm512_mul_ps(_mm512_mul_ps(_mm512_set1_ps(0.5), self.0), _mm512_mul_ps(r, r));
                F(_mm512_mul_ps(r, _mm512_sub_ps(_mm512_set1_ps(1.5), t)))
            }
        }
        /// c - a*b
        #[inline(always)]
        pub fn fnma(a: F, b: F, c: F) -> F {
            F(unsafe { _mm512_fnmadd_ps(a.0, b.0, c.0) })
        }
        /// a*b - c
        #[inline(always)]
        pub fn fms(a: F, b: F, c: F) -> F {
            F(unsafe { _mm512_fmsub_ps(a.0, b.0, c.0) })
        }
        /// 1/x 근사 (상대오차 2^-14)
        #[inline(always)]
        pub fn rcp14(self) -> F {
            F(unsafe { _mm512_rcp14_ps(self.0) })
        }
        /// 1/sqrt(x) 근사 (상대오차 2^-14). 걸음 재매개에만 쓴다: 그 값이 틀려도 궤적은 같은 곡선이다.
        #[inline(always)]
        pub fn rsqrt14(self) -> F {
            F(unsafe { _mm512_rsqrt14_ps(self.0) })
        }
        #[inline(always)]
        pub fn neg_mask(self) -> u16 {
            unsafe { _mm512_cmp_ps_mask::<_CMP_LT_OQ>(self.0, _mm512_setzero_ps()) }
        }
        /// x ≥ 0 인 레인 (NaN 은 거짓). 착지 판정에서 '걸음 처음 y ≥ 0' 을 본다.
        #[inline(always)]
        pub fn ge0_mask(self) -> u16 {
            unsafe { _mm512_cmp_ps_mask::<_CMP_GE_OQ>(self.0, _mm512_setzero_ps()) }
        }
        /// x ≤ 0 인 레인 (NaN 은 거짓)
        #[inline(always)]
        pub fn le0_mask(self) -> u16 {
            unsafe { _mm512_cmp_ps_mask::<_CMP_LE_OQ>(self.0, _mm512_setzero_ps()) }
        }
        /// [0, 1] 로 자르기. NaN 은 NaN 으로 남는다 (f64 `clamp` 와 같다).
        /// x86 max/min 은 어느 쪽이든 NaN 이면 **둘째** 피연산자를 돌려주므로, 검사할 값을 둘째 자리에 둔다
        /// (첫째 자리에 두면 NaN 이 0 으로 바뀌어 f64 판과 어긋난다).
        #[inline(always)]
        pub fn clamp01(self) -> F {
            unsafe { F(_mm512_min_ps(_mm512_set1_ps(1.0), _mm512_max_ps(_mm512_setzero_ps(), self.0))) }
        }
        #[inline(always)]
        pub fn to_array(self) -> [f32; 16] {
            let mut a = [0f32; 16];
            self.store(&mut a);
            a
        }
    }
    impl Add for F {
        type Output = F;
        #[inline(always)]
        fn add(self, o: F) -> F {
            F(unsafe { _mm512_add_ps(self.0, o.0) })
        }
    }
    impl Sub for F {
        type Output = F;
        #[inline(always)]
        fn sub(self, o: F) -> F {
            F(unsafe { _mm512_sub_ps(self.0, o.0) })
        }
    }
    impl Mul for F {
        type Output = F;
        #[inline(always)]
        fn mul(self, o: F) -> F {
            F(unsafe { _mm512_mul_ps(self.0, o.0) })
        }
    }
    impl Neg for F {
        type Output = F;
        #[inline(always)]
        fn neg(self) -> F {
            F(unsafe { _mm512_sub_ps(_mm512_setzero_ps(), self.0) })
        }
    }
}

/// aarch64 (애플 실리콘 등): q 레지스터 4개로 16레인. 근사 역수·역제곱근은 추정 명령(약 8비트) + 뉴턴 1회로
/// AVX-512 의 rcp14·rsqrt14(14비트) 이상을 맞춘다.
#[cfg(target_arch = "aarch64")]
#[allow(unused_unsafe)]
pub mod simd {
    use std::arch::aarch64::*;
    use std::ops::{Add, Mul, Neg, Sub};
    #[derive(Clone, Copy)]
    pub struct F(pub [float32x4_t; 4]);
    /// 비트마스크 4비트 → 레인 마스크
    const LUT: [[u32; 4]; 16] = {
        let mut t = [[0u32; 4]; 16];
        let mut i = 0;
        while i < 16 {
            let mut j = 0;
            while j < 4 {
                t[i][j] = if (i >> j) & 1 == 1 { u32::MAX } else { 0 };
                j += 1;
            }
            i += 1;
        }
        t
    };
    impl F {
        #[inline(always)]
        fn map(self, f: impl Fn(float32x4_t) -> float32x4_t) -> F {
            F([f(self.0[0]), f(self.0[1]), f(self.0[2]), f(self.0[3])])
        }
        #[inline(always)]
        fn zip(a: F, b: F, f: impl Fn(float32x4_t, float32x4_t) -> float32x4_t) -> F {
            F([f(a.0[0], b.0[0]), f(a.0[1], b.0[1]), f(a.0[2], b.0[2]), f(a.0[3], b.0[3])])
        }
        #[inline(always)]
        pub fn splat(x: f32) -> F {
            F([unsafe { vdupq_n_f32(x) }; 4])
        }
        #[inline(always)]
        pub fn load(a: &[f32; 16]) -> F {
            unsafe { F(std::array::from_fn(|c| vld1q_f32(a.as_ptr().add(4 * c)))) }
        }
        #[inline(always)]
        pub fn store(self, a: &mut [f32; 16]) {
            for c in 0..4 {
                unsafe { vst1q_f32(a.as_mut_ptr().add(4 * c), self.0[c]) }
            }
        }
        #[inline(always)]
        pub fn to_array(self) -> [f32; 16] {
            let mut a = [0f32; 16];
            self.store(&mut a);
            a
        }
        #[inline(always)]
        pub fn from_array(a: [f32; 16]) -> F {
            F::load(&a)
        }
        /// a*b + c
        #[inline(always)]
        pub fn fma(a: F, b: F, c: F) -> F {
            F([0, 1, 2, 3].map(|i| unsafe { vfmaq_f32(c.0[i], a.0[i], b.0[i]) }))
        }
        /// c - a*b
        #[inline(always)]
        pub fn fnma(a: F, b: F, c: F) -> F {
            F([0, 1, 2, 3].map(|i| unsafe { vfmsq_f32(c.0[i], a.0[i], b.0[i]) }))
        }
        /// a*b - c
        #[inline(always)]
        pub fn fms(a: F, b: F, c: F) -> F {
            F([0, 1, 2, 3].map(|i| unsafe { vnegq_f32(vfmsq_f32(c.0[i], a.0[i], b.0[i])) }))
        }
        /// 1/x: 추정 + 뉴턴 1회 (약 16비트)
        #[inline(always)]
        pub fn rcp14(self) -> F {
            self.map(|x| unsafe {
                let e = vrecpeq_f32(x);
                vmulq_f32(e, vrecpsq_f32(x, e))
            })
        }
        /// 1/sqrt(x): 추정 + 뉴턴 1회 (약 16비트). 걸음 재매개에만 쓴다.
        #[inline(always)]
        pub fn rsqrt14(self) -> F {
            self.map(|x| unsafe {
                let e = vrsqrteq_f32(x);
                vmulq_f32(e, vrsqrtsq_f32(vmulq_f32(x, e), e))
            })
        }
        /// 1/sqrt(x): 뉴턴 2회 (f32 정밀도)
        #[inline(always)]
        pub fn rsqrt(self) -> F {
            self.map(|x| unsafe {
                let mut e = vrsqrteq_f32(x);
                e = vmulq_f32(e, vrsqrtsq_f32(vmulq_f32(x, e), e));
                vmulq_f32(e, vrsqrtsq_f32(vmulq_f32(x, e), e))
            })
        }
        #[inline(always)]
        pub fn div(self, o: F) -> F {
            F::zip(self, o, |a, b| unsafe { vdivq_f32(a, b) })
        }
        #[inline(always)]
        pub fn min(self, o: F) -> F {
            F::zip(self, o, |a, b| unsafe { vminq_f32(a, b) })
        }
        #[inline(always)]
        pub fn max(self, o: F) -> F {
            F::zip(self, o, |a, b| unsafe { vmaxq_f32(a, b) })
        }
        #[inline(always)]
        pub fn neg_mask(self) -> u16 {
            unsafe {
                let bits = vld1q_u32([1u32, 2, 4, 8].as_ptr());
                let mut m = 0u16;
                for c in 0..4 {
                    m |= (vaddvq_u32(vandq_u32(vcltzq_f32(self.0[c]), bits)) as u16) << (4 * c);
                }
                m
            }
        }
        /// 마스크 m 의 레인은 a, 나머지는 b
        #[inline(always)]
        pub fn select(m: u16, a: F, b: F) -> F {
            F([0, 1, 2, 3].map(|c| unsafe { vbslq_f32(vld1q_u32(LUT[((m >> (4 * c)) & 15) as usize].as_ptr()), a.0[c], b.0[c]) }))
        }
    }
    impl Add for F {
        type Output = F;
        #[inline(always)]
        fn add(self, o: F) -> F {
            F::zip(self, o, |a, b| unsafe { vaddq_f32(a, b) })
        }
    }
    impl Sub for F {
        type Output = F;
        #[inline(always)]
        fn sub(self, o: F) -> F {
            F::zip(self, o, |a, b| unsafe { vsubq_f32(a, b) })
        }
    }
    impl Mul for F {
        type Output = F;
        #[inline(always)]
        fn mul(self, o: F) -> F {
            F::zip(self, o, |a, b| unsafe { vmulq_f32(a, b) })
        }
    }
    impl Neg for F {
        type Output = F;
        #[inline(always)]
        fn neg(self) -> F {
            self.map(|a| unsafe { vnegq_f32(a) })
        }
    }
}

/// 그 밖의 대상: 배열 16칸. 역수·역제곱근은 정확한 값(근사보다 느리지만 결과는 같은 곡선).
#[cfg(not(any(all(target_arch = "x86_64", target_feature = "avx512f"), target_arch = "aarch64")))]
pub mod simd {
    use std::ops::{Add, Mul, Neg, Sub};
    #[derive(Clone, Copy)]
    #[repr(align(64))]
    pub struct F(pub [f32; 16]);
    impl F {
        #[inline(always)]
        fn map(self, f: impl Fn(f32) -> f32) -> F {
            F(self.0.map(f))
        }
        #[inline(always)]
        fn zip(a: F, b: F, f: impl Fn(f32, f32) -> f32) -> F {
            F(std::array::from_fn(|i| f(a.0[i], b.0[i])))
        }
        #[inline(always)]
        pub fn splat(x: f32) -> F {
            F([x; 16])
        }
        #[inline(always)]
        pub fn load(a: &[f32; 16]) -> F {
            F(*a)
        }
        #[inline(always)]
        pub fn store(self, a: &mut [f32; 16]) {
            *a = self.0;
        }
        #[inline(always)]
        pub fn to_array(self) -> [f32; 16] {
            self.0
        }
        #[inline(always)]
        pub fn from_array(a: [f32; 16]) -> F {
            F(a)
        }
        #[inline(always)]
        pub fn fma(a: F, b: F, c: F) -> F {
            F(std::array::from_fn(|i| a.0[i] * b.0[i] + c.0[i]))
        }
        #[inline(always)]
        pub fn fnma(a: F, b: F, c: F) -> F {
            F(std::array::from_fn(|i| c.0[i] - a.0[i] * b.0[i]))
        }
        #[inline(always)]
        pub fn fms(a: F, b: F, c: F) -> F {
            F(std::array::from_fn(|i| a.0[i] * b.0[i] - c.0[i]))
        }
        #[inline(always)]
        pub fn rcp14(self) -> F {
            self.map(|x| 1.0 / x)
        }
        #[inline(always)]
        pub fn rsqrt14(self) -> F {
            self.map(|x| 1.0 / x.sqrt())
        }
        #[inline(always)]
        pub fn rsqrt(self) -> F {
            self.map(|x| 1.0 / x.sqrt())
        }
        #[inline(always)]
        pub fn div(self, o: F) -> F {
            F::zip(self, o, |a, b| a / b)
        }
        /// 어느 쪽이든 NaN 이면 NaN (NEON vminq_f32 와 같다; `f32::min` 은 NaN 을 버려서 f64 `clamp` 와 어긋난다)
        #[inline(always)]
        pub fn min(self, o: F) -> F {
            F::zip(self, o, |a, b| if a.is_nan() || b.is_nan() { f32::NAN } else { a.min(b) })
        }
        /// 어느 쪽이든 NaN 이면 NaN (NEON vmaxq_f32 와 같다)
        #[inline(always)]
        pub fn max(self, o: F) -> F {
            F::zip(self, o, |a, b| if a.is_nan() || b.is_nan() { f32::NAN } else { a.max(b) })
        }
        #[inline(always)]
        pub fn neg_mask(self) -> u16 {
            (0..16).fold(0u16, |m, i| m | (((self.0[i] < 0.0) as u16) << i))
        }
        #[inline(always)]
        pub fn select(m: u16, a: F, b: F) -> F {
            F(std::array::from_fn(|i| if (m >> i) & 1 == 1 { a.0[i] } else { b.0[i] }))
        }
    }
    impl Add for F {
        type Output = F;
        #[inline(always)]
        fn add(self, o: F) -> F {
            F::zip(self, o, |a, b| a + b)
        }
    }
    impl Sub for F {
        type Output = F;
        #[inline(always)]
        fn sub(self, o: F) -> F {
            F::zip(self, o, |a, b| a - b)
        }
    }
    impl Mul for F {
        type Output = F;
        #[inline(always)]
        fn mul(self, o: F) -> F {
            F::zip(self, o, |a, b| a * b)
        }
    }
    impl Neg for F {
        type Output = F;
        #[inline(always)]
        fn neg(self) -> F {
            self.map(|a| -a)
        }
    }
}
pub use simd::F;

pub trait FastAero: Copy + Send + Sync {
    type P: Copy;
    fn prep(&self) -> Self::P;
    /// q = |u|^2 에서 (|u|, |u|C_D, |u|C_L)
    fn kk(p: &Self::P, q: F, w: F) -> (F, F, F);
}

/// sqrt(q) = q·rsqrt(q), 뉴턴 1회를 곱에 접어 넣는다: sp = (q r)(1.5 - (q r)(r/2))
#[inline(always)]
pub fn sqrt_nr(q: F) -> F {
    let r = q.rsqrt14();
    let qr = q * r;
    qr * F::fnma(qr, F::splat(0.5) * r, F::splat(1.5))
}

impl FastAero for H2 {
    type P = [F; 5];
    fn prep(&self) -> [F; 5] {
        [self.v, self.d, self.c, self.lam * self.lam, self.e].map(|x| F::splat(x as f32))
    }
    #[inline(always)]
    fn kk(p: &[F; 5], q: F, w: F) -> (F, F, F) {
        let sp = sqrt_nr(q);
        let r2 = F::fma(p[3] * w, w, q).rsqrt();
        (sp, F::fma(p[0], sp, F::fma(p[1], w, p[4])), p[2] * w * sp * r2)
    }
}

impl FastAero for Shared {
    type P = [F; 4];
    fn prep(&self) -> [F; 4] {
        [self.v, self.d, self.c, self.l].map(|x| F::splat(x as f32))
    }
    #[inline(always)]
    fn kk(p: &[F; 4], q: F, w: F) -> (F, F, F) {
        let sp = sqrt_nr(q);
        // r = w sp / (sp + l w): 역수는 rcp14 + 뉴턴 1회
        let den = F::fma(p[3], w, sp);
        let r0 = den.rcp14();
        let inv = r0 * F::fnma(den, r0, F::splat(2.0));
        let r = w * sp * inv;
        (sp, F::fma(p[0], sp, p[1] * r), p[2] * r)
    }
}

impl<const N: i32> FastAero for SharedRe<N> {
    type P = [F; 6];
    fn prep(&self) -> [F; 6] {
        [self.v, self.e, 1.0 / (self.u0 * self.u0), self.d, self.c, self.l].map(|x| F::splat(x as f32))
    }
    #[inline(always)]
    fn kk(p: &[F; 6], q: F, w: F) -> (F, F, F) {
        let sp = sqrt_nr(q);
        let one = F::splat(1.0);
        let two = F::splat(2.0);
        // 역수 두 개를 rcp14 + 뉴턴 1회로: 1/(sp + l w), 1/(1 + (q/u0^2)^(N/2))
        let den = F::fma(p[5], w, sp);
        let a0 = den.rcp14();
        let inv = a0 * F::fnma(den, a0, two);
        let x = q * p[2];
        // x^(N/2): N 이 상수라 곱 몇 번으로 접힌다
        let xn = match N {
            4 => x * x,
            12 => {
                let x3 = x * x * x;
                x3 * x3
            }
            _ => (1..N / 2).fold(x, |a, _| a * x),
        };
        let den2 = xn + one;
        let b0 = den2.rcp14();
        let inv2 = b0 * F::fnma(den2, b0, two);
        let r = w * sp * inv;
        let cd0 = F::fma(p[1], inv2, p[0]);
        (sp, F::fma(cd0, sp, p[3] * r), p[4] * r)
    }
}

impl FastAero for F3 {
    type P = [F; 2];
    fn prep(&self) -> [F; 2] {
        [self.vev, self.h].map(|x| F::splat((x * std::f64::consts::FRAC_1_SQRT_2) as f32))
    }
    #[inline(always)]
    fn kk(p: &[F; 2], q: F, w: F) -> (F, F, F) {
        let sp = sqrt_nr(q);
        let k = F::fma(p[0], sp, p[1] * w);
        (sp, k, k)
    }
}

/// 걸음 재매개 dτ = |u|^{1/2} dt 에서의 도함수 [x' y' z' ux' uy' uz' w'] (모두 × g = |u|^{-1/2}).
/// 같은 평균 걸음 수에서 최대 오차가 시간 걸음보다 약 10배 작다(무작위 3000샷, 지수 0.3–0.75 훑어 0.5 선택).
#[inline(always)]
pub fn deriv16<A: FastAero>(p: &A::P, ux: F, uy: F, uz: F, w: F, oy: F, oz: F) -> [F; 7] {
    let q = F::fma(ux, ux, F::fma(uy, uy, uz * uz));
    let (sp, kd, kl) = A::kk(p, q, w);
    let g = sp.rsqrt14();
    let zero = F::splat(0.0);
    let (klz, kly) = (kl * oz, kl * oy);
    let ax = F::fnma(kd, ux, kl * F::fms(oy, uz, oz * uy));
    let ay = F::fnma(kd, uy, F::fms(klz, ux, F::splat(1.0)));
    let az = F::fma(kd, uz, kly * ux); // 부호는 아래에서
    [
        ux * g,
        uy * g,
        uz * g,
        ax * g,
        ay * g,
        F::fnma(az, g, zero),
        F::splat(-(BETA as f32)) * (w * sp) * g,
    ]
}

/// 한 RK4 걸음 (FSAL: 다음 걸음의 k1 을 돌려준다). 상태 [x y z ux uy uz w], 축 (oy, oz)
#[inline(always)]
pub fn rk4_16<A: FastAero>(p: &A::P, s: &[F; 7], k1: &[F; 7], oy: F, oz: F, hh: F, h: F, h6: F) -> ([F; 7], [F; 7]) {
    let two = F::splat(2.0);
    let k2 = deriv16::<A>(p, F::fma(hh, k1[3], s[3]), F::fma(hh, k1[4], s[4]), F::fma(hh, k1[5], s[5]), F::fma(hh, k1[6], s[6]), oy, oz);
    let k3 = deriv16::<A>(p, F::fma(hh, k2[3], s[3]), F::fma(hh, k2[4], s[4]), F::fma(hh, k2[5], s[5]), F::fma(hh, k2[6], s[6]), oy, oz);
    let k4 = deriv16::<A>(p, F::fma(h, k3[3], s[3]), F::fma(h, k3[4], s[4]), F::fma(h, k3[5], s[5]), F::fma(h, k3[6], s[6]), oy, oz);
    let nx: [F; 7] = std::array::from_fn(|j| F::fma(h6, F::fma(two, k2[j] + k3[j], k1[j] + k4[j]), s[j]));
    let kn = deriv16::<A>(p, nx[3], nx[4], nx[5], nx[6], oy, oz);
    (nx, kn)
}

/// 레인 묶음 G 개(각 16샷)를 한 고리에서 번갈아 돌려 지연 사슬을 숨긴다.
pub const G: usize = 1;

/// sin, cos (정의역 |x| <= pi/2, 테일러 11·12차; f32 반올림 포함 정의역 끝에서 오차 약 1.2e-7). 단순 산술이라 자동 벡터화된다.
/// 정의역 밖(|x| > pi/2)에서는 다항식이 갈라져 틀린 값이 나온다 — `stream` 은 그런 샷을 입력 단계에서 (NaN, NaN) 으로 걸러낸다.
#[inline(always)]
pub fn sincos_poly(x: f32) -> (f32, f32) {
    let x2 = x * x;
    let s = x * (1.0 + x2 * (-1.0 / 6.0 + x2 * (1.0 / 120.0 + x2 * (-1.0 / 5040.0 + x2 * (1.0 / 362880.0 + x2 * (-1.0 / 39916800.0))))));
    let c = 1.0 + x2 * (-0.5 + x2 * (1.0 / 24.0 + x2 * (-1.0 / 720.0 + x2 * (1.0 / 40320.0 + x2 * (-1.0 / 3628800.0 + x2 * (1.0 / 479001600.0))))));
    (s, c)
}

/// `stream` 입력이 정의역 안인가: 모두 유한, v0' > 0, |θ| ≤ π/2 (`sincos_poly` 의 정의역), |sin α| ≤ 1 (cos α = √(1 − sin²α) 가 실수).
/// 밖이면 `stream` 은 그 샷의 결과를 (NaN, NaN) 으로 낸다 (조용히 NaN 레인으로 max_age 까지 돌지 않는다).
#[inline]
pub fn stream_shot_ok(s: &StreamShot) -> bool {
    s.iter().all(|x| x.is_finite()) && s[0] > 0.0 && s[1].abs() <= std::f32::consts::FRAC_PI_2 && s[3].abs() <= 1.0
}

/// 착지한 레인을 모아서 처리하는 개수 K (환경변수 BATCH_K, 기본 3, 1..=16). 프로세스에서 한 번만 읽어 둔다.
/// 잘못된 값은 조용히 넘기지 않고 중단한다: K ≥ 17 이면 레인 16개로는 K 개가 영영 모이지 않아 `stream` 이 끝나지 않는다.
pub fn batch_k() -> usize {
    static K: OnceLock<usize> = OnceLock::new();
    *K.get_or_init(|| match parse_batch_k(std::env::var("BATCH_K").ok().as_deref()) {
        Ok(k) => k,
        Err(msg) => panic!("{msg}"),
    })
}

/// BATCH_K 값 해석 (환경을 건드리지 않는 순수 함수라 시험할 수 있다). None 이면 기본 3.
pub fn parse_batch_k(v: Option<&str>) -> Result<usize, String> {
    let Some(v) = v else { return Ok(3) };
    match v.trim().parse::<usize>() {
        Ok(k) if (1..=WL).contains(&k) => Ok(k),
        _ => Err(format!("환경변수 BATCH_K='{v}' 는 1..={WL} 의 정수여야 한다 (17 이상이면 레인 {WL}개로는 영영 모이지 않아 stream 이 끝나지 않는다)")),
    }
}

thread_local! {
    /// stream 의 SoA 초기 상태 완충 (호출마다 Vec 여섯 개를 새로 만들지 않도록 스레드마다 한 번 잡아 재사용한다)
    static SOA: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
}

/// 빠른 엔진: `StreamShot` = (v0', theta, S0, **sin a**) → 착지 [x, z] (무차원). h 는 τ 걸음.
/// (`run`/`fly` 의 `RunShot` 은 a 를 각도로 받는다. 바꾸기: `to_stream_shot`.) 정의역은 파일 머리와 `stream_shot_ok` 참고.
/// 착지·재충전도 벡터로 한다: 에르미트 보간은 16레인 전체에 한 번, 결과는 마스크 scatter,
/// 새 샷은 SoA 입력에서 마스크 expand-load (떨어진 레인에만 다음 샷이 차례로 들어간다).
pub fn stream<A: FastAero>(ae: &A, shots: &[StreamShot], h: f32, out: &mut [[f32; 2]]) {
    stream_k(ae, shots, h, out, batch_k())
}

/// `stream` 에 모으는 개수 K 를 직접 준다 (1..=16 이 아니면 중단).
pub fn stream_k<A: FastAero>(ae: &A, shots: &[StreamShot], h: f32, out: &mut [[f32; 2]], batch_k: usize) {
    assert!((1..=WL).contains(&batch_k), "stream: batch_k = {batch_k} 는 1..={WL} 이어야 한다 (17 이상이면 stream 이 끝나지 않는다)");
    assert!(h.is_finite() && h > 0.0, "stream: 걸음 h 는 0 보다 큰 유한한 값이어야 한다 (h = {h})");
    let n = shots.len();
    if n == 0 {
        return;
    }
    assert!(n < i32::MAX as usize / 2 && out.len() == n, "stream: 샷 {n} 개에 out 길이 {} (같아야 한다)", out.len());
    SOA.with(|cell| {
        let mut buf = cell.borrow_mut();
        // SoA 초기 상태 5줄 (ux0, uy0, w0, oy, oz), 줄 간격 stride. 끝에 16칸 여유(마스크 expand-load 가 읽는 끝)
        let stride = n + WL;
        buf.clear();
        buf.resize(5 * stride, 0.0);
        let mut bad: Vec<usize> = Vec::new();
        for (k, s) in shots.iter().enumerate() {
            if !stream_shot_ok(s) {
                // 정의역 밖: 무해한 샷으로 바꿔 날리고 마지막에 (NaN, NaN) 으로 덮는다
                bad.push(k);
                (buf[k], buf[stride + k], buf[4 * stride + k]) = (1.0, 0.3, 1.0);
                continue;
            }
            let (sn, cs) = sincos_poly(s[1]);
            // 스핀 부호는 접는다 (fly 와 같다): S0 < 0 은 반대 회전 = 축 π 회전. S0 ≥ 0 이면 ±1.0 곱이 값을 안 바꾼다.
            let sg = if s[2] < 0.0 { -1.0 } else { 1.0 };
            buf[k] = s[0] * cs;
            buf[stride + k] = s[0] * sn;
            buf[2 * stride + k] = s[2].abs() * s[0];
            buf[3 * stride + k] = sg * s[3];
            buf[4 * stride + k] = sg * (1.0 - s[3] * s[3]).sqrt();
        }
        stream_core::<A>(ae, &buf, stride, n, h, out, batch_k);
        for k in bad {
            out[k] = [f32::NAN; 2];
        }
    })
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
#[allow(unused_unsafe)]
fn stream_core<A: FastAero>(ae: &A, init: &[f32], stride: usize, n: usize, h: f32, out: &mut [[f32; 2]], batch_k: usize) {
    use std::arch::x86_64::*;
    let p = ae.prep();
    let max_age = (8.0 / h) as i32 + 2;
    let (hh, hv, h6) = (F::splat(0.5 * h), F::splat(h), F::splat(h / 6.0));
    let zero = F::splat(0.0);
    unsafe {
        let iota = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
        let (one_i, maxa) = (_mm512_set1_epi32(1), _mm512_set1_epi32(max_age));
        let nanv = _mm512_set1_ps(f32::NAN);
        let outp = out.as_mut_ptr() as *mut f32;
        let mut next = 0usize;
        let mut st = [[zero; 7]; G];
        let mut k1 = [[zero; 7]; G];
        let (mut oy, mut oz) = ([zero; G], [F::splat(1.0); G]);
        let mut id = [_mm512_setzero_si512(); G];
        let mut age = [_mm512_setzero_si512(); G];
        let mut act = [0u16; G];
        let mut pend = [0u16; G];
        // 레인 채우기: 마스크 rm 의 레인에 다음 샷을 차례로
        macro_rules! refill {
            ($g:expr, $rm:expr) => {{
                let (g, rm) = ($g, $rm);
                // 줄 row 의 next 번째부터 popcount(rm) 개를 읽는다 (next + popcount ≤ n ≤ stride − 16 이라 줄 안)
                let ld = |row: usize, src: F| F(_mm512_mask_expandloadu_ps(src.0, rm, init.as_ptr().add(row * stride + next)));
                for k in [0usize, 1, 2, 5] {
                    st[g][k] = F(_mm512_mask_mov_ps(st[g][k].0, rm, zero.0));
                }
                st[g][3] = ld(0, st[g][3]);
                st[g][4] = ld(1, st[g][4]);
                st[g][6] = ld(2, st[g][6]);
                oy[g] = ld(3, oy[g]);
                oz[g] = ld(4, oz[g]);
                id[g] = _mm512_mask_expand_epi32(id[g], rm, _mm512_add_epi32(_mm512_set1_epi32(next as i32), iota));
                age[g] = _mm512_mask_mov_epi32(age[g], rm, _mm512_setzero_si512());
                next += rm.count_ones() as usize;
                act[g] |= rm;
            }};
        }
        for g in 0..G {
            let k = (n - next).min(WL);
            let rm = if k == WL { 0xFFFF } else { ((1u32 << k) - 1) as u16 };
            refill!(g, rm);
            // 빈 레인은 무해한 상태로
            if rm != 0xFFFF {
                st[g][3] = F(_mm512_mask_mov_ps(_mm512_set1_ps(1.0), rm, st[g][3].0));
                st[g][4] = F(_mm512_mask_mov_ps(_mm512_set1_ps(0.3), rm, st[g][4].0));
                oz[g] = F(_mm512_mask_mov_ps(_mm512_set1_ps(1.0), rm, oz[g].0));
            }
            k1[g] = deriv16::<A>(&p, st[g][3], st[g][4], st[g][5], st[g][6], oy[g], oz[g]);
        }
        while act.iter().any(|&m| m != 0) {
            let res: [([F; 7], [F; 7]); G] = std::array::from_fn(|g| rk4_16::<A>(&p, &st[g], &k1[g], oy[g], oz[g], hh, hv, h6));
            for g in 0..G {
                let (nx, kn) = res[g];
                age[g] = _mm512_add_epi32(age[g], one_i);
                let over = _mm512_mask_cmpgt_epi32_mask(act[g] & !pend[g], age[g], maxa);
                // 착지: 걸음 처음 y ≥ 0 이고 끝 y < 0 (첫 걸음 포함: θ ≤ 0 샷은 첫 걸음에서 내려간다). 시간 초과는 NaN 으로 내보낸다.
                pend[g] |= (nx[1].neg_mask() & act[g] & st[g][1].ge0_mask()) | over;
                let m = pend[g];
                if m == 0 {
                    // 빠른 길: 착지한 레인이 없으면 마스크 합치기 없이 그대로 다음 걸음 (이식판과 같은 길)
                    st[g] = nx;
                    k1[g] = kn;
                    continue;
                }
                // 착지한 레인은 얼려 두고(상태를 그대로 두면 다음 걸음이 같은 nx 를 다시 낸다) K 개가 모이면 한꺼번에 처리
                if (m.count_ones() as usize) < batch_k && next < n && over == 0 {
                    st[g] = std::array::from_fn(|k| F(_mm512_mask_mov_ps(nx[k].0, m, st[g][k].0)));
                    k1[g] = std::array::from_fn(|k| F(_mm512_mask_mov_ps(kn[k].0, m, k1[g][k].0)));
                    continue;
                }
                pend[g] = 0;
                // 에르미트 착지 (16레인 한 번에; 쓰는 건 m 레인뿐)
                let (y0, y1) = (st[g][1], nx[1]);
                let (m0, m1) = (k1[g][1] * hv, kn[1] * hv);
                let (c2, c3) = (F::splat(2.0), F::splat(3.0));
                // 시작값: y0 > 0 이면 선형 근. y0 ≤ 0 (발사점에서 시작한 첫 걸음)이면 t = 0 이 자명한 근이라 걸음 끝(t = 1)에서 찾는다
                let nonpos = y0.le0_mask();
                let mut t = F(_mm512_mask_blend_ps(nonpos, _mm512_div_ps(y0.0, (y0 - y1).0), _mm512_set1_ps(1.0)));
                for _ in 0..3 {
                    let (t2, t3) = (t * t, t * t * t);
                    let f = (c2 * t3 - c3 * t2 + F::splat(1.0)) * y0 + (t3 - c2 * t2 + t) * m0 + (c3 * t2 - c2 * t3) * y1 + (t3 - t2) * m1;
                    let df = (F::splat(6.0) * (t2 - t)) * (y0 - y1) + (c3 * t2 - F::splat(4.0) * t + F::splat(1.0)) * m0 + (c3 * t2 - c2 * t) * m1;
                    // NaN 은 NaN 으로 남는다 (fly 의 f64 clamp 와 같다)
                    t = (t - F(_mm512_div_ps(f.0, df.0))).clamp01();
                }
                // 발사점에서 곧바로 내려가는 레인 (y0 ≤ 0, m0 ≤ 0: θ ≤ 0)은 착지가 발사점. 뉴턴이 0/0 이 되는 자리라 t = 0 으로 못 박는다.
                t = F(_mm512_mask_mov_ps(t.0, nonpos & m0.le0_mask(), zero.0));
                let (t2, t3) = (t * t, t * t * t);
                let (b0, b1, b2, b3) = (c2 * t3 - c3 * t2 + F::splat(1.0), t3 - c2 * t2 + t, c3 * t2 - c2 * t3, t3 - t2);
                let xl = b0 * st[g][0] + b1 * k1[g][0] * hv + b2 * nx[0] + b3 * kn[0] * hv;
                let zl = b0 * st[g][2] + b1 * k1[g][2] * hv + b2 * nx[2] + b3 * kn[2] * hv;
                let (xl, zl) = (_mm512_mask_mov_ps(xl.0, over, nanv), _mm512_mask_mov_ps(zl.0, over, nanv));
                let i2 = _mm512_slli_epi32::<1>(id[g]);
                _mm512_mask_i32scatter_ps::<4>(outp, m, i2, xl);
                _mm512_mask_i32scatter_ps::<4>(outp.add(1), m, i2, zl);
                // 남은 샷 수만큼만 다시 채운다 (낮은 레인부터)
                let avail = n - next;
                let mut rm = m;
                while rm.count_ones() as usize > avail {
                    rm &= !(1u16 << (15 - rm.leading_zeros()));
                }
                act[g] &= !m;
                st[g] = nx;
                if rm != 0 {
                    refill!(g, rm);
                    let kr = deriv16::<A>(&p, st[g][3], st[g][4], st[g][5], st[g][6], oy[g], oz[g]);
                    k1[g] = std::array::from_fn(|k| F(_mm512_mask_mov_ps(kn[k].0, rm, kr[k].0)));
                } else {
                    k1[g] = kn;
                }
            }
        }
    }
}

/// 이식판 stream: AVX-512 판과 같은 순서·같은 식. 마스크 연산은 F::select 와 u16 비트마스크로,
/// expand-load·scatter 는 레인 고리로 한다(착지·재충전 때만 돌아 걸음당 비용은 거의 없다).
#[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
fn stream_core<A: FastAero>(ae: &A, init: &[f32], stride: usize, n: usize, h: f32, out: &mut [[f32; 2]], batch_k: usize) {
    let p = ae.prep();
    let max_age = (8.0 / h) as i32 + 2;
    let (hh, hv, h6) = (F::splat(0.5 * h), F::splat(h), F::splat(h / 6.0));
    let zero = F::splat(0.0);
    let mut next = 0usize;
    let mut st = [zero; 7];
    let (mut oy, mut oz) = (zero, F::splat(1.0));
    let mut id = [0usize; WL];
    let mut age = [0i32; WL];
    let mut act = 0u16;
    let mut pend = 0u16;
    // 레인 채우기: 마스크 rm 의 레인에 다음 샷을 차례로
    macro_rules! refill {
        ($rm:expr) => {{
            let rm: u16 = $rm;
            let mut s = st.map(F::to_array);
            let (mut ya, mut za) = (oy.to_array(), oz.to_array());
            for lane in 0..WL {
                if (rm >> lane) & 1 == 1 {
                    for k in [0usize, 1, 2, 5] {
                        s[k][lane] = 0.0;
                    }
                    s[3][lane] = init[next];
                    s[4][lane] = init[stride + next];
                    s[6][lane] = init[2 * stride + next];
                    ya[lane] = init[3 * stride + next];
                    za[lane] = init[4 * stride + next];
                    id[lane] = next;
                    age[lane] = 0;
                    next += 1;
                }
            }
            st = s.map(F::from_array);
            oy = F::from_array(ya);
            oz = F::from_array(za);
            act |= rm;
        }};
    }
    let k = n.min(WL);
    let rm = if k == WL { 0xFFFF } else { ((1u32 << k) - 1) as u16 };
    refill!(rm);
    if rm != 0xFFFF {
        // 빈 레인은 무해한 상태로
        st[3] = F::select(rm, st[3], F::splat(1.0));
        st[4] = F::select(rm, st[4], F::splat(0.3));
        oz = F::select(rm, oz, F::splat(1.0));
    }
    let mut k1 = deriv16::<A>(&p, st[3], st[4], st[5], st[6], oy, oz);
    let (one, c2, c3) = (F::splat(1.0), F::splat(2.0), F::splat(3.0));
    while act != 0 {
        let (nx, kn) = rk4_16::<A>(&p, &st, &k1, oy, oz, hh, hv, h6);
        let mut over = 0u16;
        for lane in 0..WL {
            age[lane] += 1;
            over |= ((age[lane] > max_age) as u16) << lane;
        }
        over &= act & !pend;
        // 착지: 걸음 처음 y ≥ 0 이고 끝 y < 0 (첫 걸음 포함). NaN 상태는 nx[1] 도 NaN 이라 착지로 안 잡힌다.
        let ge0 = !st[1].neg_mask();
        pend |= (nx[1].neg_mask() & act & ge0) | over;
        let m = pend;
        if m == 0 {
            // 빠른 길: 착지한 레인이 없으면 마스크 합치기 없이 그대로 다음 걸음
            (st, k1) = (nx, kn);
            continue;
        }
        // 착지한 레인은 얼려 두고 K 개가 모이면 한꺼번에 처리
        if (m.count_ones() as usize) < batch_k && next < n && over == 0 {
            st = std::array::from_fn(|k| F::select(m, st[k], nx[k]));
            k1 = std::array::from_fn(|k| F::select(m, k1[k], kn[k]));
            continue;
        }
        pend = 0;
        // 에르미트 착지 (16레인 한 번에; 쓰는 건 m 레인뿐)
        let (y0, y1) = (st[1], nx[1]);
        let (m0, m1) = (k1[1] * hv, kn[1] * hv);
        // 시작값: y0 > 0 이면 선형 근. y0 ≤ 0 (발사점에서 시작한 첫 걸음)이면 걸음 끝(t = 1)에서 찾는다
        let pos0 = (-y0).neg_mask(); // y0 > 0
        let mut t = F::select(pos0, y0.div(y0 - y1), one);
        for _ in 0..3 {
            let (t2, t3) = (t * t, t * t * t);
            let f = (c2 * t3 - c3 * t2 + one) * y0 + (t3 - c2 * t2 + t) * m0 + (c3 * t2 - c2 * t3) * y1 + (t3 - t2) * m1;
            let df = (F::splat(6.0) * (t2 - t)) * (y0 - y1) + (c3 * t2 - F::splat(4.0) * t + one) * m0 + (c3 * t2 - c2 * t) * m1;
            // F::max/min 은 NaN 을 전파한다 (AVX 판의 clamp01, fly 의 f64 clamp 와 같다)
            t = (t - f.div(df)).max(zero).min(one);
        }
        // 발사점에서 곧바로 내려가는 레인 (y0 ≤ 0, m0 ≤ 0)은 착지가 발사점: t = 0
        let flat = !pos0 & !(-m0).neg_mask();
        t = F::select(flat, zero, t);
        let (t2, t3) = (t * t, t * t * t);
        let (b0, b1, b2, b3) = (c2 * t3 - c3 * t2 + one, t3 - c2 * t2 + t, c3 * t2 - c2 * t3, t3 - t2);
        let xl = (b0 * st[0] + b1 * k1[0] * hv + b2 * nx[0] + b3 * kn[0] * hv).to_array();
        let zl = (b0 * st[2] + b1 * k1[2] * hv + b2 * nx[2] + b3 * kn[2] * hv).to_array();
        for lane in 0..WL {
            if (m >> lane) & 1 == 1 {
                out[id[lane]] = if (over >> lane) & 1 == 1 { [f32::NAN; 2] } else { [xl[lane], zl[lane]] };
            }
        }
        // 남은 샷 수만큼만 다시 채운다 (낮은 레인부터)
        let avail = n - next;
        let mut rm = m;
        while rm.count_ones() as usize > avail {
            rm &= !(1u16 << (15 - rm.leading_zeros()));
        }
        act &= !m;
        st = nx;
        if rm != 0 {
            refill!(rm);
            let kr = deriv16::<A>(&p, st[3], st[4], st[5], st[6], oy, oz);
            k1 = std::array::from_fn(|k| F::select(rm, kr[k], kn[k]));
        } else {
            k1 = kn;
        }
    }
}

pub fn stream_par<A: FastAero>(ae: &A, shots: &[StreamShot], h: f32, threads: usize) -> Vec<[f32; 2]> {
    let mut out = vec![[0f32; 2]; shots.len()];
    if threads <= 1 {
        stream(ae, shots, h, &mut out);
        return out;
    }
    let per = shots.len().div_ceil(threads).max(1);
    std::thread::scope(|sc| {
        for (p, r) in shots.chunks(per).zip(out.chunks_mut(per)) {
            sc.spawn(move || stream(ae, p, h, r));
        }
    });
    out
}

pub fn dimless(mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
    let v = mph * MPH;
    [v / U_MS, deg.to_radians(), R_BALL * rpm * std::f64::consts::TAU / 60.0 / v]
}

// ------------------------------------------------------------------ 자료

/// (볼스피드 mph, 발사각 deg, 스핀 rpm, 캐리 yd) — 앞 네 열만 쓰는 옛 표(h2.rs 의 파이썬 재현용).
/// 자료의 원본은 하나다: `harness::PGA12`/`LPGA11`. 4번째 행부터(3번 아이언 ~ 9번 아이언 구간) 잘라 오며, 여기에 숫자를 다시 적지 않는다.
pub const PGA: [[f64; 4]; 7] = tour_cols(&crate::harness::PGA12, 4);
pub const LPGA: [[f64; 4]; 6] = tour_cols(&crate::harness::LPGA11, 4);

/// 투어표 6열 중 앞 4열(mph, deg, rpm, 캐리)만 at 행부터 N 행 잘라 온다
const fn tour_cols<const N: usize, const M: usize>(src: &[[f64; 6]; M], at: usize) -> [[f64; 4]; N] {
    let mut out = [[0.0; 4]; N];
    let mut i = 0;
    while i < N {
        out[i] = [src[at + i][0], src[at + i][1], src[at + i][2], src[at + i][3]];
        i += 1;
    }
    out
}
/// USGA R22-01: (라벨, 볼스피드, sd, 발사각, sd, 스핀, sd, 캐리)
pub const USGA: [(&str, f64, f64, f64, f64, f64, f64, f64); 20] = [
    ("F 6i Pro", 112.9, 5.7, 15.1, 1.8, 5217.0, 678.0, 155.5), ("F 6i C1", 102.8, 8.2, 14.2, 2.9, 4685.0, 955.0, 132.0),
    ("F 6i C2", 90.5, 15.1, 15.1, 3.2, 4096.0, 1252.0, 106.7), ("F 6i C3", 77.6, 9.5, 13.7, 5.2, 3712.0, 1230.0, 75.1),
    ("F 6i C4", 71.2, 13.5, 14.3, 4.9, 3460.0, 1565.0, 64.5), ("F DR Pro", 127.6, 11.3, 14.3, 2.7, 2872.0, 910.0, 216.7),
    ("F DR C1", 122.2, 10.6, 14.1, 2.9, 3067.0, 1057.0, 173.6), ("F DR C2", 109.6, 9.8, 13.2, 4.6, 2784.0, 983.0, 135.8),
    ("F DR C3", 97.0, 9.1, 11.3, 3.9, 2671.0, 909.0, 100.1), ("F DR C4", 86.8, 13.9, 10.4, 5.1, 3095.0, 1392.0, 76.5),
    ("M 6i Pro", 125.0, 9.2, 12.8, 2.4, 6097.0, 645.0, 176.4), ("M 6i C1", 120.2, 9.9, 12.4, 3.0, 5171.0, 925.0, 164.4),
    ("M 6i C2", 112.2, 13.2, 12.1, 4.0, 4835.0, 1191.0, 145.5), ("M 6i C3", 105.9, 15.0, 12.5, 4.7, 4354.0, 1333.0, 129.9),
    ("M 6i C4", 103.8, 19.2, 11.7, 5.1, 4942.0, 1492.0, 119.2), ("M DR Pro", 169.0, 6.6, 11.1, 2.3, 2259.0, 1120.0, 262.2),
    ("M DR C1", 143.8, 9.9, 11.4, 2.8, 3274.0, 780.0, 215.1), ("M DR C2", 130.6, 13.1, 10.7, 3.6, 3313.0, 1140.0, 183.6),
    ("M DR C3", 121.9, 15.3, 12.2, 4.9, 3202.0, 1250.0, 163.3), ("M DR C4", 125.6, 16.6, 10.2, 4.8, 3839.0, 1309.0, 163.1),
];

// ------------------------------------------------------------------ 보조

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn uni(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
    pub fn gauss(&mut self) -> f64 {
        let (u1, u2) = (self.next().max(1e-300), self.next());
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

pub fn bench<F: FnMut() -> f64>(n: usize, reps: usize, mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t0 = Instant::now();
        black_box(f());
        best = best.min(t0.elapsed().as_nanos() as f64 / n as f64);
    }
    best
}

pub fn rmse(e: &[f64]) -> f64 {
    (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt()
}

pub fn carry_rows<A: Aero>(ae: &A, rows: &[[f64; 4]], dt: f64) -> Vec<f64> {
    let shots: Vec<[f64; 4]> = rows.iter().map(|r| { let d = dimless(r[0], r[1], r[2]); [d[0], d[1], d[2], 0.0] }).collect();
    run(ae, &shots, dt).iter().map(|p| p.0 * YD).collect()
}

// ------------------------------------------------------------------ 러스트 보정 (Levenberg–Marquardt)

/// 4×4 확대 행렬 풀이 (`linalg::solve_fixed` 의 4원 꼴; 옛 이름 유지)
pub fn solve4(a: [[f64; 5]; 4]) -> [f64; 4] {
    crate::linalg::solve_fixed::<4, 5>(a)
}

pub fn fit_h2(rows: &[[f64; 4]], start: [f64; 4], dt: f64) -> ([f64; 4], f64, usize) {
    let lo = [0.05, -1.0, 0.0, 0.0];
    let hi = [0.6, 2.0, 20.0, 80.0];
    let mut res = |p: &[f64]| -> Vec<f64> {
        let m = H2 { v: p[0], d: p[1], c: p[2], lam: p[3], e: 0.0 };
        carry_rows(&m, rows, dt).iter().zip(rows).map(|(c, r)| c - r[3]).collect()
    };
    // 공통 레벤버그-마쿼트(lm.rs). 감쇠 μ·max(A_xx, 1e-12), 안쪽 12번, 상대 개선 1e-12 이하에서 끝
    let cfg = crate::lm::LmCfg {
        lo: &lo,
        hi: &hi,
        step: crate::lm::Step::Rel { rel: 1e-6, floor: 1e-3 },
        damping: crate::lm::Damping::Diag,
        max_iter: 200,
        tries: 12,
        mu_min: 1e-12,
        stop_rel: 1e-12,
        nonfinite_cost: None,
    };
    let o = crate::lm::lm(&cfg, &start, &mut res, None);
    ([o.p[0], o.p[1], o.p[2], o.p[3]], o.cost, o.evals)
}

// ------------------------------------------------------------------ USGA 분포 평균

pub fn usga_samples(row: usize, n: usize, seed: u64) -> Vec<[f64; 4]> {
    let (_, mv, sv, md, sd, ms, ss, _) = USGA[row];
    let mut rng = Rng(seed ^ (0x9E37_79B9_7F4A_7C15u64.wrapping_mul(row as u64 + 1)));
    (0..n)
        .map(|_| {
            let v = (mv + sv * rng.gauss()).max(20.0);
            let d = (md + sd * rng.gauss()).max(0.5);
            let s = (ms + ss * rng.gauss()).max(200.0);
            let x = dimless(v, d, s);
            [x[0], x[1], x[2], 0.0]
        })
        .collect()
}

/// 행별 (평균 캐리 yd, 표준오차 yd). 유한한 캐리만 센다.
/// 유한한 표본이 0개면 (NaN, NaN), 1개면 (그 값, NaN) — 표본 분산에는 표본이 2개 이상 필요하다.
pub fn usga_dist<A: Aero>(ae: &A, n: usize, dt: f64, seed: u64) -> Vec<(f64, f64)> {
    (0..USGA.len())
        .map(|row| {
            let s = usga_samples(row, n, seed);
            let c: Vec<f64> = run_par(ae, &s, dt, 2).iter().map(|p| p.0 * YD).filter(|x| x.is_finite()).collect();
            mean_se(&c)
        })
        .collect()
}

/// (평균, 표준오차). 표본 0개면 (NaN, NaN), 1개면 (값, NaN).
pub fn mean_se(c: &[f64]) -> (f64, f64) {
    let k = c.len();
    if k == 0 {
        return (f64::NAN, f64::NAN);
    }
    let m = c.iter().sum::<f64>() / k as f64;
    if k < 2 {
        return (m, f64::NAN);
    }
    let var = c.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (k - 1) as f64;
    (m, (var / k as f64).sqrt())
}

// ------------------------------------------------------------------ 모드 장 (H2, 3차원)

#[inline(always)]
pub fn cr(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [0.5 * (-t3 + 2.0 * t2 - t), 0.5 * (3.0 * t3 - 5.0 * t2 + 2.0), 0.5 * (-3.0 * t3 + 4.0 * t2 + t), 0.5 * (t3 - t2)]
}

/// 모드 다항식을 맞춘 스핀축 기울기 범위의 사인 상한 (sin 45°, 반올림 여유 포함)
pub const SA_MAX: f32 = std::f32::consts::FRAC_1_SQRT_2 * 1.000_01;

pub struct Modes {
    pub x0: [f64; 3],
    pub inv: [f64; 3],
    pub n: [usize; 3],
    pub c: Vec<[f32; 8]>,
}

pub fn lsq4(rows: &[[f64; 4]], y: &[f64]) -> [f64; 4] {
    let mut a = [[0.0f64; 5]; 4];
    for (r, &yy) in rows.iter().zip(y) {
        for i in 0..4 {
            for j in 0..4 {
                a[i][j] += r[i] * r[j];
            }
            a[i][4] += r[i] * yy;
        }
    }
    solve4(a)
}

impl Modes {
    /// 셋째 축은 sigma = sqrt(S) 로 편다: H2 양력은 S ~ 1/lambda 에서 꺾여 S 축 등간격이 그곳을 놓친다.
    /// 펼치기는 빠른 엔진(f32, hτ=0.0685, 오차 1.4e-4 yd)으로 한다.
    pub fn unfold<A: FastAero>(ae: &A, n: [usize; 3], n_a: usize, h: f32) -> Modes {
        let lo = [0.8, 0.0, 0.02f64.sqrt()];
        let hi = [3.9, 50f64.to_radians(), 0.62f64.sqrt()];
        let at = |q: usize, i: usize| lo[q] + (hi[q] - lo[q]) * i as f64 / (n[q] - 1) as f64;
        let al: Vec<f64> = (0..n_a).map(|j| 45f64.to_radians() * j as f64 / (n_a - 1) as f64).collect();
        let nodes = n[0] * n[1] * n[2];
        let pts: Vec<[f32; 4]> = (0..nodes * n_a)
            .map(|k| {
                let (node, j) = (k / n_a, k % n_a);
                let sg = at(2, node % n[2]);
                [at(0, node / (n[1] * n[2])), at(1, (node / n[2]) % n[1]), sg * sg, al[j].sin()].map(|v| v as f32)
            })
            .collect();
        let res = stream_par(ae, &pts, h, 2);
        let even: Vec<[f64; 4]> = al.iter().map(|a| { let s = a.sin(); [1.0, s * s, s.powi(4), s.powi(6)] }).collect();
        let odd: Vec<[f64; 4]> = al.iter().map(|a| { let s = a.sin(); [s, s.powi(3), s.powi(5), s.powi(7)] }).collect();
        let c = (0..nodes)
            .map(|node| {
                let xs: Vec<f64> = (0..n_a).map(|j| res[node * n_a + j][0] as f64).collect();
                let zs: Vec<f64> = (0..n_a).map(|j| res[node * n_a + j][1] as f64).collect();
                let ce = lsq4(&even, &xs);
                let co = lsq4(&odd[1..], &zs[1..]);
                [ce[0], ce[1], ce[2], ce[3], co[0], co[1], co[2], co[3]].map(|v| v as f32)
            })
            .collect();
        Modes { x0: lo, inv: [0, 1, 2].map(|q| (n[q] - 1) as f64 / (hi[q] - lo[q])), n, c }
    }

    /// 격자 안에서만 읽는다. 입력이 격자 밖(또는 NaN)이면 None.
    /// 안쪽 = 캣멀-롬 4×4×4 이웃이 모두 격자 안에 드는 구간: 세 축(v, θ, σ = √S) 각각에서 마디 1 ~ 마디 n−2,
    /// 그리고 |sin α| ≤ sin 45° (모드 다항식을 맞춘 α 범위).
    /// 밖에서 값이 필요하면 `read_clamped` 를 명시적으로 부른다.
    #[inline(always)]
    pub fn read(&self, v: f32, th: f32, s: f32, sa: f32) -> Option<(f32, f32)> {
        if self.in_range(v, th, s, sa) { Some(self.read_clamped(v, th, s, sa)) } else { None }
    }

    /// `read` 가 받아 주는 입력인가
    #[inline(always)]
    pub fn in_range(&self, v: f32, th: f32, s: f32, sa: f32) -> bool {
        let inside = |q: usize, x: f32| {
            let f = (x - self.x0[q] as f32) * self.inv[q] as f32;
            f >= 1.0 && f <= (self.n[q] - 2) as f32 // NaN 은 거짓
        };
        inside(0, v) && inside(1, th) && inside(2, s.sqrt()) && sa.abs() <= SA_MAX
    }

    /// 격자 밖 입력을 **조용히 가장자리 칸으로 잘라** 읽는다 (밖에서는 외삽한 값이라 정밀도 보증이 없다).
    /// 입력이 격자 안임을 이미 아는 속도 시험(h2)이 쓴다. 안전한 읽기는 `read`.
    #[inline(always)]
    pub fn read_clamped(&self, v: f32, th: f32, s: f32, sa: f32) -> (f32, f32) {
        let cell = |q: usize, x: f32| {
            let f = ((x - self.x0[q] as f32) * self.inv[q] as f32).clamp(1.0, (self.n[q] - 3) as f32 + 0.999_99);
            let i = f as usize;
            (i, f - i as f32)
        };
        let crf = |t: f32| cr(t as f64).map(|x| x as f32);
        let (c0, c1, c2) = (cell(0, v), cell(1, th), cell(2, s.sqrt()));
        let (w0, w1, w2) = (crf(c0.1), crf(c1.1), crf(c2.1));
        let (st0, ns) = (self.n[1] * self.n[2], self.n[2]);
        let b = (c0.0 - 1) * st0 + (c1.0 - 1) * ns + (c2.0 - 1);
        let mut part = [[0f32; 8]; 4];
        for p in 0..4 {
            for q in 0..4 {
                let row = b + p * st0 + q * ns;
                let wpq = w0[p] * w1[q];
                for r in 0..4 {
                    // SAFETY: cell 이 [1, n-3] 로 잘라 row + 3 < len
                    let g = unsafe { self.c.get_unchecked(row + r) };
                    let w = wpq * w2[r];
                    for m in 0..8 {
                        part[r][m] += w * g[m];
                    }
                }
            }
        }
        let a: [f32; 8] = std::array::from_fn(|m| (part[0][m] + part[1][m]) + (part[2][m] + part[3][m]));
        let s2 = sa * sa;
        (a[0] + s2 * (a[1] + s2 * (a[2] + s2 * a[3])), sa * (a[4] + s2 * (a[5] + s2 * (a[6] + s2 * a[7]))))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// h2.rs 와 같은 무작위 샷 (v0' 1.0–3.7, θ 5–40°, S0 0.03–0.55, 스핀축 ±40°)
    fn rnd_shots(n: usize, seed: u64) -> Vec<RunShot> {
        let mut rng = Rng(seed);
        (0..n)
            .map(|_| [rng.uni(1.0, 3.7), rng.uni(5f64.to_radians(), 40f64.to_radians()), rng.uni(0.03, 0.55), rng.uni(-40f64.to_radians(), 40f64.to_radians())])
            .collect()
    }

    fn max_err_yd(a: &[[f32; 2]], b: &[(f64, f64)]) -> f64 {
        a.iter().zip(b).fold(0.0f64, |m, (x, t)| m.max((x[0] as f64 - t.0).abs() * YD).max((x[1] as f64 - t.1).abs() * YD))
    }

    /// C9: stream(sin α) 과 run(α) 은 같은 샷에서 같은 착지를 낸다 (세 장, 무작위 300샷).
    #[test]
    fn stream_matches_run() {
        let shots = rnd_shots(300, 0x5EED_1234);
        let s32: Vec<StreamShot> = shots.iter().map(to_stream_shot).collect();
        fn check<A: Aero + FastAero>(name: &str, ae: &A, shots: &[RunShot], s32: &[StreamShot]) {
            let truth = run(ae, shots, 0.004);
            for (h, tol) in [(0.137f32, 1e-2), (0.0685, 3e-4)] {
                let got = stream_par(ae, s32, h, 1);
                let e = max_err_yd(&got, &truth);
                assert!(e < tol, "{name} h={h}: 최대 차 {e:.2e} yd (허용 {tol:.0e})");
            }
        }
        check("공유위기12", &default_field(), &shots, &s32);
        check("공유", &Shared { v: 0.1156, d: 1.7695, c: 2.5881, l: 4.5843 }, &shots, &s32);
        check("H2", &H2 { v: 0.2418, d: 0.2475, c: 2.401, lam: 7.08, e: 0.0 }, &shots, &s32);
        // 축 부호가 서로 바뀌지 않았는지: 두 엔진에서 z 의 부호와 크기가 같다
        let one = [[2.5, 0.2, 0.2, 20f64.to_radians()]];
        let (r, s) = (run(&default_field(), &one, 0.004)[0], stream_par(&default_field(), &[to_stream_shot(&one[0])], 0.0685, 1)[0]);
        assert!(r.1 * (s[1] as f64) > 0.0 && (r.1 - s[1] as f64).abs() * YD < 1e-3, "z 부호/크기: run {r:?} stream {s:?}");
    }

    /// C1: 걸음 처음 y ≥ 0 일 때만 착지로 본다. 수평·아래로 쏜 샷은 뜨지 못하고 발사점(0, 0)에 떨어진다 (두 엔진 모두).
    #[test]
    fn nonflying_shots_land_at_launch_point() {
        let f = default_field();
        // 스핀이 없으면 양력도 없다: θ ≤ 0 은 첫 걸음에 땅 밑으로 간다. 급하게 내려 쏘면 스핀이 있어도 마찬가지.
        let th = [0.0, -0.01, -0.1, -0.5, -1.2];
        let mut shots: Vec<RunShot> = th.iter().map(|&t| [2.5, t, 0.0, 0.1]).collect();
        shots.extend([[2.5, -0.5, 0.2, 0.1], [2.5, -1.2, 0.3, -0.2]]);
        let th = [0.0, -0.01, -0.1, -0.5, -1.2, -0.5, -1.2];
        for (t, p) in th.iter().zip(run(&f, &shots, 0.004)) {
            assert_eq!(p, (0.0, 0.0), "run θ={t}");
        }
        let s32: Vec<StreamShot> = shots.iter().map(to_stream_shot).collect();
        for (t, p) in th.iter().zip(stream_par(&f, &s32, 0.0685, 1)) {
            assert_eq!(p, [0.0, 0.0], "stream θ={t}");
        }
        // 한 묶음 안에서 섞여도 보통 샷은 영향받지 않는다 (레인 재충전·묶음 처리 경로)
        let normal = rnd_shots(60, 7);
        let alone = stream_par(&f, &normal.iter().map(to_stream_shot).collect::<Vec<_>>(), 0.0685, 1);
        let mut mixed: Vec<RunShot> = Vec::new();
        for (i, s) in normal.iter().enumerate() {
            mixed.push(*s);
            if i % 3 == 0 {
                mixed.push([2.0, -0.05 * (i % 5) as f64, 0.0, 0.0]); // 스핀 없는 θ ≤ 0 (i%5 == 0 이면 정확히 0)
            }
        }
        let got = stream_par(&f, &mixed.iter().map(to_stream_shot).collect::<Vec<_>>(), 0.0685, 1);
        let mut k = 0;
        for (i, (s, g)) in mixed.iter().zip(&got).enumerate() {
            if s[1] <= 0.0 {
                assert_eq!(*g, [0.0, 0.0], "묶음 안 θ ≤ 0 샷 {i}");
            } else {
                assert!((g[0] - alone[k][0]).abs() < 1e-6 && (g[1] - alone[k][1]).abs() < 1e-6, "보통 샷 {k}: {g:?} 대 {:?}", alone[k]);
                k += 1;
            }
        }
        assert_eq!(k, normal.len());
        // 양력이 중력을 이기면 θ = 0 (또는 약간 아래)이어도 올라가 날아간다 — 착지 판정은 발사각이 아니라 운동으로 한다
        let lifted: Vec<RunShot> = [[2.5, 0.0, 0.2, 0.0], [2.5, 0.0, 0.3, 0.2]].to_vec();
        let truth = run(&f, &lifted, 0.002);
        let got = stream_par(&f, &lifted.iter().map(to_stream_shot).collect::<Vec<_>>(), 0.0685, 1);
        for (t, g) in truth.iter().zip(&got) {
            assert!(t.0 * YD > 20.0, "양력으로 나는 샷이 발사점에 떨어졌다: {t:?}");
            assert!((g[0] as f64 - t.0).abs() * YD < 0.05, "stream {g:?} 대 run {t:?}");
        }
        // 아주 낮은 양의 발사각(0.2°–2°): 한 걸음 안에서 올라갔다 내려와도 유한하고 두 엔진이 맞는다
        let degs = [0.2f64, 0.5, 1.0, 2.0];
        let low: Vec<RunShot> = degs.iter().map(|d| [2.5, d.to_radians(), 0.2, 0.0]).collect();
        let truth = run(&f, &low, 0.0005);
        let got = stream_par(&f, &low.iter().map(to_stream_shot).collect::<Vec<_>>(), 0.0685, 1);
        for ((d, t), g) in degs.iter().zip(&truth).zip(&got) {
            assert!(g[0].is_finite() && t.0.is_finite() && t.0 >= 0.0, "θ={d}°: {g:?} {t:?}");
            assert!((g[0] as f64 - t.0).abs() * YD < 0.05, "θ={d}°: stream {} yd 대 run {} yd", g[0] as f64 * YD, t.0 * YD);
        }
    }

    /// C3: BATCH_K 는 1..=16 이어야 한다. 17 이상은 이전에는 stream 이 끝나지 않았다. 범위 안의 모든 K 는 같은 결과(비트까지).
    #[test]
    fn batch_k_validated() {
        assert_eq!(parse_batch_k(None), Ok(3));
        assert_eq!(parse_batch_k(Some(" 5 ")), Ok(5));
        assert_eq!(parse_batch_k(Some("1")), Ok(1));
        assert_eq!(parse_batch_k(Some("16")), Ok(16));
        for bad in ["0", "17", "100", "-1", "abc", "", "3.5"] {
            assert!(parse_batch_k(Some(bad)).is_err(), "'{bad}' 는 거부돼야 한다");
        }
        let f = default_field();
        let s32: Vec<StreamShot> = rnd_shots(100, 3).iter().map(to_stream_shot).collect();
        let mut base = vec![[0f32; 2]; s32.len()];
        stream_k(&f, &s32, 0.137, &mut base, 3);
        for k in 1..=WL {
            let mut out = vec![[0f32; 2]; s32.len()];
            stream_k(&f, &s32, 0.137, &mut out, k);
            for (a, b) in out.iter().zip(&base) {
                assert_eq!((a[0].to_bits(), a[1].to_bits()), (b[0].to_bits(), b[1].to_bits()), "K={k}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "batch_k")]
    fn batch_k_17_panics() {
        let mut out = [[0f32; 2]; 4];
        stream_k(&default_field(), &[[2.0, 0.3, 0.2, 0.0]; 4], 0.137, &mut out, 17);
    }

    #[test]
    #[should_panic(expected = "batch_k")]
    fn batch_k_0_panics() {
        let mut out = [[0f32; 2]; 4];
        stream_k(&default_field(), &[[2.0, 0.3, 0.2, 0.0]; 4], 0.137, &mut out, 0);
    }

    /// C4: NaN 은 두 엔진에서 같게 다룬다 — 어느 입력이 NaN 이면 결과도 NaN (유한한 값으로 둔갑하지 않는다).
    #[test]
    fn nan_propagates_in_both_engines() {
        let f = default_field();
        let good = [2.5, 0.25, 0.2, 0.1];
        for i in 0..4 {
            let mut bad = good;
            bad[i] = f64::NAN;
            let r = run(&f, &[good, bad, good], 0.004);
            assert!(r[1].0.is_nan() && r[1].1.is_nan(), "run 입력 {i} NaN: {:?}", r[1]);
            assert!(r[0].0.is_finite() && (r[0].0 - r[2].0).abs() < 1e-12, "이웃 레인에 번지면 안 된다");
            let s = [good, bad, good].map(|s| to_stream_shot(&s));
            let o = stream_par(&f, &s, 0.0685, 1);
            assert!(o[1][0].is_nan() && o[1][1].is_nan(), "stream 입력 {i} NaN: {:?}", o[1]);
            assert!(o[0][0].is_finite() && o[2][0].is_finite());
        }
        // 0–1 로 자르기도 NaN 을 지키고 ±inf 는 경계로: f64 clamp 와 같은 약속 (뉴턴 걸음이 0/0 이 되는 자리)
        for (x, want) in [(f32::NAN, f32::NAN), (f32::INFINITY, 1.0), (f32::NEG_INFINITY, 0.0), (-0.5, 0.0), (0.4, 0.4), (1.5, 1.0)] {
            let got = clamp01_lanes(x);
            assert!(if want.is_nan() { got.is_nan() } else { got == want }, "clamp01({x}) = {got}, 기대 {want}");
            let refv = (x as f64).clamp(0.0, 1.0);
            assert!(if want.is_nan() { refv.is_nan() } else { refv as f32 == want });
        }
    }

    /// 0–1 로 자르기를 이 대상의 F 로 한 번 해 본다 (AVX-512: clamp01, 이식판·NEON: max/min)
    fn clamp01_lanes(x: f32) -> f32 {
        let v = F::splat(x);
        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
        let r = v.clamp01();
        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
        let r = v.max(F::splat(0.0)).min(F::splat(1.0));
        let mut a = [0f32; 16];
        r.store(&mut a);
        assert!(a.iter().all(|y| y.to_bits() == a[0].to_bits() || (y.is_nan() && a[0].is_nan())), "레인마다 다름: {a:?}");
        a[0]
    }

    /// C5: 음수 스핀은 반대 회전으로 접힌다 — 극(w = −|u|/l)을 건드리지 않고 유한하며, 축을 π 돌린 샷과 같다.
    #[test]
    fn negative_spin_is_folded() {
        let f = default_field();
        // S0 = −0.16 은 이 장의 극(S = −1/l = −0.1538) 바로 아래다 (접지 않으면 분모가 0 을 지난다)
        for s0 in [0.05, 0.1538, 0.16, 0.3, 0.6] {
            let a = 20f64.to_radians();
            let neg = run(&f, &[[2.5, 0.25, -s0, a]], 0.004)[0];
            let rot = run(&f, &[[2.5, 0.25, s0, a + std::f64::consts::PI]], 0.004)[0];
            assert!(neg.0.is_finite() && neg.1.is_finite(), "S0=-{s0}: {neg:?}");
            assert!((neg.0 - rot.0).abs() < 1e-9 && (neg.1 - rot.1).abs() < 1e-9, "S0=-{s0}: 접힌 값 {neg:?} 대 축 π 회전 {rot:?}");
            let st = stream_par(&f, &[to_stream_shot(&[2.5, 0.25, -s0, a])], 0.0685, 1)[0];
            assert!(st[0].is_finite() && (st[0] as f64 - neg.0).abs() * YD < 1e-3 && (st[1] as f64 - neg.1).abs() * YD < 1e-3, "S0=-{s0}: stream {st:?} 대 run {neg:?}");
        }
        // 정상 입력(S0 ≥ 0)은 접기가 값을 안 바꾼다: 스핀 0 과 -0.0 도 같다
        let a = run(&f, &[[2.5, 0.25, 0.0, 0.1]], 0.004)[0];
        let b = run(&f, &[[2.5, 0.25, -0.0, 0.1]], 0.004)[0];
        assert_eq!((a.0.to_bits(), a.1.to_bits()), (b.0.to_bits(), b.1.to_bits()));
        // 장 자체의 정의역: w ≥ 0, l ≥ 0 에서 분모는 양수
        let (kd, kl) = f.kk(2.0, 0.3);
        assert!(kd.is_finite() && kl.is_finite() && kl > 0.0);
    }

    /// C6: stream 의 sincos 다항식은 |θ| ≤ π/2, 코사인은 |sin α| ≤ 1 에서만 쓸 수 있다. 밖은 NaN 으로 바로 돌려준다.
    #[test]
    fn stream_rejects_out_of_domain_shots() {
        let f = default_field();
        let good: StreamShot = [2.5, 0.25, 0.2, 0.1];
        let bads: [StreamShot; 8] = [
            [2.5, 2.0, 0.2, 0.1],            // θ > π/2
            [2.5, -2.0, 0.2, 0.1],           // θ < -π/2
            [2.5, 0.25, 0.2, 1.2],           // sin α > 1
            [2.5, 0.25, 0.2, -1.01],         // sin α < -1
            [0.0, 0.25, 0.2, 0.1],           // 정지
            [-1.0, 0.25, 0.2, 0.1],          // 음의 속력
            [f32::INFINITY, 0.25, 0.2, 0.1], // 무한
            [2.5, 0.25, f32::NAN, 0.1],      // NaN 스핀
        ];
        for b in bads {
            let o = stream_par(&f, &[good, b, good], 0.137, 1);
            assert!(o[1][0].is_nan() && o[1][1].is_nan(), "{b:?} -> {:?}", o[1]);
            assert!(o[0][0].is_finite() && o[0][0] == o[2][0], "이웃 샷이 영향을 받았다: {o:?}");
        }
        // 경계(θ = π/2 바로 안, |sin α| = 1)는 받아 준다
        let edge = stream_par(&f, &[[2.5, 1.57, 0.2, 1.0], [2.5, 0.25, 0.2, -1.0]], 0.137, 1);
        assert!(edge.iter().all(|p| p[0].is_finite() && p[1].is_finite()), "{edge:?}");
        // 다항식 정확도: 정의역 끝에서도 3e-7 이하 (f32 반올림 포함 약 1.2e-7)
        for x in [-std::f32::consts::FRAC_PI_2, -1.0, 0.0, 0.3, 1.0, std::f32::consts::FRAC_PI_2] {
            let (s, c) = sincos_poly(x);
            assert!((s as f64 - (x as f64).sin()).abs() < 3e-7 && (c as f64 - (x as f64).cos()).abs() < 3e-7, "x={x}");
        }
        // 정의역 밖에서는 다항식이 실제로 어긋난다 (그래서 거르는 것이다)
        let (s, _) = sincos_poly(4.0);
        assert!((s as f64 - 4.0f64.sin()).abs() > 1e-3);
    }

    /// run/fly: 유한하지 않거나 v0' ≤ 0 인 샷은 (NaN, NaN), 이웃 레인은 그대로
    #[test]
    fn run_rejects_nonfinite_and_still_shots() {
        let f = default_field();
        let good = [2.5, 0.25, 0.2, 0.1];
        for bad in [[0.0, 0.25, 0.2, 0.1], [-2.0, 0.25, 0.2, 0.1], [2.5, f64::INFINITY, 0.2, 0.1], [2.5, 0.25, 0.2, f64::NAN]] {
            let r = run(&f, &[good, bad, good], 0.004);
            assert!(r[1].0.is_nan() && r[1].1.is_nan(), "{bad:?}");
            assert!(r[0] == r[2] && r[0].0.is_finite());
        }
    }

    /// C8: 유한한 표본이 없으면 (NaN, NaN), 하나면 (값, NaN) — 이전에는 c.len() - 1 이 넘쳤다
    #[test]
    fn usga_dist_survives_empty_samples() {
        #[derive(Clone, Copy)]
        struct NanField;
        impl Aero for NanField {
            fn kk(&self, _: f64, _: f64) -> (f64, f64) {
                (f64::NAN, f64::NAN)
            }
        }
        let d = usga_dist(&NanField, 16, 0.064, 1);
        assert_eq!(d.len(), USGA.len());
        assert!(d.iter().all(|p| p.0.is_nan() && p.1.is_nan()));
        assert!(mean_se(&[]).0.is_nan());
        let (m, se) = mean_se(&[4.0]);
        assert_eq!(m, 4.0);
        assert!(se.is_nan());
        let (m, se) = mean_se(&[1.0, 2.0, 3.0]);
        assert!((m - 2.0).abs() < 1e-15 && (se - (1.0f64 / 3.0).sqrt()).abs() < 1e-15);
        // 정상 장은 모든 행이 유한
        let ok = usga_dist(&default_field(), 64, 0.064, 1);
        assert!(ok.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
    }

    /// C7: Modes::read 는 격자 밖(NaN 포함)에서 None, 안에서는 read_clamped 와 같은 값
    #[test]
    fn modes_read_reports_out_of_grid() {
        let m = Modes::unfold(&default_field(), [14, 18, 14], 8, 0.137);
        // 격자: v 0.8–3.9, θ 0–50°, √S 0.141–0.787 (둘째 마디 ~ 끝에서 둘째 마디가 안쪽)
        let inside = (2.5f32, 0.25f32, 0.2f32, 0.1f32);
        let got = m.read(inside.0, inside.1, inside.2, inside.3).expect("격자 안");
        assert_eq!(got, m.read_clamped(inside.0, inside.1, inside.2, inside.3));
        for (v, th, s, sa) in [
            (0.5, 0.25, 0.2, 0.1),  // v 가 작다
            (4.5, 0.25, 0.2, 0.1),  // v 가 크다
            (2.5, -0.1, 0.2, 0.1),  // θ < 0
            (2.5, 1.2, 0.2, 0.1),   // θ > 50°
            (2.5, 0.25, 0.01, 0.1), // 스핀 작다
            (2.5, 0.25, 0.9, 0.1),  // 스핀 크다
            (2.5, 0.25, -0.2, 0.1), // 음수 S (sqrt NaN)
            (2.5, 0.25, 0.2, 0.9),  // sin α > sin 45°
            (f32::NAN, 0.25, 0.2, 0.1),
            (2.5, f32::NAN, 0.2, 0.1),
        ] {
            assert!(m.read(v, th, s, sa).is_none(), "({v}, {th}, {s}, {sa}) 는 None 이어야 한다");
            let _ = m.read_clamped(v.clamp(0.0, 9.0), th.clamp(-1.0, 3.0), s.abs().min(2.0), sa.clamp(-1.0, 1.0)); // 잘라 읽기는 패닉 없이 값을 낸다
        }
        // 읽은 값이 엔진과 맞는다 (이 격자는 성기므로 느슨하게)
        let truth = run(&default_field(), &[[2.5, 0.25, 0.2, 0.1f64.asin()]], 0.004)[0];
        assert!(((got.0 as f64 - truth.0).abs() * YD) < 0.5 && ((got.1 as f64 - truth.1).abs() * YD) < 0.5, "{got:?} 대 {truth:?}");
    }

    /// D2: 옛 7행·6행 표는 harness 의 12행·11행 표에서 잘라 온 것과 값이 같다
    #[test]
    fn legacy_tour_tables_come_from_harness() {
        assert_eq!(PGA[0], [142.0, 10.4, 4630.0, 212.0]);
        assert_eq!(PGA[3], [127.0, 14.1, 6231.0, 183.0]);
        assert_eq!(PGA[6], [109.0, 20.4, 8647.0, 148.0]);
        assert_eq!(LPGA[0], [116.0, 14.3, 4801.0, 169.0]);
        assert_eq!(LPGA[5], [93.0, 23.9, 7589.0, 119.0]);
    }

    /// 공통 LM 으로 옮긴 fit_h2 가 이전 판(h2 바이너리 출력)과 같은 해를 낸다: v=0.27049 d=0.15363 c=3.13613 λ=10.1670
    #[test]
    fn fit_h2_reproduces_published_fit() {
        let rows: Vec<[f64; 4]> = PGA.to_vec();
        let mut best = ([0.0; 4], f64::INFINITY, 0);
        for s in [[0.25, 0.2, 2.0, 5.0], [0.22, 0.3, 4.0, 12.0], [0.3, 0.1, 1.5, 2.0]] {
            let f = fit_h2(&rows, s, 0.008);
            if f.1 < best.1 {
                best = f;
            }
        }
        let want = [0.27049, 0.15363, 3.13613, 10.1670];
        for i in 0..4 {
            assert!((best.0[i] - want[i]).abs() < 6e-5, "p[{i}] = {} 대 {}", best.0[i], want[i]);
        }
    }

    #[test]
    fn to_stream_shot_uses_sine() {
        let s = to_stream_shot(&[2.0, 0.3, 0.2, 30f64.to_radians()]);
        assert_eq!(s[0], 2.0);
        assert!((s[3] - 0.5).abs() < 1e-7);
    }
}
