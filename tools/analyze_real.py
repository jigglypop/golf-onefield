"""공개 TrackMan 드라이버 자료 대조의 뒷분석: 세션·공 종류별 잔차, 같은 발사 조건 쌍의 흩어짐(바닥), 설명된 분산.

사용:
  tools/fetch_trackman_public.sh
  (cd flightbench && TM_CSV=../data/real/external/trackman_public/data.csv MODELS=models_search.txt \
     ./target/quick/real 공유Rec --dump=/tmp/res_prod.csv)
  PYTHONUTF8=1 python -B tools/analyze_real.py /tmp/res_prod.csv data/real/external/trackman_public/data.csv

입력 1 은 real --dump 가 쓴 행별 잔차 (idx = 원래 자료의 행 번호), 입력 2 는 TrackMan 원본.
"""
import csv
import itertools
import sys

import numpy as np

CLEAN = ("Premium", "Limited Distance Soft")  # 공 종류. Medium 은 따로 본다


def main(res_path, raw_path):
    res = list(csv.DictReader(open(res_path, encoding="utf-8")))
    raw = list(csv.reader(open(raw_path, encoding="utf-8")))[2:]  # 머리줄·단위줄 제외
    rows = sorted(((int(x["idx"]), raw[int(x["idx"])][1], x) for x in res), key=lambda t: t[0])
    f = lambda t, k: float(t[2][k])
    n = len(rows)
    e_all = np.array([f(t, "e_carry") for t in rows])
    print(f"추적 샷 {n}개: 캐리 RMSE {np.sqrt((e_all**2).mean()):.2f}  편향 {e_all.mean():+.2f}")

    # 1) 연속 세션 (행 번호 간격 40 초과면 새 세션)
    segs, cur = [], [rows[0]]
    for t in rows[1:]:
        if t[0] - cur[-1][0] > 40:
            segs.append(cur)
            cur = [t]
        else:
            cur.append(t)
    segs.append(cur)
    print(f"\n연속 세션 {len(segs)}개")
    for s in segs:
        e = np.array([f(t, "e_carry") for t in s])
        balls = "/".join(sorted({t[1][:8] for t in s}))
        print(f"  행 {s[0][0]:4d}-{s[-1][0]:4d} n={len(s):3d}  공 {balls:<28s} 편향 {e.mean():+6.1f}  표준편차 {e.std():5.1f}")

    # 2) 공 종류별
    print("\n공 종류별")
    for ball in sorted({t[1] for t in rows}):
        e = np.array([f(t, "e_carry") for t in rows if t[1] == ball])
        print(f"  {ball:<24s} n={len(e):4d}  편향 {e.mean():+6.1f}  RMSE {np.sqrt((e**2).mean()):5.1f}")

    # 3) 같은 발사 조건 쌍의 흩어짐 (같은 공, 같은 연속 구간 안). 잔차 차이 표준편차/√2 가 입력으로 설명 안 되는 몫의 크기
    def pairs(tol):
        out = []
        for _, grp in itertools.groupby(rows, key=lambda t: t[1]):
            g = list(grp)
            run, runs = [g[0]], []
            for t in g[1:]:
                if t[0] - run[-1][0] > 40:
                    runs.append(run)
                    run = [t]
                else:
                    run.append(t)
            runs.append(run)
            for r in runs:
                for a, b in itertools.combinations(r, 2):
                    if all(abs(f(a, k) - f(b, k)) <= d for k, d in zip(("mph", "vla", "rpm", "axis", "ld"), tol)):
                        out.append(f(a, "e_carry") - f(b, "e_carry"))
        return np.array(out)

    print("\n같은 발사 조건 쌍 (Δ볼스피드 mph, Δ발사각 도, Δ스핀 rpm, Δ축 도, Δ방향 도)")
    for tol in ((2.0, 0.8, 200, 5, 2.5), (3.0, 1.2, 300, 8, 4.0)):
        d = pairs(tol)
        if len(d) >= 5:
            print(f"  허용 {tol}: 쌍 {len(d):4d}  잔차 차이 표준편차/√2 = {d.std() / np.sqrt(2):.2f} yd")

    # 4) 깨끗한 공 + 이상치(25 yd 초과) 제외 부분집합의 설명된 분산
    keep = [t for t in rows if t[1] in CLEAN and abs(f(t, "e_carry")) <= 25]
    print(f"\n깨끗한 공, 이상치 제외 {len(keep)}샷 (제외 {sum(1 for t in rows if t[1] in CLEAN) - len(keep)}샷)")
    for name, y, e in (("캐리", "carry", "e_carry"), ("옆 편차", "side", "e_side"), ("최고 높이", "apex_yd", "e_apex"), ("착지각", "land", "e_land")):
        yy = np.array([f(t, y) for t in keep])
        ee = np.array([f(t, e) for t in keep])
        print(f"  {name:<6s} 실측 표준편차 {yy.std():6.2f}  RMSE {np.sqrt((ee**2).mean()):5.2f}  편향 {ee.mean():+5.2f}  R² {1 - (ee**2).mean() / yy.var():.3f}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
