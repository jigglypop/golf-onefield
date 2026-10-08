//! 스핀 역보정: 카메라로 본 비행 초반 위치들에서 스핀량과 스핀축을 거꾸로 찾는다.
//!
//! 관측: 초당 fps 프레임, 처음 T 초 동안 공 중심의 3차원 위치(노이즈 σ). 카메라 한 대는 깊이 방향이 나쁘다고 보고
//!   깊이 축만 노이즈를 5배로 둔다 ('옆' 카메라: 깊이 = 좌우 z, '뒤' 카메라: 깊이 = 앞뒤 x).
//! 미지수 8개: 첫 프레임 위치 3, 볼스피드, 발사각, 방위각, 스핀 rpm, 스핀축 기울기.
//! 맞춤: 레벤버그-마쿼트 (장 = 공유 포화 장 + 항력 위기, 투어 23행 맞춤값). 맞춘 값으로 끝까지 날려 캐리 오차를 본다.
//!
//! cargo run --release --bin spininv

use flightbench::engine::{dimless, run, Aero, SharedRe, U_MS, YD};
use flightbench::harness::RE_PER_U;
use std::time::Instant;

const G: f64 = 9.81;
const L: f64 = U_MS * U_MS / G; // 53.53 m
const TU: f64 = U_MS / G; // 무차원 시간 1 = 2.336 s

fn field() -> SharedRe {
    SharedRe { v: 0.1330, e: 0.3247, u0: 62859.6581 / RE_PER_U, d: 1.3688, c: 2.7901, l: 5.9226 }
}

/// p = [x0 y0 z0 (m), 속도 m/s, 발사각 rad, 방위각 rad, rpm, 축 rad] → 프레임 시각들의 위치 (m)
fn sample(ae: &SharedRe, p: &[f64; 8], n_frames: usize, fdt: f64) -> Vec<[f64; 3]> {
    let (v, th, ph) = (p[3] / U_MS, p[4], p[5]);
    let dir = [th.cos() * ph.cos(), th.sin(), th.cos() * ph.sin()];
    let nrm = [-ph.sin(), 0.0, ph.cos()];
    let om = [p[7].cos() * nrm[0], p[7].sin(), p[7].cos() * nrm[2]];
    let w0 = 0.021335 * p[6] * std::f64::consts::TAU / 60.0 / U_MS;
    let mut s = [p[0] / L, p[1] / L, p[2] / L, v * dir[0], v * dir[1], v * dir[2], w0];
    let f = |s: &[f64; 7]| -> [f64; 7] {
        let sp = (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt();
        let (kd, kl) = ae.kk(sp, s[6]);
        let c = [om[1] * s[5] - om[2] * s[4], om[2] * s[3] - om[0] * s[5], om[0] * s[4] - om[1] * s[3]];
        [s[3], s[4], s[5], -kd * s[3] + kl * c[0], -kd * s[4] + kl * c[1] - 1.0, -kd * s[5] + kl * c[2], -ae.beta() * s[6] * sp]
    };
    let sub = 4;
    let h = fdt / TU / sub as f64;
    let mut out = Vec::with_capacity(n_frames);
    for k in 0..n_frames {
        if k > 0 {
            for _ in 0..sub {
                let k1 = f(&s);
                let a: [f64; 7] = std::array::from_fn(|i| s[i] + 0.5 * h * k1[i]);
                let k2 = f(&a);
                let b: [f64; 7] = std::array::from_fn(|i| s[i] + 0.5 * h * k2[i]);
                let k3 = f(&b);
                let c: [f64; 7] = std::array::from_fn(|i| s[i] + h * k3[i]);
                let k4 = f(&c);
                s = std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
            }
        }
        out.push([s[0] * L, s[1] * L, s[2] * L]);
    }
    out
}

fn carry(ae: &SharedRe, p: &[f64; 8]) -> f64 {
    let mph = p[3] / 0.44704;
    let d = dimless(mph, p[4].to_degrees(), p[6]);
    run(ae, &[[d[0], d[1], d[2], p[7]]], 0.004)[0].0 * YD
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

/// 사전분포: 투어 23행 회귀 rpm ≈ 2233 + 303.7 발사각(deg) - 9.91 볼스피드(mph) (잔차 940 rpm), 폭 1500 rpm.
/// 축은 0 ± 20°. 관측이 짧아 스핀이 안 보이면 맞춤이 이 값으로 돌아온다(폭주하지 않는다).
pub const PRIOR_SD: f64 = 1500.0;
pub const AXIS_SD: f64 = 20.0;
fn prior_rpm(mph: f64, deg: f64) -> f64 {
    (2233.36 + 303.69 * deg - 9.909 * mph).clamp(1500.0, 11000.0)
}

/// 가중 잔차 (각 축 노이즈로 나눔) + 사전분포 두 줄
fn resid(ae: &SharedRe, p: &[f64; 8], obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, prior: f64) -> Vec<f64> {
    let m = sample(ae, p, obs.len(), fdt);
    let mut r = Vec::with_capacity(obs.len() * 3 + 2);
    for (a, b) in m.iter().zip(obs) {
        let a = project(*a);
        for c in 0..3 {
            // 빠진 프레임은 NaN: 잔차 0
            r.push(if b[c].is_finite() { (a[c] - b[c]) / sig[c] } else { 0.0 });
        }
    }
    r.push((p[6] - prior) / PRIOR_SD);
    r.push(p[7].to_degrees() / AXIS_SD);
    r
}

/// 레벤버그-마쿼트. 반환 (매개변수, 카이제곱, 공분산 대각)
fn fit(ae: &SharedRe, p0: [f64; 8], obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, prior: f64) -> ([f64; 8], f64, [f64; 8]) {
    let step = [1e-4, 1e-4, 1e-4, 1e-3, 1e-5, 1e-5, 1.0, 1e-4];
    let lo = [-1.0, -1.0, -1.0, 5.0, -0.2, -1.0, 0.0, -1.2];
    let hi = [1.0, 1.0, 1.0, 100.0, 1.2, 1.0, 15000.0, 1.2];
    let cost = |r: &[f64]| r.iter().map(|x| x * x).sum::<f64>();
    let mut p = p0;
    let mut r = resid(ae, &p, obs, sig, fdt, prior);
    let mut mu = 1e-3;
    let mut jtj = vec![vec![0.0; 8]; 8];
    for _ in 0..60 {
        let mut jac = vec![[0.0f64; 8]; r.len()];
        for k in 0..8 {
            let mut q = p;
            q[k] += step[k];
            let rq = resid(ae, &q, obs, sig, fdt, prior);
            for i in 0..r.len() {
                jac[i][k] = (rq[i] - r[i]) / step[k];
            }
        }
        jtj = vec![vec![0.0; 8]; 8];
        let mut jtr = vec![0.0; 8];
        for i in 0..r.len() {
            for a in 0..8 {
                jtr[a] += jac[i][a] * r[i];
                for b in 0..8 {
                    jtj[a][b] += jac[i][a] * jac[i][b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut a: Vec<Vec<f64>> = (0..8).map(|x| { let mut row = jtj[x].clone(); row[x] *= 1.0 + mu; row[x] += 1e-12; row.push(-jtr[x]); row }).collect();
            let dp = solve(std::mem::take(&mut a));
            let q: [f64; 8] = std::array::from_fn(|k| (p[k] + dp[k]).clamp(lo[k], hi[k]));
            let rq = resid(ae, &q, obs, sig, fdt, prior);
            if cost(&rq) < cost(&r) {
                let rel = (cost(&r) - cost(&rq)) / cost(&r).max(1e-30);
                p = q;
                r = rq;
                mu = (mu * 0.3).max(1e-10);
                improved = true;
                if rel < 1e-10 {
                    break;
                }
                break;
            }
            mu *= 10.0;
        }
        if !improved {
            break;
        }
    }
    // 공분산 = (J^T J)^-1 의 대각 (노이즈로 정규화했으므로 그대로)
    let mut cov = [0.0; 8];
    for k in 0..8 {
        let mut a: Vec<Vec<f64>> = jtj.iter().map(|row| { let mut r = row.clone(); r.push(0.0); r }).collect();
        a[k][8] = 1.0;
        for x in 0..8 {
            a[x][x] += 1e-12;
        }
        cov[k] = solve(a)[k];
    }
    (p, cost(&r), cov)
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

/// 노이즈 없는 앞 몇 프레임의 2차 맞춤으로 처음 속도를 어림 (맞춤 시작점)
fn initial_guess(obs: &[[f64; 3]], fdt: f64, rpm0: f64) -> [f64; 8] {
    let ks: Vec<usize> = (0..obs.len()).filter(|&k| obs[k][0].is_finite()).take(8).collect();
    // 각 축 x(t) = a + b t + c t^2 최소제곱
    let mut coef = [[0.0; 3]; 3];
    for c in 0..3 {
        let mut a = vec![vec![0.0; 4]; 3];
        for &k in &ks {
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
        coef[c] = [s[0], s[1], s[2]];
    }
    let (vx, vy, vz) = (coef[0][1], coef[1][1], coef[2][1]);
    let v = (vx * vx + vy * vy + vz * vz).sqrt();
    [coef[0][0], coef[1][0], coef[2][0], v, (vy / v).asin(), vz.atan2(vx), rpm0, 0.0]
}

/// CSV (t,x,y,z 미터·초; x 앞, y 위, z 오른쪽) → 프레임 배열 (빠진 프레임 NaN), 프레임 간격
/// 옆 카메라 투영 [xc, yc, Z0, sgn]: 평면 밖으로 z 만큼 나온 점이 화면 평면에 비치는 위치
static CAM: std::sync::OnceLock<[f64; 4]> = std::sync::OnceLock::new();
#[inline]
fn project(p: [f64; 3]) -> [f64; 3] {
    match CAM.get() {
        Some(c) => {
            let s = c[2] / (c[2] + c[3] * p[2]);
            [c[0] + (p[0] - c[0]) * s, c[1] + (p[1] - c[1]) * s, p[2]]
        }
        None => p,
    }
}

fn read_csv(path: &str) -> (Vec<[f64; 3]>, f64) {
    let rows: Vec<[f64; 4]> = std::fs::read_to_string(path).expect("CSV 읽기").lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| { let v: Vec<f64> = l.split(|c| c == ',' || c == ' ' || c == '\t').filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect(); (v.len() >= 4).then(|| [v[0], v[1], v[2], v[3]]) })
        .collect();
    assert!(rows.len() >= 5, "프레임이 5개 이상 필요");
    // "# cam xc yc Z0 sgn": 카메라 투영 (track_ball.py 가 씀)
    if let Some(l) = std::fs::read_to_string(path).unwrap().lines().find(|l| l.starts_with("# cam")) {
        let v: Vec<f64> = l[5..].split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() == 4 {
            let _ = CAM.set([v[0], v[1], v[2], v[3]]);
            eprintln!("카메라 투영 사용: 축 위치 ({:.3}, {:.3}) m, 거리 {:.3} m", v[0], v[1], v[2]);
        }
    }
    let mut dts: Vec<f64> = rows.windows(2).map(|w| w[1][0] - w[0][0]).filter(|d| *d > 0.0).collect();
    dts.sort_by(f64::total_cmp);
    let fdt = dts[dts.len() / 2];
    let n = ((rows.last().unwrap()[0] - rows[0][0]) / fdt).round() as usize + 1;
    let mut obs = vec![[f64::NAN; 3]; n];
    for r in &rows {
        let k = ((r[0] - rows[0][0]) / fdt).round() as usize;
        if k < n { obs[k] = [r[1], r[2], r[3]]; }
    }
    (obs, fdt)
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    let ae = field();
    if let Some(path) = arg("--csv") {
        // 실제 프레임 역보정 (비행만): --sig 위치 노이즈 mm (기본 5), --cam side|behind (깊이 축 ×5)
        let (obs, fdt) = read_csv(&path);
        let sg = arg("--sig").and_then(|v| v.parse::<f64>().ok()).unwrap_or(5.0) / 1000.0;
        let sig = if arg("--cam").as_deref() == Some("behind") { [5.0 * sg, sg, sg] } else { [sg, sg, 5.0 * sg] };
        let g0 = initial_guess(&obs, fdt, 0.0);
        let prior = prior_rpm(g0[3] / 0.44704, g0[4].to_degrees());
        let mut best = ([0.0; 8], f64::INFINITY, [0.0; 8]);
        for r0 in [prior, 2000.0, 5000.0, 9000.0] {
            let f = fit(&ae, initial_guess(&obs, fdt, r0), &obs, sig, fdt, prior);
            if f.1 < best.1 { best = f; }
        }
        let (p, chi2, cov) = best;
        let nobs = obs.iter().filter(|o| o[0].is_finite()).count();
        let c0 = carry(&ae, &p);
        let mut var = 0.0;
        for k in [3usize, 4, 6] {
            let mut q = p; let h = [0.0, 0.0, 0.0, 0.05, 1e-3, 0.0, 20.0, 0.0][k]; q[k] += h;
            let g = (carry(&ae, &q) - c0) / h; var += g * g * cov[k];
        }
        println!("프레임 {nobs}개 (간격 {:.2} ms, {:.0} fps), 카이제곱/자유도 {:.2}", fdt * 1e3, 1.0 / fdt, chi2 / (3 * nobs).saturating_sub(8).max(1) as f64);
        println!("볼스피드 {:.1} mph ±{:.1}, 발사각 {:.2}° ±{:.2}, 방위각 {:.2}°", p[3] / 0.44704, cov[3].sqrt() / 0.44704, p[4].to_degrees(), cov[4].sqrt().to_degrees(), p[5].to_degrees());
        println!("스핀 {:.0} rpm ±{:.0} (사전값 {:.0}), 스핀축 {:.1}° ±{:.1}", p[6], cov[6].sqrt(), prior, p[7].to_degrees(), cov[7].sqrt().to_degrees());
        println!("캐리 {:.1} yd ±{:.1} (대각 공분산만, 근사)", c0, var.sqrt());
        if cov[6].sqrt() > 0.8 * PRIOR_SD { println!("주의: 스핀이 관측으로 거의 정해지지 않음 (사전값에 기댐). 더 길게 찍거나 벽 반동(screen --csv)을 쓰세요."); }
        return;
    }
    if std::env::args().any(|a| a == "--dump") {
        // 웹 페이지 대조용: 드라이버, 240 fps, 0.2 s 의 깨끗한 프레임과 캐리
        let truth = [0.0, 0.02, 0.0, 167.0 * 0.44704, 10.9f64.to_radians(), 2f64.to_radians(), 2686.0, 8f64.to_radians()];
        let fr = sample(&ae, &truth, 49, 1.0 / 240.0);
        for k in [0usize, 12, 24, 48] {
            println!("frame {k}: {:.9} {:.9} {:.9}", fr[k][0], fr[k][1], fr[k][2]);
        }
        println!("carry {:.6}", carry(&ae, &truth));
        println!("prior {:.3}", prior_rpm(167.0, 10.9));
        return;
    }
    let mc: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    // (이름, mph, 발사각, rpm) — 방위 2°, 축 8° (약한 페이드), 시작 높이 2 cm
    let shots = [("드라이버", 167.0, 10.9, 2686.0), ("7번", 120.0, 16.3, 7097.0), ("아마7번", 100.0, 18.0, 6000.0), ("웨지", 86.0, 25.7, 8403.0)];
    let windows: [f64; 5] = [0.03, 0.05, 0.1, 0.2, 0.3];
    let sigmas: [f64; 3] = [0.002, 0.005, 0.01];
    let fps_list: [f64; 1] = [240.0];
    println!("스핀 역보정 가관측성 (몬테카를로 {mc}회, 장: 공유 포화 장 + 항력 위기)");
    println!("관측: 공 중심 3차원 위치, 깊이 축 노이즈 ×5. 오차는 RMS. 사전분포: 스핀 = 투어 회귀 ± 1500 rpm, 축 = 0 ± 20°\n");
    let t_all = Instant::now();
    for &fps in &fps_list {
        let fdt = 1.0 / fps;
        for cam in ["옆", "뒤"] {
            println!("== 카메라 {cam}, {fps} fps");
            println!("{:<8} {:>6} {:>7} {:>6} | {:>9} {:>8} {:>9} | {:>11} {:>9}", "샷", "관측 s", "비행 m", "σ mm", "스핀 rpm", "축 deg", "캐리 yd", "공분산 rpm", "±1500 yd");
            for (name, mph, deg, rpm) in shots {
                let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, 8f64.to_radians()];
                let c_true = carry(&ae, &truth);
                // 스핀 모름 기준: 사전분포 평균(회귀)으로 찍은 캐리의 오차, 그리고 ±1500 rpm 의 RMS
                let pr = prior_rpm(mph, deg);
                let c_pr = carry(&ae, &{ let mut q = truth; q[6] = pr; q[7] = 0.0; q });
                let c_lo = carry(&ae, &{ let mut q = truth; q[6] -= 1500.0; q });
                let c_hi = carry(&ae, &{ let mut q = truth; q[6] += 1500.0; q });
                let unknown = (((c_lo - c_true).powi(2) + (c_hi - c_true).powi(2)) / 2.0).sqrt();
                println!("  {name}: 참 스핀 {rpm:.0}, 회귀 사전값 {pr:.0} rpm → 스핀 모름 캐리 오차 {:+.1} yd (±1500 rpm 이면 {unknown:.1} yd)", c_pr - c_true);
                for &win in &windows {
                    let nf = (win * fps).round() as usize + 1;
                    let clean = sample(&ae, &truth, nf, fdt);
                    let dist = ((clean[nf - 1][0] - clean[0][0]).powi(2) + (clean[nf - 1][1] - clean[0][1]).powi(2)).sqrt();
                    for &sg in &sigmas {
                        let sig = if cam == "옆" { [sg, sg, 5.0 * sg] } else { [5.0 * sg, sg, sg] };
                        let mut rng = Rng(0x9E3779B97F4A7C15 ^ ((win * 1e4) as u64) ^ ((sg * 1e6) as u64) << 20);
                        let (mut es, mut ea, mut ec, mut sdc) = (0.0, 0.0, 0.0, 0.0);
                        for _ in 0..mc {
                            let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sig[0] * rng.g(), p[1] + sig[1] * rng.g(), p[2] + sig[2] * rng.g()]).collect();
                            // 시작점 세 개 (스핀 2000 / 5000 / 9000), 카이제곱 최소
                            let mut best = ([0.0; 8], f64::INFINITY, [0.0; 8]);
                            let g0 = initial_guess(&obs, fdt, 0.0);
                            let prior = prior_rpm(g0[3] / 0.44704, g0[4].to_degrees());
                            for r0 in [prior, 2000.0, 9000.0] {
                                let f = fit(&ae, initial_guess(&obs, fdt, r0), &obs, sig, fdt, prior);
                                if f.1 < best.1 {
                                    best = f;
                                }
                            }
                            let p = best.0;
                            es += (p[6] - rpm).powi(2);
                            ea += (p[7] - truth[7]).to_degrees().powi(2);
                            ec += (carry(&ae, &p) - c_true).powi(2);
                            sdc += best.2[6].max(0.0);
                        }
                        let k = mc as f64;
                        println!("{:<8} {:>6.2} {:>7.1} {:>6.0} | {:>9.0} {:>8.2} {:>9.2} | {:>11.0} {:>9.1}",
                            name, win, dist, sg * 1e3, (es / k).sqrt(), (ea / k).sqrt(), (ec / k).sqrt(), (sdc / k).sqrt(), unknown);
                    }
                }
            }
            println!();
        }
    }
    println!("총 {:.1} s", t_all.elapsed().as_secs_f64());
}
