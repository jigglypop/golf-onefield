//! 스크린을 스핀 센서로: 3 m 앞 스크린에 맞고 튀어나오는 공의 방향으로 스핀을 찾는다.
//!
//! 3 m 비행만으로는 스핀이 거의 안 보인다(spininv: 드라이버 캐리 ±7–10 yd). 그런데 스크린 충돌에서는
//! 접점 미끄럼 속도 v_c = v + ω × r_c 의 접선 성분이 반동의 접선 속도를 바꾼다. 굴림 영역(마찰이 충분)에서는
//!   Δv_t = -k_t (v_t + (ω × r_c)_t),  k_t = 2/7 (강체 구, I = 0.4 m R²),   Δω = (5 / 2R²) r_c × Δv_t
//! 이라서 1000 rpm 차이가 반동 접선 속도 (2/7)·R·ω ≈ 0.64 m/s 차이가 된다. 이 값은 마찰계수·반발계수와 무관하다.
//! 실제 천 스크린은 강체가 아니므로 k_t 를 미지수로 두고(사전값 ± 폭) 함께 맞춘다. 반발계수 e 는 자유.
//!
//! 관측: 공 뒤 카메라 240 fps, 깊이(앞뒤) 노이즈 ×5. 발사 → 스크린(3 m) → 반동 T_after 초.
//! 미지수 10개: 첫 위치 3, 볼스피드·발사각·방위각, 스핀, 축, k_t, e.
//!
//! 결과 (2026-10-08, 몬테카를로 30회, 반동 0.1 s, σ 5 mm, 참 k_t 0.20)
//!   대각 카메라 + k_t 보정: 드라이버 캐리 1.71 yd·스핀 ±202 rpm, 7번 0.87 yd, 아마7번 0.65 yd, 웨지 0.34 yd
//!   (스크린 없이 같은 3 m: 드라이버 9.7 yd, 7번 3.4 yd — spininv)
//!   k_t 모름(2/7 ± 0.1): 드라이버 5.2 yd. 공 뒤 카메라는 깊이 = 진행 방향이라 볼스피드가 흐려지고, 옆 카메라는 좌우(사이드 스핀)가 흐려진다.
//!   k_t 자가 보정 (--selfcal): 샷 여러 개가 k_t 를 공유한다고 두고 맞추면 5샷에 ±0.006, 40샷에 ±0.0023 —
//!   '보정함'(±0.01)보다 좋다. 단, 참 스핀이 투어 회귀를 평균으로 둔다는 가정에 기댄다(사용자 스핀이 체계적으로 다르면 치우침).
//!
//! 닫힌 꼴 (--closed): ω = -(Δv_t/k_t + v_t)/R 를 충돌 전·후 2차 맞춤 속도로 바로 계산 (5–16 µs/샷).
//!   단독으로는 노이즈를 미분하는 셈이라 나쁘다 (60 fps 드라이버 캐리 17.9 yd, 7번 11.4 yd).
//!   닫힌 꼴을 시작점으로 LM 한 번 (2–9 ms/샷, 격자 탐색 없음): 60 fps 드라이버 5.9 yd·7번 1.4·웨지 0.7,
//!   240 fps 드라이버 3.1·7번 0.7·웨지 0.35 yd. 격자 66만 후보가 필요 없어진다.
//!
//! cargo run --release --bin screen [몬테카를로 횟수]     AFTER=0.05,0.1,0.15 로 반동 관측 시간 바꾸기, --selfcal

use flightbench::engine::{dimless, run, Aero, SharedRe, U_MS, YD};
use flightbench::harness::RE_PER_U;

const G: f64 = 9.81;
const TU: f64 = U_MS / G;
const R: f64 = 0.021335;
const NP: usize = 10;

fn field() -> SharedRe {
    SharedRe { v: 0.1330, e: 0.3247, u0: 62859.6581 / RE_PER_U, d: 1.3688, c: 2.7901, l: 5.9226 }
}

#[derive(Clone, Copy)]
struct Screen {
    dist: f64, // 공 중심 출발점에서 스크린 면까지 (m)
    mu: f64,
}

#[inline]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// 상태 (실단위): 위치 m, 속도 m/s, 스핀 벡터 rad/s
fn accel(ae: &SharedRe, v: [f64; 3], om: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let u = v.map(|x| x / U_MS);
    let sp = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
    let omn = (om[0] * om[0] + om[1] * om[1] + om[2] * om[2]).sqrt();
    let w = R * omn / U_MS;
    let (kd, kl) = ae.kk(sp, w);
    let oh = if omn > 1e-9 { om.map(|x| x / omn) } else { [0.0, 0.0, 1.0] };
    let c = cross(oh, u);
    // 무차원 가속도 × g
    let a = [(-kd * u[0] + kl * c[0]) * G, (-kd * u[1] + kl * c[1] - 1.0) * G, (-kd * u[2] + kl * c[2]) * G];
    let decay = -ae.beta() * sp / TU; // dω/dt = -β |u| ω  (무차원 시간 → 초)
    (a, om.map(|x| decay * x))
}

/// 스크린 충돌 (법선 = -x). 반환 (새 속도, 새 스핀)
fn bounce(v: [f64; 3], om: [f64; 3], kt: f64, e: f64, mu: f64) -> ([f64; 3], [f64; 3]) {
    let rc = [R, 0.0, 0.0];
    let wr = cross(om, rc);
    let vc = [v[0] + wr[0], v[1] + wr[1], v[2] + wr[2]];
    let vct = [0.0, vc[1], vc[2]];
    let vn = v[0].max(0.0);
    let jn = (1.0 + e) * vn; // 법선 충격량 / m
    let slip = (vct[1] * vct[1] + vct[2] * vct[2]).sqrt();
    let dvt = if mu * jn >= kt * slip || slip < 1e-12 {
        vct.map(|x| -kt * x) // 굴림
    } else {
        vct.map(|x| -mu * jn * x / slip) // 미끄럼
    };
    let nv = [v[0] - jn, v[1] + dvt[1], v[2] + dvt[2]];
    let dw = cross(rc, dvt).map(|x| 2.5 / (R * R) * x);
    (nv, [om[0] + dw[0], om[1] + dw[1], om[2] + dw[2]])
}

/// p = [x0 y0 z0, 속도, 발사각, 방위각, rpm, 축, k_t, e] → 프레임 위치들
fn sample(ae: &SharedRe, p: &[f64; NP], scr: Screen, n: usize, fdt: f64) -> Vec<[f64; 3]> {
    let (th, ph) = (p[4], p[5]);
    let mut pos = [p[0], p[1], p[2]];
    let mut v = [p[3] * th.cos() * ph.cos(), p[3] * th.sin(), p[3] * th.cos() * ph.sin()];
    let nrm = [-ph.sin(), 0.0, ph.cos()];
    let omg = p[6] * std::f64::consts::TAU / 60.0;
    let mut om = [omg * p[7].cos() * nrm[0], omg * p[7].sin(), omg * p[7].cos() * nrm[2]];
    let sub = 8;
    let h = fdt / sub as f64;
    let mut hit = false;
    let wall = scr.dist - R;
    let mut out = Vec::with_capacity(n);
    let add = |a: [f64; 3], b: [f64; 3], c: f64| [a[0] + c * b[0], a[1] + c * b[1], a[2] + c * b[2]];
    for k in 0..n {
        if k > 0 {
            for _ in 0..sub {
                let (a1, w1) = accel(ae, v, om);
                let (a2, w2) = accel(ae, add(v, a1, 0.5 * h), add(om, w1, 0.5 * h));
                let (a3, w3) = accel(ae, add(v, a2, 0.5 * h), add(om, w2, 0.5 * h));
                let (a4, w4) = accel(ae, add(v, a3, h), add(om, w3, h));
                let v2 = add(v, a1, 0.5 * h);
                let v3 = add(v, a2, 0.5 * h);
                let v4 = add(v, a3, h);
                let np: [f64; 3] = std::array::from_fn(|i| pos[i] + h / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]));
                let nv: [f64; 3] = std::array::from_fn(|i| v[i] + h / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]));
                let no: [f64; 3] = std::array::from_fn(|i| om[i] + h / 6.0 * (w1[i] + 2.0 * w2[i] + 2.0 * w3[i] + w4[i]));
                if !hit && np[0] >= wall {
                    // 걸음 안에서 벽에 닿는 비율 f (선형), 그 순간 충돌 후 남은 시간만큼 직선으로
                    let f = ((wall - pos[0]) / (np[0] - pos[0])).clamp(0.0, 1.0);
                    let pc: [f64; 3] = std::array::from_fn(|i| pos[i] + f * (np[i] - pos[i]));
                    let vcn: [f64; 3] = std::array::from_fn(|i| v[i] + f * (nv[i] - v[i]));
                    let ocn: [f64; 3] = std::array::from_fn(|i| om[i] + f * (no[i] - om[i]));
                    let (bv, bo) = bounce(vcn, ocn, p[8], p[9], scr.mu);
                    let rest = (1.0 - f) * h;
                    pos = add(pc, bv, rest);
                    v = bv;
                    om = bo;
                    hit = true;
                } else {
                    pos = np;
                    v = nv;
                    om = no;
                }
            }
        }
        out.push(pos);
    }
    out
}

fn carry(ae: &SharedRe, p: &[f64; NP]) -> f64 {
    let d = dimless(p[3] / 0.44704, p[4].to_degrees(), p[6]);
    run(ae, &[[d[0], d[1], d[2], p[7]]], 0.004)[0].0 * YD
}

fn prior_rpm(mph: f64, deg: f64) -> f64 {
    (2233.36 + 303.69 * deg - 9.909 * mph).clamp(1500.0, 11000.0)
}

fn solve(mut a: Vec<Vec<f64>>) -> Vec<f64> {
    let n = a.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        for r in 0..n {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..=n {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    (0..n).map(|i| a[i][n] / a[i][i]).collect()
}

struct Prior {
    rpm: f64,
    kt: f64,
    kt_sd: f64,
}

fn resid(ae: &SharedRe, p: &[f64; NP], scr: Screen, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, pr: &Prior) -> Vec<f64> {
    let m = sample(ae, p, scr, obs.len(), fdt);
    let mut r = Vec::with_capacity(obs.len() * 3 + 3);
    for (a, b) in m.iter().zip(obs) {
        for c in 0..3 {
            r.push((a[c] - b[c]) / sig[c]);
        }
    }
    r.push((p[6] - pr.rpm) / 1500.0);
    r.push(p[7].to_degrees() / 20.0);
    r.push((p[8] - pr.kt) / pr.kt_sd.max(1e-6));
    r
}

fn fit(ae: &SharedRe, p0: [f64; NP], scr: Screen, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, pr: &Prior) -> ([f64; NP], f64) {
    let step = [1e-4, 1e-4, 1e-4, 1e-3, 1e-5, 1e-5, 1.0, 1e-4, 1e-4, 1e-4];
    let lo = [-1.0, -1.0, -1.0, 5.0, -0.2, -1.0, 0.0, -1.2, 0.0, 0.02];
    let hi = [1.0, 1.0, 1.0, 100.0, 1.2, 1.0, 15000.0, 1.2, 1.0, 0.95];
    let cost = |r: &[f64]| r.iter().map(|x| x * x).sum::<f64>();
    let mut p = p0;
    let mut r = resid(ae, &p, scr, obs, sig, fdt, pr);
    let mut mu = 1e-3;
    for _ in 0..80 {
        let mut jac = vec![[0.0f64; NP]; r.len()];
        for k in 0..NP {
            let mut q = p;
            q[k] += step[k];
            let rq = resid(ae, &q, scr, obs, sig, fdt, pr);
            for i in 0..r.len() {
                jac[i][k] = (rq[i] - r[i]) / step[k];
            }
        }
        let mut jtj = vec![vec![0.0; NP]; NP];
        let mut jtr = vec![0.0; NP];
        for i in 0..r.len() {
            for a in 0..NP {
                jtr[a] += jac[i][a] * r[i];
                for b in 0..NP {
                    jtj[a][b] += jac[i][a] * jac[i][b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..12 {
            let a: Vec<Vec<f64>> = (0..NP).map(|x| { let mut row = jtj[x].clone(); row[x] = row[x] * (1.0 + mu) + 1e-12; row.push(-jtr[x]); row }).collect();
            let dp = solve(a);
            let q: [f64; NP] = std::array::from_fn(|k| (p[k] + dp[k]).clamp(lo[k], hi[k]));
            let rq = resid(ae, &q, scr, obs, sig, fdt, pr);
            if cost(&rq) < cost(&r) {
                let rel = (cost(&r) - cost(&rq)) / cost(&r).max(1e-30);
                p = q;
                r = rq;
                mu = (mu * 0.3).max(1e-10);
                improved = rel > 1e-10;
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

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn g(&mut self) -> f64 {
        let (a, b) = (self.u().max(1e-300), self.u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// 충돌 전 프레임 앞부분 2차 맞춤으로 시작점
fn initial_guess(obs: &[[f64; 3]], fdt: f64, rpm0: f64, kt: f64) -> [f64; NP] {
    let n = obs.len().min(6);
    let mut co = [[0.0; 3]; 3];
    for c in 0..3 {
        let mut a = vec![vec![0.0; 4]; 3];
        for k in 0..n {
            let t = k as f64 * fdt;
            let b = [1.0, t, t * t];
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] += b[i] * b[j];
                }
                a[i][3] += b[i] * obs[k][c];
            }
        }
        let s = solve(a);
        co[c] = [s[0], s[1], s[2]];
    }
    let (vx, vy, vz) = (co[0][1], co[1][1], co[2][1]);
    let v = (vx * vx + vy * vy + vz * vz).sqrt();
    [co[0][0], co[1][0], co[2][0], v, (vy / v).asin(), vz.atan2(vx), rpm0, 0.0, kt, 0.3]
}

/// 스크린 상수 k_t 자가 보정: 여러 샷이 같은 k_t 를 나눠 쓴다. k_t 를 하나 정하고 샷마다 맞춘 비용 합을 최소로 하는 k_t 를 찾는다.
fn selfcal(ae: &SharedRe, scr: Screen) {
    let fps = 240.0;
    let fdt = 1.0 / fps;
    let sig = [2.2 * 0.005, 0.005, 2.2 * 0.005]; // 대각 카메라, σ 5 mm
    println!("k_t 자가 보정 (참 k_t 0.20, 대각 카메라 σ 5 mm, 반동 0.1 s). 샷은 클럽 4종을 돌려 쓰고, 참 스핀 = 투어 회귀 + N(0, 1000 rpm)");
    println!("{:>5} | {:>10} {:>10}", "샷 수", "k_t 오차", "시행");
    let clubs = [(167.0, 10.9), (120.0, 16.3), (100.0, 18.0), (86.0, 25.7)];
    for nshot in [5usize, 10, 20, 40] {
        let trials = 6;
        let mut err2 = 0.0;
        for tr in 0..trials {
            let mut rng = Rng(0xC0FFEE ^ (nshot as u64) << 16 ^ tr as u64);
            let mut shots = Vec::new();
            for i in 0..nshot {
                let (mph, deg) = clubs[i % 4];
                let rpm = prior_rpm(mph, deg) + 1000.0 * rng.g();
                let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), (3.0 * rng.g()).to_radians(), rpm.max(1500.0), (8.0 * rng.g()).to_radians(), 0.20, 0.30];
                let probe = sample(ae, &truth, scr, 200, fdt);
                let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
                let n = k_hit + 25;
                let obs: Vec<[f64; 3]> = sample(ae, &truth, scr, n, fdt).iter().map(|p| [p[0] + sig[0] * rng.g(), p[1] + sig[1] * rng.g(), p[2] + sig[2] * rng.g()]).collect();
                shots.push(obs);
            }
            let total = |kt: f64| -> f64 {
                shots.iter().map(|obs| {
                    let g0 = initial_guess(obs, fdt, 0.0, kt);
                    let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), kt, kt_sd: 1e-4 };
                    let mut best = f64::INFINITY;
                    for r0 in [pr.rpm, 2500.0, 8000.0] {
                        best = best.min(fit(ae, initial_guess(obs, fdt, r0, kt), scr, obs, sig, fdt, &pr).1);
                    }
                    best
                }).sum()
            };
            // 황금분할 탐색 0.05–0.45
            let (mut a, mut b) = (0.05, 0.45);
            let gr = 0.618_033_988_75;
            let (mut c, mut d) = (b - gr * (b - a), a + gr * (b - a));
            let (mut fc, mut fd) = (total(c), total(d));
            for _ in 0..18 {
                if fc < fd { b = d; d = c; fd = fc; c = b - gr * (b - a); fc = total(c); } else { a = c; c = d; fc = fd; d = a + gr * (b - a); fd = total(d); }
            }
            let kt = 0.5 * (a + b);
            err2 += (kt - 0.20).powi(2);
        }
        println!("{:>5} | {:>10.4} {:>10}", nshot, (err2 / trials as f64).sqrt(), trials);
    }
}

/// 다항식 최소제곱 (차수 deg), t 는 기준 시각에서 뺀 값
fn polyfit(ts: &[f64], ys: &[f64], deg: usize) -> Vec<f64> {
    let m = deg + 1;
    let mut a = vec![vec![0.0; m + 1]; m];
    for (t, y) in ts.iter().zip(ys) {
        let b: Vec<f64> = (0..m).map(|k| t.powi(k as i32)).collect();
        for i in 0..m {
            for j in 0..m {
                a[i][j] += b[i] * b[j];
            }
            a[i][m] += b[i] * y;
        }
    }
    solve(a)
}

/// 닫힌 꼴 추정: 충돌 전·후 프레임을 따로 2차 맞춤 → 충돌 순간 접선 속도 차 → ω = -(Δv_t/k_t + v_t)/R.
/// 반환 [볼스피드 m/s, 발사각, 방위각, rpm, 축]
fn closed_form(obs: &[[f64; 3]], fdt: f64, wall: f64, kt: f64) -> [f64; 5] {
    let k_last = (0..obs.len()).max_by(|&a, &b| obs[a][0].total_cmp(&obs[b][0])).unwrap();
    // 충돌 전: 0..=k_last-? (가장 앞 프레임은 이미 반동했을 수 있어 하나 덜어냄)
    let pre: Vec<usize> = (0..k_last.max(2)).collect();
    let post: Vec<usize> = (k_last + 1..obs.len()).collect();
    let t = |k: usize| k as f64 * fdt;
    let fitc = |idx: &[usize], c: usize, deg: usize| polyfit(&idx.iter().map(|&k| t(k)).collect::<Vec<_>>(), &idx.iter().map(|&k| obs[k][c]).collect::<Vec<_>>(), deg.min(idx.len().saturating_sub(1)));
    let pd = if pre.len() >= 4 { 2 } else { 1 };
    let px: Vec<f64> = fitc(&pre, 0, pd);
    let py = fitc(&pre, 1, pd);
    let pz = fitc(&pre, 2, pd);
    let ev = |c: &Vec<f64>, tt: f64| c.iter().enumerate().map(|(k, a)| a * tt.powi(k as i32)).sum::<f64>();
    let dv = |c: &Vec<f64>, tt: f64| c.iter().enumerate().skip(1).map(|(k, a)| k as f64 * a * tt.powi(k as i32 - 1)).sum::<f64>();
    // 충돌 시각: x(t) = wall 의 근 (뉴턴)
    let mut th = t(k_last);
    for _ in 0..20 {
        let f = ev(&px, th) - wall;
        let d = dv(&px, th).max(1.0);
        th -= f / d;
    }
    let qd = if post.len() >= 4 { 2 } else { 1 };
    let qy = fitc(&post, 1, qd);
    let qz = fitc(&post, 2, qd);
    let (vy0, vz0) = (dv(&py, th), dv(&pz, th));
    let (vy1, vz1) = (dv(&qy, th), dv(&qz, th));
    let (dvy, dvz) = (vy1 - vy0, vz1 - vz0);
    // Δv_y = -k_t (v_y + R ω_z),  Δv_z = -k_t (v_z - R ω_y)
    let wz = -(dvy / kt + vy0) / R;
    let wy = (dvz / kt + vz0) / R;
    let om = (wy * wy + wz * wz).sqrt();
    let (vx, vy, vz) = (dv(&px, 0.0), dv(&py, 0.0), dv(&pz, 0.0));
    let v = (vx * vx + vy * vy + vz * vz).sqrt();
    [v, (vy / v).asin(), vz.atan2(vx), om * 60.0 / std::f64::consts::TAU, wy.atan2(wz)]
}

fn closed_study(ae: &SharedRe) {
    let scr = Screen { dist: 3.0, mu: 0.5 };
    let mc = 40;
    let shots = [("드라이버", 167.0, 10.9, 2686.0), ("7번", 120.0, 16.3, 7097.0), ("아마7번", 100.0, 18.0, 6000.0), ("웨지", 86.0, 25.7, 8403.0)];
    println!("닫힌 꼴 스핀 (탐색 없음) 대 LM 맞춤. 대각 카메라 σ 5 mm, 반동 0.2 s, k_t 보정(0.20). 몬테카를로 {mc}회");
    println!("{:<8} {:>4} | {:>8} {:>6} {:>7} {:>8} | {:>8} {:>6} {:>7} {:>8}", "샷", "fps", "닫힌rpm", "축deg", "캐리yd", "µs/샷", "LM rpm", "축deg", "캐리yd", "ms/샷");
    for fps in [60.0f64, 120.0, 240.0] {
        let fdt = 1.0 / fps;
        let sig = [2.2 * 0.005, 0.005, 2.2 * 0.005];
        for (name, mph, deg, rpm) in shots {
            let wall = std::env::var("WALL").is_ok();
            let (ktv, ev) = if wall { (2.0 / 7.0, 0.6) } else { (0.20, 0.30) };
            let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, 8f64.to_radians(), ktv, ev];
            let c_true = carry(ae, &truth);
            let probe = sample(ae, &truth, scr, 400, fdt);
            let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
            let n = k_hit + (0.2 * fps).round() as usize + 1;
            let clean = sample(ae, &truth, scr, n, fdt);
            let mut rng = Rng(0xBEEF ^ (fps as u64) << 12);
            let (mut e1, mut a1, mut c1, mut e2, mut a2, mut c2) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            let (mut t1, mut t2) = (0.0, 0.0);
            for _ in 0..mc {
                let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sig[0] * rng.g(), p[1] + sig[1] * rng.g(), p[2] + sig[2] * rng.g()]).collect();
                let t0 = std::time::Instant::now();
                let cf = closed_form(&obs, fdt, scr.dist - R, ktv);
                t1 += t0.elapsed().as_secs_f64();
                let pc = [0.0, 0.02, 0.0, cf[0], cf[1], cf[2], cf[3], cf[4], ktv, ev];
                e1 += (cf[3] - rpm).powi(2);
                a1 += (cf[4] - truth[7]).to_degrees().powi(2);
                c1 += (carry(ae, &pc) - c_true).powi(2);
                let t0 = std::time::Instant::now();
                let g0 = initial_guess(&obs, fdt, 0.0, ktv);
                let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), kt: ktv, kt_sd: 0.01 };
                // LM 시작점을 닫힌 꼴 값으로 (탐색 시작점 하나)
                let mut st = initial_guess(&obs, fdt, cf[3].clamp(500.0, 14000.0), ktv);
                st[9] = ev;
                st[7] = cf[4].clamp(-1.0, 1.0);
                let (p, _) = fit(ae, st, scr, &obs, sig, fdt, &pr);
                t2 += t0.elapsed().as_secs_f64();
                e2 += (p[6] - rpm).powi(2);
                a2 += (p[7] - truth[7]).to_degrees().powi(2);
                c2 += (carry(ae, &p) - c_true).powi(2);
            }
            let k = mc as f64;
            println!("{:<8} {:>4.0} | {:>8.0} {:>6.2} {:>7.2} {:>8.1} | {:>8.0} {:>6.2} {:>7.2} {:>8.1}", name, fps,
                (e1 / k).sqrt(), (a1 / k).sqrt(), (c1 / k).sqrt(), t1 / k * 1e6, (e2 / k).sqrt(), (a2 / k).sqrt(), (c2 / k).sqrt(), t2 / k * 1e3);
        }
    }
}

fn main() {
    let ae = field();
    if std::env::args().any(|a| a == "--closed") {
        closed_study(&ae);
        return;
    }
    if std::env::args().any(|a| a == "--selfcal") {
        selfcal(&ae, Screen { dist: 3.0, mu: 0.5 });
        return;
    }
    let mc: usize = std::env::args().skip(1).find_map(|s| s.parse().ok()).unwrap_or(40);
    let scr = Screen { dist: 3.0, mu: 0.5 };
    let fps: f64 = std::env::var("FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(240.0);
    let fdt = 1.0 / fps;
    // 참 스크린: k_t (천 스크린은 2/7 보다 작을 수 있다), e = 0.3
    let shots = [("드라이버", 167.0, 10.9, 2686.0), ("7번", 120.0, 16.3, 7097.0), ("아마7번", 100.0, 18.0, 6000.0), ("웨지", 86.0, 25.7, 8403.0)];
    println!("스크린 반동으로 스핀 찾기 (스크린 3 m, 공 뒤 카메라 {fps} fps, 깊이 노이즈 ×5, 몬테카를로 {mc}회)");
    println!("참 스크린 k_t = 0.20, e = 0.30. 맞춤의 k_t 사전값: 보정함 = 0.20 ± 0.01, 모름 = 2/7 ± 0.1\n");
    println!("{:<8} {:<4} {:>6} {:>4} {:>6} | {:>8} {:>6} {:>7} {:>7} {:>6}", "샷", "카메라", "반동 s", "σmm", "k_t", "스핀rpm", "축deg", "속도mph", "캐리yd", "k_t");
    let cams: [(&str, [f64; 3]); 3] = [("뒤", [5.0, 1.0, 1.0]), ("옆", [1.0, 1.0, 5.0]), ("대각", [2.2, 1.0, 2.2])];
    let afters: Vec<f64> = std::env::var("AFTER").ok().map(|v| v.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or(vec![0.1]);
    for (name, mph, deg, rpm) in shots {
        let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, 8f64.to_radians(), 0.20, 0.30];
        let c_true = carry(&ae, &truth);
        let probe = sample(&ae, &truth, scr, 200, fdt);
        let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
        for &after in &afters {
            let n = k_hit + (after * fps).round() as usize + 1;
            let clean = sample(&ae, &truth, scr, n, fdt);
            for (cam, cw) in cams {
                for sg in [0.002f64, 0.005] {
                    let sig = cw.map(|c| c * sg);
                    for (tag, pr_kt, pr_sd) in [("보정", 0.20, 0.01), ("모름", 2.0 / 7.0, 0.1)] {
                        let mut rng = Rng(0xA5A5_5A5A_1234_5678 ^ ((after * 1e3) as u64) << 8 ^ ((sg * 1e5) as u64) << 24 ^ (cw[0] * 10.0) as u64);
                        let (mut es, mut ea, mut ec, mut ek, mut ev) = (0.0, 0.0, 0.0, 0.0, 0.0);
                        for _ in 0..mc {
                            let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sig[0] * rng.g(), p[1] + sig[1] * rng.g(), p[2] + sig[2] * rng.g()]).collect();
                            let g0 = initial_guess(&obs, fdt, 0.0, pr_kt);
                            let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), kt: pr_kt, kt_sd: pr_sd };
                            let mut best = ([0.0; NP], f64::INFINITY);
                            for r0 in [pr.rpm, 2500.0, 8000.0] {
                                let f = fit(&ae, initial_guess(&obs, fdt, r0, pr_kt), scr, &obs, sig, fdt, &pr);
                                if f.1 < best.1 {
                                    best = f;
                                }
                            }
                            let p = best.0;
                            es += (p[6] - rpm).powi(2);
                            ea += (p[7] - truth[7]).to_degrees().powi(2);
                            ec += (carry(&ae, &p) - c_true).powi(2);
                            ek += (p[8] - 0.20).powi(2);
                            ev += ((p[3] - truth[3]) / 0.44704).powi(2);
                        }
                        let k = mc as f64;
                        println!("{:<8} {:<4} {:>6.2} {:>4.0} {:>6} | {:>8.0} {:>6.2} {:>7.2} {:>7.2} {:>6.3}", name, cam, after, sg * 1e3, tag, (es / k).sqrt(), (ea / k).sqrt(), (ev / k).sqrt(), (ec / k).sqrt(), (ek / k).sqrt());
                    }
                }
            }
        }
    }
}
