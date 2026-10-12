/**
 * 코스: 직접 설계한 9홀 + 롱드라이브 그리드 (실제 골프장 자료는 라이선스 문제로 쓰지 않는다).
 * 홀 자료는 `courses/classic9.json` 에 있고, 이 파일은 그것을 읽어 `Hole` 로 바꾼다 (`buildHole`).
 *
 * 좌표: 홀마다 따로, 1 단위 = 1 m. 티는 원점 근처, 그린 쪽이 대체로 +x, +z 는 오른쪽 (convention.ts 와 같다).
 * 땅은 평평하다(y = 0). 그린만 기울기(높이 기울기 dh/dx, dh/dz)를 가지며, 굴림에만 쓴다.
 * 지면 우선순위: OB(경계 밖) > 물(땅 다각형 안은 제외: 섬 그린) > 벙커 > 그린 > 프린지 > 티 > 페어웨이 > 러프.
 * 나무(`trees`)는 땅 높이를 바꾸지 않는다: 비행하는 공만 막는 원기둥이다 (obstacles.ts).
 */
import classic9 from './courses/classic9.json';

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

/**
 * 나무 장애물: 바닥 (x, z) 에 선 원기둥 (반지름 r, 꼭대기 높이 h, m). 땅은 평평한 채 두고 날아가는 공만 막는다.
 */
export interface Tree {
  x: number;
  z: number;
  r: number;
  h: number;
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
  /** 플레이 구역 안의 나무 (비행하는 공만 막는다) */
  trees?: Tree[];
  /** 롱드라이브 그리드 (그린 없이 거리만 잰다) */
  kind?: 'longdrive';
}

/** 프린지: 그린 타원 반지름에 더하는 폭 (m) */
export const FRINGE_M = 2.6;
/** 티 박스 반 길이(조준 방향)·반 폭 (m) */
export const TEE_BOX: Vec2 = [5.5, 4];
/** 홀컵 반지름 (m, 4.25 인치) */
export const CUP_R = 0.054;

// ------------------------------------------------------------------ 자료 읽기

/** 섬 둘레 규격: [cx, cz, rx, rz, rot] → `ring()` 으로 다각형을 만든다 */
type RingSpec = readonly [number, number, number, number, number];

/** JSON 에 적힌 홀. `land` 대신 `landRings` 규격을 둔다 */
type RawHole = Omit<Hole, 'land' | 'tee' | 'teeTarget' | 'pin' | 'fairways' | 'green' | 'bunkers' | 'water' | 'bounds' | 'trees'> & {
  tee: number[];
  teeTarget: number[];
  pin: number[];
  fairways: { path: number[][]; width: number }[];
  green: { c: number[]; rx: number; rz: number; rot: number; slope: number[] };
  bunkers: { c: number[]; rx: number; rz: number; rot: number }[];
  water: number[][][];
  bounds: number[][];
  landRings?: RingSpec[];
  trees?: Tree[];
};

/** JSON 홀 → Hole. `landRings` 는 섬 둘레 다각형 `land` 로 펼친다 (옛 코드의 `ring()` 호출과 같은 값) */
export function buildHole(raw: unknown): Hole {
  const { landRings, ...rest } = raw as RawHole;
  const hole = rest as unknown as Hole;
  if (landRings && landRings.length > 0) hole.land = landRings.map(([cx, cz, rx, rz, rot]) => ring(cx, cz, rx, rz, rot));
  return hole;
}

export const HOLES: Hole[] = (classic9.holes as unknown[]).map(buildHole);

/** 롱드라이브 그리드: 폭 50 m 페어웨이, 좌우 45 m 밖은 OB. 거리는 티에서 +x 로 잰다 */
export const LONG_DRIVE: Hole = buildHole(classic9.longDrive);

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
