"""CE 최신 힉스 연구(ce-dimension-qgt-bridge 브랜치, 2026-10-02 문서)의 두 원리를 반영한 재판정.

원리 1. q[<Psi>] 와 <q[Psi]> 는 다르다 (interlocking_higgs_shape_and_score.md §3)
  USGA 행은 샷 수백~수천 개의 평균이다. 평균 입력 하나의 캐리가 아니라, 보고된 입력 표준편차로
  캐리의 분포 평균을 낸다. 입력은 서로 독립인 정규분포로 둔다 [가정: 상관은 보고되지 않음].

원리 2. 반경과 영향 결합 (higgs_influence_and_clock_bridge.md §2)
  R = sqrt(2 Phi^dag Phi), 결합 g = dm/dR |_v. 골프공 장을 배경(항력)과 스핀 들뜸(양력)의 두 성분으로 나누고,
  양력을 반경 노름으로 포화시킨다:
     H2: C_D = v + d S,   C_L = c S / sqrt(1 + lambda^2 S^2)
  S -> 0 에서 C_L -> 0 이고 기울기 dC_L/dS|_0 = c 가 영향 결합 자리다. |u| C_L = c w |u| / sqrt(|u|^2 + lambda^2 w^2)
  로 초월함수 없이 sqrt 하나로 계산한다.
  [주의] 이 구조는 USGA 에서 각도 잠금이 진 것을 본 뒤 골랐다. 매개변수는 PGA 7행으로만 맞춘다.

결과 (2026-10-08)
  H2 적합: v=0.2705 d=0.1536 c=3.136 lambda=10.17, PGA 훈련 0.74 / LPGA 보류 2.96 yd (B4 3.16)
  캐리 RMSE yd, 전체 20행:            B4     F3     Z0     H2
    평균 입력 하나 (q[<Psi>])        11.60  14.68  14.42  11.85
    입력 분포 평균 (<q[Psi]>)         8.96  12.09  11.92   9.93
  평균 부호 오차: 분포 평균에서 B4 +3.97, F3 +7.64, Z0 +6.97, H2 +1.65 yd
  아마추어 16행 (분포 평균): B4 7.27, F3 11.84, Z0 11.44, H2 5.85
  6번 아이언 10행 (분포 평균): B4 5.50, H2 5.31
판독
  - 원리 1 은 네 모형 모두에서 오차를 줄인다 (B4 -23%, F3 -18%, Z0 -17%, H2 -16%). 아마추어 과대 예측의 큰 몫이
    '평균 입력의 캐리'와 '캐리의 평균'의 차이에서 왔다.
  - H2 는 아마추어·아이언에서 가장 좋고 치우침이 가장 작다. 남자 프로 드라이버는 분포 평균에서 -22 yd 로 무너진다:
    보고된 스핀 표준편차(2259 ± 1120 rpm)로 표본의 상당수가 저스핀이 되고, H2 는 S -> 0 에서 양력이 0 으로 간다.
    투어 프로의 스핀 산포로는 지나치게 크고 캐리 sd 66.9 yd 도 이상해, 이 행의 입력 분포 자체가 의심스럽다.
    다만 그 판단으로 행을 빼지는 않는다.
  - 각도 잠금(F3, Z0)은 모든 구간에서 가장 나쁘다. 기본 장으로 쓰지 않는다.

python -B usga_v2.py
"""

from __future__ import annotations

import numpy as np
from scipy.optimize import least_squares

import onefield_carry as ref
from fewshot import G, carry, m_B4, m_F2, m_Z1, rmse
from usga_holdout import table

# USGA R22-01 Table 6 의 입력 표준편차 (볼스피드 mph, 발사각 deg, 스핀 rpm), usga_holdout.USGA 와 같은 순서
SD_IN = np.array([
    (5.7, 1.8, 678), (8.2, 2.9, 955), (15.1, 3.2, 1252), (9.5, 5.2, 1230), (13.5, 4.9, 1565),
    (11.3, 2.7, 910), (10.6, 2.9, 1057), (9.8, 4.6, 983), (9.1, 3.9, 909), (13.9, 5.1, 1392),
    (9.2, 2.4, 645), (9.9, 3.0, 925), (13.2, 4.0, 1191), (15.0, 4.7, 1333), (19.2, 5.1, 1492),
    (6.6, 2.3, 1120), (9.9, 2.8, 780), (13.1, 3.6, 1140), (15.3, 4.9, 1250), (16.6, 4.8, 1309),
])


def m_H2(p):
    v, d, c, lam = p
    return G(lambda S: (v + d * S) + 1j * (c * S / np.sqrt(1 + (lam * S) ** 2)))


def fit_H2():
    def res(p):
        return np.nan_to_num(carry(m_H2(p), ref.PGA) - ref.PGA[:, 3], nan=1e3)
    best = None
    for s in ([0.25, 0.2, 2.0, 5.0], [0.22, 0.3, 4.0, 12.0], [0.3, 0.1, 1.5, 2.0]):
        r = least_squares(res, s, bounds=([0.05, -1, 0.0, 0.0], [0.6, 2, 20, 80]), x_scale="jac")
        if best is None or r.cost < best.cost:
            best = r
    return best.x


def dist_mean_carry(model, mu, sd, n=1500, seed=0):
    """입력 분포로 캐리 평균. 볼스피드 >= 20 mph, 발사각 >= 0.5°, 스핀 >= 200 rpm 으로 자른다."""
    rng = np.random.default_rng(seed)
    out = []
    for m, s in zip(mu, sd):
        z = rng.standard_normal((n, 3))
        x = m + z * s
        x[:, 0] = np.maximum(x[:, 0], 20)
        x[:, 1] = np.maximum(x[:, 1], 0.5)
        x[:, 2] = np.maximum(x[:, 2], 200)
        rows = np.column_stack([x, np.zeros(n)])
        out.append(np.nanmean(carry(model, rows)))
    return np.array(out)


def main():
    p_h2 = fit_H2()
    tr = rmse(carry(m_H2(p_h2), ref.PGA) - ref.PGA[:, 3])
    ho = rmse(carry(m_H2(p_h2), ref.LPGA) - ref.LPGA[:, 3])
    print(f"H2 적합 (PGA 7행): v={p_h2[0]:.4f} d={p_h2[1]:.4f} c={p_h2[2]:.4f} lambda={p_h2[3]:.3f}  "
          f"훈련 {tr:.2f} / LPGA {ho:.2f} yd")
    for S in (0.0, 0.067, 0.1, 0.2, 0.3):
        g = m_H2(p_h2).gamma(np.array([S]))[0]
        print(f"   S={S:<5} C_D={g.real:.3f} C_L={g.imag:.3f}")

    models = {
        "B4 (4매개)": m_B4([0.2627, 0.1852, 0.3612, 0.1765]),
        "F3 (2매개)": m_F2([0.3881, 0.2714]),
        "Z0 (0매개)": m_Z1([np.pi / 8]),
        "H2 (4매개)": m_H2(p_h2),
    }
    t = table()
    mu = np.column_stack([t["mph"], t["deg"], t["rpm"]])
    rows = np.column_stack([mu, t["carry"]])
    point = {k: carry(m, rows) for k, m in models.items()}
    dist = {k: dist_mean_carry(m, mu, SD_IN) for k, m in models.items()}

    groups = {
        "프로 4행": t["grp"] == "Pro",
        "프로 3행(여 드라이버 제외)": (t["grp"] == "Pro") & ~((t["sex"] == "F") & (t["club"] == "DR")),
        "전체 20행": np.ones(len(mu), bool),
        "6번 아이언 10행": t["club"] == "6i",
        "드라이버 10행": t["club"] == "DR",
        "아마추어 16행": t["grp"] != "Pro",
    }
    for label, pred in (("평균 입력 하나 (q[<Psi>])", point), ("입력 분포 평균 (<q[Psi]>)", dist)):
        print(f"\n캐리 RMSE (yd) — {label}")
        print(f"{'':26} " + " ".join(f"{k:>11}" for k in models))
        for g, m in groups.items():
            print(f"{g:26} " + " ".join(f"{rmse(pred[k][m] - t['carry'][m]):11.2f}" for k in models))
        print(f"{'평균 부호 오차(전체)':26} " + " ".join(f"{np.mean(pred[k] - t['carry']):+11.2f}" for k in models))

    print("\n행별 (분포 평균, 차 yd)")
    print(f"{'':12} {'실측':>6} " + " ".join(f"{k[:2]:>7}" for k in models))
    for i in range(len(mu)):
        lab = f"{t['sex'][i]} {t['club'][i]} {t['grp'][i]}"
        print(f"{lab:12} {t['carry'][i]:6.1f} " + " ".join(f"{dist[k][i] - t['carry'][i]:+7.1f}" for k in models))


if __name__ == "__main__":
    main()
