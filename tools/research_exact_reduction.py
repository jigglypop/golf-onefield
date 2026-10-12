"""공유위기12 식의 호길이·일정 바람 축약을 독립 SciPy ODE로 검산한다.

production 엔진을 호출하지 않는 수학 검산이다. 기본 환경, 수평 평지,
고정 스핀축·beta, 균일 수평 바람만 다룬다. 성능이나 실측 정확도는 측정하지 않는다.
실행: python tools/research_exact_reduction.py
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

import numpy as np
import scipy
from scipy.integrate import solve_ivp

ROOT = Path(__file__).resolve().parents[1]
L = 53.53157581531039
U = 22.916037151920374
T = L / U
YD = 0.9144
RE_PER_U = U * 0.04267 * 1.2 / 1.81e-5
COEF = {"v": 0.1383, "e": 0.2540, "re0": 74003.0320,
        "d": 1.4838, "c": 2.8062, "l": 6.5014, "n": 12, "beta": 0.06}
U0 = COEF["re0"] / RE_PER_U
G = np.array([0.0, 1.0, 0.0])
SOLVER = {"method": "RK45", "rtol": 2e-11, "atol": 2e-13, "max_step": 0.03}


def source_provenance():
    """계수 변경을 확인하고 실행 시점의 원본 코드를 해시로 고정한다."""
    core = (ROOT / "golf-core/src/lib.rs").read_text(encoding="utf-8")
    block = re.search(r"SHARED_RE: Coef = Coef \{(.*?)\n    \};", core, re.S)
    if block is None:
        raise RuntimeError("공유위기12 계수 블록을 찾지 못했다")
    for key in ("v", "e", "d", "c", "l"):
        found = re.search(rf"\b{key}:\s*([0-9.]+)", block.group(1))
        if found is None or float(found.group(1)) != COEF[key]:
            raise RuntimeError(f"production {key}가 이 검산의 동결 계수와 다르다")
    if "74003.0320 / RE_PER_U" not in block.group(1) or "beta: BETA" not in block.group(1):
        raise RuntimeError("production Re0 또는 beta 참조가 달라졌다")
    if not re.search(r"pub const BETA: f64 = 0\.06;", core):
        raise RuntimeError("production beta가 달라졌다")
    paths = ["flightbench/src/engine.rs", "golf-core/src/lib.rs", "tools/research_exact_reduction.py"]
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths}


def aero(v, spin):
    q = float(np.linalg.norm(v))
    if q <= 1e-9:
        raise RuntimeError("호길이 좌표의 q=0 특이점에 접근했다")
    r = spin / (q + COEF["l"] * spin)
    cd = COEF["v"] + COEF["e"] / (1 + (q / U0) ** COEF["n"]) + COEF["d"] * r
    return q, cd, COEF["c"] * r


def landing(_, state):
    return state[1]


landing.terminal = True
landing.direction = -1


def integrate_time(initial, axis, wind=None):
    wind = np.zeros(3) if wind is None else wind

    def rhs(_, y):
        relative = y[3:6] - wind
        q, cd, cl = aero(relative, y[6])
        accel = -q * cd * relative + q * cl * np.cross(axis, relative) - G
        return np.r_[y[3:6], accel, -COEF["beta"] * y[6] * q]

    result = solve_ivp(rhs, (0.0, 20.0), initial, events=landing, **SOLVER)
    if not result.success or len(result.t_events[0]) != 1:
        raise RuntimeError("시간 ODE가 착지까지 완료되지 않았다")
    return result.t_events[0][0], result.y_events[0][0]


def integrate_arc(initial, axis):
    spin0 = initial[6]

    def rhs(distance, y):
        spin = spin0 * np.exp(-COEF["beta"] * distance)
        q, cd, cl = aero(y[3:6], spin)
        accel_per_distance = -cd * y[3:6] + cl * np.cross(axis, y[3:6]) - G / q
        return np.r_[y[3:6] / q, accel_per_distance, 1.0 / q]

    result = solve_ivp(rhs, (0.0, 40.0), np.r_[initial[:6], 0.0], events=landing, **SOLVER)
    if not result.success or len(result.t_events[0]) != 1:
        raise RuntimeError("호길이 ODE가 착지까지 완료되지 않았다")
    final = result.y_events[0][0]
    spin = spin0 * np.exp(-COEF["beta"] * result.t_events[0][0])
    return final[6], np.r_[final[:6], spin]


def make_initial(speed, angle_deg, spin_ratio, axis_angle):
    theta = np.deg2rad(angle_deg)
    velocity = np.array([speed * np.cos(theta), speed * np.sin(theta), 0.0])
    axis = np.array([0.0, np.sin(axis_angle), np.cos(axis_angle)])
    return np.r_[np.zeros(3), velocity, spin_ratio * speed], axis


def difference(t1, s1, t2, s2):
    return {
        "max_position_error_yd": float(np.max(np.abs(s1[:3] - s2[:3])) * L / YD),
        "time_error_s": float(abs(t1 - t2) * T),
        "max_velocity_error_m_s": float(np.max(np.abs(s1[3:6] - s2[3:6])) * U),
        "spin_state_error_dimensionless": float(abs(s1[6] - s2[6])),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "docs/research/exact-reduction-20261013.json")
    args = parser.parse_args()
    sources = source_provenance()
    shots = [(3.3, 11.0, .07, 0.0), (2.3, 17.0, .22, .4),
             (1.8, 24.0, .30, -.35), (1.2, 38.0, .40, .12)]
    winds = [np.array([.12, 0, 0]), np.array([-.15, 0, 0]),
             np.array([0, 0, .18]), np.array([.08, 0, -.22])]
    arc_cases, wind_cases = [], []
    for i, (shot, wind) in enumerate(zip(shots, winds)):
        initial, axis = make_initial(*shot)
        time_t, time_s = integrate_time(initial, axis)
        arc_t, arc_s = integrate_arc(initial, axis)
        arc_cases.append({"case": i + 1, "launch": list(shot),
                          "landing_time_s": float(time_t * T),
                          **difference(time_t, time_s, arc_t, arc_s)})
        world_t, world_s = integrate_time(initial, axis, wind)
        relative_initial = initial.copy()
        relative_initial[3:6] -= wind
        rel_t, rel_s = integrate_time(relative_initial, axis)
        boosted = rel_s.copy()
        boosted[:3] += wind * rel_t
        boosted[3:6] += wind
        wind_cases.append({"case": i + 1, "launch": list(shot),
                           "wind_dimensionless": wind.tolist(),
                           "spin_axis_in_original_frame": axis.tolist(),
                           **difference(world_t, world_s, rel_t, boosted)})
    checks = arc_cases + wind_cases
    maxima = {k: max(x[k] for x in checks) for k in difference(0, np.zeros(7), 0, np.zeros(7))}
    thresholds = {"max_position_error_yd": 5e-7, "time_error_s": 5e-9,
                  "max_velocity_error_m_s": 5e-8, "spin_state_error_dimensionless": 5e-10}
    passed = all(maxima[k] <= thresholds[k] for k in thresholds)
    report = {
        "status": "passed" if passed else "failed", "scipy_version": scipy.__version__,
        "numpy_version": np.__version__, "solver": SOLVER,
        "source_sha256": sources, "coefficients": COEF,
        "units": {"L_m": L, "U_m_s": U, "T_s": T, "Re_per_u": RE_PER_U},
        "launch_schema": ["speed_dimensionless", "launch_deg", "initial_spin_ratio", "axis_tilt_radians"],
        "interpretation": [
            "동일 공유위기12 ODE를 독립 Python으로 재작성한 수학 동치 검산. production 호출, 성능 및 실측 검증은 아니다.",
            "호길이 s=integral(|u| dt)에서 w=w0 exp(-beta s). 시간 출력에는 dt/ds=1/|u| 보조 적분이 필요해 전체 상태 수가 반드시 줄지는 않는다.",
            "균일 수평 바람에서 u_rel=u_ground-W, r_ground=r_rel+W t. 스핀축은 원래 좌표계의 전체 벡터를 그대로 유지했다.",
            "상대 발사 방향으로 좌표를 회전하면 스핀축에 x 성분이 생길 수 있다. production의 (0,sin(a),cos(a)) 1각도 입력 또는 기존 4축 표로 일반 옆바람을 표현한다는 검산이 아니다.",
            "기본 밀도·점성, 평지 y=0, 고정 beta·축. 돌풍, 높이별 밀도, 축 세차, 수직 바람, 지면 충돌은 범위 밖이다. Re_scale가 변하는 환경은 계수 위치도 조정해야 한다.",
        ],
        "arc_cases": arc_cases, "uniform_wind_cases": wind_cases,
        "combined_maxima": maxima, "thresholds": thresholds,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "out": str(args.out), "maxima": maxima}, ensure_ascii=False))
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
