/**
 * 코스: 직접 설계한 9홀 + 롱드라이브 그리드 (실제 골프장 자료는 라이선스 문제로 쓰지 않는다).
 *
 * 좌표: 홀마다 따로, 1 단위 = 1 m. 티는 원점 근처, 그린 쪽이 대체로 +x, +z 는 오른쪽 (convention.ts 와 같다).
 * 땅은 평평하다(y = 0). 그린만 기울기(높이 기울기 dh/dx, dh/dz)를 가지며, 굴림에만 쓴다.
 * 지면 우선순위: OB(경계 밖) > 물(땅 다각형 안은 제외: 섬 그린) > 벙커 > 그린 > 프린지 > 티 > 페어웨이 > 러프.
 */

/** (x, z) m */
export type Vec2 = readonly [number, number];

export type Surface = 'tee' | 'fairway' | 'fringe' | 'green' | 'rough' | 'bunker' | 'water' | 'ob';

export const SURFACE_LABEL: Record<Surface, string> = {
  tee: '티',
  fairway: '페어웨이',
  fringe: '프린지',
  green: '그린',
  rough: '러프',
  bunker: '벙커',
  water: '해저드(물)',
  ob: 'OB',
};

/** 회전한 타원: 중심, 반지름(x, z 방향), 회전(rad, x 축에서 +z 쪽으로) */
export interface Ellipse {
  c: Vec2;
  rx: number;
  rz: number;
  rot: number;
}

export interface Fairway {
  /** 가운데 선 */
  path: Vec2[];
  /** 폭 (m) */
  width: number;
}

export interface Green extends Ellipse {
  /** 높이 기울기 (dh/dx, dh/dz). 공은 낮은 쪽, 곧 −기울기 방향으로 흐른다 */
  slope: Vec2;
}

export interface Hole {
  number: number;
  name: string;
  par: number;
  tee: Vec2;
  /** 티샷 기본 조준점 (도그레그 꺾이는 곳 등) */
  teeTarget: Vec2;
  pin: Vec2;
  fairways: Fairway[];
  green: Green;
  bunkers: Ellipse[];
  water: Vec2[][];
  /** 플레이 구역 (밖은 OB) */
  bounds: Vec2[];
  /** 물 위의 땅 (섬 그린 둘레 등): 이 안은 물이 아니다 */
  land?: Vec2[][];
  /** 롱드라이브 그리드 (그린 없이 거리만 잰다) */
  kind?: 'longdrive';
}

/** 프린지: 그린 타원 반지름에 더하는 폭 (m) */
export const FRINGE_M = 2.6;
/** 티 박스 반 길이(조준 방향)·반 폭 (m) */
export const TEE_BOX: Vec2 = [5.5, 4];
/** 홀컵 반지름 (m, 4.25 인치) */
export const CUP_R = 0.054;

export const HOLES: Hole[] = [
  {
    number: 1,
    name: '솔숲 도그레그',
    par: 4,
    tee: [0, 0],
    teeTarget: [225, 12],
    pin: [341, 59],
    fairways: [
      {
        path: [
          [28, 0],
          [120, 2],
          [200, 8],
          [255, 26],
          [300, 46],
          [322, 56],
        ],
        width: 34,
      },
    ],
    green: { c: [338, 62], rx: 16, rz: 13, rot: 0.5, slope: [-0.012, 0.018] },
    bunkers: [
      { c: [222, -12], rx: 10, rz: 6, rot: 0.1 },
      { c: [318, 82], rx: 7, rz: 5, rot: 0.6 },
      { c: [357, 44], rx: 6, rz: 4.5, rot: -0.4 },
    ],
    water: [],
    bounds: [
      [-25, -45],
      [240, -45],
      [300, -10],
      [390, 20],
      [395, 105],
      [300, 110],
      [230, 70],
      [180, 50],
      [-25, 45],
    ],
  },
  {
    number: 2,
    name: '호수 건너기',
    par: 3,
    tee: [0, 0],
    teeTarget: [153, -5],
    pin: [153, -5],
    fairways: [
      {
        path: [
          [121, -8],
          [131, -8],
        ],
        width: 20,
      },
    ],
    green: { c: [150, -8], rx: 14, rz: 11, rot: 0.2, slope: [0.018, 0.006] },
    bunkers: [
      { c: [168, -1], rx: 6, rz: 4, rot: 0.3 },
      { c: [146, -24], rx: 7, rz: 4, rot: 0 },
    ],
    water: [
      [
        [26, -36],
        [104, -40],
        [112, -18],
        [110, 14],
        [100, 30],
        [38, 32],
        [22, 8],
      ],
    ],
    bounds: [
      [-25, -60],
      [200, -60],
      [200, 45],
      [-25, 45],
    ],
  },
  {
    number: 3,
    name: '개울 넘어 긴 여정',
    par: 5,
    tee: [0, 0],
    teeTarget: [250, -8],
    pin: [472, 33],
    fairways: [
      {
        path: [
          [28, 0],
          [140, -6],
          [250, -10],
          [330, 4],
          [390, 21],
        ],
        width: 36,
      },
      {
        path: [
          [426, 30],
          [446, 33],
        ],
        width: 26,
      },
    ],
    green: { c: [468, 36], rx: 17, rz: 13, rot: 0.3, slope: [-0.01, -0.016] },
    bunkers: [
      { c: [232, 14], rx: 9, rz: 5.5, rot: 0.2 },
      { c: [268, -30], rx: 9, rz: 6, rot: -0.2 },
      { c: [318, -14], rx: 8, rz: 5, rot: 0.4 },
      { c: [455, 58], rx: 7, rz: 4.5, rot: 0.5 },
      { c: [490, 22], rx: 6, rz: 4, rot: -0.6 },
    ],
    water: [
      [
        [400, -60],
        [412, -60],
        [420, 82],
        [408, 82],
      ],
    ],
    bounds: [
      [-25, -55],
      [330, -55],
      [520, -10],
      [525, 85],
      [380, 80],
      [200, 45],
      [-25, 40],
    ],
  },
  {
    number: 4,
    name: '섬 그린',
    par: 3,
    tee: [0, 0],
    teeTarget: [136, 3],
    pin: [136, 3],
    fairways: [],
    green: { c: [134, 4], rx: 12, rz: 10, rot: 0.15, slope: [0.012, -0.008] },
    bunkers: [],
    water: [
      [
        [40, -45],
        [185, -45],
        [190, 50],
        [40, 52],
      ],
    ],
    land: [ring(134, 4, 18, 16, 0.15)],
    bounds: [
      [-25, -60],
      [205, -60],
      [205, 65],
      [-25, 65],
    ],
  },
  {
    number: 5,
    name: '왼쪽으로 굽은 길',
    par: 4,
    tee: [0, 0],
    teeTarget: [230, -10],
    pin: [355, -77],
    fairways: [
      {
        path: [
          [28, 0],
          [150, 0],
          [230, -8],
          [290, -32],
          [338, -62],
        ],
        width: 32,
      },
    ],
    green: { c: [352, -74], rx: 15, rz: 12, rot: -0.6, slope: [0.01, 0.015] },
    bunkers: [
      { c: [245, 14], rx: 9, rz: 5, rot: -0.2 },
      { c: [235, -32], rx: 7, rz: 4.5, rot: -0.4 },
      { c: [335, -56], rx: 6, rz: 4, rot: -0.6 },
      { c: [372, -90], rx: 6, rz: 4, rot: -0.6 },
    ],
    water: [
      [
        [362, -52],
        [400, -48],
        [404, -22],
        [372, -25],
      ],
    ],
    bounds: [
      [-25, -45],
      [200, -45],
      [280, -80],
      [330, -110],
      [400, -115],
      [420, -60],
      [410, 0],
      [300, 30],
      [200, 40],
      [-25, 40],
    ],
  },
  {
    number: 6,
    name: '짧은 파4',
    par: 4,
    tee: [0, 0],
    teeTarget: [200, 6],
    pin: [278, -2],
    fairways: [
      {
        path: [
          [28, 0],
          [120, 3],
          [200, 6],
          [248, 1],
        ],
        width: 28,
      },
    ],
    green: { c: [275, -4], rx: 14, rz: 11, rot: 0, slope: [0.016, 0] },
    bunkers: [
      { c: [250, -13], rx: 6, rz: 3.5, rot: 0.3 },
      { c: [255, 12], rx: 6, rz: 4, rot: -0.3 },
      { c: [175, -14], rx: 8, rz: 4.5, rot: 0.1 },
      { c: [205, 20], rx: 7, rz: 4, rot: 0 },
    ],
    water: [
      [
        [225, -40],
        [300, -42],
        [305, -24],
        [240, -21],
      ],
    ],
    bounds: [
      [-25, -55],
      [320, -55],
      [325, 45],
      [-25, 45],
    ],
  },
  {
    number: 7,
    name: '긴 파3',
    par: 3,
    tee: [0, 0],
    teeTarget: [183, 8],
    pin: [183, 8],
    fairways: [
      {
        path: [
          [130, 4],
          [160, 6],
        ],
        width: 22,
      },
    ],
    green: { c: [180, 7], rx: 15, rz: 12, rot: 0.1, slope: [-0.012, 0.01] },
    bunkers: [
      { c: [168, -10], rx: 7, rz: 4, rot: 0.2 },
      { c: [172, 24], rx: 7, rz: 4, rot: -0.2 },
      { c: [200, 10], rx: 5, rz: 3.5, rot: 1.2 },
    ],
    water: [],
    bounds: [
      [-25, -50],
      [225, -50],
      [225, 55],
      [-25, 55],
    ],
  },
  {
    number: 8,
    name: '물길',
    par: 4,
    tee: [0, 0],
    teeTarget: [240, 8],
    pin: [375, 5],
    fairways: [
      {
        path: [
          [28, 0],
          [140, 6],
          [240, 10],
          [330, 6],
          [356, 4],
        ],
        width: 34,
      },
    ],
    green: { c: [372, 2], rx: 16, rz: 12, rot: 0, slope: [0.01, -0.014] },
    bunkers: [
      { c: [250, 32], rx: 9, rz: 5, rot: 0 },
      { c: [365, 22], rx: 7, rz: 4, rot: 0.2 },
      { c: [393, 0], rx: 4, rz: 6, rot: 0 },
    ],
    water: [
      [
        [60, -28],
        [380, -30],
        [400, -60],
        [60, -70],
      ],
    ],
    bounds: [
      [-25, -80],
      [420, -80],
      [420, 55],
      [-25, 55],
    ],
  },
  {
    number: 9,
    name: '마지막 홀',
    par: 5,
    tee: [0, 0],
    teeTarget: [250, 12],
    pin: [490, -15],
    fairways: [
      {
        path: [
          [28, 0],
          [150, 6],
          [250, 14],
          [330, -6],
          [380, -22],
        ],
        width: 34,
      },
      {
        path: [
          [408, -26],
          [452, -20],
        ],
        width: 28,
      },
    ],
    green: { c: [486, -12], rx: 17, rz: 13, rot: 0.2, slope: [-0.014, 0.008] },
    bunkers: [
      { c: [238, 34], rx: 9, rz: 5, rot: 0.1 },
      { c: [318, -26], rx: 8, rz: 5, rot: -0.3 },
      { c: [470, -32], rx: 7, rz: 4, rot: 0.2 },
      { c: [508, -2], rx: 4, rz: 6, rot: 0.4 },
    ],
    water: [
      [
        [388, -80],
        [398, -80],
        [400, 60],
        [390, 60],
      ],
      [
        [474, 9],
        [530, 11],
        [532, 40],
        [478, 36],
      ],
    ],
    bounds: [
      [-25, -55],
      [300, -60],
      [545, -50],
      [545, 55],
      [300, 60],
      [-25, 45],
    ],
  },
];

/** 롱드라이브 그리드: 폭 50 m 페어웨이, 좌우 45 m 밖은 OB. 거리는 티에서 +x 로 잰다 */
export const LONG_DRIVE: Hole = {
  number: 0,
  name: '롱드라이브 그리드',
  par: 0,
  tee: [0, 0],
  teeTarget: [300, 0],
  pin: [520, 0],
  fairways: [
    {
      path: [
        [24, 0],
        [430, 0],
      ],
      width: 50,
    },
  ],
  green: { c: [520, 0], rx: 6, rz: 6, rot: 0, slope: [0, 0] },
  bunkers: [],
  water: [],
  bounds: [
    [-25, -45],
    [470, -45],
    [470, 45],
    [-25, 45],
  ],
  kind: 'longdrive',
};

/** 니어핀 챌린지 홀 (섬 그린) */
export const CTP_HOLE = 3;

/** 섬 둘레 같은 타원 다각형 (HOLES 를 만들 때 쓴다) */
function ring(cx: number, cz: number, rx: number, rz: number, rot: number, n = 40): Vec2[] {
  const c = Math.cos(rot);
  const s = Math.sin(rot);
  const out: Vec2[] = [];
  for (let i = 0; i < n; i++) {
    const a = (2 * Math.PI * i) / n;
    const u = rx * Math.cos(a);
    const v = rz * Math.sin(a);
    out.push([cx + u * c - v * s, cz + u * s + v * c]);
  }
  return out;
}

// ------------------------------------------------------------------ 기하

export function inEllipse(e: Ellipse, x: number, z: number, grow = 0): boolean {
  const dx = x - e.c[0];
  const dz = z - e.c[1];
  const c = Math.cos(e.rot);
  const s = Math.sin(e.rot);
  const u = (dx * c + dz * s) / (e.rx + grow);
  const v = (-dx * s + dz * c) / (e.rz + grow);
  return u * u + v * v <= 1;
}

/** 짝홀 규칙 */
export function inPolygon(poly: readonly Vec2[], x: number, z: number): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, zi] = poly[i]!;
    const [xj, zj] = poly[j]!;
    if (zi > z !== zj > z && x < ((xj - xi) * (z - zi)) / (zj - zi) + xi) inside = !inside;
  }
  return inside;
}

/** 점에서 꺾은선까지 거리 */
export function distToPolyline(path: readonly Vec2[], x: number, z: number): number {
  let best = Infinity;
  for (let i = 0; i + 1 < path.length; i++) {
    const [ax, az] = path[i]!;
    const [bx, bz] = path[i + 1]!;
    const dx = bx - ax;
    const dz = bz - az;
    const len2 = dx * dx + dz * dz;
    const t = len2 > 0 ? Math.min(Math.max(((x - ax) * dx + (z - az) * dz) / len2, 0), 1) : 0;
    best = Math.min(best, Math.hypot(x - (ax + t * dx), z - (az + t * dz)));
  }
  return best;
}

/** 티 박스 안인가 (티에서 조준점 쪽으로 놓인 직사각형) */
export function inTeeBox(h: Hole, x: number, z: number): boolean {
  const yaw = Math.atan2(h.teeTarget[1] - h.tee[1], h.teeTarget[0] - h.tee[0]);
  const dx = x - h.tee[0];
  const dz = z - h.tee[1];
  const u = dx * Math.cos(yaw) + dz * Math.sin(yaw);
  const v = -dx * Math.sin(yaw) + dz * Math.cos(yaw);
  return Math.abs(u + 1.5) <= TEE_BOX[0] && Math.abs(v) <= TEE_BOX[1];
}

export function surfaceAt(h: Hole, x: number, z: number): Surface {
  if (!inPolygon(h.bounds, x, z)) return 'ob';
  for (const w of h.water) if (inPolygon(w, x, z) && !h.land?.some((l) => inPolygon(l, x, z))) return 'water';
  for (const b of h.bunkers) if (inEllipse(b, x, z)) return 'bunker';
  if (inEllipse(h.green, x, z)) return 'green';
  if (inEllipse(h.green, x, z, FRINGE_M)) return 'fringe';
  if (inTeeBox(h, x, z)) return 'tee';
  for (const f of h.fairways) if (distToPolyline(f.path, x, z) <= f.width / 2) return 'fairway';
  return 'rough';
}

/** 홀 길이 (yd): 티 → 조준점 → 핀 (파3 은 티 → 핀) */
export function holeLengthM(h: Hole): number {
  const a = Math.hypot(h.teeTarget[0] - h.tee[0], h.teeTarget[1] - h.tee[1]);
  const b = Math.hypot(h.pin[0] - h.teeTarget[0], h.pin[1] - h.teeTarget[1]);
  return a + b;
}

/** 방향 (rad): from 에서 to 로. 0 = +x, +π/2 = +z(오른쪽) */
export function yawTo(from: Vec2 | { x: number; z: number }, to: Vec2): number {
  const fx = Array.isArray(from) ? (from as Vec2)[0] : (from as { x: number }).x;
  const fz = Array.isArray(from) ? (from as Vec2)[1] : (from as { z: number }).z;
  return Math.atan2(to[1] - fz, to[0] - fx);
}

/** 타원 둘레 점 (그리기·나무 배치용) */
export function ellipsePoints(e: Ellipse, grow = 0, n = 48): Vec2[] {
  const c = Math.cos(e.rot);
  const s = Math.sin(e.rot);
  const out: Vec2[] = [];
  for (let i = 0; i < n; i++) {
    const a = (2 * Math.PI * i) / n;
    const u = (e.rx + grow) * Math.cos(a);
    const v = (e.rz + grow) * Math.sin(a);
    out.push([e.c[0] + u * c - v * s, e.c[1] + u * s + v * c]);
  }
  return out;
}

/** 페어웨이 윤곽: 가운데 선 양옆으로 폭/2, 끝은 반원 */
export function fairwayOutline(f: Fairway, capSegments = 10): Vec2[] {
  const p = f.path;
  const hw = f.width / 2;
  const n = p.length;
  const normals: Vec2[] = [];
  for (let i = 0; i < n; i++) {
    // 왼쪽 법선 (전진 (dx, dz) 의 왼쪽 = (dz, −dx)), 꺾이는 곳은 앞뒤 법선의 평균
    const a = p[Math.max(i - 1, 0)]!;
    const b = p[Math.min(i + 1, n - 1)]!;
    const dx = b[0] - a[0];
    const dz = b[1] - a[1];
    const l = Math.hypot(dx, dz) || 1;
    normals.push([dz / l, -dx / l]);
  }
  const left: Vec2[] = p.map((q, i) => [q[0] + normals[i]![0] * hw, q[1] + normals[i]![1] * hw]);
  const right: Vec2[] = p.map((q, i) => [q[0] - normals[i]![0] * hw, q[1] - normals[i]![1] * hw]);
  const cap = (center: Vec2, from: Vec2, to: Vec2): Vec2[] => {
    // from → to 로 center 둘레 반원 (바깥쪽)
    const a0 = Math.atan2(from[1] - center[1], from[0] - center[0]);
    let a1 = Math.atan2(to[1] - center[1], to[0] - center[0]);
    // 바깥쪽으로 도는 방향: 끝 반원은 시계 방향/반시계 방향 중 진행 방향 앞쪽을 지나는 쪽
    if (a1 <= a0) a1 += 2 * Math.PI;
    const out: Vec2[] = [];
    for (let k = 1; k < capSegments; k++) {
      const a = a0 + ((a1 - a0) * k) / capSegments;
      out.push([center[0] + hw * Math.cos(a), center[1] + hw * Math.sin(a)]);
    }
    return out;
  };
  const endCap = cap(p[n - 1]!, left[n - 1]!, right[n - 1]!);
  const startCap = cap(p[0]!, right[0]!, left[0]!);
  return [...left, ...endCap, ...right.slice().reverse(), ...startCap];
}
