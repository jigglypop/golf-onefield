"""적은 샷 보정 실험: 장 하나의 이득이 '보정에 드는 샷 수'에 있는지 판정한다.

PGA 7행에서 k개(1..7)를 고르는 모든 부분집합마다 모형을 맞추고, LPGA 6행(보류)으로 채점한다.
  B4 : 분리 법칙 C_D = d0 + d1 S, C_L = l0 S^p          (4 매개, k >= 4)
  F2 : 장 하나 (vev + h S) e^{i pi/4}                   (2 매개, k >= 2)
  Z1 : 장 하나, h/vev = 1/sqrt2 고정, vev 만 보정          (1 매개, k >= 1)
  Z0 : vev = pi/8, h/vev = 1/sqrt2                      (0 매개, 보정 없음)
초기값은 모두 일반값(전체 적합값을 쓰지 않음). 구조(pi/4, 1/sqrt2)는 PGA 전체를 본 뒤 정한 것이라
PGA 부분집합 성적은 참고만 하고, 판정은 LPGA 보류로 한다.

결과 (2026-10-08). 왼쪽: LPGA 보류 중앙/최악, 오른쪽: 보정에 안 쓴 나머지 PGA 중앙/최악 (yd)
   k  B4              F2              Z1
   1  -               -               3.59/4.43  0.85/1.90
   2  -               3.65/5.53 1.74/6.12   3.62/4.05  0.89/1.23
   3  -               3.63/4.65 1.04/4.54   3.65/3.92  0.92/1.20
   4  3.89/76.8 2.13/11.3  3.71/4.50 1.06/2.26   3.64/3.84  0.89/1.24
   5  3.48/25.1 1.63/7.19  3.69/4.03 0.89/1.46   3.66/3.78  0.80/1.42
   6  3.17/6.69 1.18/2.72  3.69/3.81 0.67/1.81   3.66/3.72  0.41/1.71
  Z0(보정 없음): LPGA 3.10, PGA 1.48
판독
  - Z1 은 1–2샷 보정으로 B4 의 6샷 보정보다 같은 집단 오차가 작거나 같고, 최악 사례가 훨씬 안정적이다
    (LPGA 최악 B4 76.8 yd vs Z1 4.4 yd).
  - 주의 1: Z1 의 구조(pi/4, 1/sqrt2)는 PGA 7행 전체를 본 뒤 정했다. 남은 PGA 점수에는 이 누설이 들어 있다.
  - 주의 2: LPGA 점수는 PGA–LPGA 체계적 차이에 묶여 있어 보정할수록 오히려 나빠진다(Z0 가 가장 좋음).
  - 주의 3: B4 는 정칙화 없이 맞췄다. 사전분포를 주면 적은 샷에서 덜 무너질 것이다.

python -B fewshot.py
"""

from __future__ import annotations

import itertools

import numpy as np
from scipy.optimize import least_squares

import onefield_carry as ref
from higgs_engine import Air, carry_yd

AIR = Air()
DT = 0.008
R2 = 1 / np.sqrt(2)


class G:
    def __init__(self, fn):
        self.gamma = fn


def m_B4(p):
    d0, d1, l0, pw = p
    return G(lambda S: (d0 + d1 * S) + 1j * (l0 * np.power(S, pw)))


def m_F2(p):
    vev, h = p
    return G(lambda S: (vev + h * S) * np.exp(1j * np.pi / 4) * np.ones_like(S))


def m_Z1(p):
    (vev,) = p
    return G(lambda S: vev * (1 + R2 * S) * np.exp(1j * np.pi / 4) * np.ones_like(S))


MODELS = {
    "B4": (m_B4, 4, [[0.22, 0.2, 0.5, 0.5], [0.3, 0.0, 0.3, 0.3]], ([0.05, -1, 0.01, 0.05], [0.6, 2, 2, 2])),
    "F2": (m_F2, 2, [[0.3, 0.3], [0.45, 0.1]], ([0.05, -2], [1.0, 3])),
    "Z1": (m_Z1, 1, [[0.3], [0.45]], ([0.05], [1.0])),
}


def carry(model, rows):
    return carry_yd(model, AIR, *rows[:, :3].T, dt=DT)


def rmse(e):
    return float(np.sqrt(np.mean(np.square(e))))


def fit(make, starts, bounds, rows):
    best = None
    for s in starts:
        r = least_squares(lambda p: np.nan_to_num(carry(make(p), rows) - rows[:, 3], nan=1e3),
                          s, bounds=bounds, x_scale="jac")
        if best is None or r.cost < best.cost:
            best = r
    return best.x


def main():
    z0 = rmse(carry(m_Z1([np.pi / 8]), ref.LPGA) - ref.LPGA[:, 3])
    z0p = rmse(carry(m_Z1([np.pi / 8]), ref.PGA) - ref.PGA[:, 3])
    print(f"Z0 (보정 없음): LPGA {z0:.2f} yd, PGA {z0p:.2f} yd\n")
    print(" k   모형  부분집합 | LPGA 중앙  LPGA 최악 | 남은PGA 중앙  남은PGA 최악")
    summary = {}
    for k in range(1, 8):
        subsets = list(itertools.combinations(range(7), k))
        for name, (make, npar, starts, bounds) in MODELS.items():
            if k < npar:
                continue
            ho, inn = [], []
            for idx in subsets:
                rows = ref.PGA[list(idx)]
                p = fit(make, starts, bounds, rows)
                ho.append(rmse(carry(make(p), ref.LPGA) - ref.LPGA[:, 3]))
                rest = ref.PGA[[i for i in range(7) if i not in idx]]
                if len(rest):
                    inn.append(rmse(carry(make(p), rest) - rest[:, 3]))
            ho = np.array(ho)
            inn = np.array(inn) if inn else np.array([np.nan])
            summary[(k, name)] = (ho, inn)
            print(f" {k}   {name}   {len(subsets):>5}   | {np.median(ho):8.2f}  {ho.max():8.2f} | "
                  f"{np.median(inn):10.2f}  {np.max(inn):10.2f}", flush=True)
    return summary


if __name__ == "__main__":
    main()
