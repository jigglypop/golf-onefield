//! 빠른 시험대: 항을 글로 적은 장 모형을 PGA 투어표 세 양으로 맞추고 LPGA·USGA 로 판정한다.
//!
//! 속도를 낸 곳
//!  - 궤적: τ 걸음 RK4 (dτ = |u|^½ dt, 평균 약 56걸음, 수치 오차 1e-5 yd 수준), 착지·최고점은 에르미트 보간.
//!  - USGA 분포 평균: 몬테카를로 20만 표본 대신 가우스-에르미트 구적 7^3 = 343 점 (결정적, 표본 잡음 없음).
//!  - 보정: 레벤버그-마쿼트, 수치 야코비안.

use crate::engine::{dimless, USGA, YD};
use crate::expr::{parse, E};

/// (볼스피드 mph, 발사각 deg, 스핀 rpm, 캐리 yd, 최고 높이 yd, 착지각 deg) — TrackMan PGA 투어 평균
pub const PGA12: [[f64; 6]; 12] = [
    [167.0, 10.9, 2686.0, 275.0, 32.0, 38.0], [158.0, 9.2, 3655.0, 243.0, 30.0, 43.0], [152.0, 9.4, 4350.0, 230.0, 31.0, 47.0],
    [146.0, 10.2, 4437.0, 225.0, 29.0, 47.0], [142.0, 10.4, 4630.0, 212.0, 27.0, 46.0], [137.0, 11.0, 4836.0, 203.0, 28.0, 48.0],
    [132.0, 12.1, 5361.0, 194.0, 31.0, 49.0], [127.0, 14.1, 6231.0, 183.0, 30.0, 50.0], [120.0, 16.3, 7097.0, 172.0, 32.0, 50.0],
    [115.0, 18.1, 7998.0, 160.0, 31.0, 50.0], [109.0, 20.4, 8647.0, 148.0, 30.0, 51.0], [102.0, 24.2, 9304.0, 136.0, 29.0, 52.0],
];
pub const PGA_LAB: [&str; 12] = ["DR", "3W", "5W", "HY", "3i", "4i", "5i", "6i", "7i", "8i", "9i", "PW"];
/// TrackMan LPGA 투어 평균
pub const LPGA11: [[f64; 6]; 11] = [
    [140.0, 13.2, 2611.0, 218.0, 25.0, 37.0], [132.0, 11.2, 2704.0, 195.0, 23.0, 39.0], [128.0, 12.1, 4501.0, 185.0, 26.0, 43.0],
    [123.0, 12.7, 4693.0, 174.0, 25.0, 46.0], [116.0, 14.3, 4801.0, 169.0, 24.0, 43.0], [112.0, 14.8, 5081.0, 161.0, 23.0, 45.0],
    [109.0, 17.1, 5943.0, 152.0, 25.0, 46.0], [104.0, 19.0, 6699.0, 141.0, 26.0, 47.0], [100.0, 20.8, 7494.0, 130.0, 25.0, 47.0],
    [93.0, 23.9, 7589.0, 119.0, 26.0, 47.0], [86.0, 25.7, 8403.0, 107.0, 23.0, 48.0],
];
pub const LPGA_LAB: [&str; 11] = ["DR", "3W", "5W", "7W", "4i", "5i", "6i", "7i", "8i", "9i", "PW"];

/// 레이놀즈 수 = u × RE_PER_U (공기 rho 1.2, 점성 1.81e-5, 지름 42.67 mm)
pub const RE_PER_U: f64 = 22.916037151920374 * 0.04267 * 1.2 / 1.81e-5;

/// 비행 모형 하나: 입력 (mph, deg, rpm) → (캐리 yd, 최고 높이 yd, 착지각 deg)
pub trait Flight {
    fn obs(&self, mph: f64, deg: f64, rpm: f64) -> [f64; 3];
}

/// 글로 적은 장. 변수: u = |u| (무차원 속도), w (무차원 스핀), S = w/u, Re, 그리고 매개변수 이름.
/// kd = |u| C_D, kl = |u| C_L. 매개변수 이름이 beta 면 스핀 감쇠로 쓴다 (없으면 0.06).
#[derive(Clone)]
pub struct Model {
    pub name: String,
    pub kd: E,
    pub kl: E,
    pub kd_src: String,
    pub kl_src: String,
    pub pnames: Vec<String>,
    pub p: Vec<f64>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub beta_idx: Option<usize>,
    pub h: f64,
}

pub const BASE_VARS: [&str; 4] = ["u", "w", "S", "Re"];

impl Model {
    /// params: "이름=시작[:하한:상한]" 들. 하한·상한이 없으면 고정.
    pub fn new(name: &str, kd: &str, kl: &str, params: &[(&str, f64, f64, f64)]) -> Result<Model, String> {
        let mut names: Vec<String> = BASE_VARS.iter().map(|s| s.to_string()).collect();
        names.extend(params.iter().map(|p| p.0.to_string()));
        Ok(Model {
            name: name.into(),
            kd: parse(kd, &names).map_err(|e| format!("{name} kd: {e}"))?,
            kl: parse(kl, &names).map_err(|e| format!("{name} kl: {e}"))?,
            kd_src: kd.into(),
            kl_src: kl.into(),
            pnames: params.iter().map(|p| p.0.to_string()).collect(),
            p: params.iter().map(|p| p.1).collect(),
            lo: params.iter().map(|p| p.2).collect(),
            hi: params.iter().map(|p| p.3).collect(),
            beta_idx: params.iter().position(|p| p.0 == "beta"),
            h: 0.05,
        })
    }

    pub fn free(&self) -> Vec<usize> {
        (0..self.p.len()).filter(|&i| self.hi[i] > self.lo[i]).collect()
    }

    #[inline]
    fn f(&self, vars: &mut [f64], s: &[f64; 5]) -> [f64; 5] {
        let sp = (s[2] * s[2] + s[3] * s[3]).sqrt();
        vars[0] = sp;
        vars[1] = s[4];
        vars[2] = s[4] / sp;
        vars[3] = sp * RE_PER_U;
        let (kd, kl) = (self.kd.eval(vars), self.kl.eval(vars));
        let beta = self.beta_idx.map(|i| vars[4 + i]).unwrap_or(0.06);
        let g = 1.0 / sp.sqrt();
        [s[2] * g, s[3] * g, (-kd * s[2] - kl * s[3]) * g, (-kd * s[3] + kl * s[2] - 1.0) * g, -beta * s[4] * sp * g]
    }
}

#[inline]
fn herm(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2]
}

impl Flight for Model {
    fn obs(&self, mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
        let [v0, th, s0] = dimless(mph, deg, rpm);
        let mut vars = vec![0.0; 4 + self.p.len()];
        vars[4..].copy_from_slice(&self.p);
        let h = self.h;
        let mut s = [0.0, 0.0, v0 * th.cos(), v0 * th.sin(), s0 * v0];
        let mut k1 = self.f(&mut vars, &s);
        let mut apex = 0.0f64;
        for _ in 0..(12.0 / h) as usize {
            let a: [f64; 5] = std::array::from_fn(|i| s[i] + 0.5 * h * k1[i]);
            let k2 = self.f(&mut vars, &a);
            let b: [f64; 5] = std::array::from_fn(|i| s[i] + 0.5 * h * k2[i]);
            let k3 = self.f(&mut vars, &b);
            let c: [f64; 5] = std::array::from_fn(|i| s[i] + h * k3[i]);
            let k4 = self.f(&mut vars, &c);
            let n: [f64; 5] = std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
            let kn = self.f(&mut vars, &n);
            // 최고점: y' 가 + 에서 - 로. 에르미트 3차의 도함수(2차식)의 근
            if k1[1] > 0.0 && kn[1] <= 0.0 {
                let (y0, y1, m0, m1) = (s[1], n[1], k1[1] * h, kn[1] * h);
                // y(t) = H·(y0,m0,y1,m1), y'(t) = a t^2 + b t + c
                let a = 6.0 * y0 + 3.0 * m0 - 6.0 * y1 + 3.0 * m1;
                let b = -6.0 * y0 - 4.0 * m0 + 6.0 * y1 - 2.0 * m1;
                let c = m0;
                let t = if a.abs() < 1e-14 { -c / b } else {
                    let d = (b * b - 4.0 * a * c).max(0.0).sqrt();
                    let (r1, r2) = ((-b - d) / (2.0 * a), (-b + d) / (2.0 * a));
                    if (0.0..=1.0).contains(&r1) { r1 } else { r2 }
                }
                .clamp(0.0, 1.0);
                let hh = herm(t);
                apex = apex.max(hh[0] * y0 + hh[1] * m0 + hh[2] * y1 + hh[3] * m1);
            }
            if n[1] < 0.0 && s[1] >= 0.0 {
                let (y0, y1, m0, m1) = (s[1], n[1], k1[1] * h, kn[1] * h);
                // 첫 걸음 안에서 떨어지면(y0 = 0) t = 0 이 아닌 근을 위에서부터 찾는다
                let mut t = if y0 <= 0.0 { 1.0 } else { y0 / (y0 - y1) };
                for _ in 0..4 {
                    let hh = herm(t);
                    let f = hh[0] * y0 + hh[1] * m0 + hh[2] * y1 + hh[3] * m1;
                    let (t2, _) = (t * t, 0);
                    let df = (6.0 * t2 - 6.0 * t) * (y0 - y1) + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (3.0 * t2 - 2.0 * t) * m1;
                    t = (t - f / df).clamp(0.0, 1.0);
                }
                let hh = herm(t);
                let x = hh[0] * s[0] + hh[1] * k1[0] * h + hh[2] * n[0] + hh[3] * kn[0] * h;
                let vx = hh[0] * s[2] + hh[1] * k1[2] * h + hh[2] * n[2] + hh[3] * kn[2] * h;
                let vy = hh[0] * s[3] + hh[1] * k1[3] * h + hh[2] * n[3] + hh[3] * kn[3] * h;
                return [x * YD, apex * YD, (-vy).atan2(vx).to_degrees()];
            }
            s = n;
            k1 = kn;
        }
        [f64::NAN; 3]
    }
}

/// 가우스-에르미트 n점 (표준정규 기댓값): 마디와 가중치. 골럽-웰시 대신 뉴턴으로 에르미트 다항식의 근을 찾는다.
pub fn gh(n: usize) -> (Vec<f64>, Vec<f64>) {
    // 확률론자 에르미트 He_n 의 근과 가중치 w = n! / (n He_{n-1}(x))^2
    let he = |x: f64| -> (f64, f64) {
        let (mut p0, mut p1) = (1.0, x);
        for k in 1..n {
            let p2 = x * p1 - k as f64 * p0;
            p0 = p1;
            p1 = p2;
        }
        (p1, p0) // He_n, He_{n-1}
    };
    let mut xs = Vec::new();
    let mut ws = Vec::new();
    let fact: f64 = (1..=n).map(|k| k as f64).product();
    // 근은 |x| < sqrt(4n+2) 안에 있다: 잘게 훑어 부호가 바뀌는 칸을 이분법으로 좁힌다
    let r = (4.0 * n as f64 + 2.0).sqrt();
    let m = 20000;
    let mut prev = (-r, he(-r).0);
    for i in 1..=m {
        let x = -r + 2.0 * r * i as f64 / m as f64;
        let p = he(x).0;
        if (p == 0.0) || (p.signum() != prev.1.signum()) {
            let (mut lo, mut hi) = (prev.0, x);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if he(mid).0.signum() == he(lo).0.signum() { lo = mid } else { hi = mid }
            }
            let x0 = 0.5 * (lo + hi);
            let (_, q) = he(x0);
            xs.push(x0);
            ws.push(fact / (n as f64 * q).powi(2));
        }
        prev = (x, p);
    }
    assert_eq!(xs.len(), n, "가우스-에르미트 근 개수");
    (xs, ws)
}

pub fn gh7() -> (Vec<f64>, Vec<f64>) {
    gh(7)
}

/// USGA 행별 분포 평균 캐리 (입력 세 개 독립 정규, 20 mph·0.5°·200 rpm 에서 자름) — 구적 343 점
pub fn usga_gh<F: Flight + ?Sized>(m: &F) -> Vec<f64> {
    usga_gh_n(m, 7)
}

pub fn usga_gh_n<F: Flight + ?Sized>(m: &F, n: usize) -> Vec<f64> {
    usga_gh3(m, [n, n, n])
}

/// 축별 점 수 (볼스피드, 발사각, 스핀)
pub fn usga_gh3<F: Flight + ?Sized>(m: &F, n: [usize; 3]) -> Vec<f64> {
    let (g0, g1, g2) = (gh(n[0]), gh(n[1]), gh(n[2]));
    USGA.iter()
        .map(|r| {
            let (_, mv, sv, md, sd, ms, ss, _) = *r;
            let mut acc = 0.0;
            for i in 0..n[0] {
                for j in 0..n[1] {
                    for k in 0..n[2] {
                        let c = m.obs((mv + sv * g0.0[i]).max(20.0), (md + sd * g1.0[j]).max(0.5), (ms + ss * g2.0[k]).max(200.0))[0];
                        acc += g0.1[i] * g1.1[j] * g2.1[k] * c;
                    }
                }
            }
            acc
        })
        .collect()
}

pub fn rmse(e: &[f64]) -> f64 {
    (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt()
}

/// 표 하나에 대한 세 양 RMSE
pub fn score3<F: Flight + ?Sized>(m: &F, rows: &[[f64; 6]]) -> [f64; 3] {
    let o: Vec<[f64; 3]> = rows.iter().map(|r| m.obs(r[0], r[1], r[2])).collect();
    std::array::from_fn(|c| rmse(&o.iter().zip(rows).map(|(o, r)| o[c] - r[3 + c]).collect::<Vec<_>>()))
}

/// 맞출 수 있는 모형: 자유 매개변수의 값과 범위
pub trait Tunable: Flight {
    fn get(&self) -> Vec<f64>;
    fn set(&mut self, p: &[f64]);
    fn bounds(&self) -> (Vec<f64>, Vec<f64>);
}

impl Tunable for Model {
    fn get(&self) -> Vec<f64> {
        self.free().iter().map(|&i| self.p[i]).collect()
    }
    fn set(&mut self, q: &[f64]) {
        for (j, i) in self.free().into_iter().enumerate() {
            self.p[i] = q[j];
        }
    }
    fn bounds(&self) -> (Vec<f64>, Vec<f64>) {
        let f = self.free();
        (f.iter().map(|&i| self.lo[i]).collect(), f.iter().map(|&i| self.hi[i]).collect())
    }
}

/// 상자 제약 레벤버그-마쿼트. 자유 매개변수만 움직인다. 반환: 비용(잔차 제곱합)
/// w: 캐리·높이·착지각 잔차 가중치 (0 이면 빼기)
pub fn fit<T: Tunable>(m: &mut T, rows: &[[f64; 6]], w: [f64; 3]) -> f64 {
    let mut p = m.get();
    let (lo, hi) = m.bounds();
    let n = p.len();
    if n == 0 {
        return f64::NAN;
    }
    let res = |m: &T| -> Vec<f64> {
        let mut out = Vec::with_capacity(rows.len() * 3);
        for r in rows {
            let o = m.obs(r[0], r[1], r[2]);
            for c in 0..3 {
                if w[c] > 0.0 {
                    out.push(w[c] * (o[c] - r[3 + c]));
                }
            }
        }
        out.iter().map(|x| if x.is_finite() { *x } else { 1e3 }).collect()
    };
    let cost = |r: &[f64]| r.iter().map(|x| x * x).sum::<f64>();
    let mut r = res(m);
    let mut mu = 1e-3;
    for _ in 0..200 {
        let mut jac = vec![vec![0.0; n]; r.len()];
        for k in 0..n {
            let h = 1e-6 * p[k].abs().max(1e-3);
            let mut q = p.clone();
            q[k] += h;
            m.set(&q);
            let rq = res(m);
            for i in 0..r.len() {
                jac[i][k] = (rq[i] - r[i]) / h;
            }
        }
        m.set(&p);
        let mut improved = false;
        for _ in 0..14 {
            let mut a = vec![vec![0.0; n + 1]; n];
            for i in 0..r.len() {
                for x in 0..n {
                    a[x][n] -= jac[i][x] * r[i];
                    for y in 0..n {
                        a[x][y] += jac[i][x] * jac[i][y];
                    }
                }
            }
            for x in 0..n {
                a[x][x] = a[x][x] * (1.0 + mu) + 1e-12;
            }
            let dp = solve(a);
            let q: Vec<f64> = (0..n).map(|k| (p[k] + dp[k]).clamp(lo[k], hi[k])).collect();
            m.set(&q);
            let rq = res(m);
            if cost(&rq) < cost(&r) {
                let rel = (cost(&r) - cost(&rq)) / cost(&r).max(1e-30);
                p = q;
                r = rq;
                mu = (mu * 0.3).max(1e-12);
                improved = true;
                if rel < 1e-12 {
                    return cost(&r);
                }
                break;
            }
            m.set(&p);
            mu *= 10.0;
        }
        if !improved {
            break;
        }
    }
    cost(&r)
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

/// models.txt 읽기.
///
/// ```text
/// [H2]
/// kd = v*u + d*w
/// kl = c*w*u/sqrt(u^2 + (l*w)^2)
/// v = 0.24 0.05 0.6      # 시작 하한 상한 (하한·상한을 빼면 고정)
/// ```
pub fn load_models(text: &str) -> Result<Vec<Model>, String> {
    let mut out = Vec::new();
    let mut cur: Option<(String, String, String, Vec<(String, f64, f64, f64)>)> = None;
    let flush = |c: Option<(String, String, String, Vec<(String, f64, f64, f64)>)>, out: &mut Vec<Model>| -> Result<(), String> {
        if let Some((name, kd, kl, ps)) = c {
            let ps2: Vec<(&str, f64, f64, f64)> = ps.iter().map(|p| (p.0.as_str(), p.1, p.2, p.3)).collect();
            out.push(Model::new(&name, &kd, &kl, &ps2)?);
        }
        Ok(())
    };
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            flush(cur.take(), &mut out)?;
            cur = Some((line[1..line.len() - 1].trim().to_string(), String::new(), String::new(), Vec::new()));
            continue;
        }
        let (k, v) = line.split_once('=').ok_or(format!("{} 줄: '이름 = 값' 꼴이 아님", ln + 1))?;
        let (k, v) = (k.trim(), v.trim());
        let c = cur.as_mut().ok_or(format!("{} 줄: [모형 이름] 이 먼저 와야 함", ln + 1))?;
        match k {
            "kd" => c.1 = v.into(),
            "kl" => c.2 = v.into(),
            _ => {
                let nums: Vec<f64> = v.split_whitespace().map(|x| x.parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("{} 줄: 숫자 아님", ln + 1))?;
                let (s, lo, hi) = match nums.as_slice() {
                    [s] => (*s, *s, *s),
                    [s, lo, hi] => (*s, *lo, *hi),
                    _ => return Err(format!("{} 줄: '시작' 또는 '시작 하한 상한'", ln + 1)),
                };
                c.3.push((k.into(), s, lo, hi));
            }
        }
    }
    flush(cur, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gh_moments() {
        for n in [5, 7, 11] {
            let (x, w) = gh(n);
            let m = |k: i32| x.iter().zip(&w).map(|(x, w)| w * x.powi(k)).sum::<f64>();
            assert!((m(0) - 1.0).abs() < 1e-12 && m(1).abs() < 1e-12 && (m(2) - 1.0).abs() < 1e-12 && (m(4) - 3.0).abs() < 1e-10, "n={n}");
        }
    }
}

/// 3차원 비행 (스핀축 기울기 포함): (캐리 yd = 처음 수평 방향으로의 거리, 최고 높이 yd)
pub trait Flight3 {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2];
}

impl Model {
    #[inline]
    fn f3(&self, vars: &mut [f64], s: &[f64; 7], oy: f64, oz: f64) -> [f64; 7] {
        let sp = (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt();
        vars[0] = sp;
        vars[1] = s[6];
        vars[2] = s[6] / sp;
        vars[3] = sp * RE_PER_U;
        let (kd, kl) = (self.kd.eval(vars), self.kl.eval(vars));
        let beta = self.beta_idx.map(|i| vars[4 + i]).unwrap_or(0.06);
        let g = 1.0 / sp.sqrt();
        let (cx, cy, cz) = (oy * s[5] - oz * s[4], oz * s[3], -oy * s[3]);
        [s[3] * g, s[4] * g, s[5] * g, (-kd * s[3] + kl * cx) * g, (-kd * s[4] + kl * cy - 1.0) * g, (-kd * s[5] + kl * cz) * g, -beta * s[6] * sp * g]
    }
}

impl Flight3 for Model {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2] {
        let [v0, th, s0] = dimless(mph, deg, rpm);
        let (oy, oz) = (axis_deg.to_radians().sin(), axis_deg.to_radians().cos());
        let mut vars = vec![0.0; 4 + self.p.len()];
        vars[4..].copy_from_slice(&self.p);
        let h = self.h;
        let mut s = [0.0, 0.0, 0.0, v0 * th.cos(), v0 * th.sin(), 0.0, s0 * v0];
        let mut k1 = self.f3(&mut vars, &s, oy, oz);
        let mut apex = 0.0f64;
        for _ in 0..(12.0 / h) as usize {
            let st = |k: &[f64; 7], c: f64| -> [f64; 7] { std::array::from_fn(|i| s[i] + c * k[i]) };
            let k2 = self.f3(&mut vars, &st(&k1, 0.5 * h), oy, oz);
            let k3 = self.f3(&mut vars, &st(&k2, 0.5 * h), oy, oz);
            let k4 = self.f3(&mut vars, &st(&k3, h), oy, oz);
            let n: [f64; 7] = std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
            let kn = self.f3(&mut vars, &n, oy, oz);
            let (y0, y1, m0, m1) = (s[1], n[1], k1[1] * h, kn[1] * h);
            if k1[1] > 0.0 && kn[1] <= 0.0 {
                let a = 6.0 * y0 + 3.0 * m0 - 6.0 * y1 + 3.0 * m1;
                let b = -6.0 * y0 - 4.0 * m0 + 6.0 * y1 - 2.0 * m1;
                let t = if a.abs() < 1e-14 { -m0 / b } else {
                    let d = (b * b - 4.0 * a * m0).max(0.0).sqrt();
                    let (r1, r2) = ((-b - d) / (2.0 * a), (-b + d) / (2.0 * a));
                    if (0.0..=1.0).contains(&r1) { r1 } else { r2 }
                }
                .clamp(0.0, 1.0);
                let hh = herm(t);
                apex = apex.max(hh[0] * y0 + hh[1] * m0 + hh[2] * y1 + hh[3] * m1);
            }
            if n[1] < 0.0 && s[1] >= 0.0 {
                let mut t = if y0 <= 0.0 { 1.0 } else { y0 / (y0 - y1) };
                for _ in 0..4 {
                    let hh = herm(t);
                    let f = hh[0] * y0 + hh[1] * m0 + hh[2] * y1 + hh[3] * m1;
                    let t2 = t * t;
                    let df = (6.0 * t2 - 6.0 * t) * (y0 - y1) + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (3.0 * t2 - 2.0 * t) * m1;
                    t = (t - f / df).clamp(0.0, 1.0);
                }
                let hh = herm(t);
                let x = hh[0] * s[0] + hh[1] * k1[0] * h + hh[2] * n[0] + hh[3] * kn[0] * h;
                return [x * YD, apex * YD];
            }
            s = n;
            k1 = kn;
        }
        [f64::NAN; 2]
    }
}

/// FlightScope 계산값 175샷: (mph, 발사각, 총 스핀, 스핀축, 캐리 yd, 최고 높이 ft)
pub fn load_fs(path: &str) -> Vec<[f64; 6]> {
    std::fs::read_to_string(path)
        .map(|t| {
            t.lines()
                .filter(|l| !l.starts_with('#') && !l.starts_with("speed"))
                .filter_map(|l| {
                    let v: Vec<f64> = l.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    (v.len() == 6).then(|| [v[0], v[1], v[2], v[3], v[4], v[5]])
                })
                .collect()
        })
        .unwrap_or_default()
}

/// FS 자료 채점: [캐리 RMSE 전체, 느린 공(<80 mph), 중간(80–110), 빠른(>110), 최고 높이 RMSE yd, 캐리 평균 부호 오차]
pub fn score_fs<F: Flight3 + ?Sized>(m: &F, fs: &[[f64; 6]]) -> [f64; 6] {
    let o: Vec<[f64; 2]> = fs.iter().map(|r| m.obs3(r[0], r[1], r[2], r[3])).collect();
    let ec: Vec<f64> = o.iter().zip(fs).map(|(o, r)| o[0] - r[4]).collect();
    let ea: Vec<f64> = o.iter().zip(fs).map(|(o, r)| o[1] - r[5] / 3.0).collect();
    let bin = |lo: f64, hi: f64| rmse(&fs.iter().zip(&ec).filter(|(r, _)| r[0] >= lo && r[0] < hi).map(|(_, e)| *e).collect::<Vec<_>>());
    [rmse(&ec), bin(0.0, 80.0), bin(80.0, 110.0), bin(110.0, 1e9), rmse(&ea), ec.iter().sum::<f64>() / ec.len() as f64]
}

/// 한 행 빼기 교차검증: 행 하나를 빼고 맞춘 뒤 그 행을 예측한다 (전체 맞춤값에서 시작).
/// 반환 [캐리, 높이, 착지 RMSE, 세 양 합친 RMSE]. 판정 자료를 보지 않고 꼴을 고르는 기준.
pub fn loo_cv(m: &Model, rows: &[[f64; 6]], w: [f64; 3], threads: usize) -> [f64; 4] {
    let n = rows.len();
    let idx: Vec<usize> = (0..n).collect();
    let chunk = n.div_ceil(threads.max(1));
    let mut errs: Vec<[f64; 3]> = vec![[0.0; 3]; n];
    std::thread::scope(|sc| {
        let hs: Vec<_> = idx
            .chunks(chunk)
            .map(|part| {
                let m = m.clone();
                sc.spawn(move || {
                    part.iter()
                        .map(|&i| {
                            let tr: Vec<[f64; 6]> = rows.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, r)| *r).collect();
                            let mut mi = m.clone();
                            fit(&mut mi, &tr, w);
                            let r = rows[i];
                            let o = mi.obs(r[0], r[1], r[2]);
                            (i, [o[0] - r[3], o[1] - r[4], o[2] - r[5]])
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in hs {
            for (i, e) in h.join().unwrap() {
                errs[i] = e;
            }
        }
    });
    let col = |c: usize| (errs.iter().map(|e| e[c] * e[c]).sum::<f64>() / n as f64).sqrt();
    let all = (errs.iter().map(|e| (0..3).map(|c| w[c].min(1.0) * e[c] * e[c]).sum::<f64>()).sum::<f64>() / (n as f64 * w.iter().filter(|x| **x > 0.0).count() as f64)).sqrt();
    [col(0), col(1), col(2), all]
}
