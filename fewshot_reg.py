"""정칙화한 적은 샷 비교: 분리 모형(B4)에도 사전분포를 주고 다시 판정한다.

fewshot.py 의 B4 는 정칙화가 없어 적은 샷에서 무너졌다. 공정하게 하려고
B4 와 F2 에 '골프공 계수의 흔한 크기' 수준의 약한 가우스 사전분포를 준다.
사전분포 중심은 우리 적합값을 쓰지 않고 정했다(C_D 0.25 안팎, C_L 0.3 안팎, 넓은 폭).
MAP 추정: 관측 잔차(yd) 옆에 (p - mu)/sigma 를 덧붙인다 (관측 잡음 1 yd 로 둔 것과 같다).

  B4r : C_D = d0 + d1 S, C_L = l0 S^p,   mu = (0.25, 0.20, 0.40, 0.40), sigma = (0.05, 0.20, 0.20, 0.30)
  F2r : (vev + h S) e^{i pi/4},           mu = (0.35, 0.30),             sigma = (0.10, 0.20)
  Z1  : h/vev = 1/sqrt2 고정, vev 만 (사전분포 없음) — fewshot.py 와 같음

결과 (2026-10-08). 보정에 안 쓴 나머지 PGA, 중앙/최악 (yd). 괄호는 LPGA 최악
   k  B4r                 F2r                 Z1
   1  4.97/8.00 (6.37)    1.83/2.89 (4.33)    0.85/1.90 (4.43)
   2  1.42/6.87 (6.12)    1.66/4.31 (4.70)    0.89/1.23 (4.05)
   3  1.35/5.56 (4.84)    1.03/4.31 (4.55)    0.92/1.20 (3.92)
   4  1.19/5.77 (4.61)    1.07/2.26 (4.43)    0.89/1.24 (3.84)
   5  1.27/1.88 (3.66)    0.89/1.47 (3.98)    0.80/1.42 (3.78)
   6  0.41/2.31 (3.48)    0.64/1.81 (3.80)    0.41/1.71 (3.72)
  사전분포 중심만: B4r PGA 5.45 / F2r PGA 5.88 yd
판독: 정칙화로 B4 의 파국(LPGA 최악 76.8 yd)은 사라졌다. 그래도 k <= 4 에서 Z1 의 중앙값과 최악값이
가장 작고, k = 6 에서 셋이 비슷해진다. Z1 구조의 PGA 누설 주의는 fewshot.py 와 같다.

python -B fewshot_reg.py
"""

from __future__ import annotations

import itertools

import numpy as np
from scipy.optimize import least_squares

import onefield_carry as ref
from fewshot import carry, m_B4, m_F2, m_Z1, rmse

PRIORS = {
    "B4r": (m_B4, np.array([0.25, 0.20, 0.40, 0.40]), np.array([0.05, 0.20, 0.20, 0.30]),
            ([0.05, -1, 0.01, 0.05], [0.6, 2, 2, 2])),
    "F2r": (m_F2, np.array([0.35, 0.30]), np.array([0.10, 0.20]), ([0.05, -2], [1.0, 3])),
    "Z1": (m_Z1, None, None, ([0.05], [1.0])),
}


def fit(make, mu, sd, bounds, rows):
    p0 = mu if mu is not None else np.array([0.3])

    def res(p):
        r = np.nan_to_num(carry(make(p), rows) - rows[:, 3], nan=1e3)
        return r if mu is None else np.concatenate([r, (p - mu) / sd])

    return least_squares(res, p0, bounds=bounds, x_scale="jac").x


def main():
    print(" k   모형   부분집합 | LPGA 중앙  LPGA 최악 | 남은PGA 중앙  남은PGA 최악")
    for k in range(1, 7):
        subsets = list(itertools.combinations(range(7), k))
        for name, (make, mu, sd, bounds) in PRIORS.items():
            ho, inn = [], []
            for idx in subsets:
                rows = ref.PGA[list(idx)]
                p = fit(make, mu, sd, bounds, rows)
                ho.append(rmse(carry(make(p), ref.LPGA) - ref.LPGA[:, 3]))
                rest = ref.PGA[[i for i in range(7) if i not in idx]]
                inn.append(rmse(carry(make(p), rest) - rest[:, 3]))
            ho, inn = np.array(ho), np.array(inn)
            print(f" {k}   {name:<4}  {len(subsets):>5}   | {np.median(ho):8.2f}  {ho.max():8.2f} | "
                  f"{np.median(inn):10.2f}  {inn.max():10.2f}", flush=True)
    # 사전분포 자체(보정 0샷)의 성적
    for name, (make, mu, sd, _) in PRIORS.items():
        if mu is not None:
            print(f"사전분포 중심만 쓴 {name}: LPGA {rmse(carry(make(mu), ref.LPGA) - ref.LPGA[:, 3]):.2f}, "
                  f"PGA {rmse(carry(make(mu), ref.PGA) - ref.PGA[:, 3]):.2f} yd")


if __name__ == "__main__":
    main()
