"""장 하나를 펼쳐 두고 읽는 골프공 비행 엔진.

1) 무차원화. 길이 L = 2m/(rho A), 속도 U = sqrt(gL), 시간 U/g 로 재면 운동방정식은

       du/dt = -|u| conj(Gamma(S)) u - i,      S = w/|u|
       dw/dt = -beta w |u|,                    beta = 0.024 m R^2 / I  (= 0.06, I = 0.4 m R^2)

   가 되어 공기밀도·공 질량·크기·중력이 모두 사라진다. 남는 것은 장 Gamma(S) 하나와
   발사 상태 (v0', theta, S0) 뿐이다. (w = R omega / U, 스핀 감쇠는 C_M = 0.012 S 가정)

2) 힉스꼴 장. Gamma(S) = (vev + h S) e^{i psi(S)},  psi(S) = psi_inf (1 - e^{-S/s0}).
   vev 는 스핀과 무관한 배경 값, h S 는 스핀이 일으키는 들뜸이다.

3) 펼치기. 무차원 발사 공간 (v0', theta, S0) 전체를 한 번 적분해 표로 둔다.
   샷 하나는 표를 읽는 보간 한 번이다. 무차원이므로 고도·기온·공 규격이 바뀌어도
   같은 표를 L, U 만 바꿔 읽는다.

python -B higgs_engine.py
"""

from __future__ import annotations

import time
from dataclasses import dataclass

import numpy as np
from scipy.interpolate import RegularGridInterpolator
from scipy.optimize import least_squares

MPH = 0.44704
YD = 0.9144
G = 9.81
BALL_M = 0.04593
BALL_R = 0.04267 / 2
I_FACTOR = 0.4


@dataclass(frozen=True)
class Field:
    """힉스꼴 장: Gamma(S) = (vev + h S) e^{i psi_inf (1 - e^{-S/s0})}."""
    vev: float
    h: float
    psi_inf: float = np.pi / 4
    s0: float = 1e-3

    def gamma(self, S):
        psi = self.psi_inf * (1 - np.exp(-S / self.s0))
        return (self.vev + self.h * S) * np.exp(1j * psi)


@dataclass(frozen=True)
class Air:
    rho: float = 1.2
    m: float = BALL_M
    r: float = BALL_R
    g: float = G

    @property
    def L(self):
        return 2 * self.m / (self.rho * np.pi * self.r**2)

    @property
    def U(self):
        return np.sqrt(self.g * self.L)


BETA = 0.024 / I_FACTOR  # = 0.06, 공기밀도와 무관


def fly(field: Field, v0, theta, S0, dt=0.002, t_max=6.0):
    """무차원 배치 RK4. 모든 입력은 같은 길이의 배열. 착지(y=0) 상태를 돌려준다."""
    v0 = np.asarray(v0, float)
    theta = np.asarray(theta, float)
    S0 = np.asarray(S0, float)
    n = v0.size
    z = np.zeros(n, complex)                  # 위치 x + i y
    u = v0 * np.exp(1j * theta)               # 속도
    w = S0 * v0                               # 표면 회전 속도
    apex = np.zeros(n)
    out = {k: np.full(n, np.nan) for k in ("carry", "apex", "t", "vx", "vy", "w")}
    alive = np.ones(n, bool)

    def deriv(u, w):
        sp = np.abs(u)
        S = w / sp
        du = -sp * np.conj(field.gamma(S)) * u - 1j
        dw = -BETA * w * sp
        return du, dw

    t = 0.0
    while alive.any() and t < t_max:
        idx = np.flatnonzero(alive)
        z0, u0, w0 = z[idx], u[idx], w[idx]
        k1u, k1w = deriv(u0, w0)
        k2u, k2w = deriv(u0 + 0.5 * dt * k1u, w0 + 0.5 * dt * k1w)
        k3u, k3w = deriv(u0 + 0.5 * dt * k2u, w0 + 0.5 * dt * k2w)
        k4u, k4w = deriv(u0 + dt * k3u, w0 + dt * k3w)
        z1 = z0 + dt / 6 * (u0 + 2 * (u0 + 0.5 * dt * k1u) + 2 * (u0 + 0.5 * dt * k2u) + (u0 + dt * k3u))
        u1 = u0 + dt / 6 * (k1u + 2 * k2u + 2 * k3u + k4u)
        w1 = w0 + dt / 6 * (k1w + 2 * k2w + 2 * k3w + k4w)
        t += dt
        apex[idx] = np.maximum(apex[idx], z1.imag)

        land = (z1.imag < 0) & (t > dt)
        if land.any():
            j = idx[land]
            # 3차 Hermite로 y(s)=0 인 s in [0,1] 을 뉴턴으로 찾는다
            y0, y1 = z0[land].imag, z1[land].imag
            m0, m1 = u0[land].imag * dt, u1[land].imag * dt
            s = y0 / (y0 - y1)
            for _ in range(4):
                h00 = 2 * s**3 - 3 * s**2 + 1
                h10 = s**3 - 2 * s**2 + s
                h01 = -2 * s**3 + 3 * s**2
                h11 = s**3 - s**2
                f = h00 * y0 + h10 * m0 + h01 * y1 + h11 * m1
                df = (6 * s**2 - 6 * s) * y0 + (3 * s**2 - 4 * s + 1) * m0 \
                    + (-6 * s**2 + 6 * s) * y1 + (3 * s**2 - 2 * s) * m1
                s = np.clip(s - f / df, 0, 1)
            x0, x1 = z0[land].real, z1[land].real
            mx0, mx1 = u0[land].real * dt, u1[land].real * dt
            h00 = 2 * s**3 - 3 * s**2 + 1
            h10 = s**3 - 2 * s**2 + s
            h01 = -2 * s**3 + 3 * s**2
            h11 = s**3 - s**2
            out["carry"][j] = h00 * x0 + h10 * mx0 + h01 * x1 + h11 * mx1
            ul = u0[land] + s * (u1[land] - u0[land])
            out["vx"][j], out["vy"][j] = ul.real, ul.imag
            out["w"][j] = w0[land] + s * (w1[land] - w0[land])
            out["t"][j] = t - dt + s * dt
            out["apex"][j] = apex[j]
            alive[j] = False
        keep = ~land
        z[idx[keep]], u[idx[keep]], w[idx[keep]] = z1[keep], u1[keep], w1[keep]
    return out


def to_dimless(air: Air, ball_mph, launch_deg, spin_rpm):
    v = np.asarray(ball_mph, float) * MPH
    om = np.asarray(spin_rpm, float) * 2 * np.pi / 60
    return v / air.U, np.radians(launch_deg), air.r * om / v


def carry_yd(field: Field, air: Air, ball_mph, launch_deg, spin_rpm, **kw):
    v0, th, S0 = to_dimless(air, ball_mph, launch_deg, spin_rpm)
    return fly(field, v0, th, S0, **kw)["carry"] * air.L / YD


# ------------------------------------------------------------------ 펼친 표

AXES = (
    np.linspace(0.8, 3.9, 32),            # v0' (약 41–196 mph @ rho=1.2)
    np.radians(np.linspace(0, 50, 51)),   # theta
    np.linspace(0.02, 0.62, 31),          # S0
)
KEYS = ("carry", "apex", "t", "vx", "vy", "w")


def unfold(field: Field, axes=AXES, **kw):
    V, T, S = np.meshgrid(*axes, indexing="ij")
    res = fly(field, V.ravel(), T.ravel(), S.ravel(), **kw)
    shape = V.shape
    return {k: res[k].reshape(shape) for k in KEYS}


class UnfoldedField:
    """펼친 장. 샷 하나는 3차원 보간 한 번이다."""

    def __init__(self, field: Field, axes=AXES, table=None):
        self.field = field
        self.axes = axes
        self.table = table if table is not None else unfold(field, axes)
        self._interp = {k: RegularGridInterpolator(axes, self.table[k], method="cubic")
                        for k in KEYS}

    def read(self, v0, theta, S0, key="carry"):
        pts = np.column_stack([np.ravel(v0), np.ravel(theta), np.ravel(S0)])
        return self._interp[key](pts)

    def carry_yd(self, air: Air, ball_mph, launch_deg, spin_rpm):
        return self.read(*to_dimless(air, ball_mph, launch_deg, spin_rpm)) * air.L / YD

    def save(self, path):
        np.savez_compressed(path, **{f"axis{i}": a for i, a in enumerate(self.axes)},
                            **self.table, vev=self.field.vev, h=self.field.h,
                            psi_inf=self.field.psi_inf, s0=self.field.s0)
