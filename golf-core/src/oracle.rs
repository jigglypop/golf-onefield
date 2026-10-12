//! 얼린 옛 판본 (검산용 기준 구현).
//!
//! 최적화 이전의 `fly` 를 글자 그대로 옮겨 둔 것이다: 56회 이분법, 나눗셈 3개짜리 rhs, 8성분 배열 RK4.
//! 새 엔진의 답이 이것과 어긋나지 않는지 시험(`cargo test`)과 벤치(`bench`)가 대조한다.
//! 환경(밀도·점성·바람) 입력은 없다: 기본 환경(ρ = 1.2, μ = 1.81e-5, 무풍) 전용이다.
//! 이 파일은 고치지 않는다 (위기 지수 `CRISIS_N` 만 lib.rs 와 맞췄다). 장의 계수를 바꾸는 일은 `Coef::SHARED_RE` 에서만 한다.

use crate::{dimless, Coef, Landing, Real, Shot, L_M, T_S};

const N: usize = 8; // t x y z ux uy uz w
type St<T> = [T; N];

#[derive(Clone, Copy)]
struct P<T> {
    v: T,
    e: T,
    inv_u0: T,
    d: T,
    c: T,
    l: T,
    beta: T,
    a4: T,
}

impl<T: Real> P<T> {
    fn new(c: &Coef) -> Self {
        P {
            v: T::lit(c.v),
            e: T::lit(c.e),
            inv_u0: T::lit(1.0 / c.u0),
            d: T::lit(c.d),
            c: T::lit(c.c),
            l: T::lit(c.l),
            beta: T::lit(c.beta),
            a4: T::lit(4.0 * c.a),
        }
    }
}

/// (|u| C_D, |u| C_L) 를 |u| 와 w 로
#[inline(always)]
fn kk<T: Real>(p: &P<T>, sp: T, w: T) -> (T, T) {
    let one = T::lit(1.0);
    let ur = w * sp / (sp + p.l * w);
    let q = sp * p.inv_u0;
    let q2 = q * q;
    // 위기 꺾임 지수는 lib.rs 의 CRISIS_N 과 같아야 한다 (4: 이전 공유위기, 12: 공유위기12)
    let qn = match crate::CRISIS_N {
        4 => q2 * q2,
        12 => {
            let q3 = q2 * q2 * q2;
            q3 * q3
        }
        _ => unreachable!(),
    };
    let crisis = p.e / (one + qn);
    let qq = one / (one + qn);
    ((p.v + crisis) * sp + p.d * ur, p.c * ur * (one - p.a4 * qq * (one - qq)))
}

/// 걸음 재매개의 도함수 dX/dτ = (dX/dt) · |u|^{-1/2}
#[inline(always)]
fn rhs<T: Real>(p: &P<T>, oy: T, oz: T, s: &St<T>) -> St<T> {
    let one = T::lit(1.0);
    let (ux, uy, uz, w) = (s[4], s[5], s[6], s[7]);
    let sp = (ux * ux + uy * uy + uz * uz).sqrt();
    let g = one / sp.sqrt();
    let (kd, kl) = kk(p, sp, w);
    // ω̂ × u,  ω̂ = (0, oy, oz)
    let cx = oy * uz - oz * uy;
    let cy = oz * ux;
    let cz = -(oy * ux);
    let ax = -kd * ux + kl * cx;
    let ay = -kd * uy + kl * cy - one;
    let az = -kd * uz + kl * cz;
    let aw = -p.beta * w * sp;
    [g, ux * g, uy * g, uz * g, ax * g, ay * g, az * g, aw * g]
}

#[inline(always)]
fn rk4<T: Real>(p: &P<T>, oy: T, oz: T, s: &St<T>, f: &St<T>, h: T) -> St<T> {
    let half = h * T::lit(0.5);
    let six = h / T::lit(6.0);
    let two = T::lit(2.0);
    let mut s2 = [T::lit(0.0); N];
    for i in 0..N {
        s2[i] = s[i] + half * f[i];
    }
    let k2 = rhs(p, oy, oz, &s2);
    let mut s3 = [T::lit(0.0); N];
    for i in 0..N {
        s3[i] = s[i] + half * k2[i];
    }
    let k3 = rhs(p, oy, oz, &s3);
    let mut s4 = [T::lit(0.0); N];
    for i in 0..N {
        s4[i] = s[i] + h * k3[i];
    }
    let k4 = rhs(p, oy, oz, &s4);
    let mut out = [T::lit(0.0); N];
    for i in 0..N {
        out[i] = s[i] + six * (f[i] + two * k2[i] + two * k3[i] + k4[i]);
    }
    out
}

/// 3차 에르미트 보간 (구간 길이 h, 0 ≤ th ≤ 1)
#[inline(always)]
fn herm<T: Real>(s0: &St<T>, f0: &St<T>, s1: &St<T>, f1: &St<T>, h: T, th: T) -> St<T> {
    let (h00, h10, h01, h11) = basis(th);
    let mut out = [T::lit(0.0); N];
    for i in 0..N {
        out[i] = h00 * s0[i] + h10 * h * f0[i] + h01 * s1[i] + h11 * h * f1[i];
    }
    out
}

#[inline(always)]
fn herm1<T: Real>(a0: T, b0: T, a1: T, b1: T, th: T) -> T {
    let (h00, h10, h01, h11) = basis(th);
    h00 * a0 + h10 * b0 + h01 * a1 + h11 * b1
}

#[inline(always)]
fn basis<T: Real>(th: T) -> (T, T, T, T) {
    let one = T::lit(1.0);
    let (two, three) = (T::lit(2.0), T::lit(3.0));
    let t2 = th * th;
    let t3 = t2 * th;
    (two * t3 - three * t2 + one, t3 - two * t2 + th, three * t2 - two * t3, t3 - t2)
}

/// 부호가 + 에서 - 로 바뀌는 θ ∈ (0, 1] 을 이분법으로 찾는다 (성분 하나의 에르미트)
#[inline(always)]
fn cross<T: Real>(a0: T, b0: T, a1: T, b1: T) -> T {
    let half = T::lit(0.5);
    let (mut lo, mut hi) = (T::lit(0.0), T::lit(1.0));
    for _ in 0..56 {
        let mid = (lo + hi) * half;
        if herm1(a0, b0, a1, b1, mid) > T::lit(0.0) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * half
}

/// 샷 하나를 날린다 (옛 판본). `path` 가 있으면 걸음마다 [t s, x m, y m, z m] 을 채운다.
pub fn fly<T: Real>(c: &Coef, shot: &Shot, h: f64, mut path: Option<&mut Vec<[f32; 4]>>) -> Landing {
    let p = P::<T>::new(c);
    let (u0, th0, s0) = dimless(shot);
    let a = shot.axis_deg.to_radians();
    let (oy, oz) = (T::lit(a.sin()), T::lit(a.cos()));
    let mut s = [T::lit(0.0); N];
    s[4] = T::lit(u0 * th0.cos());
    s[5] = T::lit(u0 * th0.sin());
    s[7] = T::lit(s0 * u0);
    let hh = T::lit(h);
    let mut f = rhs(&p, oy, oz, &s);
    let mut out = Landing::default();
    let mut apex_done = false;
    let mut push = |st: &St<T>| {
        if let Some(buf) = path.as_deref_mut() {
            buf.push([(st[0].to_f64() * T_S) as f32, (st[1].to_f64() * L_M) as f32, (st[2].to_f64() * L_M) as f32, (st[3].to_f64() * L_M) as f32]);
        }
    };
    push(&s);
    for k in 1..=2_000_000u32 {
        let s1 = rk4(&p, oy, oz, &s, &f, hh);
        let f1 = rhs(&p, oy, oz, &s1);
        out.steps = k;
        if !apex_done && s[5] > T::lit(0.0) && s1[5] <= T::lit(0.0) {
            let tha = cross(s[5], f[5] * hh, s1[5], f1[5] * hh);
            out.apex_m = herm(&s, &f, &s1, &f1, hh, tha)[2].to_f64() * L_M;
            apex_done = true;
        }
        if s[2] > T::lit(0.0) && s1[2] <= T::lit(0.0) {
            let thl = cross(s[2], f[2] * hh, s1[2], f1[2] * hh);
            let land = herm(&s, &f, &s1, &f1, hh, thl);
            let (ux, uy) = (land[4].to_f64(), land[5].to_f64());
            out.carry_m = land[1].to_f64() * L_M;
            out.lateral_m = land[3].to_f64() * L_M;
            out.land_deg = (-uy).atan2(ux).to_degrees();
            out.time_s = land[0].to_f64() * T_S;
            let mut end = land;
            end[2] = T::lit(0.0);
            push(&end);
            break;
        }
        push(&s1);
        s = s1;
        f = f1;
    }
    out
}
