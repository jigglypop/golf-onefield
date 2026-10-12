//! 자유 형 공기역학 곡면 시험: 공유위기12 의 C_D(Re, S), C_L(Re, S) 를 자유 형으로 풀어 실측에 맞춘다.
//!
//! 질문: 착지 속력 −5 mph·착지각 +2.8°·비행 시간 +0.2 s 의 계통 오차는 (Re, S) 평면의 어떤 계수 모양이 빠져서인가?
//! 식 한두 항(R1–R3, 위기 없음/지수, β 자유)으로는 못 고쳤다. 곡면 자체를 풀어 고칠 수 있는 꼴이 있는지, 있다면 어디인지 본다.
//!
//!   TM_CSV=../data/real/external/trackman_public/data.csv ./target/quick/aeroflex [--grid | --sep] [옵션]
//!     --grid                  기준 장에 C_D·C_L 이중선형 보정표(Re 5 × S 5)를 더한다 (기본). --drag/--lift 로 한쪽만
//!     --sep                   분리형: C_D = f(Re) + d r(S), C_L = c r(S) g(Re). f 는 Re 마디 7개 (g 는 --glift 일 때만 자유)
//!     --win | --win0 | --wq   공유위기12 에 양력 위기창 3개 매개변수를 더한 꼴 | 창 없이 기준 6개만 다시 맞춘 대조군 | 창을 항력 위기에 묶은 꼴(깊이 하나)
//!     --beta                  스핀 감쇠 β 도 자유 (기본 고정 0.06)
//!     --lam=0.5 --smooth=0.5  표 값 능선·이차 차분 평활 무게
//!     --wtm=1                 실측 묶음 무게 (투어 PGA 12행 묶음 = 1)
//!     --no-spd                착지 속력·비행 시간·최고점 거리를 맞춤에 안 씀
//!     --nocv                  세션 보류를 건너뜀 (전체 맞춤만)
//!     --dump                  전체로 맞춘 표를 찍음
//!
//! 판정: 실측 연속 세션 하나씩 빼고 맞춘 뒤 뺀 세션을 채점(세션 단위 보류). 투어 LPGA 11행·USGA 20행·FS 175샷은 맞춤에 쓰지 않는다.
//! 표 값은 이 자료의 이 맞춤에서만 나온다: 물리 계수라는 증거가 아니라 '필요한 보정이 어디인가' 를 보는 도구다.

use flightbench::engine::{default_field, Aero, USGA};
use flightbench::harness::*;
use flightbench::realdata::{load, sessions, Tm};

/// 맞출 수 있는 공기역학 곡면: 비행·매개변수·정칙화·출력
trait Aeros: Tunable + Flight + Flight3 + Clone + Sync {
    /// [앞 yd, 옆 yd (오른쪽 +), 높이 yd, 착지각, 착지 속력 mph, 시간 s, 최고점 거리 yd]
    fn fly(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7];
    /// 정칙화 잔차 (맞춤 잔차 뒤에 붙는다)
    fn penalty(&self, cfg: &Cfg) -> Vec<f64>;
    fn dump(&self);
}

fn fly_with<A: Fn(f64, f64) -> (f64, f64)>(kk: A, beta: f64, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7] {
    let mut f = |sp: f64, w: f64| kk(sp, w);
    fly3_with(&mut f, beta, 0.137, mph, deg, rpm, axis_deg)
}

fn flight_impls<M: Aeros>(m: &M, mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
    let o = m.fly(mph, deg, rpm, 0.0);
    [o[0], o[2], o[3]]
}

// ------------------------------------------------------------------ 이중선형 보정표

const NX: usize = 5;
const NY: usize = 5;
/// 레이놀즈 수 마디 (로그 간격)와 스핀비 마디
const RE_KNOT: [f64; NX] = [4.5e4, 7.0e4, 1.1e5, 1.7e5, 2.6e5];
const S_KNOT: [f64; NY] = [0.04, 0.10, 0.18, 0.28, 0.42];

#[derive(Clone)]
struct Grid {
    dcd: [f64; NX * NY],
    dcl: [f64; NX * NY],
    beta: f64,
    free_d: bool,
    free_l: bool,
    free_b: bool,
}

/// 이중선형 보간 (마디 밖은 가장자리 값)
fn bilinear(tab: &[f64; NX * NY], re: f64, s: f64) -> f64 {
    let x = re.max(RE_KNOT[0]).min(RE_KNOT[NX - 1]).ln();
    let y = s.max(S_KNOT[0]).min(S_KNOT[NY - 1]);
    let (mut i, mut j) = (0, 0);
    while i + 2 < NX && x > RE_KNOT[i + 1].ln() {
        i += 1;
    }
    while j + 2 < NY && y > S_KNOT[j + 1] {
        j += 1;
    }
    let tx = (x - RE_KNOT[i].ln()) / (RE_KNOT[i + 1].ln() - RE_KNOT[i].ln());
    let ty = (y - S_KNOT[j]) / (S_KNOT[j + 1] - S_KNOT[j]);
    let at = |a: usize, b: usize| tab[a * NY + b];
    (1.0 - tx) * ((1.0 - ty) * at(i, j) + ty * at(i, j + 1)) + tx * ((1.0 - ty) * at(i + 1, j) + ty * at(i + 1, j + 1))
}

impl Grid {
    fn new(free_d: bool, free_l: bool, free_b: bool) -> Self {
        Grid { dcd: [0.0; NX * NY], dcl: [0.0; NX * NY], beta: 0.06, free_d, free_l, free_b }
    }
    #[inline]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let (kd, kl) = default_field().kk(sp, w);
        let (re, s) = (sp * RE_PER_U, w / sp);
        (kd + sp * bilinear(&self.dcd, re, s), kl + sp * bilinear(&self.dcl, re, s))
    }
}

impl Flight for Grid {
    fn obs(&self, mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
        flight_impls(self, mph, deg, rpm)
    }
}
impl Flight3 for Grid {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2] {
        let o = self.fly(mph, deg, rpm, axis_deg);
        [o[0], o[2]]
    }
}
impl Tunable for Grid {
    fn get(&self) -> Vec<f64> {
        let mut p = Vec::new();
        if self.free_d {
            p.extend(self.dcd);
        }
        if self.free_l {
            p.extend(self.dcl);
        }
        if self.free_b {
            p.push(self.beta);
        }
        p
    }
    fn set(&mut self, p: &[f64]) {
        let mut k = 0;
        if self.free_d {
            self.dcd.copy_from_slice(&p[k..k + NX * NY]);
            k += NX * NY;
        }
        if self.free_l {
            self.dcl.copy_from_slice(&p[k..k + NX * NY]);
            k += NX * NY;
        }
        if self.free_b {
            self.beta = p[k];
        }
    }
    fn bounds(&self) -> (Vec<f64>, Vec<f64>) {
        let (mut lo, mut hi) = (Vec::new(), Vec::new());
        for free in [self.free_d, self.free_l] {
            if free {
                lo.extend([-0.25; NX * NY]);
                hi.extend([0.25; NX * NY]);
            }
        }
        if self.free_b {
            lo.push(0.0);
            hi.push(0.6);
        }
        (lo, hi)
    }
}
impl Aeros for Grid {
    fn fly(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7] {
        fly_with(|sp, w| self.kk(sp, w), self.beta, mph, deg, rpm, axis_deg)
    }
    fn penalty(&self, cfg: &Cfg) -> Vec<f64> {
        let mut out = Vec::new();
        for (free, tab) in [(self.free_d, &self.dcd), (self.free_l, &self.dcl)] {
            if !free {
                continue;
            }
            out.extend(tab.iter().map(|x| cfg.lam * x));
            for i in 0..NX {
                for j in 1..NY - 1 {
                    out.push(cfg.smooth * (tab[i * NY + j - 1] - 2.0 * tab[i * NY + j] + tab[i * NY + j + 1]));
                }
            }
            for j in 0..NY {
                for i in 1..NX - 1 {
                    out.push(cfg.smooth * (tab[(i - 1) * NY + j] - 2.0 * tab[i * NY + j] + tab[(i + 1) * NY + j]));
                }
            }
        }
        out
    }
    fn dump(&self) {
        let f = default_field();
        let show = |name: &str, tab: &[f64; NX * NY], idx: usize| {
            println!("{name} (행 = Re, 열 = S {:?}), 괄호 = 보정 뒤 전체 계수", S_KNOT);
            for i in 0..NX {
                let row: Vec<String> = (0..NY)
                    .map(|j| {
                        let u = RE_KNOT[i] / RE_PER_U;
                        let k = f.kk(u, S_KNOT[j] * u);
                        let base = if idx == 0 { k.0 } else { k.1 } / u;
                        format!("{:+.3} ({:.3})", tab[i * NY + j], base + tab[i * NY + j])
                    })
                    .collect();
                println!("  Re {:>7.0}: {}", RE_KNOT[i], row.join("  "));
            }
        };
        show("ΔC_D", &self.dcd, 0);
        show("ΔC_L", &self.dcl, 1);
        println!("β = {:.4}", self.beta);
    }
}

// ------------------------------------------------------------------ 분리형

const NR: usize = 7;
const RE_SEP: [f64; NR] = [4.0e4, 5.5e4, 7.0e4, 9.0e4, 1.2e5, 1.7e5, 2.6e5];

#[derive(Clone)]
struct Sep {
    /// 배경 항력 계수 f(Re) 마디 값
    f: [f64; NR],
    /// 양력 배율 g(Re) 마디 값 (1 = 기준)
    g: [f64; NR],
    d: f64,
    c: f64,
    l: f64,
    beta: f64,
    free_g: bool,
    free_b: bool,
}

fn interp1(tab: &[f64; NR], re: f64) -> f64 {
    let x = re.max(RE_SEP[0]).min(RE_SEP[NR - 1]).ln();
    let mut i = 0;
    while i + 2 < NR && x > RE_SEP[i + 1].ln() {
        i += 1;
    }
    let t = (x - RE_SEP[i].ln()) / (RE_SEP[i + 1].ln() - RE_SEP[i].ln());
    (1.0 - t) * tab[i] + t * tab[i + 1]
}

impl Sep {
    fn new(free_g: bool, free_b: bool) -> Self {
        let b = default_field();
        // 기준 장의 배경 항력 v + e/(1 + (Re/r0)^12) 를 마디에 얹는다
        let f = std::array::from_fn(|k| b.v + b.e / (1.0 + (RE_SEP[k] / (b.u0 * RE_PER_U)).powi(12)));
        Sep { f, g: [1.0; NR], d: b.d, c: b.c, l: b.l, beta: 0.06, free_g, free_b }
    }
    #[inline]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let re = sp * RE_PER_U;
        let r = w * sp / (sp + self.l * w);
        (sp * interp1(&self.f, re) + self.d * r, self.c * r * if self.free_g { interp1(&self.g, re) } else { 1.0 })
    }
}
impl Flight for Sep {
    fn obs(&self, mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
        flight_impls(self, mph, deg, rpm)
    }
}
impl Flight3 for Sep {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2] {
        let o = self.fly(mph, deg, rpm, axis_deg);
        [o[0], o[2]]
    }
}
impl Tunable for Sep {
    fn get(&self) -> Vec<f64> {
        let mut p: Vec<f64> = self.f.to_vec();
        p.extend([self.d, self.c, self.l]);
        if self.free_g {
            p.extend(self.g);
        }
        if self.free_b {
            p.push(self.beta);
        }
        p
    }
    fn set(&mut self, p: &[f64]) {
        self.f.copy_from_slice(&p[..NR]);
        (self.d, self.c, self.l) = (p[NR], p[NR + 1], p[NR + 2]);
        let mut k = NR + 3;
        if self.free_g {
            self.g.copy_from_slice(&p[k..k + NR]);
            k += NR;
        }
        if self.free_b {
            self.beta = p[k];
        }
    }
    fn bounds(&self) -> (Vec<f64>, Vec<f64>) {
        let (mut lo, mut hi) = (vec![0.02; NR], vec![0.8; NR]);
        lo.extend([0.0, 0.5, 0.5]);
        hi.extend([5.0, 8.0, 40.0]);
        if self.free_g {
            lo.extend([0.4; NR]);
            hi.extend([2.0; NR]);
        }
        if self.free_b {
            lo.push(0.0);
            hi.push(0.6);
        }
        (lo, hi)
    }
}
impl Aeros for Sep {
    fn fly(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7] {
        fly_with(|sp, w| self.kk(sp, w), self.beta, mph, deg, rpm, axis_deg)
    }
    fn penalty(&self, cfg: &Cfg) -> Vec<f64> {
        let mut out = Vec::new();
        for i in 1..NR - 1 {
            out.push(cfg.smooth * (self.f[i - 1] - 2.0 * self.f[i] + self.f[i + 1]));
            if self.free_g {
                out.push(cfg.smooth * (self.g[i - 1] - 2.0 * self.g[i] + self.g[i + 1]));
            }
        }
        if self.free_g {
            out.extend(self.g.iter().map(|x| cfg.lam * (x - 1.0)));
        }
        out
    }
    fn dump(&self) {
        println!("분리형: C_D = f(Re) + d r,  C_L = c r g(Re),  r = S/(1 + l S);  d {:.3} c {:.3} l {:.3} β {:.4}", self.d, self.c, self.l, self.beta);
        let b = default_field();
        for k in 0..NR {
            let base = b.v + b.e / (1.0 + (RE_SEP[k] / (b.u0 * RE_PER_U)).powi(12));
            println!("  Re {:>7.0}: f = {:.3} (기준 {:.3})  g = {:.3}", RE_SEP[k], self.f[k], base, self.g[k]);
        }
    }
}

// ------------------------------------------------------------------ 양력 창 (매개변수 3개를 더한 공유위기12)

/// 공유위기12 + 양력 위기창: C_L = c r (1 − a exp(−(ln(Re/r1)/s)²)).
/// 분리형에서 g(Re) 가 Re 7e4 부근에서 푹 꺼진 모양을 3개 매개변수(깊이 a, 중심 r1, 폭 s)로 줄인 꼴.
/// 기준 6개(v e r0 d c l)도 같이 풀고, 투어 PGA 12행 + 실측 샷으로 맞춘다.
#[derive(Clone)]
struct Win {
    v: f64,
    e: f64,
    r0: f64,
    d: f64,
    c: f64,
    l: f64,
    a: f64,
    r1: f64,
    s: f64,
    beta: f64,
    free_b: bool,
    /// 창을 쓰지 않으면 기준 6개만 풀린다 (같은 자료로 맞춘 공유위기12 의 대조군)
    free_win: bool,
    /// 창의 중심·폭을 항력 위기에 묶는다: g = 1 − 4 a q (1 − q),  q = 1/(1 + (Re/r0)^12). 풀리는 창 매개변수는 깊이 a 하나
    tied: bool,
    /// 항력에도 같은 꼴의 봉우리 bd·4q(1−q) 를 더한다 (--wq2)
    bd: f64,
    free_bd: bool,
}

impl Win {
    fn new(free_win: bool, free_b: bool, tied: bool) -> Self {
        let b = default_field();
        Win { v: b.v, e: b.e, r0: b.u0 * RE_PER_U, d: b.d, c: b.c, l: b.l, a: 0.0, r1: 7.0e4, s: 0.2, beta: 0.06, free_b, free_win, tied, bd: 0.0, free_bd: false }
    }
    #[inline]
    fn kk(&self, sp: f64, w: f64) -> (f64, f64) {
        let re = sp * RE_PER_U;
        let r = w * sp / (sp + self.l * w);
        let q = (re / self.r0).powi(12);
        let g = if self.tied {
            1.0 - 4.0 * self.a * (1.0 / (1.0 + q)) * (q / (1.0 + q))
        } else {
            let x = (re / self.r1).ln() / self.s;
            1.0 - self.a * (-x * x).exp()
        };
        ((self.v + self.e / (1.0 + q) + self.bd * 4.0 * (q / (1.0 + q)) / (1.0 + q)) * sp + self.d * r, self.c * r * g)
    }
}
impl Flight for Win {
    fn obs(&self, mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
        flight_impls(self, mph, deg, rpm)
    }
}
impl Flight3 for Win {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2] {
        let o = self.fly(mph, deg, rpm, axis_deg);
        [o[0], o[2]]
    }
}
impl Tunable for Win {
    fn get(&self) -> Vec<f64> {
        let mut p = vec![self.v, self.e, self.r0, self.d, self.c, self.l];
        if self.free_win && self.tied {
            p.push(self.a);
        } else if self.free_win {
            p.extend([self.a, self.r1, self.s]);
        }
        if self.free_bd {
            p.push(self.bd);
        }
        if self.free_b {
            p.push(self.beta);
        }
        p
    }
    fn set(&mut self, p: &[f64]) {
        (self.v, self.e, self.r0, self.d, self.c, self.l) = (p[0], p[1], p[2], p[3], p[4], p[5]);
        let mut k = 6;
        if self.free_win && self.tied {
            self.a = p[k];
            k += 1;
        } else if self.free_win {
            (self.a, self.r1, self.s) = (p[k], p[k + 1], p[k + 2]);
            k += 3;
        }
        if self.free_bd {
            self.bd = p[k];
            k += 1;
        }
        if self.free_b {
            self.beta = p[k];
        }
    }
    fn bounds(&self) -> (Vec<f64>, Vec<f64>) {
        let (mut lo, mut hi) = (vec![0.0, 0.0, 3e4, 0.0, 0.0, 0.0], vec![0.6, 1.0, 1.5e5, 5.0, 20.0, 80.0]);
        if self.free_win && self.tied {
            lo.push(-0.6);
            hi.push(0.9);
        } else if self.free_win {
            lo.extend([-0.6, 4.5e4, 0.05]);
            hi.extend([0.9, 1.6e5, 1.0]);
        }
        if self.free_bd {
            lo.push(-0.3);
            hi.push(0.5);
        }
        if self.free_b {
            lo.push(0.0);
            hi.push(0.6);
        }
        (lo, hi)
    }
}
impl Aeros for Win {
    fn fly(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 7] {
        fly_with(|sp, w| self.kk(sp, w), self.beta, mph, deg, rpm, axis_deg)
    }
    fn penalty(&self, _cfg: &Cfg) -> Vec<f64> {
        Vec::new()
    }
    fn dump(&self) {
        println!(
            "v {:.4} e {:.4} r0 {:.0} d {:.4} c {:.4} l {:.4} | 창 a {:.3} r1 {:.0} s {:.3} | 항력 봉우리 bd {:.3} | β {:.4}",
            self.v, self.e, self.r0, self.d, self.c, self.l, self.a, self.r1, self.s, self.bd, self.beta
        );
    }
}

// ------------------------------------------------------------------ 맞춤·채점

/// 잡음 눈금: 잔차를 나누는 값. 실측 샷은 (캐리 6 yd, 옆 5 yd, 높이 2 yd, 착지각 5°, 착지 속력 7 mph, 시간 0.4 s, 최고점 거리 5 yd)
const SIGMA_TM: [f64; 7] = [6.0, 5.0, 2.0, 5.0, 7.0, 0.4, 5.0];
/// 투어 표 행: 캐리 3 yd, 높이 1.5 yd, 착지각 3°
const SIGMA_TOUR: [f64; 3] = [3.0, 1.5, 3.0];

struct Cfg {
    wtm: f64,
    lam: f64,
    smooth: f64,
    use_spd: bool,
}

/// 표적선 기준 오차 [캐리, 옆, 높이, 착지각, 착지 속력, 시간, 최고점 거리] (예측 − 실측)
fn tm_error<M: Aeros>(m: &M, r: &Tm) -> Option<[f64; 7]> {
    let o = m.fly(r.mph, r.vla, r.rpm, -r.axis);
    if o.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let (c, s) = (r.ld.to_radians().cos(), r.ld.to_radians().sin());
    Some([o[0] * c - o[1] * s - r.carry, o[0] * s + o[1] * c - r.side, o[2] - r.apex_yd, o[3] - r.land, o[4] - r.land_mph, o[5] - r.time_s, o[6] - r.apex_x_yd])
}

/// 맞춤 잔차: 투어 PGA 12행 + 실측 샷(무게 wtm) + 정칙화
fn residuals<M: Aeros>(m: &M, tm: &[&Tm], cfg: &Cfg) -> Vec<f64> {
    let mut out = Vec::new();
    for r in PGA12.iter() {
        let o = m.obs(r[0], r[1], r[2]);
        for c in 0..3 {
            out.push((o[c] - r[3 + c]) / SIGMA_TOUR[c]);
        }
    }
    let w = (cfg.wtm * PGA12.len() as f64 / tm.len().max(1) as f64).sqrt();
    let chunk = tm.len().div_ceil(12).max(1);
    let parts: Vec<Vec<f64>> = std::thread::scope(|sc| {
        let hs: Vec<_> = tm
            .chunks(chunk)
            .map(|part| {
                sc.spawn(move || {
                    let mut v = Vec::with_capacity(part.len() * 7);
                    for r in part {
                        match tm_error(m, r) {
                            Some(e) => v.extend((0..7).map(|k| if !cfg.use_spd && k >= 4 { 0.0 } else { w * e[k] / SIGMA_TM[k] })),
                            None => v.extend([1e3; 7]),
                        }
                    }
                    v
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    out.extend(parts.into_iter().flatten());
    out.extend(m.penalty(cfg));
    out
}

fn fit_surf<M: Aeros>(base: &M, tm: &[&Tm], cfg: &Cfg) -> M {
    let mut m = base.clone();
    lm(&mut m, &|m: &M| residuals(m, tm, cfg));
    m
}

fn moments(es: &[[f64; 7]]) -> [(f64, f64); 7] {
    std::array::from_fn(|k| {
        let v: Vec<f64> = es.iter().map(|e| e[k]).collect();
        (rmse(&v), v.iter().sum::<f64>() / v.len().max(1) as f64)
    })
}

fn print_score(tag: &str, s: &[(f64, f64); 7]) {
    println!(
        "{tag:<20} 캐리 {:5.2}/{:+5.2} | 옆 {:5.2}/{:+5.2} | 높이 {:4.2}/{:+5.2} | 착지각 {:4.2}/{:+5.2} | 속력 {:4.2}/{:+5.2} | 시간 {:4.2}/{:+5.2} | 최고점거리 {:4.2}/{:+5.2}",
        s[0].0, s[0].1, s[1].0, s[1].1, s[2].0, s[2].1, s[3].0, s[3].1, s[4].0, s[4].1, s[5].0, s[5].1, s[6].0, s[6].1
    );
}

fn tour_check<M: Aeros>(tag: &str, m: &M) {
    let tr = score3(m, &PGA12);
    let ho = score3(m, &LPGA11);
    let u = usga_gh(m);
    let e: Vec<f64> = u.iter().zip(USGA.iter()).map(|(a, r)| a - r.7).collect();
    let am: Vec<f64> = (0..20).filter(|&i| !USGA[i].0.ends_with("Pro")).map(|i| e[i]).collect();
    let fs = load_fs("fs_shots.csv");
    let f = score_fs(m, &fs);
    println!(
        "  {tag:<18} PGA(훈련) {:.2}/{:.2}/{:.2}  LPGA {:.2}/{:.2}/{:.2}  USGA {:.2} 아마 {:.2}  FS 캐리 {:.2} 높이 {:.2}",
        tr[0], tr[1], tr[2], ho[0], ho[1], ho[2], rmse(&e), rmse(&am), f[0], f[4]
    );
}

fn run<M: Aeros>(base: M, kept: &[Tm], sess: &[usize], cfg: &Cfg, flag: &dyn Fn(&str) -> bool) {
    let n_sess = sess.iter().max().unwrap() + 1;
    let all: Vec<&Tm> = kept.iter().collect();
    // 세션 단위 보류
    let mut pooled: Vec<[f64; 7]> = Vec::new();
    if !flag("--nocv") {
        for s in 0..n_sess {
            let train: Vec<&Tm> = kept.iter().zip(sess).filter(|(_, q)| **q != s).map(|(r, _)| r).collect();
            let test: Vec<&Tm> = kept.iter().zip(sess).filter(|(_, q)| **q == s).map(|(r, _)| r).collect();
            let t0 = std::time::Instant::now();
            let m = fit_surf(&base, &train, cfg);
            let es: Vec<[f64; 7]> = test.iter().filter_map(|r| tm_error(&m, r)).collect();
            print_score(&format!("세션 {s} 빼기 ({}샷)", test.len()), &moments(&es));
            println!("    ({:.1} s)", t0.elapsed().as_secs_f64());
            pooled.extend(es);
        }
        print_score("세션 보류 합계", &moments(&pooled));
    }
    // 전체로 맞춘 모형과 투어·USGA·FS 점검 (실측 묶음을 맞춤에 쓴 뒤의 보류 자료 성적)
    let full = fit_surf(&base, &all, cfg);
    let es: Vec<[f64; 7]> = all.iter().filter_map(|r| tm_error(&full, r)).collect();
    print_score("전체 맞춤 (표본 안)", &moments(&es));
    tour_check("전체 맞춤", &full);
    if flag("--dump") {
        full.dump();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let opt = |k: &str| args.iter().find_map(|a| a.strip_prefix(k).map(|x| x.to_string()));
    let csv = std::env::var("TM_CSV").expect("TM_CSV=...data.csv 를 지정");
    let (mut rows, _, _) = load(&csv, false);
    rows.retain(|r| r.ball == "Premium" || r.ball == "Limited Distance Soft");
    let trim: f64 = opt("--trim=").and_then(|x| x.parse().ok()).unwrap_or(25.0);
    let cfg = Cfg {
        wtm: opt("--wtm=").and_then(|x| x.parse().ok()).unwrap_or(1.0),
        lam: opt("--lam=").and_then(|x| x.parse().ok()).unwrap_or(0.5),
        smooth: opt("--smooth=").and_then(|x| x.parse().ok()).unwrap_or(0.5),
        use_spd: !flag("--no-spd"),
    };
    let free_b = flag("--beta");

    // 이상치(추적 오류)는 기준 장의 캐리 오차로 한 번 정해 뺀다 (real --trim 과 같다)
    let b0 = Grid::new(false, false, false);
    let kept: Vec<Tm> = rows.iter().filter(|r| tm_error(&b0, r).map(|e| e[0].abs() <= trim).unwrap_or(false)).cloned().collect();
    let sess = sessions(&kept);
    let n_sess = sess.iter().max().unwrap() + 1;
    println!(
        "실측 {}샷, 세션 {n_sess}개 {:?}. 정칙화 lam {} smooth {}, 실측 묶음 무게 {}, 속력·시간·최고점거리 {}, β {}",
        kept.len(),
        (0..n_sess).map(|s| sess.iter().filter(|&&q| q == s).count()).collect::<Vec<_>>(),
        cfg.lam,
        cfg.smooth,
        cfg.wtm,
        if cfg.use_spd { "사용" } else { "미사용" },
        if free_b { "자유" } else { "0.06 고정" }
    );
    let all: Vec<&Tm> = kept.iter().collect();
    let es: Vec<[f64; 7]> = all.iter().filter_map(|r| tm_error(&b0, r)).collect();
    print_score("기준 공유위기12", &moments(&es));
    tour_check("공유위기12", &b0);

    if flag("--win") || flag("--win0") || flag("--wq") || flag("--wq2") {
        // --win0: 창 없이 기준 6개만 같은 자료로 다시 맞춘 대조군, --wq: 창을 항력 위기에 묶은 꼴 (깊이 하나)
        let win = flag("--win") || flag("--wq") || flag("--wq2");
        println!(
            "[양력 창] 기준 6개{}{}",
            if flag("--wq2") { " + 위기에 묶은 양력 창(a) + 항력 봉우리(bd)" } else if flag("--wq") { " + 위기에 묶은 창(a)" } else if win { " + 창(a, r1, s)" } else { " (창 없음, 대조군)" },
            if free_b { " + β" } else { "" }
        );
        let mut w = Win::new(win, free_b, flag("--wq") || flag("--wq2"));
        w.free_bd = flag("--wq2");
        run(w, &kept, &sess, &cfg, &flag);
    } else if flag("--sep") {
        println!("[분리형] f(Re) 7마디 + d c l{}{}", if flag("--glift") { " + g(Re) 7마디" } else { "" }, if free_b { " + β" } else { "" });
        run(Sep::new(flag("--glift"), free_b), &kept, &sess, &cfg, &flag);
    } else {
        let (free_d, free_l) = (!flag("--lift"), !flag("--drag"));
        println!("[이중선형 표] C_D 표 {free_d}, C_L 표 {free_l}");
        run(Grid::new(free_d, free_l, free_b), &kept, &sess, &cfg, &flag);
    }
}
