"""슬로모 영상 → 공 위치 (미터) CSV. 옆에서 찍은 영상(카메라가 비행 방향과 직각) 기준.

순서
  1. 배경 = 전체 프레임의 중앙값 (날아가는 공과 사람은 지워지고, 서 있던 공도 대부분 지워진다)
  2. 프레임마다 배경과의 차이 → 덩어리 → 공 크기에 맞는 것만 후보
  3. 처음 위치(--ball u,v 또는 첫 프레임에서 자동)에서 앞쪽으로 움직인 첫 후보를 발사로 보고, 등속 예측으로 따라감
  4. 픽셀 → 미터: 공 지름 42.67 mm 와 화각(--hfov)으로 깊이 Z = f D / (2 r), X = (u - cx) Z / f
     공 반지름은 덩어리 2차 모멘트의 작은 고유값으로 (번진 공도 짧은 축은 지름 그대로)

출력 CSV: t,x,y,z  (x 앞, y 위, z 오른쪽, 출발점 0) — flightbench 의 spininv/screen --csv 입력.
옆 카메라 한 대로는 좌우(z)를 공 크기로만 알 수 있어 기본은 비워 둔다(nan). 사이드 스핀은 사전값에 기댄다.
축척은 --wall (벽까지 거리)이 가장 정확하다. 없으면 --scale (화면 속 기준 길이), 마지막으로 공 크기.

python -B tools/track_ball.py shot.mp4 --fps 240 --hfov 70 -o shot.csv [--ball 412,530] [--flip] [--debug dbg.mp4]
주의: 실제 폰 화각은 기종·배율마다 다르다(--hfov). 롤링 셔터, 렌즈 왜곡은 아직 보정하지 않는다.
"""

from __future__ import annotations

import argparse

import cv2
import numpy as np

D_BALL = 0.04267


def read_frames(path, max_frames=4000):
    cap = cv2.VideoCapture(path)
    frames = []
    while len(frames) < max_frames:
        ok, fr = cap.read()
        if not ok:
            break
        frames.append(fr)
    cap.release()
    if not frames:
        raise SystemExit(f"{path}: 프레임을 읽지 못했습니다")
    return frames


def blobs(gray, bg, thr, r_lo, r_hi):
    d = cv2.absdiff(gray, bg)
    _, m = cv2.threshold(d, thr, 255, cv2.THRESH_BINARY)
    m = cv2.morphologyEx(m, cv2.MORPH_OPEN, np.ones((3, 3), np.uint8))
    n, lab, stats, cents = cv2.connectedComponentsWithStats(m)
    out = []
    for i in range(1, n):
        area = stats[i, cv2.CC_STAT_AREA]
        if area < np.pi * r_lo**2 * 0.5 or area > np.pi * r_hi**2 * 12:
            continue
        ys, xs = np.nonzero(lab == i)
        w = d[ys, xs].astype(np.float64)
        cx, cy = np.average(xs, weights=w), np.average(ys, weights=w)
        cov = np.cov(np.vstack([xs, ys]), aweights=w)
        lmin = max(np.linalg.eigvalsh(cov)[0], 1e-6)
        r = 2.0 * np.sqrt(lmin)  # 원판의 표준편차 = r/2
        if r_lo <= r <= r_hi:
            out.append((cx, cy, r, area))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("video")
    ap.add_argument("-o", "--out", default="shot.csv")
    ap.add_argument("--fps", type=float, required=True, help="실제 촬영 fps (슬로모 240 등)")
    ap.add_argument("--hfov", type=float, default=70.0, help="가로 화각 (도)")
    ap.add_argument("--ball", default=None, help="처음 공 위치 u,v (픽셀). 없으면 첫 프레임에서 자동")
    ap.add_argument("--thr", type=float, default=25)
    ap.add_argument("--flip", action="store_true", help="공이 화면 왼쪽으로 날아갈 때")
    ap.add_argument("--debug", default=None, help="검출 표시 영상 저장")
    ap.add_argument("--wall", type=float, default=None, help="공 출발점~벽 면 거리 (m). 주면 충돌 위치로 축척을 잡는다 (가장 정확)")
    ap.add_argument("--scale", default=None, help="화면 속 기준 길이: u1,v1,u2,v2,미터 (예: 바닥 정렬 막대)")
    ap.add_argument("--depth", action="store_true", help="공 크기로 깊이(z)도 기록 (해상도가 높을 때만)")
    a = ap.parse_args()

    frames = read_frames(a.video)
    H, W = frames[0].shape[:2]
    f = W / 2 / np.tan(np.radians(a.hfov) / 2)
    grays = [cv2.cvtColor(fr, cv2.COLOR_BGR2GRAY) for fr in frames]
    step = max(1, len(grays) // 60)
    bg = np.median(np.stack(grays[::step]), axis=0).astype(np.uint8)

    # 처음 공: 첫 프레임과 배경의 차이에서 (공은 배경 중앙값에서 지워졌다고 본다)
    if a.ball:
        u0, v0 = map(float, a.ball.split(","))
        cand = blobs(grays[0], bg, a.thr, 2, 80)
        r0 = min(cand, key=lambda b: (b[0] - u0) ** 2 + (b[1] - v0) ** 2)[2] if cand else 10.0
    else:
        cand = blobs(grays[0], bg, a.thr, 2, 80)
        if not cand:
            raise SystemExit("첫 프레임에서 공을 못 찾았습니다. --ball u,v 로 알려 주세요")
        u0, v0, r0, _ = max(cand, key=lambda b: b[3])
    print(f"처음 공: ({u0:.1f}, {v0:.1f}) 반지름 {r0:.1f} px, 초점거리 {f:.0f} px")
    sgn = -1.0 if a.flip else 1.0

    track = []  # (frame, u, v, r)
    state = None  # (u, v, du, dv)
    for k, g in enumerate(grays):
        c = blobs(g, bg, a.thr, 0.5 * r0, 1.6 * r0)
        if state is None:
            moved = [b for b in c if sgn * (b[0] - u0) > 1.5 * r0 and abs(b[1] - v0) < 20 * r0]
            if moved:
                b = min(moved, key=lambda b: sgn * (b[0] - u0))
                track.append((k, b[0], b[1], b[2]))
                state = (b[0], b[1], b[0] - u0, b[1] - v0)
            continue
        pu, pv = state[0] + state[2], state[1] + state[3]
        sp = np.hypot(state[2], state[3])
        gate = max(4 * r0, 0.6 * sp)
        near = [b for b in c if np.hypot(b[0] - pu, b[1] - pv) < gate]
        if not near:
            # 벽 반동: 예측이 빗나가면 마지막 위치 둘레에서 다시 찾는다 (방향이 바뀌므로)
            near = [b for b in c if np.hypot(b[0] - state[0], b[1] - state[1]) < max(4 * r0, 1.2 * sp)]
        if not near:
            if k - track[-1][0] > 6:
                break  # 잃어버림
            continue
        b = min(near, key=lambda b: np.hypot(b[0] - pu, b[1] - pv))
        dk = k - track[-1][0]
        state = (b[0], b[1], (b[0] - track[-1][1]) / dk, (b[1] - track[-1][2]) / dk)
        track.append((k, b[0], b[1], b[2]))

    if len(track) < 5:
        raise SystemExit(f"공을 {len(track)}프레임만 따라갔습니다. --thr, --ball 을 바꿔 보세요")

    # 축척 (m/px): 벽 충돌 위치 > 기준 길이 > 공 크기 순으로 믿는다.
    # 공 크기(반지름 수 px)는 1 px 만 틀려도 축척이 10% 넘게 틀려서 마지막 수단이다.
    Z0 = f * D_BALL / (2 * r0)
    m_per_px = Z0 / f
    how = "공 크기"
    if a.scale:
        u1, v1, u2, v2, L = map(float, a.scale.split(","))
        m_per_px = L / np.hypot(u2 - u1, v2 - v1)
        how = "기준 길이"
    if a.wall:
        # 충돌 픽셀: 충돌 전·후 궤적을 각각 2차식으로 맞춰 만나는 점 (프레임 사이에 있음)
        ks = np.array([t[0] for t in track], float)
        us = np.array([sgn * (t[1] - u0) for t in track])
        im = int(np.argmax(us))
        pre, post = slice(max(0, im - 6), im + 1), slice(im + 1, min(len(us), im + 8))
        if im >= 2 and len(us[post]) >= 2:
            cpre = np.polyfit(ks[pre], us[pre], min(2, len(ks[pre]) - 1))
            cpost = np.polyfit(ks[post], us[post], min(2, len(ks[post]) - 1))
            roots = np.roots(np.polysub(cpre, cpost))
            roots = [r.real for r in roots if abs(r.imag) < 1e-9 and ks[im] - 1e-9 <= r.real <= ks[im] + 1.0 + 1e-9]
            u_hit = np.polyval(cpre, roots[0]) if roots else us[im]
        else:
            u_hit = us[im]
        m_per_px = (a.wall - D_BALL / 2) / u_hit
        how = "벽 충돌 위치"
    print(f"축척 {m_per_px * 1000:.3f} mm/px ({how})")
    Zp = m_per_px * f

    def to3d(u, v, r):
        return (u - W / 2) * m_per_px, -(v - H / 2) * m_per_px, (f * D_BALL / (2 * r) if a.depth else np.nan)

    X0, Y0, _ = to3d(u0, v0, r0)
    Z0 = Zp
    k0 = track[0][0]
    # 카메라: 공이 출발 평면에서 벗어나면(z) 원근 때문에 x, y 가 달라진다. 역보정이 이걸 투영으로 되돌리도록 남긴다.
    xc = sgn * (W / 2 - u0) * m_per_px
    yc = (v0 - H / 2) * m_per_px
    with open(a.out, "w") as fo:
        fo.write(f"# cam {xc:.5f} {yc:.5f} {Zp:.5f} {sgn:.0f}\n")
        fo.write("t,x,y,z\n")
        for k, u, v, r in track:
            X, Y, Z = to3d(u, v, r)
            z = sgn * (Z - Z0) if np.isfinite(Z) else float("nan")
            fo.write(f"{(k - k0) / a.fps:.6f},{sgn * (X - X0):.5f},{Y - Y0:.5f},{z:.5f}\n")
    print(f"{a.out}: {len(track)}프레임 ({track[0][0]}–{track[-1][0]}), 빠진 프레임 {track[-1][0] - track[0][0] + 1 - len(track)}")

    if a.debug:
        vw = cv2.VideoWriter(a.debug, cv2.VideoWriter_fourcc(*"mp4v"), 30, (W, H))
        tk = {k: (u, v, r) for k, u, v, r in track}
        for k, fr in enumerate(frames):
            im = fr.copy()
            if k in tk:
                u, v, r = tk[k]
                cv2.circle(im, (int(u), int(v)), int(r * 2), (0, 140, 255), 2)
            vw.write(im)
        vw.release()


if __name__ == "__main__":
    main()
