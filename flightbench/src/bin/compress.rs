//! 계수 압축 진단 (CE 의 '맞춤 좌표 줄이기'를 장에 옮김): 맞춘 계수 중 자료가 실제로 정하는 방향이 몇 개인가.
//!
//!   cargo run --profile quick --bin compress [-- 모형이름,…] [--pga]          # 느슨한 방향 (기본 모형 파일 models.txt)
//!   TM_CSV=../data/real/external/trackman_public/data.csv 이면 공개 TrackMan 실측 767샷 블록도 같이 본다.
//!   MODELS=models_search.txt ... compress K2,공유위기12 --fewshot              # PGA k행만으로 맞춘 뒤 나머지 채점 (k = 1..4, 모든 조합)
//!   TM_CSV=... MODELS=... compress 모형 --ball=Medium --free=v[,r0]          # 공 종류 하나의 샷으로 고른 계수만 다시 맞춤
//!
//! 방법: 투어표로 맞춘 최적점에서 잔차의 야코비안 J = ∂r/∂ln p (중앙 차분)를 구하고 H = JᵀJ 를 고유분해한다.
//!   고유벡터 v_k 방향으로 ln p 를 δ 만큼 옮기면 잔차 제곱합이 λ_k δ² 늘어난다(선형 근사).
//!   σ_k = s / √λ_k (s = 잔차 RMS)는 그 조합의 1σ 불확실도다(ln 단위). σ_k 가 크면 그 조합은 자료가 못 정한다
//!   → 고정해도 손해가 없다(느슨한 방향, sloppy direction). 묶기 후보는 이 방향에서 고른다(훈련 자료만 봄).
//!   'δ=0.1 비용'은 그 방향으로 10 % 옮겼을 때 잔차 RMS 가 얼마나 오르는지(yd·deg 섞인 RMS).
//!
//! 결과 (2026-10-13, Ryzen 5 7500F. 상세와 판정은 README "힉스·클라루스 3차: 계수 압축 사다리")
//!   공유위기12 (투어 23행 맞춤, 잔차 RMS 2.219): λ 2.5e5 · 5.6e4 · 2.9e3 · 1.3e3 · 2.6e2 · 8.2e1 (3.5자리).
//!     σ_k = 0.4 · 0.9 · 4 · 6 · 14 · 25 %. 아래 두 방향은 10 % 옮겨도 RMS 가 0.009·0.003 오른다. 가장 느슨한 성분은 e (위기 세기).
//!     실측 767샷 블록만 봐도 같다 (가장 느슨한 방향 −0.86 ln e − 0.40 ln l).
//!   묶음 e = 1/4, d = c/2, l = 1/v, c = 20 v 를 넣은 K2 (맞춤 v, r0): λ 3.8e5 · 7.1e3, σ 0.4 % · 2.7 % → 느슨한 방향 없음.
//!   --fewshot (남은 PGA 캐리 중앙 / 최악, yd):  k=1  공유위기12 4.06 / 5.71   K2 3.53 / 5.72
//!                                              k=2             7.46 / 25.48       3.13 / 5.23
//!                                              k=4             5.01 / 12.87       2.90 / 4.72   (LPGA 최악 18.85 대 3.60)
//!   --ball=Medium: 항력 배율·v·(v, r0)·(항력, 양력 배율) 어느 하나로도 캐리 12 → 6–9 yd, 높이 편향 ±2–3 yd 가 남는다(세션 하나, 시간에 따른 표류).

use flightbench::harness::*;
use flightbench::realdata::{load, Tm};

/// 투어표 세 양 잔차 (캐리 yd, 높이 yd, 착지 deg)
fn tour_res(m: &Model, rows: &[[f64; 6]]) -> Vec<f64> {
    rows.iter().flat_map(|r| {
        let o = m.obs(r[0], r[1], r[2]);
        [o[0] - r[3], o[1] - r[4], o[2] - r[5]]
    }).collect()
}

/// TrackMan 샷 잔차 (캐리, 옆, 높이 yd, 착지각 deg, 착지 속력 mph). real.rs 의 Shooter 와 같은 식
fn tm_one(m: &Model, r: &Tm) -> Option<[f64; 5]> {
    let o = m.fly3(r.mph, r.vla, r.rpm, -r.axis);
    if o.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let (c, s) = (r.ld.to_radians().cos(), r.ld.to_radians().sin());
    Some([o[0] * c - o[1] * s - r.carry, o[0] * s + o[1] * c - r.side, o[2] - r.apex_yd, o[3] - r.land, o[4] - r.land_mph])
}

fn tm_res(m: &Model, rows: &[Tm]) -> Vec<f64> {
    rows.iter().flat_map(|r| tm_one(m, r).unwrap_or([1e3; 5])).collect()
}

/// J[i][k] = ∂r_i / ∂ln p_k (자유 매개변수만), 중앙 차분 h = 1e-4
fn jac(m: &Model, res: &(dyn Fn(&Model) -> Vec<f64> + Sync)) -> Vec<Vec<f64>> {
    let free = m.free();
    let cols: Vec<Vec<f64>> = std::thread::scope(|sc| {
        let hs: Vec<_> = free.iter().map(|&k| {
            let m = m.clone();
            sc.spawn(move || {
                let h = 1e-4f64;
                let (mut a, mut b) = (m.clone(), m.clone());
                a.p[k] = m.p[k] * h.exp();
                b.p[k] = m.p[k] * (-h).exp();
                let (ra, rb) = (res(&a), res(&b));
                ra.iter().zip(&rb).map(|(x, y)| (x - y) / (2.0 * h)).collect::<Vec<f64>>()
            })
        }).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = cols[0].len();
    (0..n).map(|i| cols.iter().map(|c| c[i]).collect()).collect()
}

/// 대칭 행렬 야코비 고유분해 → (고유값 내림차순, 고유벡터 열)
fn eig(mut a: Vec<Vec<f64>>) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    let mut v: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for _ in 0..100 {
        let off: f64 = (0..n).flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j))).map(|(i, j)| a[i][j] * a[i][j]).sum();
        if off < 1e-30 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let th = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
                let (c, s) = (th.cos(), th.sin());
                for k in 0..n {
                    let (akp, akq) = (a[k][p], a[k][q]);
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k][p], v[k][q]);
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| a[j][j].total_cmp(&a[i][i]));
    (idx.iter().map(|&i| a[i][i]).collect(), idx.iter().map(|&i| (0..n).map(|k| v[k][i]).collect()).collect())
}

fn report(title: &str, m: &Model, j: &[Vec<f64>], r: &[f64]) {
    let free = m.free();
    let np = free.len();
    let h: Vec<Vec<f64>> = (0..np).map(|a| (0..np).map(|b| j.iter().map(|row| row[a] * row[b]).sum()).collect()).collect();
    let (lam, vec) = eig(h);
    let n = r.len() as f64;
    let ss: f64 = r.iter().map(|x| x * x).sum();
    let s = (ss / n).sqrt();
    println!("\n{title}: 잔차 {} 개, RMS {s:.3}", r.len());
    let names: Vec<&str> = free.iter().map(|&k| m.pnames[k].as_str()).collect();
    println!("  {:>3} {:>11} {:>9} {:>11} | 고유벡터 (ln {})", "k", "λ", "σ_k(ln)", "δ=0.1 비용", names.join(", ln "));
    for k in 0..np {
        let sig = s / lam[k].max(1e-300).sqrt();
        let cost = ((ss + lam[k] * 0.01) / n).sqrt() - s;
        let comps = vec[k].iter().zip(&names).filter(|(c, _)| c.abs() > 0.15).map(|(c, nm)| format!("{c:+.2} {nm}")).collect::<Vec<_>>().join("  ");
        println!("  {:>3} {:>11.3e} {:>9.4} {:>11.4} | {}", k + 1, lam[k], sig, cost, comps);
    }
}

/// 적은 샷 보정: PGA 12행에서 k행 조합마다 시작값부터 맞추고, 보정에 안 쓴 PGA 행과 LPGA 11행의 캐리 RMSE 를 본다.
/// 출력: 조합들의 중앙값 / 90% / 최악 (yd). 계수가 적을수록 적은 샷으로 정해지는가.
fn fewshot(m0: &Model) {
    println!("\n=== {} 적은 샷 보정 (맞춤 {}개)", m0.name, m0.free().len());
    println!("  {:>2} {:>5} | {:^24} | {:^24} | {:^24}", "k", "조합", "남은 PGA 캐리 중앙/90%/최악", "LPGA 캐리 중앙/90%/최악", "LPGA 착지각 중앙/90%/최악");
    for k in 1..=4usize {
        let mut combos: Vec<Vec<usize>> = Vec::new();
        let mut c: Vec<usize> = (0..k).collect();
        loop {
            combos.push(c.clone());
            let mut i = k;
            while i > 0 && c[i - 1] == 12 - k + i - 1 {
                i -= 1;
            }
            if i == 0 {
                break;
            }
            c[i - 1] += 1;
            for j in i..k {
                c[j] = c[j - 1] + 1;
            }
        }
        let res: Vec<[f64; 3]> = std::thread::scope(|sc| {
            let chunks: Vec<Vec<Vec<usize>>> = combos.chunks(combos.len().div_ceil(12)).map(|x| x.to_vec()).collect();
            let hs: Vec<_> = chunks.into_iter().map(|part| {
                let m0 = m0.clone();
                sc.spawn(move || part.iter().map(|cb| {
                    let mut q = m0.clone();
                    q.reset();
                    let tr: Vec<[f64; 6]> = cb.iter().map(|&i| PGA12[i]).collect();
                    fit(&mut q, &tr, [1.0, 1.0, 1.0]);
                    let rest: Vec<[f64; 6]> = (0..12).filter(|i| !cb.contains(i)).map(|i| PGA12[i]).collect();
                    let a = score3(&q, &rest);
                    let b = score3(&q, &LPGA11);
                    [if a[0].is_finite() { a[0] } else { 1e3 }, if b[0].is_finite() { b[0] } else { 1e3 }, if b[2].is_finite() { b[2] } else { 1e3 }]
                }).collect::<Vec<_>>())
            }).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let q = |c: usize| {
            let mut v: Vec<f64> = res.iter().map(|r| r[c]).collect();
            v.sort_by(|a, b| a.total_cmp(b));
            (v[v.len() / 2], v[(v.len() * 9 / 10).min(v.len() - 1)], v[v.len() - 1])
        };
        let (a, b, d) = (q(0), q(1), q(2));
        println!("  {k:>2} {:>5} | {:>6.2} {:>6.2} {:>8.2} | {:>6.2} {:>6.2} {:>8.2} | {:>6.2} {:>6.2} {:>8.2}", res.len(), a.0, a.1, a.2, b.0, b.1, b.2, d.0, d.1, d.2);
    }
}

/// 공 종류 보정: 투어로 맞춘 장 m 에서 `free` 계수만 그 공의 샷(캐리·높이·착지각)으로 다시 맞춘다.
/// 세션 하나씩 빼고 맞춘 뒤 뺀 세션을 채점한다. 이상치는 |캐리 오차| > 25 yd (보정 뒤 모형 기준, 훈련 세션에서만 거름).
fn ball_calib(m0: &Model, rows: &[Tm], free: &[String]) {
    let sess = flightbench::realdata::sessions(rows);
    let ns = sess.iter().max().map(|x| x + 1).unwrap_or(0);
    let stat = |m: &Model, set: &[&Tm]| -> [f64; 7] {
        let e: Vec<[f64; 5]> = set.iter().filter_map(|r| tm_one(m, r)).filter(|e| e[0].abs() <= 25.0).collect();
        let n = e.len().max(1) as f64;
        let mean = |c: usize| e.iter().map(|x| x[c]).sum::<f64>() / n;
        let rms = |c: usize| (e.iter().map(|x| x[c] * x[c]).sum::<f64>() / n).sqrt();
        [e.len() as f64, rms(0), mean(0), mean(2), mean(3), mean(4), rms(2)]
    };
    let fixed_of = |m: &Model| -> Model {
        let mut q = m.clone();
        for i in 0..q.p.len() {
            if free.iter().any(|f| *f == q.pnames[i]) {
                q.lo[i] = q.p[i] * 0.3;
                q.hi[i] = q.p[i] * 3.0;
            } else {
                q.lo[i] = q.p[i];
                q.hi[i] = q.p[i];
            }
        }
        q
    };
    println!("\n=== {} 공 {}샷, 세션 {ns}개, 다시 맞추는 계수 {:?}", m0.name, rows.len(), free);
    let all: Vec<&Tm> = rows.iter().collect();
    let s0 = stat(m0, &all);
    println!("  보정 없음 (투어 그대로)   {:>4.0}샷 캐리 {:.2} / {:+.2}  높이 편향 {:+.2}  착지각 {:+.2}°  착지속력 {:+.2} mph  높이 RMSE {:.2}", s0[0], s0[1], s0[2], s0[3], s0[4], s0[5], s0[6]);
    // 나누기: 세션이 둘 이상이면 세션 하나씩 빼기. 하나뿐이면 시간 순서로 앞 N샷 보정 → 나머지 시험 (N = 5, 10, 20, 절반)
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&i| rows[i].idx);
    let splits: Vec<(String, Vec<&Tm>, Vec<&Tm>)> = if ns > 1 {
        (0..ns).map(|s| (format!("세션 {s} 뺌"),
            rows.iter().zip(&sess).filter(|(_, q)| **q != s).map(|(r, _)| r).collect(),
            rows.iter().zip(&sess).filter(|(_, q)| **q == s).map(|(r, _)| r).collect())).collect()
    } else {
        [5usize, 10, 20, rows.len() / 2].iter().map(|&n| (format!("앞 {n}샷 보정"),
            order[..n].iter().map(|&i| &rows[i]).collect(),
            order[n..].iter().map(|&i| &rows[i]).collect())).collect()
    };
    let single = ns <= 1;
    let mut acc: Vec<[f64; 5]> = Vec::new();
    for (label, tr, te) in splits {
        let mut q = fixed_of(m0);
        // 훈련 세션의 이상치는 투어 장 기준이 아니라 보정 뒤 기준으로: 두 번 맞춘다
        let mut keep: Vec<&Tm> = tr.clone();
        for _ in 0..2 {
            let kk = keep.clone();
            lm(&mut q, &|mm: &Model| kk.iter().flat_map(|r| tm_one(mm, r).map(|e| [e[0], e[2], e[3]]).unwrap_or([1e3; 3])).collect());
            keep = tr.iter().copied().filter(|r| tm_one(&q, r).map(|e| e[0].abs() <= 25.0).unwrap_or(false)).collect();
        }
        let ps = free.iter().map(|f| { let i = q.pnames.iter().position(|n| n == f).unwrap(); format!("{f}={:.4}", q.p[i]) }).collect::<Vec<_>>().join(" ");
        if single {
            let st = stat(&q, &te);
            println!("  {label:<22} {:>4.0}샷 캐리 {:.2} / {:+.2}  높이 편향 {:+.2}  착지각 {:+.2}°  착지속력 {:+.2} mph  높이 RMSE {:.2}  | {ps}", st[0], st[1], st[2], st[3], st[4], st[5], st[6]);
            continue;
        }
        for r in &te {
            if let Some(e) = tm_one(&q, r).filter(|e| e[0].abs() <= 25.0) {
                acc.push(e);
            }
        }
        println!("    {label} ({}샷 훈련, {}샷 시험): {ps}", keep.len(), te.len());
    }
    if single {
        return;
    }
    let n = acc.len().max(1) as f64;
    let mean = |c: usize| acc.iter().map(|x| x[c]).sum::<f64>() / n;
    let rms = |c: usize| (acc.iter().map(|x| x[c] * x[c]).sum::<f64>() / n).sqrt();
    println!("  세션 보류 보정            {:>4}샷 캐리 {:.2} / {:+.2}  높이 편향 {:+.2}  착지각 {:+.2}°  착지속력 {:+.2} mph  높이 RMSE {:.2}", acc.len(), rms(0), mean(0), mean(2), mean(3), mean(4), rms(2));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let path = std::env::var("MODELS").unwrap_or("models.txt".into());
    let mut models = load_models(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    // 기본 장(models.txt 의 공유위기12 등)은 다른 모형 파일을 쓸 때도 이름으로 고를 수 있게 뒤에 붙인다
    if path != "models.txt" {
        if let Ok(t) = std::fs::read_to_string("models.txt") {
            for m in load_models(&t).unwrap_or_default() {
                if !models.iter().any(|x| x.name == m.name) {
                    models.push(m);
                }
            }
        }
    }
    let sel = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or("공유위기12".into());
    let want: Vec<&str> = sel.split(',').collect();
    models.retain(|m| want.contains(&m.name.as_str()));
    let train: Vec<[f64; 6]> = if flag("--pga") { PGA12.to_vec() } else { PGA12.iter().chain(LPGA11.iter()).copied().collect() };
    let tm: Option<Vec<Tm>> = std::env::var("TM_CSV").ok().map(|p| {
        let (mut rows, _, _) = load(&p, false);
        rows.retain(|r| r.ball == "Premium" || r.ball == "Limited Distance Soft");
        rows
    });
    // --fewshot: PGA 12행 중 k행만으로 맞추고 나머지 PGA 행 + LPGA 11행을 채점 (k = 1..4, 모든 조합)
    if flag("--fewshot") {
        for m in &models {
            fewshot(m);
        }
        return;
    }
    // --ball=Medium --free=v[,r0]: 투어로 맞춘 장에서 그 공의 샷으로 고른 계수만 다시 맞춘다 (세션 하나씩 빼기)
    let opt = |k: &str| args.iter().find_map(|a| a.strip_prefix(k).map(|x| x.to_string()));
    if let (Some(ball), Ok(p)) = (opt("--ball="), std::env::var("TM_CSV")) {
        let (mut rows, _, _) = load(&p, false);
        rows.retain(|r| r.ball == ball);
        let free: Vec<String> = opt("--free=").unwrap_or("v".into()).split(',').map(|s| s.to_string()).collect();
        for mut m in models {
            fit(&mut m, &train, [1.0, 1.0, 1.0]);
            ball_calib(&m, &rows, &free);
        }
        return;
    }
    for mut m in models {
        fit(&mut m, &train, [1.0, 1.0, 1.0]);
        let ps = m.pnames.iter().zip(&m.p).map(|(n, v)| format!("{n}={v:.4}")).collect::<Vec<_>>().join(" ");
        println!("\n=== {} (훈련 {} 행) {ps}", m.name, train.len());
        let rt = tour_res(&m, &train);
        let jt = jac(&m, &|q: &Model| tour_res(q, &train));
        report("투어 세 양", &m, &jt, &rt);
        if let Some(rows) = &tm {
            // 이상치(이 모형 캐리 오차 25 yd 초과)는 맞춘 모형으로 한 번 정해 고정
            let kept: Vec<Tm> = rows.iter().filter(|r| tm_one(&m, r).map(|e| e[0].abs() <= 25.0).unwrap_or(false)).cloned().collect();
            let r = tm_res(&m, &kept);
            let j = jac(&m, &|q: &Model| tm_res(q, &kept));
            report(&format!("TrackMan 실측 {}샷 다섯 양 (캐리·옆·높이 yd, 착지각°, 착지 속력 mph)", kept.len()), &m, &j, &r);
            // 실측 블록을 투어 23행 무게로 줄여 합친 것 (real --calib 의 wtm = 1 과 같은 생각)
            let wt = (train.len() as f64 / kept.len() as f64).sqrt();
            let mut jc = jt.clone();
            jc.extend(j.iter().map(|row| row.iter().map(|x| x * wt).collect()));
            let mut rc = rt.clone();
            rc.extend(r.iter().map(|x| x * wt));
            report("투어 + 실측(투어 무게)", &m, &jc, &rc);
        }
    }
}
