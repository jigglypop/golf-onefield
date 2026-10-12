//! 총거리: 비행 모형이 예측한 착지 상태(착지각, 착지 속력) → 롤(굴러가는 거리).
//!
//! 이것은 지면 물리가 아니라 **TrackMan 기본 표면이 표시하는 총거리를 따라가는 경험식**이다 (`tools/fit_roll.py`).
//! 공개 TrackMan 드라이버 자료에서 TrackMan 의 Total − Carry 를 우리 모형의 예측 착지 상태로 회귀했다(2차 다항식 6계수).
//! 실측 세션 단위 보류 점검 (공유위기12 의 예측 착지 상태): 롤 RMSE 4.9 yd, 총거리 RMSE 8.2 yd (상수 롤 12.5, 롤 없음 27.0).
//!
//! 한계 (README "착지 상태 결함과 총거리"):
//! - 드라이버(92–163 mph, 발사각 3–22°, 스핀 1650–5600 rpm)로만 훈련했다. 그 상자 밖에서는 `launch_in_range` 가 false 다. 아이언·웨지는 미검증.
//! - 비행 모형의 착지 속력(−5 mph)·착지각(+1.9°) 편향이 실측 보정으로 풀리지 않아서, 이 롤 모형이 그 편향을 안고 맞춰져 있다.
//!   `Coef::SHARED_RE` 가 바뀌면 `tools/fit_roll.py` 로 계수를 다시 얻어야 한다.
//! - 바람·고도에서의 롤은 검증하지 않았다 (자료에 환경 기록이 없다).

use crate::{Landing, MPH};

/// 롤 모형 하나: roll = W · [1, a, v, a², a·v, v²],  a = (착지각° − MU[0]) / SD[0],  v = (착지 속력 mph − MU[1]) / SD[1].
/// 훈련 자료(그 장의 예측 착지 상태) 1–99 분위 밖은 잘라서 쓴다 (다항식 외삽 방지).
pub struct RollFit {
    pub w: [f64; 6],
    pub mu: [f64; 2],
    pub sd: [f64; 2],
    pub angle_range: (f64, f64),
    pub speed_range: (f64, f64),
}

/// 공유위기12 (`tools/fit_roll.py`, 전체로 맞춤. 세션 보류 롤 RMSE 4.94, 총거리 8.20 yd)
pub const ROLL_12: RollFit = RollFit {
    w: [28.1568, -2.47379, 8.23485, -3.0593, -1.85228, -0.622516],
    mu: [31.9334, 56.0169],
    sd: [10.3959, 7.17672],
    angle_range: (8.8, 51.4),
    speed_range: (45.6, 78.8),
};

/// 공유위기12L (세션 보류 롤 RMSE 4.96, 총거리 7.90 yd)
pub const ROLL_12L: RollFit = RollFit {
    w: [27.6088, -3.83466, 6.8439, -2.60749, -2.72006, -1.05615],
    mu: [31.4279, 59.9328],
    sd: [9.93238, 6.21705],
    angle_range: (8.6, 49.8),
    speed_range: (48.3, 79.8),
};

/// 지금 고른 장(`set_field`)의 롤 모형
pub fn roll_fit() -> &'static RollFit {
    if crate::field() == 1 {
        &ROLL_12L
    } else {
        &ROLL_12
    }
}

/// 착지 속력, mph (지면 기준)
pub fn landing_mph(l: &Landing) -> f64 {
    let v = l.vel_ms;
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() / MPH
}

/// 롤, yd. 입력은 훈련 범위로 자른다. 음수는 0 으로.
pub fn roll_yd(land_deg: f64, land_mph: f64) -> f64 {
    let f = roll_fit();
    let a = (land_deg.clamp(f.angle_range.0, f.angle_range.1) - f.mu[0]) / f.sd[0];
    let v = (land_mph.clamp(f.speed_range.0, f.speed_range.1) - f.mu[1]) / f.sd[1];
    let w = &f.w;
    (w[0] + w[1] * a + w[2] * v + w[3] * a * a + w[4] * a * v + w[5] * v * v).max(0.0)
}

/// 드라이버 자료의 입력 상자 안인가 (이 안에서만 총거리를 믿을 만하다)
pub fn launch_in_range(ball_mph: f64, launch_deg: f64, spin_rpm: f64) -> bool {
    (92.0..=163.0).contains(&ball_mph) && (3.0..=22.0).contains(&launch_deg) && (1650.0..=5600.0).contains(&spin_rpm)
}

/// 착지 상태에서 총거리(yd). 캐리가 NaN 이면 NaN.
pub fn total_yd(l: &Landing) -> f64 {
    l.carry_m / crate::YARD + roll_yd(l.land_deg, landing_mph(l))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_is_in_golf_range_for_typical_driver() {
        // 우리 모형이 내는 전형적인 드라이버 착지 상태 근처: 롤은 수십 yd 이내의 양수
        let r = roll_yd(33.0, 56.0);
        assert!((15.0..40.0).contains(&r), "롤 {r:.1} yd");
    }

    #[test]
    fn roll_decreases_with_steeper_landing_and_is_clamped() {
        assert!(roll_yd(45.0, 56.0) < roll_yd(20.0, 56.0));
        assert_eq!(roll_yd(89.0, 56.0), roll_yd(roll_fit().angle_range.1, 56.0));
        assert!(roll_yd(33.0, 500.0).is_finite());
    }

    #[test]
    fn range_box() {
        assert!(launch_in_range(167.0 - 5.0, 10.9, 2686.0));
        assert!(!launch_in_range(120.0, 16.3, 7097.0)); // 7번 아이언: 스핀이 상자 밖
        assert!(!launch_in_range(86.0, 25.7, 8403.0)); // 웨지
    }
}
