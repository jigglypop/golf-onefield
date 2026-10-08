"""골프공 '장 하나' 모형의 캐리 검산.

CE OBJ-04의 구조(세 힘 = 공통장의 투영, 중력 = 배경)를 골프공에 옮긴다.
공 표면 응력장 t(x) 하나에서 항력·양력·스핀 감쇠 토크가 모두 투영으로 나온다.
2차원(순수 백스핀)에서는 공기력 계수를 복소수 하나 Gamma = C_D + i C_L 로 두면

    a_aero = -(rho A / 2m) |u| conj(Gamma) u,   u = v_x + i v_y

즉 공기력은 속도를 각도 psi = arg(Gamma) 만큼 돌리고 |Gamma| 만큼 키운 것이다.

자료: TrackMan 투어 평균 아이언 행 (GolfMagic 게재본에서 확인한 값만 사용).
  훈련: PGA 3i–9i (7행), 보류: LPGA 4i–9i (6행). 보류 자료로는 재피팅하지 않는다.
모형:
  B  : 분리 법칙 C_D = d0 + d1 S, C_L = l0 S^p                 (4 매개변수)
  F1 : 장 하나, |Gamma| = C0(1+eta S), psi = (pi/2)(1-e^{-S/s0}) (3 매개변수)
  F2 : 장 하나(오일러꼴), Gamma = C0 exp((lam + i kap) S)        (3 매개변수)
  F3 : 장 하나, 각도 잠금 psi -> pi/4, |Gamma| = c0 + c1 S       (3, 실효 2 매개변수)
  F4 : F3에서 점근 각도 자유                                      (4 매개변수)
스핀 감쇠는 모든 모형에서 C_M = 0.012 S 로 고정한다(Tavares 꼴, 이 자료로는 검증 불가).

결과 (2026-10-08, 훈련 PGA 7행 / 보류 LPGA 6행, RMSE yd)
  B  4  0.71 / 3.16   F1 3  5.43 / 26.08 [반례]   F2 3  5.10 / 8.76 [반례]
  F3 3  0.72 / 3.67   F4 4  0.70 / 3.34 (psi_inf = 43.1°)
  F3의 s0는 하한(0.0013)으로 가서 식별되지 않는다: 자료의 S가 0 근처에 없으므로
  실효적으로 Gamma = (c0 + c1 S) e^{i pi/4}, 곧 C_D = C_L 인 2 매개변수 모형이다.
  F3의 pi/4는 훈련 자료에서 B의 각도(40–42°)를 본 뒤 고른 [경험식]이다. 보류 자료는 보지 않았다.

python -B onefield_carry.py            # 전체
python -B onefield_carry.py B F3       # 일부
"""

from __future__ import annotations

import numpy as np
from scipy.integrate import solve_ivp
from scipy.optimize import least_squares

MPH = 0.44704
YD = 0.9144
M = 0.04593          # kg
R = 0.04267 / 2      # m
A = np.pi * R**2
RHO = 1.2            # kg/m^3 (해수면 근사, 자료의 대기 조건은 미상)
G = 9.81
I_BALL = 0.4 * M * R**2
K = RHO * A / (2 * M)

# (ball speed mph, launch deg, spin rpm, carry yd)
PGA = np.array([
    (142, 10.4, 4630, 212),  # 3i
    (137, 11.0, 4836, 203),  # 4i
    (132, 12.1, 5361, 194),  # 5i
    (127, 14.1, 6231, 183),  # 6i
    (120, 16.3, 7097, 172),  # 7i
    (115, 18.1, 7998, 160),  # 8i
    (109, 20.4, 8647, 148),  # 9i
], dtype=float)
LPGA = np.array([
    (116, 14.3, 4801, 169),  # 4i
    (112, 14.8, 5081, 161),  # 5i
    (109, 17.1, 5943, 152),  # 6i
    (104, 19.0, 6699, 141),  # 7i
    (100, 20.8, 7494, 130),  # 8i
    (93, 23.9, 7589, 119),   # 9i
], dtype=float)


def gamma_B(S, p):
    d0, d1, l0, pw = p
    return (d0 + d1 * S) + 1j * (l0 * S**pw)


def gamma_F1(S, p):
    c0, eta, s0 = p
    psi = 0.5 * np.pi * (1 - np.exp(-S / s0))
    return c0 * (1 + eta * S) * np.exp(1j * psi)


def gamma_F2(S, p):
    c0, lam, kap = p
    return c0 * np.exp((lam + 1j * kap) * S)


def gamma_F3(S, p):
    """[경험식] 각도 잠금: psi -> pi/4, 크기는 S에 선형. 훈련 자료의 B 각도(40–42°)를 본 뒤 세움."""
    c0, c1, s0 = p
    psi = 0.25 * np.pi * (1 - np.exp(-S / s0))
    return (c0 + c1 * S) * np.exp(1j * psi)


def gamma_F4(S, p):
    """F3에서 점근 각도 psi_inf를 자유로 둔 판본(4 매개변수, B와 같은 수)."""
    c0, c1, s0, psi_inf = p
    psi = psi_inf * (1 - np.exp(-S / s0))
    return (c0 + c1 * S) * np.exp(1j * psi)


MODELS = {
    "B ": (gamma_B, [0.22, 0.2, 0.5, 0.5], ([0.05, -1, 0.01, 0.05], [0.6, 2, 2, 2])),
    "F1": (gamma_F1, [0.24, 1.0, 0.2], ([0.05, -3, 0.005], [0.6, 10, 5])),
    "F2": (gamma_F2, [0.24, 1.0, 3.0], ([0.05, -10, 0.0], [0.6, 10, 20])),
    "F3": (gamma_F3, [0.3, 0.2, 0.03], ([0.05, -1, 0.001], [0.8, 2, 1])),
    "F4": (gamma_F4, [0.3, 0.2, 0.03, 0.75], ([0.05, -1, 0.001, 0.1], [0.8, 2, 1, 1.5])),
}


def carry_yd(gamma, p, ball_mph, launch_deg, spin_rpm):
    v0 = ball_mph * MPH
    th = np.radians(launch_deg)
    w0 = spin_rpm * 2 * np.pi / 60

    def rhs(t, s):
        x, y, vx, vy, w = s
        sp = np.hypot(vx, vy)
        S = R * w / sp
        gm = gamma(S, p)
        u = vx + 1j * vy
        a = -K * sp * np.conj(gm) * u
        cm = 0.012 * S
        dw = -R * RHO * A * cm * sp**2 / I_BALL
        return [vx, vy, a.real, a.imag - G, dw]

    def land(t, s):
        return s[1]
    land.terminal = True
    land.direction = -1

    sol = solve_ivp(rhs, (0, 20), [0, 0, v0 * np.cos(th), v0 * np.sin(th), w0],
                    events=land, rtol=1e-8, atol=1e-10, max_step=0.05)
    if sol.t_events[0].size == 0:
        return np.nan
    return sol.y_events[0][0][0] / YD


def predict(gamma, p, data):
    return np.array([carry_yd(gamma, p, *row[:3]) for row in data])


def rmse(x):
    return float(np.sqrt(np.mean(x**2)))


def main():
    import sys
    names = sys.argv[1:] or [n.strip() for n in MODELS]
    print(f"{'모형':4} {'k':>2} {'훈련 RMSE(yd)':>14} {'보류 RMSE(yd)':>14}  매개변수")
    results = {}
    for name, (gamma, p0, bounds) in MODELS.items():
        if name.strip() not in names:
            continue
        best = None
        rng = np.random.default_rng(0)
        starts = [np.array(p0)] + [
            bounds[0] + rng.random(len(p0)) * (np.array(bounds[1]) - bounds[0])
            for _ in range(3)
        ]
        for s in starts:
            def res(p):
                r = predict(gamma, p, PGA) - PGA[:, 3]
                return np.nan_to_num(r, nan=1e3)
            fit = least_squares(res, s, bounds=bounds, x_scale="jac")
            if best is None or fit.cost < best.cost:
                best = fit
        p = best.x
        tr = predict(gamma, p, PGA) - PGA[:, 3]
        ho = predict(gamma, p, LPGA) - LPGA[:, 3]
        results[name] = (gamma, p, tr, ho, best)
        print(f"{name:4} {len(p):>2} {rmse(tr):>14.2f} {rmse(ho):>14.2f}  "
              + ", ".join(f"{v:.4g}" for v in p))

    print("\n함축된 계수 (C_D, C_L, psi=arg Gamma)")
    for name, (gamma, p, *_ ) in results.items():
        parts = []
        for S in (0.1, 0.2, 0.3):
            gm = gamma(S, p)
            parts.append(f"S={S}: ({gm.real:.3f}, {gm.imag:.3f}, {np.degrees(np.angle(gm)):.1f}°)")
        print(f"{name}  " + "  ".join(parts))

    print("\n보류(LPGA) 행별 오차 yd")
    for name, (_, _, _, ho, _) in results.items():
        print(f"{name}  " + " ".join(f"{v:+6.1f}" for v in ho))

    print("\n식별성: 훈련 야코비안 특이값 비(최대/최소)")
    for name, (*_, best) in results.items():
        sv = np.linalg.svd(best.jac, compute_uv=False)
        print(f"{name}  cond = {sv[0] / sv[-1]:.3g}")


if __name__ == "__main__":
    main()
