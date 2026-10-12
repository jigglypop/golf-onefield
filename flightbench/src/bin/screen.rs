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
//! 2026-10-12 기본 장을 공유위기12 (engine::default_field, 위기 n = 12) 로 바꾼 뒤 같은 설정으로 다시 잼:
//!   대각 + k_t 보정 드라이버 1.80 yd·±200 rpm, 7번 0.84, 아마7번 0.64, 웨지 0.35 / k_t 모름 드라이버 6.51 yd·±850 rpm
//!   닫힌 꼴 + LM: 240 fps 드라이버 3.35·7번 0.72·웨지 0.35, 60 fps 6.11·1.36·0.68 / 자가 보정 5샷 ±0.0067, 40샷 ±0.0024
//!   단단한 벽(WALL=1) LM: 240 fps 2.31·0.60·0.29, 60 fps 4.60·1.09·0.56 yd. 장을 바꿔도 측정 오차는 거의 그대로다.
//!
//! 2026-10-13 닫힌 꼴 궤적 (`sample_cf`, 궤적 적분 없음): 조각(≤ 0.06 s)마다 장의 가속도·저크로 테일러 3차, 벽 시각은 3차식의 근,
//!   반동은 `bounce` 그대로. --cfcheck: 고운 RK4(프레임당 256걸음) 대비 벽 전 0.001–0.02 mm·반동 뒤 0.02–0.08 mm.
//!   예전 RK4(프레임당 8걸음)는 반동 뒤 0.35–0.98 mm (걸음 안 충돌을 선형으로 잡기 때문) → 닫힌 꼴이 10배 정확하고, 한 번 평가가 0.8–1.3 µs 로 RK4(57–110 µs)의 60–80배 빠르다.
//!   --cfstudy (대각 σ 5 mm, 반동 0.2 s, k_t 보정, 40회): 같은 시작점 넷이면 정확도가 같고 13배 빠름, 닫힌 꼴 시작점 하나(반복 30)도 정확도가 같다.
//!     240 fps 드라이버 스핀 255 → 253 rpm·캐리 3.29 → 3.28 yd, 19.2 ms → 0.38 ms; 7번 0.74 yd 18.9 ms → 0.37 ms; 웨지 0.35 yd 29.4 ms → 0.79 ms.
//!     60 fps 6–7 ms → 0.16–0.19 ms. 단단한 벽(WALL=1)도 같다 (240 fps 18.6–25.6 ms → 0.28–0.38 ms).
//!   --csv 의 기본을 닫힌 꼴로 바꿨다(시작점 하나 먼저, 카이제곱/자유도 > 2 이거나 스핀이 경계에 붙으면 나머지 시작점). --ode 는 예전 RK4, --compare 는 둘 다.
//!   가짜 영상 왕복(tools/e2e_synth.sh, Ryzen 5 7500F): 7번 168.4 (참 168.5), 드라이버 265.8 (270.0), 웨지 103.7 (104.7) yd, 던지기 k_t 0.2833 (0.2857);
//!     역보정 시간 RK4 9–23 ms → 닫힌 꼴 0.18–1.5 ms. 예전 RK4 로는 7번 168.4, 드라이버 265.7, 웨지 103.7, k_t 0.2846.
//!
//! cargo run --release --bin screen [몬테카를로 횟수]     AFTER=0.05,0.1,0.15 로 반동 관측 시간 바꾸기, --selfcal, --cfcheck, --cfstudy

use flightbench::engine::{dimless, run, Aero, Rng, SharedRe, U_MS, YD};
use flightbench::linalg::solve;
use flightbench::lm::{lm, Damping, LmCfg, Step};
use flightbench::util::{arg, launch_from_quad, prior_rpm, project, quad_coeffs, read_csv};

const G: f64 = 9.81;
const TU: f64 = U_MS / G;
const R: f64 = 0.021335;
const NP: usize = 10;

fn field() -> SharedRe {
    flightbench::engine::default_field()
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

/// p = [x0 y0 z0, 속도, 발사각, 방위각, rpm, 축, k_t, e] → 프레임 위치들 (프레임마다 RK4 8걸음)
fn sample(ae: &SharedRe, p: &[f64; NP], scr: Screen, n: usize, fdt: f64) -> Vec<[f64; 3]> {
    sample_sub(ae, p, scr, n, fdt, 8)
}

/// `sample` 과 같고 프레임당 RK4 걸음 수를 정한다 (검산용 고운 기준)
fn sample_sub(ae: &SharedRe, p: &[f64; NP], scr: Screen, n: usize, fdt: f64, sub: usize) -> Vec<[f64; 3]> {
    let (th, ph) = (p[4], p[5]);
    let mut pos = [p[0], p[1], p[2]];
    let mut v = [p[3] * th.cos() * ph.cos(), p[3] * th.sin(), p[3] * th.cos() * ph.sin()];
    let nrm = [-ph.sin(), 0.0, ph.cos()];
    let omg = p[6] * std::f64::consts::TAU / 60.0;
    let mut om = [omg * p[7].cos() * nrm[0], omg * p[7].sin(), omg * p[7].cos() * nrm[2]];
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

/// 닫힌 꼴 궤적 (궤적 적분 없음): 같은 매개변수 → 같은 프레임 위치를 장의 닫힌 꼴로 낸다.
///   조각마다 시작 상태에서 테일러 3차 p(τ) = p + v τ + ½A τ² + ⅙J τ³ (A = 장의 가속도, J = dA/dt 를 유한 차분으로).
///   조각 길이는 CF_PIECE 초 이하. 벽 시각은 조각 안 3차식의 근(뉴턴), 반동은 `bounce` 그대로(이미 대수식),
///   반동 뒤는 그 상태에서 다시 전개한다. 조각 하나 = 장 계산 3번 (`sample` 은 프레임마다 RK4 8걸음 = 장 계산 32번).
const CF_PIECE: f64 = 0.06;

fn sample_cf(ae: &SharedRe, q: &[f64; NP], scr: Screen, n: usize, fdt: f64) -> Vec<[f64; 3]> {
    let (th, ph) = (q[4], q[5]);
    let nrm = [-ph.sin(), 0.0, ph.cos()];
    let omg = q[6] * std::f64::consts::TAU / 60.0;
    let wall = scr.dist - R;
    // 한 점에서의 전개: (가속도, 저크, 스핀 변화율)
    let expand = |v: [f64; 3], om: [f64; 3]| -> ([f64; 3], [f64; 3], [f64; 3]) {
        let (a, wd) = accel(ae, v, om);
        let h = 1e-3;
        let (ap, _) = accel(ae, std::array::from_fn(|i| v[i] + h * a[i]), std::array::from_fn(|i| om[i] + h * wd[i]));
        let (am, _) = accel(ae, std::array::from_fn(|i| v[i] - h * a[i]), std::array::from_fn(|i| om[i] - h * wd[i]));
        (a, std::array::from_fn(|i| (ap[i] - am[i]) / (2.0 * h)), wd)
    };
    // 조각: (시작 시각, 위치, 속도, 스핀, A, J, 스핀 변화율)
    type Piece = (f64, [f64; 3], [f64; 3], [f64; 3], [f64; 3], [f64; 3], [f64; 3]);
    let at = |c: &Piece, t: f64| -> [f64; 3] { std::array::from_fn(|i| c.1[i] + c.2[i] * t + 0.5 * c.4[i] * t * t + c.5[i] * t * t * t / 6.0) };
    let vat = |c: &Piece, t: f64| -> [f64; 3] { std::array::from_fn(|i| c.2[i] + c.4[i] * t + 0.5 * c.5[i] * t * t) };
    let wat = |c: &Piece, t: f64| -> [f64; 3] { std::array::from_fn(|i| c.3[i] + c.6[i] * t) };
    let make = |t0: f64, p: [f64; 3], v: [f64; 3], om: [f64; 3]| -> Piece {
        let (a, j, wd) = expand(v, om);
        (t0, p, v, om, a, j, wd)
    };
    let p0 = [q[0], q[1], q[2]];
    let v0 = [q[3] * th.cos() * ph.cos(), q[3] * th.sin(), q[3] * th.cos() * ph.sin()];
    let om0 = [omg * q[7].cos() * nrm[0], omg * q[7].sin(), omg * q[7].cos() * nrm[2]];
    let t_end = (n.max(1) - 1) as f64 * fdt;
    let mut pieces: Vec<Piece> = vec![make(0.0, p0, v0, om0)];
    let mut bounced = false;
    loop {
        let c = *pieces.last().unwrap();
        let len = CF_PIECE.min((t_end - c.0).max(0.0));
        if len <= 0.0 {
            break;
        }
        // 이 조각 안에서 벽에 닿는가 (반동 전, 앞으로 갈 때만)
        let mut hit = None;
        if !bounced && c.2[0] > 0.0 {
            let xe = at(&c, len)[0];
            if xe >= wall {
                let mut t = ((wall - c.1[0]) / c.2[0]).clamp(0.0, len);
                for _ in 0..8 {
                    let f = at(&c, t)[0] - wall;
                    let d = vat(&c, t)[0].max(1e-6);
                    t = (t - f / d).clamp(0.0, len);
                }
                hit = Some(t);
            }
        }
        match hit {
            Some(t) => {
                let (bv, bo) = bounce(vat(&c, t), wat(&c, t), q[8], q[9], scr.mu);
                pieces.push(make(c.0 + t, at(&c, t), bv, bo));
                bounced = true;
            }
            None => {
                if c.0 + len >= t_end {
                    break;
                }
                pieces.push(make(c.0 + len, at(&c, len), vat(&c, len), wat(&c, len)));
            }
        }
    }
    let mut ci = 0;
    (0..n)
        .map(|k| {
            let t = k as f64 * fdt;
            while ci + 1 < pieces.len() && pieces[ci + 1].0 <= t {
                ci += 1;
            }
            at(&pieces[ci], t - pieces[ci].0)
        })
        .collect()
}

fn carry(ae: &SharedRe, p: &[f64; NP]) -> f64 {
    let d = dimless(p[3] / 0.44704, p[4].to_degrees(), p[6]);
    run(ae, &[[d[0], d[1], d[2], p[7]]], 0.004)[0].0 * YD
}

struct Prior {
    rpm: f64,
    rpm_sd: f64,
    /// 방위각 사전 폭 (rad). 좌우(z)를 못 보는 옆 카메라에서만 유한하게
    phi_sd: f64,
    kt: f64,
    kt_sd: f64,
}

fn resid(ae: &SharedRe, p: &[f64; NP], scr: Screen, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, pr: &Prior, cf: bool) -> Vec<f64> {
    let m = if cf { sample_cf(ae, p, scr, obs.len(), fdt) } else { sample(ae, p, scr, obs.len(), fdt) };
    let mut r = Vec::with_capacity(obs.len() * 3 + 3);
    for (a, b) in m.iter().zip(obs) {
        let a = project(*a);
        for c in 0..3 {
            r.push(if b[c].is_finite() { (a[c] - b[c]) / sig[c] } else { 0.0 });
        }
    }
    r.push((p[6] - pr.rpm) / pr.rpm_sd);
    r.push(p[7].to_degrees() / 20.0);
    r.push((p[8] - pr.kt) / pr.kt_sd.max(1e-6));
    if pr.phi_sd.is_finite() {
        r.push(p[5] / pr.phi_sd);
    }
    r
}

/// cf = true 면 닫힌 꼴 궤적(`sample_cf`, 적분 없음), false 면 RK4 궤적(`sample`). 반환 (매개변수, 비용)
fn fit(ae: &SharedRe, p0: [f64; NP], scr: Screen, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, pr: &Prior, cf: bool) -> ([f64; NP], f64) {
    fit_n(ae, p0, scr, obs, sig, fdt, pr, cf, 80).0
}

/// `fit` 과 같고 반복 상한을 정한다. 반환 ((매개변수, 비용), 잔차 평가 수)
fn fit_n(ae: &SharedRe, p0: [f64; NP], scr: Screen, obs: &[[f64; 3]], sig: [f64; 3], fdt: f64, pr: &Prior, cf: bool, max_iter: usize) -> (([f64; NP], f64), usize) {
    let step = [1e-4, 1e-4, 1e-4, 1e-3, 1e-5, 1e-5, 1.0, 1e-4, 1e-4, 1e-4];
    let lo = [-1.0, -1.0, -1.0, 5.0, -0.2, -1.0, 0.0, -1.2, 0.0, 0.02];
    let hi = [1.0, 1.0, 1.0, 100.0, 1.2, 1.0, 15000.0, 1.2, 1.0, 0.95];
    // 공통 레벤버그-마쿼트(lm.rs): 고정 야코비안 걸음, 감쇠 A_xx(1+μ)+1e-12, 80회 × 안쪽 12번, 상대 개선 1e-10 이하에서 끝
    let cfg = LmCfg { lo: &lo, hi: &hi, step: Step::Fixed(&step), damping: Damping::Mul, max_iter, tries: 12, mu_min: 1e-10, stop_rel: 1e-10, nonfinite_cost: None };
    let mut res = |q: &[f64]| resid(ae, &<[f64; NP]>::try_from(q).unwrap(), scr, obs, sig, fdt, pr, cf);
    let o = lm(&cfg, &p0, &mut res, None);
    ((<[f64; NP]>::try_from(o.p.as_slice()).unwrap(), o.cost), o.evals)
}

/// 충돌 전 프레임 앞부분 2차 맞춤으로 시작점
fn initial_guess(obs: &[[f64; 3]], fdt: f64, rpm0: f64, kt: f64) -> [f64; NP] {
    let mut co = [[0.0; 3]; 3];
    for c in 0..3 {
        let ks: Vec<usize> = (0..obs.len()).filter(|&k| obs[k][c].is_finite()).take(6).collect();
        if ks.len() < 3 {
            continue; // 이 축은 관측 없음 (옆 카메라의 좌우 등): 0 으로 둔다
        }
        co[c] = quad_coeffs(obs, fdt, c, &ks);
    }
    let l = launch_from_quad(&co);
    [l[0], l[1], l[2], l[3], l[4], l[5], rpm0, 0.0, kt, 0.3]
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
                let rpm = prior_rpm(mph, deg) + 1000.0 * rng.gauss();
                let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), (3.0 * rng.gauss()).to_radians(), rpm.max(1500.0), (8.0 * rng.gauss()).to_radians(), 0.20, 0.30];
                let probe = sample(ae, &truth, scr, 200, fdt);
                let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
                let n = k_hit + 25;
                let obs: Vec<[f64; 3]> = sample(ae, &truth, scr, n, fdt).iter().map(|p| [p[0] + sig[0] * rng.gauss(), p[1] + sig[1] * rng.gauss(), p[2] + sig[2] * rng.gauss()]).collect();
                shots.push(obs);
            }
            let total = |kt: f64| -> f64 {
                shots.iter().map(|obs| {
                    let g0 = initial_guess(obs, fdt, 0.0, kt);
                    let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), rpm_sd: 1500.0, phi_sd: f64::INFINITY, kt, kt_sd: 1e-4 };
                    let mut best = f64::INFINITY;
                    for r0 in [pr.rpm, 2500.0, 8000.0] {
                        best = best.min(fit(ae, initial_guess(obs, fdt, r0, kt), scr, obs, sig, fdt, &pr, false).1);
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
/// 반환 [볼스피드 m/s, 발사각, 방위각, rpm, 축, 반발계수 e]
fn closed_form(obs: &[[f64; 3]], fdt: f64, wall: f64, kt: f64) -> [f64; 6] {
    let k_last = (0..obs.len()).filter(|&k| obs[k][0].is_finite()).max_by(|&a, &b| obs[a][0].total_cmp(&obs[b][0])).unwrap();
    // 충돌 전: 0..=k_last-? (가장 앞 프레임은 이미 반동했을 수 있어 하나 덜어냄)
    let pre: Vec<usize> = (0..k_last.max(2)).collect();
    let post: Vec<usize> = (k_last + 1..obs.len()).collect();
    let t = |k: usize| k as f64 * fdt;
    let fitc = |idx: &[usize], c: usize, deg: usize| -> Vec<f64> {
        let idx: Vec<usize> = idx.iter().copied().filter(|&k| obs[k][c].is_finite()).collect();
        if idx.len() < 2 {
            return vec![0.0, 0.0];
        }
        polyfit(&idx.iter().map(|&k| t(k)).collect::<Vec<_>>(), &idx.iter().map(|&k| obs[k][c]).collect::<Vec<_>>(), deg.min(idx.len() - 1))
    };
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
    let qx = fitc(&post, 0, qd);
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
    let e = (-dv(&qx, th) / dv(&px, th).max(1.0)).clamp(0.05, 0.95);
    [v, (vy / v).asin(), vz.atan2(vx), om * 60.0 / std::f64::consts::TAU, wy.atan2(wz), e]
}

/// --cfcheck: 닫힌 꼴 궤적(sample_cf)과 RK4 궤적(sample)의 프레임 위치 차 (같은 매개변수), 평가 한 번 시간
fn cf_check(ae: &SharedRe) {
    let scr = Screen { dist: 3.0, mu: 0.5 };
    let fdt = 1.0 / 240.0;
    println!("닫힌 꼴 궤적·RK4 궤적의 정확도: 같은 매개변수, 기준은 고운 RK4 (프레임당 256걸음), 반동 0.2 s 까지, 240 fps");
    let shots = [("드라이버", 167.0, 10.9, 2686.0, 8.0), ("7번", 120.0, 16.3, 7097.0, 8.0), ("아마7번", 100.0, 18.0, 6000.0, 8.0), ("웨지", 86.0, 25.7, 8403.0, -5.0), ("던지기", 30.0, 22.0, 0.0, 0.0)];
    for (kt, e, tag) in [(0.20, 0.30, "천 스크린 k_t 0.20 e 0.3"), (2.0 / 7.0, 0.6, "단단한 벽 k_t 2/7 e 0.6")] {
        println!("  {tag}");
        for (name, mph, deg, rpm, axis) in shots {
            let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, (axis as f64).to_radians(), kt, e];
            let probe = sample(ae, &truth, scr, 2000, fdt);
            let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
            let n = k_hit + 49;
            let fine = sample_sub(ae, &truth, scr, n, fdt, 256);
            let a = sample(ae, &truth, scr, n, fdt);
            let b = sample_cf(ae, &truth, scr, n, fdt);
            let dmax = |u: &Vec<[f64; 3]>| -> (f64, f64) {
                let (mut pre, mut post) = (0.0f64, 0.0f64);
                for (k, (x, y)) in fine.iter().zip(u).enumerate() {
                    let d = ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt() * 1e3;
                    if k <= k_hit { pre = pre.max(d) } else { post = post.max(d) }
                }
                (pre, post)
            };
            let (pre_rk, post_rk) = dmax(&a);
            let (pre, post) = dmax(&b);
            let reps = 2000;
            let t0 = std::time::Instant::now();
            let mut s = 0.0;
            for _ in 0..reps { s += sample(ae, &truth, scr, n, fdt)[n - 1][0]; }
            let t_rk = t0.elapsed().as_secs_f64() / reps as f64;
            let t0 = std::time::Instant::now();
            for _ in 0..reps { s += sample_cf(ae, &truth, scr, n, fdt)[n - 1][0]; }
            let t_cf = t0.elapsed().as_secs_f64() / reps as f64;
            std::hint::black_box(s);
            println!("    {name:<6} 프레임 {n:>3} (충돌 {k_hit:>3}): 고운 RK4(256걸음) 대비 최대 차 — 닫힌 꼴 벽 전 {pre:.3}·반동 뒤 {post:.3} mm, RK4 8걸음 벽 전 {pre_rk:.3}·반동 뒤 {post_rk:.3} mm | 평가 RK4 {:.1} µs, 닫힌 꼴 {:.2} µs", t_rk * 1e6, t_cf * 1e6);
        }
    }
}

/// --cfstudy: 벽 반동 역보정을 닫힌 꼴 궤적으로 (적분 없음). closed_study 와 같은 조건(대각 카메라 σ 5 mm, 반동 0.2 s, k_t 보정 0.20)
fn cf_study(ae: &SharedRe) {
    let scr = Screen { dist: 3.0, mu: 0.5 };
    let mc: usize = std::env::args().skip(1).find_map(|s| s.parse().ok()).unwrap_or(40);
    let shots = [("드라이버", 167.0, 10.9, 2686.0), ("7번", 120.0, 16.3, 7097.0), ("아마7번", 100.0, 18.0, 6000.0), ("웨지", 86.0, 25.7, 8403.0)];
    let wall = std::env::var("WALL").is_ok();
    let (ktv, ev) = if wall { (2.0 / 7.0, 0.6) } else { (0.20, 0.30) };
    println!("벽 반동 역보정: RK4 궤적 LM 대 닫힌 꼴 궤적 LM (적분 없음). 대각 카메라 σ 5 mm, 반동 0.2 s, k_t {ktv:.3} 보정(± 0.01), e {ev}, 몬테카를로 {mc}회");
    println!("  RK4×4 = 지금 --csv 와 같은 방법(시작점 4개, 반복 80). 닫힌꼴×4 = 같은 시작점·반복을 닫힌 꼴 궤적으로. 닫힌꼴×1 = 닫힌 꼴 시작점 하나, 반복 30");
    println!("  RMS 스핀 rpm / 축° / 캐리 yd, 샷당 시간, 잔차 평가 수");
    for fps in [60.0f64, 120.0, 240.0] {
        let fdt = 1.0 / fps;
        let sig = [2.2 * 0.005, 0.005, 2.2 * 0.005];
        for (name, mph, deg, rpm) in shots {
            let truth = [0.0, 0.02, 0.0, mph * 0.44704, (deg as f64).to_radians(), 2f64.to_radians(), rpm, 8f64.to_radians(), ktv, ev];
            let c_true = carry(ae, &truth);
            let probe = sample(ae, &truth, scr, 400, fdt);
            let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
            let n = k_hit + (0.2 * fps).round() as usize + 1;
            let clean = sample(ae, &truth, scr, n, fdt);
            let mut rng = Rng(0xBEEF ^ (fps as u64) << 12 ^ (rpm as u64));
            // [스핀², 축², 캐리², 시간, 평가]
            let mut acc = [[0.0f64; 5]; 3];
            for _ in 0..mc {
                let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sig[0] * rng.gauss(), p[1] + sig[1] * rng.gauss(), p[2] + sig[2] * rng.gauss()]).collect();
                for (m, (cf, starts_all, iters)) in [(false, true, 80usize), (true, true, 80), (true, false, 30)].into_iter().enumerate() {
                    let t0 = std::time::Instant::now();
                    let g0 = initial_guess(&obs, fdt, 0.0, ktv);
                    let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), rpm_sd: 1500.0, phi_sd: f64::INFINITY, kt: ktv, kt_sd: 0.01 };
                    let cfm = closed_form(&obs, fdt, scr.dist - R, ktv);
                    let starts: Vec<f64> = if starts_all { vec![cfm[3].clamp(500.0, 14000.0), pr.rpm, 2500.0, 8000.0] } else { vec![cfm[3].clamp(500.0, 14000.0)] };
                    let mut best = ([0.0; NP], f64::INFINITY);
                    let mut evs = 0;
                    for r0 in starts {
                        let mut st = initial_guess(&obs, fdt, r0, ktv);
                        st[7] = cfm[4].clamp(-1.0, 1.0);
                        st[9] = cfm[5];
                        let (f, e_) = fit_n(ae, st, scr, &obs, sig, fdt, &pr, cf, iters);
                        evs += e_;
                        if f.1 < best.1 {
                            best = f;
                        }
                    }
                    let dt = t0.elapsed().as_secs_f64();
                    let p = best.0;
                    acc[m][0] += (p[6] - rpm).powi(2);
                    acc[m][1] += (p[7] - truth[7]).to_degrees().powi(2);
                    acc[m][2] += (carry(ae, &p) - c_true).powi(2);
                    acc[m][3] += dt;
                    acc[m][4] += evs as f64;
                }
            }
            let k = mc as f64;
            let cell = |m: usize| format!("{:>5.0} {:>5.2} {:>5.2} {:>8} {:>5.0}", (acc[m][0] / k).sqrt(), (acc[m][1] / k).sqrt(), (acc[m][2] / k).sqrt(), fmt_t(acc[m][3] / k), acc[m][4] / k);
            println!("{name:<6} {fps:>3.0} fps | RK4×4 {} | 닫힌꼴×4 {} | 닫힌꼴×1 {}", cell(0), cell(1), cell(2));
        }
    }
}

fn fmt_t(s: f64) -> String {
    if s < 1e-3 { format!("{:.0}µs", s * 1e6) } else { format!("{:.1}ms", s * 1e3) }
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
                let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sig[0] * rng.gauss(), p[1] + sig[1] * rng.gauss(), p[2] + sig[2] * rng.gauss()]).collect();
                let t0 = std::time::Instant::now();
                let cf = closed_form(&obs, fdt, scr.dist - R, ktv);
                t1 += t0.elapsed().as_secs_f64();
                let pc = [0.0, 0.02, 0.0, cf[0], cf[1], cf[2], cf[3], cf[4], ktv, ev];
                e1 += (cf[3] - rpm).powi(2);
                a1 += (cf[4] - truth[7]).to_degrees().powi(2);
                c1 += (carry(ae, &pc) - c_true).powi(2);
                let t0 = std::time::Instant::now();
                let g0 = initial_guess(&obs, fdt, 0.0, ktv);
                let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), rpm_sd: 1500.0, phi_sd: f64::INFINITY, kt: ktv, kt_sd: 0.01 };
                // LM 시작점을 닫힌 꼴 값으로 (탐색 시작점 하나)
                let mut st = initial_guess(&obs, fdt, cf[3].clamp(500.0, 14000.0), ktv);
                st[9] = ev;
                st[7] = cf[4].clamp(-1.0, 1.0);
                let (p, _) = fit(ae, st, scr, &obs, sig, fdt, &pr, false);
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

/// 실제 프레임: 벽(스크린) 반동 역보정. --wall 공 출발점~벽 면 거리 m, --kt (기본 2/7), --kt-sd (기본 0.02),
/// --sig mm (기본 5), --cam side|behind|diag, --throw (손으로 던진 보정 샷: 스핀 0 ± 300, k_t 자유)
fn csv_mode(ae: &SharedRe, path: &str) {
    let (obs, fdt) = read_csv(path);
    let num = |n: &str, d: f64| arg(n).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let scr = Screen { dist: num("--wall", 3.0), mu: 0.5 };
    let sg = num("--sig", 5.0) / 1000.0;
    let sig = match arg("--cam").as_deref() { Some("behind") => [5.0 * sg, sg, sg], Some("diag") => [2.2 * sg, sg, 2.2 * sg], _ => [sg, sg, 5.0 * sg] };
    let throw = std::env::args().any(|a| a == "--throw");
    let (kt0, kt_sd) = if throw { (2.0 / 7.0, 1.0) } else { (num("--kt", 2.0 / 7.0), num("--kt-sd", 0.02)) };
    let g0 = initial_guess(&obs, fdt, 0.0, kt0);
    let pr = if throw { Prior { rpm: 0.0, rpm_sd: num("--throw-spin-sd", 300.0), phi_sd: f64::INFINITY, kt: kt0, kt_sd } } else { Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), rpm_sd: 1500.0, phi_sd: f64::INFINITY, kt: kt0, kt_sd } };
    let mut pr = pr;
    let z_seen = obs.iter().filter(|o| o[2].is_finite()).count();
    if z_seen < 3 {
        pr.phi_sd = 3f64.to_radians();
        println!("좌우(z) 관측 없음: 방위각 0 ± 3°, 스핀축은 사전값(0 ± 20°)에 기댐");
    }
    let cf = closed_form(&obs, fdt, scr.dist - R, kt0);
    let nobs = obs.iter().filter(|o| o[0].is_finite()).count();
    let dof = (3 * nobs).saturating_sub(NP).max(1) as f64;
    // (스핀 시작값, k_t 시작값): 충돌이 매끄럽지 않아 LM 이 한 시작점에서 멈출 수 있어 여러 곳에서 출발
    let starts: Vec<(f64, f64)> = if throw { vec![(0.0, 0.15), (0.0, 0.2857), (0.0, 0.40)] } else { vec![cf[3].clamp(500.0, 14000.0), pr.rpm, 2500.0, 8000.0].into_iter().map(|r| (r, kt0)).collect() };
    // 궤적 모형: 기본은 닫힌 꼴(`sample_cf`, 궤적 적분 없음). --ode 면 예전 RK4 궤적, --compare 면 둘 다 풀어 나란히.
    // 닫힌 꼴은 최소 계산으로: 닫힌 꼴 시작점 하나(반복 30)로 먼저 풀고, 카이제곱/자유도가 2 를 넘거나 스핀이 경계에 붙을 때만
    // 나머지 시작점(반복 80)을 더 푼다. 던지기 보정(k_t 자유)은 처음부터 시작점 셋. RK4 는 예전처럼 시작점 넷·반복 80.
    let solve_with = |cfm: bool| -> (([f64; NP], f64), usize, usize, f64) {
        let t0 = std::time::Instant::now();
        let (mut best, mut evs, mut used) = (([0.0; NP], f64::INFINITY), 0usize, 0usize);
        let mut go = |(r0, k0): (f64, f64), iters: usize, best: &mut ([f64; NP], f64)| {
            let mut st = initial_guess(&obs, fdt, r0, k0);
            if !throw {
                st[7] = cf[4].clamp(-1.0, 1.0);
            }
            st[9] = cf[5];
            let (f, e) = fit_n(ae, st, scr, &obs, sig, fdt, &pr, cfm, iters);
            evs += e;
            used += 1;
            if f.1 < best.1 {
                *best = f;
            }
        };
        if cfm && !throw {
            go(starts[0], 30, &mut best);
            let p = best.0;
            if best.1 / dof > 2.0 || p[6] < 600.0 || p[6] > 14500.0 {
                for &s0 in &starts[1..] {
                    go(s0, 80, &mut best);
                }
            }
        } else {
            for &s0 in &starts {
                go(s0, 80, &mut best);
            }
        }
        (best, evs, used, t0.elapsed().as_secs_f64())
    };
    let use_cf = !std::env::args().any(|a| a == "--ode");
    if std::env::args().any(|a| a == "--compare") {
        for (tag, m) in [("RK4 궤적 (--ode)", false), ("닫힌 꼴 궤적 (기본)", true)] {
            let (b, evs, used, dt) = solve_with(m);
            let q = b.0;
            println!("[비교] {tag:<18} 시작점 {used}개, 잔차 평가 {evs}번, {:.2} ms: 스핀 {:.0} rpm, 축 {:.1}°, 볼스피드 {:.1} mph, 발사각 {:.2}°, k_t {:.4}, e {:.3}, 캐리 {:.1} yd, 카이제곱/자유도 {:.2}",
                dt * 1e3, q[6], q[7].to_degrees(), q[3] / 0.44704, q[4].to_degrees(), q[8], q[9], carry(ae, &q), b.1 / dof);
        }
    }
    let (best, evs, used, dt) = solve_with(use_cf);
    println!("역보정: {} 궤적, 시작점 {used}개, 잔차 평가 {evs}번, {:.2} ms", if use_cf { "닫힌 꼴(적분 없음)" } else { "RK4" }, dt * 1e3);
    let p = best.0;
    if std::env::var("DEBUG").is_ok() {
        let r = resid(ae, &p, scr, &obs, sig, fdt, &pr, use_cf);
        eprintln!("resid NaN {} / {}, 처음 10: {:?}, 끝 12: {:?}", r.iter().filter(|x| !x.is_finite()).count(), r.len(), &r[..10], &r[r.len()-12..]);
    }
    println!("프레임 {nobs}개 ({:.0} fps), 벽 {:.2} m, 카이제곱/자유도 {:.2}", 1.0 / fdt, scr.dist, best.1 / dof);
    println!("볼스피드 {:.1} mph, 발사각 {:.2}°, 방위각 {:.2}°", p[3] / 0.44704, p[4].to_degrees(), p[5].to_degrees());
    println!("벽: k_t {:.4} (강체 0.2857), 반발계수 e {:.3}", p[8], p[9]);
    if throw {
        println!("보정 샷: 다음 샷부터 --kt {:.4} --kt-sd 0.01 로 쓰세요. 스핀(던진 공) {:.0} rpm", p[8], p[6]);
    } else {
        println!("닫힌 꼴 첫 값: 스핀 {:.0} rpm, 축 {:.1}°", cf[3], cf[4].to_degrees());
        println!("스핀 {:.0} rpm, 스핀축 {:.1}°, 캐리 {:.1} yd", p[6], p[7].to_degrees(), carry(ae, &p));
    }
}

/// 가짜 영상용 참 프레임: --synth OUT.csv --mph --deg --rpm --axis --fps --wall --after
fn synth(ae: &SharedRe, out: &str) {
    let num = |n: &str, d: f64| arg(n).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let fps = num("--fps", 240.0);
    let scr = Screen { dist: num("--wall", 3.0), mu: 0.5 };
    let truth = [0.0, 0.0, 0.0, num("--mph", 120.0) * 0.44704, num("--deg", 16.3).to_radians(), 0.0, num("--rpm", 7097.0), num("--axis", 8.0).to_radians(), num("--kt", 2.0 / 7.0), num("--e", 0.6)];
    let fdt = 1.0 / fps;
    let probe = sample(ae, &truth, scr, 2000, fdt);
    let k_hit = probe.windows(2).position(|w| w[1][0] < w[0][0]).unwrap_or(10);
    let n = k_hit + (num("--after", 0.2) * fps).round() as usize + 1;
    let fr = sample(ae, &truth, scr, n, fdt);
    let mut s = String::from("t,x,y,z\n");
    for (k, p) in fr.iter().enumerate() {
        s += &format!("{:.6},{:.6},{:.6},{:.6}\n", k as f64 * fdt, p[0], p[1], p[2]);
    }
    std::fs::write(out, s).unwrap();
    println!("참값: 스핀 {} rpm, 축 {}°, 캐리 {:.1} yd, 충돌 프레임 {}", truth[6], num("--axis", 8.0), carry(ae, &truth), k_hit);
}

fn main() {
    let ae = field();
    if let Some(out) = arg("--synth") {
        synth(&ae, &out);
        return;
    }
    if let Some(path) = arg("--csv") {
        csv_mode(&ae, &path);
        return;
    }
    if std::env::args().any(|a| a == "--closed") {
        closed_study(&ae);
        return;
    }
    if std::env::args().any(|a| a == "--cfcheck") {
        cf_check(&ae);
        return;
    }
    if std::env::args().any(|a| a == "--cfstudy") {
        cf_study(&ae);
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
                            let obs: Vec<[f64; 3]> = clean.iter().map(|p| [p[0] + sig[0] * rng.gauss(), p[1] + sig[1] * rng.gauss(), p[2] + sig[2] * rng.gauss()]).collect();
                            let g0 = initial_guess(&obs, fdt, 0.0, pr_kt);
                            let pr = Prior { rpm: prior_rpm(g0[3] / 0.44704, g0[4].to_degrees()), rpm_sd: 1500.0, phi_sd: f64::INFINITY, kt: pr_kt, kt_sd: pr_sd };
                            let mut best = ([0.0; NP], f64::INFINITY);
                            for r0 in [pr.rpm, 2500.0, 8000.0] {
                                let f = fit(&ae, initial_guess(&obs, fdt, r0, pr_kt), scr, &obs, sig, fdt, &pr, false);
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
