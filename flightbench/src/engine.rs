//! 장 하나 비행 엔진: H2(힉스 이중항 장)·B4·F3, f64 기준 적분기, f32×16 빠른 엔진, 보정, USGA 분포 평균, 모드 장.
//! 실행 파일은 src/bin/h2.rs (속도·수치 정확도) 와 src/bin/calib.rs (자료 대비 정확도).

use std::hint::black_box;
use std::time::Instant;

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

/// N 샷 동시 진행 RK4. 입력 (v0', theta, S0, a), 출력 착지 (x, z).
#[inline(always)]
pub fn fly<A: Aero, const N: usize>(ae: &A, s: &[[f64; 4]; N], dt: f64) -> [(f64, f64); N] {
    let mut st = [[0.0f64; N]; 7]; // x y z ux uy uz w
    let (mut oy, mut oz) = ([0.0; N], [0.0; N]);
    for i in 0..N {
        st[3][i] = s[i][0] * s[i][1].cos();
        st[4][i] = s[i][0] * s[i][1].sin();
        st[6][i] = s[i][2] * s[i][0];
        oy[i] = s[i][3].sin();
        oz[i] = s[i][3].cos();
    }
    #[inline(always)]
    fn d<A: Aero>(ae: &A, ux: f64, uy: f64, uz: f64, w: f64, oy: f64, oz: f64) -> [f64; 4] {
        let sp = (ux * ux + uy * uy + uz * uz).sqrt();
        let (kd, kl) = ae.kk(sp, w);
        let (cx, cy, cz) = (oy * uz - oz * uy, oz * ux, -oy * ux);
        [-kd * ux + kl * cx, -kd * uy + kl * cy - 1.0, -kd * uz + kl * cz, -ae.beta() * w * sp]
    }
    let mut out = [(f64::NAN, f64::NAN); N];
    let mut done = [false; N];
    let mut nd = 0;
    let (h, s6) = (0.5 * dt, dt / 6.0);
    let mut steps = 0u32;
    while nd < N && steps < (8.0 / dt) as u32 + 2 {
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
        if steps > 1 {
            for i in 0..N {
                if !done[i] && nx[1][i] < 0.0 {
                    let (y0, y1, m0, m1) = (st[1][i], nx[1][i], st[4][i] * dt, nx[4][i] * dt);
                    let mut t = y0 / (y0 - y1);
                    for _ in 0..4 {
                        let (t2, t3) = (t * t, t * t * t);
                        let f = (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * m1;
                        let df = (6.0 * t2 - 6.0 * t) * y0 + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (-6.0 * t2 + 6.0 * t) * y1 + (3.0 * t2 - 2.0 * t) * m1;
                        t = (t - f / df).clamp(0.0, 1.0);
                    }
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
        }
        st = nx;
    }
    out
}

pub fn run<A: Aero>(ae: &A, shots: &[[f64; 4]], dt: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(shots.len());
    for ch in shots.chunks(8) {
        let mut a = [ch[0]; 8];
        a[..ch.len()].copy_from_slice(ch);
        out.extend_from_slice(&fly::<A, 8>(ae, &a, dt)[..ch.len()]);
    }
    out
}

pub fn run_par<A: Aero>(ae: &A, shots: &[[f64; 4]], dt: f64, threads: usize) -> Vec<(f64, f64)> {
    let per = shots.len().div_ceil(threads).div_ceil(8) * 8;
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

#[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
compile_error!("빠른 엔진은 AVX-512 가 필요하다 (.cargo/config.toml 의 target-cpu=native)");

pub const WL: usize = 16;

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

/// sin, cos (|x| <= pi/2, 테일러 11·12차, 오차 < 6e-8). 단순 산술이라 자동 벡터화된다.
#[inline(always)]
pub fn sincos_poly(x: f32) -> (f32, f32) {
    let x2 = x * x;
    let s = x * (1.0 + x2 * (-1.0 / 6.0 + x2 * (1.0 / 120.0 + x2 * (-1.0 / 5040.0 + x2 * (1.0 / 362880.0 + x2 * (-1.0 / 39916800.0))))));
    let c = 1.0 + x2 * (-0.5 + x2 * (1.0 / 24.0 + x2 * (-1.0 / 720.0 + x2 * (1.0 / 40320.0 + x2 * (-1.0 / 3628800.0 + x2 * (1.0 / 479001600.0))))));
    (s, c)
}

/// 입력 (v0', theta, S0, sin a) → 착지 [x, z] (무차원). h 는 τ 걸음.
/// 착지·재충전도 벡터로 한다: 에르미트 보간은 16레인 전체에 한 번, 결과는 마스크 scatter,
/// 새 샷은 SoA 입력에서 마스크 expand-load (떨어진 레인에만 다음 샷이 차례로 들어간다).
#[allow(unused_unsafe)]
pub fn stream<A: FastAero>(ae: &A, shots: &[[f32; 4]], h: f32, out: &mut [[f32; 2]]) {
    let batch_k: usize = std::env::var("BATCH_K").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    use std::arch::x86_64::*;
    let n = shots.len();
    if n == 0 {
        return;
    }
    assert!(n < i32::MAX as usize / 2 && out.len() == n);
    let p = ae.prep();
    // SoA 초기 상태 (ux0, uy0, w0, oy, oz). 끝에 16칸 여유
    let mut init = vec![vec![0f32; n + WL]; 5];
    {
        let [i0, i1, i2, i3, i4] = &mut init[..] else { unreachable!() };
        for k in 0..n {
            let s = shots[k];
            let (sn, cs) = sincos_poly(s[1]);
            i0[k] = s[0] * cs;
            i1[k] = s[0] * sn;
            i2[k] = s[2] * s[0];
            i3[k] = s[3];
            i4[k] = (1.0 - s[3] * s[3]).sqrt();
        }
    }
    let max_age = (8.0 / h) as i32 + 2;
    let (hh, hv, h6) = (F::splat(0.5 * h), F::splat(h), F::splat(h / 6.0));
    let zero = F::splat(0.0);
    unsafe {
        let iota = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
        let (one_i, two_i, maxa) = (_mm512_set1_epi32(1), _mm512_set1_epi32(2), _mm512_set1_epi32(max_age));
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
                let ld = |v: &Vec<f32>, src: F| F(_mm512_mask_expandloadu_ps(src.0, rm, v.as_ptr().add(next)));
                for k in [0usize, 1, 2, 5] {
                    st[g][k] = F(_mm512_mask_mov_ps(st[g][k].0, rm, zero.0));
                }
                st[g][3] = ld(&init[0], st[g][3]);
                st[g][4] = ld(&init[1], st[g][4]);
                st[g][6] = ld(&init[2], st[g][6]);
                oy[g] = ld(&init[3], oy[g]);
                oz[g] = ld(&init[4], oz[g]);
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
                pend[g] |= (nx[1].neg_mask() & act[g] & _mm512_cmpge_epi32_mask(age[g], two_i)) | over;
                let m = pend[g];
                // 착지한 레인은 얼려 두고(상태를 그대로 두면 다음 걸음이 같은 nx 를 다시 낸다) K 개가 모이면 한꺼번에 처리
                if m == 0 || ((m.count_ones() as usize) < batch_k && next < n && over == 0) {
                    st[g] = std::array::from_fn(|k| F(_mm512_mask_mov_ps(nx[k].0, m, st[g][k].0)));
                    k1[g] = std::array::from_fn(|k| F(_mm512_mask_mov_ps(kn[k].0, m, k1[g][k].0)));
                    continue;
                }
                pend[g] = 0;
                // 에르미트 착지 (16레인 한 번에; 쓰는 건 m 레인뿐)
                let (y0, y1) = (st[g][1], nx[1]);
                let (m0, m1) = (k1[g][1] * hv, kn[1] * hv);
                let mut t = F(_mm512_div_ps(y0.0, (y0 - y1).0));
                let (c2, c3) = (F::splat(2.0), F::splat(3.0));
                for _ in 0..3 {
                    let (t2, t3) = (t * t, t * t * t);
                    let f = (c2 * t3 - c3 * t2 + F::splat(1.0)) * y0 + (t3 - c2 * t2 + t) * m0 + (c3 * t2 - c2 * t3) * y1 + (t3 - t2) * m1;
                    let df = (F::splat(6.0) * (t2 - t)) * (y0 - y1) + (c3 * t2 - F::splat(4.0) * t + F::splat(1.0)) * m0 + (c3 * t2 - c2 * t) * m1;
                    t = F(_mm512_min_ps(_mm512_max_ps((t - F(_mm512_div_ps(f.0, df.0))).0, zero.0), _mm512_set1_ps(1.0)));
                }
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

pub fn stream_par<A: FastAero>(ae: &A, shots: &[[f32; 4]], h: f32, threads: usize) -> Vec<[f32; 2]> {
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

/// (볼스피드 mph, 발사각 deg, 스핀 rpm, 캐리 yd)
pub const PGA: [[f64; 4]; 7] = [
    [142.0, 10.4, 4630.0, 212.0], [137.0, 11.0, 4836.0, 203.0], [132.0, 12.1, 5361.0, 194.0], [127.0, 14.1, 6231.0, 183.0],
    [120.0, 16.3, 7097.0, 172.0], [115.0, 18.1, 7998.0, 160.0], [109.0, 20.4, 8647.0, 148.0],
];
pub const LPGA: [[f64; 4]; 6] = [
    [116.0, 14.3, 4801.0, 169.0], [112.0, 14.8, 5081.0, 161.0], [109.0, 17.1, 5943.0, 152.0],
    [104.0, 19.0, 6699.0, 141.0], [100.0, 20.8, 7494.0, 130.0], [93.0, 23.9, 7589.0, 119.0],
];
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

pub fn solve4(mut a: [[f64; 5]; 4]) -> [f64; 4] {
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        for r in 0..4 {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..5 {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    std::array::from_fn(|i| a[i][4] / a[i][i])
}

pub fn fit_h2(rows: &[[f64; 4]], start: [f64; 4], dt: f64) -> ([f64; 4], f64, usize) {
    let lo = [0.05, -1.0, 0.0, 0.0];
    let hi = [0.6, 2.0, 20.0, 80.0];
    let res = |p: &[f64; 4]| -> Vec<f64> {
        let m = H2 { v: p[0], d: p[1], c: p[2], lam: p[3], e: 0.0 };
        carry_rows(&m, rows, dt).iter().zip(rows).map(|(c, r)| c - r[3]).collect()
    };
    let cost = |r: &[f64]| r.iter().map(|x| x * x).sum::<f64>();
    let mut p = start;
    let mut r = res(&p);
    let mut mu = 1e-3;
    let mut evals = 1;
    for _ in 0..200 {
        let mut jac = vec![[0.0f64; 4]; rows.len()];
        for k in 0..4 {
            let h = 1e-6 * p[k].abs().max(1e-3);
            let mut q = p;
            q[k] += h;
            let rq = res(&q);
            evals += 1;
            for i in 0..rows.len() {
                jac[i][k] = (rq[i] - r[i]) / h;
            }
        }
        let mut jtj = [[0.0f64; 4]; 4];
        let mut jtr = [0.0f64; 4];
        for i in 0..rows.len() {
            for a in 0..4 {
                jtr[a] += jac[i][a] * r[i];
                for b in 0..4 {
                    jtj[a][b] += jac[i][a] * jac[i][b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut m = [[0.0f64; 5]; 4];
            for a in 0..4 {
                for b in 0..4 {
                    m[a][b] = jtj[a][b];
                }
                m[a][a] += mu * jtj[a][a].max(1e-12);
                m[a][4] = -jtr[a];
            }
            let dp = solve4(m);
            let q: [f64; 4] = std::array::from_fn(|k| (p[k] + dp[k]).clamp(lo[k], hi[k]));
            let rq = res(&q);
            evals += 1;
            if cost(&rq) < cost(&r) {
                let rel = (cost(&r) - cost(&rq)) / cost(&r).max(1e-30);
                p = q;
                r = rq;
                mu = (mu * 0.3).max(1e-12);
                improved = true;
                if rel < 1e-12 {
                    return (p, cost(&r), evals);
                }
                break;
            }
            mu *= 10.0;
        }
        if !improved {
            break;
        }
    }
    (p, cost(&r), evals)
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

/// 행별 (평균 캐리 yd, 표준오차 yd)
pub fn usga_dist<A: Aero>(ae: &A, n: usize, dt: f64, seed: u64) -> Vec<(f64, f64)> {
    (0..USGA.len())
        .map(|row| {
            let s = usga_samples(row, n, seed);
            let c: Vec<f64> = run_par(ae, &s, dt, 2).iter().map(|p| p.0 * YD).filter(|x| x.is_finite()).collect();
            let m = c.iter().sum::<f64>() / c.len() as f64;
            let var = c.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (c.len() - 1) as f64;
            (m, (var / c.len() as f64).sqrt())
        })
        .collect()
}

// ------------------------------------------------------------------ 모드 장 (H2, 3차원)

#[inline(always)]
pub fn cr(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [0.5 * (-t3 + 2.0 * t2 - t), 0.5 * (3.0 * t3 - 5.0 * t2 + 2.0), 0.5 * (-3.0 * t3 + 4.0 * t2 + t), 0.5 * (t3 - t2)]
}

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

    #[inline(always)]
    pub fn read(&self, v: f32, th: f32, s: f32, sa: f32) -> (f32, f32) {
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

