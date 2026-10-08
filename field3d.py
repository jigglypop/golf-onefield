"""3차원 장: 사이드 스핀(스핀축 기울기)과 대칭 축소.

좌표: x 비행 방향, y 위, z 오른쪽. 무차원 단위는 higgs_engine 과 같다.
장 하나 (각도 잠금):  k = (vev |u| + h w) / sqrt2
    du/dt = -k u + k (omega_hat x u) - y_hat,    dw/dt = -0.06 w |u|
omega_hat = (0, sin a, cos a) 로 공간에 고정(세차 없음). a = 0 이면 순수 백스핀이고
2차원 식 du/dt = -|u| conj(Gamma) u - i 와 정확히 같다.
a > 0 이면 양력이 -z(왼쪽)로 기운다.

대칭으로 펼칠 축 줄이기
  - 발사 방위각: 중력과 장이 수직축 회전에 불변이므로 결과를 돌리기만 하면 된다 (축 불필요)
  - a -> -a : 비행 방향 거리는 같고 좌우 거리만 부호가 바뀐다 (a >= 0 만 펼침)
남는 축: (v0', theta, S0, a) 4개.

결과 (2026-10-08, 무작위 2000샷)
  a = 0 에서 2차원 엔진과 캐리 차이 1.6e-13 yd
  a -> -a 대칭, 방위각 회전 대칭 모두 1e-12 yd 이하로 성립
  PGA 7번 아이언, a = 10° 에서 좌우 -16.7 yd, 캐리 -1.6 yd (실측 대조 없음, 감 잡기용)
  4차원 장 809,472점 펼침 51 s, f32 6.5 MB, 4선형 읽기 오차 최대 x 0.19 / z 0.16 yd
  다음: 3차 읽기(앞 세 축) + 선형(a) 로 0.01 yd 대를 노린다

python -B field3d.py
"""

from __future__ import annotations

import time

import numpy as np
from scipy.interpolate import RegularGridInterpolator

from higgs_engine import Air, Field, fly as fly2d

BETA = 0.06
R2 = 1 / np.sqrt(2)
YD = Air().L / 0.9144


def fly3d(field: Field, v0, theta, S0, alpha, azim=0.0, dt=0.004, t_max=6.0):
    v0, theta, S0, alpha = (np.asarray(a, float) for a in (v0, theta, S0, alpha))
    azim = np.broadcast_to(np.asarray(azim, float), v0.shape)
    n = v0.size
    ca, sa = np.cos(azim), np.sin(azim)

    def rot(vx, vy, vz):  # 수직축(y) 둘레로 azim 만큼 회전
        return ca * vx + sa * vz, vy, -sa * vx + ca * vz

    p = np.zeros((3, n))
    u = np.array(rot(v0 * np.cos(theta), v0 * np.sin(theta), np.zeros(n)))
    om = np.array(rot(np.zeros(n), np.sin(alpha), np.cos(alpha)))
    w = S0 * v0
    out = {k: np.full(n, np.nan) for k in ("x", "z", "apex", "t", "w")}
    apex = np.zeros(n)
    alive = np.ones(n, bool)
    vev, h = field.vev, field.h

    def deriv(u, w, om):
        sp = np.sqrt((u * u).sum(0))
        k = (vev * sp + h * w) * R2
        cr = np.cross(om, u, axis=0)
        a = -k * u + k * cr
        a[1] -= 1.0
        return a, -BETA * w * sp

    t = 0.0
    while alive.any() and t < t_max:
        idx = np.flatnonzero(alive)
        p0, u0, w0, o = p[:, idx], u[:, idx], w[idx], om[:, idx]
        a1, d1 = deriv(u0, w0, o)
        a2, d2 = deriv(u0 + 0.5 * dt * a1, w0 + 0.5 * dt * d1, o)
        a3, d3 = deriv(u0 + 0.5 * dt * a2, w0 + 0.5 * dt * d2, o)
        a4, d4 = deriv(u0 + dt * a3, w0 + dt * d3, o)
        p1 = p0 + dt / 6 * (u0 + 2 * (u0 + 0.5 * dt * a1) + 2 * (u0 + 0.5 * dt * a2) + (u0 + dt * a3))
        u1 = u0 + dt / 6 * (a1 + 2 * a2 + 2 * a3 + a4)
        w1 = w0 + dt / 6 * (d1 + 2 * d2 + 2 * d3 + d4)
        t += dt
        apex[idx] = np.maximum(apex[idx], p1[1])
        land = (p1[1] < 0) & (t > dt)
        if land.any():
            j = idx[land]
            y0, y1 = p0[1, land], p1[1, land]
            m0, m1 = u0[1, land] * dt, u1[1, land] * dt
            s = y0 / (y0 - y1)
            for _ in range(4):
                f = (2*s**3 - 3*s**2 + 1) * y0 + (s**3 - 2*s**2 + s) * m0 + (-2*s**3 + 3*s**2) * y1 + (s**3 - s**2) * m1
                df = (6*s**2 - 6*s) * y0 + (3*s**2 - 4*s + 1) * m0 + (-6*s**2 + 6*s) * y1 + (3*s**2 - 2*s) * m1
                s = np.clip(s - f / df, 0, 1)
            H = (2*s**3 - 3*s**2 + 1, s**3 - 2*s**2 + s, -2*s**3 + 3*s**2, s**3 - s**2)
            for c, key in ((0, "x"), (2, "z")):
                out[key][j] = H[0] * p0[c, land] + H[1] * u0[c, land] * dt + H[2] * p1[c, land] + H[3] * u1[c, land] * dt
            out["apex"][j] = apex[j]
            out["t"][j] = t - dt + s * dt
            out["w"][j] = w0[land] + s * (w1[land] - w0[land])
            alive[j] = False
        keep = ~land
        p[:, idx[keep]], u[:, idx[keep]], w[idx[keep]] = p1[:, keep], u1[:, keep], w1[keep]
    return out


AXES4 = (
    np.linspace(0.8, 3.9, 32),
    np.radians(np.linspace(0, 50, 51)),
    np.linspace(0.02, 0.62, 31),
    np.radians(np.linspace(0, 45, 16)),
)


class Field4:
    """펼친 4차원 장. 방위각과 a 의 부호는 대칭으로 처리한다."""

    def __init__(self, field: Field, axes=AXES4, dt=0.032):
        self.axes = axes
        G = np.meshgrid(*axes, indexing="ij")
        res = fly3d(field, *(g.ravel() for g in G), dt=dt)
        shape = G[0].shape
        self.x = RegularGridInterpolator(axes, res["x"].reshape(shape))
        self.z = RegularGridInterpolator(axes, res["z"].reshape(shape))
        self.npts = res["x"].size

    def read(self, v0, theta, S0, alpha, azim=0.0):
        sgn = np.sign(alpha) + (alpha == 0)
        pts = np.column_stack([v0, theta, S0, np.abs(alpha)])
        x, z = self.x(pts), self.z(pts) * sgn
        ca, sa = np.cos(azim), np.sin(azim)
        return ca * x + sa * z, -sa * x + ca * z


def main():
    f = Field(vev=0.3881, h=0.2714)
    air = Air()
    rng = np.random.default_rng(5)
    n = 2000
    v0 = rng.uniform(1.56, 3.61, n)
    th = np.radians(rng.uniform(5, 40, n))
    S0 = rng.uniform(0.05, 0.55, n)

    print("== 1. a = 0 에서 2차원 엔진과 일치")
    c2 = fly2d(f, v0, th, S0, dt=0.004)["carry"]
    r3 = fly3d(f, v0, th, S0, np.zeros(n), dt=0.004)
    print(f"캐리 최대 차이 {np.max(np.abs(r3['x'] - c2)) * YD:.2e} yd, 좌우 최대 {np.max(np.abs(r3['z'])) * YD:.2e} yd")

    print("\n== 2. 대칭 검사 (직접 적분)")
    al = np.radians(rng.uniform(0, 40, n))
    rp = fly3d(f, v0, th, S0, al, dt=0.004)
    rm = fly3d(f, v0, th, S0, -al, dt=0.004)
    print(f"a -> -a : x 차이 {np.max(np.abs(rp['x'] - rm['x'])) * YD:.2e} yd, z 합 {np.max(np.abs(rp['z'] + rm['z'])) * YD:.2e} yd")
    az = rng.uniform(-0.5, 0.5, n)
    ra = fly3d(f, v0, th, S0, al, azim=az, dt=0.004)
    xr = np.cos(az) * rp["x"] + np.sin(az) * rp["z"]
    zr = -np.sin(az) * rp["x"] + np.cos(az) * rp["z"]
    print(f"방위각 회전 : 회전시켜 적분 vs 결과만 회전, 최대 차이 {np.max(np.hypot(ra['x'] - xr, ra['z'] - zr)) * YD:.2e} yd")

    print("\n== 3. 감 잡기: PGA 7번 아이언 (120 mph, 16.3°, 7097 rpm), 스핀축 기울기별")
    from higgs_engine import to_dimless
    v, t, s = to_dimless(air, 120, 16.3, 7097)
    for deg in (0, 5, 10, 20, 30):
        r = fly3d(f, [v], [t], [s], [np.radians(deg)], dt=0.004)
        print(f"  a={deg:>2}°  캐리 {r['x'][0] * YD:6.1f} yd   좌우 {r['z'][0] * YD:+6.1f} yd")

    print("\n== 4. 4차원 장 펼치기와 읽기 (3·4선형)")
    t0 = time.perf_counter()
    F4 = Field4(f)
    print(f"{F4.npts:,}점 펼침 {time.perf_counter() - t0:.1f} s, f32 로 {F4.npts * 2 * 4 / 1e6:.1f} MB (x, z)")
    al_s = np.radians(rng.uniform(-40, 40, n))
    az_s = rng.uniform(-0.3, 0.3, n)
    truth = fly3d(f, v0, th, S0, al_s, azim=az_s, dt=0.004)
    t0 = time.perf_counter()
    xr, zr = F4.read(v0, th, S0, al_s, az_s)
    t_read = time.perf_counter() - t0
    ex, ez = (xr - truth["x"]) * YD, (zr - truth["z"]) * YD
    print(f"x 오차 RMSE {np.sqrt(np.mean(ex**2)):.3f} / 최대 {np.max(np.abs(ex)):.3f} yd,  "
          f"z 오차 RMSE {np.sqrt(np.mean(ez**2)):.3f} / 최대 {np.max(np.abs(ez)):.3f} yd")
    print(f"파이썬 읽기 {t_read / n * 1e6:.2f} µs/샷 (scipy, 참고용)")


if __name__ == "__main__":
    main()
