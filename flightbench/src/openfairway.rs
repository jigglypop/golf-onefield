//! OpenFairway(https://github.com/digitalhand/openfairway, MIT) 비행 물리의 러스트 이식 — 비교 기준용.
//!
//! 이식 범위: addons/openfairway/physics 의 FlightAerodynamicsModel·FlightProfile(기본값 55개)·
//! BallPhysics(공기력·스핀 감쇠·120 Hz 반암시 오일러)·BallPhysicsProfile(발사 영역별 배율 표 34칸)·
//! ShotRegimeKey·PhysicsAdapter(시작 높이 0.02 m, 75°F 해수면 공기, 첫 접지에서 캐리 기록, 보간 없음).
//! 원본은 C# float 이지만 여기서는 f64 로 계산한다(120 Hz 오일러 오차가 반올림보다 훨씬 크다).
//! 지면(바운스·구름)은 이식하지 않았다.

pub const MASS: f64 = 0.04592623;
pub const RADIUS: f64 = 0.021335;
const AREA: f64 = std::f64::consts::PI * RADIUS * RADIUS;
const SPIN_DECAY_TAU: f64 = 5.0;
pub const HZ: f64 = 120.0;
const MPS_PER_MPH: f64 = 0.44704;
const RAD_PER_RPM: f64 = 0.10472;
const YD_PER_M: f64 = 1.09361;

#[inline]
fn smooth01(t: f64) -> f64 {
    let c = t.clamp(0.0, 1.0);
    c * c * (3.0 - 2.0 * c)
}
#[inline]
fn sss(v: f64, a: f64, b: f64) -> f64 {
    let r = b - a;
    if r.abs() < 1e-6 {
        return if v >= b { 1.0 } else { 0.0 };
    }
    smooth01((v - a) / r)
}
#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// 75°F, 해수면 (PhysicsAdapter 기본값): (밀도 kg/m^3, 점성 kg/(m s))
pub fn air() -> (f64, f64) {
    let tk: f64 = (75.0 - 32.0) * 5.0 / 9.0 + 273.15;
    let rho = 101325.0 / (287.058 * tk);
    let mu = 1.716e-5 * (tk / 273.15).powf(1.5) * (273.15 + 198.72) / (tk + 198.72);
    (rho, mu)
}

fn cd_re(re: f64) -> f64 {
    if re > 200000.0 {
        return 0.2;
    }
    if re >= 50000.0 {
        return 1.1948 - 0.0000209661 * re + 1.42472e-10 * re * re - 3.14383e-16 * re * re * re;
    }
    if re <= 30000.0 {
        return 0.38;
    }
    lerp(0.38, 0.4632, sss(re, 30000.0, 50000.0))
}

fn cl_max(s: f64) -> f64 {
    if s <= 0.35 {
        0.268
    } else if s >= 0.50 {
        0.32
    } else {
        lerp(0.268, 0.32, sss(s, 0.35, 0.50))
    }
}

fn cl_high_re(s: f64) -> f64 {
    // HighReSpinGain = HighReMidSpinGain = 16 이라 이득 구간 보간은 늘 16
    let g = 16.0;
    cl_max(s) * s * g / (1.0 + s * g)
}

fn atten_high(s: f64, cl: f64, m1: f64, m2: f64) -> f64 {
    cl * (1.0 - m1 * sss(s, 0.45, 0.55)) * (1.0 - m2 * sss(s, 0.58, 0.85))
}

fn cl_at(idx: usize, s: f64) -> f64 {
    match idx {
        0 => 0.0472121 + 2.84795 * s - 23.4342 * s * s + 45.4849 * s * s * s,
        1 => 0.320524 - 4.7032 * s + 14.0613 * s * s,
        2 => 0.266667 - 4.0 * s + 13.3333 * s * s,
        3 => 0.0496189 + 0.00211396 * s + 2.34201 * s * s,
        _ => cl_high_re(s),
    }
}

fn cl_re(re: f64, s: f64) -> f64 {
    let s = s.max(0.0);
    if s <= 0.0 {
        return 0.0;
    }
    let m = cl_max(s);
    if re < 50000.0 {
        if re <= 30000.0 {
            return 0.0;
        }
        let t = smooth01((re - 30000.0) / 20000.0);
        return atten_high(s, cl_at(0, s).clamp(0.0, m) * t, 0.10, 0.06);
    }
    if re >= 75000.0 {
        return atten_high(s, cl_high_re(s).clamp(0.0, m), 0.09, 0.10);
    }
    let rv = [50000.0, 60000.0, 65000.0, 70000.0, 75000.0];
    let hi = rv.iter().position(|&r| re <= r).unwrap_or(4);
    let lo = hi.saturating_sub(1);
    let (cl_lo, cl_hi) = (cl_at(lo, s).max(0.0), cl_at(hi, s).max(0.0));
    let w = if rv[hi] != rv[lo] { (re - rv[lo]) / (rv[hi] - rv[lo]) } else { 0.0 };
    atten_high(s, lerp(cl_lo, cl_hi, w).clamp(0.0, m), 0.10, 0.06)
}

fn spin_drag_mult(s: f64, re: f64) -> f64 {
    if s <= 0.0 {
        return 1.0;
    }
    let relief = sss(s, 0.30, 0.48) * (1.0 - sss(re, 90000.0, 105000.0));
    let mut cap = lerp(1.20, 1.03, relief);
    cap += 0.25 * sss(s, 0.33, 0.50);
    cap = lerp(cap, 1.21, sss(s, 0.57, 0.77));
    (1.0 + 4.0 * s * s).min(cap)
}

fn low_launch_lift(vla: f64, s: f64, re: f64) -> f64 {
    let lf = sss(9.5 - vla, 0.0, 9.5 - 6.5);
    if lf <= 0.0 {
        return 1.0;
    }
    let rf = sss(re, 85000.0, 110000.0);
    if rf <= 0.0 {
        return 1.0;
    }
    let sf = 1.0 - sss(s, 0.18, 0.22);
    if sf <= 0.0 {
        return 1.0;
    }
    lerp(1.0, 1.08, lf * rf * sf)
}

fn high_launch_drag(vla: f64, s: f64) -> f64 {
    let lf = sss(vla, 24.5, 31.5);
    if lf <= 0.0 {
        return 1.0;
    }
    let sf = sss(s, 0.50, 0.70);
    if sf <= 0.0 {
        return 1.0;
    }
    lerp(1.0, 1.24, lf * sf)
}

fn mid_spin_boost(s: f64) -> f64 {
    let t = sss(s, 0.17, 0.31);
    1.0 + 0.45 * t * (1.0 - t) * 4.0
}

/// 발사 영역 배율 (BallPhysicsProfile.BuildDefaultRegimeOverrides) → (항력 배율, 양력 배율)
pub fn regime(mph: f64, vla: f64, rpm: f64) -> (f64, f64) {
    let fam = if mph < 60.0 { "C" } else if mph > 110.0 && vla < 18.0 { "D" } else if vla > 30.0 { "W" } else { "I" };
    let sp = if mph < 60.0 { "S0" } else if mph < 72.0 { "S1a" } else if mph < 85.0 { "S1b" } else if mph < 105.0 { "S2" } else if mph < 120.0 { "S3" } else { "S4" };
    let la = if vla < 10.0 { "V0" } else if vla < 18.0 { "V1" } else if vla < 25.0 { "V2" } else if vla < 33.0 { "V3" } else { "V4" };
    let sn = if rpm < 2500.0 { "P0" } else if rpm < 4000.0 { "P1" } else if rpm < 5500.0 { "P2" } else if rpm < 7500.0 { "P3" } else { "P4" };
    const T: [(&str, f64, f64); 34] = [
        ("C-S0", 0.70, 1.20), ("C-S0-V1-P0", 0.55, 1.15), ("C-S0-V4-P3", 0.65, 1.25),
        ("I-S1a-V0-P1", 0.94, 1.04), ("I-S1a-V2-P1", 0.80, 1.14), ("I-S1a-V2-P2", 0.82, 1.13), ("I-S1a-V2-P3", 0.82, 1.12),
        ("I-S1a-V3-P2", 0.79, 1.14), ("I-S1a-V3-P3", 0.94, 1.04), ("I-S1a-V1-P2", 0.92, 1.05),
        ("I-S1b-V0-P0", 0.97, 1.02), ("I-S1b-V2-P2", 0.94, 1.03), ("I-S1b-V2-P3", 0.97, 1.01), ("I-S1b-V3-P2", 0.88, 1.06),
        ("I-S1b-V3-P3", 0.97, 1.02), ("I-S1b-V1-P2", 0.98, 1.01),
        ("W-S1a-V3-P3", 0.83, 1.10),
        ("I-S3-V2-P3", 1.11, 0.94), ("I-S3-V1-P2", 1.04, 0.98), ("I-S2-V2-P3", 1.05, 1.0), ("I-S2-V2-P4", 1.06, 0.95),
        ("I-S2-V1-P2", 1.03, 0.99), ("I-S2-V0-P2", 1.06, 0.96), ("I-S2-V1-P0", 0.96, 1.03), ("I-S2-V1-P1", 0.97, 1.02),
        ("D-S3-V1", 1.04, 0.99), ("D-S4-V0-P1", 1.03, 0.98), ("D-S4-V0-P2", 1.09, 0.94), ("D-S4-V1-P0", 0.98, 1.02),
        ("D-S4-V1-P1", 1.04, 1.0), ("D-S4-V1-P2", 1.04, 1.0),
        ("W-S2-V3-P4", 1.06, 0.97), ("_", 1.0, 1.0), ("_", 1.0, 1.0),
    ];
    for key in [format!("{fam}-{sp}-{la}-{sn}"), format!("{fam}-{sp}-{la}"), format!("{fam}-{sp}"), fam.to_string()] {
        if let Some(r) = T.iter().find(|r| r.0 == key) {
            return (r.1, r.2);
        }
    }
    (1.0, 1.0)
}

#[derive(Clone, Copy)]
pub struct Shot {
    pub mph: f64,
    pub vla: f64,
    pub hla: f64,
    pub back: f64,
    pub side: f64,
    pub total: f64,
}

#[derive(Clone, Copy)]
struct Ctx {
    rho: f64,
    mu: f64,
    drag_scale: f64,
    lift_scale: f64,
    vla: f64,
}

#[inline]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline]
fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// 가속도와 스핀 변화율
#[inline]
fn deriv(c: &Ctx, v: [f64; 3], om: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let mut a = [0.0, -9.81, 0.0];
    let sp = norm(v);
    if sp >= 0.5 {
        let omn = norm(om);
        let s = omn * RADIUS / sp;
        let re = c.rho * sp * RADIUS * 2.0 / c.mu;
        let cd = cd_re(re) * spin_drag_mult(s, re) * c.drag_scale * high_launch_drag(c.vla, s);
        let cl = cl_re(re, s) * c.lift_scale * low_launch_lift(c.vla, s, re) * mid_spin_boost(s);
        let k = 0.5 * c.rho * AREA / MASS;
        for i in 0..3 {
            a[i] -= k * cd * v[i] * sp;
        }
        if omn > 0.1 {
            let x = cross(om, v);
            for i in 0..3 {
                a[i] += k * cl * x[i] * sp / omn;
            }
        }
    }
    (a, om.map(|w| -w / SPIN_DECAY_TAU))
}

fn setup(s: &Shot, mul: (f64, f64)) -> (Ctx, [f64; 3], [f64; 3], [f64; 3]) {
    let (rho, mu) = air();
    let (rd, rl) = regime(s.mph, s.vla, s.total);
    let c = Ctx { rho, mu, drag_scale: 1.01 * rd * mul.0, lift_scale: rl * mul.1, vla: s.vla };
    let (th, ph) = (s.vla.to_radians(), -s.hla.to_radians());
    let v0 = s.mph * MPS_PER_MPH;
    let v = [v0 * th.cos() * ph.cos(), v0 * th.sin(), -v0 * th.cos() * ph.sin()];
    let (b, sd) = (s.back * RAD_PER_RPM, s.side * RAD_PER_RPM);
    let om = [b * ph.sin(), -sd, b * ph.cos()];
    let fl = (v[0] * v[0] + v[2] * v[2]).sqrt();
    let dir = [v[0] / fl, 0.0, v[2] / fl];
    (c, v, om, dir)
}

/// 원본 그대로: 120 Hz 반암시 오일러, 시작 높이 0.02 m, 첫 접지 걸음의 위치를 캐리로 (보간 없음).
/// 반환 (캐리 yd, 최고 높이 yd, 착지각 deg)
pub fn fly_original(s: &Shot, hz: f64) -> [f64; 3] {
    fly_original_mul(s, hz, (1.0, 1.0))
}

pub fn fly_original_mul(s: &Shot, hz: f64, mul: (f64, f64)) -> [f64; 3] {
    let (c, mut v, mut om, dir) = setup(s, mul);
    let dt = 1.0 / hz;
    let mut p = [0.0, 0.02, 0.0];
    let mut apex = p[1];
    for _ in 0..(12.0 * hz) as usize {
        let (a, dw) = deriv(&c, v, om);
        for i in 0..3 {
            v[i] += a[i] * dt;
            om[i] += dw[i] * dt;
        }
        for i in 0..3 {
            p[i] += v[i] * dt;
        }
        apex = apex.max(p[1]);
        if p[1] <= 0.0 {
            let carry = (p[0] * dir[0] + p[2] * dir[2]).max(0.0);
            let vt = (v[0] * v[0] + v[2] * v[2]).sqrt();
            return [carry * YD_PER_M, apex * YD_PER_M, v[1].abs().atan2(vt.max(1e-4)).to_degrees()];
        }
    }
    [f64::NAN; 3]
}

/// 같은 힘 법칙을 RK4 와 작은 걸음, 착지 선형 보간으로 푼 수렴해 (적분 오차를 떼어 보기 위한 기준)
pub fn fly_converged(s: &Shot, dt: f64) -> [f64; 3] {
    let (c, mut v, mut om, dir) = setup(s, (1.0, 1.0));
    let mut p = [0.0, 0.02, 0.0];
    let mut apex = p[1];
    let add = |a: [f64; 3], b: [f64; 3], h: f64| [a[0] + h * b[0], a[1] + h * b[1], a[2] + h * b[2]];
    for _ in 0..(12.0 / dt) as usize {
        let (a1, w1) = deriv(&c, v, om);
        let (v2, o2) = (add(v, a1, 0.5 * dt), add(om, w1, 0.5 * dt));
        let (a2, w2) = deriv(&c, v2, o2);
        let (v3, o3) = (add(v, a2, 0.5 * dt), add(om, w2, 0.5 * dt));
        let (a3, w3) = deriv(&c, v3, o3);
        let (v4, o4) = (add(v, a3, dt), add(om, w3, dt));
        let (a4, w4) = deriv(&c, v4, o4);
        let np: [f64; 3] = std::array::from_fn(|i| p[i] + dt / 6.0 * (v[i] + 2.0 * v2[i] + 2.0 * v3[i] + v4[i]));
        let nv: [f64; 3] = std::array::from_fn(|i| v[i] + dt / 6.0 * (a1[i] + 2.0 * a2[i] + 2.0 * a3[i] + a4[i]));
        let no: [f64; 3] = std::array::from_fn(|i| om[i] + dt / 6.0 * (w1[i] + 2.0 * w2[i] + 2.0 * w3[i] + w4[i]));
        apex = apex.max(np[1]);
        if np[1] <= 0.0 {
            let t = p[1] / (p[1] - np[1]);
            let q: [f64; 3] = std::array::from_fn(|i| p[i] + t * (np[i] - p[i]));
            let u: [f64; 3] = std::array::from_fn(|i| v[i] + t * (nv[i] - v[i]));
            let carry = (q[0] * dir[0] + q[2] * dir[2]).max(0.0);
            let vt = (u[0] * u[0] + u[2] * u[2]).sqrt();
            return [carry * YD_PER_M, apex * YD_PER_M, u[1].abs().atan2(vt).to_degrees()];
        }
        p = np;
        v = nv;
        om = no;
    }
    [f64::NAN; 3]
}

/// 원본 회귀 시험(LmCarryWindowRegressionTests) 입력과 비교 창: (이름, 샷, 창 하한, 창 상한), 창은 ±2 yd 를 더해 쓴다.
pub fn regression_cases() -> Vec<(&'static str, Shot, f64, f64)> {
    let s = |mph, vla, hla, back, side, total| Shot { mph, vla, hla, back, side, total };
    vec![
        ("Driver 1", s(124.0, 9.4, -9.5, 2107.923, -973.19, 2322.0), 158.0, 168.0),
        ("Driver 2", s(124.6, 13.2, -6.8, 3971.26, -423.52, 3994.0), 186.2, 186.7),
        ("Driver 3", s(119.1, 16.0, -8.9, 4868.89, -806.27, 4935.0), 172.8, 174.1),
        ("Driver 4", s(119.0, 15.5, -10.2, 4412.0, -608.12, 4454.0), 174.9, 175.6),
        ("5 Iron", s(102.1, 17.4, 1.5, 5266.0, 1151.0, 5391.0), 138.1, 139.0),
        ("Wood 1", s(124.2, 6.7, -8.1, 4512.0, 378.0, 4528.0), 165.5, 170.1),
        ("Wood 2", s(118.8, 14.5, -3.3, 2968.0, 586.0, 3026.0), 175.6, 179.1),
        ("Wedge 1", s(66.4, 23.2, -1.4, 6399.0, 793.0, 6449.0), 70.6, 72.0),
        ("Wedge 2", s(54.7, 26.8, 1.6, 4951.0, 494.0, 4976.0), 52.2, 52.4),
        ("Wood Low", s(114.46167398452758, 6.949296951293945, -0.6264948844909668, 1931.9213097756137, -47.75123587127419, 1932.5113525390625), 122.2, 122.2),
        ("Approach Mid", s(93.9, 22.9, -2.6, 5375.0, 0.0, 5375.0), 125.8, 125.8),
        ("Checked", s(75.054704, 38.503028869628906, 1.3455228805541992, 10500.780354572425, 699.266167598911, 10700.8457), 77.9, 77.9),
        ("Flop", s(68.054704, 45.0, 0.4455228805541992, 12000.0, 140.266167598911, 12000.0), 61.8, 61.8),
        ("P Wedge 1", s(82.27544053993225, 26.45549964904785, -3.1465682983398438, 4912.299856410009, 580.3159525795915, 4946.458984375), 104.6, 105.1),
        ("Wedge Shot 1", s(48.66421414337158, 28.955089569091797, -1.2652084827423096, 5576.983697818348, 1096.2641574167517, 5683.70849609375), 42.7, 42.9),
        ("Wedge Shot 2", s(51.825726327514644, 37.02534866333008, 2.3746323585510254, 5581.20893514589, 891.7118971017386, 5651.99462890625), 49.0, 49.7),
    ]
}

/// 시험대용: 순수 백스핀 샷 (방위각 0, 사이드 스핀 0)
#[derive(Clone)]
pub struct OpenFairway {
    /// None 이면 수렴해(RK4 dt 1e-3 s), Some(hz) 면 원본 방식
    pub hz: Option<f64>,
    /// 전역 항력·양력 배율 (원본 1, 1). 우리 자료로 다시 맞출 때만 바꾼다.
    pub mul: (f64, f64),
}
impl crate::harness::Flight for OpenFairway {
    fn obs(&self, mph: f64, deg: f64, rpm: f64) -> [f64; 3] {
        let s = Shot { mph, vla: deg, hla: 0.0, back: rpm, side: 0.0, total: rpm };
        match self.hz {
            Some(hz) => fly_original_mul(&s, hz, self.mul),
            None => fly_converged(&s, 1e-3),
        }
    }
}
impl crate::harness::Tunable for OpenFairway {
    fn get(&self) -> Vec<f64> {
        vec![self.mul.0, self.mul.1]
    }
    fn set(&mut self, p: &[f64]) {
        self.mul = (p[0], p[1]);
    }
    fn bounds(&self) -> (Vec<f64>, Vec<f64>) {
        (vec![0.3, 0.3], vec![3.0, 3.0])
    }
}

impl crate::harness::Flight3 for OpenFairway {
    fn obs3(&self, mph: f64, deg: f64, rpm: f64, axis_deg: f64) -> [f64; 2] {
        let a = axis_deg.to_radians();
        let s = Shot { mph, vla: deg, hla: 0.0, back: rpm * a.cos(), side: rpm * a.sin(), total: rpm };
        let o = match self.hz {
            Some(hz) => fly_original_mul(&s, hz, self.mul),
            None => fly_converged(&s, 1e-3),
        };
        [o[0], o[1]]
    }
}
