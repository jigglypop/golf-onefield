//! 자료 대비 정확도: 훈련은 PGA 투어 평균 12행(드라이버~PW)만, 판정은 훈련에 안 쓴 양과 자료로.
//!
//! 자료: TrackMan PGA·LPGA 투어 평균 (볼스피드, 발사각, 스핀 → 캐리, 최고 높이, 착지각).
//!   이전 판(7행)은 3번~9번 아이언만 썼다. 같은 표의 드라이버·우드·하이브리드·PW 를 넣어 S 범위를 넓힌다.
//!   USGA 는 여전히 보지 않는다.
//! 맞춤 A: PGA 12행 캐리만.   맞춤 B: PGA 12행 캐리 + 최고 높이 + 착지각 (36개 조건, 단위 yd·yd·deg 그대로).
//! 판정: PGA 높이·착지각(맞춤 A 에서는 안 본 양), LPGA 11행 세 양, USGA 20행 분포 평균 캐리 (행당 20만 표본).
//!
//! 결과 (2026-10-08). RMSE: 캐리 yd / 최고 높이 yd / 착지각 deg
//!                         PGA 훈련            LPGA 보류           USGA 캐리 (전체/프로/아마)
//!   H2 A 캐리만    4매  1.67 13.41 23.22    5.93  9.90 18.93    9.76 13.93  8.40
//!   H2 B 세 양     4매  5.17  3.06  4.16    3.87  1.63  4.35    9.91 16.39  7.45
//!   B4 A 캐리만    4매  1.75  8.37 14.88    3.62  6.78 12.91    7.02 14.30  3.23
//!   B4 B 세 양     4매  4.48  2.68  4.73    3.70  1.49  4.58    9.00 13.27  7.56
//!   Z0 (맞춤 없음) 0매  7.76  9.25  4.52    3.59  5.08  3.71   11.81 13.59 11.32
//!   이전 H2 (7행)  4매  6.42  3.43  5.48    3.35  2.24  5.26    9.75 18.49  5.78
//! 판독
//!   - [반례] 캐리만 맞추면 항력과 양력이 서로 바꿔치기된다. 맞춤 A 는 캐리를 1.7 yd 로 맞추면서 높이를 13 yd 낮게,
//!     착지각을 23° 얕게 준다. USGA 캐리는 이 꼴 오류를 못 잡는다(H2e A 가 USGA 5.95 로 가장 좋지만 높이 -12 yd).
//!     그래서 보정의 기준은 맞춤 B(세 양)로 바꾼다.
//!   - 맞춤 B 에서 H2 와 B4 는 대등하다 (LPGA 3.9/1.6/4.4 대 3.7/1.5/4.6, USGA 9.9 대 9.0). 물리 정확도의 압도는 없다.
//!     H2 의 이점은 같은 정확도를 powf 없이 sqrt 하나로 낸다는 계산 쪽에 있다.
//!   - 남는 계통 오차: 아이언 착지각이 4–6° 얕고 PGA 드라이버 캐리가 -13 yd. 세 가지 보강을 시험했고 모두 기각했다.
//!     e/|u| 항(H2e·B4e), 스핀 감쇠 자유화(H2s·B4s: β→0 으로 가며 개선 없음), 레이놀즈 계단 e/(1+(|u|/u0)^2)(H2r·B4r).
//!     e 항과 계단은 훈련 착지각을 2° 로 줄이지만 LPGA 캐리 9–10 yd, USGA 12–13 yd 로 보류가 무너진다.
//!   - 0매개 Z0 는 LPGA 캐리 3.59 로 맞춘 모형들과 같은 수준이나 높이·착지 꼴이 틀린다(드라이버 높이 +23 yd).
//!
//! cargo run --release --bin calib [행당 표본 수]       KINDS=H2,B4 로 모형을 고를 수 있다

use flightbench::engine::*;
use std::time::Instant;

/// (볼스피드 mph, 발사각 deg, 스핀 rpm, 캐리 yd, 최고 높이 yd, 착지각 deg)
const PGA12: [[f64; 6]; 12] = [
    [167.0, 10.9, 2686.0, 275.0, 32.0, 38.0],
    [158.0, 9.2, 3655.0, 243.0, 30.0, 43.0],
    [152.0, 9.4, 4350.0, 230.0, 31.0, 47.0],
    [146.0, 10.2, 4437.0, 225.0, 29.0, 47.0],
    [142.0, 10.4, 4630.0, 212.0, 27.0, 46.0],
    [137.0, 11.0, 4836.0, 203.0, 28.0, 48.0],
    [132.0, 12.1, 5361.0, 194.0, 31.0, 49.0],
    [127.0, 14.1, 6231.0, 183.0, 30.0, 50.0],
    [120.0, 16.3, 7097.0, 172.0, 32.0, 50.0],
    [115.0, 18.1, 7998.0, 160.0, 31.0, 50.0],
    [109.0, 20.4, 8647.0, 148.0, 30.0, 51.0],
    [102.0, 24.2, 9304.0, 136.0, 29.0, 52.0],
];
const PGA_LAB: [&str; 12] = ["DR", "3W", "5W", "HY", "3i", "4i", "5i", "6i", "7i", "8i", "9i", "PW"];
const LPGA11: [[f64; 6]; 11] = [
    [140.0, 13.2, 2611.0, 218.0, 25.0, 37.0],
    [132.0, 11.2, 2704.0, 195.0, 23.0, 39.0],
    [128.0, 12.1, 4501.0, 185.0, 26.0, 43.0],
    [123.0, 12.7, 4693.0, 174.0, 25.0, 46.0],
    [116.0, 14.3, 4801.0, 169.0, 24.0, 43.0],
    [112.0, 14.8, 5081.0, 161.0, 23.0, 45.0],
    [109.0, 17.1, 5943.0, 152.0, 25.0, 46.0],
    [104.0, 19.0, 6699.0, 141.0, 26.0, 47.0],
    [100.0, 20.8, 7494.0, 130.0, 25.0, 47.0],
    [93.0, 23.9, 7589.0, 119.0, 26.0, 47.0],
    [86.0, 25.7, 8403.0, 107.0, 23.0, 48.0],
];
const LPGA_LAB: [&str; 11] = ["DR", "3W", "5W", "7W", "4i", "5i", "6i", "7i", "8i", "9i", "PW"];

/// 2차원(사이드 스핀 0) f64 RK4: (캐리 yd, 최고 높이 yd, 착지각 deg)
fn obs<A: Aero>(ae: &A, mph: f64, deg: f64, rpm: f64, dt: f64) -> [f64; 3] {
    let [v0, th, s0] = dimless(mph, deg, rpm);
    let f = |s: &[f64; 5]| -> [f64; 5] {
        let sp = (s[2] * s[2] + s[3] * s[3]).sqrt();
        let (kd, kl) = ae.kk(sp, s[4]);
        [s[2], s[3], -kd * s[2] - kl * s[3], -kd * s[3] + kl * s[2] - 1.0, -ae.beta() * s[4] * sp]
    };
    let mut s = [0.0, 0.0, v0 * th.cos(), v0 * th.sin(), s0 * v0];
    let (mut ymax, mut yprev, mut yprev2) = (0.0f64, 0.0f64, f64::NAN);
    for step in 0..(10.0 / dt) as usize {
        let k1 = f(&s);
        let a: [f64; 5] = std::array::from_fn(|i| s[i] + 0.5 * dt * k1[i]);
        let k2 = f(&a);
        let b: [f64; 5] = std::array::from_fn(|i| s[i] + 0.5 * dt * k2[i]);
        let k3 = f(&b);
        let c: [f64; 5] = std::array::from_fn(|i| s[i] + dt * k3[i]);
        let k4 = f(&c);
        let n: [f64; 5] = std::array::from_fn(|i| s[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
        // 최고점: 세 점 포물선
        if step > 0 && yprev >= yprev2 && yprev >= n[1] {
            let den = yprev2 - 2.0 * yprev + n[1];
            let off = if den.abs() > 1e-300 { 0.5 * (yprev2 - n[1]) / den } else { 0.0 };
            ymax = ymax.max(yprev - 0.25 * (yprev2 - n[1]) * off);
        }
        if step > 0 && n[1] < 0.0 {
            let fn_ = f(&n);
            let (y0, y1, m0, m1) = (s[1], n[1], s[3] * dt, n[3] * dt);
            let mut t = y0 / (y0 - y1);
            for _ in 0..4 {
                let (t2, t3) = (t * t, t * t * t);
                let fv = (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * m1;
                let df = (6.0 * t2 - 6.0 * t) * y0 + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (-6.0 * t2 + 6.0 * t) * y1 + (3.0 * t2 - 2.0 * t) * m1;
                t = (t - fv / df).clamp(0.0, 1.0);
            }
            let (t2, t3) = (t * t, t * t * t);
            let hh = [2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2];
            let x = hh[0] * s[0] + hh[1] * s[2] * dt + hh[2] * n[0] + hh[3] * n[2] * dt;
            // 속도도 에르미트 (도함수는 가속도)
            let vx = hh[0] * s[2] + hh[1] * k1[2] * dt + hh[2] * n[2] + hh[3] * fn_[2] * dt;
            let vy = hh[0] * s[3] + hh[1] * k1[3] * dt + hh[2] * n[3] + hh[3] * fn_[3] * dt;
            return [x * YD, ymax * YD, (-vy).atan2(vx).to_degrees()];
        }
        yprev2 = yprev;
        yprev = n[1];
        s = n;
    }
    [f64::NAN; 3]
}

fn solve<const P: usize>(mut a: [[f64; 16]; P]) -> [f64; P] {
    for c in 0..P {
        let p = (c..P).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        for r in 0..P {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..=P {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    std::array::from_fn(|i| a[i][P] / a[i][i])
}

/// 상자 제약 Levenberg–Marquardt. 반환 (매개변수, 비용)
fn lm<const P: usize>(res: &dyn Fn(&[f64; P]) -> Vec<f64>, start: [f64; P], lo: [f64; P], hi: [f64; P]) -> ([f64; P], f64) {
    let cost = |r: &[f64]| r.iter().map(|x| if x.is_finite() { x * x } else { 1e12 }).sum::<f64>();
    let mut p = start;
    let mut r = res(&p);
    let mut mu = 1e-3;
    for _ in 0..300 {
        let m = r.len();
        let mut jac = vec![[0.0f64; P]; m];
        for k in 0..P {
            let h = 1e-6 * p[k].abs().max(1e-3);
            let mut q = p;
            q[k] += h;
            let rq = res(&q);
            for i in 0..m {
                jac[i][k] = (rq[i] - r[i]) / h;
            }
        }
        let mut improved = false;
        for _ in 0..14 {
            let mut a = [[0.0f64; 16]; P];
            for i in 0..m {
                for x in 0..P {
                    a[x][P] -= jac[i][x] * r[i];
                    for y in 0..P {
                        a[x][y] += jac[i][x] * jac[i][y];
                    }
                }
            }
            for x in 0..P {
                a[x][x] *= 1.0 + mu;
                a[x][x] += 1e-12;
            }
            let dp = solve::<P>(a);
            let q: [f64; P] = std::array::from_fn(|k| (p[k] + dp[k]).clamp(lo[k], hi[k]));
            let rq = res(&q);
            if cost(&rq) < cost(&r) {
                let rel = (cost(&r) - cost(&rq)) / cost(&r).max(1e-30);
                p = q;
                r = rq;
                mu = (mu * 0.3).max(1e-12);
                improved = true;
                if rel < 1e-13 {
                    return (p, cost(&r));
                }
                break;
            }
            mu *= 10.0;
        }
        if !improved {
            break;
        }
    }
    (p, cost(&r))
}

/// 레이놀즈 계단: C_D 에 e / (1 + (|u|/u0)^2) — 저속에서 e 만큼 오르고 고속에서 사라진다
#[derive(Clone, Copy)]
struct ReStep<A: Aero> {
    a: A,
    e: f64,
    u0: f64,
}
impl<A: Aero> Aero for ReStep<A> {
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let (kd, kl) = self.a.kk(sp, w);
        let r = sp / self.u0;
        (kd + sp * self.e / (1.0 + r * r), kl)
    }
}

#[derive(Clone, Copy)]
enum Model {
    H2(H2),
    B4(B4),
    F3(F3),
    H2s(WithBeta<H2>),
    B4s(WithBeta<B4>),
    H2r(ReStep<H2>),
    B4r(ReStep<B4>),
}
impl Model {
    fn obs(&self, r: &[f64; 6]) -> [f64; 3] {
        const DT: f64 = 0.004;
        match self {
            Model::H2(m) => obs(m, r[0], r[1], r[2], DT),
            Model::B4(m) => obs(m, r[0], r[1], r[2], DT),
            Model::F3(m) => obs(m, r[0], r[1], r[2], DT),
            Model::H2s(m) => obs(m, r[0], r[1], r[2], DT),
            Model::B4s(m) => obs(m, r[0], r[1], r[2], DT),
            Model::H2r(m) => obs(m, r[0], r[1], r[2], DT),
            Model::B4r(m) => obs(m, r[0], r[1], r[2], DT),
        }
    }
    fn usga(&self, n: usize) -> Vec<f64> {
        match self {
            Model::H2(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
            Model::B4(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
            Model::F3(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
            Model::H2s(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
            Model::B4s(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
            Model::H2r(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
            Model::B4r(m) => usga_dist(m, n, 0.064, 1).iter().map(|p| p.0).collect(),
        }
    }
}

fn residuals(m: &Model, rows: &[[f64; 6]], all3: bool) -> Vec<f64> {
    let mut out = Vec::new();
    for r in rows {
        let o = m.obs(r);
        out.push(o[0] - r[3]);
        if all3 {
            out.push(o[1] - r[4]);
            out.push(o[2] - r[5]);
        }
    }
    out
}

fn fit(kind: &str, all3: bool) -> (Model, usize) {
    let rows = &PGA12[..];
    let best = |c: Vec<(Model, f64)>| c.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
    match kind {
        "H2" => {
            let mk = |p: &[f64; 4]| Model::H2(H2 { v: p[0], d: p[1], c: p[2], lam: p[3], e: 0.0 });
            let starts = [[0.25, 0.2, 2.0, 5.0], [0.22, 0.3, 4.0, 12.0], [0.3, 0.1, 1.5, 2.0], [0.27, 0.15, 3.1, 10.2]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<4>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0, 0.0, 0.0], [0.6, 2.0, 20.0, 80.0]); (mk(&p), c) }).collect();
            (best(c), 4)
        }
        "H2e" => {
            let mk = |p: &[f64; 5]| Model::H2(H2 { v: p[0], d: p[1], c: p[2], lam: p[3], e: p[4] });
            let starts = [[0.24, 0.25, 2.4, 7.0, 0.0], [0.2, 0.25, 2.4, 7.0, 0.1], [0.27, 0.15, 3.1, 10.2, 0.05], [0.15, 0.3, 2.0, 5.0, 0.2]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<5>(&|p| residuals(&mk(p), rows, all3), *s, [0.0, -1.0, 0.0, 0.0, -1.0], [0.6, 2.0, 20.0, 80.0, 2.0]); (mk(&p), c) }).collect();
            (best(c), 5)
        }
        "B4e" => {
            let mk = |p: &[f64; 5]| Model::B4(B4 { d0: p[0], d1: p[1], l0: p[2], p: p[3], e: p[4] });
            let starts = [[0.23, 0.3, 0.47, 0.37, 0.0], [0.18, 0.3, 0.47, 0.37, 0.1], [0.2627, 0.1852, 0.3612, 0.1765, 0.05]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<5>(&|p| residuals(&mk(p), rows, all3), *s, [0.0, -1.0, 0.01, 0.05, -1.0], [0.6, 2.0, 2.0, 2.0, 2.0]); (mk(&p), c) }).collect();
            (best(c), 5)
        }
        "H2s" => {
            let mk = |p: &[f64; 5]| Model::H2s(WithBeta { a: H2 { v: p[0], d: p[1], c: p[2], lam: p[3], e: 0.0 }, b: p[4] });
            let starts = [[0.24, 0.25, 2.4, 7.0, 0.06], [0.24, 0.25, 2.4, 7.0, 0.15], [0.27, 0.15, 3.1, 10.2, 0.03], [0.2, 0.3, 2.0, 5.0, 0.3]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<5>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0, 0.0, 0.0, 0.0], [0.6, 2.0, 20.0, 80.0, 2.0]); (mk(&p), c) }).collect();
            (best(c), 5)
        }
        "B4s" => {
            let mk = |p: &[f64; 5]| Model::B4s(WithBeta { a: B4 { d0: p[0], d1: p[1], l0: p[2], p: p[3], e: 0.0 }, b: p[4] });
            let starts = [[0.23, 0.3, 0.47, 0.37, 0.06], [0.23, 0.3, 0.47, 0.37, 0.15], [0.2627, 0.1852, 0.3612, 0.1765, 0.03]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<5>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0, 0.01, 0.05, 0.0], [0.6, 2.0, 2.0, 2.0, 2.0]); (mk(&p), c) }).collect();
            (best(c), 5)
        }
        "H2r" => {
            let mk = |p: &[f64; 6]| Model::H2r(ReStep { a: H2 { v: p[0], d: p[1], c: p[2], lam: p[3], e: 0.0 }, e: p[4], u0: p[5] });
            let starts = [[0.24, 0.25, 2.4, 7.0, 0.1, 1.5], [0.2, 0.25, 2.4, 7.0, 0.2, 2.0], [0.24, 0.25, 2.4, 7.0, 0.3, 1.0], [0.18, 0.2, 2.4, 7.0, 0.15, 2.5]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<6>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0, 0.0, 0.0, 0.0, 0.3], [0.6, 2.0, 20.0, 80.0, 2.0, 5.0]); (mk(&p), c) }).collect();
            (best(c), 6)
        }
        "B4r" => {
            let mk = |p: &[f64; 6]| Model::B4r(ReStep { a: B4 { d0: p[0], d1: p[1], l0: p[2], p: p[3], e: 0.0 }, e: p[4], u0: p[5] });
            let starts = [[0.23, 0.3, 0.47, 0.37, 0.1, 1.5], [0.2, 0.3, 0.47, 0.37, 0.2, 2.0], [0.23, 0.3, 0.47, 0.37, 0.3, 1.0]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<6>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0, 0.01, 0.05, 0.0, 0.3], [0.6, 2.0, 2.0, 2.0, 2.0, 5.0]); (mk(&p), c) }).collect();
            (best(c), 6)
        }
        "B4" => {
            let mk = |p: &[f64; 4]| Model::B4(B4 { d0: p[0], d1: p[1], l0: p[2], p: p[3], e: 0.0 });
            let starts = [[0.22, 0.2, 0.5, 0.5], [0.3, 0.0, 0.3, 0.3], [0.2627, 0.1852, 0.3612, 0.1765]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<4>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0, 0.01, 0.05], [0.6, 2.0, 2.0, 2.0]); (mk(&p), c) }).collect();
            (best(c), 4)
        }
        "F3" => {
            let mk = |p: &[f64; 2]| Model::F3(F3 { vev: p[0], h: p[1] });
            let starts = [[0.3881, 0.2714], [0.3, 0.5]];
            let c = starts.iter().map(|s| { let (p, c) = lm::<2>(&|p| residuals(&mk(p), rows, all3), *s, [0.05, -1.0], [1.0, 3.0]); (mk(&p), c) }).collect();
            (best(c), 2)
        }
        _ => {
            let v = std::f64::consts::PI / 8.0;
            (Model::F3(F3 { vev: v, h: v * std::f64::consts::FRAC_1_SQRT_2 }), 0)
        }
    }
}

fn rmse_col(m: &Model, rows: &[[f64; 6]], col: usize) -> f64 {
    let e: Vec<f64> = rows.iter().map(|r| m.obs(r)[col] - r[3 + col]).collect();
    (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt()
}

fn params(m: &Model) -> String {
    match m {
        Model::H2(h) => format!("v={:.4} d={:.4} c={:.3} λ={:.2} e={:.3}", h.v, h.d, h.c, h.lam, h.e),
        Model::B4(b) => format!("d0={:.4} d1={:.4} l0={:.3} p={:.3} e={:.3}", b.d0, b.d1, b.l0, b.p, b.e),
        Model::F3(f) => format!("vev={:.4} h={:.4}", f.vev, f.h),
        Model::H2s(m) => format!("v={:.4} d={:.4} c={:.3} λ={:.2} β={:.3}", m.a.v, m.a.d, m.a.c, m.a.lam, m.b),
        Model::H2r(m) => format!("v={:.3} d={:.3} c={:.2} λ={:.1} e={:.3} u0={:.2}", m.a.v, m.a.d, m.a.c, m.a.lam, m.e, m.u0),
        Model::B4r(m) => format!("d0={:.3} d1={:.3} l0={:.3} p={:.3} e={:.3} u0={:.2}", m.a.d0, m.a.d1, m.a.l0, m.a.p, m.e, m.u0),
        Model::B4s(m) => format!("d0={:.4} d1={:.4} l0={:.3} p={:.3} β={:.3}", m.a.d0, m.a.d1, m.a.l0, m.a.p, m.b),
    }
}

fn main() {
    let nsamp: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let obs_usga: Vec<f64> = USGA.iter().map(|r| r.7).collect();
    let pro: Vec<bool> = USGA.iter().map(|r| r.0.ends_with("Pro")).collect();
    let rm = |e: &[f64]| (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt();

    println!("훈련 PGA 12행. 판정 LPGA 11행(캐리·높이·착지각), USGA 20행(분포 평균 캐리, 행당 {} 표본, dt'=0.064)", nsamp);
    println!();
    println!("{:<4} {:<5} {:>2}  {:<44} | PGA 훈련 캐리 높이 착지 | LPGA 보류 캐리 높이 착지 | USGA 전체 프로4 아마16 부호 | 맞춤 s",
        "장", "맞춤", "k", "매개변수");
    let mut keep = Vec::new();
    let prev: [(&str, Model); 3] = [
        ("H2", Model::H2(H2 { v: 0.27048651, d: 0.15362838, c: 3.13610342, lam: 10.16695585, e: 0.0 })),
        ("B4", Model::B4(B4 { d0: 0.2627, d1: 0.1852, l0: 0.3612, p: 0.1765, e: 0.0 })),
        ("F3", Model::F3(F3 { vev: 0.3881, h: 0.2714 })),
    ];
    let kinds: Vec<String> = std::env::var("KINDS").unwrap_or("H2,H2e,H2s,H2r,B4,B4e,B4s,B4r,F3,Z0,H2p,B4p,F3p".into()).split(',').map(String::from).collect();
    for kind in kinds.iter().map(|s| s.as_str()) {
        for all3 in [false, true] {
            if (kind == "Z0" || kind.ends_with('p')) && all3 {
                continue;
            }
            let t0 = Instant::now();
            let (m, k) = if let Some(p) = prev.iter().find(|p| kind.starts_with(p.0) && kind.ends_with('p')) { (p.1, 4) } else { fit(kind, all3) };
            let tf = t0.elapsed().as_secs_f64();
            let tr: Vec<f64> = (0..3).map(|c| rmse_col(&m, &PGA12, c)).collect();
            let ho: Vec<f64> = (0..3).map(|c| rmse_col(&m, &LPGA11, c)).collect();
            let u = m.usga(nsamp);
            let e: Vec<f64> = u.iter().zip(&obs_usga).map(|(a, b)| a - b).collect();
            let ep: Vec<f64> = e.iter().zip(&pro).filter(|p| *p.1).map(|p| *p.0).collect();
            let ea: Vec<f64> = e.iter().zip(&pro).filter(|p| !*p.1).map(|p| *p.0).collect();
            println!("{:<4} {:<5} {:>2}  {:<44} | {:>13.2} {:>4.2} {:>4.2} | {:>14.2} {:>4.2} {:>4.2} | {:>9.2} {:>5.2} {:>6.2} {:>+4.1} | {:>5.2}",
                kind, if kind == "Z0" { "없음" } else if kind.ends_with('p') { "7행" } else if all3 { "B 3양" } else { "A 캐리" }, k, params(&m),
                tr[0], tr[1], tr[2], ho[0], ho[1], ho[2], rm(&e), rm(&ep), rm(&ea), e.iter().sum::<f64>() / 20.0, tf);
            keep.push((kind, all3, m));
        }
    }

    println!("\n행별 (맞춤 B): 실측 → 예측 차 (캐리 yd / 높이 yd / 착지 deg)");
    for (title, rows, labs) in [("PGA", &PGA12[..], &PGA_LAB[..]), ("LPGA", &LPGA11[..], &LPGA_LAB[..])] {
        println!("  {title}");
        for (r, lab) in rows.iter().zip(labs) {
            let mut line = format!("    {:<3} {:>5.0} {:>3.0} {:>3.0} |", lab, r[3], r[4], r[5]);
            for (kind, all3, m) in &keep {
                if !*all3 && *kind != "Z0" {
                    continue;
                }
                let o = m.obs(r);
                line += &format!(" {} {:>+5.1} {:>+5.1} {:>+5.1} |", kind, o[0] - r[3], o[1] - r[4], o[2] - r[5]);
            }
            println!("{line}");
        }
    }
}
