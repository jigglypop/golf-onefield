"""매개변수 0개 가설 검산.

Z: Gamma = (pi/8)(1 + S/sqrt2) e^{i pi/4}  — 적합값(vev 0.388, h/vev 0.699)을 본 뒤 세운 [가설].
수치 일치가 우연인지 보려고, 같은 꼴의 '단순한 상수' 후보 쌍 전체를 똑같이 채점한다(눈길 효과 점검).

결과 (2026-10-08): Z 훈련 1.48 / 보류 3.10 yd. 단순 상수 42쌍 중 훈련 < 1 yd 는 3쌍(Z 아님).
(vev, h) 에 1차원 골짜기가 있고, 골짜기를 따라 훈련과 보류가 반대로 움직인다
(vev 0.34: 훈련 3.04 / 보류 2.60, vev 0.44: 3.04 / 5.03). Z 의 보류 이득은 이 맞바꿈에서 온다.
"""
import itertools
import numpy as np
import onefield_carry as ref
from higgs_engine import Air, Field, carry_yd

air = Air()
rm = lambda e: float(np.sqrt(np.mean(np.square(e))))

def score(vev, ratio):
    f = Field(vev=vev, h=vev * ratio)
    tr = rm(carry_yd(f, air, *ref.PGA[:, :3].T) - ref.PGA[:, 3])
    ho = rm(carry_yd(f, air, *ref.LPGA[:, :3].T) - ref.LPGA[:, 3])
    return tr, ho

print("기준: B(4매개) 0.71/3.16, F3 적합(2매개) 0.72/3.67")
tr, ho = score(np.pi / 8, 1 / np.sqrt(2))
print(f"Z  (0매개) vev=pi/8, h/vev=1/sqrt2 : 훈련 {tr:.2f} / 보류 {ho:.2f} yd")

vevs = {"1/e": 1/np.e, "3/8": 3/8, "1/phi^2": 1/((1+5**.5)/2)**2, "pi/8": np.pi/8,
        "2/5": 0.4, "1/sqrt(2pi)": 1/np.sqrt(2*np.pi), "ln(3/2)": np.log(1.5)}
ratios = {"2/3": 2/3, "ln2": np.log(2), "1/sqrt2": 1/np.sqrt(2), "e/4": np.e/4, "3/4": 0.75, "1/phi": 2/(1+5**.5)}
rows = []
for (a, va), (b, rb) in itertools.product(vevs.items(), ratios.items()):
    tr, ho = score(va, rb)
    rows.append((tr, ho, a, b, va, rb))
rows.sort()
print(f"\n단순 상수 후보 {len(rows)}쌍, 훈련 RMSE 순 상위 10")
for tr, ho, a, b, va, rb in rows[:10]:
    print(f"  vev={a:<11}({va:.4f}) h/vev={b:<8}({rb:.4f})  훈련 {tr:5.2f}  보류 {ho:5.2f}")
n_good = sum(r[0] < 1.0 for r in rows)
print(f"훈련 RMSE < 1.0 yd 인 쌍: {n_good}/{len(rows)}")

print("\n골짜기 모양: vev 를 고정하고 h 만 맞출 때")
from scipy.optimize import minimize_scalar
for vev in (0.34, 0.36, 0.38, 0.393, 0.40, 0.42, 0.44):
    r = minimize_scalar(lambda h: score(vev, h / vev)[0], bounds=(0.05, 0.6), method="bounded")
    tr, ho = score(vev, r.x / vev)
    print(f"  vev={vev:.3f}  최적 h={r.x:.3f} (h/vev={r.x/vev:.3f})  훈련 {tr:.2f}  보류 {ho:.2f}")
