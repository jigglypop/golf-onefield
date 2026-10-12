/**
 * 지형 3단 고리 (플레이 구역은 평지, 밖은 언덕·산). 정점 색이 풀·숲·바위를 칠하고, 흑백 잔디 질감과 법선 지도가 가까운 곳의 결을 준다.
 * 표준 재질만 써서 WebGPU·WebGL 둘 다 같다.
 *
 * 메모리: 홀 하나의 지형 지오메트리는 약 12 MB (정점 18 만 개)라서 홀마다 쌓아 두면 9홀에 100 MB 를 넘는다.
 * 높이장(`fieldOf`)은 계산이 0.2 ms 라 따로 두고, 지오메트리는 장면에 올라 있는 동안만(참조 수) + 직전 홀 하나만 캐시에 남긴다.
 * 나머지는 `dispose()` 로 GPU 버퍼를 돌려준다 (홀이 바뀔 때, 장면이 내려갈 때).
 */
import { useEffect, useMemo } from 'react';

import { Vector2, type BufferGeometry } from 'three';

import type { Hole } from '../game/course.ts';
import { terrainDetail } from './surfaces.ts';
import { buildTerrainLevel, makeTerrainField, terrainLevels, type TerrainField } from './terrain.ts';

/** 흑백 질감이 덮는 길이 (m) */
const UV_TILE = 3;

/** 지오메트리 캐시에 남겨 두는 "지금 쓰지 않는" 홀 수 (직전 홀로 돌아가거나 연습장 ↔ 코스를 오갈 때 다시 짓지 않게) */
const KEEP_IDLE = 1;

const fields = new WeakMap<Hole, TerrainField>();

/** 홀 하나의 지형 높이장 (가볍다: 같은 홀 객체면 다시 만들지 않는다) */
export function fieldOf(hole: Hole): TerrainField {
  let f = fields.get(hole);
  if (!f) {
    f = makeTerrainField(hole);
    fields.set(hole, f);
  }
  return f;
}

interface GeoEntry {
  geometries: BufferGeometry[];
  /** 장면에 올라 있는 `Terrain` 수 */
  refs: number;
  /** 마지막으로 쓴 순번 (작을수록 오래됨) */
  used: number;
}

const geoCache = new Map<Hole, GeoEntry>();
let clock = 0;

function dispose(e: GeoEntry): void {
  for (const g of e.geometries) g.dispose();
  e.geometries = [];
}

/** 홀 하나의 지형 지오메트리 (없으면 만든다. 올리고 내리는 것은 `retainTerrain` / `releaseTerrain`) */
export function terrainGeometries(hole: Hole): BufferGeometry[] {
  let e = geoCache.get(hole);
  if (!e) {
    performance.mark('terrain-start');
    const field = fieldOf(hole);
    e = { geometries: terrainLevels(field).map((lv) => buildTerrainLevel(field, lv, UV_TILE)), refs: 0, used: ++clock };
    performance.measure(`terrain build (hole ${hole.number})`, 'terrain-start');
    geoCache.set(hole, e);
  }
  return e.geometries;
}

/** 지금 쓰지 않는 항목 가운데 가장 최근 `KEEP_IDLE` 개만 남기고 놓는다 */
function trim(): void {
  const idle = [...geoCache.entries()].filter(([, e]) => e.refs === 0).sort((a, b) => b[1].used - a[1].used);
  for (const [h, e] of idle.slice(KEEP_IDLE)) {
    dispose(e);
    geoCache.delete(h);
  }
}

/** `Terrain` 이 올라갈 때 (효과 안에서 부른다: 같은 커밋의 앞선 내림이 먼저 끝난 뒤라서 직전 홀이 정리된다) */
export function retainTerrain(hole: Hole): void {
  terrainGeometries(hole);
  const e = geoCache.get(hole)!;
  e.refs++;
  e.used = ++clock;
  trim();
}

/** `Terrain` 이 내려갈 때 */
export function releaseTerrain(hole: Hole): void {
  const e = geoCache.get(hole);
  if (!e) return;
  e.refs = Math.max(0, e.refs - 1);
  e.used = ++clock;
}

/** 캐시를 전부 놓는다 (월드를 통째로 내릴 때·시험용). 올라 있는 지형이 있으면 다음 그리기에서 GPU 에 다시 올라간다 */
export function disposeTerrainCache(): void {
  for (const e of geoCache.values()) dispose(e);
  geoCache.clear();
}

/** 점검용: 캐시에 있는 홀 수와 지오메트리 수 */
export function terrainCacheInfo(): { holes: number; geometries: number } {
  let geometries = 0;
  for (const e of geoCache.values()) geometries += e.geometries.length;
  return { holes: geoCache.size, geometries };
}

const NORMAL_SCALE = new Vector2(1.0, 1.0);

export function Terrain({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => terrainGeometries(hole), [hole]);
  const detail = useMemo(() => terrainDetail(), []);
  useEffect(() => {
    retainTerrain(hole);
    return () => releaseTerrain(hole);
  }, [hole]);
  return (
    <group name="terrain">
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} receiveShadow frustumCulled={i > 0}>
          <meshStandardMaterial map={detail.map} normalMap={detail.normalMap} normalScale={NORMAL_SCALE} vertexColors roughness={0.96} metalness={0} />
        </mesh>
      ))}
    </group>
  );
}
