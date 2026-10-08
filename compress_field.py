"""장 압축 실험: 무엇을 펼쳐 저장하면 성긴 격자로도 정확한가.

같은 3선형 읽기에서 저장량만 바꿔 오차를 비교한다.
  raw     : carry 그대로
  log     : log(carry)                         (곱셈 구조를 덧셈으로)
  vac     : carry / X_vac                      X_vac = 진공(무중력 장 하나) 해석해로 만든 배경
진공 배경: 중력을 빼면 dv/ds = -(vev v + h w)/sqrt2 (s = 경로 길이), w = w0 e^{-beta s} 로
닫힌 꼴이 된다. 여기서는 그 가장 단순한 판본 X_vac = ln(1 + vev v0^2 sin2θ / sqrt2) / (vev/sqrt2) 를 쓴다.

결과 (2026-10-08, 3선형 최대 오차 yd): 32×51×31 에서 raw 0.174 / log 0.374 / vac 1.595.
모든 격자에서 raw 가 가장 좋다. log 와 진공 배경 비율은 [반례]: 굴곡을 줄이지 못하고 키운다.
"""
import numpy as np
from scipy.interpolate import RegularGridInterpolator
from higgs_engine import Field, fly

f = Field(vev=0.3881, h=0.2714)
VEV = f.vev
rng = np.random.default_rng(3)
n = 3000
pv, pt, ps = rng.uniform(1.56, 3.61, n), np.radians(rng.uniform(5, 40, n)), rng.uniform(0.05, 0.55, n)
truth = fly(f, pv, pt, ps, dt=0.004)["carry"]
YD = 53.53157581531039 / 0.9144

def vac(v, t):
    return np.log1p(VEV * v**2 * np.sin(2 * t) / np.sqrt(2)) / (VEV / np.sqrt(2))

def grid(nv, nt, ns):
    ax = (np.linspace(0.8, 3.9, nv), np.radians(np.linspace(0, 50, nt)), np.linspace(0.02, 0.62, ns))
    V, T, S = np.meshgrid(*ax, indexing="ij")
    c = fly(f, V.ravel(), T.ravel(), S.ravel(), dt=0.032)["carry"].reshape(V.shape)
    return ax, V, T, S, c

pts = np.column_stack([pv, pt, ps])
print(f"{'격자':>12} {'KB(f32)':>8} | {'raw 최대':>9} {'log 최대':>9} {'vac 최대':>9}  (yd)")
for g in [(16, 26, 16), (24, 38, 24), (32, 51, 31), (64, 101, 61)]:
    ax, V, T, S, c = grid(*g)
    res = []
    # raw
    res.append(RegularGridInterpolator(ax, c)(pts))
    # log (θ=0 행은 캐리가 작아 log 가 커지므로 하한)
    res.append(np.exp(RegularGridInterpolator(ax, np.log(np.maximum(c, 1e-6)))(pts)))
    # vac 배경 비율 (θ=0 에서 배경 0 → 극한값 대신 다음 열로 채움)
    base = vac(V, T)
    ratio = np.where(base > 1e-9, c / np.maximum(base, 1e-9), np.nan)
    ratio[:, 0, :] = ratio[:, 1, :]
    res.append(RegularGridInterpolator(ax, ratio)(pts) * vac(pv, pt))
    mx = [np.max(np.abs(r - truth)) * YD for r in res]
    kb = np.prod(g) * 4 / 1024
    print(f"{g[0]:>3}×{g[1]:>3}×{g[2]:>3} {kb:8.0f} | {mx[0]:9.4f} {mx[1]:9.4f} {mx[2]:9.4f}")
