/**
 * 장애물: 나무 원기둥. 땅(y = 0)은 그대로 평평하고, 날아가는 공만 막는다.
 *
 * 규칙: 비행 중인 공의 중심이 나무 원기둥 안(수평 거리 < r)이고 높이가 나무 꼭대기(h)보다 낮으면 공이 거기서 멈춘다
 * (가지에 맞아 힘을 잃고 수직으로 떨어진다). 처음부터 원기둥 안에 놓인 공은 그 나무를 한 번 벗어나기 전까지 맞지 않는다.
 * 비행 표본은 시간 에르미트(positionAt)로 `STEP_S` 간격으로 훑고, 닿은 걸음 안에서 이분법으로 닿는 순간을 좁힌다.
 *
 * 맞은 뒤의 낙하: 수평 속도 0, 수직 속도 0 에서 자유 낙하 ($y(t) = y_0 - \tfrac12 g t^2$) 하는 표본을 비행 표본 뒤에 이어 붙인다.
 */
import { positionAt, type TrajectorySample } from '../engine/flight.ts';
import { G } from '../engine/units.ts';
import type { Tree } from './course.ts';

/** 훑는 시간 간격 (s). 공 속도 70 m/s 에서 약 0.3 m 라 반지름 1 m 이상 나무는 놓치지 않는다 */
export const STEP_S = 1 / 240;
/** 낙하 표본 간격 (s) */
const FALL_DT = 1 / 60;

export interface TreeHit {
  /** 맞은 나무 (trees 안 순번) */
  index: number;
  tree: Tree;
  /** 닿는 순간 (비행 시각 s)과 공 중심 위치 (월드 m) */
  t: number;
  x: number;
  y: number;
  z: number;
}

/**
 * 비행 표본 (월드 m, 실시간 s) 이 나무에 처음 닿는 곳. 없으면 null.
 * 착지 표본(y = 0) 근처의 땅 위 구간은 y = 0 이라 나무 높이 안이므로 원기둥 안에 떨어지는 공도 맞은 것으로 센다.
 */
export function firstTreeHit(trees: readonly Tree[] | undefined, samples: readonly TrajectorySample[]): TreeHit | null {
  if (!trees || trees.length === 0 || samples.length < 2) return null;
  const end = samples[samples.length - 1]!.t;
  const p = { x: 0, y: 0, z: 0 };
  const q = { x: 0, y: 0, z: 0 };
  positionAt(samples, 0, p);
  // 시작 위치가 이미 원기둥 안인 나무는, 한 번 밖으로 나가기 전까지는 무장하지 않는다
  const armed = trees.map((tr) => Math.hypot(p.x - tr.x, p.z - tr.z) >= tr.r);
  const inside = (tr: Tree, o: { x: number; y: number; z: number }) => Math.hypot(o.x - tr.x, o.z - tr.z) < tr.r && o.y < tr.h;
  let hint = 0;
  let tPrev = 0;
  for (let t = STEP_S; ; t += STEP_S) {
    const tc = Math.min(t, end);
    hint = positionAt(samples, tc, q, hint);
    for (let i = 0; i < trees.length; i++) {
      const tr = trees[i]!;
      if (!armed[i]) {
        if (Math.hypot(q.x - tr.x, q.z - tr.z) >= tr.r) armed[i] = true;
        continue;
      }
      if (!inside(tr, q)) continue;
      // [tPrev, tc] 안에서 처음 안으로 들어온 순간을 이분법으로 좁힌다
      let lo = tPrev;
      let hi = tc;
      const m = { x: 0, y: 0, z: 0 };
      for (let it = 0; it < 24; it++) {
        const mid = 0.5 * (lo + hi);
        positionAt(samples, mid, m);
        if (inside(tr, m)) hi = mid;
        else lo = mid;
      }
      positionAt(samples, hi, m);
      return { index: i, tree: tr, t: hi, x: m.x, y: m.y, z: m.z };
    }
    p.x = q.x;
    p.y = q.y;
    p.z = q.z;
    tPrev = tc;
    if (tc >= end) return null;
  }
}

/**
 * 비행 표본을 hit 시각에서 자르고, 그 자리에서 수직으로 떨어지는 표본을 이어 붙인다.
 * 돌려주는 표본의 끝은 y = 0 (속도 −g·t_fall) 이다.
 */
export function cutAtTree(samples: readonly TrajectorySample[], hit: TreeHit): TrajectorySample[] {
  const kept = samples.filter((s) => s.t < hit.t - 1e-9);
  const out: TrajectorySample[] = kept.slice();
  const fall = Math.sqrt((2 * Math.max(hit.y, 0)) / G);
  const nSteps = Math.max(1, Math.ceil(fall / FALL_DT));
  for (let k = 0; k <= nSteps; k++) {
    const tau = k === nSteps ? fall : k * FALL_DT;
    out.push({ t: hit.t + tau, x: hit.x, y: k === nSteps ? 0 : Math.max(hit.y - 0.5 * G * tau * tau, 0), z: hit.z, vx: 0, vy: -G * tau, vz: 0 });
  }
  return out;
}
