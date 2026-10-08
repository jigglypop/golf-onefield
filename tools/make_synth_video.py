"""참 프레임(CSV) → 옆 카메라로 찍은 가짜 슬로모 영상. 공 검출·역보정 파이프라인 검산용.

카메라: 바늘구멍, 수평 화각 --hfov, 해상도 --size, 위치는 공 출발점 기준 (cx, cy, cz) m 에서 +z(공 쪽)를 봄.
좌표: x 앞(화면 오른쪽), y 위, z 카메라에서 멀어지는 쪽. 셔터 --shutter 초 동안의 번짐, 픽셀 노이즈, 벽(오른쪽 회색 띠).

python -B tools/make_synth_video.py truth.csv out.mp4 --fps 240
"""

from __future__ import annotations

import argparse
import csv

import cv2
import numpy as np

D_BALL = 0.04267


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("truth")
    ap.add_argument("out")
    ap.add_argument("--fps", type=float, default=240)
    ap.add_argument("--size", default="1280x720")
    ap.add_argument("--hfov", type=float, default=70.0)
    ap.add_argument("--cam", default="1.5,0.4,-3.0", help="카메라 위치 x,y,z (m)")
    ap.add_argument("--shutter", type=float, default=1 / 2000)
    ap.add_argument("--wall", type=float, default=3.0)
    ap.add_argument("--noise", type=float, default=4.0)
    ap.add_argument("--pre", type=int, default=20, help="공이 서 있는 앞 프레임 수")
    a = ap.parse_args()
    W, H = map(int, a.size.split("x"))
    f = W / 2 / np.tan(np.radians(a.hfov) / 2)
    cam = np.array([float(v) for v in a.cam.split(",")])
    rows = [list(map(float, r)) for r in csv.reader(open(a.truth)) if r and r[0][0].isdigit()]
    tr = np.array(rows)
    t, P = tr[:, 0], tr[:, 1:4]
    rng = np.random.default_rng(1)

    def proj(p):
        q = p - cam
        z = q[2]
        return W / 2 + f * q[0] / z, H / 2 - f * q[1] / z, f * D_BALL / 2 / z

    bg = np.full((H, W, 3), 70, np.uint8)
    # 바닥선과 벽
    _, vg, _ = proj(np.array([0.0, -D_BALL / 2, 0.0]))
    cv2.rectangle(bg, (0, int(vg)), (W, H), (60, 95, 60), -1)
    uw, _, _ = proj(np.array([a.wall, 0, 0]))
    cv2.rectangle(bg, (int(uw), 0), (W, H), (110, 110, 110), -1)
    vw = cv2.VideoWriter(a.out, cv2.VideoWriter_fourcc(*"mp4v"), 30, (W, H))
    fdt = t[1] - t[0]
    n_sub = 8
    for k in range(-a.pre, len(t)):
        img = bg.copy().astype(np.float32)
        layer = np.zeros((H, W), np.float32)
        for j in range(n_sub):
            # 셔터 동안의 위치 (선형 보간)
            tt = k * fdt + (j / (n_sub - 1) - 0.5) * a.shutter
            if tt <= 0:
                p = P[0]
            else:
                i = min(int(tt / fdt), len(t) - 2)
                w = tt / fdt - i
                p = (1 - w) * P[i] + w * P[i + 1]
            u, v, r = proj(p + np.array([0, D_BALL / 2, 0]))  # 공 중심은 지름/2 위
            sub = np.zeros((H, W), np.float32)
            cv2.circle(sub, (int(round(u * 4)), int(round(v * 4))), int(round(r * 4)), 1.0, -1, lineType=cv2.LINE_AA, shift=2)
            layer += sub / n_sub
        img = img * (1 - layer[..., None]) + 245 * layer[..., None]
        img += rng.normal(0, a.noise, img.shape)
        vw.write(np.clip(img, 0, 255).astype(np.uint8))
    vw.release()
    print(f"{a.out}: {len(t) + a.pre} 프레임, 공 반지름 {proj(P[0])[2]:.1f} px, 초점거리 {f:.0f} px")


if __name__ == "__main__":
    main()
