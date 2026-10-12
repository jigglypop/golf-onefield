//! 실측 대조: 공개 TrackMan 드라이버 자료(약 1만 샷)에서 '끝까지 추적된' 샷으로 엔진을 채점하고, 실측으로 보정해 본다.
//!
//!   tools/fetch_trackman_public.sh                       # 자료 받기 (data/real/external/trackman_public/data.csv)
//!   TM_CSV=../data/real/external/trackman_public/data.csv MODELS=models_search.txt cargo run --profile quick --bin real
//!   ... --pga                       # 훈련을 PGA 12행만 (기본은 PGA+LPGA 23행)
//!   ... --all                       # 추적 여부와 관계없이 완비된 모든 행 (캐리는 TrackMan 의 외삽이 섞임)
//!   ... --balls="Premium,Limited Distance Soft"   # 공 종류 거르기
//!   ... --trim=25                   # |캐리 오차| 25 yd 초과 샷(추적 오류)을 채점에서 뺌
//!   ... --of                        # 기존 엔진 OpenFairway(원본 120 Hz, 수렴해, 투어로 배율 2개 재맞춤)도 같은 샷으로 채점
//!   ... --set=kdm=1.06              # 맞춘 뒤 매개변수를 덮어씀 (진단용; 모형 파일에 kdm 같은 배율을 선언해 둔다)
//!   ... --dump=잔차.csv             # 고른 모형(첫 번째)의 행별 잔차
//!   ... --axis-scale=1.15           # 기기 스핀축에 배율을 곱해 넣는다 (기기 보정 진단; 기본 1)
//!   ... --calib                     # 실측 세션 하나씩 빼고 보정 → 뺀 세션 채점 (세션 단위 보류), 끝에 전체 보정값과 회귀 점검
//!   ... --wtm=1                     # 보정에서 실측 묶음의 무게 (투어 23행 묶음 = 1)
//!   ... 모형이름,모형이름            # 고른 모형만
//!
//! 자료: https://github.com/tim-blackmore/launch-monitor-regression (data.csv, 라이선스 표기 없음 → 저장소에 넣지 않는다).
//! 입력은 TrackMan 이 잰 볼스피드·발사각·발사 방향·스핀·스핀축, 출력은 같은 샷의 Carry Flat 길이·옆 편차·최고 높이·착지각.
//! 추적 기준: 마지막 데이터 점이 캐리의 90% 이상이고 캐리 100 yd 초과 (레이더가 비행을 거의 끝까지 본 샷).
//! 부호: TrackMan 스핀축·발사 방향·옆 편차는 오른쪽이 +. 우리 장은 축 a > 0 이 왼쪽 → 축은 부호를 뒤집어 넣는다.
//! 대기(기온·고도·바람)는 자료에 없어 보정하지 않는다: 모형은 ρ = 1.2 kg/m³, 무풍.

use flightbench::engine::USGA;
use flightbench::harness::*;
use flightbench::openfairway::{fly_xz, OpenFairway, Shot as OfShot};
use flightbench::realdata::{load, sessions, Tm};

/// 보정에서 착지 속력(mph) 잔차의 무게 (--wspd=). 0 이면 쓰지 않는다
static WSPD: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
/// 스핀축 배율 (--axis-scale=): 기기가 낸 스핀축에 곱해 넣는다 (기본 1). 기기 보정 진단용
static AXIS: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
fn axis_of(r: &Tm) -> f64 {
    -r.axis * AXIS.get().copied().unwrap_or(1.0)
}


/// 샷 하나를 날려 실측과 비교: [캐리, 옆, 높이, 착지각] 오차 (예측 − 실측), 표적선 기준
trait Shooter {
    fn shoot(&self, r: &Tm) -> Option<[f64; 5]>;
}

impl Shooter for Model {
    fn shoot(&self, r: &Tm) -> Option<[f64; 5]> {
        let o = self.fly3(r.mph, r.vla, r.rpm, axis_of(r));
        if o.iter().any(|x| !x.is_finite()) {
            return None;
        }
        // 발사 방향(오른쪽 +)만큼 돌려 표적선 기준으로
        let (c, s) = (r.ld.to_radians().cos(), r.ld.to_radians().sin());
        Some([o[0] * c - o[1] * s - r.carry, o[0] * s + o[1] * c - r.side, o[2] - r.apex_yd, o[3] - r.land, o[4] - r.land_mph])
    }
}

impl Shooter for OpenFairway {
    fn shoot(&self, r: &Tm) -> Option<[f64; 5]> {
        // OpenFairway 는 스핀을 뒤·옆 성분으로 받는다. 옆 스핀 +는 오른쪽 휨(TrackMan 부호 그대로), hla + 는 오른쪽
        let a = r.axis.to_radians();
        let s = OfShot { mph: r.mph, vla: r.vla, hla: r.ld, back: r.rpm * a.cos(), side: r.rpm * a.sin(), total: r.rpm };
        let o = fly_xz(&s, self.hz, self.mul);
        if o.iter().any(|x| !x.is_finite()) {
            return None;
        }
        Some([o[0] - r.carry, o[1] - r.side, o[2] - r.apex_yd, o[3] - r.land, o[4] - r.land_mph])
    }
}

fn stat(e: &[f64]) -> (f64, f64) {
    (rmse(e), e.iter().sum::<f64>() / e.len().max(1) as f64)
}

/// 이름 붙은 엔진 한 줄 채점
fn score_row<S: Shooter>(name: &str, k: usize, m: &S, rows: &[Tm], trim: f64, ps: &str) -> (f64, String) {
    let res: Vec<(&Tm, [f64; 5])> = rows.iter().filter_map(|r| m.shoot(r).map(|e| (r, e))).filter(|(_, e)| e[0].abs() <= trim).collect();
    let col = |k: usize| res.iter().map(|x| x.1[k]).collect::<Vec<_>>();
    let bin = |lo: f64, hi: f64| rmse(&res.iter().filter(|(r, _)| r.mph >= lo && r.mph < hi).map(|x| x.1[0]).collect::<Vec<_>>());
    let (c, s, h, l, v) = (stat(&col(0)), stat(&col(1)), stat(&col(2)), stat(&col(3)), stat(&col(4)));
    (
        c.0,
        format!(
            "{:<12} {:>2} | {:>5.2} {:>+6.2} | {:>5.2} {:>+6.2} | {:>5.2} {:>+6.2} | {:>5.2} {:>+6.2} | {:>5.2} {:>+6.2} | {:>5.2} {:>5.2} {:>5.2} | {}",
            name, k, c.0, c.1, s.0, s.1, h.0, h.1, l.0, l.1, v.0, v.1, bin(0.0, 115.0), bin(115.0, 140.0), bin(140.0, 1e9), ps
        ),
    )
}

/// 투어 표 + 실측 샷(무게 wtm 로 묶음 전체가 투어 표 한 묶음과 같아지게)으로 매개변수를 다시 맞춘다
fn calibrate<T: Tunable + Shooter + Clone>(base: &T, tour: &[[f64; 6]], tm: &[&Tm], wtm: f64) -> T {
    let mut m = base.clone();
    let w = (wtm * tour.len() as f64 / tm.len().max(1) as f64).sqrt();
    lm(&mut m, &|m: &T| {
        let mut out = Vec::with_capacity(tour.len() * 3 + tm.len() * 4);
        for r in tour {
            let o = m.obs(r[0], r[1], r[2]);
            out.extend((0..3).map(|c| o[c] - r[3 + c]));
        }
        for r in tm {
            match m.shoot(r) {
                Some(e) => out.extend(e.iter().enumerate().map(|(k, x)| w * x * if k == 4 { *WSPD.get().unwrap_or(&0.0) } else { 1.0 })),
                None => out.extend([1e3; 5]),
            }
        }
        out
    });
    m
}

fn pooled(res: &[[f64; 5]]) -> [(f64, f64); 5] {
    std::array::from_fn(|k| stat(&res.iter().map(|e| e[k]).collect::<Vec<_>>()))
}

/// 세션 하나씩 빼고 보정 → 뺀 세션 채점. 반환: 기준과 보정 각각의 모은 잔차
fn session_holdout<T: Tunable + Shooter + Clone>(name: &str, base: &T, tour: &[[f64; 6]], rows: &[Tm], sess: &[usize], wtm: f64) -> Vec<(T, Vec<[f64; 5]>, Vec<[f64; 5]>)> {
    let n_sess = sess.iter().max().unwrap() + 1;
    let mut out = Vec::new();
    for s in 0..n_sess {
        let train: Vec<&Tm> = rows.iter().zip(sess).filter(|(_, q)| **q != s).map(|(r, _)| r).collect();
        let test: Vec<&Tm> = rows.iter().zip(sess).filter(|(_, q)| **q == s).map(|(r, _)| r).collect();
        let cal = calibrate(base, tour, &train, wtm);
        let eb: Vec<[f64; 5]> = test.iter().filter_map(|r| base.shoot(r)).collect();
        let ec: Vec<[f64; 5]> = test.iter().filter_map(|r| cal.shoot(r)).collect();
        println!("  [{name}] 세션 {s} 빼기 (훈련 {}샷, 채점 {}샷): 캐리 RMSE 기준 {:.2} → 보정 {:.2} yd", train.len(), test.len(), pooled(&eb)[0].0, pooled(&ec)[0].0);
        out.push((cal, eb, ec));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let opt = |k: &str| args.iter().find_map(|a| a.strip_prefix(k).map(|x| x.to_string()));
    let csv = std::env::var("TM_CSV").expect("TM_CSV=...data.csv 를 지정");
    let (mut rows, total, full) = load(&csv, flag("--all"));
    // --balls=Premium,Limited Distance Soft  공 종류 거르기 (쉼표로 나눔)
    if let Some(b) = opt("--balls=") {
        let want: Vec<&str> = b.split(',').collect();
        rows.retain(|r| want.contains(&r.ball.as_str()));
    }
    // --trim=25  |캐리 오차| 가 이 값(yd)을 넘는 샷은 이상치로 보고 채점에서 뺀다 (추적 오류 제거용; 기본은 안 뺌)
    let trim: f64 = opt("--trim=").and_then(|x| x.parse().ok()).unwrap_or(f64::INFINITY);
    // --set=이름=값,이름=값  맞춘 뒤 매개변수를 덮어쓴다 (진단용)
    let sets: Vec<(String, f64)> = opt("--set=")
        .map(|x| x.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.parse().unwrap())).collect())
        .unwrap_or_default();
    let wtm: f64 = opt("--wtm=").and_then(|x| x.parse().ok()).unwrap_or(1.0);
    WSPD.set(opt("--wspd=").and_then(|x| x.parse().ok()).unwrap_or(0.0)).unwrap();
    AXIS.set(opt("--axis-scale=").and_then(|x| x.parse().ok()).unwrap_or(1.0)).unwrap();
    let path = std::env::var("MODELS").unwrap_or("models_search.txt".into());
    let mut models = load_models(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    if let Some(sel) = args.iter().find(|a| !a.starts_with("--")) {
        let want: Vec<&str> = sel.split(',').collect();
        models.retain(|m| want.contains(&m.name.as_str()));
    }
    let train: Vec<[f64; 6]> = if flag("--pga") { PGA12.to_vec() } else { PGA12.iter().chain(LPGA11.iter()).copied().collect() };
    let (n_slow, n_mid, n_fast) = (
        rows.iter().filter(|r| r.mph < 115.0).count(),
        rows.iter().filter(|r| (115.0..140.0).contains(&r.mph)).count(),
        rows.iter().filter(|r| r.mph >= 140.0).count(),
    );
    println!("TrackMan 드라이버 {total}행 중 열 완비 {full}행, 채점 {}행 ({}). 볼스피드 <115: {n_slow}, 115–140: {n_mid}, ≥140 mph: {n_fast}", rows.len(), if flag("--all") { "모두" } else { "끝까지 추적된 샷" });
    let med = |f: fn(&Tm) -> f64| {
        let mut v: Vec<f64> = rows.iter().map(f).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    println!("실측 중앙값: 볼스피드 {:.0} mph, 발사각 {:.1}°, 스핀 {:.0} rpm, 캐리 {:.0} yd, 최고 높이 {:.0} yd, 착지각 {:.0}°",
        med(|r| r.mph), med(|r| r.vla), med(|r| r.rpm), med(|r| r.carry), med(|r| r.apex_yd), med(|r| r.land));
    println!("훈련: {} (캐리·높이·착지 1:1:1). 채점은 TrackMan 의 Carry Flat 길이·옆 편차·최고 높이·착지각", if flag("--pga") { "PGA 12행" } else { "PGA+LPGA 23행" });
    println!("{:<12} {:>2} | {:^13} | {:^13} | {:^13} | {:^13} | {:^13} | 캐리 RMSE 볼스피드별 <115 / 115–140 / ≥140 | 매개변수", "모형", "k", "캐리 RMSE/편향", "옆 RMSE/편향", "높이 RMSE/편향", "착지각 RMSE/편향", "착지속력 mph");

    // 투어로만 맞춘 모형들
    for m in models.iter_mut() {
        fit(m, &train, [1.0, 1.0, 1.0]);
        for (k, v) in &sets {
            if let Some(i) = m.pnames.iter().position(|n| n == k) {
                m.p[i] = *v;
            }
        }
    }
    let mut table: Vec<(f64, String)> = models
        .iter()
        .map(|m| {
            let ps = m.pnames.iter().zip(&m.p).map(|(n, v)| format!("{n}={v:.4}")).collect::<Vec<_>>().join(" ");
            score_row(&m.name, m.free().len(), m, &rows, trim, &ps)
        })
        .collect();
    if flag("--of") {
        let of120 = OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) };
        let ofc = OpenFairway { hz: None, mul: (1.0, 1.0) };
        let mut of2 = OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) };
        fit(&mut of2, &train, [1.0, 1.0, 1.0]);
        table.push(score_row("OF120Hz", 0, &of120, &rows, trim, "기존 엔진 원본 설정 (FlightScope 로 맞춘 값)"));
        table.push(score_row("OF수렴", 0, &ofc, &rows, trim, "같은 힘 법칙을 RK4 로 수렴해"));
        table.push(score_row("OF+2", 2, &of2, &rows, trim, &format!("투어로 배율 재맞춤: 항력×{:.3} 양력×{:.3}", of2.mul.0, of2.mul.1)));
    }
    table.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, t) in &table {
        println!("{t}");
    }
    // --dump=파일: 고른 모형(첫 번째)의 행별 잔차. 원래 파일의 행 번호를 같이 적는다 (세션 구조를 보려는 용도)
    if let Some(path) = opt("--dump=") {
        let m = &models[0];
        let mut out = String::from("idx,mph,vla,ld,rpm,axis,carry,side,apex_yd,land,last,e_carry,e_side,e_apex,e_land,pred_land,pred_land_mph,pred_time,pred_apex_x,time,apex_x
");
        for r in &rows {
            if let Some(e) = m.shoot(r).filter(|e| e[0].abs() <= trim) {
                let o = m.fly3(r.mph, r.vla, r.rpm, axis_of(r));
                out += &format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}
",
                    r.idx, r.mph, r.vla, r.ld, r.rpm, r.axis, r.carry, r.side, r.apex_yd, r.land, r.last, e[0], e[1], e[2], e[3], o[3], o[4], o[5], o[6], r.time_s, r.apex_x_yd
                );
            }
        }
        std::fs::write(&path, out).unwrap();
        println!("잔차 {} → {path}", m.name);
    }

    if flag("--calib") {
        calib(&args, &models, &rows, &train, trim, wtm);
    }
}

/// 세션 단위 보류 보정과 회귀 점검
fn calib(args: &[String], models: &[Model], rows: &[Tm], train: &[[f64; 6]], trim: f64, wtm: f64) {
    // 이상치는 투어로만 맞춘 첫 모형의 잔차로 한 번 정해 모든 엔진에 같이 적용한다 (보정 자료에서도, 채점에서도 뺀다)
    let mask_model = &models[0];
    let kept: Vec<Tm> = rows.iter().filter(|r| mask_model.shoot(r).map(|e| e[0].abs() <= trim).unwrap_or(false)).cloned().collect();
    let sess = sessions(&kept);
    let n_sess = sess.iter().max().unwrap() + 1;
    println!("\n[--calib] 이상치({trim} yd 초과, 기준 모형 {}) 제외 {}샷, 연속 세션 {n_sess}개 {:?}", mask_model.name, kept.len(), (0..n_sess).map(|s| sess.iter().filter(|&&q| q == s).count()).collect::<Vec<_>>());
    println!("보정 자료 = 투어 {}행 + 뺀 세션 밖의 실측 샷 (실측 묶음 무게 {wtm}). 채점 = 뺀 세션의 샷 (세션 단위 보류)", train.len());
    println!("{:<14} | {:^27} | {:^27} | {:^27} | {:^27} | {:^27}", "엔진", "캐리 RMSE/편향 기준 → 보정", "옆 RMSE/편향", "높이 RMSE/편향", "착지각 RMSE/편향", "착지속력(mph) RMSE/편향");
    let show = |name: &str, folds: &[(Model, Vec<[f64; 5]>, Vec<[f64; 5]>)]| {
        let eb: Vec<[f64; 5]> = folds.iter().flat_map(|f| f.1.clone()).collect();
        let ec: Vec<[f64; 5]> = folds.iter().flat_map(|f| f.2.clone()).collect();
        let (b, c) = (pooled(&eb), pooled(&ec));
        let cell = |k: usize| format!("{:>5.2}/{:>+5.2} → {:>5.2}/{:>+5.2}", b[k].0, b[k].1, c[k].0, c[k].1);
        println!("{:<14} | {} | {} | {} | {} | {}", name, cell(0), cell(1), cell(2), cell(3), cell(4));
    };
    let only = args.iter().find_map(|a| a.strip_prefix("--calib-models=")).map(|s| s.split(',').map(|x| x.to_string()).collect::<Vec<_>>());
    println!("
회귀 점검 열: 투어 PGA·LPGA (캐리/높이/착지 RMSE), USGA 전체·아마, FS 캐리·높이. 모형마다 보정 전 → 전체 실측 보정 후");
    let fs = load_fs("fs_shots.csv");
    let check = |tag: &str, m: &Model| {
        let tr = score3(m, &PGA12);
        let ho = score3(m, &LPGA11);
        let u = usga_gh(m);
        let e: Vec<f64> = u.iter().zip(USGA.iter()).map(|(a, r)| a - r.7).collect();
        let am: Vec<f64> = (0..20).filter(|&i| !USGA[i].0.ends_with("Pro")).map(|i| e[i]).collect();
        let f = score_fs(m, &fs);
        println!("  {tag:<16} PGA {:.2}/{:.2}/{:.2}  LPGA {:.2}/{:.2}/{:.2}  USGA {:.2} 아마 {:.2}  FS {:.2} 높이 {:.2}", tr[0], tr[1], tr[2], ho[0], ho[1], ho[2], rmse(&e), rmse(&am), f[0], f[4]);
    };
    for m in models.iter().filter(|m| only.as_ref().map(|o| o.contains(&m.name)).unwrap_or(true)) {
        let folds = session_holdout(&m.name, m, train, &kept, &sess, wtm);
        show(&m.name, &folds);
        // 모든 세션으로 다시 맞춘 값 (제안 매개변수) — 표본 안 성적이다
        let refs: Vec<&Tm> = kept.iter().collect();
        let all = calibrate(m, train, &refs, wtm);
        let ps = all.pnames.iter().zip(&all.p).map(|(n, v)| format!("{n}={v:.4}")).collect::<Vec<_>>().join(" ");
        println!("  [{}] 전체 보정값: {ps}", m.name);
        check(&format!("{} 전", m.name), m);
        check(&format!("{} 후", m.name), &all);
    }

    // OpenFairway 도 같은 절차로 (배율 2개)
    if args.iter().any(|a| a == "--of") {
        let mut of = OpenFairway { hz: Some(120.0), mul: (1.0, 1.0) };
        fit(&mut of, train, [1.0, 1.0, 1.0]);
        let n_s = n_sess_of(&sess);
        let (mut eb, mut ec) = (Vec::new(), Vec::new());
        for s in 0..n_s {
            let tr: Vec<&Tm> = kept.iter().zip(&sess).filter(|(_, q)| **q != s).map(|(r, _)| r).collect();
            let te: Vec<&Tm> = kept.iter().zip(&sess).filter(|(_, q)| **q == s).map(|(r, _)| r).collect();
            let cal = calibrate(&of, train, &tr, wtm);
            eb.extend(te.iter().filter_map(|r| of.shoot(r)));
            ec.extend(te.iter().filter_map(|r| cal.shoot(r)));
        }
        let (b, c) = (pooled(&eb), pooled(&ec));
        let cell = |k: usize| format!("{:>5.2}/{:>+5.2} → {:>5.2}/{:>+5.2}", b[k].0, b[k].1, c[k].0, c[k].1);
        println!("{:<14} | {} | {} | {} | {} | {}", "OF+2 (배율 2개)", cell(0), cell(1), cell(2), cell(3), cell(4));
    }
}

fn n_sess_of(sess: &[usize]) -> usize {
    sess.iter().max().unwrap() + 1
}
