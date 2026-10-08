"""higgs_engine 검산: 정확도·속도·재피팅·각도 프로파일·보편성.

결과 (2026-10-08)
  1. 배치 RK4(dt'=0.004)와 solve_ivp(rtol 1e-8)의 캐리 차이 < 0.0001 yd
  2. 재피팅 vev=0.3881, h=0.2714 (이전과 같음), 훈련 0.72 / 보류 3.67 yd, 적합 2.0 s (이전 약 90 s)
  3. psi 프로파일: 훈련 최소 44°(0.705), 40–46°에서 0.74 이하로 평평. pi/4는 그 안에 있다
  4. 50,592점 격자 펼침 23 s, 2.3 MB(npz)
  5. 표 읽기 오차 RMSE 0.0005 yd, 최대 0.004 yd. 샷당 0.64 µs (solve_ivp 약 15,000 µs)
  6. rho = 1.2, 1.0, 0.95 에서 같은 표 읽기와 독립 적분 차이 0.001 yd
  허점: s0 가 식별되지 않아 S -> 0 극한의 항력 C_D(0) = vev = 0.388 은 자료가 정하지 않은 값이다.

python -B run_higgs.py
"""

from __future__ import annotations

import time

import numpy as np
from scipy.optimize import least_squares

import onefield_carry as ref
from higgs_engine import Air, Field, UnfoldedField, carry_yd, fly, to_dimless, AXES

AIR = Air()
F3_OLD = Field(vev=0.3881, h=0.2714, s0=0.00129)


def rmse(x):
    return float(np.sqrt(np.mean(np.square(x))))


def section(title):
    print(f"\n== {title}")


def main():
    data = np.vstack([ref.PGA, ref.LPGA])

    section("1. 배치 RK4 vs solve_ivp (같은 장, F3 이전 적합값)")
    t0 = time.perf_counter()
    c_ref = np.array([ref.carry_yd(ref.gamma_F3, (0.3881, 0.2714, 0.00129), *r[:3]) for r in data])
    t_ref = time.perf_counter() - t0
    for dt in (0.004, 0.002):
        t0 = time.perf_counter()
        c_new = carry_yd(F3_OLD, AIR, *data[:, :3].T, dt=dt)
        t_new = time.perf_counter() - t0
        print(f"dt'={dt}: 최대 |차이| {np.max(np.abs(c_new - c_ref)):.4f} yd, "
              f"13샷 {t_new*1e3:.1f} ms (solve_ivp {t_ref*1e3:.0f} ms)")

    section("2. 빠른 엔진으로 F3 재피팅 (psi = pi/4 고정, 훈련 PGA만)")
    def res(p, psi=np.pi / 4):
        f = Field(vev=p[0], h=p[1], psi_inf=psi)
        return carry_yd(f, AIR, *ref.PGA[:, :3].T) - ref.PGA[:, 3]
    t0 = time.perf_counter()
    fit = least_squares(res, [0.39, 0.27], bounds=([0.05, -2], [1.0, 3]))
    t_fit = time.perf_counter() - t0
    field = Field(vev=fit.x[0], h=fit.x[1])
    tr = carry_yd(field, AIR, *ref.PGA[:, :3].T) - ref.PGA[:, 3]
    ho = carry_yd(field, AIR, *ref.LPGA[:, :3].T) - ref.LPGA[:, 3]
    print(f"vev={field.vev:.4f}, h={field.h:.4f}  훈련 {rmse(tr):.2f} yd / 보류 {rmse(ho):.2f} yd  "
          f"(적합 {t_fit:.1f} s)")

    section("3. 각도 프로파일: psi 를 고정하고 (vev, h) 만 재적합")
    print(" psi(deg)  훈련RMSE  보류RMSE   vev     h")
    prof = []
    for deg in range(34, 57, 2):
        psi = np.radians(deg)
        f = least_squares(lambda p: res(p, psi), [0.39, 0.27], bounds=([0.05, -2], [1.5, 3]))
        fl = Field(vev=f.x[0], h=f.x[1], psi_inf=psi)
        a = rmse(carry_yd(fl, AIR, *ref.PGA[:, :3].T) - ref.PGA[:, 3])
        b = rmse(carry_yd(fl, AIR, *ref.LPGA[:, :3].T) - ref.LPGA[:, 3])
        prof.append((deg, a, b))
        print(f"  {deg:5d}   {a:7.3f}   {b:7.3f}   {f.x[0]:.3f}  {f.x[1]:+.3f}")
    prof = np.array(prof)
    print(f"훈련 최소 psi = {prof[np.argmin(prof[:,1]),0]:.0f}°, "
          f"보류 최소 psi = {prof[np.argmin(prof[:,2]),0]:.0f}° (보류 최소는 참고용, 선택에 쓰지 않음)")

    section("4. 장 펼치기 (v0', theta, S0) 격자")
    n_grid = np.prod([a.size for a in AXES])
    t0 = time.perf_counter()
    uf = UnfoldedField(field)
    t_unfold = time.perf_counter() - t0
    print(f"격자 {n_grid:,}점 펼침 {t_unfold:.1f} s, 결측 {int(np.isnan(uf.table['carry']).sum())}")
    uf.save("unfolded_field.npz")

    section("5. 읽기 정확도: 격자 밖 무작위 샷 3000개, 직접 적분과 비교")
    rng = np.random.default_rng(1)
    n = 3000
    mph = rng.uniform(80, 185, n)
    deg = rng.uniform(5, 40, n)
    v0, th, _ = to_dimless(AIR, mph, deg, np.ones(n))
    S0 = rng.uniform(0.05, 0.55, n)
    rpm = S0 * v0 * AIR.U / AIR.r * 60 / (2 * np.pi)
    t0 = time.perf_counter()
    direct = carry_yd(field, AIR, mph, deg, rpm, dt=0.001)
    t_direct = time.perf_counter() - t0
    t0 = time.perf_counter()
    read = uf.carry_yd(AIR, mph, deg, rpm)
    t_read = time.perf_counter() - t0
    err = read - direct
    print(f"캐리 오차: RMSE {rmse(err):.4f} yd, 최대 {np.max(np.abs(err)):.4f} yd")
    print(f"샷당 시간: 직접 적분 {t_direct/n*1e6:.1f} µs, 표 읽기 {t_read/n*1e6:.2f} µs, "
          f"solve_ivp {t_ref/13*1e6:.0f} µs")

    section("6. 보편성: 공기밀도를 바꿔도 같은 표를 읽는다 (독립 적분기 solve_ivp)")
    rows = ref.PGA[[0, 4, 6]]
    for rho in (1.2, 1.0, 0.95):   # 해수면, 약 1700 m, 약 2200 m 근사
        air = Air(rho=rho)
        old = ref.RHO
        ref.RHO = rho
        ref.K = rho * ref.A / (2 * ref.M)
        g = (field.vev, field.h, 1e-3)
        direct = np.array([ref.carry_yd(lambda S, p: (p[0] + p[1] * S) * np.exp(1j * np.pi / 4 * (1 - np.exp(-S / p[2]))),
                                        g, *r[:3]) for r in rows])
        ref.RHO, ref.K = old, old * ref.A / (2 * ref.M)
        read = uf.carry_yd(air, *rows[:, :3].T)
        print(f"rho={rho}: 표 읽기 {np.round(read,1)} / 직접 {np.round(direct,1)}  "
              f"최대 차 {np.max(np.abs(read-direct)):.3f} yd")


if __name__ == "__main__":
    main()
