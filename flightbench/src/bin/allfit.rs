//! 모든 자료로 맞추고 자료원별로 판정한다 (꼴 고르기·제품용 매개변수).
//!
//! 자료원: PGA 투어 12행·LPGA 11행(캐리·최고 높이·착지각), USGA R22-01 20행(분포 평균 캐리), FlightScope 계산 175샷(캐리·높이, 스핀축),
//!   그리고 data/sources/ 의 CSV 가 있으면 함께 (averages.csv: 평균표, shots.csv: 개별 샷, aero.csv: 풍동 C_D·C_L).
//! 맞춤: 자료원마다 잔차 총 무게가 같도록 1/√(잔차 수) 를 곱해 한꺼번에 레벤버그-마쿼트.
//! 판정 (맞춤에 안 쓴 자료로만):
//!   5겹 CV — 자료원마다 i % 5 로 묶어 한 묶음씩 빼고 맞춘 뒤 그 묶음을 맞힌다.
//!   LOSO — 자료원 하나를 통째로 빼고 맞춘 뒤 그 자료원을 맞힌다 (처음 보는 자료원에 통하는가).
//! 비교: 기존 엔진 OpenFairway 원본(우리 자료로 안 맞춤)과 OF+2(같은 절차로 전역 항력·양력 배율 2개 재맞춤).
//!
//!   cargo run --profile quick --bin allfit [-- 모형이름,모형이름]   (MODELS=models_search.txt 등)
//!   --final: 모든 자료로 맞춘 매개변수만 (제품용)

use flightbench::engine::{USGA, YD};
use flightbench::harness::{gh_cached, lm, load_fs, load_models, Flight, Flight3, Model, Tunable, LPGA11, PGA12, RE_PER_U};
use flightbench::openfairway::OpenFairway;
use std::time::Instant;

#[derive(Clone, Copy)]
enum Item {
    /// 평균표 한 행: (mph, 발사각, rpm) → [캐리 yd, 최고 높이 yd, 착지각 deg] (NaN = 없음)
    Tour { mph: f64, deg: f64, rpm: f64, y: [f64; 3] },
    /// 개별 샷 (스핀축 포함): → [캐리 yd, 최고 높이 yd] (NaN = 없음)
    Shot { mph: f64, deg: f64, rpm: f64, axis: f64, y: [f64; 2] },
    /// USGA 행: 입력 분포 평균 캐리
    Usga { row: usize },
    /// 풍동: (Re, S) → [C_D, C_L]
    Aero { re: f64, s: f64, cd: f64, cl: f64 },
}

struct Source {
    name: String,
    items: Vec<Item>,
}

/// 잔차 단위: 캐리·높이 yd, 착지각 deg, C_D·C_L 은 0.01 을 1 로
const AERO_UNIT: f64 = 0.01;

trait Predict: Flight + Flight3 + Tunable + Clone + Send + Sync {
    fn aero(&self, _re: f64, _s: f64) -> Option<(f64, f64)> {
        None
    }
    fn label(&self) -> String;
}

impl Predict for Model {
    fn aero(&self, re: f64, s: f64) -> Option<(f64, f64)> {
        // kd = |u| C_D, kl = |u| C_L 를 (u, w = S u) 에서
        let u = re / RE_PER_U;
        let mut vars = vec![0.0; 4 + self.p.len()];
        vars[0] = u;
        vars[1] = s * u;
        vars[2] = s;
        vars[3] = re;
        vars[4..].copy_from_slice(&self.p);
        Some((self.kd.eval(&vars) / u, self.kl.eval(&vars) / u))
    }
    fn label(&self) -> String {
        self.name.clone()
    }
}

impl Predict for OpenFairway {
    fn label(&self) -> String {
        if self.mul == (1.0, 1.0) { "OF원본".into() } else { "OF+2".into() }
    }
}

/// USGA 한 행의 분포 평균 캐리 (가우스-에르미트 n0×n1×n2, 20 mph·0.5°·200 rpm 에서 자름)
fn usga_row<F: Flight + ?Sized>(m: &F, row: usize, n: [usize; 3]) -> f64 {
    let (g0, g1, g2) = (gh_cached(n[0]), gh_cached(n[1]), gh_cached(n[2]));
    let (_, mv, sv, md, sd, ms, ss, _) = USGA[row];
    let mut acc = 0.0;
    for i in 0..n[0] {
        for j in 0..n[1] {
            for k in 0..n[2] {
                let c = m.obs((mv + sv * g0.0[i]).max(20.0), (md + sd * g1.0[j]).max(0.5), (ms + ss * g2.0[k]).max(200.0))[0];
                acc += g0.1[i] * g1.1[j] * g2.1[k] * c;
            }
        }
    }
    acc
}

/// 항목 하나의 (예측 − 자료) 들. usga_n 은 USGA 구적 점 수
fn item_res<P: Predict>(m: &P, it: &Item, usga_n: [usize; 3], out: &mut Vec<f64>) {
    match *it {
        Item::Tour { mph, deg, rpm, y } => {
            let o = m.obs(mph, deg, rpm);
            for c in 0..3 {
                if y[c].is_finite() {
                    out.push(o[c] - y[c]);
                }
            }
        }
        Item::Shot { mph, deg, rpm, axis, y } => {
            let o = m.obs3(mph, deg, rpm, axis);
            for c in 0..2 {
                if y[c].is_finite() {
                    out.push(o[c] - y[c]);
                }
            }
        }
        Item::Usga { row } => out.push(usga_row(m, row, usga_n) - USGA[row].7),
        Item::Aero { re, s, cd, cl } => {
            if let Some((a, b)) = m.aero(re, s) {
                out.push((a - cd) / AERO_UNIT);
                out.push((b - cl) / AERO_UNIT);
            }
        }
    }
}

/// 자료원마다 총 무게가 같도록 맞춘다
fn fit_all<P: Predict>(m: &mut P, train: &[(usize, Vec<Item>)], usga_n: [usize; 3]) {
    let weights: Vec<f64> = train
        .iter()
        .map(|(_, items)| {
            let mut r = Vec::new();
            for it in items {
                item_res(m, it, usga_n, &mut r);
            }
            1.0 / (r.len().max(1) as f64).sqrt()
        })
        .collect();
    lm(m, &|m: &P| {
        let mut all = Vec::new();
        for ((_, items), w) in train.iter().zip(&weights) {
            let mut r = Vec::new();
            for it in items {
                item_res(m, it, usga_n, &mut r);
            }
            all.extend(r.into_iter().map(|x| x * w));
        }
        all
    });
}

/// 자료원별 [양별 오차 모음]: Tour → 캐리·높이·착지, Shot → 캐리·높이, Usga → 캐리, Aero → C_D·C_L
fn errors<P: Predict>(m: &P, items: &[Item]) -> Vec<Vec<f64>> {
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); 3];
    for it in items {
        match *it {
            Item::Tour { mph, deg, rpm, y } => {
                let o = m.obs(mph, deg, rpm);
                for c in 0..3 {
                    if y[c].is_finite() {
                        cols[c].push(o[c] - y[c]);
                    }
                }
            }
            Item::Shot { mph, deg, rpm, axis, y } => {
                let o = m.obs3(mph, deg, rpm, axis);
                for c in 0..2 {
                    if y[c].is_finite() {
                        cols[c].push(o[c] - y[c]);
                    }
                }
            }
            Item::Usga { row } => cols[0].push(usga_row(m, row, [7, 7, 7]) - USGA[row].7),
            Item::Aero { re, s, cd, cl } => {
                if let Some((a, b)) = m.aero(re, s) {
                    cols[0].push(a - cd);
                    cols[1].push(b - cl);
                }
            }
        }
    }
    cols
}

fn rmse(v: &[f64]) -> f64 {
    if v.is_empty() { f64::NAN } else { (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt() }
}

fn load_sources() -> Vec<Source> {
    let tour = |rows: &[[f64; 6]]| rows.iter().map(|r| Item::Tour { mph: r[0], deg: r[1], rpm: r[2], y: [r[3], r[4], r[5]] }).collect::<Vec<_>>();
    let mut out = vec![
        Source { name: "PGA 투어".into(), items: tour(&PGA12) },
        Source { name: "LPGA 투어".into(), items: tour(&LPGA11) },
        Source { name: "USGA 분포".into(), items: (0..USGA.len()).map(|row| Item::Usga { row }).collect() },
        Source {
            name: "FS 계산".into(),
            items: load_fs("fs_shots.csv").iter().map(|r| Item::Shot { mph: r[0], deg: r[1], rpm: r[2], axis: r[3], y: [r[4], r[5] / 3.0] }).collect(),
        },
    ];
    // data/sources/*.csv (자료 모으기 결과). 자료원 이름은 source_id 열
    let read = |path: &str| std::fs::read_to_string(path).ok();
    let num = |s: &str| s.trim().parse::<f64>().unwrap_or(f64::NAN);
    let mut extra: std::collections::BTreeMap<String, Vec<Item>> = Default::default();
    if let Some(t) = read("data/sources/averages.csv") {
        for l in t.lines().skip(1).filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let c: Vec<&str> = l.split(',').collect();
            if c.len() < 9 {
                continue;
            }
            let (mph, deg, rpm) = (num(c[3]), num(c[4]), num(c[5]));
            if !(mph.is_finite() && deg.is_finite() && rpm.is_finite()) {
                continue;
            }
            let y = [num(c[6]), num(c[7]), num(c[8])];
            if y.iter().all(|v| !v.is_finite()) {
                continue;
            }
            extra.entry(format!("평균 {}", c[0].trim())).or_default().push(Item::Tour { mph, deg, rpm, y });
        }
    }
    if let Some(t) = read("data/sources/shots.csv") {
        for l in t.lines().skip(1).filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let c: Vec<&str> = l.split(',').collect();
            if c.len() < 9 {
                continue;
            }
            let (mph, deg, rpm, axis) = (num(c[1]), num(c[2]), num(c[4]), num(c[5]));
            if !(mph.is_finite() && deg.is_finite() && rpm.is_finite()) {
                continue;
            }
            let y = [num(c[6]), num(c[7])];
            if y.iter().all(|v| !v.is_finite()) {
                continue;
            }
            extra.entry(format!("샷 {}", c[0].trim())).or_default().push(Item::Shot { mph, deg, rpm, axis: if axis.is_finite() { axis } else { 0.0 }, y });
        }
    }
    if let Some(t) = read("data/sources/aero.csv") {
        for l in t.lines().skip(1).filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let c: Vec<&str> = l.split(',').collect();
            if c.len() < 5 {
                continue;
            }
            let (re, s, cd, cl) = (num(c[1]), num(c[2]), num(c[3]), num(c[4]));
            if re.is_finite() && s.is_finite() && cd.is_finite() && cl.is_finite() {
                extra.entry(format!("풍동 {}", c[0].trim())).or_default().push(Item::Aero { re, s, cd, cl });
            }
        }
    }
    for (name, items) in extra {
        out.push(Source { name, items });
    }
    out
}

/// 한 모형의 판정: 모든 자료 맞춤 + 5겹 CV + LOSO. 반환 (맞춘 모형, CV 오차, LOSO 오차) — 자료원·양별 RMSE
fn evaluate<P: Predict>(base: &P, sources: &[Source], usga_n: [usize; 3], refit: bool) -> (P, Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let all: Vec<(usize, Vec<Item>)> = sources.iter().enumerate().map(|(i, s)| (i, s.items.clone())).collect();
    let mut full = base.clone();
    if refit {
        fit_all(&mut full, &all, usga_n);
    }
    let k = 5;
    // 5겹 CV: 묶음마다 자료원별·양별 오차를 모은다
    let cv_parts: Vec<Vec<Vec<Vec<f64>>>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..k)
            .map(|f| {
                let full = full.clone();
                let all = &all;
                sc.spawn(move || {
                    let train: Vec<(usize, Vec<Item>)> = all.iter().map(|(i, items)| (*i, items.iter().enumerate().filter(|(j, _)| j % k != f).map(|(_, it)| *it).collect())).collect();
                    let mut m = full;
                    if refit {
                        fit_all(&mut m, &train, usga_n);
                    }
                    all.iter()
                        .map(|(_, items)| {
                            let test: Vec<Item> = items.iter().enumerate().filter(|(j, _)| j % k == f).map(|(_, it)| *it).collect();
                            errors(&m, &test)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut cv: Vec<Vec<Vec<f64>>> = (0..sources.len()).map(|_| vec![Vec::new(); 3]).collect();
    for part in cv_parts {
        for (si, cols) in part.into_iter().enumerate() {
            for (c, col) in cols.into_iter().enumerate() {
                cv[si][c].extend(col);
            }
        }
    }
    // LOSO
    let loso: Vec<Vec<Vec<f64>>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..sources.len())
            .map(|leave| {
                let full = full.clone();
                let all = &all;
                sc.spawn(move || {
                    let train: Vec<(usize, Vec<Item>)> = all.iter().filter(|(i, _)| *i != leave).cloned().collect();
                    let mut m = full;
                    if refit {
                        fit_all(&mut m, &train, usga_n);
                    }
                    errors(&m, &all[leave].1)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let cv_rmse = cv.iter().map(|cols| cols.iter().map(|c| rmse(c)).collect()).collect();
    let loso_rmse = loso.iter().map(|cols| cols.iter().map(|c| rmse(c)).collect()).collect();
    (full, cv_rmse, loso_rmse)
}

fn qty_names(it: &Item) -> [&'static str; 3] {
    match it {
        Item::Tour { .. } => ["캐리", "높이", "착지°"],
        Item::Shot { .. } => ["캐리", "높이", ""],
        Item::Usga { .. } => ["캐리", "", ""],
        Item::Aero { .. } => ["C_D", "C_L", ""],
    }
}

fn main() {
    let t0 = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sources = load_sources();
    let path = std::env::var("MODELS").unwrap_or("models.txt".into());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut models = load_models(&text).unwrap();
    if let Some(sel) = args.iter().find(|a| !a.starts_with("--")) {
        let want: Vec<&str> = sel.split(',').collect();
        models.retain(|m| want.contains(&m.name.as_str()));
    }
    // 맞춤 때 USGA 구적은 3×3×5 (판정은 7×7×7)
    let usga_n = [3, 3, 5];
    println!("자료원:");
    for s in &sources {
        println!("  {:<14} {:>4}개", s.name, s.items.len());
    }
    if args.iter().any(|a| a == "--final") {
        let all: Vec<(usize, Vec<Item>)> = sources.iter().enumerate().map(|(i, s)| (i, s.items.clone())).collect();
        for m in models.iter_mut() {
            fit_all(m, &all, usga_n);
            let ps = m.pnames.iter().zip(&m.p).map(|(n, v)| format!("{n}={v:.6}")).collect::<Vec<_>>().join(" ");
            println!("{} (모든 자료): {ps}", m.name);
        }
        return;
    }
    // 표: 자료원·양별 RMSE (CV / LOSO)
    let mut rows: Vec<(String, Vec<Vec<f64>>, Vec<Vec<f64>>, String)> = Vec::new();
    for m in &models {
        let t = Instant::now();
        let (fm, cv, lo) = evaluate(m, &sources, usga_n, true);
        let ps = fm.pnames.iter().zip(&fm.p).map(|(n, v)| format!("{n}={v:.4}")).collect::<Vec<_>>().join(" ");
        eprintln!("  {} {:.1} s", m.name, t.elapsed().as_secs_f64());
        rows.push((m.label(), cv, lo, ps));
    }
    if !args.iter().any(|a| a == "--noof") {
        let t = Instant::now();
        let (_, cv, lo) = evaluate(&OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) }, &sources, usga_n, false);
        rows.push(("OF원본".into(), cv, lo, "FS 로 보정된 원본 (우리 자료로 안 맞춤)".into()));
        let (fm, cv, lo) = evaluate(&OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) }, &sources, usga_n, true);
        rows.push(("OF+2".into(), cv, lo, format!("항력×{:.3} 양력×{:.3}", fm.mul.0, fm.mul.1)));
        eprintln!("  OF {:.1} s", t.elapsed().as_secs_f64());
    }
    for (title, pick) in [("5겹 교차검증 (자료원마다 5묶음, 한 묶음씩 빼고 맞춤)", 0usize), ("자료원 하나 빼기 (LOSO: 그 자료원을 안 보고 맞춤)", 1)] {
        println!("\n{title}. RMSE: 캐리·높이 yd, 착지 deg, C_D·C_L 은 계수");
        let mut head = format!("{:<12} {:>6}", "모형", "종합");
        for s in &sources {
            let q = qty_names(&s.items[0]);
            for n in q.iter().filter(|n| !n.is_empty()) {
                head += &format!(" {:>11}", format!("{}{}", short(&s.name), n));
            }
        }
        println!("{head}");
        for (name, cv, lo, _) in &rows {
            let tab = if pick == 0 { cv } else { lo };
            // 종합: 자료원마다 (캐리·높이 yd, 착지 deg, C_D·C_L ×100) 평균을 낸 뒤 자료원 평균
            let comp = {
                let per: Vec<f64> = sources
                    .iter()
                    .enumerate()
                    .filter_map(|(si, s)| {
                        let q = qty_names(&s.items[0]);
                        let v: Vec<f64> = (0..3).filter(|&c| !q[c].is_empty() && tab[si][c].is_finite()).map(|c| tab[si][c] * if q[c].starts_with("C_") { 100.0 } else { 1.0 }).collect();
                        (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
                    })
                    .collect();
                per.iter().sum::<f64>() / per.len().max(1) as f64
            };
            let mut line = format!("{:<12} {:>6.3}", name, comp);
            for (si, s) in sources.iter().enumerate() {
                let q = qty_names(&s.items[0]);
                for (c, n) in q.iter().enumerate() {
                    if !n.is_empty() {
                        let v = tab[si][c];
                        line += &format!(" {:>11}", if v.is_nan() { "–".into() } else if n.starts_with("C_") { format!("{v:.3}") } else { format!("{v:.2}") });
                    }
                }
            }
            println!("{line}");
        }
    }
    println!("\n모든 자료로 맞춘 매개변수:");
    for (name, _, _, ps) in &rows {
        println!("  {name}: {ps}");
    }
    println!("총 {:.1} s", t0.elapsed().as_secs_f64());
    let _ = YD;
}

fn short(name: &str) -> String {
    let n: String = name.chars().take(6).collect();
    format!("{n}·")
}
