//! 장 하나 골프공 비행 엔진의 러스트 판본과 병목 측정.
//!
//! 무차원 방정식 (higgs_engine.py 와 같음)
//!   du/dt = -|u| conj(Gamma(S)) u - i,  dw/dt = -0.06 w |u|,  S = w/|u|
//!   Gamma(S) = (vev + h S) e^{i psi(S)},  psi(S) = psi_inf (1 - e^{-S/s0})
//!
//! 결과 (2026-10-08, Xeon 2.8 GHz 1코어, AVX-512, 무작위 3000샷, 기준 dt'=0.0005)
//!   0. 파이썬 엔진과 차이 6e-10 yd
//!   병목 분해 (샷당, 정확도 1e-3 yd 이내 유지)
//!     파이썬 solve_ivp                         약 15,000 µs
//!     러스트 원래 꼴, dt'=0.004 (779 스텝)        154 µs   스텝당 198 ns
//!     초월함수 제거(각도 잠금)                     77 µs   ← 초월함수가 50%
//!     스텝 수 맞춤 dt'=0.064 (49 스텝, 4e-4 yd)    4.9 µs  ← 가장 큰 낭비는 과도한 스텝
//!     나눗셈 제거 (vev|u| + h w)                  3.1 µs
//!     8샷 동시 진행 (지연 사슬 숨김)              0.68 µs
//!   표 읽기: 3선형 21 ns(최대 0.17 yd), 3차 83 ns(최대 0.0125 yd)
//!   7번 아이언 1만 표본 분산: 8.1 ms (f32×16) / 표 0.52 ms
//!   60fps 화면 궤적 381 프레임 전체 36.5 µs
//!   f32 묶음은 f64×8보다 느렸다(벡터화가 덜 됨, 미해결)
//!   펼친 장 읽기 (f32, 경계검사 없음, 오차는 기준 dt'=0.0005 대비)
//!     3선형 128×201×121 (12.5 MB): 최대 0.0099 yd, 1코어 5.1e7 샷/초, 2코어 5.6e7 ← 메모리 병목
//!     3선형  64×101× 61 ( 1.6 MB): 최대 0.040 yd,  1코어 6.1e7, 2코어 1.6e8 (L2 안)
//!     3차    48× 76× 46 (0.67 MB): 최대 0.0040 yd, 1코어 2.1e7, 2코어 4.2e7 ← 계산 병목, 코어에 비례
//!   반례: log 저장, 진공해 배경 비율 저장 모두 원값 저장보다 2–10배 나쁨 (compress_field.py)
//!
//! cargo run --release

use std::f64::consts::FRAC_PI_4;
use std::hint::black_box;
use std::time::Instant;

const BETA: f64 = 0.06;
const VEV: f64 = 0.3881;
const H: f64 = 0.2714;
const PSI_INF: f64 = FRAC_PI_4;
const S0C: f64 = 1e-3;
const L_M: f64 = 53.53157581531039; // rho = 1.2
const YD: f64 = L_M / 0.9144; // 무차원 캐리 1 = 58.5 yd

macro_rules! engine {
    ($m:ident, $t:ty) => {
        pub mod $m {
            pub type T = $t;

            #[derive(Clone, Copy, Debug, Default)]
            pub struct Land {
                pub carry: T,
                pub apex: T,
                pub t: T,
                pub vx: T,
                pub vy: T,
                pub w: T,
                pub steps: u32,
            }

            #[inline(always)]
            fn deriv<G: Fn(T) -> (T, T)>(vx: T, vy: T, w: T, g: &G) -> (T, T, T) {
                let sp = (vx * vx + vy * vy).sqrt();
                let (gr, gi) = g(w / sp);
                let ax = -sp * (gr * vx + gi * vy);
                let ay = -sp * (gr * vy - gi * vx) - 1.0;
                (ax, ay, -(super::BETA as T) * w * sp)
            }

            /// RK4 고정 간격. 착지는 3차 Hermite 근 찾기.
            #[inline(always)]
            pub fn fly<G: Fn(T) -> (T, T)>(v0: T, th: T, s0: T, dt: T, g: &G) -> Land {
                let (mut x, mut y): (T, T) = (0.0, 0.0);
                let (mut vx, mut vy) = (v0 * th.cos(), v0 * th.sin());
                let mut w = s0 * v0;
                let (mut apex, mut t): (T, T) = (0.0, 0.0);
                let mut steps = 0u32;
                let h = 0.5 * dt;
                let s6 = dt / 6.0;
                loop {
                    let (a1x, a1y, d1) = deriv(vx, vy, w, g);
                    let (v2x, v2y, w2) = (vx + h * a1x, vy + h * a1y, w + h * d1);
                    let (a2x, a2y, d2) = deriv(v2x, v2y, w2, g);
                    let (v3x, v3y, w3) = (vx + h * a2x, vy + h * a2y, w + h * d2);
                    let (a3x, a3y, d3) = deriv(v3x, v3y, w3, g);
                    let (v4x, v4y, w4) = (vx + dt * a3x, vy + dt * a3y, w + dt * d3);
                    let (a4x, a4y, d4) = deriv(v4x, v4y, w4, g);
                    let x1 = x + s6 * (vx + 2.0 * v2x + 2.0 * v3x + v4x);
                    let y1 = y + s6 * (vy + 2.0 * v2y + 2.0 * v3y + v4y);
                    let vx1 = vx + s6 * (a1x + 2.0 * a2x + 2.0 * a3x + a4x);
                    let vy1 = vy + s6 * (a1y + 2.0 * a2y + 2.0 * a3y + a4y);
                    let w1 = w + s6 * (d1 + 2.0 * d2 + 2.0 * d3 + d4);
                    steps += 1;
                    if y1 > apex {
                        apex = y1;
                    }
                    if y1 < 0.0 && steps > 1 {
                        let (m0, m1) = (vy * dt, vy1 * dt);
                        let mut s = y / (y - y1);
                        for _ in 0..4 {
                            let (s2, s3) = (s * s, s * s * s);
                            let f = (2.0 * s3 - 3.0 * s2 + 1.0) * y
                                + (s3 - 2.0 * s2 + s) * m0
                                + (-2.0 * s3 + 3.0 * s2) * y1
                                + (s3 - s2) * m1;
                            let df = (6.0 * s2 - 6.0 * s) * y
                                + (3.0 * s2 - 4.0 * s + 1.0) * m0
                                + (-6.0 * s2 + 6.0 * s) * y1
                                + (3.0 * s2 - 2.0 * s) * m1;
                            s = (s - f / df).clamp(0.0, 1.0);
                        }
                        let (s2, s3) = (s * s, s * s * s);
                        let carry = (2.0 * s3 - 3.0 * s2 + 1.0) * x
                            + (s3 - 2.0 * s2 + s) * vx * dt
                            + (-2.0 * s3 + 3.0 * s2) * x1
                            + (s3 - s2) * vx1 * dt;
                        return Land {
                            carry,
                            apex,
                            t: t + s * dt,
                            vx: vx + s * (vx1 - vx),
                            vy: vy + s * (vy1 - vy),
                            w: w + s * (w1 - w),
                            steps,
                        };
                    }
                    x = x1;
                    y = y1;
                    vx = vx1;
                    vy = vy1;
                    w = w1;
                    t += dt;
                    if t > 6.0 {
                        return Land { carry: T::NAN, ..Default::default() };
                    }
                }
            }

            /// 장 Gamma(S): 원래 꼴(exp·cos·sin 매 평가)
            #[inline(always)]
            pub fn field_full(s: T) -> (T, T) {
                let psi = (super::PSI_INF as T) * (1.0 - (-s / (super::S0C as T)).exp());
                let m = (super::VEV as T) + (super::H as T) * s;
                (m * psi.cos(), m * psi.sin())
            }

            /// 나눗셈 없는 장 읽기 + 여러 샷을 같은 걸음으로 함께 굴리기.
            /// 각도 잠금에서 |u| conj(Gamma) = (vev |u| + h w) e^{-i pi/4} 이므로 S = w/|u| 의
            /// 나눗셈이 대수적으로 사라진다. 샷당 남는 비선형은 sqrt 하나다.
            #[inline(always)]
            pub fn fly_lanes<const N: usize>(v0: &[T; N], th: &[T; N], s0: &[T; N], dt: T) -> [T; N] {
                const C: T = std::f64::consts::FRAC_1_SQRT_2 as T;
                let (ve, he, be) = ((super::VEV as T) * C, (super::H as T) * C, super::BETA as T);
                #[inline(always)]
                fn d(vx: T, vy: T, w: T, ve: T, he: T, be: T) -> (T, T, T) {
                    let sp = (vx * vx + vy * vy).sqrt();
                    let k = ve * sp + he * w;
                    (-k * (vx + vy), -k * (vy - vx) - 1.0, -be * w * sp)
                }
                let (mut x, mut y) = ([0.0 as T; N], [0.0 as T; N]);
                let (mut vx, mut vy, mut w) = ([0.0 as T; N], [0.0 as T; N], [0.0 as T; N]);
                for i in 0..N {
                    vx[i] = v0[i] * th[i].cos();
                    vy[i] = v0[i] * th[i].sin();
                    w[i] = s0[i] * v0[i];
                }
                let mut out = [T::NAN; N];
                let mut done = [false; N];
                let mut ndone = 0usize;
                let (h, s6) = (0.5 * dt, dt / 6.0);
                let max_steps = (6.0 / dt) as u32 + 2;
                let mut steps = 0u32;
                while ndone < N && steps < max_steps {
                    let (mut x1, mut y1) = ([0.0 as T; N], [0.0 as T; N]);
                    let (mut vx1, mut vy1, mut w1) = ([0.0 as T; N], [0.0 as T; N], [0.0 as T; N]);
                    for i in 0..N {
                        let (a1x, a1y, d1) = d(vx[i], vy[i], w[i], ve, he, be);
                        let (v2x, v2y, w2) = (vx[i] + h * a1x, vy[i] + h * a1y, w[i] + h * d1);
                        let (a2x, a2y, d2) = d(v2x, v2y, w2, ve, he, be);
                        let (v3x, v3y, w3) = (vx[i] + h * a2x, vy[i] + h * a2y, w[i] + h * d2);
                        let (a3x, a3y, d3) = d(v3x, v3y, w3, ve, he, be);
                        let (v4x, v4y, w4) = (vx[i] + dt * a3x, vy[i] + dt * a3y, w[i] + dt * d3);
                        let (a4x, a4y, d4) = d(v4x, v4y, w4, ve, he, be);
                        x1[i] = x[i] + s6 * (vx[i] + 2.0 * v2x + 2.0 * v3x + v4x);
                        y1[i] = y[i] + s6 * (vy[i] + 2.0 * v2y + 2.0 * v3y + v4y);
                        vx1[i] = vx[i] + s6 * (a1x + 2.0 * a2x + 2.0 * a3x + a4x);
                        vy1[i] = vy[i] + s6 * (a1y + 2.0 * a2y + 2.0 * a3y + a4y);
                        w1[i] = w[i] + s6 * (d1 + 2.0 * d2 + 2.0 * d3 + d4);
                    }
                    steps += 1;
                    if steps > 1 {
                        for i in 0..N {
                            if !done[i] && y1[i] < 0.0 {
                                let (y0, yy, m0, m1) = (y[i], y1[i], vy[i] * dt, vy1[i] * dt);
                                let mut s = y0 / (y0 - yy);
                                for _ in 0..4 {
                                    let (s2, s3) = (s * s, s * s * s);
                                    let f = (2.0 * s3 - 3.0 * s2 + 1.0) * y0 + (s3 - 2.0 * s2 + s) * m0
                                        + (-2.0 * s3 + 3.0 * s2) * yy + (s3 - s2) * m1;
                                    let df = (6.0 * s2 - 6.0 * s) * y0 + (3.0 * s2 - 4.0 * s + 1.0) * m0
                                        + (-6.0 * s2 + 6.0 * s) * yy + (3.0 * s2 - 2.0 * s) * m1;
                                    s = (s - f / df).clamp(0.0, 1.0);
                                }
                                let (s2, s3) = (s * s, s * s * s);
                                out[i] = (2.0 * s3 - 3.0 * s2 + 1.0) * x[i] + (s3 - 2.0 * s2 + s) * vx[i] * dt
                                    + (-2.0 * s3 + 3.0 * s2) * x1[i] + (s3 - s2) * vx1[i] * dt;
                                done[i] = true;
                                ndone += 1;
                            }
                        }
                    }
                    x = x1;
                    y = y1;
                    vx = vx1;
                    vy = vy1;
                    w = w1;
                }
                out
            }

            /// 장 Gamma(S): 각도 잠금(초월함수 없음)
            #[inline(always)]
            pub fn field_locked(s: T) -> (T, T) {
                const C: T = std::f64::consts::FRAC_1_SQRT_2 as T;
                let m = (super::VEV as T) + (super::H as T) * s;
                (m * C, m * C)
            }
        }
    };
}

engine!(e64, f64);
engine!(e32, f32);

// ------------------------------------------------------------------ 펼친 표

struct Axis {
    x0: f64,
    dx: f64,
    n: usize,
}

impl Axis {
    fn new(a: f64, b: f64, n: usize) -> Self {
        Axis { x0: a, dx: (b - a) / (n - 1) as f64, n }
    }
    fn at(&self, i: usize) -> f64 {
        self.x0 + self.dx * i as f64
    }
    #[inline(always)]
    fn locate(&self, x: f64, lo: usize, hi: usize) -> (usize, f64) {
        let fi = (x - self.x0) / self.dx;
        let i = (fi.floor() as isize).clamp(lo as isize, (self.n - hi) as isize) as usize;
        (i, fi - i as f64)
    }
}

struct Table {
    ax: [Axis; 3],
    c64: Vec<f64>,
    c32: Vec<f32>,
}

impl Table {
    fn build(dt: f64, threads: usize) -> Table {
        let ax = [
            Axis::new(0.8, 3.9, 32),
            Axis::new(0.0, 50f64.to_radians(), 51),
            Axis::new(0.02, 0.62, 31),
        ];
        let (nv, nt, ns) = (ax[0].n, ax[1].n, ax[2].n);
        let mut c64 = vec![0.0; nv * nt * ns];
        let chunk = (nv + threads - 1) / threads;
        std::thread::scope(|sc| {
            for (ci, out) in c64.chunks_mut(chunk * nt * ns).enumerate() {
                let ax = &ax;
                sc.spawn(move || {
                    for (k, slot) in out.iter_mut().enumerate() {
                        let i = ci * chunk + k / (nt * ns);
                        let j = (k / ns) % nt;
                        let l = k % ns;
                        *slot = e64::fly(ax[0].at(i), ax[1].at(j), ax[2].at(l), dt, &e64::field_locked).carry;
                    }
                });
            }
        });
        let c32 = c64.iter().map(|&v| v as f32).collect();
        Table { ax, c64, c32 }
    }

    #[inline(always)]
    fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.ax[1].n + j) * self.ax[2].n + k
    }

    #[inline(always)]
    fn trilinear(&self, v: f64, th: f64, s: f64) -> f64 {
        let (i, fx) = self.ax[0].locate(v, 0, 2);
        let (j, fy) = self.ax[1].locate(th, 0, 2);
        let (k, fz) = self.ax[2].locate(s, 0, 2);
        let d = &self.c64;
        let mut acc = 0.0;
        for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
            for (dj, wy) in [(0, 1.0 - fy), (1, fy)] {
                let b = self.idx(i + di, j + dj, k);
                acc += wx * wy * ((1.0 - fz) * d[b] + fz * d[b + 1]);
            }
        }
        acc
    }

    #[inline(always)]
    fn trilinear32(&self, v: f32, th: f32, s: f32) -> f32 {
        let (i, fx) = self.ax[0].locate(v as f64, 0, 2);
        let (j, fy) = self.ax[1].locate(th as f64, 0, 2);
        let (k, fz) = self.ax[2].locate(s as f64, 0, 2);
        let (fx, fy, fz) = (fx as f32, fy as f32, fz as f32);
        let d = &self.c32;
        let mut acc = 0.0f32;
        for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
            for (dj, wy) in [(0, 1.0 - fy), (1, fy)] {
                let b = self.idx(i + di, j + dj, k);
                acc += wx * wy * ((1.0 - fz) * d[b] + fz * d[b + 1]);
            }
        }
        acc
    }

    /// Catmull-Rom 3차 (4x4x4 = 64 읽기)
    #[inline(always)]
    fn tricubic(&self, v: f64, th: f64, s: f64) -> f64 {
        #[inline(always)]
        fn cr(t: f64) -> [f64; 4] {
            let (t2, t3) = (t * t, t * t * t);
            [
                0.5 * (-t3 + 2.0 * t2 - t),
                0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
                0.5 * (-3.0 * t3 + 4.0 * t2 + t),
                0.5 * (t3 - t2),
            ]
        }
        let (i, fx) = self.ax[0].locate(v, 1, 3);
        let (j, fy) = self.ax[1].locate(th, 1, 3);
        let (k, fz) = self.ax[2].locate(s, 1, 3);
        let (wx, wy, wz) = (cr(fx), cr(fy), cr(fz));
        let d = &self.c64;
        let mut acc = 0.0;
        for a in 0..4 {
            for b in 0..4 {
                let base = self.idx(i + a - 1, j + b - 1, k - 1);
                let row = wz[0] * d[base] + wz[1] * d[base + 1] + wz[2] * d[base + 2] + wz[3] * d[base + 3];
                acc += wx[a] * wy[b] * row;
            }
        }
        acc
    }
}

// ------------------------------------------------------------------ 펼친 장 (빠른 읽기 판본)

/// f32 3차원 장. 축 정보를 역수로 미리 계산하고, 읽기 경로에 나눗셈·f64·분기가 없다.
/// GPU 3D 텍스처와 같은 배치(마지막 축이 가장 빠름)라 그대로 텍스처로 올릴 수 있다.
struct FieldTex {
    x0: [f32; 3],
    inv: [f32; 3],
    max: [f32; 3], // 셀 인덱스 상한 (n - 2)
    sj: usize,
    si: usize,
    data: Vec<f32>,
}

impl FieldTex {
    /// 장 펼치기: 격자점마다 8샷 묶음 적분(f64, dt')으로 캐리를 구한다.
    fn unfold(n: [usize; 3], dt: f64, threads: usize) -> FieldTex {
        let ax = [
            Axis::new(0.8, 3.9, n[0]),
            Axis::new(0.0, 50f64.to_radians(), n[1]),
            Axis::new(0.02, 0.62, n[2]),
        ];
        let total = n[0] * n[1] * n[2];
        let mut data = vec![0f32; total];
        let per = (total + threads - 1) / threads;
        std::thread::scope(|sc| {
            for (ci, out) in data.chunks_mut(per).enumerate() {
                let ax = &ax;
                sc.spawn(move || {
                    let base = ci * per;
                    for (cj, blk) in out.chunks_mut(8).enumerate() {
                        let (mut v, mut t, mut s) = ([1.0f64; 8], [0.3f64; 8], [0.2f64; 8]);
                        for l in 0..blk.len() {
                            let k = base + cj * 8 + l;
                            let (i, j, m) = (k / (n[1] * n[2]), (k / n[2]) % n[1], k % n[2]);
                            (v[l], t[l], s[l]) = (ax[0].at(i), ax[1].at(j), ax[2].at(m));
                        }
                        let r = e64::fly_lanes::<8>(&v, &t, &s, dt);
                        for l in 0..blk.len() {
                            blk[l] = r[l] as f32;
                        }
                    }
                });
            }
        });
        FieldTex {
            x0: [ax[0].x0 as f32, ax[1].x0 as f32, ax[2].x0 as f32],
            inv: [(1.0 / ax[0].dx) as f32, (1.0 / ax[1].dx) as f32, (1.0 / ax[2].dx) as f32],
            max: [(n[0] - 2) as f32, (n[1] - 2) as f32, (n[2] - 2) as f32],
            sj: n[2],
            si: n[1] * n[2],
            data,
        }
    }

    #[inline(always)]
    fn read(&self, v: f32, th: f32, s: f32) -> f32 {
        let fx = ((v - self.x0[0]) * self.inv[0]).clamp(0.0, self.max[0] + 0.999_999);
        let fy = ((th - self.x0[1]) * self.inv[1]).clamp(0.0, self.max[1] + 0.999_999);
        let fz = ((s - self.x0[2]) * self.inv[2]).clamp(0.0, self.max[2] + 0.999_999);
        let (i, j, k) = (fx as usize, fy as usize, fz as usize);
        let (ax, ay, az) = (fx - i as f32, fy - j as f32, fz - k as f32);
        let b = i * self.si + j * self.sj + k;
        let d = &self.data;
        let (sj, si) = (self.sj, self.si);
        let c00 = d[b] + az * (d[b + 1] - d[b]);
        let c01 = d[b + sj] + az * (d[b + sj + 1] - d[b + sj]);
        let c10 = d[b + si] + az * (d[b + si + 1] - d[b + si]);
        let c11 = d[b + si + sj] + az * (d[b + si + sj + 1] - d[b + si + sj]);
        let c0 = c00 + ay * (c01 - c00);
        let c1 = c10 + ay * (c11 - c10);
        c0 + ax * (c1 - c0)
    }

    fn bytes(&self) -> usize {
        self.data.len() * 4
    }

    /// Catmull-Rom 3차, f32, 경계검사 없음. 64번 읽지만 장이 작아 캐시 안에 머문다.
    #[inline(always)]
    fn read_cubic_nc(&self, v: f32, th: f32, s: f32) -> f32 {
        #[inline(always)]
        fn cr(t: f32) -> [f32; 4] {
            let (t2, t3) = (t * t, t * t * t);
            [
                0.5 * (-t3 + 2.0 * t2 - t),
                0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
                0.5 * (-3.0 * t3 + 4.0 * t2 + t),
                0.5 * (t3 - t2),
            ]
        }
        let fx = ((v - self.x0[0]) * self.inv[0]).clamp(1.0, self.max[0] - 1.0 + 0.999_999);
        let fy = ((th - self.x0[1]) * self.inv[1]).clamp(1.0, self.max[1] - 1.0 + 0.999_999);
        let fz = ((s - self.x0[2]) * self.inv[2]).clamp(1.0, self.max[2] - 1.0 + 0.999_999);
        let (i, j, k) = (fx as usize, fy as usize, fz as usize);
        let (wx, wy, wz) = (cr(fx - i as f32), cr(fy - j as f32), cr(fz - k as f32));
        let b0 = (i - 1) * self.si + (j - 1) * self.sj + (k - 1);
        let mut acc = 0f32;
        for a in 0..4 {
            let mut ya = 0f32;
            for c in 0..4 {
                let base = b0 + a * self.si + c * self.sj;
                // SAFETY: 인덱스를 [1, n-3] 셀로 자르므로 base + 3 < len
                let r = unsafe {
                    wz[0] * *self.data.get_unchecked(base)
                        + wz[1] * *self.data.get_unchecked(base + 1)
                        + wz[2] * *self.data.get_unchecked(base + 2)
                        + wz[3] * *self.data.get_unchecked(base + 3)
                };
                ya += wy[c] * r;
            }
            acc += wx[a] * ya;
        }
        acc
    }

    /// clamp 로 범위가 보장되므로 경계 검사를 뺀 판본(벡터화 실험용)
    #[inline(always)]
    fn read_nc(&self, v: f32, th: f32, s: f32) -> f32 {
        let fx = ((v - self.x0[0]) * self.inv[0]).clamp(0.0, self.max[0] + 0.999_999);
        let fy = ((th - self.x0[1]) * self.inv[1]).clamp(0.0, self.max[1] + 0.999_999);
        let fz = ((s - self.x0[2]) * self.inv[2]).clamp(0.0, self.max[2] + 0.999_999);
        let (i, j, k) = (fx as usize, fy as usize, fz as usize);
        let (ax, ay, az) = (fx - i as f32, fy - j as f32, fz - k as f32);
        let b = i * self.si + j * self.sj + k;
        let (sj, si) = (self.sj, self.si);
        // SAFETY: fx,fy,fz 를 [0, n-2+1) 로 자르므로 b + si + sj + 1 < len
        let g = |o: usize| unsafe { *self.data.get_unchecked(b + o) };
        let c00 = g(0) + az * (g(1) - g(0));
        let c01 = g(sj) + az * (g(sj + 1) - g(sj));
        let c10 = g(si) + az * (g(si + 1) - g(si));
        let c11 = g(si + sj) + az * (g(si + sj + 1) - g(si + sj));
        let c0 = c00 + ay * (c01 - c00);
        let c1 = c10 + ay * (c11 - c10);
        c0 + ax * (c1 - c0)
    }
}

// ------------------------------------------------------------------ 보조

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn uni(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
    fn gauss(&mut self) -> f64 {
        let (u1, u2) = (self.next().max(1e-300), self.next());
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// 같은 일을 reps 번 반복해 가장 빠른 한 번의 샷당 ns 를 낸다.
fn bench<F: FnMut() -> f64>(n_shots: usize, reps: usize, mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t0 = Instant::now();
        black_box(f());
        best = best.min(t0.elapsed().as_nanos() as f64 / n_shots as f64);
    }
    best
}

fn stats(err: &[f64]) -> (f64, f64) {
    let rmse = (err.iter().map(|e| e * e).sum::<f64>() / err.len() as f64).sqrt();
    let max = err.iter().fold(0.0f64, |m, e| m.max(e.abs()));
    (rmse, max)
}

fn main() {
    // ---- 0. 파이썬과 일치 확인
    let refs: Vec<[f64; 4]> = std::fs::read_to_string("../refs.csv")
        .expect("refs.csv")
        .lines()
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3]]
        })
        .collect();
    let mut dev = 0.0f64;
    for r in &refs {
        let c = e64::fly(r[0], r[1], r[2], 0.0005, &e64::field_full).carry;
        dev = dev.max(((c - r[3]) * YD).abs());
    }
    println!("== 0. 파이썬 엔진과 일치: 투어 13행 최대 차이 {:.2e} yd", dev);

    // ---- 무작위 샷 3000개와 기준값
    let n = 3000;
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let shots: Vec<(f64, f64, f64)> = (0..n)
        .map(|_| (rng.uni(1.56, 3.61), rng.uni(5f64.to_radians(), 40f64.to_radians()), rng.uni(0.05, 0.55)))
        .collect();
    let truth: Vec<f64> = shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.0005, &e64::field_locked).carry).collect();
    let mean_steps_ref: f64 =
        shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.004, &e64::field_locked).steps as f64).sum::<f64>() / n as f64;

    // ---- 1. 장 평가의 비용: 원래 꼴 vs 각도 잠금
    println!("\n== 1. 장 평가 비용 (f64, dt'=0.004, 평균 {:.0} 스텝/샷)", mean_steps_ref);
    let t_full = bench(n, 7, || shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.004, &e64::field_full).carry).sum());
    let t_lock = bench(n, 7, || shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.004, &e64::field_locked).carry).sum());
    let parity: Vec<f64> = shots
        .iter()
        .map(|&(v, t, s)| (e64::fly(v, t, s, 0.004, &e64::field_full).carry - e64::fly(v, t, s, 0.004, &e64::field_locked).carry) * YD)
        .collect();
    println!("원래 꼴 (exp·cos·sin): {:>9.0} ns/샷  {:>6.1} ns/스텝", t_full, t_full / mean_steps_ref);
    println!("각도 잠금           : {:>9.0} ns/샷  {:>6.1} ns/스텝", t_lock, t_lock / mean_steps_ref);
    println!("두 꼴의 캐리 차이 최대 {:.2e} yd → 초월함수 비중 {:.0}%", stats(&parity).1, 100.0 * (1.0 - t_lock / t_full));

    // ---- 2. 스텝 수: 정확도-시간 곡선
    println!("\n== 2. 시간 간격 dt' 와 정확도 (각도 잠금, 기준 dt'=0.0005)");
    println!("  dt'     스텝/샷   ns/샷     RMSE(yd)    최대(yd)    f32 최대(yd)");
    for dt in [0.001, 0.002, 0.004, 0.008, 0.016, 0.032, 0.064, 0.128] {
        let err: Vec<f64> = shots
            .iter()
            .zip(&truth)
            .map(|(&(v, t, s), &c)| (e64::fly(v, t, s, dt, &e64::field_locked).carry - c) * YD)
            .collect();
        let err32: Vec<f64> = shots
            .iter()
            .zip(&truth)
            .map(|(&(v, t, s), &c)| (e32::fly(v as f32, t as f32, s as f32, dt as f32, &e32::field_locked).carry as f64 - c) * YD)
            .collect();
        let steps: f64 =
            shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, dt, &e64::field_locked).steps as f64).sum::<f64>() / n as f64;
        let ns = bench(n, 5, || shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, dt, &e64::field_locked).carry).sum());
        let (r, m) = stats(&err);
        println!("  {:<6}  {:>7.0}  {:>8.0}   {:>9.2e}   {:>9.2e}   {:>9.2e}", dt, steps, ns, r, m, stats(&err32).1);
    }
    let t32 = bench(n, 7, || {
        shots.iter().map(|&(v, t, s)| e32::fly(v as f32, t as f32, s as f32, 0.032, &e32::field_locked).carry as f64).sum()
    });
    let t64 = bench(n, 7, || shots.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.032, &e64::field_locked).carry).sum());
    println!("dt'=0.032 에서 f64 {:.0} ns/샷, f32 {:.0} ns/샷", t64, t32);

    // ---- 3. 펼친 표
    println!("\n== 3. 펼친 표 (32×51×31 = 50,592점)");
    let t0 = Instant::now();
    let table = Table::build(0.002, 2);
    println!("펼치기 {:.2} s (2 스레드, dt'=0.002)", t0.elapsed().as_secs_f64());
    let e_lin: Vec<f64> = shots.iter().zip(&truth).map(|(&(v, t, s), &c)| (table.trilinear(v, t, s) - c) * YD).collect();
    let e_cub: Vec<f64> = shots.iter().zip(&truth).map(|(&(v, t, s), &c)| (table.tricubic(v, t, s) - c) * YD).collect();
    let e_l32: Vec<f64> = shots
        .iter()
        .zip(&truth)
        .map(|(&(v, t, s), &c)| (table.trilinear32(v as f32, t as f32, s as f32) as f64 - c) * YD)
        .collect();
    let ns_lin = bench(n, 20, || shots.iter().map(|&(v, t, s)| table.trilinear(v, t, s)).sum());
    let ns_cub = bench(n, 20, || shots.iter().map(|&(v, t, s)| table.tricubic(v, t, s)).sum());
    let ns_l32 = bench(n, 20, || shots.iter().map(|&(v, t, s)| table.trilinear32(v as f32, t as f32, s as f32) as f64).sum());
    for (name, e, ns) in [("3선형 f64", &e_lin, ns_lin), ("3선형 f32", &e_l32, ns_l32), ("3차 f64  ", &e_cub, ns_cub)] {
        let (r, m) = stats(e);
        println!("{}: {:>6.1} ns/샷  RMSE {:.2e} yd  최대 {:.2e} yd", name, ns, r, m);
    }

    // ---- 4. 실제 쓰임새: 한 샷의 분산 1만 표본, 화면용 궤적
    println!("\n== 4. 쓰임새별 비용");
    let base = refs[4]; // PGA 7i
    let mut rng = Rng(7);
    let ens: Vec<(f64, f64, f64)> = (0..10_000)
        .map(|_| {
            let dv = 0.01 * rng.gauss();
            let dth = 0.5f64.to_radians() * rng.gauss();
            let ds = 0.03 * rng.gauss();
            (base[0] * (1.0 + dv), base[1] + dth, base[2] * (1.0 + ds) / (1.0 + dv))
        })
        .collect();
    let spread = |c: &[f64]| {
        let m = c.iter().sum::<f64>() / c.len() as f64;
        (c.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / c.len() as f64).sqrt() * YD
    };
    let c_dir: Vec<f64> = ens.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.032, &e64::field_locked).carry).collect();
    let c_tab: Vec<f64> = ens.iter().map(|&(v, t, s)| table.tricubic(v, t, s)).collect();
    let t_dir = bench(1, 5, || ens.iter().map(|&(v, t, s)| e64::fly(v, t, s, 0.032, &e64::field_locked).carry).sum());
    let t_tab = bench(1, 5, || ens.iter().map(|&(v, t, s)| table.tricubic(v, t, s)).sum());
    println!(
        "7번 아이언 1만 표본 분산: 직접(dt'=0.032) {:.2} ms → σ {:.2} yd / 표 {:.3} ms → σ {:.2} yd",
        t_dir / 1e6,
        spread(&c_dir),
        t_tab / 1e6,
        spread(&c_tab)
    );
    let frame = (1.0 / 60.0) / (22.916037151920374 / 9.81); // 60fps 한 프레임의 무차원 시간
    let full = e64::fly(base[0], base[1], base[2], frame, &e64::field_locked);
    let t_traj = bench(1, 20, || e64::fly(base[0], base[1], base[2], frame, &e64::field_locked).carry);
    println!(
        "화면용 궤적(60fps 프레임 간격 = 1스텝): {} 프레임, 전체 {:.1} µs, 오차 {:.3} yd",
        full.steps,
        t_traj / 1e3,
        (full.carry - e64::fly(base[0], base[1], base[2], 0.0005, &e64::field_locked).carry) * YD
    );

    // ---- 5. 지연 사슬 끊기: 나눗셈 제거 + 샷 묶음
    println!("\n== 5. 나눗셈 제거 + N개 샷 동시 진행 (정확도는 같은 dt' 의 스칼라와 동일해야 함)");
    println!("  dt'    정밀도  N    ns/샷    최대오차(yd)");
    for dt in [0.064, 0.032] {
        for (name, nlane, ns, err) in [
            ("f64", 1, bench(n, 7, || run64::<1>(&shots, dt).iter().sum()), maxerr(&run64::<1>(&shots, dt), &truth)),
            ("f64", 4, bench(n, 7, || run64::<4>(&shots, dt).iter().sum()), maxerr(&run64::<4>(&shots, dt), &truth)),
            ("f64", 8, bench(n, 7, || run64::<8>(&shots, dt).iter().sum()), maxerr(&run64::<8>(&shots, dt), &truth)),
            ("f32", 8, bench(n, 7, || run32::<8>(&shots, dt).iter().sum()), maxerr(&run32::<8>(&shots, dt), &truth)),
            ("f32", 16, bench(n, 7, || run32::<16>(&shots, dt).iter().sum()), maxerr(&run32::<16>(&shots, dt), &truth)),
        ] {
            println!("  {:<6} {:<5} {:>3} {:>8.0}    {:.2e}", dt, name, nlane, ns, err);
        }
    }
    let t_ens = bench(1, 5, || run32::<16>(&ens, 0.064).iter().sum());
    let c_ens: Vec<f64> = run32::<16>(&ens, 0.064);
    println!(
        "7번 아이언 1만 표본 분산 (f32×16, dt'=0.064): {:.3} ms → σ {:.2} yd",
        t_ens / 1e6,
        spread(&c_ens)
    );

    // ---- 6. 장을 촘촘히 펼치고 읽기만 하기
    println!("\n== 6. 펼친 장 읽기 (f32, 경계검사 없음, 3선형)");
    // 큰 처리량 측정용 무작위 샷 100만 개
    let big = 1_000_000usize;
    let mut rng = Rng(42);
    let (bv, bt, bs): (Vec<f32>, Vec<f32>, Vec<f32>) = {
        let mut a = (Vec::with_capacity(big), Vec::with_capacity(big), Vec::with_capacity(big));
        for _ in 0..big {
            a.0.push(rng.uni(1.56, 3.61) as f32);
            a.1.push(rng.uni(5f64.to_radians(), 40f64.to_radians()) as f32);
            a.2.push(rng.uni(0.05, 0.55) as f32);
        }
        a
    };
    println!("  격자              크기      펼치기   RMSE(yd)   최대(yd)   ns/샷(3000)  1코어 샷/초   2코어 샷/초");
    let mut best: Option<FieldTex> = None;
    for g in [[32, 51, 31], [64, 101, 61], [128, 201, 121]] {
        let t0 = Instant::now();
        let ft = FieldTex::unfold(g, 0.032, 2);
        let t_build = t0.elapsed().as_secs_f64();
        let err: Vec<f64> = shots
            .iter()
            .zip(&truth)
            .map(|(&(v, t, s), &c)| (ft.read_nc(v as f32, t as f32, s as f32) as f64 - c) * YD)
            .collect();
        let (r, m) = stats(&err);
        let s32: Vec<(f32, f32, f32)> = shots.iter().map(|&(v, t, s)| (v as f32, t as f32, s as f32)).collect();
        let ns = bench(n, 50, || s32.iter().map(|&(v, t, s)| ft.read_nc(v, t, s) as f64).sum());
        let one = bench(big, 5, || {
            let mut acc = 0f32;
            for i in 0..big {
                acc += ft.read_nc(bv[i], bt[i], bs[i]);
            }
            acc as f64
        });
        let two = bench(big, 5, || {
            let h = big / 2;
            std::thread::scope(|sc| {
                let ft = &ft;
                let (bv, bt, bs) = (&bv, &bt, &bs);
                let hs: Vec<_> = (0..2)
                    .map(|p| {
                        sc.spawn(move || {
                            let mut acc = 0f32;
                            for i in p * h..(p + 1) * h {
                                acc += ft.read_nc(bv[i], bt[i], bs[i]);
                            }
                            acc
                        })
                    })
                    .collect();
                hs.into_iter().map(|x| x.join().unwrap() as f64).sum()
            })
        });
        println!(
            "  {:>3}×{:>3}×{:>3}  {:>7.1} MB  {:>6.2} s  {:>9.2e}  {:>9.2e}  {:>9.1}    {:>9.2e}   {:>9.2e}",
            g[0], g[1], g[2], ft.bytes() as f64 / 1e6, t_build, r, m, ns, 1e9 / one, 1e9 / two
        );
        best = Some(ft);
    }
    let ft = best.unwrap();
    let e32: Vec<(f32, f32, f32)> = ens.iter().map(|&(v, t, s)| (v as f32, t as f32, s as f32)).collect();
    let t_e = bench(1, 20, || e32.iter().map(|&(v, t, s)| ft.read_nc(v, t, s) as f64).sum());
    let c_e: Vec<f64> = e32.iter().map(|&(v, t, s)| ft.read_nc(v, t, s) as f64).collect();
    println!("7번 아이언 1만 표본 분산 (가장 촘촘한 장): {:.1} µs → σ {:.2} yd", t_e / 1e3, spread(&c_e));
    let safe = bench(n, 50, || shots.iter().map(|&(v, t, s)| ft.read(v as f32, t as f32, s as f32) as f64).sum());
    println!("참고: 경계검사 있는 read() {:.1} ns/샷", safe);

    // ---- 7. 작은 장 + 3차 읽기: 캐시 안에서 계산으로 정확도를 산다
    println!("\n== 7. 작은 장 + 3차 읽기 (f32, 경계검사 없음)");
    println!("  격자              크기      RMSE(yd)   최대(yd)   ns/샷(3000)  1코어 샷/초   2코어 샷/초");
    for g in [[24, 38, 24], [32, 51, 31], [48, 76, 46]] {
        let ft = FieldTex::unfold(g, 0.032, 2);
        let err: Vec<f64> = shots
            .iter()
            .zip(&truth)
            .map(|(&(v, t, s), &c)| (ft.read_cubic_nc(v as f32, t as f32, s as f32) as f64 - c) * YD)
            .collect();
        let (r, m) = stats(&err);
        let ns = bench(n, 50, || shots.iter().map(|&(v, t, s)| ft.read_cubic_nc(v as f32, t as f32, s as f32) as f64).sum());
        let one = bench(big, 5, || {
            let mut acc = 0f32;
            for i in 0..big {
                acc += ft.read_cubic_nc(bv[i], bt[i], bs[i]);
            }
            acc as f64
        });
        let two = bench(big, 5, || {
            let h = big / 2;
            std::thread::scope(|sc| {
                let ft = &ft;
                let (bv, bt, bs) = (&bv, &bt, &bs);
                let hs: Vec<_> = (0..2)
                    .map(|p| {
                        sc.spawn(move || {
                            let mut acc = 0f32;
                            for i in p * h..(p + 1) * h {
                                acc += ft.read_cubic_nc(bv[i], bt[i], bs[i]);
                            }
                            acc
                        })
                    })
                    .collect();
                hs.into_iter().map(|x| x.join().unwrap() as f64).sum()
            })
        });
        println!(
            "  {:>3}×{:>3}×{:>3}  {:>7.2} MB  {:>9.2e}  {:>9.2e}  {:>9.1}    {:>9.2e}   {:>9.2e}",
            g[0], g[1], g[2], ft.bytes() as f64 / 1e6, r, m, ns, 1e9 / one, 1e9 / two
        );
    }
}

fn maxerr(c: &[f64], truth: &[f64]) -> f64 {
    c.iter().zip(truth).fold(0.0f64, |m, (a, b)| m.max(((a - b) * YD).abs()))
}

fn run64<const N: usize>(shots: &[(f64, f64, f64)], dt: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(shots.len());
    for ch in shots.chunks(N) {
        let (mut v, mut t, mut s) = ([ch[0].0; N], [ch[0].1; N], [ch[0].2; N]);
        for (i, &(a, b, c)) in ch.iter().enumerate() {
            (v[i], t[i], s[i]) = (a, b, c);
        }
        out.extend_from_slice(&e64::fly_lanes::<N>(&v, &t, &s, dt)[..ch.len()]);
    }
    out
}

fn run32<const N: usize>(shots: &[(f64, f64, f64)], dt: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(shots.len());
    for ch in shots.chunks(N) {
        let (mut v, mut t, mut s) = ([ch[0].0 as f32; N], [ch[0].1 as f32; N], [ch[0].2 as f32; N]);
        for (i, &(a, b, c)) in ch.iter().enumerate() {
            (v[i], t[i], s[i]) = (a as f32, b as f32, c as f32);
        }
        out.extend(e32::fly_lanes::<N>(&v, &t, &s, dt as f32)[..ch.len()].iter().map(|&x| x as f64));
    }
    out
}
