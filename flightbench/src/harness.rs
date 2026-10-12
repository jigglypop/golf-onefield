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
    /// 만들 때의 시작값. 교차검증이 접은 자료로 다시 맞출 때 여기서 출발한다 (`reset`)
    pub p_init: Vec<f64>,
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
            p_init: params.iter().map(|p| p.1).collect(),
            lo: params.iter().map(|p| p.2).collect(),
            hi: params.iter().map(|p| p.3).collect(),
            beta_idx: params.iter().position(|p| p.0 == "beta"),
            h: 0.05,
        })
    }

    pub fn free(&self) -> Vec<usize> {
        (0..self.p.len()).filter(|&i| self.hi[i] > self.lo[i]).collect()
    }

    /// 매개변수를 만들 때의 시작값으로 되돌린다. 교차검증은 접마다 이것으로 되돌린 뒤 **훈련 행만으로** 다시 맞춘다:
    /// 전체 자료로 맞춘 값에서 출발하면 뺀 행의 정보가 시작점으로 새어 들어간다.
    pub fn reset(&mut self) {
        self.p.clone_from(&self.p_init);
    }

    /// 변수표 [u, w, S, Re, 매개변수...] 를 만들어 f 에 준다. 매개변수가 28개 이하면 스택 배열을 쓴다
    /// (obs 가 호출마다 Vec 을 새로 만들던 것을 없앤다).
    #[inline]
    fn with_vars<R>(&self, f: impl FnOnce(&mut [f64]) -> R) -> R {
        const STACK: usize = 32;
        let nv = 4 + self.p.len();
        if nv <= STACK {
            let mut buf = [0.0f64; STACK];
            buf[4..nv].copy_from_slice(&self.p);
            f(&mut buf[..nv])
        } else {
            let mut v = vec![0.0; nv];
            v[4..].copy_from_slice(&self.p);
            f(&mut v)
        }
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
        self.with_vars(|vars| self.obs_with(vars, mph, deg, rpm))
    }
}

impl Model {
    fn obs_with(&self, vars: &mut [f64], mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
        let [v0, th, s0] = dimless(mph, deg, rpm);
        let h = self.h;
        let mut s = [0.0, 0.0, v0 * th.cos(), v0 * th.sin(), s0 * v0];
        let mut k1 = self.f(vars, &s);
        let mut apex = 0.0f64;
        for _ in 0..(12.0 / h) as usize {
            let a: [f64; 5] = std::array::from_fn(|i| s[i] + 0.5 * h * k1[i]);
            let k2 = self.f(vars, &a);
            let b: [f64; 5] = std::array::from_fn(|i| s[i] + 0.5 * h * k2[i]);
            let k3 = self.f(vars, &b);
            let c: [f64; 5] = std::array::from_fn(|i| s[i] + h * k3[i]);
            let k4 = self.f(vars, &c);
            let n: [f64; 5] = std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
            let kn = self.f(vars, &n);
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

/// 가우스-에르미트 n점 (표준정규 기댓값): 마디와 가중치. 같은 n 은 한 번만 계산해 둔다
/// (근 찾기가 2만 칸 훑기라 `usga_gh3` 가 호출마다 다시 하면 시험대의 모형 한 줄보다 비싸다).
pub fn gh_cached(n: usize) -> &'static (Vec<f64>, Vec<f64>) {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<usize, &'static (Vec<f64>, Vec<f64>)>>> = OnceLock::new();
    let mut map = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|e| e.into_inner());
    // 근 찾기는 잠금 안에서 한다: 같은 n 을 두 스레드가 동시에 계산하지 않게 (n 이 몇 개뿐이라 경합은 무시할 만하다)
    *map.entry(n).or_insert_with(|| Box::leak(Box::new(gh(n))))
}

/// 가우스-에르미트 n점의 근과 가중치를 매번 새로 계산한다 (복사본이 필요할 때). 반복 호출에는 `gh_cached`.
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
    let (g0, g1, g2) = (gh_cached(n[0]), gh_cached(n[1]), gh_cached(n[2]));
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
    lm(m, &|m: &T| tour_res(m, rows, w))
}

/// `fit` 과 같은 결과, 야코비안 열을 스레드로 나눠 구한다 (`lm_par`). 한 모형을 혼자 맞출 때 쓴다.
pub fn fit_par<T: Tunable + Clone + Send>(m: &mut T, rows: &[[f64; 6]], w: [f64; 3]) -> f64 {
    lm_par(m, &|m: &T| tour_res(m, rows, w))
}

/// 투어표 세 양 잔차 (가중치 0 인 양은 뺀다)
fn tour_res<T: Flight + ?Sized>(m: &T, rows: &[[f64; 6]], w: [f64; 3]) -> Vec<f64> {
    let mut out = Vec::with_capacity(rows.len() * 3);
    for r in rows {
        let o = m.obs(r[0], r[1], r[2]);
        for c in 0..3 {
            if w[c] > 0.0 {
                out.push(w[c] * (o[c] - r[3 + c]));
            }
        }
    }
    out
}

/// FS 샷 잔차: 캐리 yd, 최고 높이 yd (자료는 ft)
fn fs_res<T: Flight3 + ?Sized>(m: &T, fs: &[[f64; 6]], wfs: f64, out: &mut Vec<f64>) {
    for s in fs {
        let o = m.obs3(s[0], s[1], s[2], s[3]);
        out.push(wfs * (o[0] - s[4]));
        out.push(wfs * (o[1] - s[5] / 3.0));
    }
}

/// 혼합 맞춤: 투어표 세 양 + FS 샷(사이드 스핀 포함, 캐리·최고 높이)을 함께. wfs 는 FS 잔차 가중치
pub fn fit_mixed<T: Tunable + Flight3>(m: &mut T, rows: &[[f64; 6]], w: [f64; 3], fs: &[[f64; 6]], wfs: f64) -> f64 {
    lm(m, &|m: &T| {
        let mut r = tour_res(m, rows, w);
        fs_res(m, fs, wfs, &mut r);
        r
    })
}

/// `fit_mixed` 와 같은 결과, 야코비안 열을 스레드로 나눠 구한다.
pub fn fit_mixed_par<T: Tunable + Flight3 + Clone + Send>(m: &mut T, rows: &[[f64; 6]], w: [f64; 3], fs: &[[f64; 6]], wfs: f64) -> f64 {
    lm_par(m, &|m: &T| {
        let mut r = tour_res(m, rows, w);
        fs_res(m, fs, wfs, &mut r);
        r
    })
}

/// 혼합 자료 k겹 교차검증 (꼴 고르기용). 투어 행과 FS 샷을 각각 i % k 로 나눠 한 겹씩 빼고 맞춘다.
/// 접마다 모형을 **시작값(`Model::reset`)으로 되돌린 뒤 훈련 행만으로** 맞춘다. 이전 판은 전체 자료로 맞춘 값에서
/// 출발해 뺀 행의 정보가 시작점으로 새어 들어갔다.
/// 반환 [투어 캐리, 투어 높이, 투어 착지, FS 캐리, FS 높이, 종합(훈련과 같은 가중 RMS)]
pub fn kfold_mixed(m: &Model, rows: &[[f64; 6]], w: [f64; 3], fs: &[[f64; 6]], wfs: f64, k: usize) -> [f64; 6] {
    let folds: Vec<(Vec<[f64; 5]>, Vec<[f64; 2]>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..k)
            .map(|f| {
                let m = m.clone();
                sc.spawn(move || {
                    let tr: Vec<[f64; 6]> = rows.iter().enumerate().filter(|(i, _)| i % k != f).map(|(_, r)| *r).collect();
                    let fr: Vec<[f64; 6]> = fs.iter().enumerate().filter(|(i, _)| i % k != f).map(|(_, r)| *r).collect();
                    let mut mi = m;
                    mi.reset();
                    fit_mixed(&mut mi, &tr, w, &fr, wfs);
                    let te: Vec<[f64; 5]> = rows.iter().enumerate().filter(|(i, _)| i % k == f).map(|(_, r)| {
                        let o = mi.obs(r[0], r[1], r[2]);
                        [o[0] - r[3], o[1] - r[4], o[2] - r[5], 0.0, 0.0]
                    }).collect();
                    let fe: Vec<[f64; 2]> = fs.iter().enumerate().filter(|(i, _)| i % k == f).map(|(_, s)| {
                        let o = mi.obs3(s[0], s[1], s[2], s[3]);
                        [o[0] - s[4], o[1] - s[5] / 3.0]
                    }).collect();
                    (te, fe)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let te: Vec<[f64; 5]> = folds.iter().flat_map(|f| f.0.clone()).collect();
    let fe: Vec<[f64; 2]> = folds.iter().flat_map(|f| f.1.clone()).collect();
    let col = |c: usize| rmse(&te.iter().map(|e| e[c]).collect::<Vec<_>>());
    let fcol = |c: usize| rmse(&fe.iter().map(|e| e[c]).collect::<Vec<_>>());
    let ss: f64 = te.iter().map(|e| (0..3).map(|c| (w[c] * e[c]).powi(2)).sum::<f64>()).sum::<f64>()
        + fe.iter().map(|e| (wfs * e[0]).powi(2) + (wfs * e[1]).powi(2)).sum::<f64>();
    let nres = te.len() * w.iter().filter(|x| **x > 0.0).count() + 2 * fe.len();
    [col(0), col(1), col(2), fcol(0), fcol(1), (ss / nres as f64).sqrt()]
}

/// 레벤버그-마쿼트 설정: 상대 걸음 1e-6, 감쇠 A_xx(1+μ)+1e-12, 안쪽 14번, 상대 개선 1e-12 이하에서 끝
fn lm_cfg<'a>(lo: &'a [f64], hi: &'a [f64]) -> crate::lm::LmCfg<'a> {
    crate::lm::LmCfg {
        lo,
        hi,
        step: crate::lm::Step::Rel { rel: 1e-6, floor: 1e-3 },
        damping: crate::lm::Damping::Mul,
        max_iter: 200,
        tries: 14,
        mu_min: 1e-12,
        stop_rel: 1e-12,
        nonfinite_cost: None,
    }
}

/// 수가 아닌 잔차는 1e3 으로 둔다
fn finite_res(r: Vec<f64>) -> Vec<f64> {
    r.into_iter().map(|x| if x.is_finite() { x } else { 1e3 }).collect()
}

/// 상자 제약 레벤버그-마쿼트: 잔차 함수 하나를 줄인다. 자유 매개변수만 움직이고, 수가 아닌 잔차는 1e3 으로 둔다.
/// 알고리즘은 `lm::lm` 하나다(직렬). 야코비안 열을 스레드로 나누는 판은 `lm_par`.
pub fn lm<T: Tunable>(m: &mut T, res_raw: &dyn Fn(&T) -> Vec<f64>) -> f64 {
    let p0 = m.get();
    let (lo, hi) = m.bounds();
    if p0.is_empty() {
        return f64::NAN;
    }
    let cfg = lm_cfg(&lo, &hi);
    let mut res = |q: &[f64]| {
        m.set(q);
        finite_res(res_raw(m))
    };
    let out = crate::lm::lm(&cfg, &p0, &mut res, None);
    m.set(&out.p);
    out.cost
}

/// `lm` 과 같은 결과(비트까지)인데 야코비안의 열(매개변수마다 잔차 한 벌)을 `std::thread::scope` 로 나눠 구한다.
/// 열은 서로 독립이고 출력은 열 번호 순으로 모으므로 결정적이다. 스레드마다 모형 복사본(`T: Clone + Send`)을 쓴다.
/// 이미 여러 스레드 안에서 도는 호출(교차검증)에서는 쓰지 않는다 — 스레드가 겹쳐 오히려 느리다.
pub fn lm_par<T: Tunable + Clone + Send>(m: &mut T, res_raw: &(dyn Fn(&T) -> Vec<f64> + Sync)) -> f64 {
    let p0 = m.get();
    let (lo, hi) = m.bounds();
    if p0.is_empty() {
        return f64::NAN;
    }
    let cfg = lm_cfg(&lo, &hi);
    let mut work = m.clone();
    let proto = m.clone();
    let mut res = |q: &[f64]| {
        work.set(q);
        finite_res(res_raw(&work))
    };
    let mut jac = |p: &[f64], r: &[f64], h: &[f64]| -> Vec<Vec<f64>> {
        std::thread::scope(|sc| {
            let hs: Vec<_> = (0..p.len())
                .map(|k| {
                    let mut mk = proto.clone();
                    let mut q = p.to_vec();
                    q[k] += h[k];
                    sc.spawn(move || {
                        mk.set(&q);
                        let rq = finite_res(res_raw(&mk));
                        (0..r.len()).map(|i| (rq[i] - r[i]) / h[k]).collect::<Vec<f64>>()
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        })
    };
    let out = crate::lm::lm(&cfg, &p0, &mut res, Some(&mut jac));
    m.set(&out.p);
    out.cost
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

/// 3차원 비행 한 걸음의 도함수 (τ 재매개: dτ = |u|^(1/2) dt). kk(|u|, w) → (|u| C_D, |u| C_L)
#[inline]
fn f3_with(kk: &mut dyn FnMut(f64, f64) -> (f64, f64), beta: f64, s: &[f64; 7], oy: f64, oz: f64) -> [f64; 7] {
    let sp = (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt();
    let (kd, kl) = kk(sp, s[6]);
    let g = 1.0 / sp.sqrt();
    let (cx, cy, cz) = (oy * s[5] - oz * s[4], oz * s[3], -oy * s[3]);
    [s[3] * g, s[4] * g, s[5] * g, (-kd * s[3] + kl * cx) * g, (-kd * s[4] + kl * cy - 1.0) * g, (-kd * s[5] + kl * cz) * g, -beta * s[6] * sp * g]
}

impl Flight3 for Model {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2] {
        let r = self.fly3(mph, deg, rpm, axis_deg);
        [r[0], r[2]]
    }
}

impl Model {
    /// 3차원 비행의 착지: [앞쪽 거리 yd (처음 수평 방향), 옆 편차 yd (오른쪽 +), 최고 높이 yd, 착지각 deg (내려가는 쪽 +),
    /// 착지 속력 mph, 비행 시간 s, 최고점까지 앞쪽 거리 yd].
    /// 축 기울기 a > 0 이면 왼쪽으로 휜다 (z 가속 = -sin a · ux · kl).
    pub fn fly3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7] {
        self.with_vars(|vars| {
            let beta = self.beta_idx.map(|i| vars[4 + i]).unwrap_or(0.06);
            let mut kk = |sp: f64, w: f64| {
                vars[0] = sp;
                vars[1] = w;
                vars[2] = w / sp;
                vars[3] = sp * RE_PER_U;
                (self.kd.eval(vars), self.kl.eval(vars))
            };
            fly3_with(&mut kk, beta, self.h, mph, deg, rpm, axis_deg)
        })
    }
}

/// 장 함수 kk(|u|, w) 와 스핀 감쇠 beta 로 날린 3차원 비행. 반환은 `Model::fly3` 와 같다.
/// h = τ 걸음 (무차원). 장은 식 해석기든 곡면 표든 상관없다.
pub fn fly3_with(kk: &mut dyn FnMut(f64, f64) -> (f64, f64), beta: f64, h: f64, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7] {
    let [v0, th, s0] = dimless(mph, deg, rpm);
    let (oy, oz) = (axis_deg.to_radians().sin(), axis_deg.to_radians().cos());
    let mut s = [0.0, 0.0, 0.0, v0 * th.cos(), v0 * th.sin(), 0.0, s0 * v0];
    let mut k1 = f3_with(kk, beta, &s, oy, oz);
    let mut apex = 0.0f64;
    let mut apex_x = 0.0f64;
    let mut time = 0.0f64; // dt = dτ / sqrt|u| 를 단계마다 심프슨으로 쌓는다 (무차원)
    let gi = |s: &[f64; 7]| 1.0 / (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt().sqrt();
    for _ in 0..(12.0 / h) as usize {
        let st = |k: &[f64; 7], c: f64| -> [f64; 7] { std::array::from_fn(|i| s[i] + c * k[i]) };
        let s2 = st(&k1, 0.5 * h);
        let k2 = f3_with(kk, beta, &s2, oy, oz);
        let s3 = st(&k2, 0.5 * h);
        let k3 = f3_with(kk, beta, &s3, oy, oz);
        let s4 = st(&k3, h);
        let k4 = f3_with(kk, beta, &s4, oy, oz);
        let n: [f64; 7] = std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
        let kn = f3_with(kk, beta, &n, oy, oz);
        let dt = h / 6.0 * (gi(&s) + 2.0 * gi(&s2) + 2.0 * gi(&s3) + gi(&s4));
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
            let ya = hh[0] * y0 + hh[1] * m0 + hh[2] * y1 + hh[3] * m1;
            if ya > apex {
                apex = ya;
                apex_x = hh[0] * s[0] + hh[1] * k1[0] * h + hh[2] * n[0] + hh[3] * kn[0] * h;
            }
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
            let at = |i: usize, j: usize| hh[0] * s[i] + hh[1] * k1[j] * h + hh[2] * n[i] + hh[3] * kn[j] * h;
            let (x, z) = (at(0, 0), at(2, 2));
            let (vx, vy, vz) = (at(3, 3), at(4, 4), at(5, 5));
            let t_s = (time + t * dt) * crate::engine::L_M / crate::engine::U_MS; // 걸음 안은 선형으로
            return [
                x * YD,
                z * YD,
                apex * YD,
                (-vy).atan2(vx).to_degrees(),
                (vx * vx + vy * vy + vz * vz).sqrt() * crate::engine::U_MS / crate::engine::MPH,
                t_s,
                apex_x * YD,
            ];
        }
        time += dt;
        s = n;
        k1 = kn;
    }
    [f64::NAN; 7]
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

/// 한 행 빼기 교차검증: 행 하나를 빼고 맞춘 뒤 그 행을 예측한다. 접마다 시작값(`Model::reset`)에서 출발한다
/// (전체 맞춤값에서 출발하면 뺀 행의 정보가 시작점으로 새어 든다).
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
                            mi.reset(); // 시작값에서 훈련 행만으로 (전체 맞춤값에서 출발하면 뺀 행이 시작점에 스며든다)
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
