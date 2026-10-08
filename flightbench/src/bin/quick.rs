//! 빠른 시험대. 항은 models.txt 에서 바꾸고, 다시 컴파일 없이 돌린다.
//!
//!   cargo run --profile quick --bin quick                 # models.txt 의 모든 모형 + 기존 엔진(OpenFairway)
//!   cargo run --profile quick --bin quick -- H2,B4        # 고른 모형만
//!   cargo run --profile quick --bin quick -- --carry      # 캐리만으로 맞춤 (비교용)
//!   cargo run --profile quick --bin quick -- --rows       # 행별 잔차
//!   cargo run --profile quick --bin quick -- --check      # 구적 대 몬테카를로, τ 걸음 대 고운 걸음 검산
//!   cargo run --profile quick --bin quick -- --engines    # 기존 엔진 대비 속도·수치 정확도
//!   --noof: 기존 엔진 줄 빼기, --ofref: 기존 엔진의 수렴해 줄도 (약 9 s)
//!   MODELS=다른파일.txt 로 모형 파일을 바꿀 수 있다.
//!
//! 속도: 모형 하나 맞춤+판정 약 0.1 s, 기본 실행(모형 4개 + 기존 엔진 2줄) 약 1.5 s. 항을 바꿔도 다시 컴파일하지 않는다.
//!   (이전 calib: 약 10 분. 차이는 τ 걸음 RK4 56걸음, USGA 를 몬테카를로 20만 대신 가우스-에르미트 343점)
//! 검산: τ 걸음 0.05 대 0.002 최대 1.7e-6 yd. 구적 7^3 대 [15,15,31]: 행별 최대 0.25 yd, RMS 0.10 yd
//!   (입력 자르기의 꺾임 때문에 수렴이 느리다; RMSE 9 yd 판정에는 0.01 yd 아래 영향).
//!   calib(몬테카를로 20만)와 같은 매개변수에서 USGA RMSE 일치: H2 9.91 대 9.91, H2e 12.05 대 12.06.
//!
//! 결과 (2026-10-08). RMSE 캐리 yd / 높이 yd / 착지 deg
//!                 PGA 훈련            LPGA 보류            USGA 전체/프로/아마
//!   H2     4매  5.17 3.06 4.16   3.87 1.63 4.35   9.91 16.33 7.49
//!   H2rat  4매  4.98 2.69 4.36   3.83 1.45 4.40   9.45 14.89 7.50   (양력 포화를 유리식으로)
//!   B4     4매  4.48 2.68 4.73   3.70 1.49 4.58   9.00 13.22 7.58
//!   Z0     0매  7.76 9.25 4.52   3.59 5.08 3.71  11.82 13.58 11.34
//!   OF     원본  9.82 4.18 7.44   6.09 3.31 5.04   9.31 14.21 7.61   (OpenFairway, FlightScope 로 맞춘 값 그대로)
//!   OF+2   2매  5.77 3.60 4.91   3.42 2.98 5.30   8.06 14.29 5.49   (같은 PGA 로 전역 항력·양력 배율 재맞춤)
//!   기존 엔진에 같은 자료로 배율 두 개를 다시 맞추면 LPGA 캐리와 USGA(특히 아마추어)는 기존 엔진이 낫고,
//!   궤적 꼴(높이 1.5 대 3.0 yd, 착지각 4.4 대 5.3°)은 장 하나가 낫다. 기존 엔진의 레이놀즈 수 의존 항력이
//!   느린 공(아마추어)에서 효과가 있다는 뜻이지만, 우리 장에 같은 방향의 항(H2re)을 넣으면 LPGA 가 무너진다.
//!
//! --engines (무작위 2000샷, 1코어, 자기 수렴해 대비 캐리 오차)
//!   OpenFairway 이식 120 Hz (원본)   64,724 ns   최대 0.99 yd  RMS 0.49 yd
//!   OpenFairway 이식 1000 Hz        496,622 ns   최대 0.12 yd
//!   장 하나 H2 빠른 엔진 hτ=0.137        185 ns   최대 2.8e-4 yd RMS 8.0e-5 yd
//!   → 같은 샷에서 350배 빠르고 적분 오차는 3,500배 작다. 기존 엔진은 C# 원본이 아니라 러스트 이식이라
//!     실제 격차는 이보다 크거나 같다. 이식은 원본 회귀 시험 16개 중 14개 창 안 (벗어난 최대 1.08 yd).

use flightbench::engine::{dimless, run, stream_par, usga_samples, H2 as H2f, USGA, YD};
use flightbench::harness::*;
use flightbench::openfairway::{self as of, OpenFairway};
use std::time::Instant;

fn line<F: Flight + ?Sized>(name: &str, k: usize, params: &str, m: &F, t_fit: f64) -> Vec<f64> {
    let t0 = Instant::now();
    let tr = score3(m, &PGA12);
    let ho = score3(m, &LPGA11);
    let u = usga_gh(m);
    let obs: Vec<f64> = USGA.iter().map(|r| r.7).collect();
    let e: Vec<f64> = u.iter().zip(&obs).map(|(a, b)| a - b).collect();
    let pro: Vec<f64> = (0..20).filter(|&i| USGA[i].0.ends_with("Pro")).map(|i| e[i]).collect();
    let am: Vec<f64> = (0..20).filter(|&i| !USGA[i].0.ends_with("Pro")).map(|i| e[i]).collect();
    let t = t_fit + t0.elapsed().as_secs_f64();
    println!(
        "{:<8} {:>2} | {:>5.2} {:>5.2} {:>5.2} | {:>5.2} {:>5.2} {:>5.2} | {:>5.2} {:>5.2} {:>5.2} {:>+5.1} | {:>6.0} | {}",
        name, k, tr[0], tr[1], tr[2], ho[0], ho[1], ho[2], rmse(&e), rmse(&pro), rmse(&am), e.iter().sum::<f64>() / 20.0, t * 1e3, params
    );
    u
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    if flag("--engines") {
        engines();
        return;
    }
    let t_all = Instant::now();
    let path = std::env::var("MODELS").unwrap_or("models.txt".into());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut models = load_models(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    if let Some(sel) = args.iter().find(|a| !a.starts_with("--")) {
        let want: Vec<&str> = sel.split(',').collect();
        models.retain(|m| want.contains(&m.name.as_str()));
    }
    let all3 = !flag("--carry");
    println!("훈련: PGA 12행 {}.  판정: LPGA 11행, USGA 20행(가우스-에르미트 343점 분포 평균)", if all3 { "캐리·높이·착지각" } else { "캐리만" });
    println!("RMSE: 캐리 yd / 높이 yd / 착지 deg");
    println!("{:<8} {:>2} | {:^17} | {:^17} | {:^24} | {:>6} | 매개변수", "모형", "k", "PGA 훈련", "LPGA 보류", "USGA 전체 프로 아마 부호", "ms");
    for m in models.iter_mut() {
        let t0 = Instant::now();
        fit(m, &PGA12, all3);
        let tf = t0.elapsed().as_secs_f64();
        let ps = m.pnames.iter().zip(&m.p).map(|(n, v)| format!("{n}={v:.4}")).collect::<Vec<_>>().join(" ");
        line(&m.name, m.free().len(), &ps, m, tf);
    }
    let ofs = [("OF120Hz", OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) }), ("OF수렴", OpenFairway { hz: None, mul: (1.0, 1.0) })];
    for (n, m) in ofs.iter().filter(|(n, _)| !flag("--noof") && (flag("--ofref") || *n == "OF120Hz")) {
        line(n, 0, "기존 엔진 OpenFairway 이식 그대로 (FlightScope 로 맞춘 값, 우리 자료로 다시 맞추지 않음)", m, 0.0);
    }
    if !flag("--noof") {
        // 공정 비교: 같은 PGA 자료로 기존 엔진의 전역 항력·양력 배율 2개를 다시 맞춘다
        let mut m = OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) };
        let t0 = Instant::now();
        fit(&mut m, &PGA12, all3);
        let tf = t0.elapsed().as_secs_f64();
        line("OF+2", 2, &format!("기존 엔진 + 전역 배율 재맞춤: 항력×{:.3} 양력×{:.3}", m.mul.0, m.mul.1), &m, tf);
    }
    if flag("--rows") {
        for (title, rows, labs) in [("PGA", &PGA12[..], &PGA_LAB[..]), ("LPGA", &LPGA11[..], &LPGA_LAB[..])] {
            println!("\n{title} 행별 잔차 (캐리 / 높이 / 착지)");
            for (r, lab) in rows.iter().zip(labs) {
                let mut s = format!("  {lab:<3} {:>4.0} {:>3.0} {:>3.0} |", r[3], r[4], r[5]);
                for m in &models {
                    let o = m.obs(r[0], r[1], r[2]);
                    s += &format!(" {} {:+5.1} {:+5.1} {:+5.1} |", m.name, o[0] - r[3], o[1] - r[4], o[2] - r[5]);
                }
                let o = ofs[0].1.obs(r[0], r[1], r[2]);
                s += &format!(" OF {:+5.1} {:+5.1} {:+5.1}", o[0] - r[3], o[1] - r[4], o[2] - r[5]);
                println!("{s}");
            }
        }
    }
    if flag("--check") {
        check(&models);
    }
    println!("총 {:.2} s", t_all.elapsed().as_secs_f64());
}

/// 시험대 자체의 검산: τ 걸음 대 고운 걸음, 구적 대 몬테카를로
fn check(models: &[Model]) {
    let Some(m) = models.first() else { return };
    println!("\n검산 ({})", m.name);
    let mut fine = m.clone();
    fine.h = 0.002;
    let mut worst = [0.0f64; 3];
    for r in PGA12.iter().chain(&LPGA11) {
        let (a, b) = (m.obs(r[0], r[1], r[2]), fine.obs(r[0], r[1], r[2]));
        for c in 0..3 {
            worst[c] = worst[c].max((a[c] - b[c]).abs());
        }
    }
    println!("  τ 걸음 0.05 대 0.002: 최대 차 캐리 {:.1e} yd, 높이 {:.1e} yd, 착지 {:.1e}°", worst[0], worst[1], worst[2]);
    let t0 = Instant::now();
    let gh = usga_gh(m);
    let tg = t0.elapsed().as_secs_f64();
    let n = 20000;
    let t0 = Instant::now();
    let mut dmax = 0.0f64;
    let mut se_max = 0.0f64;
    for row in 0..20 {
        let s = usga_samples(row, n, 3);
        let c: Vec<f64> = s.iter().map(|p| {
            let mph = p[0] * flightbench::engine::U_MS / flightbench::engine::MPH;
            let rpm = p[2] * p[0] * flightbench::engine::U_MS / flightbench::engine::R_BALL * 60.0 / std::f64::consts::TAU;
            m.obs(mph, p[1].to_degrees(), rpm)[0]
        }).collect();
        let mean = c.iter().sum::<f64>() / n as f64;
        let se = (c.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64 / n as f64).sqrt();
        se_max = se_max.max(se);
        dmax = dmax.max((mean - gh[row]).abs() / se);
    }
    let rf = usga_gh3(m, [15, 15, 31]);
    for n in [[7, 7, 7], [11, 11, 11], [5, 5, 15], [7, 7, 15], [7, 7, 21], [5, 7, 21]] {
        let t0 = Instant::now();
        let g = usga_gh3(m, n);
        let dq = g.iter().zip(&rf).fold(0.0f64, |a, (x, y)| a.max((x - y).abs()));
        let rm = rmse(&g.iter().zip(&rf).map(|(x, y)| x - y).collect::<Vec<_>>());
        println!("  USGA 구적 {:?} ({}점) 대 [15,15,31]: 행별 차 최대 {dq:.3} yd, RMS {rm:.3} yd, {:.0} ms", n, n[0] * n[1] * n[2], t0.elapsed().as_secs_f64() * 1e3);
    }
    println!("  USGA 구적 343점 {:.0} ms 대 몬테카를로 행당 {n} 표본 {:.0} ms: 차 최대 {:.1}σ (σ ≤ {:.2} yd)", tg * 1e3, t0.elapsed().as_secs_f64() * 1e3, dmax, se_max);
}

/// 기존 엔진 대비 속도와 수치 정확도. 각 엔진을 자기 자신의 수렴해와 비교한다(같은 힘 법칙, 적분 오차만).
fn engines() {
    let n = 2000;
    let mut st = 0x2545F4914F6CDD1Du64;
    let mut rnd = |a: f64, b: f64| {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        a + (b - a) * ((st >> 11) as f64 / (1u64 << 53) as f64)
    };
    // (mph, 발사각, 총 스핀 rpm, 스핀축 기울기 deg)
    let shots: Vec<[f64; 4]> = (0..n).map(|_| [rnd(70.0, 170.0), rnd(8.0, 30.0), rnd(2000.0, 9000.0), rnd(-20.0, 20.0)]).collect();
    let of_shot = |s: &[f64; 4]| of::Shot { mph: s[0], vla: s[1], hla: 0.0, back: s[2] * s[3].to_radians().cos(), side: s[2] * s[3].to_radians().sin(), total: s[2] };
    // 이식 검증: 원본 저장소의 회귀 시험(LmCarryWindowRegressionTests) 창 안에 드는가
    let cases = of::regression_cases();
    let mut inside = 0;
    let mut worst = 0.0f64;
    for (name, s, a, b) in &cases {
        let c = of::fly_original(s, of::HZ)[0];
        let (lo, hi) = (a.min(*b) - 2.0, a.max(*b) + 2.0);
        let out = if c < lo { lo - c } else if c > hi { c - hi } else { 0.0 };
        if out == 0.0 { inside += 1 } else { println!("  창 밖: {name} {c:.1} yd (창 {lo:.1}–{hi:.1})") }
        worst = worst.max(out);
    }
    println!("OpenFairway 이식 검증: 원본 회귀 시험 {}개 중 {}개가 창(비교값 ±2 yd) 안, 벗어난 최대 {:.2} yd\n", cases.len(), inside, worst);
    println!("기존 엔진 대비: 무작위 {n}샷 (70–170 mph, 8–30°, 2000–9000 rpm, 스핀축 ±20°), 1코어");
    println!("각 엔진의 오차는 같은 힘 법칙을 아주 잘게 푼 자기 수렴해와의 캐리 차 (적분 방식이 만드는 오차)\n");
    println!("{:<44} {:>10} {:>12} {:>12}", "엔진", "ns/샷", "최대 오차 yd", "RMS 오차 yd");
    let of_ref: Vec<f64> = shots.iter().map(|s| of::fly_converged(&of_shot(s), 2e-5)[0]).collect();
    for hz in [60.0, 120.0, 240.0, 1000.0] {
        let t0 = Instant::now();
        let c: Vec<f64> = shots.iter().map(|s| of::fly_original(&of_shot(s), hz)[0]).collect();
        let ns = t0.elapsed().as_nanos() as f64 / n as f64;
        let e: Vec<f64> = c.iter().zip(&of_ref).map(|(a, b)| a - b).collect();
        let tag = if hz == 120.0 { " (원본 설정)" } else { "" };
        println!("{:<44} {:>10.0} {:>12.3} {:>12.3}", format!("OpenFairway 이식, 반암시 오일러 {hz} Hz{tag}"), ns, e.iter().fold(0.0f64, |m, x| m.max(x.abs())), rmse(&e));
    }
    // 우리 엔진: 같은 샷을 무차원으로
    let h2 = H2f { v: 0.2418, d: 0.2475, c: 2.401, lam: 7.08, e: 0.0 };
    let dl: Vec<[f64; 4]> = shots.iter().map(|s| { let d = dimless(s[0], s[1], s[2]); [d[0], d[1], d[2], s[3].to_radians()] }).collect();
    let truth = run(&h2, &dl, 0.0002);
    for dt in [0.128, 0.064] {
        let t0 = Instant::now();
        let r = run(&h2, &dl, dt);
        let ns = t0.elapsed().as_nanos() as f64 / n as f64;
        let e: Vec<f64> = r.iter().zip(&truth).map(|(a, b)| (a.0 - b.0) * YD).collect();
        println!("{:<44} {:>10.0} {:>12.1e} {:>12.1e}", format!("장 하나 H2, f64 RK4 {} 걸음 (8샷 묶음)", (2.6 / dt) as u32), ns, e.iter().fold(0.0f64, |m, x| m.max(x.abs())), rmse(&e));
    }
    let s32: Vec<[f32; 4]> = dl.iter().map(|d| [d[0] as f32, d[1] as f32, d[2] as f32, d[3].sin() as f32]).collect();
    let big: Vec<[f32; 4]> = (0..200_000).map(|i| s32[i % n]).collect();
    for h in [0.137f32, 0.0685] {
        let r = stream_par(&h2, &s32, h, 1);
        let t0 = Instant::now();
        let rb = stream_par(&h2, &big, h, 1);
        let ns = t0.elapsed().as_nanos() as f64 / big.len() as f64;
        std::hint::black_box(rb);
        let e: Vec<f64> = r.iter().zip(&truth).map(|(a, b)| (a[0] as f64 - b.0) * YD).collect();
        println!("{:<44} {:>10.0} {:>12.1e} {:>12.1e}", format!("장 하나 H2, 빠른 엔진 f32×16 hτ={h}"), ns, e.iter().fold(0.0f64, |m, x| m.max(x.abs())), rmse(&e));
    }
}
