//! 공개 TrackMan 드라이버 자료 읽기: 샷 하나의 입력(볼스피드·발사각·방향·스핀·축)과 실측 출력(캐리·옆·높이·착지각·착지 속력·비행 시간·최고점 거리).
//! `bin/real.rs`(채점·보정)와 `bin/aeroflex.rs`(자유 형 공기역학 곡면)가 같이 쓴다.

#[derive(Clone)]
pub struct Tm {
    pub mph: f64,
    pub vla: f64,
    pub ld: f64,
    pub rpm: f64,
    pub axis: f64,
    pub carry: f64,
    pub side: f64,
    pub apex_yd: f64,
    pub land: f64,
    pub land_mph: f64,
    pub time_s: f64,
    pub apex_x_yd: f64,
    pub last: f64,
    pub idx: usize,
    pub ball: String,
}

pub fn load(path: &str, all: bool) -> (Vec<Tm>, usize, usize) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let hdr: Vec<&str> = lines.next().expect("머리줄").split(',').collect();
    lines.next(); // 단위 줄
    let ix = |k: &str| hdr.iter().position(|h| *h == k).unwrap_or_else(|| panic!("열 없음: {k}"));
    let ball_ix = ix("Ball");
    let (a, b, c, d, e, f, g, h, i, j) = (
        ix("Ball Speed"), ix("Launch Angle"), ix("Launch Direction"), ix("Spin Rate"), ix("Spin Axis"),
        ix("Carry Flat - Length"), ix("Carry Flat - Side"), ix("Max Height - Height"), ix("Carry Flat - Land. Angle"), ix("Last data Point - Length"),
    );
    let spd_ix = ix("Carry Flat - Ball Speed");
    let (time_ix, apx_ix) = (ix("Carry Flat - Time"), ix("Max Height - Dist."));
    let (mut out, mut total, mut full) = (Vec::new(), 0usize, 0usize);
    for (idx, ln) in lines.enumerate() {
        total += 1;
        let v: Vec<&str> = ln.split(',').collect();
        let get = |k: usize| v.get(k).and_then(|x| x.trim().parse::<f64>().ok());
        let (Some(mph), Some(vla), Some(ld), Some(rpm), Some(axis), Some(carry), Some(side), Some(apex_ft), Some(land), Some(last), Some(land_mph), Some(time_s), Some(apx_ft)) =
            (get(a), get(b), get(c), get(d), get(e), get(f), get(g), get(h), get(i), get(j), get(spd_ix), get(time_ix), get(apx_ix))
        else {
            continue;
        };
        full += 1;
        let tracked = last >= 0.9 * carry && carry > 100.0;
        if all || tracked {
            out.push(Tm { mph, vla, ld, rpm, axis, carry, side, apex_yd: apex_ft / 3.0, land, land_mph, time_s, apex_x_yd: apx_ft / 3.0, last, idx, ball: v.get(ball_ix).unwrap_or(&"").trim().to_string() });
        }
    }
    (out, total, full)
}

/// 연속 구간(행 번호 간격 40 이하) 번호를 샷마다 붙인다: 같은 날 같은 자리의 세션
pub fn sessions(rows: &[Tm]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&i| rows[i].idx);
    let mut sess = vec![0usize; rows.len()];
    let (mut cur, mut last) = (0usize, rows[order[0]].idx);
    for &i in &order {
        if rows[i].idx - last > 40 {
            cur += 1;
        }
        sess[i] = cur;
        last = rows[i].idx;
    }
    sess
}

