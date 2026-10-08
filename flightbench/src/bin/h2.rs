//! 힉스 이중항 장 H2 의 러스트 엔진: 파이썬 일치, 정확도 곡선, 러스트 보정, USGA 분포 평균, 모드 장.
//!
//! 장을 항력(배경)과 양력(스핀 들뜸) 두 성분으로 둔다. 무차원 단위(higgs_engine.py 와 같음)에서
//!   |u| C_D = v |u| + d w,          |u| C_L = c w |u| / sqrt(|u|^2 + lambda^2 w^2)
//!   du/dt = -(|u| C_D) u + (|u| C_L) (omega_hat x u) - y_hat,   dw/dt = -0.06 w |u|
//! 비교용으로 같은 적분기에 분리 법칙 B4 와 각도 잠금 F3 도 넣는다.
//!
//! 결과 (2026-10-08, Xeon 2.8 GHz, AVX-512, 2코어)
//!   0. 파이썬 엔진과 33행 일치: 최대 7.4e-10 yd (H2), 8.1e-10 (B4), 6.8e-10 (F3)
//!   1. f64 8샷 묶음 시간 걸음: H2 804 ns @ 6.1e-3 yd, 1501 ns @ 3.5e-4 yd. B4 는 powf 때문에 약 4배 느리다.
//!   1b. 빠른 엔진 (f32×16, rsqrt 2개, τ 걸음, FSAL, 벡터 착지·expand-load 재충전), 사이드 스핀 ±40° 무작위 3000샷:
//!       hτ=0.137 (평균 20걸음) 176 ns/샷 @ 6.7e-4 yd, 2코어 1.12e7 샷/초
//!       hτ=0.0685 (40걸음) 288 ns/샷 @ 1.4e-4 yd (f32 반올림 바닥)
//!       같은 걸음 수의 f64 시간 걸음보다 4.6배 빠르고 9배 정확하다.
//!       병목 이력: 배열 클로저 자동 벡터화(ymm 반쪽·memcpy·호출, 14 ns/샷·걸음) → __m512 직접(7.2) →
//!       τ 걸음(같은 비용에 오차 1/10) → 착지 벡터화·묶음 처리 → 레지스터 압박 때문에 묶음 2개보다 1개가 빠름.
//!   2. 러스트 LM 보정 (PGA 7행): 파이썬 매개변수를 5자리까지 재현, 시작점 3개 11 ms.
//!   3. USGA 분포 평균: f64 행당 20만 (1200만 샷 14.7 s), 빠른 엔진 행당 100만 (2000만 샷 적분 2.8 s),
//!      표준오차 최대 0.045 yd. 두 결과의 차 최대 1.8σ. 파이썬 1500 표본의 표본 잡음(약 1.2 yd)이 사라져도
//!      순위는 같다: 전체 B4 8.85 < H2 9.75 < F3 11.97, 아마추어 H2 5.78 최소, 프로 H2 18.5 최대.
//!   4. 모드 장: 셋째 축을 sqrt(S) 로 펴니 같은 1.62 MB 에서 최대 오차 0.34 → 0.021 yd (16배), 읽기 82 ns,
//!      2코어 1.8e7 샷/초. 펼치기 0.1 s (빠른 엔진). 격자를 키워도 0.016 yd 에서 멈춘다(각도 모드 K=4 의 바닥).
//!
//! cargo run --release --bin h2        (--micro: RK4 한 걸음 측정)

use flightbench::engine::*;
use std::hint::black_box;
use std::time::Instant;

// ------------------------------------------------------------------ main

fn main() {
    if std::env::args().any(|a| a == "--micro") { micro(); return; }
    let h2 = H2 { v: 0.2705, d: 0.1536, c: 3.1361, lam: 10.167, e: 0.0 };
    let b4 = B4 { d0: 0.2627, d1: 0.1852, l0: 0.3612, p: 0.1765, e: 0.0 };
    let f3 = F3 { vev: 0.3881, h: 0.2714 };

    // ---- 0. 파이썬 일치
    let refs: Vec<Vec<f64>> = std::fs::read_to_string("../refs_models.csv")
        .expect("refs_models.csv")
        .lines()
        .map(|l| l.split(',').map(|x| x.parse().unwrap()).collect())
        .collect();
    let shots: Vec<[f64; 4]> = refs.iter().map(|r| [r[0], r[1], r[2], 0.0]).collect();
    let dev = |c: Vec<(f64, f64)>, col: usize| refs.iter().zip(c).fold(0.0f64, |m, (r, p)| m.max((p.0 - r[col]).abs() * YD));
    println!("== 0. 파이썬 엔진과 일치 (33행: PGA·LPGA·USGA 평균 입력, dt'=0.0005)");
    println!("  H2 {:.1e} yd / B4 {:.1e} yd / F3 {:.1e} yd",
        dev(run(&h2, &shots, 0.0005), 3), dev(run(&b4, &shots, 0.0005), 4), dev(run(&f3, &shots, 0.0005), 5));

    // ---- 1. 수치 정확도와 속도
    let n = 3000;
    let mut rng = Rng(0x5EED_1234);
    let rs: Vec<[f64; 4]> = (0..n)
        .map(|_| [rng.uni(1.0, 3.7), rng.uni(5f64.to_radians(), 40f64.to_radians()), rng.uni(0.03, 0.55), rng.uni(-40f64.to_radians(), 40f64.to_radians())])
        .collect();
    println!("\n== 1. 수치 정확도와 속도 (무작위 3000샷, 사이드 스핀 ±40°, 기준 dt'=0.0002)");
    println!("  장   dt'      스텝   ns/샷(1코어)  최대 오차 yd");
    macro_rules! acc { ($name:expr, $m:expr) => {{
        let truth = run($m, &rs, 0.0002);
        for dt in [0.128, 0.064, 0.032, 0.016] {
            let r = run($m, &rs, dt);
            let e = r.iter().zip(&truth).fold(0.0f64, |m, (a, b)| m.max(((a.0 - b.0).abs()).max((a.1 - b.1).abs()) * YD));
            let ns = bench(n, 5, || run($m, &rs, dt).iter().map(|p| p.0).sum());
            println!("  {}  {:<7}  {:>4}   {:>9.0}     {:.1e}", $name, dt, (2.6 / dt) as u32, ns, e);
        }
        truth
    }}; }
    let truth_h2 = acc!("H2", &h2);
    let _ = acc!("B4", &b4);

    println!("\n== 1b. 빠른 엔진 (f32×16, rsqrt, τ 걸음 dτ=|u|^½dt, FSAL, 레인 재충전) — 같은 3000샷, 기준은 위 f64 dt'=0.0002");
    println!("  장   hτ      평균 걸음  ns/샷 1코어   2코어 샷/초   최대 오차 yd   (x, z 따로)");
    let rs32: Vec<[f32; 4]> = rs.iter().map(|s| [s[0] as f32, s[1] as f32, s[2] as f32, s[3].sin() as f32]).collect();
    let big32: Vec<[f32; 4]> = (0..300_000).map(|i| rs32[i % n]).collect();
    for dt in [0.137f32, 0.1, 0.0685, 0.034] {
        let r = stream_par(&h2, &rs32, dt, 1);
        let (ex, ez) = r.iter().zip(&truth_h2).fold((0.0f64, 0.0f64), |m, (a, b)| (m.0.max((a[0] as f64 - b.0).abs() * YD), m.1.max((a[1] as f64 - b.1).abs() * YD)));
        let ns = bench(big32.len(), 5, || stream_par(&h2, &big32, dt, 1).iter().map(|p| p[0] as f64).sum());
        let ns2 = bench(big32.len(), 5, || stream_par(&h2, &big32, dt, 2).iter().map(|p| p[0] as f64).sum());
        println!("  H2  {:<6}  {:>6.1}    {:>8.1}     {:>9.2e}     {:.1e}  ({:.1e}, {:.1e})", dt, 2.78 / dt, ns, 1e9 / ns2, ex.max(ez), ex, ez);
    }

    // ---- 2. 러스트 보정
    println!("\n== 2. 러스트 보정 (H2, PGA 7행, Levenberg–Marquardt, dt'=0.008)");
    let pga: Vec<[f64; 4]> = PGA.to_vec();
    let t0 = Instant::now();
    let mut best = ([0.0; 4], f64::INFINITY, 0);
    for s in [[0.25, 0.2, 2.0, 5.0], [0.22, 0.3, 4.0, 12.0], [0.3, 0.1, 1.5, 2.0]] {
        let f = fit_h2(&pga, s, 0.008);
        if f.1 < best.1 {
            best = f;
        }
    }
    let t_fit = t0.elapsed().as_secs_f64();
    let pf = best.0;
    let hf = H2 { v: pf[0], d: pf[1], c: pf[2], lam: pf[3], e: 0.0 };
    let tr = rmse(&carry_rows(&hf, &pga, 0.008).iter().zip(&pga).map(|(c, r)| c - r[3]).collect::<Vec<_>>());
    let ho = rmse(&carry_rows(&hf, &LPGA, 0.008).iter().zip(&LPGA).map(|(c, r)| c - r[3]).collect::<Vec<_>>());
    println!("  v={:.5} d={:.5} c={:.5} lambda={:.4}  (파이썬 0.27049 0.15363 3.13610 10.1670)", pf[0], pf[1], pf[2], pf[3]);
    println!("  훈련 {:.3} / LPGA 보류 {:.3} yd,  시작점 3개 합계 {:.1} ms (파이썬 수 초)", tr, ho, t_fit * 1e3);

    // ---- 3. USGA 분포 평균: 표본 잡음 제거
    println!("\n== 3. USGA 분포 평균 (원리 <q[Psi]>), 행당 표본 20만, dt'=0.064");
    let nsamp = 200_000;
    let t0 = Instant::now();
    let dh = usga_dist(&h2, nsamp, 0.064, 1);
    let db = usga_dist(&b4, nsamp, 0.064, 1);
    let df = usga_dist(&f3, nsamp, 0.064, 1);
    let t_usga = t0.elapsed().as_secs_f64();
    let total_shots = 3 * USGA.len() * nsamp;
    println!("  {}샷 적분 {:.2} s ({:.0} ns/샷, 2코어)", total_shots, t_usga, t_usga * 1e9 / total_shots as f64);
    let se_max = dh.iter().chain(&db).chain(&df).fold(0.0f64, |m, p| m.max(p.1));
    println!("  행별 표본 표준오차 최대 {:.3} yd (파이썬 1500표본이면 약 {:.2} yd)", se_max, se_max * (nsamp as f64 / 1500.0).sqrt());
    let obs: Vec<f64> = USGA.iter().map(|r| r.7).collect();
    let groups: [(&str, Box<dyn Fn(usize) -> bool>); 5] = [
        ("전체 20행", Box::new(|_| true)),
        ("프로 4행", Box::new(|i| USGA[i].0.ends_with("Pro"))),
        ("아마추어 16행", Box::new(|i| !USGA[i].0.ends_with("Pro"))),
        ("6번 아이언 10행", Box::new(|i| USGA[i].0.contains("6i"))),
        ("드라이버 10행", Box::new(|i| USGA[i].0.contains("DR"))),
    ];
    println!("  캐리 RMSE yd           H2      B4      F3");
    for (g, f) in &groups {
        let e = |d: &Vec<(f64, f64)>| rmse(&(0..20).filter(|&i| f(i)).map(|i| d[i].0 - obs[i]).collect::<Vec<_>>());
        println!("  {:<18} {:>6.2}  {:>6.2}  {:>6.2}", g, e(&dh), e(&db), e(&df));
    }
    let bias = |d: &Vec<(f64, f64)>| (0..20).map(|i| d[i].0 - obs[i]).sum::<f64>() / 20.0;
    println!("  평균 부호 오차         {:>+6.2}  {:>+6.2}  {:>+6.2}", bias(&dh), bias(&db), bias(&df));

    // 빠른 엔진으로 행당 100만 표본: 표본 잡음을 0.05 yd 아래로
    let nbig = 1_000_000;
    let t0 = Instant::now();
    let mut fast = Vec::new();
    let mut t_int = 0.0;
    for row in 0..USGA.len() {
        let s: Vec<[f32; 4]> = usga_samples(row, nbig, 7).iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, 0.0]).collect();
        let t1 = Instant::now();
        let c = stream_par(&h2, &s, 0.0685, 2);
        t_int += t1.elapsed().as_secs_f64();
        let v: Vec<f64> = c.iter().map(|p| p[0] as f64 * YD).filter(|x| x.is_finite()).collect();
        let m = v.iter().sum::<f64>() / v.len() as f64;
        let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64;
        fast.push((m, (var / v.len() as f64).sqrt()));
    }
    let t_all = t0.elapsed().as_secs_f64();
    let n_all = USGA.len() * nbig;
    let dmax = (0..20).map(|i| (fast[i].0 - dh[i].0).abs() / (fast[i].1 * fast[i].1 + dh[i].1 * dh[i].1).sqrt()).fold(0.0f64, f64::max);
    let ef: Vec<f64> = (0..20).map(|i| fast[i].0 - obs[i]).collect();
    println!("  빠른 엔진 H2, 행당 100만: {}샷 적분 {:.2} s ({:.0} ns/샷, 2코어), 표본 생성 포함 {:.2} s", n_all, t_int, t_int * 1e9 / n_all as f64, t_all);
    println!("    표준오차 최대 {:.3} yd, f64 20만 결과와 차 최대 {:.1}σ, 전체 RMSE {:.2} yd", fast.iter().fold(0.0f64, |m, p| m.max(p.1)), dmax, rmse(&ef));

    // ---- 4. H2 모드 장
    println!("\n== 4. H2 모드 장 (3차원 장 하나에 계수 8개, 앞 3축 3차 읽기)");
    let q32: Vec<[f32; 4]> = rs.iter().map(|s| [s[0] as f32, s[1] as f32, s[2] as f32, (s[3] as f32).sin()]).collect();
    println!("  격자          a표본  크기      펼치기   x 최대  z 최대(yd)  ns/샷  1코어/초   2코어/초");
    for (g, na) in [([32usize, 51, 31], 12usize), ([40, 64, 46], 12), ([48, 76, 61], 12)] {
        let t0 = Instant::now();
        let m = Modes::unfold(&h2, g, na, 0.0685);
        let tb = t0.elapsed().as_secs_f64();
        let (mut ex, mut ez) = (0.0f64, 0.0f64);
        for (p, t) in q32.iter().zip(&truth_h2) {
            if p[0] < 1.56 || p[2] < 0.05 {
                continue; // 격자 3차 읽기 범위 밖 (v0' 1.56 이상, S0 0.05 이상에서 채점)
            }
            let (x, z) = m.read(p[0], p[1], p[2], p[3]);
            ex = ex.max((x as f64 - t.0).abs() * YD);
            ez = ez.max((z as f64 - t.1).abs() * YD);
        }
        let ns = bench(n, 30, || q32.iter().map(|p| { let (x, z) = m.read(p[0], p[1], p[2], p[3]); (x + z) as f64 }).sum());
        let big: Vec<[f32; 4]> = (0..1_000_000).map(|i| q32[i % n]).collect();
        let thr = |th: usize| {
            1e9 / bench(big.len(), 5, || {
                let per = big.len() / th;
                std::thread::scope(|sc| {
                    let hs: Vec<_> = big.chunks(per).map(|c| { let m = &m; sc.spawn(move || c.iter().map(|p| { let (x, z) = m.read(p[0], p[1], p[2], p[3]); x + z }).sum::<f32>() as f64) }).collect();
                    hs.into_iter().map(|h| h.join().unwrap()).sum()
                })
            })
        };
        println!("  {:>2}×{:>3}×{:>2}   {:>4}  {:>6.2} MB  {:>5.1} s  {:>7.4}  {:>7.4}   {:>6.1}  {:>9.2e}  {:>9.2e}",
            g[0], g[1], g[2], na, m.c.len() as f64 * 32.0 / 1e6, tb, ex, ez, ns, thr(1), thr(2));
    }
}

#[allow(dead_code)]
pub fn micro() {
    let p = H2 { v: 0.2705, d: 0.1536, c: 3.1361, lam: 10.167, e: 0.0 }.prep();
    let s0: [F; 7] = [0.0, 0.0, 0.0, 2.0, 0.5, 0.0, 0.3].map(F::splat);
    let (oy, oz) = (F::splat(0.1), F::splat(0.99));
    let h = 0.001f32;
    let (hh, hv, h6) = (F::splat(0.5 * h), F::splat(h), F::splat(h / 6.0));
    let k0 = deriv16::<H2>(&p, s0[3], s0[4], s0[5], s0[6], oy, oz);
    let steps = 4_000_000;
    macro_rules! multi { ($B:expr) => {{
        let t0 = Instant::now();
        let mut ss = [(s0, k0); $B];
        for it in 0..steps / $B {
            if it % 64 == 0 { ss = [(s0, k0); $B]; }
            for b in 0..$B { ss[b] = rk4_16::<H2>(&p, &ss[b].0, &ss[b].1, oy, oz, hh, hv, h6); }
            black_box(&ss);
        }
        let ns = t0.elapsed().as_nanos() as f64 / steps as f64;
        println!("micro x{}: {:.2} ns per shot-step ({:.0} cycles per 16-lane step)", $B, ns / 16.0, ns * 2.8);
    }}; }
    multi!(1); multi!(2); multi!(3);
}
