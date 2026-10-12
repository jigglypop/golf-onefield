// golf-core `fly_flat` 의 결과 배열(Float64Array 한 개)을 읽는 순수 함수들. wasm 도 DOM 도 import 하지 않아서
// Node(scripts/check-physics.mjs)에서 그대로 불러 시험한다. 배치는 golf-core/src/lib.rs 의 `FLAT_HEAD` 설명과 같아야 한다.
//
// 배열 = 머리 12칸 + 경로 점 n 개 × 7칸.
//   머리: 0 carry_m, 1 lateral_m(오른쪽 +), 2 apex_m, 3 land_deg, 4 time_s, 5 steps, 6 land_mph, 7 roll_yd, 8 total_yd,
//         9 total_reliable(0|1), 10 n_points, 11 경로 점당 칸 수(= 7, 배열이 자기 배치를 말해 준다)
//   점:   [t s, x m, y m, z m, vx m/s, vy m/s, vz m/s]  (x 앞, y 위, z 오른쪽. 속도는 지면 기준 dX/dt)
// 경로는 걸음 수가 적어(앱 걸음 τ 0.137: 약 21점) 점 사이를 직선으로 이으면 모서리가 보인다. 위치와 속도를 둘 다 주므로
// 구간마다 3차 에르미트로 잇는다 (`hermiteAt`): 점 사이 위치 오차는 촘촘한 기준 경로 대비 0.2 mm 안쪽, 직선은 15 cm.
// 이 파일은 Node 의 타입 제거(strip-types)로도 읽히도록 enum·매개변수 속성 같은 타입 전용이 아닌 문법을 쓰지 않는다.

export const M_PER_YD = 0.9144;

/** 머리 칸 수 */
export const FLAT_HEAD = 12;
/** 경로 점당 칸 수 */
export const PATH_STRIDE = 7;

export interface ShotStats {
  carryYd: number;
  carryM: number;
  /** 최고 높이, m */
  apexM: number;
  /** 착지각, 도 (내려가는 각을 양수로) */
  landDeg: number;
  /** 휨 자체의 좌우 편차, m (출발 방향 기준, 오른쪽 +) */
  lateralM: number;
  timeS: number;
  steps: number;
  /** 착지 속력, mph */
  landMph: number;
  /** 롤, yd. 지면 물리가 아니라 TrackMan 기본 표면의 표시값을 따라가는 경험식 (golf-core ground 모듈) */
  rollYd: number;
  /** 총거리 = 캐리 + 롤, yd */
  totalYd: number;
  /** 총거리를 믿을 만한 발사 조건인가 (드라이버 자료 범위: 92–163 mph, 3–22°, 1650–5600 rpm) */
  totalReliable: boolean;
}

/** 머리를 요약값으로 푼다. 배열 배치가 다르면(pkg 와 이 파일이 어긋남) 바로 오류를 던진다. */
export function parseStats(a: Float64Array): ShotStats {
  if (a.length < FLAT_HEAD || a[11] !== PATH_STRIDE) {
    throw new Error('golf-core 결과 배열의 배치가 이 앱과 다릅니다. golf-core/build-wasm.sh 로 pkg 를 다시 만들고 npm run copy-wasm 을 하세요.');
  }
  return {
    carryYd: a[0]! / M_PER_YD,
    carryM: a[0]!,
    lateralM: a[1]!,
    apexM: a[2]!,
    landDeg: a[3]!,
    timeS: a[4]!,
    steps: a[5]!,
    landMph: a[6]!,
    rollYd: a[7]!,
    totalYd: a[8]!,
    totalReliable: a[9] !== 0,
  };
}

/** 경로 점 수 (머리의 n_points) */
export function pointCount(a: Float64Array): number {
  return a[10]!;
}

/** 경로만 (머리 뒤, 같은 메모리를 보는 뷰). 점 i 의 칸은 PATH_STRIDE * i */
export function pathOf(a: Float64Array): Float64Array {
  return a.subarray(FLAT_HEAD, FLAT_HEAD + PATH_STRIDE * pointCount(a));
}

/**
 * 시간 t 를 감싸는 구간 번호 i (path[i].t <= t < path[i+1].t). 시간이 단조 증가하는 애니메이션이면 직전 i 를 hint 로 주면 O(1).
 * 점이 2개 미만이면 0 을 돌려준다 (호출자가 먼저 n < 2 를 걸러야 한다).
 */
export function segmentAt(path: Float64Array, t: number, hint: number): number {
  const n = path.length / PATH_STRIDE;
  let i = Math.min(Math.max(hint, 0), Math.max(n - 2, 0));
  while (i > 0 && path[PATH_STRIDE * i]! > t) i--;
  while (i < n - 2 && path[PATH_STRIDE * (i + 1)]! <= t) i++;
  return i;
}

/**
 * 구간 i 의 시간 t 에서 위치 [x, y, z] 를 out[0..3] 에 쓴다. 3차 에르미트: 양 끝의 위치와 속도(dX/dt × 구간 길이)를 맞춘다.
 *   p(u) = h00 p0 + h10 Δt v0 + h01 p1 + h11 Δt v1,  u = (t − t0)/Δt ∈ [0, 1]
 * 반환: u (호출자가 궤적 선의 그릴 점 수를 정하는 데 쓴다).
 */
export function hermiteAt(path: Float64Array, i: number, t: number, out: Float64Array | number[]): number {
  const a = PATH_STRIDE * i;
  const b = a + PATH_STRIDE;
  const dt = path[b]! - path[a]!;
  const u = dt > 0 ? Math.min(Math.max((t - path[a]!) / dt, 0), 1) : 0;
  const u2 = u * u;
  const u3 = u2 * u;
  const h00 = 2 * u3 - 3 * u2 + 1;
  const h10 = u3 - 2 * u2 + u;
  const h01 = -2 * u3 + 3 * u2;
  const h11 = u3 - u2;
  for (let k = 0; k < 3; k++) {
    out[k] = h00 * path[a + 1 + k]! + h10 * dt * path[a + 4 + k]! + h01 * path[b + 1 + k]! + h11 * dt * path[b + 4 + k]!;
  }
  return u;
}
