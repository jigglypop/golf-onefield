//! 장 역산 (field inverse): 짧은 관측 창의 공 위치만으로, 장의 닫힌 꼴을 거꾸로 풀어 스핀·발사 조건을 얻는다.
//! 궤적 적분 없이 (2차 맞춤 세 번 + 스칼라 몇 개) 풀고, 끝에 앞으로 한 번만 날린다.
//!
//!   cargo run --profile quick --bin fieldinv [-- 몬테카를로횟수]
//!
//! 방법 (관측 창 가운데 시각 t_c 에서):
//!   1) 축마다 위치를 (t − t_c) 의 2차식으로 최소제곱 → 가운데 위치 p_c, 속도 v_c, 가속도 a_c.
//!      대칭 창에서는 가속도의 시간 변화(저크, t³ 항)가 a_c 를 치우치게 하지 않는다 (t³ 은 t² 와 직교).
//!   2) 공기력 가속도 A = a_c + g ŷ. C_D = −(A·v̂) L/|v|², C_L = |A_⊥| L/|v|² (L = 2m/ρA = 53.5 m), 양력 방향 n̂.
//!   3) 장을 거꾸로: C_L = c r, r = S/(1 + l S) → S = r/(1 − l r). 항력 쪽 r_D = (C_D − v − e q(Re))/d 도 같은 r 을 본다
//!      (한 장의 두 투영). 노이즈 가중으로 합치고, 사전분포(스핀 회귀 ±1500 rpm, 축 0 ± 20°)와 가우스로 합친다.
//!      스핀축 기울기: ω̂ = cos a·n_h + sin a·ŷ 가 n̂ 에 수직 → a = atan2(−n_h·n̂, n̂_y).
//!   4) 가운데 상태(p_c, v_c, 스핀, 축)에서 앞으로 한 번 날려 착지.
//!   5) (C) 짧은 창에서 정보를 덜 흘리도록, 궤적을 가운데 시각의 테일러 3차(가속도·저크를 장의 닫힌 꼴로)로 쓰고
//!      8 미지수를 가우스-뉴턴 3번 (궤적 적분 없음, 장 계산 약 80번).
//! 비교: spininv 의 LM (8 매개변수, 수치 야코비안, 시작점 3개, 반복 60, 궤적 적분 약 300–500번).
//!
//!   cargo run --profile quick --bin fieldinv -- 60 [--cam behind] [--sig 5]     # 몬테카를로 비교
//!   ... fieldinv -- --synth-csv 가짜.csv --shot driver|7i|wedge --win 0.2      # 가짜 프레임 CSV
//!   ... fieldinv -- --csv 프레임.csv [--compare]                                # 실제 프레임 (spininv --csv 와 같은 꼴, '# cam' 원근 포함)
//!
//! 결과 (2026-10-13, Ryzen 5 7500F 한 코어, 240 fps, σ 5 mm, 깊이 ×5, 60회. RMS: 스핀 rpm / 캐리 yd, 역산 시간)
//!                         B 장 역산(대수)        C 닫힌 꼴 GN             E LM (spininv 방식)
//!   옆 카메라 드라이버 0.1 s   967 / 9.32   2.6µs     594 / 5.68  21.7µs     592 / 5.66  5.04ms
//!             드라이버 0.2 s   133 / 2.32   2.8µs     127 / 2.29  36.3µs     126 / 2.30  8.55ms
//!             드라이버 0.3 s    48 / 0.93   3.2µs      48 / 0.85  52.5µs      48 / 0.85 10.95ms
//!             7번      0.2 s   686 / 1.91   2.9µs     558 / 1.38  36.4µs     558 / 1.39  9.26ms
//!             웨지     0.3 s   716 / 0.39   3.3µs     747 / 0.42  51.9µs     725 / 0.41 11.56ms
//!   뒤 카메라 드라이버 0.2 s   183 / 1.07   2.9µs     149 / 1.00  38.0µs     148 / 1.00  6.54ms
//!             7번      0.3 s   561 / 0.89   3.4µs     313 / 0.50  55.1µs     314 / 0.50  9.92ms
//!   → C 는 모든 창에서 LM 과 같은 답(차이 1 % 안팎)을 90–270배 적은 계산으로 낸다(0.1 s 이상 창 180–270배). B 는 드라이버 0.2 s 이상에서 LM 과 비슷(약 3000배).
//!     0.05 s(실내 3 m)는 어느 방법이든 스핀 오차 약 1200 rpm (사전분포로 돌아감): 계산이 아니라 정보의 한계다.
//!   가짜 CSV (드라이버 0.2 s): C 2642 rpm·271.2 yd 79µs, spininv --csv 2640 rpm·271.4 yd 9.6 ms (참 2686·270.0).

use flightbench::engine::{default_field, Aero, Rng, SharedRe, BETA, U_MS};
use flightbench::linalg::solve;
use flightbench::lm::{lm, Damping, LmCfg, Step};
use flightbench::util::{prior_rpm, project};
use std::time::Instant;

const G: f64 = 9.81;
const L: f64 = U_MS * U_MS / G; // 53.53 m
const TU: f64 = U_MS / G; // 무차원 시간 1 = 2.336 s
const RB: f64 = 0.021335; // 공 반지름 m (spininv 와 같은 값)
const PRIOR_SD: f64 = 1500.0;
const AXIS_SD: f64 = 20.0;

/// 무차원 도함수: s = [x y z ux uy uz w] (위치 L 단위), om = 스핀축 단위벡터
#[inline]
fn deriv(ae: &SharedRe, om: &[f64; 3], s: &[f64; 7]) -> [f64; 7] {
    let sp = (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt();
    let (kd, kl) = ae.kk(sp, s[6]);
    let c = [om[1] * s[5] - om[2] * s[4], om[2] * s[3] - om[0] * s[5], om[0] * s[4] - om[1] * s[3]];
    [s[3], s[4], s[5], -kd * s[3] + kl * c[0], -kd * s[4] + kl * c[1] - 1.0, -kd * s[5] + kl * c[2], -ae.beta() * s[6] * sp]
}

#[inline]
fn rk4(ae: &SharedRe, om: &[f64; 3], s: &[f64; 7], h: f64) -> [f64; 7] {
    let k1 = deriv(ae, om, s);
    let a: [f64; 7] = std::array::from_fn(|i| s[i] + 0.5 * h * k1[i]);
    let k2 = deriv(ae, om, &a);
    let b: [f64; 7] = std::array::from_fn(|i| s[i] + 0.5 * h * k2[i]);
    let k3 = deriv(ae, om, &b);
    let c: [f64; 7] = std::array::from_fn(|i| s[i] + h * k3[i]);
    let k4 = deriv(ae, om, &c);
    std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]))
}

/// 발사 매개변수 p = [x0 y0 z0 (m), 속도 m/s, 발사각 rad, 방위각 rad, rpm, 축 rad] → (상태, 스핀축)
fn launch_state(p: &[f64; 8]) -> ([f64; 7], [f64; 3]) {
    let (th, ph) = (p[4], p[5]);
    let dir = [th.cos() * ph.cos(), th.sin(), th.cos() * ph.sin()];
    let nrm = [-ph.sin(), 0.0, ph.cos()];
    let om = [p[7].cos() * nrm[0], p[7].sin(), p[7].cos() * nrm[2]];
    let v = p[3] / U_MS;
    let w = RB * p[6] * std::f64::consts::TAU / 60.0 / U_MS;
    ([p[0] / L, p[1] / L, p[2] / L, v * dir[0], v * dir[1], v * dir[2], w], om)
}

/// 프레임 시각들의 위치 (m). spininv::sample 과 같은 적분 (프레임마다 RK4 4걸음)
fn sample(ae: &SharedRe, p: &[f64; 8], n_frames: usize, fdt: f64) -> Vec<[f64; 3]> {
    let (mut s, om) = launch_state(p);
    let sub = 4;
    let h = fdt / TU / sub as f64;
    let mut out = Vec::with_capacity(n_frames);
    for k in 0..n_frames {
        if k > 0 {
            for _ in 0..sub {
                s = rk4(ae, &om, &s, h);
            }
        }
        out.push([s[0] * L, s[1] * L, s[2] * L]);
    }
    out
}

/// 상태에서 착지까지 (무차원 걸음 0.002, 착지는 에르미트 보간). 반환: 착지 (x, z) yd
fn land(ae: &SharedRe, s0: [f64; 7], om: [f64; 3]) -> (f64, f64) {
    let h = 0.002;
    let mut s = s0;
    for _ in 0..20000 {
        let n = rk4(ae, &om, &s, h);
        if s[1] >= 0.0 && n[1] < 0.0 {
            let (y0, y1, m0, m1) = (s[1], n[1], s[4] * h, n[4] * h);
            let mut t = y0 / (y0 - y1);
            for _ in 0..4 {
                let (t2, t3) = (t * t, t * t * t);
                let f = (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * m1;
                let df = (6.0 * t2 - 6.0 * t) * y0 + (3.0 * t2 - 4.0 * t + 1.0) * m0 + (-6.0 * t2 + 6.0 * t) * y1 + (3.0 * t2 - 2.0 * t) * m1;
                t = (t - f / df).clamp(0.0, 1.0);
            }
            let (t2, t3) = (t * t, t * t * t);
            let hh = [2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2];
            let x = hh[0] * s[0] + hh[1] * s[3] * h + hh[2] * n[0] + hh[3] * n[3] * h;
            let z = hh[0] * s[2] + hh[1] * s[5] * h + hh[2] * n[2] + hh[3] * n[5] * h;
            return (x * L / 0.9144, z * L / 0.9144);
        }
        s = n;
    }
    (f64::NAN, f64::NAN)
}

// ------------------------------------------------------------------ 기준: spininv 의 LM (같은 잔차·사전분포·시작점)

fn resid(ae: &SharedRe, p: &[f64; 8], obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, prior: f64) -> Vec<f64> {
    let m = sample(ae, p, obs.len(), fdt);
    let mut r = Vec::with_capacity(obs.len() * 3 + 2);
    for (a, b) in m.iter().zip(obs) {
        let a = project(*a);
        for c in 0..3 {
            r.push(if b[c].is_finite() { (a[c] - b[c]) / sig[c] } else { 0.0 });
        }
    }
    r.push((p[6] - prior) / PRIOR_SD);
    r.push(p[7].to_degrees() / AXIS_SD);
    r
}

fn fit_lm(ae: &SharedRe, p0: [f64; 8], obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, prior: f64, max_iter: usize) -> ([f64; 8], f64, usize) {
    let step = [1e-4, 1e-4, 1e-4, 1e-3, 1e-5, 1e-5, 1.0, 1e-4];
    let lo = [-1.0, -1.0, -1.0, 5.0, -0.2, -1.0, 0.0, -1.2];
    let hi = [1.0, 1.0, 1.0, 100.0, 1.2, 1.0, 15000.0, 1.2];
    let cfg = LmCfg { lo: &lo, hi: &hi, step: Step::Fixed(&step), damping: Damping::Mul, max_iter, tries: 12, mu_min: 1e-10, stop_rel: -1.0, nonfinite_cost: None };
    let mut res = |q: &[f64]| resid(ae, &<[f64; 8]>::try_from(q).unwrap(), obs, sig, fdt, prior);
    let o = lm(&cfg, &p0, &mut res, None);
    (<[f64; 8]>::try_from(o.p.as_slice()).unwrap(), o.cost, o.evals)
}

// ------------------------------------------------------------------ 장 역산

/// 가운데 시각 기준 2차 맞춤 (모든 유한 프레임). 반환 (t_c, 축별 [위치, 속도, 가속도], 축별 가속도 분산 계수 (M⁻¹)_22·4, 속도 분산 계수 (M⁻¹)_11)
fn quad_center(obs: &[[f64; 3]], fdt: f64) -> (f64, [[f64; 3]; 3], f64, f64) {
    let ks: Vec<usize> = (0..obs.len()).filter(|&k| obs[k][0].is_finite()).collect();
    let tc = (ks[0] + ks[ks.len() - 1]) as f64 * fdt / 2.0;
    let mut m = [[0.0; 3]; 3];
    let mut rhs = [[0.0; 3]; 3];
    for &k in &ks {
        let t = k as f64 * fdt - tc;
        let b = [1.0, t, t * t];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += b[i] * b[j];
            }
            for c in 0..3 {
                rhs[c][i] += b[i] * obs[k][c];
            }
        }
    }
    let solve3 = |r: [f64; 3]| -> [f64; 3] {
        let a: Vec<Vec<f64>> = (0..3).map(|i| vec![m[i][0], m[i][1], m[i][2], r[i]]).collect();
        let s = solve(a);
        [s[0], s[1], s[2]]
    };
    let mut co = [[0.0; 3]; 3];
    for c in 0..3 {
        let s = solve3(rhs[c]);
        co[c] = [s[0], s[1], 2.0 * s[2]];
    }
    let inv1 = solve3([0.0, 1.0, 0.0])[1];
    let inv2 = solve3([0.0, 0.0, 1.0])[2];
    (tc, co, 4.0 * inv2, inv1)
}

struct Inv {
    p: [f64; 8],   // 발사 시점으로 되돌린 매개변수 (짧은 LM 의 시작점, 보고용)
    s: [f64; 7],   // 가운데 시각 상태 (무차원)
    om: [f64; 3],  // 스핀축
}

/// 장 역산. use_drag: 항력 투영도 r 추정에 쓰는가
fn field_inverse(ae: &SharedRe, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, use_drag: bool) -> Inv {
    let (tc, co, ka, _kv) = quad_center(obs, fdt);
    let pc = [co[0][0], co[1][0], co[2][0]];
    let vc = [co[0][1], co[1][1], co[2][1]];
    let ac = [co[0][2], co[1][2], co[2][2]];
    let sa2 = [sig[0] * sig[0] * ka, sig[1] * sig[1] * ka, sig[2] * sig[2] * ka]; // 가속도 분산 (m/s²)²
    let sp = (vc[0] * vc[0] + vc[1] * vc[1] + vc[2] * vc[2]).sqrt();
    let vh = [vc[0] / sp, vc[1] / sp, vc[2] / sp];
    let aa = [ac[0], ac[1] + G, ac[2]];
    let along = aa[0] * vh[0] + aa[1] * vh[1] + aa[2] * vh[2];
    let ap = [aa[0] - along * vh[0], aa[1] - along * vh[1], aa[2] - along * vh[2]];
    let apn = (ap[0] * ap[0] + ap[1] * ap[1] + ap[2] * ap[2]).sqrt().max(1e-9);
    let nh = [ap[0] / apn, ap[1] / apn, ap[2] / apn];
    let k = L / (sp * sp); // 가속도 → 계수
    let cl = apn * k;
    let cd = -along * k;
    let sd_cl = (nh[0] * nh[0] * sa2[0] + nh[1] * nh[1] * sa2[1] + nh[2] * nh[2] * sa2[2]).sqrt() * k;
    let sd_cd = (vh[0] * vh[0] * sa2[0] + vh[1] * vh[1] * sa2[1] + vh[2] * vh[2] * sa2[2]).sqrt() * k;
    // 발사 시점 속도 (방위각·사전분포용): v(0) = v_c − a_c t_c
    let v0 = [vc[0] - ac[0] * tc, vc[1] - ac[1] * tc, vc[2] - ac[2] * tc];
    let s0 = (v0[0] * v0[0] + v0[1] * v0[1] + v0[2] * v0[2]).sqrt();
    let (th0, ph0) = ((v0[1] / s0).asin(), v0[2].atan2(v0[0]));
    let prior = prior_rpm(s0 / 0.44704, th0.to_degrees());
    // 장을 거꾸로: r 두 투영 + 사전분포 (r 공간에서 가우스)
    let u = sp / U_MS;
    let mut num = (cl / ae.c) / (sd_cl / ae.c).powi(2);
    let mut den = 1.0 / (sd_cl / ae.c).powi(2);
    if use_drag {
        let q = { let x = (u / ae.u0).powi(2); 1.0 / (1.0 + x * x * x * x * x * x) };
        let rd = (cd - ae.v - ae.e * q) / ae.d;
        num += rd / (sd_cd / ae.d).powi(2);
        den += 1.0 / (sd_cd / ae.d).powi(2);
    }
    let sp_prior = prior * std::f64::consts::TAU / 60.0 * RB / sp; // S
    let ssd_prior = PRIOR_SD * std::f64::consts::TAU / 60.0 * RB / sp;
    let rp = sp_prior / (1.0 + ae.l * sp_prior);
    let rsd = ssd_prior / (1.0 + ae.l * sp_prior).powi(2);
    num += rp / (rsd * rsd);
    den += 1.0 / (rsd * rsd);
    let r = (num / den).clamp(0.0, 0.98 / ae.l);
    let s_ratio = r / (1.0 - ae.l * r);
    let sd_r = (1.0 / den).sqrt();
    let sd_s = sd_r / (1.0 - ae.l * r).powi(2);
    let rpm_c = s_ratio * sp / RB * 60.0 / std::f64::consts::TAU;
    // 축: ω̂ = cos a·n_h + sin a·ŷ ⟂ n̂ (n_h = 발사 방위에 수직인 수평 단위벡터)
    let nrm = [-ph0.sin(), 0.0, ph0.cos()];
    let a_meas = (-(nrm[0] * nh[0] + nrm[2] * nh[2])).atan2(nh[1]);
    let sd_lat = (nrm[0] * nrm[0] * sa2[0] + nrm[2] * nrm[2] * sa2[2]).sqrt() / apn; // rad
    let wa = 1.0 / (sd_lat * sd_lat);
    let wp = 1.0 / (AXIS_SD.to_radians().powi(2));
    let a = (a_meas * wa) / (wa + wp);
    let om = [a.cos() * nrm[0], a.sin(), a.cos() * nrm[2]];
    // 가운데 상태
    let w_c = s_ratio * u;
    let s = [pc[0] / L, pc[1] / L, pc[2] / L, vc[0] / U_MS, vc[1] / U_MS, vc[2] / U_MS, w_c];
    // 발사 시점으로 되돌린 매개변수 (스핀 감쇠 dw/dt' = −β w |u| 를 창 절반만큼 되돌림)
    let rpm0 = rpm_c * (BETA * u * tc / TU).exp();
    let p0 = [pc[0] - vc[0] * tc + 0.5 * ac[0] * tc * tc, pc[1] - vc[1] * tc + 0.5 * ac[1] * tc * tc, pc[2] - vc[2] * tc + 0.5 * ac[2] * tc * tc];
    let p = [p0[0], p0[1], p0[2], s0, th0, ph0, rpm0, a];
    let sd_rpm = sd_s * sp / RB * 60.0 / std::f64::consts::TAU;
    let _ = (rpm_c, sd_rpm);
    Inv { p, s, om }
}

/// 장 역산의 상태는 가운데 시각 t_c 에 있으므로 착지는 그 상태에서 바로 날린다
fn carry_from_inv(ae: &SharedRe, inv: &Inv) -> (f64, f64) {
    land(ae, inv.s, inv.om)
}

// ------------------------------------------------------------------ 닫힌 꼴 궤적 맞춤 (적분 없음)
//
// 짧은 창에서는 궤적을 가운데 시각의 테일러 3차로 쓴다: p(τ) = p_c + v_c τ + ½ A τ² + ⅙ J τ³.
// A = 장의 가속도(속도·스핀·축의 닫힌 꼴), J = dA/dt = (∂A/∂v)·A + (∂A/∂ω)·ω̇ (유한 차분 두 번).
// 미지수 8개 [p_c, v_c, ω, a] 를 가우스-뉴턴으로 맞춘다. 한 번 평가 = 장 계산 3번 + 프레임 수만큼의 다항식.
// 2차 맞춤(축마다 따로 9개)과 달리 가속도가 장의 꼴(항력·양력이 같은 r, 중력 고정)에 묶여 정보가 덜 샌다.

/// SI 가속도: 속도 v (m/s), 스핀 ω (rad/s), 축 기울기 a, 방위 φ
#[inline]
fn accel(ae: &SharedRe, v: [f64; 3], omega: f64, a: f64, phi: f64) -> [f64; 3] {
    let uv = [v[0] / U_MS, v[1] / U_MS, v[2] / U_MS];
    let u = (uv[0] * uv[0] + uv[1] * uv[1] + uv[2] * uv[2]).sqrt();
    let (kd, kl) = ae.kk(u, (omega * RB / U_MS).max(0.0));
    let om = [a.cos() * -phi.sin(), a.sin(), a.cos() * phi.cos()];
    let c = [om[1] * uv[2] - om[2] * uv[1], om[2] * uv[0] - om[0] * uv[2], om[0] * uv[1] - om[1] * uv[0]];
    [G * (-kd * uv[0] + kl * c[0]), G * (-kd * uv[1] + kl * c[1] - 1.0), G * (-kd * uv[2] + kl * c[2])]
}

/// x = [pcx pcy pcz vcx vcy vcz ω a] → 프레임 위치 (테일러 3차), 방위 φ 는 v_c 에서
fn taylor_pos(ae: &SharedRe, x: &[f64; 8], ks: &[usize], fdt: f64, tc: f64, out: &mut Vec<[f64; 3]>) {
    let v = [x[3], x[4], x[5]];
    let phi = v[2].atan2(v[0]);
    let a0 = accel(ae, v, x[6], x[7], phi);
    let sp = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let wdot = -BETA * x[6] * sp / L; // dω/dt
    let h = 1e-3;
    let ap = accel(ae, [v[0] + h * a0[0], v[1] + h * a0[1], v[2] + h * a0[2]], x[6] + h * wdot, x[7], phi);
    let am = accel(ae, [v[0] - h * a0[0], v[1] - h * a0[1], v[2] - h * a0[2]], x[6] - h * wdot, x[7], phi);
    let j = [(ap[0] - am[0]) / (2.0 * h), (ap[1] - am[1]) / (2.0 * h), (ap[2] - am[2]) / (2.0 * h)];
    out.clear();
    for &k in ks {
        let t = k as f64 * fdt - tc;
        let (t2, t3) = (0.5 * t * t, t * t * t / 6.0);
        out.push([x[0] + v[0] * t + a0[0] * t2 + j[0] * t3, x[1] + v[1] * t + a0[1] * t2 + j[1] * t3, x[2] + v[2] * t + a0[2] * t2 + j[2] * t3]);
    }
}

/// 장 역산(init)에서 시작해 닫힌 꼴 궤적으로 가우스-뉴턴 iters 번 (감쇠 작게). 반환은 Inv (가운데 상태, 발사 시점 매개변수)
fn taylor_fit(ae: &SharedRe, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, init: &Inv, iters: usize) -> (Inv, usize) {
    let ks: Vec<usize> = (0..obs.len()).filter(|&k| obs[k][0].is_finite()).collect();
    let tc = (ks[0] + ks[ks.len() - 1]) as f64 * fdt / 2.0;
    let s = init.s;
    let a_init = init.p[7];
    let mut x = [s[0] * L, s[1] * L, s[2] * L, s[3] * U_MS, s[4] * U_MS, s[5] * U_MS, s[6] * U_MS / RB, a_init];
    let prior_w = prior_rpm(init.p[3] / 0.44704, init.p[4].to_degrees()) * std::f64::consts::TAU / 60.0;
    let sd_w = PRIOR_SD * std::f64::consts::TAU / 60.0;
    let sd_a = AXIS_SD.to_radians();
    let mut buf = Vec::with_capacity(ks.len());
    let mut evals = 0usize;
    let res = |x: &[f64; 8], buf: &mut Vec<[f64; 3]>, evals: &mut usize| -> Vec<f64> {
        taylor_pos(ae, x, &ks, fdt, tc, buf);
        *evals += 1;
        let mut r = Vec::with_capacity(ks.len() * 3 + 2);
        for (m, &k) in buf.iter().zip(&ks) {
            let m = project(*m); // CSV 의 '# cam' 줄이 있으면 원근을 되돌린 화면 평면 위치로 (없으면 그대로)
            for c in 0..3 {
                r.push((m[c] - obs[k][c]) / sig[c]);
            }
        }
        r.push((x[6] - prior_w) / sd_w);
        r.push(x[7] / sd_a);
        r
    };
    let hs = [1e-4, 1e-4, 1e-4, 1e-3, 1e-3, 1e-3, 0.5, 1e-4];
    for _ in 0..iters {
        let r0 = res(&x, &mut buf, &mut evals);
        let cols: Vec<Vec<f64>> = (0..8)
            .map(|k| {
                let mut q = x;
                q[k] += hs[k];
                let rq = res(&q, &mut buf, &mut evals);
                rq.iter().zip(&r0).map(|(a, b)| (a - b) / hs[k]).collect()
            })
            .collect();
        let mut a: Vec<Vec<f64>> = (0..8).map(|i| (0..9).map(|j| if j < 8 { cols[i].iter().zip(&cols[j]).map(|(p, q)| p * q).sum() } else { -cols[i].iter().zip(&r0).map(|(p, q)| p * q).sum::<f64>() }).collect()).collect();
        for i in 0..8 {
            a[i][i] *= 1.0 + 1e-6;
            a[i][i] += 1e-12;
        }
        let dx = solve(a);
        for k in 0..8 {
            x[k] += dx[k];
        }
        x[6] = x[6].max(0.0);
    }
    let v = [x[3], x[4], x[5]];
    let sp = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let phi = v[2].atan2(v[0]);
    let om = [x[7].cos() * -phi.sin(), x[7].sin(), x[7].cos() * phi.cos()];
    let st = [x[0] / L, x[1] / L, x[2] / L, v[0] / U_MS, v[1] / U_MS, v[2] / U_MS, x[6] * RB / U_MS];
    // 보고용 발사 시점 매개변수 (테일러로 되돌림, 스핀 감쇠 되돌림)
    let a0 = accel(ae, v, x[6], x[7], phi);
    let v0 = [v[0] - a0[0] * tc, v[1] - a0[1] * tc, v[2] - a0[2] * tc];
    let s0 = (v0[0] * v0[0] + v0[1] * v0[1] + v0[2] * v0[2]).sqrt();
    let rpm0 = x[6] * 60.0 / std::f64::consts::TAU * (BETA * sp / L * tc).exp();
    let p = [x[0] - v[0] * tc, x[1] - v[1] * tc, x[2] - v[2] * tc, s0, (v0[1] / s0).asin(), v0[2].atan2(v0[0]), rpm0, x[7]];
    (Inv { p, s: st, om }, evals)
}

/// 발사 매개변수로 캐리 (spininv 의 carry 와 같은 정의: 발사점에서 run, 앞쪽 거리 yd)
fn carry_of(ae: &SharedRe, p: &[f64; 8]) -> f64 {
    let d = flightbench::engine::dimless(p[3] / 0.44704, p[4].to_degrees(), p[6]);
    flightbench::engine::run(ae, &[[d[0], d[1], d[2], p[7]]], 0.004)[0].0 * flightbench::engine::YD
}

/// 실제 프레임 CSV (t,x,y,z; `spininv --csv` 와 같은 꼴) 를 장 역산으로. --compare 면 spininv 의 LM 도 돌려 나란히
fn csv_mode(ae: &SharedRe, path: &str, sig: [f64; 3]) {
    let (obs, fdt) = flightbench::util::read_csv(path);
    let nobs = obs.iter().filter(|o| o[0].is_finite()).count();
    let t0 = Instant::now();
    let b = field_inverse(ae, &obs, sig, fdt, true);
    let tb = t0.elapsed().as_secs_f64();
    let (c, ev) = taylor_fit(ae, &obs, sig, fdt, &b, 3);
    let tc = t0.elapsed().as_secs_f64();
    println!("프레임 {nobs}개 ({:.0} fps, 창 {:.3} s)", 1.0 / fdt, (obs.len() - 1) as f64 * fdt);
    let show = |lab: &str, p: &[f64; 8], t: f64| {
        println!("{lab:<26} 볼스피드 {:.1} mph, 발사각 {:.2}°, 방위 {:.2}°, 스핀 {:.0} rpm, 축 {:.1}°, 캐리 {:.1} yd  [{}]", p[3] / 0.44704, p[4].to_degrees(), p[5].to_degrees(), p[6], p[7].to_degrees(), carry_of(ae, p), fmt_t(t));
    };
    show("장 역산 (대수, 적분 0)", &b.p, tb);
    show(&format!("+ 닫힌 꼴 궤적 GN ({ev}평가)"), &c.p, tc);
    if std::env::args().any(|a| a == "--compare") {
        let t0 = Instant::now();
        let prior = prior_rpm(b.p[3] / 0.44704, b.p[4].to_degrees());
        let mut best = ([0.0; 8], f64::INFINITY, 0usize);
        let mut evs = 0;
        for r0 in [prior, 2000.0, 5000.0, 9000.0] {
            let f = fit_lm(ae, [b.p[0], b.p[1], b.p[2], b.p[3], b.p[4], b.p[5], r0, 0.0], &obs, sig, fdt, prior, 60);
            evs += f.2;
            if f.1 < best.1 {
                best = f;
            }
        }
        show(&format!("LM (spininv 방식, {evs}평가)"), &best.0, t0.elapsed().as_secs_f64());
    }
}

/// 가짜 CSV: 드라이버·7번·웨지 중 하나를 창 win 초 동안 240 fps 로, 노이즈 σ (축별) 를 넣어 쓴다
fn write_synth(ae: &SharedRe, path: &str, sig: [f64; 3]) {
    let which = flightbench::util::arg("--shot").unwrap_or("driver".into());
    let (mph, deg, rpm) = match which.as_str() { "7i" => (120.0, 16.3, 7097.0), "wedge" => (86.0, 25.7, 8403.0), _ => (167.0, 10.9, 2686.0) };
    let win = flightbench::util::arg("--win").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.2);
    let fdt: f64 = 1.0 / 240.0;
    let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, 8f64.to_radians()];
    let nf = (win / fdt).round() as usize + 1;
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    let mut s = String::from("# t,x,y,z (m, s) 가짜 프레임: fieldinv --synth-csv\n");
    for (k, p) in sample(ae, &truth, nf, fdt).iter().enumerate() {
        s += &format!("{:.6},{:.6},{:.6},{:.6}\n", k as f64 * fdt, p[0] + sig[0] * rng.gauss(), p[1] + sig[1] * rng.gauss(), p[2] + sig[2] * rng.gauss());
    }
    std::fs::write(path, s).unwrap();
    println!("{path}: {which} {mph} mph {deg}° {rpm} rpm 축 8°, {nf}프레임, 참 캐리 {:.1} yd", carry_of(ae, &truth));
}

fn main() {
    let ae = default_field();
    {
        let sg = flightbench::util::arg("--sig").and_then(|v| v.parse::<f64>().ok()).unwrap_or(5.0) / 1000.0;
        let behind = flightbench::util::arg("--cam").as_deref() == Some("behind");
        let sig = if behind { [5.0 * sg, sg, sg] } else { [sg, sg, 5.0 * sg] };
        if let Some(p) = flightbench::util::arg("--synth-csv") {
            write_synth(&ae, &p, sig);
            return;
        }
        if let Some(p) = flightbench::util::arg("--csv") {
            csv_mode(&ae, &p, sig);
            return;
        }
    }
    let mc: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let sigma_mult: f64 = if std::env::args().any(|a| a == "--clean") { 1e-6 } else { 1.0 }; // 노이즈 없는 검산 (가중치는 그대로)
    let fdt: f64 = 1.0 / 240.0;
    let shots = [("드라이버", 167.0, 10.9, 2686.0), ("7번", 120.0, 16.3, 7097.0), ("웨지", 86.0, 25.7, 8403.0)];
    let windows: [f64; 4] = [0.05, 0.1, 0.2, 0.3];
    // --sig 위치 노이즈 mm (기본 5), --cam side|behind (깊이 축 ×5): 옆이면 깊이 = 좌우 z, 뒤면 깊이 = 앞뒤 x
    let sg = flightbench::util::arg("--sig").and_then(|v| v.parse::<f64>().ok()).unwrap_or(5.0) / 1000.0;
    let behind = flightbench::util::arg("--cam").as_deref() == Some("behind");
    let sig = if behind { [5.0 * sg, sg, sg] } else { [sg, sg, 5.0 * sg] };
    println!("장 역산 대 LM ({} 카메라 240 fps, 위치 노이즈 σ {:.1} mm, 깊이 ×5, 몬테카를로 {mc}회, 장: 공유위기12)", if behind { "뒤" } else { "옆" }, sg * 1e3);
    println!("오차는 RMS: 스핀 rpm / 축 ° / 캐리 yd / 옆 yd, 그리고 역산 시간(마지막 앞으로 한 번 날리기는 뺌, 샷당).  평가 = 궤적 적분 횟수");
    println!("  A 장 역산(양력 투영)   B 장 역산(양력+항력 투영)   C B → 닫힌 꼴 궤적 가우스-뉴턴 3번 (적분 0번)");
    println!("  D A → 짧은 LM 4번 (궤적 적분)   E LM 시작점 3개·반복 60 (spininv 와 같은 방식)\n");
    for (name, mph, deg, rpm) in shots {
        let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, 8f64.to_radians()];
        let (st, om) = launch_state(&truth);
        let (cx, cz) = land(&ae, st, om);
        for &win in &windows {
            let nf = (win / fdt).round() as usize + 1;
            let clean = sample(&ae, &truth, nf, fdt);
            let mut rng = Rng(0x9E3779B97F4A7C15 ^ ((win * 1e4) as u64) ^ (rpm as u64) << 20);
            // [스핀², 축², 캐리², 옆², 시간 s, 평가 수]
            let mut acc = [[0.0f64; 6]; 5];
            for _ in 0..mc {
                let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sigma_mult * sig[0] * rng.gauss(), p[1] + sigma_mult * sig[1] * rng.gauss(), p[2] + sigma_mult * sig[2] * rng.gauss()]).collect();
                let mut add = |m: usize, p: &[f64; 8], land_xz: (f64, f64), dt: f64, ev: usize| {
                    acc[m][0] += (p[6] - rpm).powi(2);
                    acc[m][1] += (p[7] - truth[7]).to_degrees().powi(2);
                    acc[m][2] += (land_xz.0 - cx).powi(2);
                    acc[m][3] += (land_xz.1 - cz).powi(2);
                    acc[m][4] += dt;
                    acc[m][5] += ev as f64;
                };
                // 1) 장 역산, 양력 투영만
                let t0 = Instant::now();
                let inv = field_inverse(&ae, &obs, sig, fdt, false);
                let dt = t0.elapsed().as_secs_f64();
                add(0, &inv.p, carry_from_inv(&ae, &inv), dt, 0);
                // 2) 장 역산, 항력 투영도
                let t0 = Instant::now();
                let inv2 = field_inverse(&ae, &obs, sig, fdt, true);
                let dt = t0.elapsed().as_secs_f64();
                add(1, &inv2.p, carry_from_inv(&ae, &inv2), dt, 0);
                // 3) 닫힌 꼴 궤적 가우스-뉴턴 (적분 없음)
                let t0 = Instant::now();
                let (inv3, evt) = taylor_fit(&ae, &obs, sig, fdt, &inv2, 3);
                let dt = t0.elapsed().as_secs_f64();
                add(2, &inv3.p, carry_from_inv(&ae, &inv3), dt, evt);
                // 4) 장 역산에서 시작한 짧은 LM (궤적 적분)
                let t0 = Instant::now();
                let prior = prior_rpm(inv.p[3] / 0.44704, inv.p[4].to_degrees());
                let (p3, _, ev3) = fit_lm(&ae, inv.p, &obs, sig, fdt, prior, 4);
                let dt = t0.elapsed().as_secs_f64();
                let (s3, o3) = launch_state(&p3);
                add(3, &p3, land(&ae, s3, o3), dt, ev3);
                // 5) 기준 LM: spininv 와 같은 시작점 3개 (2차 맞춤 속도 + 스핀 사전값·2000·9000), 반복 60
                let t0 = Instant::now();
                let g = inv.p; // 시작 위치·속도는 2차 맞춤 (spininv 의 initial_guess 와 같은 정보)
                let mut best = ([0.0; 8], f64::INFINITY, 0usize);
                let mut ev = 0;
                for r0 in [prior, 2000.0, 9000.0] {
                    let start = [g[0], g[1], g[2], g[3], g[4], g[5], r0, 0.0];
                    let f = fit_lm(&ae, start, &obs, sig, fdt, prior, 60);
                    ev += f.2;
                    if f.1 < best.1 {
                        best = f;
                    }
                }
                let dt = t0.elapsed().as_secs_f64();
                let (s4, o4) = launch_state(&best.0);
                add(4, &best.0, land(&ae, s4, o4), dt, ev);
            }
            let k = mc as f64;
            println!("{name} {win:.2} s ({:.1} m 비행)", ((clean[nf - 1][0] - clean[0][0]).powi(2) + (clean[nf - 1][1] - clean[0][1]).powi(2)).sqrt());
            for (m, lab) in ["A", "B", "C", "D", "E"].iter().enumerate() {
                let a = &acc[m];
                println!(
                    "   {lab}  스핀 {:>5.0}  축 {:>5.2}  캐리 {:>5.2}  옆 {:>5.2}  | {:>8} {:>5.0}평가",
                    (a[0] / k).sqrt(), (a[1] / k).sqrt(), (a[2] / k).sqrt(), (a[3] / k).sqrt(), fmt_t(a[4] / k), a[5] / k
                );
            }
        }
    }
}

fn fmt_t(s: f64) -> String {
    if s < 1e-3 { format!("{:.1}µs", s * 1e6) } else { format!("{:.2}ms", s * 1e3) }
}
