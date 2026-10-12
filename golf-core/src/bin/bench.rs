//! 네이티브 벤치: 정밀도(기준 f64 대비)와 속도.
//!
//!   cargo run --release --bin bench                    # 정밀도·속도 표 (스칼라 새/옛, f32, 4레인), 10만 샷 처리량
//!   cargo run --release --bin bench -- rows            # 투어 PGA 12행 잔차 (flightbench quick --rows 와 대조)
//!   cargo run --release --bin bench -- dump 파일.csv   # f64 τ 0.137 출력 (wasm 과 대조용)
//!   cargo run --release --bin bench -- dump 파일.csv oracle   # 같은 것을 얼린 옛 판본으로
//!   cargo run --release --bin bench -- cmp 옛_dump.csv  # 지금 엔진의 f64 τ 0.137 이 옛 dump 와 얼마나 다른가 (yd)
//!
//! 샷은 저불일치 수열(칸토로비치 꼴)로 만든다. 같은 식을 bench/bench.mjs 에도 써서 wasm·JS 와 같은 샷을 쓴다.
//! 기준 = 새 스칼라 f64 τ 0.002, 얼린 옛 판본의 같은 기준과도 대조해 둘의 차이를 함께 적는다.

use golf_core::lanes::{batch_lanes, P4};
use golf_core::{batch_scalar, fly, oracle, Coef, Env, Landing, Opts, Shot, YARD};
use std::hint::black_box;
use std::time::Instant;

/// 샷 n 개. 볼스피드 60–180 mph, 발사각 5–35°, 스핀 1500–10000 rpm, 축 ±30°
pub fn shots(n: usize) -> Vec<Shot> {
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

/// 엔진 하나: 샷 묶음 → 샷당 [캐리 m, 편차 m, 최고 m, 착지각 도]
type Engine = Box<dyn Fn(&[f64]) -> Vec<f64>>;

fn eng_new<T: golf_core::Real>(h: f64) -> Engine {
    Box::new(move |f| f.chunks_exact(4).flat_map(|q| rec(&fly::<T>(&Coef::SHARED_RE, &Shot { mph: q[0], vla_deg: q[1], rpm: q[2], axis_deg: q[3] }, h, None))).collect())
}
fn eng_old<T: golf_core::Real>(h: f64) -> Engine {
    Box::new(move |f| f.chunks_exact(4).flat_map(|q| rec(&oracle::fly::<T>(&Coef::SHARED_RE, &Shot { mph: q[0], vla_deg: q[1], rpm: q[2], axis_deg: q[3] }, h, None))).collect())
}
fn eng_batch<T: golf_core::Real>(h: f64) -> Engine {
    Box::new(move |f| batch_scalar::<T>(&Coef::SHARED_RE, &Env::default(), f, h))
}
fn eng_batch_opts<T: golf_core::Real>(h: f64, opts: Opts) -> Engine {
    Box::new(move |f| golf_core::batch_scalar_opts::<T>(&Coef::SHARED_RE, &Env::default(), &opts, f, h))
}
fn eng_lanes_opts(h: f64, opts: Opts) -> Engine {
    Box::new(move |f| golf_core::lanes::batch_lanes_opts::<P4>(&Coef::SHARED_RE, &Env::default(), &opts, f, h))
}
fn eng_lanes(h: f64) -> Engine {
    Box::new(move |f| batch_lanes::<P4>(&Coef::SHARED_RE, &Env::default(), f, h))
}

fn rec(l: &Landing) -> [f64; 4] {
    [l.carry_m, l.lateral_m, l.apex_m, l.land_deg]
}

/// 샷당 시간 (ns), reps 번 돌려 가장 짧은 값
fn time_ns(f: &[f64], e: &Engine, reps: usize) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t = Instant::now();
        black_box(e(black_box(f)));
        best = best.min(t.elapsed().as_nanos() as f64 / (f.len() / 4) as f64);
    }
    best
}

struct Err {
    max: f64,
    rms: f64,
}

/// 성분 j 의 오차 (scale 로 나눈다: m → yd)
fn err(a: &[f64], b: &[f64], j: usize, scale: f64) -> Err {
    let d: Vec<f64> = a.chunks(4).zip(b.chunks(4)).map(|(x, y)| (x[j] - y[j]).abs() / scale).collect();
    Err { max: d.iter().fold(0.0, |m, &x| m.max(x)), rms: (d.iter().map(|x| x * x).sum::<f64>() / d.len() as f64).sqrt() }
}

fn rows() {
    // PGA 12행: (볼스피드, 발사각, 스핀, 캐리 yd, 높이 yd, 착지각)
    let pga: [[f64; 6]; 12] = [
        [167.0, 10.9, 2686.0, 275.0, 32.0, 38.0], [158.0, 9.2, 3655.0, 243.0, 30.0, 43.0], [152.0, 9.4, 4350.0, 230.0, 31.0, 47.0],
        [146.0, 10.2, 4437.0, 225.0, 29.0, 47.0], [142.0, 10.4, 4630.0, 212.0, 27.0, 46.0], [137.0, 11.0, 4836.0, 203.0, 28.0, 48.0],
        [132.0, 12.1, 5361.0, 194.0, 31.0, 49.0], [127.0, 14.1, 6231.0, 183.0, 30.0, 50.0], [120.0, 16.3, 7097.0, 172.0, 32.0, 50.0],
        [115.0, 18.1, 7998.0, 160.0, 31.0, 50.0], [109.0, 20.4, 8647.0, 148.0, 30.0, 51.0], [102.0, 24.2, 9304.0, 136.0, 29.0, 52.0],
    ];
    let labs = ["DR", "3W", "5W", "HY", "3i", "4i", "5i", "6i", "7i", "8i", "9i", "PW"];
    println!("PGA 12행 잔차 (예측 − 측정), 기준 f64 τ 0.137. 같은 행을 flightbench quick --rows 와 대조");
    println!("  행    캐리   높이   착지");
    for (r, lab) in pga.iter().zip(labs) {
        let s = Shot { mph: r[0], vla_deg: r[1], rpm: r[2], axis_deg: 0.0 };
        let l = fly::<f64>(&Coef::SHARED_RE, &s, 0.137, None);
        let apex_yd = l.apex_m / YARD;
        println!("  {lab:<3} {:>+6.1} {:>+6.1} {:>+6.1}", l.carry_m / YARD - r[3], apex_yd - r[4], l.land_deg - r[5]);
    }
}

fn read_dump(path: &str) -> Vec<f64> {
    std::fs::read_to_string(path).expect("dump 읽기").lines().skip(1).flat_map(|l| l.split(',').map(|x| x.parse::<f64>().unwrap()).collect::<Vec<_>>()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "rows") {
        rows();
        return;
    }
    let n = 4000;
    let f = flat(&shots(n));
    if let Some(i) = args.iter().position(|a| a == "dump") {
        // wasm 과 같은 샷·같은 걸음에서 대조하는 용도 (τ 0.137)
        //   기본: 새 f64 스칼라 / oracle: 얼린 옛 판본 / env: 환경 (28 °C, 1500 m, 뒷바람 -4, 횡풍 +3) f64 / envlanes: 같은 환경의 4레인 f32
        let (name, e): (&str, Engine) = if args.iter().any(|a| a == "oracle") {
            ("얼린 옛 판본", eng_old::<f64>(0.137))
        } else if args.iter().any(|a| a == "env") {
            ("환경 f64", Box::new(|f| batch_scalar::<f64>(&Coef::SHARED_RE, &ENV_DUMP.env(), f, 0.137)))
        } else if args.iter().any(|a| a == "envlanes") {
            ("환경 4레인 f32", Box::new(|f| batch_lanes::<P4>(&Coef::SHARED_RE, &ENV_DUMP.env(), f, 0.137)))
        } else {
            ("새 엔진", eng_new::<f64>(0.137))
        };
        let mut out = String::from("carry_m,lateral_m,apex_m,land_deg\n");
        for q in e(&f).chunks(4) {
            out += &format!("{:.17e},{:.17e},{:.17e},{:.17e}\n", q[0], q[1], q[2], q[3]);
        }
        std::fs::write(&args[i + 1], out).expect("dump 파일 쓰기");
        println!("dump {} 줄 → {} ({})", n, args[i + 1], name);
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "cmp") {
        let old = read_dump(&args[i + 1]);
        let new = eng_new::<f64>(0.137)(&f);
        println!("옛 dump 대비 새 엔진 (f64 τ 0.137, {n}샷): 캐리 최대 {:.2e} yd, 편차 {:.2e} yd, 높이 {:.2e} yd, 착지각 {:.2e}°",
            err(&new, &old, 0, YARD).max, err(&new, &old, 1, YARD).max, err(&new, &old, 2, YARD).max, err(&new, &old, 3, 1.0).max);
        let rel = new.iter().zip(&old).map(|(a, b)| (a - b).abs() / (b.abs() + 1e-12)).fold(0.0f64, f64::max);
        println!("  최대 상대 차 {rel:.2e}");
        return;
    }

    let ref_new = eng_new::<f64>(0.002)(&f);
    // 얼린 옛 판본으로도 같은 기준을 만들어 둘이 같은지 본다
    let ref_old = eng_old::<f64>(0.002)(&f);
    println!(
        "기준 f64 τ 0.002: 새 엔진 대 얼린 옛 판본 차이 — 캐리 {:.2e} yd, 편차 {:.2e} yd, 높이 {:.2e} yd, 착지각 {:.2e}°",
        err(&ref_new, &ref_old, 0, YARD).max,
        err(&ref_new, &ref_old, 1, YARD).max,
        err(&ref_new, &ref_old, 2, YARD).max,
        err(&ref_new, &ref_old, 3, 1.0).max
    );

    println!("샷 {n}개 (저불일치 수열), 기준 = 새 스칼라 f64 τ 0.002. 시간은 한 코어, 샷당 최소 5회");
    println!("{:<20} {:>6} {:>5} {:>9} {:>10} {:>10} {:>10} {:>10} {:>10}", "엔진", "τ", "걸음", "샷당 ns", "캐리 최대", "캐리 RMS", "높이 최대", "편차 최대", "착지각 최대");
    let cfgs: Vec<(&str, f64, Engine)> = vec![
        ("f64 옛(oracle)", 0.05, eng_old::<f64>(0.05)),
        ("f64 옛(oracle)", 0.137, eng_old::<f64>(0.137)),
        ("f64 새 스칼라", 0.002, eng_new::<f64>(0.002)),
        ("f64 새 스칼라", 0.02, eng_new::<f64>(0.02)),
        ("f64 새 스칼라", 0.05, eng_new::<f64>(0.05)),
        ("f64 새 스칼라", 0.137, eng_new::<f64>(0.137)),
        ("f64 새 batch", 0.137, eng_batch::<f64>(0.137)),
        ("f32 옛(oracle)", 0.05, eng_old::<f32>(0.05)),
        ("f32 옛(oracle)", 0.137, eng_old::<f32>(0.137)),
        ("f32 새 스칼라", 0.02, eng_new::<f32>(0.02)),
        ("f32 새 스칼라", 0.05, eng_new::<f32>(0.05)),
        ("f32 새 스칼라", 0.137, eng_new::<f32>(0.137)),
        ("f32 4레인(P4)", 0.02, eng_lanes(0.02)),
        ("f32 4레인(P4)", 0.05, eng_lanes(0.05)),
        ("f32 4레인(P4)", 0.137, eng_lanes(0.137)),
        ("f32 P4 G=1", 0.137, Box::new(|f| golf_core::lanes::batch_lanes_g::<P4, 1>(&Coef::SHARED_RE, &Env::default(), &golf_core::Opts::default(), f, 0.137))),
        ("f32 P4 G=3", 0.137, Box::new(|f| golf_core::lanes::batch_lanes_g::<P4, 3>(&Coef::SHARED_RE, &Env::default(), &golf_core::Opts::default(), f, 0.137))),
        ("f32 P4 G=4", 0.137, Box::new(|f| golf_core::lanes::batch_lanes_g::<P4, 4>(&Coef::SHARED_RE, &Env::default(), &golf_core::Opts::default(), f, 0.137))),
        ("f64 batch min6", 0.137, eng_batch_opts::<f64>(0.137, Opts { min_steps: 6.0, compensated: false })),
        ("f32 스칼라 보상합", 0.05, eng_batch_opts::<f32>(0.05, Opts { min_steps: 0.0, compensated: true })),
        ("f32 P4 보상합", 0.05, eng_lanes_opts(0.05, Opts { min_steps: 0.0, compensated: true })),
        ("f32 P4 min6", 0.137, eng_lanes_opts(0.137, Opts { min_steps: 6.0, compensated: false })),
        ("f32 P4 min6+보상합", 0.1, eng_lanes_opts(0.1, Opts { min_steps: 6.0, compensated: true })),
        ("f32 P4 min6+보상합", 0.137, eng_lanes_opts(0.137, Opts { min_steps: 6.0, compensated: true })),
    ];
    for (name, h, e) in &cfgs {
        let out = e(&f);
        let ec = err(&out, &ref_new, 0, YARD);
        let eh = err(&out, &ref_new, 2, YARD);
        let es = err(&out, &ref_new, 1, YARD);
        let el = err(&out, &ref_new, 3, 1.0);
        let reps = if *h < 0.01 { 1 } else { 5 };
        let ns = time_ns(&f, e, reps);
        // 평균 걸음 수는 스칼라 f64 엔진으로 센다 (min6 행은 같은 규칙으로)
        let opts = Opts { min_steps: if name.contains("min6") { 6.0 } else { 0.0 }, compensated: false };
        let steps = shots(n).iter().map(|s| golf_core::fly_with_opts::<f64>(&Coef::SHARED_RE, &Env::default(), &opts, s, *h, None).steps as f64).sum::<f64>() / n as f64;
        println!(
            "{:<20} {:>6} {:>5.0} {:>9.0} {:>10.2e} {:>10.2e} {:>10.2e} {:>10.2e} {:>10.2e}",
            name, h, steps, ns, ec.max, ec.rms, eh.max, es.max, el.max
        );
    }

    // 처리량: 샷 10만 개 한 번에 (τ 0.137)
    let big = flat(&shots(100_000));
    println!("\n처리량: 샷 100,000개 한 번에 (τ 0.137), 샷당 ns / 초당 샷");
    for (name, e) in [
        ("f64 batch", eng_batch::<f64>(0.137)),
        ("f32 batch", eng_batch::<f32>(0.137)),
        ("f32 4레인(P4)", eng_lanes(0.137)),
        ("f64 옛(oracle)", eng_old::<f64>(0.137)),
    ] {
        let ns = time_ns(&big, &e, 3);
        println!("  {name:<16} {ns:>7.0} ns  {:>10.3e} 샷/초", 1e9 / ns);
    }
}

/// 환경 대조용 고정 조건 (bench.mjs 의 batch_env 호출과 같은 값)
struct EnvDump;
const ENV_DUMP: EnvDump = EnvDump;
impl EnvDump {
    fn env(&self) -> Env {
        Env::from_conditions(28.0, 1500.0, -4.0, 3.0)
    }
}

