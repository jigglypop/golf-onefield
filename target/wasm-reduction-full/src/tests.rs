use super::*;
use crate::lanes::{batch_lanes, P4};

fn shot(mph: f64, vla: f64, rpm: f64, axis: f64) -> Shot {
    Shot { mph, vla_deg: vla, rpm, axis_deg: axis }
}

/// bench 와 같은 저불일치 샷 n 개
fn shots(n: usize) -> Vec<Shot> {
    const A: [f64; 4] = [0.618_033_988_749_895, 0.754_877_666_246_692_7, 0.569_840_290_998_053_2, 0.414_213_562_373_095_1];
    let frac = |x: f64| x - x.floor();
    (0..n)
        .map(|i| {
            let x = |k: usize| frac(0.5 + (i as f64 + 1.0) * A[k]);
            Shot { mph: 60.0 + 120.0 * x(0), vla_deg: 5.0 + 30.0 * x(1), rpm: 1500.0 + 8500.0 * x(2), axis_deg: -30.0 + 60.0 * x(3) }
        })
        .collect()
}

fn flat(sh: &[Shot]) -> Vec<f64> {
    sh.iter().flat_map(|s| [s.mph, s.vla_deg, s.rpm, s.axis_deg]).collect()
}

const DRIVER: Shot = Shot { mph: 167.0, vla_deg: 10.9, rpm: 2686.0, axis_deg: 0.0 };

fn carry_yd(env: &Env, s: &Shot, h: f64) -> f64 {
    fly_with::<f64>(&Coef::SHARED_RE, env, s, h, None).carry_m / YARD
}

// ------------------------------------------------------------------ 기본 동작 (옛 시험)

/// 7번 아이언 근처 (PGA 120 mph 16.3°, 7097 rpm): 캐리는 대략 172 yd 쪽이어야 한다
#[test]
fn carry_is_in_golf_range() {
    let r = fly::<f64>(&Coef::SHARED_RE, &shot(120.0, 16.3, 7097.0, 0.0), 0.137, None);
    let yd = r.carry_m / YARD;
    assert!(yd > 150.0 && yd < 200.0, "7번 캐리 {yd:.1} yd");
    assert!(r.apex_m > 10.0 && r.apex_m < 40.0, "최고 높이 {:.1} m", r.apex_m);
    assert!(r.land_deg > 20.0 && r.land_deg < 70.0, "착지각 {:.1}", r.land_deg);
}

/// 걸음을 줄이면 답이 수렴한다 (τ 0.137 대 0.01)
#[test]
fn step_converges() {
    for &(mph, vla, rpm, axis) in &[(142.0, 10.4, 4630.0, 0.0), (90.0, 22.0, 9000.0, 15.0), (109.0, 20.4, 8647.0, -25.0)] {
        let s = shot(mph, vla, rpm, axis);
        let a = fly::<f64>(&Coef::SHARED_RE, &s, 0.137, None);
        let b = fly::<f64>(&Coef::SHARED_RE, &s, 0.01, None);
        let dy = (a.carry_m - b.carry_m).abs() / YARD;
        assert!(dy < 0.01, "{mph} mph: 걸음 차이 {dy:.2e} yd");
    }
}

/// f32 는 f64 와 1 yd 안쪽이다
#[test]
fn f32_tracks_f64() {
    for &(mph, vla, rpm, axis) in &[(120.0, 16.3, 7097.0, 0.0), (169.0, 11.1, 2259.0, 10.0)] {
        let s = shot(mph, vla, rpm, axis);
        let a = fly::<f64>(&Coef::SHARED_RE, &s, 0.137, None);
        let b = fly::<f32>(&Coef::SHARED_RE, &s, 0.137, None);
        assert!((a.carry_m - b.carry_m).abs() / YARD < 1.0, "f32 차이 {:.3} yd", (a.carry_m - b.carry_m).abs() / YARD);
    }
}

/// 축 0 이면 좌우 대칭: 편차 0. 축 부호를 바꾸면 편차 부호가 바뀐다
#[test]
fn lateral_symmetry() {
    let s0 = fly::<f64>(&Coef::SHARED_RE, &shot(120.0, 16.0, 6000.0, 0.0), 0.137, None);
    assert!(s0.lateral_m.abs() < 1e-12);
    let p = fly::<f64>(&Coef::SHARED_RE, &shot(120.0, 16.0, 6000.0, 20.0), 0.137, None);
    let n = fly::<f64>(&Coef::SHARED_RE, &shot(120.0, 16.0, 6000.0, -20.0), 0.137, None);
    assert!(p.lateral_m < 0.0 && n.lateral_m > 0.0);
    assert!((p.lateral_m + n.lateral_m).abs() < 1e-9);
    assert!((p.carry_m - n.carry_m).abs() < 1e-9);
}

/// 경로 점 (PATH_STRIDE 개씩 평평한 배열): [t, x, y, z, vx, vy, vz]
fn pts(buf: &[f64]) -> Vec<&[f64]> {
    assert_eq!(buf.len() % PATH_STRIDE, 0);
    buf.chunks_exact(PATH_STRIDE).collect()
}

/// 경로는 시간이 늘고, 마지막 점은 착지점(y = 0)이다
#[test]
fn path_ends_on_ground() {
    let mut buf = Vec::new();
    let r = fly::<f64>(&Coef::SHARED_RE, &shot(109.0, 20.4, 8647.0, 0.0), 0.137, Some(&mut buf));
    let p = pts(&buf);
    assert!(p.len() > 10);
    assert!(p.windows(2).all(|w| w[1][0] > w[0][0]));
    let last = p.last().unwrap();
    assert!(last[2].abs() < 1e-4 && (last[1] - r.carry_m).abs() < 1e-9, "f64 경로 끝점은 착지 결과와 비트 수준으로 같다");
    assert!(last[4] > 0.0 && last[5] < 0.0, "착지 순간: 앞으로 가고 내려간다");
}

/// 바람이 있어도 경로의 마지막 점은 지면이고, 시간은 늘고, 끝점이 착지 결과와 같다
#[test]
fn path_ends_on_ground_with_wind() {
    let env = Env::from_conditions(20.0, 0.0, -6.0, 4.0);
    let mut buf = Vec::new();
    let r = fly_with::<f64>(&Coef::SHARED_RE, &env, &shot(120.0, 16.3, 7097.0, 0.0), 0.137, Some(&mut buf));
    let p = pts(&buf);
    assert!(p.windows(2).all(|w| w[1][0] > w[0][0]));
    let last = p.last().unwrap();
    assert!(last[2].abs() < 1e-4 && (last[1] - r.carry_m).abs() < 1e-9 && (last[3] - r.lateral_m).abs() < 1e-9);
    // 속도 끝점도 Landing::vel_ms 와 같다 (지면 기준)
    assert!((0..3).all(|i| (last[4 + i] - r.vel_ms[i]).abs() < 1e-9));
}

// ------------------------------------------------------------------ 속도 개선이 답을 바꾸지 않았다

/// 기본 환경은 지금까지의 API 와 비트 단위로 같다 (같은 코드 길)
#[test]
fn default_env_is_bitwise_legacy_api() {
    for s in shots(48) {
        for &h in &[0.137, 0.05] {
            let a = fly::<f64>(&Coef::SHARED_RE, &s, h, None);
            let b = fly_with::<f64>(&Coef::SHARED_RE, &Env::default(), &s, h, None);
            assert_eq!(
                [a.carry_m, a.lateral_m, a.apex_m, a.land_deg, a.time_s, a.vel_ms[0], a.vel_ms[1], a.vel_ms[2]].map(f64::to_bits),
                [b.carry_m, b.lateral_m, b.apex_m, b.land_deg, b.time_s, b.vel_ms[0], b.vel_ms[1], b.vel_ms[2]].map(f64::to_bits)
            );
            assert_eq!(a.steps, b.steps);
        }
    }
    let u = Env::default().units();
    assert_eq!((u.l_m.to_bits(), u.u_ms.to_bits(), u.t_s.to_bits(), u.re_scale.to_bits()), (L_M.to_bits(), U_MS.to_bits(), T_S.to_bits(), 1.0f64.to_bits()));
    assert_eq!(u.wind, [0.0; 3]);
}

/// 얼린 옛 판본(56회 이분법·나눗셈 3개 rhs)과 새 엔진: 같은 걸음, 같은 답 (재배열한 산술의 반올림 차이만)
#[test]
fn new_engine_matches_frozen_oracle() {
    for &h in &[0.137, 0.05, 0.01] {
        let (mut dc, mut ds, mut dh, mut dl, mut dt) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for s in shots(60) {
            let a = fly::<f64>(&Coef::SHARED_RE, &s, h, None);
            let o = oracle::fly::<f64>(&Coef::SHARED_RE, &s, h, None);
            assert_eq!(a.steps, o.steps, "걸음 수가 달라졌다: {s:?}");
            dc = dc.max((a.carry_m - o.carry_m).abs() / YARD);
            ds = ds.max((a.lateral_m - o.lateral_m).abs() / YARD);
            dh = dh.max((a.apex_m - o.apex_m).abs() / YARD);
            dl = dl.max((a.land_deg - o.land_deg).abs());
            dt = dt.max((a.time_s - o.time_s).abs());
        }
        assert!(dc < 1e-9 && ds < 1e-9 && dh < 1e-9 && dl < 1e-9 && dt < 1e-9, "τ {h}: 캐리 {dc:.2e} 편차 {ds:.2e} 높이 {dh:.2e} 각 {dl:.2e} 시간 {dt:.2e}");
        // 실제로는 반올림 수준이다
        assert!(dc < 1e-11, "τ {h}: 캐리 차이 {dc:.2e} yd");
    }
    // f32 도 옛 판본과 f32 바닥 안쪽에서 같다
    for s in shots(60) {
        let a = fly::<f32>(&Coef::SHARED_RE, &s, 0.137, None);
        let o = oracle::fly::<f32>(&Coef::SHARED_RE, &s, 0.137, None);
        assert!((a.carry_m - o.carry_m).abs() / YARD < 2e-3, "f32 캐리 {:.2e}", (a.carry_m - o.carry_m).abs() / YARD);
    }
}

/// 뉴턴 뿌리가 56회 이분법과 같다 (임의 3차식)
#[test]
fn hroot_matches_bisection() {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..5000 {
        let y0 = 0.001 + 0.3 * rnd();
        let y1 = -0.3 * rnd();
        // 단조 감소 쪽: 끝 기울기는 음수, 크기는 평균 기울기 둘레
        let avg = y1 - y0;
        let m0 = avg * (0.3 + 1.4 * rnd());
        let m1 = avg * (0.3 + 1.4 * rnd());
        let c = hcoef(y0, m0, y1, m1);
        let th = hroot(&c);
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if hval(&c, mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        // 단조 감소라서 뿌리는 하나
        assert!((th - 0.5 * (lo + hi)).abs() < 1e-11, "뿌리 {th} 대 이분법 {}", 0.5 * (lo + hi));
    }
}

// ------------------------------------------------------------------ 환경

/// 환경 값: ISA 해수면 15 °C 는 ρ = 1.225, 점성 1.79e-5; 1600 m 20 °C
#[test]
fn from_conditions_values() {
    let e = Env::from_conditions(15.0, 0.0, 0.0, 0.0);
    assert!((e.rho - 1.225).abs() < 1.5e-3, "ρ(15 °C, 0 m) = {}", e.rho);
    assert!((e.mu - 1.789e-5).abs() < 0.01e-5, "μ(15 °C) = {}", e.mu);
    assert!((isa_pressure(0.0) - 101_325.0).abs() < 1e-9);
    assert!((isa_pressure(1600.0) - 83_500.0).abs() < 400.0, "p(1600 m) = {}", isa_pressure(1600.0));
    let hi = Env::from_conditions(20.0, 1600.0, 0.0, 0.0);
    assert!((hi.rho - 0.9925).abs() < 0.01, "ρ(20 °C, 1600 m) = {}", hi.rho);
    // 기본 환경은 해수면 20 °C 근처
    let lo = Env::from_conditions(20.0, 0.0, 0.0, 0.0);
    assert!((lo.rho - RHO0).abs() / RHO0 < 0.005 && (lo.mu - MU0).abs() / MU0 < 0.005, "{lo:?}");
    // 바람 부호 규약: [앞, 위, 오른쪽]
    let w = Env::from_conditions(15.0, 0.0, 3.0, -2.0);
    assert_eq!(w.wind, [3.0, 0.0, -2.0]);
}

/// 환산: 길이 ∝ 1/ρ, 속도 ∝ ρ^{-1/2}, 위기 속도는 Re 비로 움직인다
#[test]
fn units_scaling() {
    let e = Env { wind: [4.0, 0.0, 0.0], rho: 0.9, mu: 1.7e-5 };
    let u = e.units();
    assert!((u.l_m / L_M - RHO0 / 0.9).abs() < 1e-14);
    assert!((u.u_ms / U_MS - (RHO0 / 0.9f64).sqrt()).abs() < 1e-14);
    assert!((u.t_s - u.l_m / u.u_ms).abs() < 1e-12);
    assert!((u.wind[0] * u.u_ms - 4.0).abs() < 1e-12);
    // 위기 속도(물리 단위)에서 레이놀즈 수는 환경과 무관하게 Coef 의 Re0
    let re0 = Coef::SHARED_RE.u0 * RE_PER_U;
    let p = P::<f64>::new(&Coef::SHARED_RE, &u);
    let u0_dimless = (1.0 / p.iu02).sqrt();
    let re = e.rho * (u0_dimless * u.u_ms) * 0.04267 / e.mu;
    assert!((re - re0).abs() / re0 < 1e-12, "위기 Re {re}");
    let pd = P::<f64>::new(&Coef::SHARED_RE, &Env::default().units());
    let re_d = RHO0 * ((1.0 / pd.iu02).sqrt() * U_MS) * 0.04267 / MU0;
    assert!((re_d - re0).abs() / re0 < 1e-12, "기본 환경 위기 Re {re_d}");
}

/// 갈릴레이 불변: 수평 바람 (wf, wr) 에서 날린 공은 바람과 함께 움직이는 좌표계의 무풍 비행과 같다.
///   캐리_풍 = 캐리_무풍(u0 - w) + wf·T,  편차_풍 = 편차_무풍 + wr·T,  비행 시간·정점·수직 속도 같음, 착지 속도는 w 만큼 이동
#[test]
fn galilean_invariance_horizontal_wind() {
    let cases = [
        (DRIVER, 15.0, 0.0, 7.0, -3.0, 0.137),
        (DRIVER, 15.0, 0.0, -9.0, 5.5, 0.01),
        (shot(120.0, 16.3, 7097.0, 12.0), 25.0, 1600.0, -4.0, -6.0, 0.05),
        (shot(90.0, 22.0, 9000.0, -15.0), 5.0, 800.0, 11.0, 2.0, 0.137),
        (shot(150.0, 8.0, 3500.0, 4.0), 30.0, 0.0, -12.0, 8.0, 0.01),
    ];
    for (s, temp, alt, wf, wr, h) in cases {
        let env = Env::from_conditions(temp, alt, wf, wr);
        let still = Env { wind: [0.0; 3], ..env };
        let un = env.units();
        let p_w = P::<f64>::new(&Coef::SHARED_RE, &un);
        let p_s = P::<f64>::new(&Coef::SHARED_RE, &still.units());
        let init = Init::from_shot(&s, &un);
        // 움직이는 좌표계의 초기 속도 = 지면 속도 - 바람 (스핀 w 는 물리량 Rω/U 라 그대로)
        let moving = Init { u: [init.u[0] - un.wind[0], init.u[1], init.u[2] - un.wind[2]], ..init };
        let rw = fly_init::<f64>(&p_w, &un, true, false, &init, h, None);
        let rs = fly_init::<f64>(&p_s, &still.units(), false, false, &moving, h, None);
        let t = rs.time_s;
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1.0);
        assert_eq!(rw.steps, rs.steps);
        assert!(rel(rw.time_s, rs.time_s) < 1e-9, "비행 시간 {} {}", rw.time_s, rs.time_s);
        assert!(rel(rw.carry_m, rs.carry_m + wf * t) < 1e-9, "캐리 {} 대 {}", rw.carry_m, rs.carry_m + wf * t);
        assert!(rel(rw.lateral_m, rs.lateral_m + wr * t) < 1e-9, "편차 {} 대 {}", rw.lateral_m, rs.lateral_m + wr * t);
        assert!(rel(rw.apex_m, rs.apex_m) < 1e-9, "정점");
        // 착지 속도: 이동 좌표계에서 본 값이 무풍 값과 같다 (그래서 이동 좌표계의 착지각도 같다)
        assert!(rel(rw.vel_ms[0] - wf, rs.vel_ms[0]) < 1e-9 && rel(rw.vel_ms[1], rs.vel_ms[1]) < 1e-9 && rel(rw.vel_ms[2] - wr, rs.vel_ms[2]) < 1e-9);
        let deg = (-rw.vel_ms[1]).atan2(rw.vel_ms[0] - wf).to_degrees();
        assert!(rel(deg, rs.land_deg) < 1e-9, "이동 좌표계 착지각 {deg} 대 {}", rs.land_deg);
    }
}

/// 방향별 단조성: 얇은 공기(고도↑, 따뜻함↑)는 멀리, 짙고 찬 공기는 가까이, 맞바람은 짧게, 뒷바람은 길게, 왼쪽 바람은 오른쪽으로
#[test]
fn environment_monotonic() {
    for s in [DRIVER, shot(120.0, 16.3, 7097.0, 0.0), shot(100.0, 22.0, 8500.0, 0.0)] {
        let base = carry_yd(&Env::from_conditions(15.0, 0.0, 0.0, 0.0), &s, 0.05);
        // 고도
        // 느린 공(100 mph 웨지)은 고도에서 Re 가 내려가 가파른 위기(n = 12)를 건너면 오히려 짧아질 수 있다(밀도 효과가 지는 구간, 검증 안 됨).
        let mut prev = base;
        let mut line = format!("{} mph 고도별 캐리:", s.mph);
        for alt in [500.0, 1000.0, 1600.0, 2500.0] {
            let c = carry_yd(&Env::from_conditions(15.0, alt, 0.0, 0.0), &s, 0.05);
            line += &format!(" {alt} m {c:.2}");
            if s.mph > 110.0 {
                assert!(c > prev, "고도 {alt} m: {c:.2} <= {prev:.2}");
            } else {
                assert!(c > prev - 0.5, "고도 {alt} m: {c:.2} << {prev:.2}");
            }
            prev = c;
        }
        println!("{line}");
        // 기온: 따뜻할수록 공기가 얇다. 다만 점성도 오르므로 Re 가 내려가 항력 위기 속도가 올라간다(느린 공은 항력 계수가 커진다).
        // 빠른 공(드라이버·7번)은 밀도 효과가 이겨 단조, 느린 공(100 mph)은 20–40 °C 에서 거의 평평하다(아래 출력).
        let mut prev = carry_yd(&Env::from_conditions(-10.0, 0.0, 0.0, 0.0), &s, 0.05);
        let fast = s.mph > 110.0;
        let mut line = format!("{} mph 기온별 캐리:", s.mph);
        for t in [0.0, 10.0, 20.0, 30.0, 40.0] {
            let c = carry_yd(&Env::from_conditions(t, 0.0, 0.0, 0.0), &s, 0.05);
            line += &format!(" {t}°C {c:.2}");
            if fast {
                assert!(c > prev, "{t} °C: {c:.2} <= {prev:.2}");
            } else {
                assert!(c > prev - 0.8, "{t} °C: {c:.2} << {prev:.2}"); // 가파른 위기(n = 12) 에서는 0.5 yd 까지 줄어든다
            }
            prev = c;
        }
        println!("{line}");
        // 맞바람 / 뒷바람
        let calm = carry_yd(&Env::from_conditions(15.0, 0.0, 0.0, 0.0), &s, 0.05);
        let mut prev_h = calm;
        let mut prev_t = calm;
        for w in [2.0, 5.0, 10.0] {
            let head = carry_yd(&Env::from_conditions(15.0, 0.0, -w, 0.0), &s, 0.05);
            let tail = carry_yd(&Env::from_conditions(15.0, 0.0, w, 0.0), &s, 0.05);
            assert!(head < prev_h && tail > prev_t, "{w} m/s: 맞바람 {head:.2} 뒷바람 {tail:.2} 무풍 {calm:.2}");
            prev_h = head;
            prev_t = tail;
        }
        // 맞바람이 뒷바람보다 더 깎는다 (비선형)
        let (h10, t10) = (carry_yd(&Env::from_conditions(15.0, 0.0, -10.0, 0.0), &s, 0.05), carry_yd(&Env::from_conditions(15.0, 0.0, 10.0, 0.0), &s, 0.05));
        assert!(calm - h10 > t10 - calm, "맞바람 손실 {:.2} <= 뒷바람 이득 {:.2}", calm - h10, t10 - calm);
    }
    // 왼쪽에서 부는 바람(wind_right > 0) → 오른쪽(편차 +); 오른쪽에서 부는 바람 → 왼쪽
    let l = fly_with::<f64>(&Coef::SHARED_RE, &Env::from_conditions(15.0, 0.0, 0.0, 5.0), &DRIVER, 0.05, None);
    let r = fly_with::<f64>(&Coef::SHARED_RE, &Env::from_conditions(15.0, 0.0, 0.0, -5.0), &DRIVER, 0.05, None);
    assert!(l.lateral_m > 1.0 && r.lateral_m < -1.0, "횡풍 편차 {} / {}", l.lateral_m, r.lateral_m);
    assert!((l.lateral_m + r.lateral_m).abs() < 1e-9 * l.lateral_m.abs() + 1e-9, "횡풍은 좌우 대칭");
}

/// 크기 점검 (맞추지 않고 보고만): 해수면 15 °C 대 고도 1600 m 20 °C, 드라이버 167 mph / 10.9° / 2686 rpm
#[test]
fn altitude_magnitude() {
    let sea = Env::from_conditions(15.0, 0.0, 0.0, 0.0);
    let high = Env::from_conditions(20.0, 1600.0, 0.0, 0.0);
    let (a, b) = (carry_yd(&sea, &DRIVER, 0.05), carry_yd(&high, &DRIVER, 0.05));
    let pct = (b / a - 1.0) * 100.0;
    println!("드라이버 캐리: 해수면 15 °C {a:.2} yd (ρ {:.4}), 1600 m 20 °C {b:.2} yd (ρ {:.4}): {:+.2} yd = {pct:+.2} %", sea.rho, high.rho, b - a);
    assert!(pct > 2.0 && pct < 15.0, "고도 효과 {pct:.2} %");
}

/// 환경이 기본값이면 항력 위기 속도가 움직이지 않는다: 같은 ρ, μ 를 Env 로 명시해도 같다. ρ, μ 만 따로 바꿔도 단조
#[test]
fn viscosity_moves_crisis() {
    let base = Env::default();
    let a = carry_yd(&base, &DRIVER, 0.05);
    let b = carry_yd(&Env { mu: MU0, rho: RHO0, wind: [0.0; 3] }, &DRIVER, 0.05);
    assert_eq!(a.to_bits(), b.to_bits());
    // 점성만 높이면 같은 속도에서 Re 가 낮아져 위기 속도가 오른다 → 결과가 달라진다
    let c = carry_yd(&Env { mu: 2.0 * MU0, ..base }, &DRIVER, 0.05);
    assert!((c - a).abs() > 0.05, "점성이 결과에 안 들어간다: {a} {c}");
}

// ------------------------------------------------------------------ 퇴화 입력

#[test]
fn degenerate_inputs() {
    let flat = fly::<f64>(&Coef::SHARED_RE, &shot(100.0, 0.0, 3000.0, 0.0), 0.137, None);
    assert_eq!(flat.carry_m, 0.0);
    let down = fly::<f64>(&Coef::SHARED_RE, &shot(100.0, -10.0, 3000.0, 0.0), 0.137, None);
    assert_eq!(down.carry_m, 0.0);
    assert!(down.land_deg > 9.0 && down.land_deg < 11.0);
    let nan = fly::<f64>(&Coef::SHARED_RE, &shot(f64::NAN, 10.0, 3000.0, 0.0), 0.137, None);
    assert!(nan.carry_m.is_nan());
    let none = batch_scalar::<f64>(&Coef::SHARED_RE, &Env::default(), &[100.0, 12.0, 3000.0, 0.0], 0.0);
    assert!(none[0].is_nan());
}

// ------------------------------------------------------------------ 4레인 커널

fn max_diff(a: &[f64], b: &[f64]) -> (f64, f64, f64, f64) {
    let mut m = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.chunks(4).zip(b.chunks(4)) {
        m.0 = m.0.max((x[0] - y[0]).abs() / YARD);
        m.1 = m.1.max((x[1] - y[1]).abs() / YARD);
        m.2 = m.2.max((x[2] - y[2]).abs() / YARD);
        m.3 = m.3.max((x[3] - y[3]).abs());
    }
    m
}

/// 레인 커널(이식 가능판)은 스칼라 f32 와 같은 연산이라 같은 답이다. 샷 수가 4의 배수가 아니어도, 퇴화 샷이 섞여도 된다.
#[test]
fn lanes_match_scalar_f32() {
    for &n in &[0usize, 1, 2, 3, 4, 5, 7, 9, 64, 203] {
        let sh = shots(n);
        let f = flat(&sh);
        for &h in &[0.137, 0.05] {
            let a = batch_lanes::<P4>(&Coef::SHARED_RE, &Env::default(), &f, h);
            let b = batch_scalar::<f32>(&Coef::SHARED_RE, &Env::default(), &f, h);
            assert_eq!(a.len(), b.len());
            let d = max_diff(&a, &b);
            assert!(d.0 < 1e-6 && d.1 < 1e-6 && d.2 < 1e-6 && d.3 < 1e-6, "n {n} τ {h}: 차이 {d:?}");
        }
    }
    // 위로 나가지 않는 샷과 NaN 샷이 중간에 끼어도 나머지는 그대로
    let mut f = flat(&shots(10));
    f[4 * 3 + 1] = -5.0;
    f[4 * 6] = f64::NAN;
    let a = batch_lanes::<P4>(&Coef::SHARED_RE, &Env::default(), &f, 0.137);
    let b = batch_scalar::<f32>(&Coef::SHARED_RE, &Env::default(), &f, 0.137);
    for i in 0..10 {
        for j in 0..4 {
            let (x, y) = (a[4 * i + j], b[4 * i + j]);
            assert!((x.is_nan() && y.is_nan()) || (x - y).abs() < 1e-5, "샷 {i} 성분 {j}: {x} {y}");
        }
    }
    assert_eq!(a[12], 0.0);
    assert!(a[24].is_nan());
}

/// 레인 커널은 f64 기준과 f32 바닥 안쪽에서 같다 (캐리 4e-4 yd: τ 0.05 에서 f32 바닥 2e-4 근처)
#[test]
fn lanes_track_f64() {
    let sh = shots(400);
    let f = flat(&sh);
    let a = batch_lanes::<P4>(&Coef::SHARED_RE, &Env::default(), &f, 0.05);
    let b = batch_scalar::<f64>(&Coef::SHARED_RE, &Env::default(), &f, 0.05);
    let d = max_diff(&a, &b);
    assert!(d.0 < 4e-4 && d.1 < 4e-4 && d.2 < 1e-3 && d.3 < 1e-3, "차이 {d:?}");
}

/// 환경(밀도·점성·바람)이 레인 커널에도 같게 들어간다
#[test]
fn lanes_with_environment() {
    let sh = shots(150);
    let f = flat(&sh);
    for env in [Env::from_conditions(30.0, 1800.0, 0.0, 0.0), Env::from_conditions(5.0, 0.0, -7.0, 4.0), Env::from_conditions(20.0, 600.0, 9.0, -6.0)] {
        let a = batch_lanes::<P4>(&Coef::SHARED_RE, &env, &f, 0.137);
        let b = batch_scalar::<f32>(&Coef::SHARED_RE, &env, &f, 0.137);
        let d = max_diff(&a, &b);
        assert!(d.0 < 1e-6 && d.1 < 1e-6 && d.2 < 1e-6 && d.3 < 1e-6, "{env:?}: 차이 {d:?}");
        let c = batch_scalar::<f64>(&Coef::SHARED_RE, &env, &f, 0.137);
        let d = max_diff(&a, &c);
        assert!(d.0 < 1e-3 && d.1 < 1e-3, "{env:?}: f64 대비 {d:?}");
    }
}

/// 걸음이 잘못되면(0) 커널이 돌아오고 NaN 이다
#[test]
fn lanes_reject_bad_step() {
    let a = batch_lanes::<P4>(&Coef::SHARED_RE, &Env::default(), &[100.0, 12.0, 3000.0, 0.0], 0.0);
    assert!(a.iter().all(|x| x.is_nan()));
}

// ------------------------------------------------------------------ 실험 (stretch): 오차가 어디서 오나
/// 착지 보간 대신 마지막 부분 걸음을 RK4 로 (Hénon 식)
#[test]
#[ignore]
fn exp_landing_substep() {
    let un = Env::default().units();
    let p = P::<f64>::new(&Coef::SHARED_RE, &un);
    let sh = shots(4000);
    let run_ref = |h: f64| -> Vec<f64> { sh.iter().map(|s| fly::<f64>(&Coef::SHARED_RE, s, h, None).carry_m / YARD).collect() };
    let refv = run_ref(0.002);
    for &h in &[0.137, 0.1, 0.07, 0.05] {
        let (mut e_cur, mut e_sub, mut e_sub1) = (0.0f64, 0.0f64, 0.0f64);
        for (i, s) in sh.iter().enumerate() {
            let init = Init::from_shot(s, &un);
            let (oy, oz) = (init.oy, init.oz);
            let mut st = S { t: 0.0, x: 0.0, y: 0.0, z: 0.0, ux: init.u[0], uy: init.u[1], uz: init.u[2], w: init.w };
            let mut k = rhs::<f64, false>(&p, oy, oz, st.ux, st.uy, st.uz, st.w);
            let (hh, h6) = (0.5 * h, h / 6.0);
            loop {
                let (s1, k1) = rk4::<f64, false>(&p, oy, oz, &st, &k, h, hh, h6);
                if st.y > 0.0 && s1.y <= 0.0 {
                    let td = touchdown(&st, &k, &s1, &k1, h);
                    e_cur = e_cur.max((td.x * un.l_m / YARD - refv[i]).abs());
                    // 부분 걸음: 보간 뿌리 θ 에서 RK4 한 걸음 (θh)
                    let th = hroot(&hcoef(st.y, k.vy * h, s1.y, k1.vy * h));
                    let hs = th * h;
                    let (sa, ka) = rk4::<f64, false>(&p, oy, oz, &st, &k, hs, 0.5 * hs, hs / 6.0);
                    e_sub = e_sub.max((sa.x * un.l_m / YARD - refv[i]).abs());
                    // y 잔차를 속도로 한 번 보정
                    let corr = sa.x - sa.y * (ka.vx / ka.vy);
                    e_sub1 = e_sub1.max((corr * un.l_m / YARD - refv[i]).abs());
                    break;
                }
                st = s1;
                k = k1;
            }
        }
        println!("τ {h}: 캐리 최대 오차 현재(에르미트) {e_cur:.2e}  부분걸음 RK4 {e_sub:.2e}  +y 보정 {e_sub1:.2e}");
    }
}

/// 실험: 재매개 지수 p (dτ = |u|^p dt) 별 (평균 걸음 수, 캐리 최대 오차)
#[test]
#[ignore]
fn exp_reparam_exponent() {
    let un = Env::default().units();
    let sh = shots(1000);
    let c = Coef::SHARED_RE;
    let iu02 = 1.0 / (c.u0 * c.u0);
    // 일반 지수 rhs (느려도 됨)
    let rhs_p = |pp: f64, oy: f64, oz: f64, ux: f64, uy: f64, uz: f64, w: f64| -> [f64; 8] {
        let q = ux * ux + uy * uy + uz * uz;
        let sp = q.sqrt();
        let ur = w * sp / (sp + c.l * w);
        let q2 = q * iu02;
        let cr = c.e / (1.0 + q2 * q2);
        let kd = (c.v + cr) * sp + c.d * ur;
        let kl = c.c * ur;
        let g = sp.powf(-pp);
        let (cx, cy, cz) = (oy * uz - oz * uy, oz * ux, -(oy * ux));
        [g, ux * g, uy * g, uz * g, (kl * cx - kd * ux) * g, (kl * cy - kd * uy - 1.0) * g, (kl * cz - kd * uz) * g, -(c.beta * w * sp) * g]
    };
    let fly_p = |pp: f64, s: &Shot, h: f64| -> (f64, u32) {
        let init = Init::from_shot(s, &un);
        let (oy, oz) = (init.oy, init.oz);
        let mut st = [0.0, 0.0, 0.0, 0.0, init.u[0], init.u[1], init.u[2], init.w];
        let mut k = rhs_p(pp, oy, oz, st[4], st[5], st[6], st[7]);
        let mut steps = 0;
        loop {
            steps += 1;
            if steps > 5000 {
                return (f64::NAN, steps);
            }
            let st2: Vec<f64> = (0..8).map(|i| st[i] + 0.5 * h * k[i]).collect();
            let k2 = rhs_p(pp, oy, oz, st2[4], st2[5], st2[6], st2[7]);
            let st3: Vec<f64> = (0..8).map(|i| st[i] + 0.5 * h * k2[i]).collect();
            let k3 = rhs_p(pp, oy, oz, st3[4], st3[5], st3[6], st3[7]);
            let st4: Vec<f64> = (0..8).map(|i| st[i] + h * k3[i]).collect();
            let k4 = rhs_p(pp, oy, oz, st4[4], st4[5], st4[6], st4[7]);
            let mut n = [0.0; 8];
            for i in 0..8 {
                n[i] = st[i] + h / 6.0 * (k[i] + 2.0 * (k2[i] + k3[i]) + k4[i]);
            }
            let k5 = rhs_p(pp, oy, oz, n[4], n[5], n[6], n[7]);
            if st[2] > 0.0 && n[2] <= 0.0 {
                let th = hroot(&hcoef(st[2], k[2] * h, n[2], k5[2] * h));
                let x = hval(&hcoef(st[1], k[1] * h, n[1], k5[1] * h), th);
                return (x * un.l_m / YARD, steps);
            }
            st = n;
            k = k5;
        }
    };
    let refv: Vec<f64> = sh.iter().map(|s| fly_p(0.5, s, 0.002).0).collect();
    for &pp in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut line = format!("p={pp}:");
        for &h in &[0.4, 0.3, 0.2, 0.15, 0.1] {
            // τ 단위가 p 마다 달라 걸음은 h 로 직접 맞춘다 (평균 걸음 수로 비교)
            let (mut emax, mut ns) = (0.0f64, 0.0f64);
            for (i, s) in sh.iter().enumerate() {
                let (x, st) = fly_p(pp, s, h);
                emax = if x.is_nan() { f64::INFINITY } else { emax.max((x - refv[i]).abs()) };
                ns += st as f64;
            }
            line += &format!("  [h {h}: 걸음 {:.1}, 오차 {emax:.1e}]", ns / sh.len() as f64);
        }
        println!("{line}");
    }
}

/// 실험: τ 0.137 오차가 샷의 어떤 특징과 붙어 있나
#[test]
#[ignore]
fn exp_error_vs_shot() {
    let sh = shots(4000);
    let mut v: Vec<(f64, Shot, u32, f64)> = sh
        .iter()
        .map(|s| {
            let a = fly::<f64>(&Coef::SHARED_RE, s, 0.137, None);
            let r = fly::<f64>(&Coef::SHARED_RE, s, 0.002, None);
            ((a.carry_m - r.carry_m).abs() / YARD, *s, a.steps, r.carry_m / YARD)
        })
        .collect();
    let mean: f64 = v.iter().map(|x| x.0).sum::<f64>() / v.len() as f64;
    println!("평균 오차 {mean:.2e}");
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    for x in v.iter().take(10) {
        println!("오차 {:.2e}  mph {:.0} vla {:.1} rpm {:.0} axis {:.0}  걸음 {} 캐리 {:.0}", x.0, x.1.mph, x.1.vla_deg, x.1.rpm, x.1.axis_deg, x.2, x.3);
    }
    // 구간별 최대 오차 (볼스피드)
    for lo in (60..180).step_by(20) {
        let m = v.iter().filter(|x| x.1.mph >= lo as f64 && x.1.mph < lo as f64 + 20.0).map(|x| x.0).fold(0.0f64, f64::max);
        let st = v.iter().filter(|x| x.1.mph >= lo as f64 && x.1.mph < lo as f64 + 20.0).map(|x| x.2 as f64).sum::<f64>() / v.iter().filter(|x| x.1.mph >= lo as f64 && x.1.mph < lo as f64 + 20.0).count() as f64;
        println!("{lo}-{} mph: 최대 오차 {m:.2e}, 평균 걸음 {st:.1}", lo + 20);
    }
}

/// 실험: 비행 τ 길이 추정 (진공 비행 시간 × √속도) 대 실제, 그리고 최소 걸음 수 규칙의 효과
#[test]
#[ignore]
fn exp_min_steps_rule() {
    let un = Env::default().units();
    let sh = shots(4000);
    let refc: Vec<f64> = sh.iter().map(|s| fly::<f64>(&Coef::SHARED_RE, s, 0.002, None).carry_m / YARD).collect();
    let tau_est = |s: &Shot| {
        let i = Init::from_shot(s, &un);
        let sp = (i.u[0] * i.u[0] + i.u[1] * i.u[1]).sqrt();
        2.0 * i.u[1] * sp.sqrt()
    };
    // 실제 τ 길이: 작은 걸음의 걸음 수 × 걸음
    let (mut rmin, mut rmax) = (f64::INFINITY, 0.0f64);
    for s in &sh {
        let r = fly::<f64>(&Coef::SHARED_RE, s, 0.01, None);
        let tau = r.steps as f64 * 0.01;
        let q = tau / tau_est(s);
        rmin = rmin.min(q);
        rmax = rmax.max(q);
    }
    println!("τ 실제/추정 비: {rmin:.3} .. {rmax:.3}");
    for &nmin in &[0.0f64, 6.0, 8.0, 10.0, 12.0, 16.0] {
        for &h in &[0.137f64, 0.1] {
            let (mut emax, mut steps) = (0.0f64, 0.0f64);
            for (i, s) in sh.iter().enumerate() {
                let he = if nmin > 0.0 { h.min(tau_est(s) * 0.8 / nmin) } else { h };
                let r = fly::<f64>(&Coef::SHARED_RE, s, he, None);
                emax = emax.max((r.carry_m / YARD - refc[i]).abs());
                steps += r.steps as f64;
            }
            println!("최소걸음 {nmin:>4} h {h}: 평균 걸음 {:.2}, 캐리 최대 오차 {emax:.2e}", steps / sh.len() as f64);
        }
    }
}

// ------------------------------------------------------------------ 선택 사항: 최소 걸음 수

/// 짧은 비행의 최소 걸음 수 보장: 기본값(0)은 그대로, 켜면 τ 0.137 의 최대 오차가 줄고 평균 걸음은 조금만 는다
#[test]
fn min_steps_option() {
    let sh = shots(1500);
    let env = Env::default();
    let (mut e0, mut e1, mut s0, mut s1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let on = Opts { min_steps: 6.0, compensated: false };
    for s in &sh {
        let r = fly_with::<f64>(&Coef::SHARED_RE, &env, s, 0.002, None).carry_m / YARD;
        let a = fly_with::<f64>(&Coef::SHARED_RE, &env, s, 0.137, None);
        let b = fly_with_opts::<f64>(&Coef::SHARED_RE, &env, &on, s, 0.137, None);
        let off = fly_with_opts::<f64>(&Coef::SHARED_RE, &env, &Opts::default(), s, 0.137, None);
        assert_eq!(a.carry_m.to_bits(), off.carry_m.to_bits(), "기본값은 옛 동작과 비트까지 같다");
        e0 = e0.max((a.carry_m / YARD - r).abs());
        e1 = e1.max((b.carry_m / YARD - r).abs());
        s0 += a.steps as f64;
        s1 += b.steps as f64;
    }
    println!("최소 걸음: 최대 오차 {e0:.2e} → {e1:.2e} yd, 평균 걸음 {:.2} → {:.2}", s0 / sh.len() as f64, s1 / sh.len() as f64);
    assert!(e1 < 0.85 * e0 && e1 < 5e-4, "오차 {e0:.2e} → {e1:.2e}");
    assert!(s1 < 1.08 * s0, "걸음 {s0} → {s1}");
}

/// 최소 걸음 수 규칙이 4레인 커널에도 같게 들어간다 (레인마다 걸음이 다르다)
#[test]
fn lanes_min_steps_match_scalar() {
    let sh = shots(130);
    let f = flat(&sh);
    let on = Opts { min_steps: 6.0, compensated: false };
    for env in [Env::default(), Env::from_conditions(10.0, 900.0, -5.0, 4.0)] {
        let a = lanes::batch_lanes_opts::<P4>(&Coef::SHARED_RE, &env, &on, &f, 0.137);
        let b = batch_scalar_opts::<f32>(&Coef::SHARED_RE, &env, &on, &f, 0.137);
        let d = max_diff(&a, &b);
        assert!(d.0 < 1e-6 && d.1 < 1e-6 && d.2 < 1e-6 && d.3 < 1e-6, "차이 {d:?}");
        // 규칙이 실제로 걸음을 바꿨는가
        let c = lanes::batch_lanes_opts::<P4>(&Coef::SHARED_RE, &env, &Opts::default(), &f, 0.137);
        assert!(max_diff(&a, &c).0 > 1e-5);
    }
}

/// 묶음 수 G 를 바꿔도 답은 같다
#[test]
fn lanes_group_count_independent() {
    let f = flat(&shots(77));
    let g1 = lanes::batch_lanes_g::<P4, 1>(&Coef::SHARED_RE, &Env::default(), &Opts::default(), &f, 0.137);
    for r in [lanes::batch_lanes_g::<P4, 2>(&Coef::SHARED_RE, &Env::default(), &Opts::default(), &f, 0.137), lanes::batch_lanes_g::<P4, 3>(&Coef::SHARED_RE, &Env::default(), &Opts::default(), &f, 0.137), lanes::batch_lanes_g::<P4, 4>(&Coef::SHARED_RE, &Env::default(), &Opts::default(), &f, 0.137)] {
        assert_eq!(g1.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), r.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    }
}

/// 실험: 최소 걸음 규칙과 큰 τ 의 조합 (걸음 수 대 오차)
#[test]
#[ignore]
fn exp_min_steps_with_larger_h() {
    let sh = shots(4000);
    let env = Env::default();
    let refc: Vec<f64> = sh.iter().map(|s| fly::<f64>(&Coef::SHARED_RE, s, 0.002, None).carry_m / YARD).collect();
    for &nmin in &[0.0f64, 6.0, 8.0] {
        for &h in &[0.1f64, 0.12, 0.137, 0.15, 0.17, 0.2] {
            let (mut emax, mut steps, mut erms) = (0.0f64, 0.0f64, 0.0f64);
            for (i, s) in sh.iter().enumerate() {
                let r = fly_with_opts::<f64>(&Coef::SHARED_RE, &env, &Opts { min_steps: nmin, compensated: false }, s, h, None);
                let e = (r.carry_m / YARD - refc[i]).abs();
                emax = emax.max(e);
                erms += e * e;
                steps += r.steps as f64;
            }
            println!("min {nmin:>3} h {h:<5}: 평균 걸음 {:>5.2}  캐리 최대 {emax:.2e}  RMS {:.2e}", steps / sh.len() as f64, (erms / sh.len() as f64).sqrt());
        }
    }
}

/// 실험: f32 바닥이 어디서 오나 — 누적 합 보상(Kahan)을 위치만 / 전부에 넣어 본다
#[test]
#[ignore]
fn exp_f32_compensated() {
    let un = Env::default().units();
    let p = P::<f32>::new(&Coef::SHARED_RE, &un);
    let sh = shots(4000);
    let refc: Vec<f64> = sh.iter().map(|s| fly::<f64>(&Coef::SHARED_RE, s, 0.002, None).carry_m / YARD).collect();
    // mode 0: 현재, 1: 위치(t x y z)만 보상, 2: 전부 보상
    let run = |mode: u32, h: f64, s: &Shot| -> f64 {
        let init = Init::from_shot(s, &un);
        let (oy, oz) = (init.oy as f32, init.oz as f32);
        let mut st = S { t: 0.0f32, x: 0.0, y: 0.0, z: 0.0, ux: init.u[0] as f32, uy: init.u[1] as f32, uz: init.u[2] as f32, w: init.w as f32 };
        let mut cmp = [0.0f32; 8];
        let hv = h as f32;
        let (hh, h6) = (hv * 0.5, hv / 6.0);
        let mut k = rhs::<f32, false>(&p, oy, oz, st.ux, st.uy, st.uz, st.w);
        loop {
            // RK4 증분 (보상 없이) — rk4 를 쓰되 증분은 새 상태 - 옛 상태 대신 직접 계산한다
            let k2 = rhs::<f32, false>(&p, oy, oz, st.ux + hh * k.ax, st.uy + hh * k.ay, st.uz + hh * k.az, st.w + hh * k.aw);
            let k3 = rhs::<f32, false>(&p, oy, oz, st.ux + hh * k2.ax, st.uy + hh * k2.ay, st.uz + hh * k2.az, st.w + hh * k2.aw);
            let k4 = rhs::<f32, false>(&p, oy, oz, st.ux + hv * k3.ax, st.uy + hv * k3.ay, st.uz + hv * k3.az, st.w + hv * k3.aw);
            let comb = |a: f32, b: f32, c: f32, d: f32| h6 * (a + 2.0 * (b + c) + d);
            let inc = [comb(k.g, k2.g, k3.g, k4.g), comb(k.vx, k2.vx, k3.vx, k4.vx), comb(k.vy, k2.vy, k3.vy, k4.vy), comb(k.vz, k2.vz, k3.vz, k4.vz), comb(k.ax, k2.ax, k3.ax, k4.ax), comb(k.ay, k2.ay, k3.ay, k4.ay), comb(k.az, k2.az, k3.az, k4.az), comb(k.aw, k2.aw, k3.aw, k4.aw)];
            let mut v = [st.t, st.x, st.y, st.z, st.ux, st.uy, st.uz, st.w];
            let old = v;
            for i in 0..8 {
                let use_c = (mode == 1 && i < 4) || mode == 2;
                if use_c {
                    let y = inc[i] - cmp[i];
                    let t = v[i] + y;
                    cmp[i] = (t - v[i]) - y;
                    v[i] = t;
                } else {
                    v[i] += inc[i];
                }
            }
            let n = S { t: v[0], x: v[1], y: v[2], z: v[3], ux: v[4], uy: v[5], uz: v[6], w: v[7] };
            let k5 = rhs::<f32, false>(&p, oy, oz, n.ux, n.uy, n.uz, n.w);
            let _ = old;
            if st.y > 0.0 && n.y <= 0.0 {
                let td = touchdown(&st, &k, &n, &k5, hv as f64);
                return td.x * un.l_m / YARD;
            }
            st = n;
            k = k5;
        }
    };
    for &h in &[0.05f64, 0.137] {
        for mode in 0..3u32 {
            let mut emax = 0.0f64;
            let mut erms = 0.0f64;
            for (i, s) in sh.iter().enumerate() {
                let e = (run(mode, h, s) - refc[i]).abs();
                emax = emax.max(e);
                erms += e * e;
            }
            println!("τ {h} 모드 {mode}: 캐리 최대 {emax:.2e} RMS {:.2e}", (erms / sh.len() as f64).sqrt());
        }
    }
}

// ------------------------------------------------------------------ 선택 사항: 보상 합 (f32 바닥)

/// f32 의 누적 반올림 바닥이 카한 보상 합으로 내려간다 (τ 0.05: 1.9e-4 → 3e-5 yd 수준)
#[test]
fn compensated_lowers_f32_floor() {
    let sh = shots(800);
    let env = Env::default();
    let on = Opts { min_steps: 0.0, compensated: true };
    let (mut e0, mut e1) = (0.0f64, 0.0f64);
    for s in &sh {
        let r = fly_with::<f64>(&Coef::SHARED_RE, &env, s, 0.02, None).carry_m / YARD;
        let a = fly_with::<f32>(&Coef::SHARED_RE, &env, s, 0.05, None).carry_m / YARD;
        let b = fly_with_opts::<f32>(&Coef::SHARED_RE, &env, &on, s, 0.05, None).carry_m / YARD;
        e0 = e0.max((a - r).abs());
        e1 = e1.max((b - r).abs());
    }
    println!("f32 τ 0.05 캐리 최대 오차: 보상 없음 {e0:.2e} → 보상 합 {e1:.2e} yd");
    assert!(e1 < 0.5 * e0 && e1 < 8e-5, "{e0:.2e} → {e1:.2e}");
}

/// 4레인 커널의 보상 합은 스칼라 f32 의 보상 합과 같다 (환경·최소 걸음과 함께)
#[test]
fn lanes_compensated_match_scalar() {
    let f = flat(&shots(90));
    for opts in [Opts { min_steps: 0.0, compensated: true }, Opts { min_steps: 6.0, compensated: true }] {
        for env in [Env::default(), Env::from_conditions(18.0, 400.0, 6.0, -3.0)] {
            let a = lanes::batch_lanes_opts::<P4>(&Coef::SHARED_RE, &env, &opts, &f, 0.1);
            let b = batch_scalar_opts::<f32>(&Coef::SHARED_RE, &env, &opts, &f, 0.1);
            let d = max_diff(&a, &b);
            assert!(d.0 < 1e-6 && d.1 < 1e-6 && d.2 < 1e-6 && d.3 < 1e-6, "차이 {d:?}");
        }
    }
}

/// 실험: 위기 속도를 Re 에 묶지 않고 무차원 속도에 고정했다면 고도 효과가 얼마나 달랐을까
#[test]
#[ignore]
fn exp_altitude_without_re_coupling() {
    let sea = Env::from_conditions(15.0, 0.0, 0.0, 0.0);
    let high = Env::from_conditions(20.0, 1600.0, 0.0, 0.0);
    for (label, shot) in [("드라이버 167 mph", DRIVER), ("7번 120 mph", shot(120.0, 16.3, 7097.0, 0.0)), ("웨지 100 mph", shot(100.0, 22.0, 8500.0, 0.0))] {
        let run = |env: &Env, fixed: bool| {
            let mut un = env.units();
            if fixed {
                un.re_scale = 1.0;
            }
            let p = P::<f64>::new(&Coef::SHARED_RE, &un);
            let init = Init::from_shot(&shot, &un);
            fly_init::<f64>(&p, &un, false, false, &init, 0.05, None).carry_m / YARD
        };
        let (a, b) = (run(&sea, false), run(&high, false));
        let (c, d) = (run(&sea, true), run(&high, true));
        println!("{label}: Re 연동 {a:.2} → {b:.2} yd ({:+.2} %), 위기 속도 고정 {c:.2} → {d:.2} yd ({:+.2} %)", (b / a - 1.0) * 100.0, (d / c - 1.0) * 100.0);
    }
}

// ------------------------------------------------------------------ 환경의 독립 검산: SI 단위 직접 적분

/// 무차원 환산(L, U, Re 비, 바람)이 맞는지 독립적으로 본다: m, s, kg 단위로 운동방정식을 직접 RK4 적분한 결과와 비교한다.
///   dv/dt = -(1/L) C_D |v_rel| v_rel + (1/L) C_L |v_rel| (ω̂ × v_rel) - g ŷ,  L = 2m/(ρA)
///   C_D = v + e/(1+(Re/Re0)^N) + d r  (N = CRISIS_N),  C_L = c r,  r = S/(1+l S),  S = Rω/|v_rel|,  Re = ρ|v_rel|D/μ,  dω/dt = -β ω |v_rel|/L
#[test]
fn si_reference_integrator_matches_env() {
    const G: f64 = 9.81;
    let (m, d_ball) = (0.04593f64, 0.04267f64);
    let area = core::f64::consts::PI * (d_ball / 2.0).powi(2);
    let c = Coef::SHARED_RE;
    let re0 = c.u0 * RE_PER_U;
    // 상수 L_M, U_MS 가 m, A, ρ0 와 g 에서 나온 값과 같다
    let l0 = 2.0 * m / (RHO0 * area);
    assert!((l0 / L_M - 1.0).abs() < 1e-9, "L_M 상수 {L_M} 대 2m/(ρA) {l0}");
    assert!(((G * l0).sqrt() / U_MS - 1.0).abs() < 1e-9, "U_MS 상수");
    assert!((RE_PER_U * 1.81e-5 / (RHO0 * d_ball) / U_MS - 1.0).abs() < 1e-12);

    let si = |env: &Env, s: &Shot| -> (f64, f64, f64) {
        let l = 2.0 * m / (env.rho * area);
        let v0 = s.mph * MPH;
        let th = s.vla_deg.to_radians();
        let a = s.axis_deg.to_radians();
        let (oy, oz) = (a.sin(), a.cos());
        let mut st = [0.0, 0.0, 0.0, v0 * th.cos(), v0 * th.sin(), 0.0, s.rpm * core::f64::consts::TAU / 60.0]; // x y z vx vy vz ω
        let f = |q: &[f64; 7]| -> [f64; 7] {
            let (rx, ry, rz) = (q[3] - env.wind[0], q[4] - env.wind[1], q[5] - env.wind[2]);
            let sp = (rx * rx + ry * ry + rz * rz).sqrt();
            let re = env.rho * sp * d_ball / env.mu;
            let spin = R_BALL * q[6] / sp;
            let r = spin / (1.0 + c.l * spin);
            let cd = c.v + c.e / (1.0 + (re / re0).powi(crate::CRISIS_N as i32)) + c.d * r;
            let cl = c.c * r;
            let (cx, cy, cz) = (oy * rz - oz * ry, oz * rx, -(oy * rx));
            [q[3], q[4], q[5], (-cd * sp * rx + cl * sp * cx) / l, (-cd * sp * ry + cl * sp * cy) / l - G, (-cd * sp * rz + cl * sp * cz) / l, -c.beta * q[6] * sp / l]
        };
        let dt = 2e-4;
        loop {
            let k1 = f(&st);
            let mut q = st;
            for i in 0..7 { q[i] = st[i] + 0.5 * dt * k1[i]; }
            let k2 = f(&q);
            for i in 0..7 { q[i] = st[i] + 0.5 * dt * k2[i]; }
            let k3 = f(&q);
            for i in 0..7 { q[i] = st[i] + dt * k3[i]; }
            let k4 = f(&q);
            let mut n = st;
            for i in 0..7 { n[i] = st[i] + dt / 6.0 * (k1[i] + 2.0 * (k2[i] + k3[i]) + k4[i]); }
            if st[1] > 0.0 && n[1] <= 0.0 {
                // 3차 에르미트 대신 선형 보간 (dt 가 작아 오차 ~1e-7 m)
                let t = st[1] / (st[1] - n[1]);
                return (st[0] + t * (n[0] - st[0]), st[2] + t * (n[2] - st[2]), 0.0);
            }
            st = n;
        }
    };
    let cases = [
        (DRIVER, Env::default()),
        (shot(120.0, 16.3, 7097.0, 12.0), Env::from_conditions(15.0, 0.0, 0.0, 0.0)),
        (DRIVER, Env::from_conditions(20.0, 1600.0, 0.0, 0.0)),
        (shot(100.0, 22.0, 8500.0, -8.0), Env::from_conditions(5.0, 600.0, -6.0, 4.0)),
        (shot(150.0, 12.0, 4000.0, 5.0), Env { wind: [3.0, 1.5, -5.0], rho: 1.05, mu: 1.9e-5 }),
    ];
    for (s, env) in cases {
        let (x, z, _) = si(&env, &s);
        let r = fly_with::<f64>(&Coef::SHARED_RE, &env, &s, 0.01, None);
        println!("SI 직접 적분 대조: 캐리 차 {:.2e} m, 편차 차 {:.2e} m (캐리 {x:.2} m)", (r.carry_m - x).abs(), (r.lateral_m - z).abs());
        assert!((r.carry_m - x).abs() < 1e-6, "{env:?}: 캐리 {} 대 SI {x}", r.carry_m);
        assert!((r.lateral_m - z).abs() < 1e-6, "{env:?}: 편차 {} 대 SI {z}", r.lateral_m);
    }
}

/// 거친 입력 (아주 느리거나 빠른 공, 스핀 0, 높은 로브, 큰 축 기울기): 4레인과 스칼라 f32 가 샷마다 같고 f64 와 크게 어긋나지 않는다
#[test]
fn lanes_wild_shots() {
    let mut x = 0xDEAD_BEEF_1234_5678u64;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut f = Vec::new();
    for i in 0..300 {
        let rpm = if i % 7 == 0 { 0.0 } else { 12000.0 * rnd() };
        f.extend_from_slice(&[20.0 + 180.0 * rnd(), 2.0 + 83.0 * rnd(), rpm, -60.0 + 120.0 * rnd()]);
    }
    for opts in [Opts::default(), Opts { min_steps: 6.0, compensated: true }] {
        let a = lanes::batch_lanes_opts::<P4>(&Coef::SHARED_RE, &Env::default(), &opts, &f, 0.137);
        let b = batch_scalar_opts::<f32>(&Coef::SHARED_RE, &Env::default(), &opts, &f, 0.137);
        let c = batch_scalar_opts::<f64>(&Coef::SHARED_RE, &Env::default(), &Opts { min_steps: opts.min_steps, compensated: false }, &f, 0.137);
        for i in 0..300 {
            for j in 0..4 {
                let (p, q) = (a[4 * i + j], b[4 * i + j]);
                assert!(p.is_finite() && (p - q).abs() < 1e-6 * (1.0 + q.abs()), "샷 {i} 성분 {j}: 4레인 {p} 스칼라 {q}");
            }
            // f64 대비 캐리: f32 + τ 0.137 이라 상대 1e-3 안쪽
            let (p, r) = (a[4 * i], c[4 * i]);
            assert!((p - r).abs() < 2e-3 * r.abs() + 0.01, "샷 {i}: f32 {p} f64 {r}");
        }
    }
}

/// τ 걸음보다 짧게 나는 공(첫 걸음 안에 착지)도 착지점을 찾는다. (옛 판본은 y > 0 에서 출발해야 한다는 조건 때문에 이 경우 2백만 걸음을 돌고 0 을 냈다)
#[test]
fn lands_within_first_step() {
    let s = shot(32.79, 6.21, 0.0, -37.7);
    let fine = fly::<f64>(&Coef::SHARED_RE, &s, 0.002, None);
    for &h in &[0.137, 0.3] {
        let r = fly::<f64>(&Coef::SHARED_RE, &s, h, None);
        assert!(r.carry_m.is_finite() && r.steps <= 3, "h {h}: 걸음 {} 캐리 {}", r.steps, r.carry_m);
        println!("h {h}: 캐리 {:.4} m (미세 걸음 {:.4} m), 걸음 {}", r.carry_m, fine.carry_m, r.steps);
        assert!((r.carry_m - fine.carry_m).abs() < 0.03 * fine.carry_m, "h {h}: {} 대 {}", r.carry_m, fine.carry_m);
    }
    // 4레인 커널도 같다
    let f = [32.79, 6.21, 0.0, -37.7, 120.0, 16.3, 7097.0, 0.0];
    let a = batch_lanes::<P4>(&Coef::SHARED_RE, &Env::default(), &f, 0.137);
    let b = batch_scalar::<f32>(&Coef::SHARED_RE, &Env::default(), &f, 0.137);
    assert!(a.iter().all(|x| x.is_finite()) && max_diff(&a, &b).0 < 1e-6);
    assert!((a[0] - fine.carry_m).abs() < 0.02 * fine.carry_m);
}

/// 표: 환경별 드라이버·7번 캐리 (RESULTS.md 용)
#[test]
#[ignore]
fn exp_env_table() {
    let h = 0.05;
    let c = |t: f64, alt: f64, wf: f64, wr: f64, s: &Shot| fly_with::<f64>(&Coef::SHARED_RE, &Env::from_conditions(t, alt, wf, wr), s, h, None);
    let iron = shot(120.0, 16.3, 7097.0, 0.0);
    let base_d = c(15.0, 0.0, 0.0, 0.0, &DRIVER).carry_m / YARD;
    let base_i = c(15.0, 0.0, 0.0, 0.0, &iron).carry_m / YARD;
    println!("해수면 15 °C 무풍: 드라이버 {base_d:.2} yd, 7번 {base_i:.2} yd");
    for alt in [0.0, 500.0, 1000.0, 1600.0, 2500.0] {
        let env = Env::from_conditions(15.0, alt, 0.0, 0.0);
        let (d, i) = (c(15.0, alt, 0.0, 0.0, &DRIVER).carry_m / YARD, c(15.0, alt, 0.0, 0.0, &iron).carry_m / YARD);
        println!("고도 {alt:>6} m (15 °C, ρ {:.4}): 드라이버 {d:.2} ({:+.2} %), 7번 {i:.2} ({:+.2} %)", env.rho, (d / base_d - 1.0) * 100.0, (i / base_i - 1.0) * 100.0);
    }
    for t in [-10.0, 0.0, 10.0, 20.0, 30.0, 40.0] {
        let env = Env::from_conditions(t, 0.0, 0.0, 0.0);
        let (d, i) = (c(t, 0.0, 0.0, 0.0, &DRIVER).carry_m / YARD, c(t, 0.0, 0.0, 0.0, &iron).carry_m / YARD);
        println!("기온 {t:>5} °C (해수면, ρ {:.4}, μ {:.4e}): 드라이버 {d:.2} ({:+.2} %), 7번 {i:.2} ({:+.2} %)", env.rho, env.mu, (d / base_d - 1.0) * 100.0, (i / base_i - 1.0) * 100.0);
    }
    for w in [-10.0, -5.0, 5.0, 10.0] {
        let (d, i) = (c(15.0, 0.0, w, 0.0, &DRIVER), c(15.0, 0.0, w, 0.0, &iron));
        println!("앞뒤 바람 {w:>5} m/s: 드라이버 {:.2} yd ({:+.2} %), 7번 {:.2} yd ({:+.2} %)", d.carry_m / YARD, (d.carry_m / YARD / base_d - 1.0) * 100.0, i.carry_m / YARD, (i.carry_m / YARD / base_i - 1.0) * 100.0);
    }
    for w in [5.0, 10.0] {
        let (d, i) = (c(15.0, 0.0, 0.0, w, &DRIVER), c(15.0, 0.0, 0.0, w, &iron));
        println!("횡풍(왼쪽에서) {w:>4} m/s: 드라이버 편차 {:+.2} m 캐리 {:.2} yd, 7번 편차 {:+.2} m 캐리 {:.2} yd", d.lateral_m, d.carry_m / YARD, i.lateral_m, i.carry_m / YARD);
    }
}

/// 양력 위기 장(공유위기12L): 새 엔진과 얼린 기준 구현이 같은 답을 내고, flightbench 의 같은 장(식 해석기판)과 PGA 드라이버 행이 맞는다
#[test]
fn lift_crisis_field_matches_oracle_and_flightbench() {
    for s in [DRIVER, shot(120.0, 16.3, 7097.0, 0.0), shot(100.0, 22.0, 8500.0, 8.0), shot(80.0, 30.0, 6000.0, -5.0)] {
        let a = fly::<f64>(&Coef::SHARED_RE_L, &s, 0.137, None);
        let o = oracle::fly::<f64>(&Coef::SHARED_RE_L, &s, 0.137, None);
        assert!((a.carry_m - o.carry_m).abs() < 1e-9 * o.carry_m.abs().max(1.0), "캐리 {} 대 기준 {}", a.carry_m, o.carry_m);
        assert!((a.land_deg - o.land_deg).abs() < 1e-9, "착지각");
    }
    // flightbench `quick --rows` (models_real.txt [공유위기12L고정]) 의 PGA DR 행: 275 yd 대비 잔차 −2.5 (소수 한 자리 반올림), 착지각 잔차 +1.7
    let dr = fly::<f64>(&Coef::SHARED_RE_L, &shot(167.0, 10.9, 2686.0, 0.0), 0.05, None);
    let (carry, land) = (dr.carry_m / YARD, dr.land_deg);
    assert!((carry - 272.5).abs() < 0.1 && (land - (38.0 + 1.7)).abs() < 0.1, "DR 캐리 {carry:.2} 착지각 {land:.2}");
    // 양력 위기가 실제로 작용한다: 같은 발사에서 공유위기12 와 달라야 한다 (착지 속력이 더 빠르다)
    let base = fly::<f64>(&Coef::SHARED_RE, &shot(167.0, 10.9, 2686.0, 0.0), 0.05, None);
    assert!(ground::landing_mph(&dr) > ground::landing_mph(&base) + 2.0, "L 착지 속력 {:.1} 대 12 {:.1}", ground::landing_mph(&dr), ground::landing_mph(&base));
}

/// 4레인 SIMD 도 양력 위기 장에서 스칼라 f32 와 샷마다 같다
#[test]
fn lift_crisis_lanes_match_scalar() {
    let f: Vec<f64> = (0..40)
        .flat_map(|i| {
            let t = i as f64 / 40.0;
            [70.0 + 100.0 * t, 6.0 + 28.0 * (1.0 - t), 1500.0 + 8000.0 * t, -15.0 + 30.0 * t]
        })
        .collect();
    let a = lanes::batch_lanes::<P4>(&Coef::SHARED_RE_L, &Env::default(), &f, 0.137);
    let b = batch_scalar::<f32>(&Coef::SHARED_RE_L, &Env::default(), &f, 0.137);
    for k in 0..a.len() {
        assert!((a[k] - b[k]).abs() < 1e-6 * (1.0 + b[k].abs()), "성분 {k}: 4레인 {} 스칼라 {}", a[k], b[k]);
    }
}

// ------------------------------------------------------------------ 앱 걸음 (H_APP + MIN_STEPS_APP), 한 번의 호출 API, 웹 참값

/// web3d 의 비행 엔진 시험이 쓰는 참값 (같은 식을 dt = 2e-5 RK4 로 푼 값). 열: mph,deg,rpm,axis_deg,carry_yd,lateral_yd,apex_yd,land_deg,time_s.
/// 축은 엔진 부호 그대로다 (axis > 0 이면 훅 = 왼쪽 = 편차 −). golf-core 도 같은 부호다.
const WEB_REFS_CSV: &str = include_str!("../../web3d/test/web_refs.csv");

/// web3d/test/engine.test.ts 의 허용 오차와 같다: 캐리·좌우·최고 높이 0.05 yd, 착지각 0.05°, 체공 시간 0.01 s
const WEB_TOL: [f64; 5] = [0.05, 0.05, 0.05, 0.05, 0.01];

struct WebRef {
    shot: Shot,
    /// carry yd, lateral yd, apex yd, land °, time s
    v: [f64; 5],
}

fn web_refs() -> Vec<WebRef> {
    let rows: Vec<WebRef> = WEB_REFS_CSV
        .trim()
        .lines()
        .skip(1)
        .map(|l| {
            let c: Vec<f64> = l.split(',').map(|x| x.trim().parse::<f64>().expect("참값 숫자")).collect();
            assert_eq!(c.len(), 9, "web_refs.csv 열 수");
            WebRef { shot: shot(c[0], c[1], c[2], c[3]), v: [c[4], c[5], c[6], c[7], c[8]] }
        })
        .collect();
    assert_eq!(rows.len(), 52, "web3d 시험도 52샷을 읽는다 (engine.test.ts)");
    rows
}

/// web3d 의 착지각 정의: 내려가는 속도 대 **수평 속력** (앞 + 옆). `Landing::land_deg` 는 앞 성분만 쓴다 (`fill_landing`):
/// 옆 속도가 없는 샷(축 0)에서는 같고, 스핀축이 기울면 옆 속도만큼 달라진다 (축 10° 0.3°, 축 −25° 1° 남짓: `land_angle_definition_gap`).
/// 비교는 정의를 맞춰 `vel_ms` (착지 순간 지면 기준 속도)에서 web3d 식으로 다시 계산한다.
fn land_deg_web(l: &Landing) -> f64 {
    (-l.vel_ms[1]).atan2(l.vel_ms[0].hypot(l.vel_ms[2])).to_degrees()
}

/// 한 샷의 오차 [캐리 yd, 편차 yd, 높이 yd, 착지각 °(web3d 정의), 시간 s] (절댓값)
fn ref_err(l: &Landing, r: &WebRef) -> [f64; 5] {
    [(l.carry_m / YARD - r.v[0]).abs(), (l.lateral_m / YARD - r.v[1]).abs(), (l.apex_m / YARD - r.v[2]).abs(), (land_deg_web(l) - r.v[3]).abs(), (l.time_s - r.v[4]).abs()]
}

fn app_opts() -> Opts {
    Opts { min_steps: MIN_STEPS_APP, compensated: false }
}

/// T2/T3: 앱이 쓰는 그 걸음(H_APP, MIN_STEPS_APP)과 그 호출(`fly_flat_into`, f64)이 web3d 와 같은 참값 52샷에서 같은 허용 오차 안이다.
/// 이 시험이 고정하는 것: 앱 걸음을 거칠게 바꾸면(예: τ 0.2) 여기서 먼저 걸린다.
#[test]
fn app_step_matches_web_refs() {
    let refs = web_refs();
    let mut max = [0.0f64; 5];
    let mut buf = Vec::new();
    let mut steps = 0.0;
    for r in &refs {
        let l = fly_flat_into(&mut buf, &Coef::SHARED_RE, &Env::default(), &app_opts(), &r.shot, H_APP);
        let e = ref_err(&l, r);
        steps += l.steps as f64;
        for k in 0..5 {
            max[k] = max[k].max(e[k]);
            assert!(e[k] <= WEB_TOL[k], "{:?}: 성분 {k} 오차 {:.3e} > {}", r.shot, e[k], WEB_TOL[k]);
        }
    }
    println!(
        "앱 걸음 τ {H_APP} + min{MIN_STEPS_APP} (f64, {}샷) 대 web_refs.csv: 최대 오차 캐리 {:.2e} yd, 편차 {:.2e} yd, 높이 {:.2e} yd, 착지각 {:.2e}°, 시간 {:.2e} s, 평균 걸음 {:.1}",
        refs.len(),
        max[0],
        max[1],
        max[2],
        max[3],
        max[4],
        steps / refs.len() as f64
    );
    // 허용(0.05 yd)의 2% 안쪽이어야 걸음을 조금 키우거나 장 값이 조금 바뀌어도 여유가 남는다 (실측 4.7e-4, 허용의 1%)
    assert!(max[0] < 1e-3, "캐리 최대 오차 {:.3e} yd", max[0]);
}

/// T3: 같은 걸음의 일괄 계산(클럽표): 4레인 f32 커널 `V` (wasm SIMD 빌드는 W4)가 참값 52샷에서 같은 허용 오차 안이다
#[test]
fn app_step_batch_matches_web_refs() {
    let refs = web_refs();
    let f: Vec<f64> = refs.iter().flat_map(|r| [r.shot.mph, r.shot.vla_deg, r.shot.rpm, r.shot.axis_deg]).collect();
    let out = lanes::batch_lanes_opts::<lanes::V>(&Coef::SHARED_RE, &Env::default(), &app_opts(), &f, H_APP);
    // 일괄 출력의 착지각은 앞 성분만 쓰는 정의라 옆 속도가 없는 샷(축 0)에서만 web3d 참값과 견준다
    let mut max = [0.0f64; 4];
    for (r, q) in refs.iter().zip(out.chunks_exact(4)) {
        let e = [(q[0] / YARD - r.v[0]).abs(), (q[1] / YARD - r.v[1]).abs(), (q[2] / YARD - r.v[2]).abs(), (q[3] - r.v[3]).abs()];
        let n = if r.shot.axis_deg == 0.0 { 4 } else { 3 };
        for k in 0..n {
            max[k] = max[k].max(e[k]);
            assert!(e[k] <= WEB_TOL[k], "{:?}: 성분 {k} 오차 {:.3e}", r.shot, e[k]);
        }
    }
    println!("앱 걸음 일괄(4레인 f32) 대 web_refs.csv: 최대 오차 캐리 {:.2e} yd, 편차 {:.2e} yd, 높이 {:.2e} yd, 착지각(축 0 샷만) {:.2e}°", max[0], max[1], max[2], max[3]);
}

/// 착지각 정의의 차이를 숫자로 고정한다 (고치지 않았다: `Landing::land_deg` 는 앞 성분만, web3d 는 수평 속력).
/// 축 0 이면 같고 축이 기울수록 벌어진다. `ground::roll_yd` 의 입력이므로 정의를 바꾸면 옆회전 샷의 롤이 달라진다 → 결정은 호출자 몫.
#[test]
fn land_angle_definition_gap() {
    let refs = web_refs();
    let mut buf = Vec::new();
    let (mut gap0, mut gap_side) = (0.0f64, 0.0f64);
    for r in &refs {
        let l = fly_flat_into(&mut buf, &Coef::SHARED_RE, &Env::default(), &app_opts(), &r.shot, H_APP);
        let gap = (l.land_deg - land_deg_web(&l)).abs();
        if r.shot.axis_deg == 0.0 {
            gap0 = gap0.max(gap);
        } else {
            gap_side = gap_side.max(gap);
        }
    }
    println!("착지각 정의 차이 (앞 성분 대 수평 속력): 축 0 최대 {gap0:.2e}°, 축 ±10°·−25° 최대 {gap_side:.2}°");
    assert!(gap0 < 1e-9);
    assert!(gap_side > 0.1 && gap_side < 3.0);
}

/// 한 번의 호출 API (`fly_flat_into`) 의 배열이 객체 API(`Flight`)·`Landing` 과 같은 값을 담고, 배치가 문서대로다
#[test]
fn flat_api_matches_flight_api() {
    let mut buf = Vec::new();
    for (s, h, ms) in [(DRIVER, 0.05, 0.0), (shot(120.0, 16.3, 7097.0, 8.0), H_APP, MIN_STEPS_APP), (shot(60.0, 5.0, 3000.0, -12.0), H_APP, MIN_STEPS_APP), (shot(40.0, 45.0, 3000.0, 0.0), 0.2, 0.0)] {
        let l = fly_flat_into(&mut buf, &Coef::SHARED_RE, &Env::default(), &Opts { min_steps: ms, compensated: false }, &s, h);
        let fl = Flight { r: l, path: buf[FLAT_HEAD..].to_vec(), total_ok: ground::launch_in_range(s.mph, s.vla_deg, s.rpm) };
        let n = buf[10] as usize;
        assert_eq!(buf.len(), FLAT_HEAD + n * PATH_STRIDE, "머리 + 경로 점 n 개");
        assert_eq!(buf[11] as usize, PATH_STRIDE);
        assert_eq!(buf[0].to_bits(), fl.carry_m().to_bits());
        assert_eq!(buf[1].to_bits(), fl.lateral_m().to_bits());
        assert_eq!(buf[2].to_bits(), fl.apex_m().to_bits());
        assert_eq!(buf[3].to_bits(), fl.land_deg().to_bits());
        assert_eq!(buf[4].to_bits(), fl.time_s().to_bits());
        assert_eq!(buf[5] as u32, fl.steps());
        assert_eq!(buf[6].to_bits(), fl.land_mph().to_bits());
        assert_eq!(buf[7].to_bits(), fl.roll_yd().to_bits());
        assert_eq!(buf[8].to_bits(), fl.total_yd().to_bits());
        assert_eq!(buf[9] != 0.0, fl.total_reliable());
        // 같은 입력의 객체 API (경로 포함) 와 비트까지 같은 값 (옵션이 없는 호출만)
        if ms == 0.0 {
            let g = flight_env::<f64>(s.mph, s.vla_deg, s.rpm, s.axis_deg, h, &Env::default());
            assert_eq!(g.r.carry_m.to_bits(), l.carry_m.to_bits());
            assert_eq!(g.path, buf[FLAT_HEAD..], "객체 API 의 경로 = 한 번의 호출 API 의 경로");
        }
        // 호환 path(): 앞의 4칸을 f32 로, 점 수 같음
        let legacy = fl.path();
        assert_eq!(legacy.len(), 4 * n);
        for i in 0..n {
            for j in 0..4 {
                assert_eq!(legacy[4 * i + j], buf[FLAT_HEAD + PATH_STRIDE * i + j] as f32);
            }
        }
        // 첫 점 = 출발 (t = 0, 위치 0, 속력 = 입력), 마지막 점 = 착지 (y = 0)
        let first = &buf[FLAT_HEAD..FLAT_HEAD + PATH_STRIDE];
        assert!(first[..4].iter().all(|&x| x == 0.0));
        let v0 = s.mph * MPH;
        let speed0 = (first[4] * first[4] + first[5] * first[5] + first[6] * first[6]).sqrt();
        assert!((speed0 - v0).abs() < 1e-9 * v0, "첫 점 속력 {speed0} 대 입력 {v0}");
        let last = &buf[buf.len() - PATH_STRIDE..];
        assert_eq!(last[2], 0.0);
        assert_eq!(last[1].to_bits(), l.carry_m.to_bits());
        assert_eq!(last[0].to_bits(), l.time_s.to_bits());
    }
    // 걸음이 잘못되면 머리가 NaN 이고 경로가 없다
    let l = fly_flat_into(&mut buf, &Coef::SHARED_RE, &Env::default(), &Opts::default(), &DRIVER, 0.0);
    assert!(l.carry_m.is_nan() && buf.len() == FLAT_HEAD && buf[10] == 0.0 && buf[0].is_nan());
    // 위로 나가지 않는 샷: 캐리 0, 경로 없음
    fly_flat_into(&mut buf, &Coef::SHARED_RE, &Env::default(), &Opts::default(), &shot(100.0, -5.0, 3000.0, 0.0), H_APP);
    assert!(buf[0] == 0.0 && buf[10] == 0.0 && buf.len() == FLAT_HEAD);
}

/// 경로의 3차 에르미트 보간 (위치 + 속도): 시간 t 의 위치 [x, y, z]. JS 소비자(ShotBall.tsx, HUD)가 같은 식을 쓴다.
fn hermite_pos(p: &[f64], t: f64) -> [f64; 3] {
    let n = p.len() / PATH_STRIDE;
    let mut i = 0;
    while i + 2 < n && p[PATH_STRIDE * (i + 1)] <= t {
        i += 1;
    }
    let (a, b) = (&p[PATH_STRIDE * i..PATH_STRIDE * (i + 1)], &p[PATH_STRIDE * (i + 1)..PATH_STRIDE * (i + 2)]);
    let dt = b[0] - a[0];
    let u = ((t - a[0]) / dt).clamp(0.0, 1.0);
    let (u2, u3) = (u * u, u * u * u);
    let (h00, h10, h01, h11) = (2.0 * u3 - 3.0 * u2 + 1.0, u3 - 2.0 * u2 + u, -2.0 * u3 + 3.0 * u2, u3 - u2);
    core::array::from_fn(|k| h00 * a[1 + k] + h10 * dt * a[4 + k] + h01 * b[1 + k] + h11 * dt * b[4 + k])
}

/// P3: 앱 걸음(성긴 경로, 약 21점)의 에르미트 곡선이 촘촘한 기준 경로(τ 0.005)에 얼마나 붙는가, 선형 이음과 비교한다.
/// 같은 시간에서 두 경로의 위치를 비교한다 (기준 경로의 점은 촘촘해서 점 위에서 바로 비교한다).
#[test]
fn hermite_path_tracks_fine_reference() {
    let sparse_lin = |p: &[f64], t: f64| {
        let n = p.len() / PATH_STRIDE;
        let mut i = 0;
        while i + 2 < n && p[PATH_STRIDE * (i + 1)] <= t {
            i += 1;
        }
        let (a, b) = (&p[PATH_STRIDE * i..], &p[PATH_STRIDE * (i + 1)..]);
        let u = ((t - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0);
        [a[1] + (b[1] - a[1]) * u, a[2] + (b[2] - a[2]) * u, a[3] + (b[3] - a[3]) * u]
    };
    let (mut worst_h, mut worst_l) = (0.0f64, 0.0f64);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for s in [DRIVER, shot(158.0, 9.2, 3655.0, 10.0), shot(120.0, 16.3, 7097.0, -8.0), shot(86.0, 25.7, 8403.0, 0.0), shot(60.0, 30.0, 6000.0, 5.0), shot(190.0, 9.0, 2000.0, 0.0)] {
        let l = fly_flat_into(&mut a, &Coef::SHARED_RE, &Env::default(), &app_opts(), &s, H_APP);
        fly_flat_into(&mut b, &Coef::SHARED_RE, &Env::default(), &Opts::default(), &s, 0.005);
        let (pa, pb) = (&a[FLAT_HEAD..], &b[FLAT_HEAD..]);
        let nb = pb.len() / PATH_STRIDE;
        for i in 0..nb {
            let t = pb[PATH_STRIDE * i];
            if t > l.time_s {
                break;
            }
            let rf = [pb[PATH_STRIDE * i + 1], pb[PATH_STRIDE * i + 2], pb[PATH_STRIDE * i + 3]];
            let (h, li) = (hermite_pos(pa, t), sparse_lin(pa, t));
            for k in 0..3 {
                worst_h = worst_h.max((h[k] - rf[k]).abs());
                worst_l = worst_l.max((li[k] - rf[k]).abs());
            }
        }
    }
    println!("성긴 경로(앱 걸음) 위치 오차 최대: 에르미트 {worst_h:.2e} m, 선형 {worst_l:.2e} m");
    assert!(worst_h < 0.02, "에르미트 {worst_h:.3e} m");
    assert!(worst_h < 0.05 * worst_l, "에르미트가 선형보다 훨씬 낫다: {worst_h:.3e} 대 {worst_l:.3e}");
}

/// 레인 커널 V4 (네이티브는 이식 가능한 `P4`, wasm simd128 빌드는 진짜 `W4`)가 스칼라 f32 와 허용 오차 안에서 같고, f64 와도 f32 바닥 안쪽이다.
/// 샷 수가 4의 배수가 아닌 경우, 앱 걸음(+최소 걸음 수), 환경(바람·밀도), 퇴화 샷이 섞인 경우를 본다. 최대 차이를 돌려준다.
fn check_lane_kernel<V4: lanes::F4>() -> f64 {
    let mut worst = 0.0f64;
    for &n in &[0usize, 1, 3, 4, 5, 9, 64, 203] {
        let f = flat(&shots(n));
        for (env, opts, h) in [
            (Env::default(), Opts::default(), 0.137),
            (Env::default(), Opts::default(), 0.05),
            (Env::default(), app_opts(), H_APP),
            (Env::from_conditions(10.0, 900.0, -5.0, 4.0), app_opts(), H_APP),
        ] {
            let a = lanes::batch_lanes_opts::<V4>(&Coef::SHARED_RE, &env, &opts, &f, h);
            let b = batch_scalar_opts::<f32>(&Coef::SHARED_RE, &env, &opts, &f, h);
            assert_eq!(a.len(), b.len());
            let d = max_diff(&a, &b);
            worst = worst.max(d.0).max(d.1).max(d.2).max(d.3);
            assert!(d.0 < 1e-6 && d.1 < 1e-6 && d.2 < 1e-6 && d.3 < 1e-6, "n {n} τ {h} {opts:?}: 스칼라 f32 와 차이 {d:?}");
            if n >= 64 {
                let c = batch_scalar_opts::<f64>(&Coef::SHARED_RE, &env, &opts, &f, h);
                let d = max_diff(&a, &c);
                assert!(d.0 < 1e-3 && d.1 < 1e-3, "n {n} τ {h}: f64 와 차이 {d:?}");
            }
        }
    }
    // 위로 나가지 않는 샷과 NaN 샷이 끼어도 나머지는 그대로
    let mut f = flat(&shots(10));
    f[4 * 3 + 1] = -5.0;
    f[4 * 6] = f64::NAN;
    let a = lanes::batch_lanes_opts::<V4>(&Coef::SHARED_RE, &Env::default(), &app_opts(), &f, H_APP);
    let b = batch_scalar_opts::<f32>(&Coef::SHARED_RE, &Env::default(), &app_opts(), &f, H_APP);
    for i in 0..10 {
        for j in 0..4 {
            let (x, y) = (a[4 * i + j], b[4 * i + j]);
            assert!((x.is_nan() && y.is_nan()) || (x - y).abs() < 1e-5, "샷 {i} 성분 {j}: {x} {y}");
        }
    }
    assert_eq!(a[12], 0.0);
    assert!(a[24].is_nan());
    worst
}

/// 이 빌드의 기본 레인 벡터 `lanes::V` (네이티브 P4). wasm simd128 빌드의 `W4` 는 아래 `w4_matches_scalar`
#[test]
fn default_lane_vector_matches_scalar() {
    let worst = check_lane_kernel::<lanes::V>();
    println!("V ({}) 대 스칼라 f32 최대 차이 {worst:.2e}", if lanes::SIMD { "W4" } else { "P4" });
}

/// T1: 진짜 wasm simd128 4레인 경로 (`W4`). `sh test-wasm.sh` (wasm-pack test --node, simd128 켬)로 돈다. 네이티브 `cargo test` 에는 나타나지 않는다.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[wasm_bindgen_test::wasm_bindgen_test]
fn w4_matches_scalar() {
    assert!(lanes::SIMD, "이 시험은 진짜 simd128 경로를 본다");
    let worst = check_lane_kernel::<lanes::W4>();
    wasm_bindgen_test::console_log!("W4 대 스칼라 f32 최대 차이 {worst:.2e}");
    // W4 도 P4 와 샷마다 같은 답이어야 한다 (둘 다 스칼라 f32 와 같은 연산 순서)
    let f = flat(&shots(203));
    let a = lanes::batch_lanes_opts::<lanes::W4>(&Coef::SHARED_RE, &Env::default(), &app_opts(), &f, H_APP);
    let b = lanes::batch_lanes_opts::<P4>(&Coef::SHARED_RE, &Env::default(), &app_opts(), &f, H_APP);
    assert_eq!(a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), "W4 와 P4 는 비트까지 같다");
}
