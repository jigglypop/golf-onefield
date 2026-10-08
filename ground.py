"""지면 접촉: 착지 상태 → 바운스·미끄럼·구름 → 런.

비행 장은 착지 상태 (vx, vy, w) 를 이미 준다. 지면은 닫힌 꼴로 풀려서 따로 펼칠 필요가 없다.
  총거리 = 캐리(비행 장) + 런(착지 상태의 닫힌 꼴)

바운스: Penner 꼴 강체 충돌. 잔디의 변형을 '유효각 beta 만큼 기운 단단한 면'으로 본다.
  기운 면의 법선·접선 성분으로 나눠, 법선은 반발계수 r, 접선은 쿨롱 마찰 mu 로 미끄럼을 줄이되
  미끄럼이 0 이 되면 멈춘다(구름으로 이탈). I = 0.4 m R^2.
  계수는 Biber 등 (Sports Engineering 2024, arXiv 2302.02758) 의 Penner 고정 beta 적합값,
  실제 티 구역 잔디(Campaign B): beta = 18.4°, r = 0.147, mu = 0.998.
튀어 오름: 낮고 느려서 공기력을 빼고 포물선으로 둔다 [가정].
미끄럼: 운동 마찰 mu_k g 로 구름 조건 v = -R omega 에 닿을 때까지 [가정 mu_k = 0.4].
구름: 일정한 구름 저항 감속 a_r 로 멈출 때까지. a_r 는 이 파일의 유일한 자유 매개변수다.

결과 (2026-10-08) [반례]: USGA 런(총거리 - 캐리)에서 a_r 를 남자 프로 드라이버 하나로 맞추고 19행 채점.
  비행 F3 + 잔디 A: 보류 RMSE 69.0 / F3 + B: 19.8 / B4 + A: 20.6 / B4 + B: 14.8 yd
  기준(런 평균 22.9 yd 하나로 찍기): 6.9 yd. 모든 조합이 기준보다 나쁘다.
  맞춘 a_r 는 0.001–0.013 m/s^2 (스팀프 환산 130–1500 m) 로 비물리적이다.
  원인 후보: 브리스톨 계수는 부드러운 티 잔디의 저속 충돌값이라 단단한 투어 페어웨이와 다르다,
  장 하나 비행이 드라이버 착지각을 50° 로 가파르게 준다(B4 는 46°), USGA 총거리의 산출 방법이 적혀 있지 않다.
  지면 모형은 검증되지 않은 상태로 둔다. 페어웨이 바운스 실측이 필요하다.

python -B ground.py
"""

from __future__ import annotations

import numpy as np
from scipy.optimize import brentq

from higgs_engine import Air, Field, fly, to_dimless
from usga_holdout import table

G = 9.81
R = 0.04267 / 2
YD = 0.9144
TURF = {"beta": np.radians(18.4), "r": 0.147, "mu": 0.998}
MU_K = 0.4
FIELD = Field(vev=0.3881, h=0.2714)


def bounce(vx, vy, om, beta, r, mu):
    """Penner 강체 충돌. om > 0 이 백스핀. 반환: 이탈 (vx, vy, om)."""
    sb, cb = np.sin(beta), np.cos(beta)
    vn = -vx * sb + vy * cb          # 기운 면 법선 성분 (접근 중 < 0)
    vt = vx * cb + vy * sb           # 접선 성분
    vc = vt + R * om                 # 접점 미끄럼 속도
    J = (1 + r) * np.abs(vn)
    dvt = -np.sign(vc) * np.minimum(mu * J, (2 / 7) * np.abs(vc))
    vt2, vn2 = vt + dvt, -r * vn
    om2 = om + 2.5 * dvt / R
    return vt2 * cb - vn2 * sb, vt2 * sb + vn2 * cb, om2


def run_out(vx, vy, om, a_r, turf=TURF, v_hop=0.3, max_bounce=30):
    """착지 상태에서 멈출 때까지의 수평 거리 (m). 벡터화."""
    vx, vy, om = (np.array(a, float) for a in (vx, vy, om))
    dist = np.zeros_like(vx)
    rolling = np.zeros(vx.shape, bool)
    for _ in range(max_bounce):
        act = ~rolling
        if not act.any():
            break
        bx, by, bo = bounce(vx[act], vy[act], om[act], **turf)
        hop = by > v_hop
        t = np.where(hop, 2 * by / G, 0.0)
        dist[act] += bx * t
        vx[act], vy[act], om[act] = bx, np.where(hop, -by, 0.0), bo
        idx = np.flatnonzero(act)
        rolling[idx[~hop]] = True
    # 미끄럼 → 구름
    vc = vx + R * om
    ts = np.abs(vc) / (3.5 * MU_K * G)
    s = np.sign(vc)
    dist += vx * ts - 0.5 * s * MU_K * G * ts**2
    v_roll = vx - s * MU_K * G * ts
    dist += np.sign(v_roll) * v_roll**2 / (2 * a_r)
    return dist


TURFS = {
    "A 인조 티 잔디": {"beta": np.radians(12.9), "r": 0.420, "mu": 0.852},
    "B 실제 티 구역": {"beta": np.radians(18.4), "r": 0.147, "mu": 0.998},
}


def landing_state(mph, deg, rpm, model=FIELD, air=Air()):
    v0, th, S0 = to_dimless(air, mph, deg, rpm)
    o = fly(model, v0, th, S0, dt=0.004)
    U = air.U
    return o["carry"] * air.L / YD, o["vx"] * U, o["vy"] * U, o["w"] * U / R


def main():
    from fewshot import m_B4
    t = table()
    run_obs = t["total"] - t["carry"]
    cal = int(np.flatnonzero((t["sex"] == "M") & (t["club"] == "DR") & (t["grp"] == "Pro"))[0])
    test = np.arange(len(USGA_LABELS(t))) != cal
    flights = {"장 하나 F3": FIELD, "분리 B4": m_B4([0.2627, 0.1852, 0.3612, 0.1765])}
    labels = USGA_LABELS(t)

    print("착지각 (deg): " + "  ".join(f"{k}" for k in flights))
    states = {k: landing_state(t["mph"], t["deg"], t["rpm"], m) for k, m in flights.items()}
    for i, lab in enumerate(labels):
        print(f"  {lab:10} " + "  ".join(
            f"{np.degrees(np.arctan2(-s[2][i], s[1][i])):6.1f}° {s[3][i] * 60 / (2 * np.pi):5.0f}rpm" for s in states.values()))

    print("\n유일한 자유 매개변수 a_r 를 남자 프로 드라이버 런 하나로 맞추고 나머지 19행으로 채점")
    results = {}
    for fname, (_, vx, vy, om) in states.items():
        for tname, turf in TURFS.items():
            f = lambda a: run_out(vx[[cal]], vy[[cal]], om[[cal]], a, turf)[0] / YD - run_obs[cal]
            run_max = run_out(vx[[cal]], vy[[cal]], om[[cal]], 1e-3, turf)[0] / YD
            if f(1e-3) < 0:
                print(f"  [{fname} / {tname}] 성립 안 함: 구름 저항 0 에서도 런 {run_max:.1f} yd < {run_obs[cal]:.1f} yd")
                continue
            a_r = brentq(f, 1e-3, 50.0)
            run_p = run_out(vx, vy, om, a_r, turf) / YD
            results[(fname, tname)] = run_p
            e = run_p - run_obs
            pro = test & (t["grp"] == "Pro")
            print(f"  [{fname} / {tname}] a_r={a_r:.3f} m/s² (스팀프 환산 {1.83**2 / (2 * a_r):.1f} m)  "
                  f"보류 19행 RMSE {np.sqrt(np.mean(e[test]**2)):5.2f}, 보류 프로 3행 {np.sqrt(np.mean(e[pro]**2)):5.2f} yd")
    for (fname, tname), run_p in results.items():
        print(f"\n런 (yd) [{fname} / {tname}]: 실측 | 예측 | 차")
        for i, lab in enumerate(labels):
            mark = "  ← 보정" if i == cal else ""
            print(f"  {lab:10} {run_obs[i]:6.1f} | {run_p[i]:6.1f} | {run_p[i] - run_obs[i]:+6.1f}{mark}")
    print(f"\n비교 기준: 런 평균({run_obs[test].mean():.1f} yd) 하나로 찍으면 보류 RMSE "
          f"{np.sqrt(np.mean((run_obs[test].mean() - run_obs[test]) ** 2)):.2f} yd")


def USGA_LABELS(t):
    return [f"{t['sex'][i]} {t['club'][i]} {t['grp'][i]}" for i in range(len(t["sex"]))]


if __name__ == "__main__":
    main()
