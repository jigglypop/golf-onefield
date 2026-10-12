//! 영상 역보정 도구(`bin/screen.rs`, `bin/spininv.rs`)가 같이 쓰는 조각들.
//! 두 파일에 글자 그대로 복사돼 있던 것을 한 곳에 모았다 (동작은 같다).

use crate::linalg::solve;

/// 투어 23행 회귀 rpm ≈ 2233 + 303.7 발사각(deg) − 9.91 볼스피드(mph) (잔차 940 rpm), 사전분포 평균
pub fn prior_rpm(mph: f64, deg: f64) -> f64 {
    (2233.36 + 303.69 * deg - 9.909 * mph).clamp(1500.0, 11000.0)
}

/// 옆 카메라 투영 [xc, yc, Z0, sgn]: 평면 밖으로 z 만큼 나온 점이 화면 평면에 비치는 위치.
/// `read_csv` 가 CSV 의 `# cam xc yc Z0 sgn` 줄에서 한 번 채운다.
static CAM: std::sync::OnceLock<[f64; 4]> = std::sync::OnceLock::new();

#[inline]
pub fn project(p: [f64; 3]) -> [f64; 3] {
    match CAM.get() {
        Some(c) => {
            let s = c[2] / (c[2] + c[3] * p[2]);
            [c[0] + (p[0] - c[0]) * s, c[1] + (p[1] - c[1]) * s, p[2]]
        }
        None => p,
    }
}

/// CSV (t,x,y,z 미터·초; x 앞, y 위, z 오른쪽) → 프레임 배열 (빠진 프레임 NaN), 프레임 간격.
/// `# cam xc yc Z0 sgn` 줄이 있으면 카메라 투영을 켠다.
pub fn read_csv(path: &str) -> (Vec<[f64; 3]>, f64) {
    let rows: Vec<[f64; 4]> = std::fs::read_to_string(path).expect("CSV 읽기").lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| { let v: Vec<f64> = l.split(|c| c == ',' || c == ' ' || c == '\t').filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect(); (v.len() >= 4).then(|| [v[0], v[1], v[2], v[3]]) })
        .collect();
    assert!(rows.len() >= 5, "프레임이 5개 이상 필요");
    // "# cam xc yc Z0 sgn": 카메라 투영 (track_ball.py 가 씀)
    if let Some(l) = std::fs::read_to_string(path).unwrap().lines().find(|l| l.starts_with("# cam")) {
        let v: Vec<f64> = l[5..].split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() == 4 {
            let _ = CAM.set([v[0], v[1], v[2], v[3]]);
            eprintln!("카메라 투영 사용: 축 위치 ({:.3}, {:.3}) m, 거리 {:.3} m", v[0], v[1], v[2]);
        }
    }
    let mut dts: Vec<f64> = rows.windows(2).map(|w| w[1][0] - w[0][0]).filter(|d| *d > 0.0).collect();
    dts.sort_by(f64::total_cmp);
    let fdt = dts[dts.len() / 2];
    let n = ((rows.last().unwrap()[0] - rows[0][0]) / fdt).round() as usize + 1;
    let mut obs = vec![[f64::NAN; 3]; n];
    for r in &rows {
        let k = ((r[0] - rows[0][0]) / fdt).round() as usize;
        if k < n { obs[k] = [r[1], r[2], r[3]]; }
    }
    (obs, fdt)
}

/// `--이름 값` 꼴 명령줄 인자
pub fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

/// 프레임 번호 ks 의 한 축(c) 위치를 2차식 a + b t + c t² 로 최소제곱 맞춤 (t = k·fdt). 계수 [a, b, c]
pub fn quad_coeffs(obs: &[[f64; 3]], fdt: f64, c: usize, ks: &[usize]) -> [f64; 3] {
    let mut a = vec![vec![0.0; 4]; 3];
    for &k in ks {
        let t = k as f64 * fdt;
        let b = [1.0, t, t * t];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += b[i] * b[j];
            }
            a[i][3] += b[i] * obs[k][c];
        }
    }
    let s = solve(a);
    [s[0], s[1], s[2]]
}

/// 세 축 2차 맞춤 계수 → 발사 시작 [x0, y0, z0, 속력, 발사각 rad, 방위각 rad] (맞춤 시작점)
pub fn launch_from_quad(co: &[[f64; 3]; 3]) -> [f64; 6] {
    let (vx, vy, vz) = (co[0][1], co[1][1], co[2][1]);
    let v = (vx * vx + vy * vy + vz * vz).sqrt();
    [co[0][0], co[1][0], co[2][0], v, (vy / v).asin(), vz.atan2(vx)]
}
