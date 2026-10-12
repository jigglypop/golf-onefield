//! golf-core: 골프공 장 비행 엔진의 이식 가능한 코어 (네이티브와 wasm32 공용).
//!
//! 장 = 공유 포화 + 항력 위기 (`flightbench/src/engine.rs` 의 `SharedRe` 와 같은 식):
//!
//!   |u| C_D = (v + e/(1+(|u|/u0)^12)) |u| + d·ũ,   |u| C_L = c·ũ,   ũ = w|u|/(|u| + l w)
//!   du/dt = -(|u| C_D) u + (|u| C_L) (ω̂ × u) - ŷ,   dw/dt = -β w |u|,   ω̂ = (0, sin a, cos a)
//!
//! 무차원: 길이 L = 2m/(ρA) (ρ = 1.2 에서 53.53 m), 속도 U = √(gL) (22.92 m/s), 시간 L/U (2.336 s).
//! 좌표: x 앞(목표 방향), y 위, z 오른쪽. 축 기울기 a > 0 이면 공이 왼쪽으로 휜다(훅).
//! 적분: 걸음 재매개 dτ = |u|^{1/2} dt 의 RK4. 착지점과 최고점은 3차 에르미트 보간 + 뉴턴(이분법 대체).
//!
//! 환경 (`Env`): 공기 밀도 ρ, 점성 μ, 바람 w⃗.
//!  - 방정식은 ρ 에 길이 단위 L ∝ 1/ρ 로만 들어간다 (무차원 식은 그대로, 환산만 바뀐다).
//!  - 항력 위기는 레이놀즈 수 Re = ρ|v_rel|D/μ 로 정의돼 있으므로 `Coef::u0` 가 Re 비 (∝ √ρ/μ) 만큼 움직인다.
//!  - 바람은 공기에 대한 상대속도 u_rel = u - w⃗/U 로 공기역학 항(항력, 양력 방향 ω̂×u_rel, S = w/|u_rel|, Re, 스핀 감쇠)에만 들어간다.
//!    중력과 운동학은 지면 기준 속도를 쓴다. 지면 y = 0 은 그대로다.
//!
//! 정밀도는 제네릭 `Real` (f32 / f64) 로 고른다. wasm 내보내기는 f64 가 기본이다.
//! 4레인 f32 묶음 커널(`lanes`)은 wasm simd128 (있으면) 또는 이식 가능한 배열 판본으로 같은 식을 4샷씩 나란히 돌린다.
//! 선택 사항 `Opts` (기본값 = 지금까지의 동작): `min_steps` 짧은 비행의 최소 걸음 수, `compensated` f32 의 카한 보상 합.

use core::ops::{Add, Div, Mul, Neg, Sub};
use wasm_bindgen::prelude::*;

pub mod ground;
pub mod lanes;
pub mod oracle;

pub const L_M: f64 = 53.531_575_815_310_39; // 2m/(ρA), ρ = 1.2
pub const U_MS: f64 = 22.916_037_151_920_374; // √(gL)
pub const T_S: f64 = L_M / U_MS; // 무차원 시간 1 의 초
pub const MPH: f64 = 0.44704;
pub const YARD: f64 = 0.9144;
pub const R_BALL: f64 = 0.04267 / 2.0;
pub const BETA: f64 = 0.06;
/// 기본 환경의 공기 밀도 kg/m³ 와 점성 Pa·s (`L_M`, `U_MS`, `RE_PER_U` 가 이 값 기준)
pub const RHO0: f64 = 1.2;
pub const MU0: f64 = 1.81e-5;
/// 레이놀즈 수 = |u| × RE_PER_U (flightbench 와 같은 값. 기본 환경 기준)
pub const RE_PER_U: f64 = U_MS * 0.04267 * 1.2 / 1.81e-5;
/// 기본 τ 걸음 (f64 에서 샷당 약 55 걸음, 캐리 최대 오차 7e-6 yd. 표는 `cargo run --release --bin bench`).
/// 기준·시험용이다. 앱이 쓰는 걸음은 `H_APP` + `MIN_STEPS_APP` (아래).
pub const H_DEFAULT: f64 = 0.05;
/// 앱(web/golf-game, web/glasses-hud)의 샷 하나·클럽표 계산이 쓰는 τ 걸음. f64 단일 샷에서 `MIN_STEPS_APP` 와 함께 쓰면
/// 샷당 평균 21.2 걸음(τ 0.05 의 약 55 걸음보다 2.6배 적음)에 `web3d/test/web_refs.csv` 52샷 대비 최대 오차가
/// 캐리 4.7e-4 yd · 편차 1.4e-4 yd · 높이 5.2e-4 yd · 착지각 6e-4° · 시간 4e-5 s 로 web3d 의 허용(0.05 yd, 0.05°, 0.01 s)의 약 1% 이하다
/// (시험 `app_step_matches_web_refs`, 수치와 해석은 RESULTS.md §11).
/// 걸음이 줄어 경로 점이 성기므로 소비자는 경로를 3차 에르미트로 이어야 한다 (점마다 위치와 속도를 준다: `PATH_STRIDE`).
/// `H_DEFAULT` 를 바꾸지 않은 이유: `default_step()` 의 의미(기준 정밀도)와 그에 의존하는 벤치·시험을 그대로 둔다.
pub const H_APP: f64 = 0.137;
/// 앱의 최소 걸음 수 (`Opts::min_steps`). 3–5걸음에 끝나는 낮고 느린 샷의 오차를 줄인다 (RESULTS.md §6.1).
pub const MIN_STEPS_APP: f64 = 6.0;
/// 경로 점 하나의 f64 개수: [t s, x m, y m, z m, vx m/s, vy m/s, vz m/s] (속도는 지면 기준 dX/dt)
pub const PATH_STRIDE: usize = 7;

// ------------------------------------------------------------------ 정밀도

/// 사칙·제곱근만 있는 최소 인터페이스. 스칼라(f32/f64)와 4레인 벡터가 같은 식 코드를 공유한다.
pub trait Lane: Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> {
    /// 모든 레인에 같은 값
    fn lit(x: f64) -> Self;
    fn sqrt(self) -> Self;
    fn max(self, o: Self) -> Self;
}

/// f32 와 f64 를 같은 코드로 돌리기 위한 최소 인터페이스 (비교와 f64 변환이 되는 스칼라)
pub trait Real: Lane + PartialOrd {
    fn is_nan(self) -> bool;
    const NAME: &'static str;
    fn to_f64(self) -> f64;
}

impl Lane for f64 {
    #[inline(always)]
    fn lit(x: f64) -> Self {
        x
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline(always)]
    fn max(self, o: Self) -> Self {
        f64::max(self, o)
    }
}

impl Real for f64 {
    #[inline(always)]
    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }
    const NAME: &'static str = "f64";
    #[inline(always)]
    fn to_f64(self) -> f64 {
        self
    }
}

impl Lane for f32 {
    #[inline(always)]
    fn lit(x: f64) -> Self {
        x as f32
    }
    #[inline(always)]
    fn sqrt(self) -> Self {
        f32::sqrt(self)
    }
    #[inline(always)]
    fn max(self, o: Self) -> Self {
        f32::max(self, o)
    }
}

impl Real for f32 {
    #[inline(always)]
    fn is_nan(self) -> bool {
        f32::is_nan(self)
    }
    const NAME: &'static str = "f32";
    #[inline(always)]
    fn to_f64(self) -> f64 {
        self as f64
    }
}

// ------------------------------------------------------------------ 장과 입력

/// 장의 매개변수 (물리 단위가 없는 무차원 값). 기본 환경(ρ = 1.2, μ = 1.81e-5) 기준이다.
#[derive(Clone, Copy, Debug)]
pub struct Coef {
    pub v: f64,
    pub e: f64,
    pub u0: f64,
    pub d: f64,
    pub c: f64,
    pub l: f64,
    pub beta: f64,
    /// 양력 위기 깊이: C_L = c r (1 − 4 a q (1 − q)), q = 1/(1 + (|u|/u0)^12). 0 이면 없음 (공유위기12 와 비트까지 같다)
    pub a: f64,
}

impl Coef {
    /// 공유위기12 장 (위기 지수 n = 12), PGA 12행 세 양으로만 맞춘 값. `flightbench` 의 `engine::default_field()` 와 같은 값이다.
    pub const SHARED_RE: Coef = Coef {
        v: 0.1383,
        e: 0.2540,
        u0: 74003.0320 / RE_PER_U,
        d: 1.4838,
        c: 2.8062,
        l: 6.5014,
        beta: BETA,
        a: 0.0,
    };

    /// 공유위기12L: 공유위기12 + 양력 위기 (매개변수 1개). 투어 PGA 12행 + 공개 TrackMan 드라이버 767샷(실측)으로 함께 맞춘 값
    /// (`flightbench/src/bin/aeroflex.rs --wq`). 실측 세션 보류: 착지 속력 편향 −5.0 → −1.1 mph, 비행 시간 편향 +0.11 → −0.03 s, 캐리 6.25 → 5.91 yd.
    /// 맞바꿈: 옆 편차 5.62 → 6.03 yd, FS 캐리 3.33 → 3.50, LPGA 캐리 2.78 → 2.87. 실측 자료를 맞춤에 썼으므로 기본 장으로는 두지 않는다.
    pub const SHARED_RE_L: Coef = Coef {
        v: 0.1354,
        e: 0.1881,
        u0: 68077.0 / RE_PER_U,
        d: 1.4080,
        c: 2.6035,
        l: 5.5338,
        beta: BETA,
        a: 0.413,
    };
}

/// 샷 입력 (단위: 볼스피드 mph, 발사각 도, 스핀 rpm, 축 기울기 도)
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub mph: f64,
    pub vla_deg: f64,
    pub rpm: f64,
    pub axis_deg: f64,
}

/// 착지 결과. 길이는 m, 시간은 s, 각은 도 (아래로 내려가는 각을 양수로). 모두 지면 기준.
#[derive(Clone, Copy, Debug, Default)]
pub struct Landing {
    pub carry_m: f64,
    pub lateral_m: f64,
    pub apex_m: f64,
    pub land_deg: f64,
    pub time_s: f64,
    pub steps: u32,
    /// 착지 순간 지면 기준 속도 [x, y, z], m/s
    pub vel_ms: [f64; 3],
}

// ------------------------------------------------------------------ 환경

/// 환경: 바람 [x 앞, y 위, z 오른쪽] m/s, 공기 밀도 kg/m³, 공기 점성 Pa·s.
/// 바람은 "공기가 움직이는 속도"다: x > 0 이면 뒷바람(순풍), z > 0 이면 왼쪽에서 불어 공을 오른쪽으로 민다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    pub wind: [f64; 3],
    pub rho: f64,
    pub mu: f64,
}

impl Default for Env {
    /// 지금까지의 동작: ρ = 1.2, μ = 1.81e-5, 무풍
    fn default() -> Env {
        Env { wind: [0.0; 3], rho: RHO0, mu: MU0 }
    }
}

/// ISA 표준 대기 기압, Pa. 대류권 공식 p = 101325 (1 - 0.0065 h/288.15)^5.25588.
/// 고도는 -500..11000 m 로 자른다 (골프장 범위를 한참 넘는다).
pub fn isa_pressure(altitude_m: f64) -> f64 {
    let h = altitude_m.clamp(-500.0, 11_000.0);
    101_325.0 * (1.0 - 0.0065 * h / 288.15).powf(5.255_88)
}

/// 서덜랜드 공식 공기 점성, Pa·s (T 는 켈빈). μ = 1.458e-6 T^1.5 / (T + 110.4)
pub fn sutherland_mu(t_kelvin: f64) -> f64 {
    1.458e-6 * t_kelvin * t_kelvin.sqrt() / (t_kelvin + 110.4)
}

impl Env {
    /// 기온(°C)·고도(m)·바람(앞뒤, 좌우 m/s)으로 환경을 만든다.
    /// 기압은 ISA 고도 공식, 밀도는 이상기체 ρ = p/(R_공기 T) (습도 무시, R = 287.058 J/kg/K), 점성은 서덜랜드.
    /// `wind_forward_ms` > 0 이면 뒷바람, `wind_right_ms` > 0 이면 공을 오른쪽으로 미는 바람(왼쪽에서 오른쪽으로 분다).
    /// 참고: 기본 환경(ρ = 1.2, μ = 1.81e-5)은 해수면 약 20 °C 와 거의 같다 (`from_conditions(20, 0, ..)` = 1.204, 1.8135e-5).
    pub fn from_conditions(temp_c: f64, altitude_m: f64, wind_forward_ms: f64, wind_right_ms: f64) -> Env {
        let t = temp_c + 273.15;
        Env { wind: [wind_forward_ms, 0.0, wind_right_ms], rho: isa_pressure(altitude_m) / (287.058 * t), mu: sutherland_mu(t) }
    }

    /// 이 환경의 무차원 환산값
    pub fn units(&self) -> Units {
        // L = 2m/(ρA) = L_M · (ρ0/ρ), U = √(gL),  Re/|u| = ρUD/μ ∝ √ρ/μ.  기본 환경에서는 인자가 정확히 1 이다.
        let s = RHO0 / self.rho;
        let l_m = L_M * s;
        let u_ms = U_MS * s.sqrt();
        Units {
            l_m,
            u_ms,
            t_s: l_m / u_ms,
            re_scale: (self.rho / RHO0).sqrt() * (MU0 / self.mu),
            wind: [self.wind[0] / u_ms, self.wind[1] / u_ms, self.wind[2] / u_ms],
        }
    }
}

/// 환경에서 나오는 환산값: 길이·속도·시간 단위와 무차원 바람
#[derive(Clone, Copy, Debug)]
pub struct Units {
    pub l_m: f64,
    pub u_ms: f64,
    pub t_s: f64,
    /// 이 환경의 (Re/|u|) / 기본 환경의 (Re/|u|). 항력 위기 속도는 u0 / re_scale 로 움직인다.
    pub re_scale: f64,
    /// 무차원 바람 w⃗/U
    pub wind: [f64; 3],
}

/// 초기 조건 (무차원): 지면 기준 속도, 스핀 w = Rω/U, 축 (sin a, cos a)
#[derive(Clone, Copy, Debug)]
pub struct Init {
    pub u: [f64; 3],
    pub w: f64,
    pub oy: f64,
    pub oz: f64,
}

impl Init {
    pub fn from_shot(shot: &Shot, un: &Units) -> Init {
        let v = shot.mph * MPH;
        let u0 = v / un.u_ms;
        let th0 = shot.vla_deg.to_radians();
        let s0 = R_BALL * shot.rpm * core::f64::consts::TAU / 60.0 / v;
        let a = shot.axis_deg.to_radians();
        Init { u: [u0 * th0.cos(), u0 * th0.sin(), 0.0], w: s0 * u0, oy: a.sin(), oz: a.cos() }
    }
}

/// 초기 조건 (무차원, 기본 환경): (|u0|, 발사각 rad, S0 = Rω/v)
pub fn dimless(shot: &Shot) -> (f64, f64, f64) {
    let v = shot.mph * MPH;
    (v / U_MS, shot.vla_deg.to_radians(), R_BALL * shot.rpm * core::f64::consts::TAU / 60.0 / v)
}

// ------------------------------------------------------------------ 적분

/// 상태: 시간, 위치, 지면 기준 속도, 스핀 (모두 무차원)
#[derive(Clone, Copy)]
pub(crate) struct S<T> {
    pub t: T,
    pub x: T,
    pub y: T,
    pub z: T,
    pub ux: T,
    pub uy: T,
    pub uz: T,
    pub w: T,
}

/// 걸음 재매개의 도함수 dX/dτ = (dX/dt) · g,  g = |u_rel|^{-1/2}.  (g = dt/dτ, v = u g 는 위치의 도함수)
#[derive(Clone, Copy)]
pub(crate) struct K<T> {
    pub g: T,
    pub vx: T,
    pub vy: T,
    pub vz: T,
    pub ax: T,
    pub ay: T,
    pub az: T,
    pub aw: T,
}

/// 계수 (제네릭 정밀도). 바람은 무차원, `iu02` = 1/u0² (환경의 레이놀즈 비로 옮긴 u0)
#[derive(Clone, Copy)]
pub(crate) struct P<T> {
    pub v: T,
    pub e: T,
    pub iu02: T,
    pub d: T,
    pub c: T,
    pub l: T,
    pub beta: T,
    /// 4a (양력 위기 깊이 × 4)
    pub a4: T,
    pub wx: T,
    pub wy: T,
    pub wz: T,
}

impl<T: Lane> P<T> {
    pub(crate) fn new(c: &Coef, un: &Units) -> Self {
        let u0 = c.u0 / un.re_scale; // 항력 위기는 Re 로 정의: 환경이 바뀌면 속도 위치가 Re 비만큼 움직인다
        P {
            v: T::lit(c.v),
            e: T::lit(c.e),
            iu02: T::lit(1.0 / (u0 * u0)),
            d: T::lit(c.d),
            c: T::lit(c.c),
            l: T::lit(c.l),
            beta: T::lit(c.beta),
            a4: T::lit(4.0 * c.a),
            wx: T::lit(un.wind[0]),
            wy: T::lit(un.wind[1]),
            wz: T::lit(un.wind[2]),
        }
    }
}

static FIELD: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// 쓸 장을 고른다 (웹 내보내기·배치·`fly_*` 가 읽는다). 0 = 공유위기12 (기본, 투어표로만 맞춤), 1 = 공유위기12L (양력 위기, 실측으로 함께 맞춤)
#[wasm_bindgen]
pub fn set_field(id: u32) {
    FIELD.store(id, core::sync::atomic::Ordering::Relaxed);
}

/// 지금 고른 장 번호
#[wasm_bindgen]
pub fn field() -> u32 {
    FIELD.load(core::sync::atomic::Ordering::Relaxed)
}

/// 지금 고른 장의 계수
pub(crate) fn coef() -> &'static Coef {
    if field() == 1 {
        &Coef::SHARED_RE_L
    } else {
        &Coef::SHARED_RE
    }
}

/// 항력 위기 꺾임의 가파름: 위기 인자는 1/(1 + (|u|/u0)^CRISIS_N). 12 = 공유위기12(기본), 4 = 이전 공유위기.
/// 상수라서 분기 없이 곱 몇 번으로 접힌다.
pub(crate) const CRISIS_N: u32 = 12;

/// (|u|/u0)^CRISIS_N, 입력은 q2 = (|u|/u0)² (곱만 쓴다)
#[inline(always)]
pub(crate) fn crisis_pow<T: Lane>(q2: T) -> T {
    match CRISIS_N {
        4 => q2 * q2,
        12 => {
            let q3 = q2 * q2 * q2;
            q3 * q3
        }
        _ => unreachable!("CRISIS_N 은 4 나 12"),
    }
}

/// 걸음 재매개의 도함수.  q = |u_rel|², sp = √q.  나눗셈을 하나로 줄이고 임계 경로를 짧게 한다:
///   역수 하나 inv = 1/(den1·den2·sp) (den1 = sp + l w, den2 = 1 + (q/u0²)^(N/2)) 에서 1/den1, 1/den2, 1/sp 를 곱으로 얻는다.
///   ũ = w sp/den1 = w q den2 inv,  e/den2 = e den1 sp inv  →  |u|C_D = v sp + q inv (e den1 + d w den2),  |u|C_L = c w q den2 inv
///   g = sp^{-1/2} = √sp · (1/sp): √sp 는 나눗셈과 나란히 계산돼(sqrt 3개, div 1개) sqrt→div→sqrt 사슬이 sqrt→div 로 짧아진다.
#[inline(always)]
pub(crate) fn rhs<T: Lane, const WIND: bool>(p: &P<T>, oy: T, oz: T, ux: T, uy: T, uz: T, w: T) -> K<T> {
    let one = T::lit(1.0);
    let (rx, ry, rz) = if WIND { (ux - p.wx, uy - p.wy, uz - p.wz) } else { (ux, uy, uz) };
    let mut q = rx * rx + ry * ry + rz * rz;
    if WIND {
        q = q.max(T::lit(1e-12)); // 공기에 대해 멈춘 공(상대속도 0)에서 0 으로 나누지 않게
    }
    let sp = q.sqrt();
    let sq = sp.sqrt();
    let den1 = sp + p.l * w;
    let den2 = one + crisis_pow(q * p.iu02);
    let a = den1 * den2;
    let inv = one / (a * sp);
    let g = sq * (a * inv);
    let kd = p.v * sp + (q * (p.e * den1 + (p.d * w) * den2)) * inv;
    // 양력 위기: 1 − 4a q(1−q), q = 1/den2.  1/den2 = den1 sp inv.  a = 0 이면 인수가 정확히 1 이라 답이 바뀌지 않는다
    let iden2 = (den1 * sp) * inv;
    let kl = ((p.c * w) * (q * den2)) * inv * (one - p.a4 * (den2 - one) * (iden2 * iden2));
    // ω̂ × u_rel,  ω̂ = (0, oy, oz)
    let cx = oy * rz - oz * ry;
    let cy = oz * rx;
    let cz = -(oy * rx);
    K {
        g,
        vx: ux * g,
        vy: uy * g,
        vz: uz * g,
        ax: (kl * cx - kd * rx) * g,
        ay: (kl * cy - kd * ry - one) * g,
        az: (kl * cz - kd * rz) * g,
        aw: -(p.beta * w) * sq, // dw/dτ = -β w sp g = -β w √sp
    }
}

/// RK4 한 걸음 (FSAL: 새 상태와 그 상태의 도함수 k1' 를 돌려준다). hh = h/2, h6 = h/6
#[cfg(test)]
#[inline(always)]
pub(crate) fn rk4<T: Lane, const WIND: bool>(p: &P<T>, oy: T, oz: T, s: &S<T>, k1: &K<T>, h: T, hh: T, h6: T) -> (S<T>, K<T>) {
    let zero = T::lit(0.0);
    let c0 = S { t: zero, x: zero, y: zero, z: zero, ux: zero, uy: zero, uz: zero, w: zero };
    let (n, _, k5) = rk4g::<T, WIND, false, 1>(p, &[oy], &[oz], &[*s], &[c0], &[*k1], &[h], &[hh], &[h6]);
    (n[0], k5[0])
}

/// 누적 합 한 번: COMP 이면 카한 보상 합 (v + inc, 잃은 낮은 자리를 c 에 모아 다음 걸음에 되먹인다)
#[inline(always)]
fn acc<T: Lane, const COMP: bool>(v: T, inc: T, c: T) -> (T, T) {
    if COMP {
        let y = inc - c;
        let t = v + y;
        (t, (t - v) - y)
    } else {
        (v + inc, c)
    }
}

/// RK4 한 걸음을 독립된 G 묶음에 대해 단계별로 번갈아 한다 (비순차 실행이 한 묶음의 지연 사이에 다른 묶음을 끼워 넣게).
/// COMP 이면 상태 누적에 카한 보상 합을 쓴다: f32 에서 누적 반올림(걸음 수 × 반올림)을 거의 없앤다. `cmp` 는 보상 항.
/// 반환: (새 상태, 새 보상 항, 새 상태의 도함수)
#[inline(always)]
pub(crate) fn rk4g<T: Lane, const WIND: bool, const COMP: bool, const G: usize>(
    p: &P<T>,
    oy: &[T; G],
    oz: &[T; G],
    s: &[S<T>; G],
    cmp: &[S<T>; G],
    k1: &[K<T>; G],
    h: &[T; G],
    hh: &[T; G],
    h6: &[T; G],
) -> ([S<T>; G], [S<T>; G], [K<T>; G]) {
    let k2: [K<T>; G] = core::array::from_fn(|g| rhs::<T, WIND>(p, oy[g], oz[g], s[g].ux + hh[g] * k1[g].ax, s[g].uy + hh[g] * k1[g].ay, s[g].uz + hh[g] * k1[g].az, s[g].w + hh[g] * k1[g].aw));
    let k3: [K<T>; G] = core::array::from_fn(|g| rhs::<T, WIND>(p, oy[g], oz[g], s[g].ux + hh[g] * k2[g].ax, s[g].uy + hh[g] * k2[g].ay, s[g].uz + hh[g] * k2[g].az, s[g].w + hh[g] * k2[g].aw));
    let k4: [K<T>; G] = core::array::from_fn(|g| rhs::<T, WIND>(p, oy[g], oz[g], s[g].ux + h[g] * k3[g].ax, s[g].uy + h[g] * k3[g].ay, s[g].uz + h[g] * k3[g].az, s[g].w + h[g] * k3[g].aw));
    let two = T::lit(2.0);
    let comb = |h6: T, a: T, b: T, c: T, d: T| h6 * (a + two * (b + c) + d);
    let nc: [(S<T>, S<T>); G] = core::array::from_fn(|g| {
        let (k1, k2, k3, k4, s, c) = (&k1[g], &k2[g], &k3[g], &k4[g], &s[g], &cmp[g]);
        let h6 = h6[g];
        let (t, ct) = acc::<T, COMP>(s.t, comb(h6, k1.g, k2.g, k3.g, k4.g), c.t);
        let (x, cx) = acc::<T, COMP>(s.x, comb(h6, k1.vx, k2.vx, k3.vx, k4.vx), c.x);
        let (y, cy) = acc::<T, COMP>(s.y, comb(h6, k1.vy, k2.vy, k3.vy, k4.vy), c.y);
        let (z, cz) = acc::<T, COMP>(s.z, comb(h6, k1.vz, k2.vz, k3.vz, k4.vz), c.z);
        let (ux, cux) = acc::<T, COMP>(s.ux, comb(h6, k1.ax, k2.ax, k3.ax, k4.ax), c.ux);
        let (uy, cuy) = acc::<T, COMP>(s.uy, comb(h6, k1.ay, k2.ay, k3.ay, k4.ay), c.uy);
        let (uz, cuz) = acc::<T, COMP>(s.uz, comb(h6, k1.az, k2.az, k3.az, k4.az), c.uz);
        let (w, cw) = acc::<T, COMP>(s.w, comb(h6, k1.aw, k2.aw, k3.aw, k4.aw), c.w);
        (S { t, x, y, z, ux, uy, uz, w }, S { t: ct, x: cx, y: cy, z: cz, ux: cux, uy: cuy, uz: cuz, w: cw })
    });
    let n: [S<T>; G] = core::array::from_fn(|g| nc[g].0);
    let cn: [S<T>; G] = core::array::from_fn(|g| nc[g].1);
    let k5: [K<T>; G] = core::array::from_fn(|g| rhs::<T, WIND>(p, oy[g], oz[g], n[g].ux, n[g].uy, n[g].uz, n[g].w));
    (n, cn, k5)
}

// ------------------------------------------------------------------ 에르미트 보간과 뿌리 (모두 f64)

/// 3차 에르미트를 단항식 계수로: H(θ) = c0 + c1 θ + c2 θ² + c3 θ³  (끝값 y0, y1 과 기울기 m0, m1 = (dX/dτ)·h)
#[inline(always)]
pub(crate) fn hcoef(y0: f64, m0: f64, y1: f64, m1: f64) -> [f64; 4] {
    let d = y1 - y0;
    [y0, m0, 3.0 * d - 2.0 * m0 - m1, m0 + m1 - 2.0 * d]
}

#[inline(always)]
pub(crate) fn hval(c: &[f64; 4], th: f64) -> f64 {
    ((c[3] * th + c[2]) * th + c[1]) * th + c[0]
}

/// H(0) > 0 ≥ H(1) 인 3차식의 뿌리 θ ∈ (0, 1] (H(0) = 0 이면 H/θ 의 뿌리). 선형 보간에서 출발하는 뉴턴, 구간을 벗어나면 이분.
/// 2차 수렴이라 보통 4–5회로 1e-16 에 닿는다 (옛 판본의 56회 이분법을 대체).
#[inline(always)]
pub(crate) fn hroot(c: &[f64; 4]) -> f64 {
    // 지면에서 막 출발한 첫 걸음(H(0) = 0)에서 끝나는 아주 짧은 비행: θ = 0 은 자명한 뿌리이므로 H/θ 의 뿌리를 찾는다.
    let c = if c[0] == 0.0 { &[c[1], c[2], c[3], 0.0] } else { c };
    if !(c[0] > 0.0) {
        return 0.0;
    }
    let y1 = c[0] + c[1] + c[2] + c[3];
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    let mut th = c[0] / (c[0] - y1);
    for _ in 0..60 {
        let f = ((c[3] * th + c[2]) * th + c[1]) * th + c[0];
        if f == 0.0 {
            break;
        }
        if f > 0.0 {
            lo = th;
        } else {
            hi = th;
        }
        let df = (3.0 * c[3] * th + 2.0 * c[2]) * th + c[1];
        let mut nt = th - f / df;
        // 구간 끝에 정확히 닿는 것은 수렴이다 (반올림으로 th 와 같아진 경우). 벗어나거나 NaN 이면 이분.
        if !(nt >= lo && nt <= hi) {
            nt = 0.5 * (lo + hi);
        }
        let step = (nt - th).abs();
        th = nt;
        if step < 1e-7 {
            break; // 2차 수렴: 이 걸음이 1e-7 이면 남은 오차는 ~1e-14 (캐리로는 1e-11 m 아래)
        }
    }
    th
}

/// 정점: uy 에르미트의 뿌리에서 y 에르미트 값 (무차원 높이)
#[inline(always)]
pub(crate) fn apex_height<T: Real>(s: &S<T>, k: &K<T>, s1: &S<T>, k1: &K<T>, h: f64) -> f64 {
    let th = hroot(&hcoef(s.uy.to_f64(), k.ay.to_f64() * h, s1.uy.to_f64(), k1.ay.to_f64() * h));
    hval(&hcoef(s.y.to_f64(), k.vy.to_f64() * h, s1.y.to_f64(), k1.vy.to_f64() * h), th)
}

/// 착지 순간의 무차원 값
#[derive(Clone, Copy, Debug)]
pub(crate) struct Touch {
    pub t: f64,
    pub x: f64,
    pub z: f64,
    pub ux: f64,
    pub uy: f64,
    pub uz: f64,
}

/// 착지: y 에르미트의 뿌리 θ 에서 나머지 성분을 에르미트로
#[inline(always)]
pub(crate) fn touchdown<T: Real>(s: &S<T>, k: &K<T>, s1: &S<T>, k1: &K<T>, h: f64) -> Touch {
    let th = hroot(&hcoef(s.y.to_f64(), k.vy.to_f64() * h, s1.y.to_f64(), k1.vy.to_f64() * h));
    let at = |a0: T, d0: T, a1: T, d1: T| hval(&hcoef(a0.to_f64(), d0.to_f64() * h, a1.to_f64(), d1.to_f64() * h), th);
    Touch {
        t: at(s.t, k.g, s1.t, k1.g),
        x: at(s.x, k.vx, s1.x, k1.vx),
        z: at(s.z, k.vz, s1.z, k1.vz),
        ux: at(s.ux, k.ax, s1.ux, k1.ax),
        uy: at(s.uy, k.ay, s1.uy, k1.ay),
        uz: at(s.uz, k.az, s1.uz, k1.az),
    }
}

/// 착지 순간 값을 결과 구조체에 옮긴다 (물리 단위, 각)
#[inline(always)]
pub(crate) fn fill_landing(out: &mut Landing, td: &Touch, un: &Units) {
    out.carry_m = td.x * un.l_m;
    out.lateral_m = td.z * un.l_m;
    out.land_deg = (-td.uy).atan2(td.ux).to_degrees();
    out.time_s = td.t * un.t_s;
    out.vel_ms = [td.ux * un.u_ms, td.uy * un.u_ms, td.uz * un.u_ms];
}

/// 값이 깨진 샷의 결과: 모두 NaN
pub(crate) fn nan_landing(steps: u32) -> Landing {
    Landing { carry_m: f64::NAN, lateral_m: f64::NAN, apex_m: f64::NAN, land_deg: f64::NAN, time_s: f64::NAN, steps, vel_ms: [f64::NAN; 3] }
}

/// 위로 나가지 않거나 값이 깨진 샷의 결과: 바로 떨어진다(캐리 0) 또는 NaN
pub(crate) fn degenerate(init: &Init, un: &Units) -> Landing {
    if init.u.iter().any(|x| !x.is_finite()) || !init.w.is_finite() {
        return nan_landing(0);
    }
    let mut out = Landing::default();
    out.land_deg = (-init.u[1]).atan2(init.u[0]).to_degrees();
    out.vel_ms = [init.u[0] * un.u_ms, init.u[1] * un.u_ms, init.u[2] * un.u_ms];
    out
}

const MAX_STEPS: u32 = 2_000_000;

fn run<T: Real, const WIND: bool, const COMP: bool>(p: &P<T>, un: &Units, init: &Init, h: f64, mut path: Option<&mut Vec<f64>>) -> Landing {
    if !(h > 0.0 && h.is_finite()) {
        return nan_landing(0); // 걸음이 0 이하면 제자리: 돌려보내지 않고 NaN
    }
    if !(init.u[1] > 0.0) || !init.u[0].is_finite() || !init.w.is_finite() {
        return degenerate(init, un);
    }
    let (oy, oz) = (T::lit(init.oy), T::lit(init.oz));
    let zero = T::lit(0.0);
    let mut s = S { t: zero, x: zero, y: zero, z: zero, ux: T::lit(init.u[0]), uy: T::lit(init.u[1]), uz: T::lit(init.u[2]), w: T::lit(init.w) };
    let mut cmp = S { t: zero, x: zero, y: zero, z: zero, ux: zero, uy: zero, uz: zero, w: zero };
    let hv = T::lit(h);
    let (hh, h6) = (hv * T::lit(0.5), hv / T::lit(6.0));
    let h_eff = hv.to_f64();
    let mut k = rhs::<T, WIND>(p, oy, oz, s.ux, s.uy, s.uz, s.w);
    let mut out = Landing::default();
    let mut apex_done = false;
    // 경로 점 하나를 평평한 f64 배열 뒤에 바로 붙인다 (중간 배열·변환 복사 없음). 속도는 지면 기준 u·U 이고 dX/dt 와 같다.
    let mut push = |t: f64, x: f64, y: f64, z: f64, ux: f64, uy: f64, uz: f64| {
        if let Some(buf) = path.as_deref_mut() {
            buf.extend_from_slice(&[t * un.t_s, x * un.l_m, y * un.l_m, z * un.l_m, ux * un.u_ms, uy * un.u_ms, uz * un.u_ms]);
        }
    };
    let state_pt = |st: &S<T>| (st.t.to_f64(), st.x.to_f64(), st.y.to_f64(), st.z.to_f64(), st.ux.to_f64(), st.uy.to_f64(), st.uz.to_f64());
    let p0 = state_pt(&s);
    push(p0.0, p0.1, p0.2, p0.3, p0.4, p0.5, p0.6);
    for step in 1..=MAX_STEPS {
        let (s1a, c1a, k1a) = rk4g::<T, WIND, COMP, 1>(p, &[oy], &[oz], &[s], &[cmp], &[k], &[hv], &[hh], &[h6]);
        let (s1, k1) = (s1a[0], k1a[0]);
        out.steps = step;
        if !apex_done && s.uy > zero && s1.uy <= zero {
            out.apex_m = apex_height(&s, &k, &s1, &k1, h_eff) * un.l_m;
            apex_done = true;
        }
        if (s.y > zero || step == 1) && s1.y <= zero {
            let td = touchdown(&s, &k, &s1, &k1, h_eff);
            fill_landing(&mut out, &td, un);
            push(td.t, td.x, 0.0, td.z, td.ux, td.uy, td.uz);
            return out;
        }
        if s1.y.is_nan() {
            break; // NaN: 입력이 깨졌다
        }
        let p1 = state_pt(&s1);
        push(p1.0, p1.1, p1.2, p1.3, p1.4, p1.5, p1.6);
        s = s1;
        cmp = c1a[0];
        k = k1;
    }
    nan_landing(out.steps)
}

/// 선택 사항 (기본값 = 지금까지의 동작)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Opts {
    /// 짧은 비행에도 최소 이만큼의 걸음을 보장한다 (0 = 끔). 걸음은 h 와 0.8·τ_추정/min_steps 중 작은 쪽.
    /// τ 0.137 로 3–5 걸음에 끝나는 낮고 느린 샷이 최대 오차(4.4e-4 yd)를 만드는데, 이 규칙은 평균 걸음을 3% 늘려
    /// 최대 오차를 1.8e-4 yd 로 줄인다 (`RESULTS.md`).
    pub min_steps: f64,
    /// 상태 누적에 카한 보상 합을 쓴다 (f32 에서만 의미가 있다). 걸음 수 × 반올림으로 쌓이는 f32 바닥을 거의 없앤다:
    /// τ 0.05 캐리 최대 오차 1.9e-4 → 2.8e-5 yd, 비용은 약 +9% (`RESULTS.md`). f64 에서는 효과가 없다.
    pub compensated: bool,
}

/// 걸음 h 를 이 샷에 맞춘다: 진공 비행 시간 × √속도 로 τ 길이를 어림해 최소 걸음 수를 지킨다.
#[inline(always)]
pub(crate) fn step_for(h: f64, init: &Init, opts: &Opts) -> f64 {
    if opts.min_steps > 0.0 {
        let sp = (init.u[0] * init.u[0] + init.u[1] * init.u[1] + init.u[2] * init.u[2]).sqrt();
        let tau_est = 2.0 * init.u[1] * sp.sqrt();
        let he = 0.8 * tau_est / opts.min_steps;
        if he > 0.0 && he < h {
            return he;
        }
    }
    h
}

/// 샷 하나를 날린다. `path` 가 있으면 걸음마다 [t s, x m, y m, z m] 을 채운다. 기본 환경(무풍, ρ = 1.2).
pub fn fly<T: Real>(c: &Coef, shot: &Shot, h: f64, path: Option<&mut Vec<f64>>) -> Landing {
    fly_with::<T>(c, &Env::default(), shot, h, path)
}

/// 환경(바람·밀도·점성)을 주고 샷 하나를 날린다. 길이·시간·착지각은 지면 기준 물리 단위.
pub fn fly_with<T: Real>(c: &Coef, env: &Env, shot: &Shot, h: f64, path: Option<&mut Vec<f64>>) -> Landing {
    fly_with_opts::<T>(c, env, &Opts::default(), shot, h, path)
}

/// `fly_with` 에 선택 사항(`Opts`)을 준다
pub fn fly_with_opts<T: Real>(c: &Coef, env: &Env, opts: &Opts, shot: &Shot, h: f64, path: Option<&mut Vec<f64>>) -> Landing {
    let un = env.units();
    let p = P::<T>::new(c, &un);
    let init = Init::from_shot(shot, &un);
    fly_init::<T>(&p, &un, env.wind != [0.0; 3], opts.compensated, &init, step_for(h, &init, opts), path)
}

/// 무차원 초기 조건에서 직접 날린다 (시험용: 지면 기준이 아닌 임의 속도 벡터를 줄 수 있다)
pub(crate) fn fly_init<T: Real>(p: &P<T>, un: &Units, wind: bool, comp: bool, init: &Init, h: f64, path: Option<&mut Vec<f64>>) -> Landing {
    match (wind, comp) {
        (false, false) => run::<T, false, false>(p, un, init, h, path),
        (true, false) => run::<T, true, false>(p, un, init, h, path),
        (false, true) => run::<T, false, true>(p, un, init, h, path),
        (true, true) => run::<T, true, true>(p, un, init, h, path),
    }
}

/// 여러 샷을 한 번에 (경로 없음, 샷당 4개: 캐리 m, 편차 m, 최고 높이 m, 착지각 도). `shots` 는 [mph, 발사각, rpm, 축] 이어 붙임.
pub fn batch_scalar<T: Real>(c: &Coef, env: &Env, shots: &[f64], h: f64) -> Vec<f64> {
    batch_scalar_opts::<T>(c, env, &Opts::default(), shots, h)
}

/// `batch_scalar` 에 선택 사항(`Opts`)을 준다
pub fn batch_scalar_opts<T: Real>(c: &Coef, env: &Env, opts: &Opts, shots: &[f64], h: f64) -> Vec<f64> {
    let un = env.units();
    let p = P::<T>::new(c, &un);
    let wind = env.wind != [0.0; 3];
    let mut out = Vec::with_capacity(shots.len() / 4 * 4);
    for q in shots.chunks_exact(4) {
        let init = Init::from_shot(&Shot { mph: q[0], vla_deg: q[1], rpm: q[2], axis_deg: q[3] }, &un);
        let r = fly_init::<T>(&p, &un, wind, opts.compensated, &init, step_for(h, &init, opts), None);
        out.extend_from_slice(&[r.carry_m, r.lateral_m, r.apex_m, r.land_deg]);
    }
    out
}

// ------------------------------------------------------------------ wasm 내보내기

/// 샷 하나의 결과와 걸음별 경로. JS 에서는 `path()` 를 4개씩 끊어 쓴다.
/// (값 하나마다 경계를 건너는 이 객체 API 는 호환용이다. 샷 하나를 한 번의 호출로 받으려면 `fly_flat`.)
#[wasm_bindgen]
pub struct Flight {
    r: Landing,
    /// 경로 원본: `PATH_STRIDE` 개씩 평평하게 (`run` 이 바로 채운다). `path()` 가 호환 꼴로 바꿔 준다.
    path: Vec<f64>,
    /// 드라이버 자료의 입력 상자 안인가 (`ground::launch_in_range`): 총거리를 믿을 만한가
    total_ok: bool,
}

#[wasm_bindgen]
impl Flight {
    #[wasm_bindgen(getter)]
    pub fn carry_yd(&self) -> f64 {
        self.r.carry_m / YARD
    }
    #[wasm_bindgen(getter)]
    pub fn carry_m(&self) -> f64 {
        self.r.carry_m
    }
    #[wasm_bindgen(getter)]
    pub fn apex_m(&self) -> f64 {
        self.r.apex_m
    }
    /// 오른쪽(+)·왼쪽(-) 착지 편차, m (지면 기준)
    #[wasm_bindgen(getter)]
    pub fn lateral_m(&self) -> f64 {
        self.r.lateral_m
    }
    /// 착지각, 도 (아래로 내려가는 각을 양수로)
    #[wasm_bindgen(getter)]
    pub fn land_deg(&self) -> f64 {
        self.r.land_deg
    }
    /// 비행 시간, 초
    #[wasm_bindgen(getter)]
    pub fn time_s(&self) -> f64 {
        self.r.time_s
    }
    /// 걸음 수 (τ 걸음, 보간 점 제외)
    #[wasm_bindgen(getter)]
    pub fn steps(&self) -> u32 {
        self.r.steps
    }
    /// 착지 속력, mph (지면 기준)
    #[wasm_bindgen(getter)]
    pub fn land_mph(&self) -> f64 {
        ground::landing_mph(&self.r)
    }
    /// 롤(굴러가는 거리), yd. TrackMan 기본 표면이 표시하는 값을 따라가는 경험식이다 (`ground` 모듈 설명 참고)
    #[wasm_bindgen(getter)]
    pub fn roll_yd(&self) -> f64 {
        ground::roll_yd(self.r.land_deg, ground::landing_mph(&self.r))
    }
    /// 총거리 = 캐리 + 롤, yd. `total_reliable` 이 false 면 참고용도 못 된다(드라이버가 아닌 발사)
    #[wasm_bindgen(getter)]
    pub fn total_yd(&self) -> f64 {
        ground::total_yd(&self.r)
    }
    /// 총거리를 믿을 만한 발사 조건(드라이버 자료 범위)인가
    #[wasm_bindgen(getter)]
    pub fn total_reliable(&self) -> bool {
        self.total_ok
    }
    /// [t s, x m, y m, z m] 을 점마다 4개씩 펼친 배열. 마지막 점은 착지점
    pub fn path(&self) -> Vec<f32> {
        // 점마다 앞의 4개(t, x, y, z)만 f32 로: 한 번 훑어 새 배열을 만들고 글루가 그것을 JS 로 복사해 해제한다 (예전의 clone 은 없다)
        self.path.chunks_exact(PATH_STRIDE).flat_map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]).collect()
    }
}

fn flight_env<T: Real>(ball_mph: f64, launch_deg: f64, spin_rpm: f64, axis_deg: f64, h: f64, env: &Env) -> Flight {
    let shot = Shot { mph: ball_mph, vla_deg: launch_deg, rpm: spin_rpm, axis_deg };
    let mut path = Vec::with_capacity(PATH_STRIDE * 64);
    let r = fly_with::<T>(coef(), env, &shot, h, Some(&mut path));
    Flight { r, path, total_ok: ground::launch_in_range(ball_mph, launch_deg, spin_rpm) }
}

// ------------------------------------------------------------------ 한 번의 호출로 받는 샷 하나 (경계 비용 줄이기)

/// `fly_flat` 결과 배열의 머리 길이 (f64 개수). 머리 뒤에 경로 점이 `PATH_STRIDE` 개씩 n_points 개 이어진다.
/// 머리 칸: 0 carry_m, 1 lateral_m (오른쪽 +), 2 apex_m, 3 land_deg, 4 time_s, 5 steps, 6 land_mph, 7 roll_yd, 8 total_yd,
/// 9 total_reliable (0 또는 1), 10 n_points, 11 PATH_STRIDE (배열이 자기 배치를 말해 준다: JS 가 어긋남을 바로 알아챈다).
/// 경로 점: [t s, x m, y m, z m, vx m/s, vy m/s, vz m/s]. 마지막 점이 착지점 (y = 0, 속도는 착지 순간).
pub const FLAT_HEAD: usize = 12;

/// 샷 하나를 `out` 한 배열에 담는다: 머리(`FLAT_HEAD`) + 경로. 배열을 새로 만들지 않고 `out` 을 비우고 다시 쓴다.
/// 경로는 `run` 이 머리 바로 뒤에 직접 채우므로 복사가 없다. 기본 환경, f64.
pub fn fly_flat_into(out: &mut Vec<f64>, c: &Coef, env: &Env, opts: &Opts, shot: &Shot, h: f64) -> Landing {
    out.clear();
    out.resize(FLAT_HEAD, 0.0);
    let r = fly_with_opts::<f64>(c, env, opts, shot, h, Some(out));
    let n = (out.len() - FLAT_HEAD) / PATH_STRIDE;
    out[0] = r.carry_m;
    out[1] = r.lateral_m;
    out[2] = r.apex_m;
    out[3] = r.land_deg;
    out[4] = r.time_s;
    out[5] = r.steps as f64;
    out[6] = ground::landing_mph(&r);
    out[7] = ground::roll_yd(r.land_deg, ground::landing_mph(&r));
    out[8] = ground::total_yd(&r);
    out[9] = if ground::launch_in_range(shot.mph, shot.vla_deg, shot.rpm) { 1.0 } else { 0.0 };
    out[10] = n as f64;
    out[11] = PATH_STRIDE as f64;
    r
}

/// 샷 하나를 f64 로 날려 결과 전부를 **한 배열**로 돌려준다 (배열 배치는 `FLAT_HEAD` 설명).
/// `Flight` 객체 API 는 값 하나(getter 11개 + `path()` + `free()`)마다 JS↔wasm 경계를 건너 샷당 15번쯤 넘었다.
/// 이 함수는 호출 한 번 + 글루의 해제 한 번이고 복사는 JS 쪽 한 번(배열 → 새 Float64Array)이다.
/// `min_steps` 는 `Opts::min_steps` (0 = 끔). 앱은 `app_step()`, `app_min_steps()` 를 넘긴다.
/// 걸음이 0 이하·NaN 이면 머리 값이 NaN 이고 경로가 없다 (n_points = 0).
#[wasm_bindgen]
pub fn fly_flat(ball_mph: f64, launch_deg: f64, spin_rpm: f64, axis_deg: f64, h: f64, min_steps: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(FLAT_HEAD + PATH_STRIDE * 40);
    let shot = Shot { mph: ball_mph, vla_deg: launch_deg, rpm: spin_rpm, axis_deg };
    fly_flat_into(&mut out, coef(), &Env::default(), &Opts { min_steps, compensated: false }, &shot, h);
    out
}

/// f64 로 날린다 (기본, 최대 정밀도). 인수: 볼스피드 mph, 발사각 도, 스핀 rpm, 축 기울기 도, τ 걸음
#[wasm_bindgen]
pub fn fly_f64(ball_mph: f64, launch_deg: f64, spin_rpm: f64, axis_deg: f64, h: f64) -> Flight {
    flight_env::<f64>(ball_mph, launch_deg, spin_rpm, axis_deg, h, &Env::default())
}

/// f32 로 날린다 (빠른 판본, 정밀도 비교용)
#[wasm_bindgen]
pub fn fly_f32(ball_mph: f64, launch_deg: f64, spin_rpm: f64, axis_deg: f64, h: f64) -> Flight {
    flight_env::<f32>(ball_mph, launch_deg, spin_rpm, axis_deg, h, &Env::default())
}

/// 환경을 주고 f64 로 날린다. 기온 °C, 고도 m (ISA 기압), 바람: 앞뒤(+ 뒷바람)와 좌우(+ 오른쪽으로 부는 바람) m/s.
/// `fly_env(.., 15.0, 0.0, 0.0, 0.0)` 은 해수면 15 °C 표준 대기 (ρ = 1.225). 기본 환경과는 ρ 가 2% 다르다.
#[wasm_bindgen]
pub fn fly_env(ball_mph: f64, launch_deg: f64, spin_rpm: f64, axis_deg: f64, h: f64, temp_c: f64, altitude_m: f64, wind_forward_ms: f64, wind_right_ms: f64) -> Flight {
    flight_env::<f64>(ball_mph, launch_deg, spin_rpm, axis_deg, h, &Env::from_conditions(temp_c, altitude_m, wind_forward_ms, wind_right_ms))
}

/// 여러 샷을 한 번에 (경로 없음, 샷당 4개: 캐리 m, 편차 m, 최고 높이 m, 착지각 도).
/// `shots` 는 [mph, 발사각, rpm, 축] 을 샷마다 이어 붙인 배열. `single` 이 true 면 f32 로 계산한다.
#[wasm_bindgen]
pub fn batch(shots: &[f64], h: f64, single: bool) -> Vec<f64> {
    batch_with(shots, h, single, &Env::default())
}

/// `batch` 에 환경을 준다 (기온 °C, 고도 m, 앞뒤 바람 m/s, 좌우 바람 m/s)
#[wasm_bindgen]
pub fn batch_env(shots: &[f64], h: f64, single: bool, temp_c: f64, altitude_m: f64, wind_forward_ms: f64, wind_right_ms: f64) -> Vec<f64> {
    batch_with(shots, h, single, &Env::from_conditions(temp_c, altitude_m, wind_forward_ms, wind_right_ms))
}

/// 선택 사항을 주는 `batch` (기본 환경). `min_steps` 0 = 끔, `compensated` false = 끔 → `batch` 와 같다.
/// 권장 (τ 0.137): min_steps 6 → 평균 걸음 +3%, 캐리 최대 오차 4.4e-4 → 1.8e-4 yd. `compensated` 는 f32(single)에서만 의미가 있다.
#[wasm_bindgen]
pub fn batch_opts(shots: &[f64], h: f64, single: bool, min_steps: f64, compensated: bool) -> Vec<f64> {
    batch_with_opts(shots, h, single, &Env::default(), &Opts { min_steps, compensated })
}

/// `batch_opts` + 환경 (기온 °C, 고도 m, 앞뒤 바람 m/s, 좌우 바람 m/s)
#[wasm_bindgen]
pub fn batch_env_opts(shots: &[f64], h: f64, single: bool, min_steps: f64, compensated: bool, temp_c: f64, altitude_m: f64, wind_forward_ms: f64, wind_right_ms: f64) -> Vec<f64> {
    batch_with_opts(shots, h, single, &Env::from_conditions(temp_c, altitude_m, wind_forward_ms, wind_right_ms), &Opts { min_steps, compensated })
}

fn batch_with_opts(shots: &[f64], h: f64, single: bool, env: &Env, opts: &Opts) -> Vec<f64> {
    if single {
        batch_scalar_opts::<f32>(coef(), env, opts, shots, h)
    } else {
        batch_scalar_opts::<f64>(coef(), env, opts, shots, h)
    }
}

fn batch_with(shots: &[f64], h: f64, single: bool, env: &Env) -> Vec<f64> {
    if single {
        batch_scalar::<f32>(coef(), env, shots, h)
    } else {
        batch_scalar::<f64>(coef(), env, shots, h)
    }
}

/// 이 빌드가 진짜 wasm simd128 을 쓰는가 (false 면 `batch_simd*` 는 같은 연산의 스칼라 f32 — 답은 같고 느리다)
#[wasm_bindgen]
pub fn has_simd() -> bool {
    lanes::SIMD
}

/// `batch_simd*` 의 몸통. simd128 이 켜진 wasm 과 네이티브에서는 4레인 커널, SIMD 없는 wasm 에서는 같은 연산의 스칼라 f32
/// (벡터가 없는 곳에서는 4레인 배열 판본이 스칼라보다 느리다: 측정). 두 길은 샷마다 같은 답을 낸다 (시험이 보장).
fn simd_batch(env: &Env, opts: &Opts, shots: &[f64], h: f64) -> Vec<f64> {
    #[cfg(all(target_arch = "wasm32", not(target_feature = "simd128")))]
    {
        batch_scalar_opts::<f32>(coef(), env, opts, shots, h)
    }
    #[cfg(not(all(target_arch = "wasm32", not(target_feature = "simd128"))))]
    {
        lanes::batch_lanes_opts::<lanes::V>(coef(), env, opts, shots, h)
    }
}

/// 4레인 f32 묶음 커널 (wasm 은 simd128). 출력은 `batch(.., single = true)` 와 같은 꼴, 같은 샷에 대해 f32 반올림 수준으로 같다.
#[wasm_bindgen]
pub fn batch_simd(shots: &[f64], h: f64) -> Vec<f64> {
    simd_batch(&Env::default(), &Opts::default(), shots, h)
}

/// `batch_simd` 에 환경을 준다 (기온 °C, 고도 m, 앞뒤 바람 m/s, 좌우 바람 m/s)
#[wasm_bindgen]
pub fn batch_simd_env(shots: &[f64], h: f64, temp_c: f64, altitude_m: f64, wind_forward_ms: f64, wind_right_ms: f64) -> Vec<f64> {
    simd_batch(&Env::from_conditions(temp_c, altitude_m, wind_forward_ms, wind_right_ms), &Opts::default(), shots, h)
}

/// 선택 사항을 주는 `batch_simd` (기본 환경). `batch_opts(.., single = true)` 와 같은 연산이라 같은 답이다.
#[wasm_bindgen]
pub fn batch_simd_opts(shots: &[f64], h: f64, min_steps: f64, compensated: bool) -> Vec<f64> {
    simd_batch(&Env::default(), &Opts { min_steps, compensated }, shots, h)
}

/// `batch_simd_opts` + 환경 (기온 °C, 고도 m, 앞뒤 바람 m/s, 좌우 바람 m/s)
#[wasm_bindgen]
pub fn batch_simd_env_opts(shots: &[f64], h: f64, min_steps: f64, compensated: bool, temp_c: f64, altitude_m: f64, wind_forward_ms: f64, wind_right_ms: f64) -> Vec<f64> {
    simd_batch(&Env::from_conditions(temp_c, altitude_m, wind_forward_ms, wind_right_ms), &Opts { min_steps, compensated }, shots, h)
}

/// 기본 τ 걸음 (빌드 확인용)
#[wasm_bindgen]
pub fn default_step() -> f64 {
    H_DEFAULT
}

/// 앱이 샷 하나·클럽표에 쓰는 τ 걸음 (`H_APP`). JS 가 숫자를 따로 적지 않고 여기서 읽는다.
#[wasm_bindgen]
pub fn app_step() -> f64 {
    H_APP
}

/// 앱이 쓰는 최소 걸음 수 (`MIN_STEPS_APP`)
#[wasm_bindgen]
pub fn app_min_steps() -> f64 {
    MIN_STEPS_APP
}

#[cfg(test)]
mod tests;
