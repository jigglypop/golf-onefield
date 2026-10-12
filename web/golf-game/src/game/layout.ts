// 한 홀의 지형 상수. 단위는 모두 m, 좌표는 three.js (Y 위, 목표 방향은 -Z).
export type Vec3 = [number, number, number];

/** 볼의 시각 반지름. 실물 반지름은 0.0213 m 이지만 화면에서 보이도록 키웠다 (물리에는 쓰지 않음) */
export const BALL_RADIUS = 0.05;

/** 티잉 지점 (볼의 기준점, 바닥 위) */
export const TEE: Vec3 = [0, 0, 0];

/** 깃대 위치. 티에서 230 m (약 250 yd) 앞 -Z 방향 */
export const FLAG: Vec3 = [0, 0, -230];

/** 골퍼 시작 위치: 볼 옆 약간 뒤, 목표 쪽을 본다 */
export const GOLFER_START: Vec3 = [-1.8, 2, 1.2];

/** 볼에서 이 거리 안에 있어야 샷을 할 수 있다 (수평 거리, m) */
export const REACH_M = 3;

/** 페어웨이: 티 앞쪽 -6 m 부터 그린 앞 -216 m 까지, 폭 36 m */
export const FAIRWAY = { zNear: -6, zFar: -216, halfWidth: 18 } as const;

export const GREEN = { z: -230, radius: 13 } as const;

export const BUNKERS: { pos: [number, number]; scale: [number, number] }[] = [
  { pos: [-19, -150], scale: [8, 5] },
  { pos: [17, -190], scale: [7, 5] },
];

/** 나무 줄기 중심 (x, z). 줄기는 물리 충돌체로 막고 수관은 보이기만 한다 */
export const TREES: [number, number][] = [
  [-28, -70],
  [30, -95],
  [-34, -150],
  [28, -205],
  [-40, -190],
];
