/**
 * 게임용 지면 물리: 착지 뒤 바운드·굴림, 퍼팅, 홀컵.
 *
 * 저장소의 지면 모형(ground.py)은 USGA 런 자료 검증에 실패했다. 이 모형은 게임 손맛을 위한 것이고 자료로 검증하지 않았다.
 *
 * 바운드 (강체 구, 수직 반발 + 접선 마찰 충격):
 *   반발계수 $e = e_0/(1 + v_n/v_e)$ (세게 떨어질수록 덜 튐).
 *   접점 미끄럼 $s = v_h + R\omega$ ($\omega$ 는 역회전 +). 마찰 충격 $J = \min(s/3.5,\ \mu(1+e)v_n)$:
 *   $s/3.5$ 에서 접점이 멈춰 구름이 되고(그 뒤로는 마찰이 없다), 그 전에 한도에 닿으면 미끄럼으로 끝난다.
 *   $v_h' = (v_h - J)\cdot k$, $k = 1-(1-k_0)\min(1, v_n/12)$ (세게 박힐수록 잔디가 많이 먹는다).
 *   역회전이 크면 $v_h' < 0$ 이 되어 공이 뒤로 온다(백스핀).
 * 굴림: 감속 $c_r g$ (그린 $c_r$ = 0.056 → 1.83 m/s 로 굴리면 3.05 m, 스팀프 10), 그린 경사 가속 $-\tfrac57 g\,\nabla h$.
 * 홀컵: 공 중심이 컵 반지름 안을 지나며 속도가 충분히 느리면 들어가고, 빠르면 가장자리에 걸려 꺾인다(립아웃).
 *   바운드로 떨어지는 공도 착지점이 컵 반지름 안이고 수평 속도가 `CAPTURE_SPEED_DROP` 보다 느리면 들어간다.
 * 바람: 굴러가는 공도 공기에 대한 상대 속도 $\mathbf r=\mathbf w-\mathbf v$ 의 끌림을 받는다. 무풍의 끌림은 굴림 저항 $c_r g$ 에 이미 들어 있으니
 *   바람 때문에 달라지는 몫만 더한다: $\mathbf a_w = k_w\,(|\mathbf r|\,\mathbf r + |\mathbf v|\,\mathbf v)$ (바람 0 이면 정확히 0).
 *   $k_w$ = `ROLL_WIND_K` (자유 공기 값 $\tfrac12\rho C_d A/m \approx 0.0047\ \mathrm{m^{-1}}$ 의 약 4분의 1: 잔디 가까이라 가려진다).
 *   5 m/s 면 0.03 m/s², 그린 굴림 감속 0.55 m/s² 의 약 5 %. 정지 판정에도 같이 들어가 바람이 멎은 공을 밀지는 못한다.
 */
import { R_BALL } from '../engine/units.ts';
import { CUP_R, surfaceAt, type Hole, type Surface } from './course.ts';

const G = 9.81;

export interface Turf {
  /** 반발계수 (느린 충돌의 극한) */
  e0: number;
  /** 반발계수가 반으로 주는 수직 속도 (m/s) */
  ve: number;
  /** 미끄럼 마찰 */
  mu: number;
  /** 세게(수직 12 m/s 이상) 박힐 때 남는 수평 속도의 몫. 약한 바운드일수록 1 에 가깝다 */
  damp: number;
  /** 굴림 저항 (g 의 배수) */
  roll: number;
}

export const TURF: Record<Surface, Turf> = {
  tee: { e0: 0.45, ve: 6, mu: 0.4, damp: 0.5, roll: 0.11 },
  fairway: { e0: 0.45, ve: 6, mu: 0.4, damp: 0.5, roll: 0.11 },
  fringe: { e0: 0.35, ve: 6, mu: 0.5, damp: 0.4, roll: 0.08 },
  green: { e0: 0.3, ve: 6, mu: 0.6, damp: 0.3, roll: 0.056 },
  rough: { e0: 0.2, ve: 6, mu: 0.7, damp: 0.35, roll: 0.35 },
  bunker: { e0: 0.03, ve: 6, mu: 0.9, damp: 0.08, roll: 2.5 },
  water: { e0: 0, ve: 1, mu: 1, damp: 0, roll: 99 },
  ob: { e0: 0.2, ve: 6, mu: 0.7, damp: 0.35, roll: 0.35 },
};

/** 컵 한가운데를 지날 때 들어가는 최대 속도 (m/s). 가장자리일수록 작아진다 */
export const CAPTURE_SPEED = 1.4;
/** 바운드로 컵 위에 떨어질 때 들어가는 최대 수평 속도 (m/s). 위에서 떨어지는 공은 굴러 오는 공보다 잘 들어간다 */
export const CAPTURE_SPEED_DROP = 2.5;
/** 굴림 중 바람 계수 $k_w$ (1/m). 위 설명 참고 */
export const ROLL_WIND_K = 0.0012;
/** 퍼팅: 파워 1 이 굴리는 평평한 그린 거리의 상한 (m). 실제 범위는 puttMaxFor */
export const PUTT_MAX_M = 24;

const DT = 1 / 240;
const SAMPLE_DT = 1 / 60;

export interface GroundSample {
  t: number;
  x: number;
  y: number;
  z: number;
}

export type GroundEvent = 'rest' | 'holed' | 'water' | 'ob';

/** 바운드로 땅에 닿은 순간 (첫 착지는 뺀다): 시각 (s, 착지 = 0)과 수직 속도 (m/s) */
export interface GroundHit {
  t: number;
  vn: number;
}

/** 월드 틀 바람 (m/s). x = +x 방향, z = +z 방향 */
export interface GroundWind {
  x: number;
  z: number;
}

export interface GroundResult {
  /** 첫 점은 착지점(또는 퍼트 시작점), 시간 0 부터 */
  samples: GroundSample[];
  rest: { x: number; z: number };
  /** 멈춘 곳의 지면 (물·OB 이면 그곳) */
  surface: Surface;
  event: GroundEvent;
  /** 처음 닿은 지면 */
  firstSurface: Surface;
  duration: number;
  bounces: number;
  /** 첫 착지점에서 멈춘 곳까지 (m) */
  runM: number;
  /** 두 번째 이후 착지 (효과음 시각) */
  hits: GroundHit[];
  /** 굴림이 시작된 시각 (s, 착지 = 0). 퍼트는 0 */
  rollStart: number;
}

export interface Landing {
  x: number;
  z: number;
  vx: number;
  /** 착지 때 음수 */
  vy: number;
  vz: number;
  /** 역회전 성분 (rpm, +) */
  backspinRpm: number;
}

/** 선분 (ax, az)–(bx, bz) 와 점 (px, pz) 의 거리 */
function segDist(ax: number, az: number, bx: number, bz: number, px: number, pz: number): number {
  const dx = bx - ax;
  const dz = bz - az;
  const l2 = dx * dx + dz * dz;
  const t = l2 > 0 ? Math.min(Math.max(((px - ax) * dx + (pz - az) * dz) / l2, 0), 1) : 0;
  return Math.hypot(px - (ax + t * dx), pz - (az + t * dz));
}

interface Track {
  hole: Hole;
  samples: GroundSample[];
  x0: number;
  z0: number;
  t: number;
  bounces: number;
  firstSurface: Surface | null;
  hits: GroundHit[];
  rollStart: number;
  wind: GroundWind | null;
}

function finish(tr: Track, x: number, z: number, event: GroundEvent): GroundResult {
  const last = tr.samples[tr.samples.length - 1]!;
  if (event === 'holed') {
    // 컵으로 떨어지는 짧은 모습
    tr.samples.push({ t: tr.t, x, y: 0, z }, { t: tr.t + 0.12, x, y: -0.06, z });
    tr.t += 0.12;
  } else if (last.t < tr.t || last.x !== x || last.z !== z) {
    tr.samples.push({ t: tr.t, x, y: 0, z });
  }
  const surface = event === 'holed' ? 'green' : surfaceAt(tr.hole, x, z);
  return {
    samples: tr.samples,
    rest: { x, z },
    surface,
    event,
    firstSurface: tr.firstSurface ?? surface,
    duration: tr.t,
    bounces: tr.bounces,
    runM: Math.hypot(x - tr.x0, z - tr.z0),
    hits: tr.hits,
    rollStart: tr.rollStart,
  };
}

/** 굴림: (x, z) 에서 수평 속도 (vx, vz) 로. 멈추거나 홀컵·물·OB 에서 끝난다 */
function roll(tr: Track, x0: number, z0: number, vx0: number, vz0: number): GroundResult {
  const { hole } = tr;
  const [px, pz] = hole.pin;
  let x = x0;
  let z = z0;
  let vx = vx0;
  let vz = vz0;
  let nextSample = tr.t + SAMPLE_DT;
  let lipped = false;
  tr.rollStart = tr.t;
  const wind = tr.wind;
  for (let i = 0; i < 240 * 90; i++) {
    const surf = surfaceAt(hole, x, z);
    if (tr.firstSurface === null) tr.firstSurface = surf;
    if (surf === 'water' || surf === 'ob') return finish(tr, x, z, surf);
    const turf = TURF[surf];
    const onGreen = surf === 'green' || surf === 'fringe';
    // 굴러가는 공의 경사 가속은 (5/7) g sinθ (회전 관성 몫)
    let gx = onGreen ? -(5 / 7) * G * hole.green.slope[0] : 0;
    let gz = onGreen ? -(5 / 7) * G * hole.green.slope[1] : 0;
    if (wind) {
      // 바람 때문에 달라지는 끌림만 더한다: k (|r| r + |v| v), r = 바람 − 공.
      // 무풍의 끌림은 굴림 저항 계수(roll)에 이미 들어 있으므로 바람이 0 이면 정확히 0 이다.
      const rx = wind.x - vx;
      const rz = wind.z - vz;
      const rn = Math.hypot(rx, rz);
      const vn = Math.hypot(vx, vz);
      gx += ROLL_WIND_K * (rn * rx + vn * vx);
      gz += ROLL_WIND_K * (rn * rz + vn * vz);
    }
    const fr = turf.roll * G;
    const v = Math.hypot(vx, vz);
    if (v < 0.03 && Math.hypot(gx, gz) <= fr * 1.2) return finish(tr, x, z, 'rest');
    let nvx = vx + gx * DT;
    let nvz = vz + gz * DT;
    const nv = Math.hypot(nvx, nvz);
    const dv = fr * DT;
    if (nv <= dv) {
      nvx = 0;
      nvz = 0;
    } else {
      nvx -= (nvx / nv) * dv;
      nvz -= (nvz / nv) * dv;
    }
    const nx = x + 0.5 * (vx + nvx) * DT;
    const nz = z + 0.5 * (vz + nvz) * DT;
    if (onGreen && Math.abs(nx - px) < 1 && Math.abs(nz - pz) < 1) {
      const d = segDist(x, z, nx, nz, px, pz);
      if (d < CUP_R) {
        const speed = 0.5 * (v + Math.hypot(nvx, nvz));
        if (speed < CAPTURE_SPEED * (1 - 0.6 * (d / CUP_R))) {
          tr.t += DT;
          return finish(tr, px, pz, 'holed');
        }
        if (!lipped) {
          // 립아웃: 컵 가장자리에 걸려 컵 반대쪽으로 꺾이고 느려진다
          lipped = true;
          const side = Math.sign(nvx * (pz - z) - nvz * (px - x)) || 1;
          const ang = -side * 0.5 * (1 - d / CUP_R);
          const c = Math.cos(ang);
          const s = Math.sin(ang);
          const rx = nvx * c - nvz * s;
          const rz = nvx * s + nvz * c;
          nvx = rx * 0.7;
          nvz = rz * 0.7;
        }
      } else if (d > CUP_R * 2) {
        lipped = false;
      }
    }
    x = nx;
    z = nz;
    vx = nvx;
    vz = nvz;
    tr.t += DT;
    if (tr.t >= nextSample) {
      tr.samples.push({ t: tr.t, x, y: 0, z });
      nextSample += SAMPLE_DT;
    }
  }
  return finish(tr, x, z, 'rest');
}

/** 착지 뒤: 바운드 몇 번 → 굴림. wind 는 굴림에만 쓴다 (비행 중 바람은 flight.ts) */
export function bounceAndRoll(hole: Hole, l: Landing, wind?: GroundWind): GroundResult {
  const tr: Track = { hole, samples: [{ t: 0, x: l.x, y: 0, z: l.z }], x0: l.x, z0: l.z, t: 0, bounces: 0, firstSurface: null, hits: [], rollStart: 0, wind: wind ?? null };
  let x = l.x;
  let z = l.z;
  let vx = l.vx;
  let vz = l.vz;
  let vy = l.vy;
  let omega = (l.backspinRpm * 2 * Math.PI) / 60;
  let dirx = 1;
  let dirz = 0;
  for (; tr.bounces < 12; tr.bounces++) {
    const surf = surfaceAt(hole, x, z);
    if (tr.firstSurface === null) tr.firstSurface = surf;
    if (surf === 'water' || surf === 'ob') return finish(tr, x, z, surf);
    const turf = TURF[surf];
    const vn = Math.max(-vy, 0);
    const vh = Math.hypot(vx, vz);
    // 컵 위에 떨어지면 (바운드 중이든 첫 착지든) 수평 속도가 느릴 때 들어간다
    if (surf === 'green' || surf === 'fringe') {
      const d = Math.hypot(x - hole.pin[0], z - hole.pin[1]);
      if (d < CUP_R && vh < CAPTURE_SPEED_DROP * (1 - 0.6 * (d / CUP_R))) return finish(tr, hole.pin[0], hole.pin[1], 'holed');
    }
    if (vh > 1e-6) {
      dirx = vx / vh;
      dirz = vz / vh;
    }
    const e = turf.e0 / (1 + vn / turf.ve);
    const slip = vh + R_BALL * omega;
    const jRoll = slip / 3.5;
    const jMax = turf.mu * (1 + e) * vn;
    const rolls = Math.abs(jRoll) <= jMax;
    const j = rolls ? jRoll : Math.sign(jRoll) * jMax;
    const k = 1 - (1 - turf.damp) * Math.min(1, vn / 12);
    const vhNew = (vh - j) * k;
    omega = rolls ? -vhNew / R_BALL : omega - (2.5 / R_BALL) * j;
    const vyNew = e * vn;
    vx = dirx * vhNew;
    vz = dirz * vhNew;
    if (vyNew < 0.45) break;
    // 다음 착지까지 포물선 (바운드는 낮고 느려 공기 힘을 뺀다)
    const hop = (2 * vyNew) / G;
    for (let s = SAMPLE_DT; s < hop; s += SAMPLE_DT) {
      tr.samples.push({ t: tr.t + s, x: x + vx * s, y: vyNew * s - 0.5 * G * s * s, z: z + vz * s });
    }
    x += vx * hop;
    z += vz * hop;
    tr.t += hop;
    tr.hits.push({ t: tr.t, vn: vyNew });
    vy = -vyNew;
    tr.samples.push({ t: tr.t, x, y: 0, z });
  }
  return roll(tr, x, z, vx, vz);
}

/** 퍼트 세기 범위: 파워 1 이 굴리는 평평한 그린 거리 = 핀까지의 1.6 배 (4–24 m). 짧은 퍼트도 미터를 넉넉히 쓴다 */
export function puttMaxFor(distToPinM: number): number {
  return Math.min(Math.max(1.6 * distToPinM, 4), PUTT_MAX_M);
}

/** 퍼트: 파워 → 평평한 그린 기준 거리 → 첫 속도 */
export function puttSpeed(power: number, maxM = PUTT_MAX_M): number {
  const d = Math.max(power, 0) * maxM;
  return Math.sqrt(2 * TURF.green.roll * G * d);
}

/** 평평한 그린에서 거리 d (m) 를 굴리는 파워 */
export function puttPowerFor(distanceM: number, maxM = PUTT_MAX_M): number {
  return distanceM / maxM;
}

/** 퍼트. 조준 방향 yaw (rad), 파워 1 = 평평한 그린에서 maxM */
export function putt(hole: Hole, x: number, z: number, yaw: number, power: number, maxM = PUTT_MAX_M, wind?: GroundWind): GroundResult {
  const v = puttSpeed(power, maxM);
  const tr: Track = { hole, samples: [{ t: 0, x, y: 0, z }], x0: x, z0: z, t: 0, bounces: 0, firstSurface: null, hits: [], rollStart: 0, wind: wind ?? null };
  return roll(tr, x, z, v * Math.cos(yaw), v * Math.sin(yaw));
}

/**
 * 굴림 구간의 (시각, 속력) 점들 (효과음 세기 곡선용). 시각은 착지 = 0 이고 최대 maxPoints 개로 솎는다.
 * 바운드로 뜬 구간(y > 0)은 빼고, 굴림이 시작된 뒤의 표본 간격으로 속력을 구한다.
 */
export function rollProfile(g: GroundResult, maxPoints = 16): { t: number; v: number }[] {
  const pts: { t: number; v: number }[] = [];
  const s = g.samples;
  for (let i = 1; i < s.length; i++) {
    const a = s[i - 1]!;
    const b = s[i]!;
    if (b.t < g.rollStart || a.y > 1e-6 || b.y > 1e-6 || b.y < -1e-6) continue;
    const dt = b.t - a.t;
    if (dt > 1e-6) pts.push({ t: a.t, v: Math.hypot(b.x - a.x, b.z - a.z) / dt });
  }
  if (pts.length <= maxPoints) return pts;
  const stride = (pts.length - 1) / (maxPoints - 1);
  return Array.from({ length: maxPoints }, (_, k) => pts[Math.round(k * stride)]!);
}
