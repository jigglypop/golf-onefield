"""러스트 일치 검사용 기준값 refs_models.csv 를 만든다 (flightbench/src/bin/h2.rs 0절이 읽는다).

행: PGA 아이언 7행, LPGA 아이언 6행, USGA 평균 입력 20행 (33행).
열: v0', theta, S0, 그리고 H2·B4·F3 의 무차원 캐리 (파이썬 fly, dt' = 0.0005).
H2 는 PGA 7행 맞춤값(usga_v2.fit_H2), B4·F3 는 이전 판 고정값이다.

python -B export_refs_models.py
"""

import numpy as np

import onefield_carry as ref
from fewshot import m_B4, m_F2
from higgs_engine import Air, fly, to_dimless
from usga_holdout import table
from usga_v2 import m_H2

MODELS = [m_H2([0.2705, 0.1536, 3.1361, 10.167]), m_B4([0.2627, 0.1852, 0.3612, 0.1765]), m_F2([0.3881, 0.2714])]


def main():
    t = table()
    rows = np.vstack([ref.PGA[:, :3], ref.LPGA[:, :3], np.column_stack([t["mph"], t["deg"], t["rpm"]])])
    v0, th, S0 = to_dimless(Air(), *rows.T)
    cols = [v0, th, S0] + [fly(m, v0, th, S0, dt=0.0005)["carry"] for m in MODELS]
    np.savetxt("refs_models.csv", np.column_stack(cols), delimiter=",", fmt="%.12g")


if __name__ == "__main__":
    main()
