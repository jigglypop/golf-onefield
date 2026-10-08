//! 3차원 장(사이드 스핀)의 러스트 판본: 직접 적분, 4차원 장 펼치기, 3차 읽기.
//!
//! 장 하나, 각도 잠금:  k = (vev |u| + h w)/sqrt2
//!   du/dt = -k u + k (omega x u) - y,  dw/dt = -0.06 w |u|,  omega = (0, sin a, cos a)
//! 방위각은 회전 대칭, a -> -a 는 좌우 반전이므로 펼칠 축은 (v0', theta, S0, a) 4개.
//! a 축은 음수 쪽 두 칸을 대칭으로 채워 경계에서도 3차 읽기가 정확하게 했다.
//!
//! 결과 (2026-10-08, 무작위 3000샷, a 는 ±40°, 기준 dt'=0.0005)
//!   0. 파이썬 field3d 와 차이 7.7e-10 yd
//!   1. 직접 적분 f64×8: dt'=0.064 에서 1,025 ns/샷, 최대 4e-4 yd
//!   2. 4차원 장 (a 축까지 펼침): 7.3–13.4 MB, 캐시를 넘어 코어 확장이 안 된다 [반례: 차원이 늘면 펼치기 이점이 준다]
//!        4선형 85 ns (0.20 yd) / 3차+선형 238 ns (0.11 yd) / 4차원 3차 333 ns (0.012 yd)
//!   3. 각도 모드 전개 (a 를 펼치지 않고 sin a 의 짝·홀 거듭제곱으로): 3차원 장 하나에 계수 2K 개
//!        K=3: 1.2 MB, 최대 0.16 yd (모드 잘림 오차, 격자를 늘려도 그대로)
//!        K=4: 1.6 MB, 최대 x 0.014 / z 0.005 yd, 87 ns, 1코어 9.8e6 / 2코어 1.9e7 샷/초
//!        K=4, 24×38×24: 0.70 MB, 최대 x 0.031 yd, 70 ns, 2코어 2.4e7
//!      누적기를 4갈래로 나누기 전에는 덧셈 지연 사슬(64번 × 4사이클 ≈ 92 ns)이 병목이라 2코어 확장이 없었다.
//!
//! cargo run --release --bin field4

use std::hint::black_box;
use std::time::Instant;

const VEV: f64 = 0.3881;
const H: f64 = 0.2714;
const BETA: f64 = 0.06;
const C: f64 = std::f64::consts::FRAC_1_SQRT_2;
const YD: f64 = 53.53157581531039 / 0.9144;

/// N 샷 동시 진행 3차원 RK4. 돌려주는 값은 착지 시 (x, z).
fn fly3<const N: usize>(v0: &[f64; N], th: &[f64; N], s0: &[f64; N], al: &[f64; N], dt: f64) -> ([f64; N], [f64; N]) {
    let (ve, he) = (VEV * C, H * C);
    let mut st = [[0.0f64; N]; 7]; // x y z ux uy uz w
    let (mut oy, mut oz) = ([0.0; N], [0.0; N]);
    for i in 0..N {
        st[3][i] = v0[i] * th[i].cos();
        st[4][i] = v0[i] * th[i].sin();
        st[6][i] = s0[i] * v0[i];
        oy[i] = al[i].sin();
        oz[i] = al[i].cos();
    }
    #[inline(always)]
    fn d(ux: f64, uy: f64, uz: f64, w: f64, oy: f64, oz: f64, ve: f64, he: f64) -> [f64; 4] {
        let sp = (ux * ux + uy * uy + uz * uz).sqrt();
        let k = ve * sp + he * w;
        // omega x u, omega = (0, oy, oz)
        let (cx, cy, cz) = (oy * uz - oz * uy, oz * ux, -oy * ux);
        [-k * ux + k * cx, -k * uy + k * cy - 1.0, -k * uz + k * cz, -BETA * w * sp]
    }
    let (mut ox, mut oz_out) = ([f64::NAN; N], [f64::NAN; N]);
    let mut done = [false; N];
    let mut nd = 0;
    let (h, s6) = (0.5 * dt, dt / 6.0);
    let mut steps = 0u32;
    while nd < N && steps < (6.0 / dt) as u32 + 2 {
        let mut nx = [[0.0f64; N]; 7];
        for i in 0..N {
            let (ux, uy, uz, w) = (st[3][i], st[4][i], st[5][i], st[6][i]);
            let k1 = d(ux, uy, uz, w, oy[i], oz[i], ve, he);
            let (u2x, u2y, u2z, w2) = (ux + h * k1[0], uy + h * k1[1], uz + h * k1[2], w + h * k1[3]);
            let k2 = d(u2x, u2y, u2z, w2, oy[i], oz[i], ve, he);
            let (u3x, u3y, u3z, w3) = (ux + h * k2[0], uy + h * k2[1], uz + h * k2[2], w + h * k2[3]);
            let k3 = d(u3x, u3y, u3z, w3, oy[i], oz[i], ve, he);
            let (u4x, u4y, u4z, w4) = (ux + dt * k3[0], uy + dt * k3[1], uz + dt * k3[2], w + dt * k3[3]);
            let k4 = d(u4x, u4y, u4z, w4, oy[i], oz[i], ve, he);
            nx[0][i] = st[0][i] + s6 * (ux + 2.0 * u2x + 2.0 * u3x + u4x);
            nx[1][i] = st[1][i] + s6 * (uy + 2.0 * u2y + 2.0 * u3y + u4y);
            nx[2][i] = st[2][i] + s6 * (uz + 2.0 * u2z + 2.0 * u3z + u4z);
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
                    let mut s = y0 / (y0 - y1);
                    for _ in 0..4 {
                        let (s2, s3) = (s * s, s * s * s);
                        let f = (2.0 * s3 - 3.0 * s2 + 1.0) * y0 + (s3 - 2.0 * s2 + s) * m0 + (-2.0 * s3 + 3.0 * s2) * y1 + (s3 - s2) * m1;
                        let df = (6.0 * s2 - 6.0 * s) * y0 + (3.0 * s2 - 4.0 * s + 1.0) * m0 + (-6.0 * s2 + 6.0 * s) * y1 + (3.0 * s2 - 2.0 * s) * m1;
                        s = (s - f / df).clamp(0.0, 1.0);
                    }
                    let (s2, s3) = (s * s, s * s * s);
                    let hh = [2.0 * s3 - 3.0 * s2 + 1.0, s3 - 2.0 * s2 + s, -2.0 * s3 + 3.0 * s2, s3 - s2];
                    ox[i] = hh[0] * st[0][i] + hh[1] * st[3][i] * dt + hh[2] * nx[0][i] + hh[3] * nx[3][i] * dt;
                    oz_out[i] = hh[0] * st[2][i] + hh[1] * st[5][i] * dt + hh[2] * nx[2][i] + hh[3] * nx[5][i] * dt;
                    done[i] = true;
                    nd += 1;
                }
            }
        }
        st = nx;
    }
    (ox, oz_out)
}

fn run3<const N: usize>(shots: &[[f64; 4]], dt: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(shots.len());
    for ch in shots.chunks(N) {
        let mut a = [[0.0f64; N]; 4];
        for q in 0..4 {
            a[q] = [ch[0][q]; N];
        }
        for (i, s) in ch.iter().enumerate() {
            for q in 0..4 {
                a[q][i] = s[q];
            }
        }
        let (x, z) = fly3::<N>(&a[0], &a[1], &a[2], &a[3], dt);
        out.extend((0..ch.len()).map(|i| (x[i], z[i])));
    }
    out
}

// ------------------------------------------------------------------ 4차원 장

#[inline(always)]
fn cr(t: f32) -> [f32; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [0.5 * (-t3 + 2.0 * t2 - t), 0.5 * (3.0 * t3 - 5.0 * t2 + 2.0), 0.5 * (-3.0 * t3 + 4.0 * t2 + t), 0.5 * (t3 - t2)]
}

struct Field4 {
    x0: [f32; 4],
    inv: [f32; 4],
    n: [usize; 4],
    st: [usize; 4],
    gx: Vec<f32>,
    gz: Vec<f32>,
}

impl Field4 {
    /// n = (v0', theta, S0, a≥0 칸 수). a 축은 음수 쪽 2칸을 대칭으로 덧붙인다.
    fn unfold(n: [usize; 4], a_max_deg: f64, dt: f64, threads: usize) -> Field4 {
        let lo = [0.8, 0.0, 0.02];
        let hi = [3.9, 50f64.to_radians(), 0.62];
        let da = a_max_deg.to_radians() / (n[3] - 1) as f64;
        let na = n[3] + 2; // 음수 쪽 2칸
        let dims = [n[0], n[1], n[2], na];
        let st = [n[1] * n[2] * na, n[2] * na, na, 1];
        let at = |q: usize, i: usize| lo[q] + (hi[q] - lo[q]) * i as f64 / (n[q] - 1) as f64;
        // a >= 0 칸만 적분
        let pts: Vec<[f64; 4]> = (0..n[0] * n[1] * n[2] * n[3])
            .map(|k| {
                let (i, j, m, l) = (k / (n[1] * n[2] * n[3]), (k / (n[2] * n[3])) % n[1], (k / n[3]) % n[2], k % n[3]);
                [at(0, i), at(1, j), at(2, m), da * l as f64]
            })
            .collect();
        let mut res = vec![(0.0, 0.0); pts.len()];
        let per = (pts.len() + threads - 1) / threads;
        std::thread::scope(|sc| {
            for (p, r) in pts.chunks(per).zip(res.chunks_mut(per)) {
                sc.spawn(move || r.copy_from_slice(&run3::<8>(p, dt)));
            }
        });
        let total = dims.iter().product();
        let (mut gx, mut gz) = (vec![0f32; total], vec![0f32; total]);
        for (k, &(x, z)) in res.iter().enumerate() {
            let (i, j, m, l) = (k / (n[1] * n[2] * n[3]), (k / (n[2] * n[3])) % n[1], (k / n[3]) % n[2], k % n[3]);
            let b = i * st[0] + j * st[1] + m * st[2];
            gx[b + l + 2] = x as f32;
            gz[b + l + 2] = z as f32;
            if l == 1 || l == 2 {
                // a = -da, -2da  ←  a = +da, +2da
                gx[b + 2 - l] = x as f32;
                gz[b + 2 - l] = -z as f32;
            }
        }
        Field4 {
            x0: [lo[0] as f32, lo[1] as f32, lo[2] as f32, (-2.0 * da) as f32],
            inv: [
                ((n[0] - 1) as f64 / (hi[0] - lo[0])) as f32,
                ((n[1] - 1) as f64 / (hi[1] - lo[1])) as f32,
                ((n[2] - 1) as f64 / (hi[2] - lo[2])) as f32,
                (1.0 / da) as f32,
            ],
            n: dims,
            st,
            gx,
            gz,
        }
    }

    fn bytes(&self) -> usize {
        (self.gx.len() + self.gz.len()) * 4
    }

    #[inline(always)]
    fn cell(&self, q: usize, x: f32) -> (usize, f32) {
        let f = ((x - self.x0[q]) * self.inv[q]).clamp(1.0, (self.n[q] - 3) as f32 + 0.999_99);
        let i = f as usize;
        (i, f - i as f32)
    }

    /// 4선형 (16점). 대칭 처리: a<0 이면 |a| 로 읽고 z 부호 반전.
    #[inline(always)]
    fn read_lin(&self, v: f32, th: f32, s: f32, a: f32) -> (f32, f32) {
        let sg = if a < 0.0 { -1.0 } else { 1.0 };
        let (c0, c1, c2, c3) = (self.cell(0, v), self.cell(1, th), self.cell(2, s), self.cell(3, a.abs()));
        let b = c0.0 * self.st[0] + c1.0 * self.st[1] + c2.0 * self.st[2] + c3.0;
        let (mut x, mut z) = (0f32, 0f32);
        for (di, wi) in [(0, 1.0 - c0.1), (1, c0.1)] {
            for (dj, wj) in [(0, 1.0 - c1.1), (1, c1.1)] {
                for (dk, wk) in [(0, 1.0 - c2.1), (1, c2.1)] {
                    let o = b + di * self.st[0] + dj * self.st[1] + dk * self.st[2];
                    let w = wi * wj * wk;
                    // SAFETY: cell() 이 인덱스를 [1, n-3] 로 자른다
                    unsafe {
                        x += w * ((1.0 - c3.1) * self.gx.get_unchecked(o) + c3.1 * self.gx.get_unchecked(o + 1));
                        z += w * ((1.0 - c3.1) * self.gz.get_unchecked(o) + c3.1 * self.gz.get_unchecked(o + 1));
                    }
                }
            }
        }
        (x, sg * z)
    }

    /// 앞 3축 Catmull-Rom 3차 + a 축 선형 (64×2 점)
    #[inline(always)]
    fn read_c3l1(&self, v: f32, th: f32, s: f32, a: f32) -> (f32, f32) {
        let sg = if a < 0.0 { -1.0 } else { 1.0 };
        let (c0, c1, c2, c3) = (self.cell(0, v), self.cell(1, th), self.cell(2, s), self.cell(3, a.abs()));
        let (w0, w1, w2) = (cr(c0.1), cr(c1.1), cr(c2.1));
        let b = (c0.0 - 1) * self.st[0] + (c1.0 - 1) * self.st[1] + (c2.0 - 1) * self.st[2] + c3.0;
        let (ta, tb) = (1.0 - c3.1, c3.1);
        let (mut x, mut z) = (0f32, 0f32);
        for p in 0..4 {
            for q in 0..4 {
                for r in 0..4 {
                    let o = b + p * self.st[0] + q * self.st[1] + r * self.st[2];
                    let w = w0[p] * w1[q] * w2[r];
                    unsafe {
                        x += w * (ta * self.gx.get_unchecked(o) + tb * self.gx.get_unchecked(o + 1));
                        z += w * (ta * self.gz.get_unchecked(o) + tb * self.gz.get_unchecked(o + 1));
                    }
                }
            }
        }
        (x, sg * z)
    }

    /// 4축 모두 3차 (256×2 점)
    #[inline(always)]
    fn read_c4(&self, v: f32, th: f32, s: f32, a: f32) -> (f32, f32) {
        let sg = if a < 0.0 { -1.0 } else { 1.0 };
        let (c0, c1, c2, c3) = (self.cell(0, v), self.cell(1, th), self.cell(2, s), self.cell(3, a.abs()));
        let (w0, w1, w2, w3) = (cr(c0.1), cr(c1.1), cr(c2.1), cr(c3.1));
        let b = (c0.0 - 1) * self.st[0] + (c1.0 - 1) * self.st[1] + (c2.0 - 1) * self.st[2] + (c3.0 - 1);
        let (mut x, mut z) = (0f32, 0f32);
        for p in 0..4 {
            for q in 0..4 {
                for r in 0..4 {
                    let o = b + p * self.st[0] + q * self.st[1] + r * self.st[2];
                    let w = w0[p] * w1[q] * w2[r];
                    unsafe {
                        let gx = self.gx.get_unchecked(o..o + 4);
                        let gz = self.gz.get_unchecked(o..o + 4);
                        x += w * (w3[0] * gx[0] + w3[1] * gx[1] + w3[2] * gx[2] + w3[3] * gx[3]);
                        z += w * (w3[0] * gz[0] + w3[1] * gz[1] + w3[2] * gz[2] + w3[3] * gz[3]);
                    }
                }
            }
        }
        (x, sg * z)
    }
}

// ------------------------------------------------------------------ 각도 모드로 전개한 3차원 장

/// 기울기 a 를 펼치지 않고 s = sin a 의 거듭제곱 모드로 전개한다.
/// 대칭(a -> -a 에서 x 짝, z 홀)을 꼴 자체에 넣었다:
///   x = c0 + c1 s^2 + c2 s^4,   z = c3 s + c4 s^3 + c5 s^5
/// 격자점마다 계수 6개를 이웃하게 저장하므로 3차원 3차 읽기 한 번에 6개를 함께 읽는다.
struct FieldModes<const K: usize, const M: usize> {
    x0: [f32; 3],
    inv: [f32; 3],
    n: [usize; 3],
    st: [usize; 3],
    c: Vec<[f32; M]>, // M = 2K: 앞 K 개는 x 의 짝수 모드, 뒤 K 개는 z 의 홀수 모드
}

/// 작은 정규방정식 (K x K) 풀이
fn lsq<const K: usize>(rows: &[[f64; K]], y: &[f64]) -> [f64; K] {
    let mut a = vec![vec![0.0f64; K + 1]; K];
    for (r, &yy) in rows.iter().zip(y) {
        for i in 0..K {
            for j in 0..K {
                a[i][j] += r[i] * r[j];
            }
            a[i][K] += r[i] * yy;
        }
    }
    for c in 0..K {
        let p = (c..K).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        for r in 0..K {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..=K {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    std::array::from_fn(|i| a[i][K] / a[i][i])
}

impl<const K: usize, const M: usize> FieldModes<K, M> {
    fn unfold(n: [usize; 3], n_a: usize, a_max_deg: f64, dt: f64, threads: usize) -> Self {
        let lo = [0.8, 0.0, 0.02];
        let hi = [3.9, 50f64.to_radians(), 0.62];
        let at = |q: usize, i: usize| lo[q] + (hi[q] - lo[q]) * i as f64 / (n[q] - 1) as f64;
        let alphas: Vec<f64> = (0..n_a).map(|j| a_max_deg.to_radians() * j as f64 / (n_a - 1) as f64).collect();
        let nodes = n[0] * n[1] * n[2];
        let pts: Vec<[f64; 4]> = (0..nodes * n_a)
            .map(|k| {
                let (node, j) = (k / n_a, k % n_a);
                let (i, jj, m) = (node / (n[1] * n[2]), (node / n[2]) % n[1], node % n[2]);
                [at(0, i), at(1, jj), at(2, m), alphas[j]]
            })
            .collect();
        let mut res = vec![(0.0, 0.0); pts.len()];
        let per = (pts.len() + threads - 1) / threads;
        std::thread::scope(|sc| {
            for (p, r) in pts.chunks(per).zip(res.chunks_mut(per)) {
                sc.spawn(move || r.copy_from_slice(&run3::<8>(p, dt)));
            }
        });
        let even: Vec<[f64; K]> = alphas.iter().map(|a| { let s = a.sin(); std::array::from_fn(|i| s.powi(2 * i as i32)) }).collect();
        let odd: Vec<[f64; K]> = alphas.iter().map(|a| { let s = a.sin(); std::array::from_fn(|i| s.powi(2 * i as i32 + 1)) }).collect();
        let mut c = vec![[0f32; M]; nodes];
        for (node, cc) in c.iter_mut().enumerate() {
            let xs: Vec<f64> = (0..n_a).map(|j| res[node * n_a + j].0).collect();
            let zs: Vec<f64> = (0..n_a).map(|j| res[node * n_a + j].1).collect();
            let ce = lsq::<K>(&even, &xs);
            // z(0) = 0 행은 홀수 기저에서 정보가 없으므로 빼고 푼다
            let co = lsq::<K>(&odd[1..], &zs[1..]);
            for i in 0..K {
                cc[i] = ce[i] as f32;
                cc[K + i] = co[i] as f32;
            }
        }
        FieldModes {
            x0: [lo[0] as f32, lo[1] as f32, lo[2] as f32],
            inv: [0, 1, 2].map(|q| ((n[q] - 1) as f64 / (hi[q] - lo[q])) as f32),
            n,
            st: [n[1] * n[2], n[2], 1],
            c,
        }
    }

    fn bytes(&self) -> usize {
        self.c.len() * M * 4
    }

    /// 마지막 입력은 sin a. 스핀축 벡터의 수직 성분이 곧 sin a 라 삼각함수가 필요 없다.
    #[inline(always)]
    fn read_cubic(&self, v: f32, th: f32, s: f32, sa: f32) -> (f32, f32) {
        let cell = |q: usize, x: f32| {
            let f = ((x - self.x0[q]) * self.inv[q]).clamp(1.0, (self.n[q] - 3) as f32 + 0.999_99);
            let i = f as usize;
            (i, f - i as f32)
        };
        let (c0, c1, c2) = (cell(0, v), cell(1, th), cell(2, s));
        let (w0, w1, w2) = (cr(c0.1), cr(c1.1), cr(c2.1));
        let b = (c0.0 - 1) * self.st[0] + (c1.0 - 1) * self.st[1] + (c2.0 - 1);
        // 누적기를 r 별로 4개 두어 덧셈 지연 사슬을 4갈래로 끊는다
        let mut part = [[0f32; M]; 4];
        for p in 0..4 {
            for q in 0..4 {
                let row = b + p * self.st[0] + q * self.st[1];
                let wpq = w0[p] * w1[q];
                for r in 0..4 {
                    // SAFETY: cell 이 [1, n-3] 로 잘라 row + 3 < len
                    let g = unsafe { self.c.get_unchecked(row + r) };
                    let w = wpq * w2[r];
                    for m in 0..M {
                        part[r][m] += w * g[m];
                    }
                }
            }
        }
        let acc: [f32; M] = std::array::from_fn(|m| (part[0][m] + part[1][m]) + (part[2][m] + part[3][m]));
        let s2 = sa * sa;
        let (mut x, mut z) = (acc[K - 1], acc[M - 1]);
        for i in (0..K - 1).rev() {
            x = acc[i] + s2 * x;
            z = acc[K + i] + s2 * z;
        }
        (x, sa * z)
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
}

fn bench<F: FnMut() -> f64>(n: usize, reps: usize, mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t0 = Instant::now();
        black_box(f());
        best = best.min(t0.elapsed().as_nanos() as f64 / n as f64);
    }
    best
}

fn stats(e: &[f64]) -> (f64, f64) {
    ((e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt(), e.iter().fold(0.0f64, |m, x| m.max(x.abs())))
}

fn throughput<F: Fn(f32, f32, f32, f32) -> (f32, f32) + Sync>(q: &[[f32; 4]], threads: usize, f: F) -> f64 {
    let ns = bench(q.len(), 5, || {
        let per = q.len() / threads;
        std::thread::scope(|sc| {
            let f = &f;
            let hs: Vec<_> = q
                .chunks(per)
                .map(|c| {
                    sc.spawn(move || {
                        let mut acc = 0f32;
                        for p in c {
                            let (x, z) = f(p[0], p[1], p[2], p[3]);
                            acc += x + z;
                        }
                        acc as f64
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).sum()
        })
    });
    1e9 / ns
}

fn main() {
    // ---- 0. 파이썬 일치
    let refs: Vec<[f64; 6]> = std::fs::read_to_string("../refs3d.csv")
        .expect("refs3d.csv")
        .lines()
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3], v[4], v[5]]
        })
        .collect();
    let rs: Vec<[f64; 4]> = refs.iter().map(|r| [r[0], r[1], r[2], r[3]]).collect();
    let out = run3::<8>(&rs, 0.0005);
    let dev = refs.iter().zip(&out).fold(0.0f64, |m, (r, o)| m.max(((o.0 - r[4]).abs()).max((o.1 - r[5]).abs()) * YD));
    println!("== 0. 파이썬 field3d 와 일치: 40샷 최대 차이 {:.2e} yd", dev);

    // ---- 무작위 샷과 기준값 (a 는 음수 포함)
    let n = 3000;
    let mut rng = Rng(0xA5A5_1234_5678_9ABC);
    let shots: Vec<[f64; 4]> = (0..n)
        .map(|_| [rng.uni(1.56, 3.61), rng.uni(5f64.to_radians(), 40f64.to_radians()), rng.uni(0.05, 0.55), rng.uni(-40f64.to_radians(), 40f64.to_radians())])
        .collect();
    let truth = run3::<8>(&shots, 0.0005);
    let q32: Vec<[f32; 4]> = shots.iter().map(|s| [s[0] as f32, s[1] as f32, s[2] as f32, s[3] as f32]).collect();

    // ---- 1. 직접 적분 비용
    println!("\n== 1. 3차원 직접 적분 (f64×8)");
    for dt in [0.064, 0.032] {
        let ns = bench(n, 5, || run3::<8>(&shots, dt).iter().map(|p| p.0).sum());
        let r = run3::<8>(&shots, dt);
        let e: Vec<f64> = r.iter().zip(&truth).map(|(a, b)| ((a.0 - b.0).abs()).max((a.1 - b.1).abs()) * YD).collect();
        println!("  dt'={:<6} {:>6.0} ns/샷   최대 오차 {:.2e} yd", dt, ns, stats(&e).1);
    }

    // ---- 2. 4차원 장
    let big = 1_000_000;
    let mut rng = Rng(77);
    let qbig: Vec<[f32; 4]> = (0..big)
        .map(|_| [rng.uni(1.56, 3.61) as f32, rng.uni(5f64.to_radians(), 40f64.to_radians()) as f32, rng.uni(0.05, 0.55) as f32, rng.uni(-40f64.to_radians(), 40f64.to_radians()) as f32])
        .collect();
    println!("\n== 2. 4차원 장 읽기 (f32)");
    println!("  격자(a≥0)            크기     펼치기  읽기      x 최대(yd)  z 최대(yd)  ns/샷   1코어/초   2코어/초");
    for g in [[32, 51, 31, 16], [32, 51, 31, 31]] {
        let t0 = Instant::now();
        let f4 = Field4::unfold(g, 45.0, 0.032, 2);
        let tb = t0.elapsed().as_secs_f64();
        type R = fn(&Field4, f32, f32, f32, f32) -> (f32, f32);
        let modes: [(&str, R); 3] = [("4선형  ", Field4::read_lin), ("3차+선형", Field4::read_c3l1), ("4차원3차", Field4::read_c4)];
        for (name, rd) in modes {
            let (mut ex, mut ez) = (Vec::new(), Vec::new());
            for (p, t) in q32.iter().zip(&truth) {
                let (x, z) = rd(&f4, p[0], p[1], p[2], p[3]);
                ex.push((x as f64 - t.0) * YD);
                ez.push((z as f64 - t.1) * YD);
            }
            let ns = bench(n, 30, || q32.iter().map(|p| { let (x, z) = rd(&f4, p[0], p[1], p[2], p[3]); (x + z) as f64 }).sum());
            let t1 = throughput(&qbig, 1, |a, b, c, d| rd(&f4, a, b, c, d));
            let t2 = throughput(&qbig, 2, |a, b, c, d| rd(&f4, a, b, c, d));
            println!(
                "  {:>2}×{:>3}×{:>2}×{:>2}  {:>7.1} MB  {:>5.1} s  {}  {:>9.4}   {:>9.4}  {:>6.1}  {:>9.2e}  {:>9.2e}",
                g[0], g[1], g[2], g[3], f4.bytes() as f64 / 1e6, tb, name, stats(&ex).1, stats(&ez).1, ns, t1, t2
            );
        }
    }

    // ---- 3. 각도 모드 전개: 3차원 장 하나에 계수 6개
    println!("\n== 3. 각도 모드 전개 (x: 1, s², s⁴ / z: s, s³, s⁵, s = sin a), 3차원 3차 읽기");
    println!("  모드 격자      a 표본   크기     펼치기   x 최대(yd)  z 최대(yd)  x RMSE   z RMSE   ns/샷   1코어/초   2코어/초");
    let qs: Vec<[f32; 4]> = q32.iter().map(|p| [p[0], p[1], p[2], p[3].sin()]).collect();
    let qbs: Vec<[f32; 4]> = qbig.iter().map(|p| [p[0], p[1], p[2], p[3].sin()]).collect();
    macro_rules! modes_run { ($k:expr, $m:expr, $g:expr, $na:expr) => {{
        let (g, na): ([usize; 3], usize) = ($g, $na);
        let t0 = Instant::now();
        let fm = FieldModes::<$k, $m>::unfold(g, na, 45.0, 0.032, 2);
        let tb = t0.elapsed().as_secs_f64();
        let (mut ex, mut ez) = (Vec::new(), Vec::new());
        for (p, t) in qs.iter().zip(&truth) {
            let (x, z) = fm.read_cubic(p[0], p[1], p[2], p[3]);
            ex.push((x as f64 - t.0) * YD);
            ez.push((z as f64 - t.1) * YD);
        }
        let ns = bench(n, 30, || qs.iter().map(|p| { let (x, z) = fm.read_cubic(p[0], p[1], p[2], p[3]); (x + z) as f64 }).sum());
        let t1 = throughput(&qbs, 1, |a, b, c, d| fm.read_cubic(a, b, c, d));
        let t2 = throughput(&qbs, 2, |a, b, c, d| fm.read_cubic(a, b, c, d));
        let (sx, sz) = (stats(&ex), stats(&ez));
        println!(
            "  K={} {:>2}×{:>3}×{:>2} {:>4}   {:>6.2} MB  {:>5.1} s  {:>9.4}   {:>9.4}  {:>7.4}  {:>7.4}  {:>6.1}  {:>9.2e}  {:>9.2e}",
            $k, g[0], g[1], g[2], na, fm.bytes() as f64 / 1e6, tb, sx.1, sz.1, sx.0, sz.0, ns, t1, t2
        );
    }}; }
    modes_run!(3, 6, [32, 51, 31], 10);
    modes_run!(4, 8, [32, 51, 31], 12);
    modes_run!(5, 10, [32, 51, 31], 14);
    modes_run!(4, 8, [24, 38, 24], 12);
}
