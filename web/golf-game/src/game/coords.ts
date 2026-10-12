import { FLAG, type Vec3 } from './layout';

/**
 * 좌표 변환 (wasm → three.js).
 * wasm 출력: x = 앞(m), y = 위(m), z = 오른쪽(m). three.js: x = 오른쪽, y = 위, z = 뒤(= -앞).
 * 그래서 로컬 좌표는 (z, y, -x) 이다. 골퍼가 -Z 를 보고 있으면 오른쪽은 +X 라서 z 양수가 오른쪽이 된다.
 */
export function flightToLocal(x: number, y: number, z: number): Vec3 {
  return [z, y, -x];
}

/**
 * 샷 루트 그룹의 Y 회전각 (라디안). HLA 가 양수(오른쪽)이면 three 의 rotation.y 는 음수여야 한다.
 * R_y(θ) 는 로컬 (0,0,-1) 을 (-sinθ, 0, -cosθ) 로 보내므로, 오른쪽(+X)으로 가려면 sinθ < 0.
 */
export function aimRotationY(hlaDeg: number): number {
  return (-hlaDeg * Math.PI) / 180;
}

/** 샷 루트 기준 로컬 점을 월드 좌표로 바꾼다 (루트 원점 = 출발점, Y 회전 = aimRotationY). */
export function localToWorld(start: Vec3, hlaDeg: number, local: Vec3): Vec3 {
  const th = aimRotationY(hlaDeg);
  const c = Math.cos(th);
  const s = Math.sin(th);
  const [lx, ly, lz] = local;
  return [start[0] + c * lx + s * lz, start[1] + ly, start[2] - s * lx + c * lz];
}

/**
 * 출발점에서 깃대로 가는 목표선을 기준으로 한 좌우 편차 (m, 오른쪽 +).
 * 전진 방향 f 의 오른쪽은 f × up = (-f_z, f_x) (수평면, 오른손 좌표계).
 */
export function lateralFromTargetM(start: Vec3, point: Vec3): number {
  const dx = FLAG[0] - start[0];
  const dz = FLAG[2] - start[2];
  const len = Math.hypot(dx, dz) || 1;
  const fx = dx / len;
  const fz = dz / len;
  return (point[0] - start[0]) * -fz + (point[2] - start[2]) * fx;
}
