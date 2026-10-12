"""총거리용 롤(굴러가는 거리) 모형: 우리 비행 모형이 예측한 착지 상태(착지각, 착지 속력) → TrackMan Total − Carry.

TrackMan 의 Total 은 레이더가 본 마지막 점 뒤를 TrackMan 의 지면 모형으로 이은 값이다(추적 샷에서 마지막 점은 Total 보다 중앙값 21.5 yd 앞).
그래서 이 모형은 '실제 잔디'가 아니라 'TrackMan 기본 표면이 표시하는 총거리'를 따라간다.
우리 모형의 착지 상태는 실측과 어긋나므로(착지 속력 −5 mph, 착지각 +2.8°) TrackMan 의 착지 상태가 아니라 **우리 예측 상태**로 훈련한다.

사용:
  tools/fetch_trackman_public.sh
  (cd flightbench && TM_CSV=../data/real/external/trackman_public/data.csv MODELS=models_search.txt \
     ./target/quick/real 공유Rec --balls="Premium,Limited Distance Soft" --trim=25 --dump=/tmp/res.csv)
  PYTHONUTF8=1 python -B tools/fit_roll.py /tmp/res.csv data/real/external/trackman_public/data.csv

비행 모형의 매개변수가 바뀌면 다시 돌려 계수를 새로 얻는다. 세션 단위 보류(LOSO)로 오차를 낸 뒤, 전체로 맞춘 계수를 러스트 상수로 찍는다.
"""
import csv
import sys

import numpy as np


def load(res_path, raw_path):
    raw = list(csv.reader(open(raw_path, encoding="utf-8")))
    hdr, data = raw[0], raw[2:]
    ix = {h: i for i, h in enumerate(hdr)}
    res = list(csv.DictReader(open(res_path, encoding="utf-8")))
    idx = np.array([int(x["idx"]) for x in res])
    col = lambda k: np.array([float(data[i][ix[k]]) for i in idx])
    g = lambda k: np.array([float(x[k]) for x in res])
    return {
        "idx": idx,
        "carry_pred": g("carry") + g("e_carry"),
        "land_deg": g("pred_land"),
        "land_mph": g("pred_land_mph"),
        "tm_total": col("Total"),
        "tm_carry": col("Carry Flat - Length"),
    }


def sessions(idx):
    order = np.argsort(idx)
    sess = np.zeros(len(idx), int)
    cur = 0
    for a, b in zip(order[:-1], order[1:]):
        if idx[b] - idx[a] > 40:
            cur += 1
        sess[b] = cur
    return sess


def design(deg_deg, mph, mu, sd):
    """2차 다항식: 1, a, v, a², a v, v²  (a, v 는 표준화한 착지각, 착지 속력)"""
    a = (deg_deg - mu[0]) / sd[0]
    v = (mph - mu[1]) / sd[1]
    return np.column_stack([np.ones_like(a), a, v, a * a, a * v, v * v])


def fit(a, v, y, lam=1e-3):
    mu = np.array([a.mean(), v.mean()])
    sd = np.array([a.std(), v.std()])
    F = design(a, v, mu, sd)
    w = np.linalg.solve(F.T @ F + lam * np.eye(F.shape[1]), F.T @ y)
    return w, mu, sd


def main(res_path, raw_path):
    d = load(res_path, raw_path)
    roll = d["tm_total"] - d["tm_carry"]
    sess = sessions(d["idx"])
    rm = lambda e: float(np.sqrt((e**2).mean()))
    n_s = sess.max() + 1
    pred = np.zeros(len(roll))
    for s in range(n_s):
        tr = sess != s
        w, mu, sd = fit(d["land_deg"][tr], d["land_mph"][tr], roll[tr])
        pred[~tr] = design(d["land_deg"][~tr], d["land_mph"][~tr], mu, sd) @ w
    tot_err = d["carry_pred"] + pred - d["tm_total"]
    print(f"샷 {len(roll)}, 세션 {n_s}개 {[int((sess == s).sum()) for s in range(n_s)]}")
    print(f"롤 실측 평균 {roll.mean():.1f} 표준편차 {roll.std():.1f} yd")
    print(f"세션 단위 보류: 롤 RMSE {rm(pred - roll):.2f} yd, 총거리 RMSE {rm(tot_err):.2f} yd, 총거리 편향 {tot_err.mean():+.2f}")
    base = np.array([roll[sess != s].mean() for s in sess])
    print(f"비교: 상수 롤 총거리 RMSE {rm(d['carry_pred'] + base - d['tm_total']):.2f}, 롤 없음 {rm(d['carry_pred'] - d['tm_total']):.2f}")
    w, mu, sd = fit(d["land_deg"], d["land_mph"], roll)
    print("\n// 전체로 맞춘 계수 (tools/fit_roll.py). roll_yd = w · [1, a, v, a², a·v, v²],  a = (착지각° − MU[0]) / SD[0],  v = (착지 속력 mph − MU[1]) / SD[1]")
    print("pub const ROLL_W: [f64; 6] = [" + ", ".join(f"{x:.6}" for x in w) + "];")
    print(f"pub const ROLL_MU: [f64; 2] = [{mu[0]:.6}, {mu[1]:.6}];")
    print(f"pub const ROLL_SD: [f64; 2] = [{sd[0]:.6}, {sd[1]:.6}];")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
