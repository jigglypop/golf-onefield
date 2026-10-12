/**
 * 숲 배치 계획 (React·three 렌더러 없이 계산만): 지형 높이장 위에 거리 구간별로 나무를 심고, 공간 칸(cell)으로 나눈다.
 *   가까운 구간 (플레이 구역 밖 3–220 m): 6.5 m 간격, 층진 수관 + 줄기, 그림자를 드리운다
 *   중간 (220–800 m): 16 m 간격, 낮은 폴리, 그림자 없음
 *   먼 (800–2800 m): 46 m 간격, 아주 단순, 능선에 톱니 윤곽을 준다
 * 심는 위치·밀도는 지형의 `forest()` 와 같은 함수라서 땅 색(숲 바닥)과 나무가 어긋나지 않는다.
 * 칸 나누기는 배치를 바꾸지 않는다: 같은 시드면 나무 위치·키·색은 칸 없이 심던 때와 한 그루도 다르지 않다.
 */
import { rngOf } from './noise.ts';
import type { TerrainField } from './terrain.ts';

export interface Spot {
  x: number;
  y: number;
  z: number;
  /** 전체 키 (m) */
  h: number;
  /** 수관 반지름 (m) */
  r: number;
  tint: number;
  yaw: number;
  broad: boolean;
}

export type ZoneName = 'near' | 'mid' | 'far';

export interface Zone {
  name: ZoneName;
  d0: number;
  d1: number;
  /** 격자 간격 (m) */
  step: number;
  /** 키 배율 (먼 나무는 보이게 조금 크게) */
  hScale: number;
  /** 공간 칸 한 변 (m): 시야 밖 칸의 나무는 그리지 않는다 (칸 단위로 판정). 먼 구간은 나무가 작아 칸을 크게 잡는다 */
  cell: number;
}

export const ZONES: readonly Zone[] = [
  { name: 'near', d0: 3, d1: 220, step: 6.5, hScale: 1, cell: 50 },
  { name: 'mid', d0: 220, d1: 800, step: 16, hScale: 1.15, cell: 130 },
  { name: 'far', d0: 800, d1: 2800, step: 46, hScale: 1.5, cell: 450 },
];

/** 수관이 키에서 차지하는 비율 (나머지는 맨 줄기) */
export const CROWN = { pine: 0.8, broad: 0.66 } as const;

/** 구간 하나의 나무 (공간 칸 나누기 전, 격자 순서 그대로) */
export function scatter(field: TerrainField, zone: Zone, seed: number, density: number): Spot[] {
  const rnd = rngOf(seed);
  const out: Spot[] = [];
  const step = zone.step / Math.sqrt(density);
  const reach = Math.max(field.hx, field.hz) + zone.d1;
  for (let gz = field.cz - reach; gz <= field.cz + reach; gz += step) {
    for (let gx = field.cx - reach; gx <= field.cx + reach; gx += step) {
      const x = gx + (rnd() - 0.5) * step * 0.9;
      const z = gz + (rnd() - 0.5) * step * 0.9;
      const d = field.distOutside(x, z);
      if (d < zone.d0 || d >= zone.d1) continue;
      const h = field.heightAt(x, z);
      const e = 1.5;
      const dhx = (field.heightAt(x + e, z) - field.heightAt(x - e, z)) / (2 * e);
      const dhz = (field.heightAt(x, z + e) - field.heightAt(x, z - e)) / (2 * e);
      const ny = 1 / Math.hypot(dhx, 1, dhz);
      if (rnd() > field.forest(x, z, d, h, ny)) continue;
      const alt = Math.min(1, h / 420);
      const broad = rnd() < 0.3 * (1 - Math.min(1, h / 200)) * (d < 800 ? 1 : 0.5);
      const size = (broad ? 8 + rnd() * 6 : 9 + rnd() * 10) * (1 - 0.35 * alt) * zone.hScale;
      const r = broad ? size * (0.36 + rnd() * 0.1) : size * (0.17 + rnd() * 0.07);
      out.push({ x, y: h, z, h: size, r, tint: rnd(), yaw: rnd() * Math.PI * 2, broad });
    }
  }
  return out;
}

/** 공간 칸 하나: 소나무·활엽수 나무 목록과 중심(칸 격자의 중심, 나무 무게중심이 아님) */
export interface ForestCell {
  zone: ZoneName;
  ix: number;
  iz: number;
  /** 칸 중심 (m). LOD 거리 판정에 쓴다 */
  cx: number;
  cz: number;
  pine: Spot[];
  broad: Spot[];
}

/** 나무들을 구간의 칸 격자(원점 기준)로 나눈다. 칸 순서는 (iz, ix) 오름차순이라 결정적이다 */
export function binCells(spots: readonly Spot[], zone: Zone): ForestCell[] {
  const map = new Map<string, ForestCell>();
  for (const p of spots) {
    const ix = Math.floor(p.x / zone.cell);
    const iz = Math.floor(p.z / zone.cell);
    const key = `${ix},${iz}`;
    let c = map.get(key);
    if (!c) {
      c = { zone: zone.name, ix, iz, cx: (ix + 0.5) * zone.cell, cz: (iz + 0.5) * zone.cell, pine: [], broad: [] };
      map.set(key, c);
    }
    (p.broad ? c.broad : c.pine).push(p);
  }
  return [...map.values()].sort((a, b) => a.iz - b.iz || a.ix - b.ix);
}

export interface ForestPlan {
  zone: Zone;
  /** 칸 나누기 전 구간 전체의 나무 수 (소나무 + 활엽수) */
  total: number;
  cells: ForestCell[];
}

/** 홀 하나의 숲: 구간마다 심고 칸으로 나눈다. 시드는 예전과 같다 (9100 + 홀 번호·17 + 구간 번호·101) */
export function planForest(field: TerrainField, holeNumber: number, density: number): ForestPlan[] {
  return ZONES.map((zone, i) => {
    const spots = scatter(field, zone, 9100 + holeNumber * 17 + i * 101, density);
    return { zone, total: spots.length, cells: binCells(spots, zone) };
  });
}
