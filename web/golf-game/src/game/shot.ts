import { PATH_STRIDE } from '../physics/flat';
import { M_PER_YD, simulateShot, type ShotParams, type ShotStats } from '../physics/golfCore';
import { flightToLocal, lateralFromTargetM, localToWorld } from './coords';
import type { Vec3 } from './layout';

/** 샷 한 번의 결과 기록. 장면(볼 애니메이션)과 HUD(결과 카드)가 함께 쓴다. */
export interface ShotRecord {
  id: number;
  /** 출발점 (월드) */
  start: Vec3;
  hlaDeg: number;
  /** 점마다 [t, x, y, z, vx, vy, vz] (PATH_STRIDE = 7칸, 위치 m·속도 m/s). 마지막 점이 착지. 점이 없을 수 있다(위로 나가지 않는 샷) */
  path: Float64Array;
  stats: ShotStats;
  /** 착지점 (월드, y = 0) */
  landing: Vec3;
  /** 목표선 기준 좌우 편차, yd (오른쪽 +) */
  lateralYd: number;
}

/** 현재 입력으로 샷을 날리고 기록을 만든다. wasm 호출은 동기이고 빠르다 (수 ms). */
export function fireShot(start: Vec3, params: ShotParams, id: number): ShotRecord {
  const { stats, path } = simulateShot(params);
  const n = path.length / PATH_STRIDE;
  // 점이 없으면(발사각 0° 처럼 위로 나가지 않는 샷) 출발점에 그대로 떨어진다
  const last = PATH_STRIDE * (n - 1);
  const local = n > 0 ? flightToLocal(path[last + 1]!, path[last + 2]!, path[last + 3]!) : flightToLocal(0, 0, 0);
  const landing = localToWorld(start, params.hlaDeg, local);
  landing[1] = 0; // 착지 후 굴러가는 거리는 모델에 없으므로 착지점에 멈춘다
  const lateralM = lateralFromTargetM(start, landing);
  return {
    id,
    start,
    hlaDeg: params.hlaDeg,
    path,
    stats,
    landing,
    lateralYd: lateralM / M_PER_YD,
  };
}
