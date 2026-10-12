/**
 * 무차원 단위와 상수. flightbench/src/engine.rs·harness.rs 와 같은 값.
 *
 * 길이 단위 $L = 2m/(\rho A)$, 속도 단위 $U = \sqrt{gL}$, 시간 단위 $T = U/g$.
 * 무차원 식: $\dot{\mathbf u} = -k_d\,\mathbf u + k_l\,(\hat\omega\times\mathbf u) - \hat y$, $\dot w = -\beta\,w\,|\mathbf u|$.
 */

/** 중력 가속도 (m/s²) */
export const G = 9.81;
/** 길이 단위 $L = 2m/(\rho A)$ (m), rho = 1.2 */
export const L_M = 53.53157581531039;
/** 속도 단위 $U = \sqrt{gL}$ (m/s) */
export const U_MS = 22.916037151920374;
/** 시간 단위 $T = U/g$ (s) */
export const T_S = U_MS / G;
/** 무차원 길이 → yd */
export const YD = L_M / 0.9144;
/** 1 yd (m) */
export const M_PER_YD = 0.9144;
/** 공 반지름 (m) */
export const R_BALL = 0.04267 / 2;
/** 1 mph (m/s) */
export const MPH = 0.44704;
/** 레이놀즈 수 = |u| × RE_PER_U (공기 rho 1.2, 점성 1.81e-5, 지름 42.67 mm) */
export const RE_PER_U = (22.916037151920374 * 0.04267 * 1.2) / 1.81e-5;

/**
 * (볼스피드 mph, 발사각 deg, 스핀 rpm) → [u0, θ (rad), S0 = Rω/v]. engine.rs `dimless` 그대로.
 * 무차원 스핀 $w_0 = S_0 u_0 = R\omega/U$.
 */
export function dimless(mph: number, deg: number, rpm: number): [number, number, number] {
  const v = mph * MPH;
  return [v / U_MS, (deg * Math.PI) / 180, (R_BALL * rpm * 2 * Math.PI) / 60 / v];
}
