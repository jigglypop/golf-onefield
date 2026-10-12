/**
 * 지형 높이장: 플레이 구역은 평지(공 물리가 평지를 가정), 밖은 올라가는 언덕·산, 호수는 파여 있고 섬은 그대로.
 * 같은 함수가 높이·숲 밀도·색을 정하므로 이 성질이 깨지면 나무가 공중에 뜨거나 공이 땅속에 놓인다.
 */
import { describe, expect, it } from 'vitest';
import { Color } from 'three';

import { HOLES, inPolygon, LONG_DRIVE } from '../src/game/course.ts';
import { fbm, ridged, smooth01, vnoise } from '../src/photo/noise.ts';
import { buildTerrainLevel, LAKE_DEPTH, makeTerrainField, terrainLevels } from '../src/photo/terrain.ts';

describe('잡음', () => {
  it('0–1 안, 같은 입력은 같은 값, 이웃과 연속', () => {
    for (let i = 0; i < 400; i++) {
      const x = (i * 0.37) % 53;
      const y = (i * 0.91) % 47;
      for (const v of [vnoise(x, y, 3), fbm(x, y, 4, 9), ridged(x, y, 4, 5)]) {
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(1);
      }
      expect(fbm(x, y, 4, 9)).toBe(fbm(x, y, 4, 9));
      expect(Math.abs(vnoise(x + 1e-3, y, 3) - vnoise(x, y, 3))).toBeLessThan(0.02);
    }
    expect(smooth01(-1)).toBe(0);
    expect(smooth01(2)).toBe(1);
  });
});

describe('지형 높이장', () => {
  for (const hole of [...HOLES, LONG_DRIVE]) {
    const f = makeTerrainField(hole);
    const xs = hole.bounds.map((p) => p[0]);
    const zs = hole.bounds.map((p) => p[1]);
    const [x0, x1, z0, z1] = [Math.min(...xs), Math.max(...xs), Math.min(...zs), Math.max(...zs)];
    const inLake = (x: number, z: number) => hole.water.some((w) => inPolygon(w, x, z));

    it(`${hole.number}번 홀: 플레이 구역은 (호수 밖에서) 평지 y = 0, 숲 없음`, () => {
      let n = 0;
      for (let i = 0; i < 60; i++) {
        for (let j = 0; j < 60; j++) {
          const x = x0 + ((i + 0.5) / 60) * (x1 - x0);
          const z = z0 + ((j + 0.5) / 60) * (z1 - z0);
          if (!inPolygon(hole.bounds, x, z) || inLake(x, z)) continue;
          n++;
          expect(f.distOutside(x, z)).toBe(0);
          expect(f.heightAt(x, z)).toBe(0);
          expect(f.forest(x, z, 0, 0, 1)).toBe(0);
        }
      }
      expect(n).toBeGreaterThan(200);
    });

    it(`${hole.number}번 홀: 밖은 0 이상이고 멀수록 높다 (언덕 → 산)`, () => {
      const ring = (d0: number, d1: number) => {
        let sum = 0;
        let n = 0;
        let min = Infinity;
        for (let k = 0; k < 720; k++) {
          const a = (k / 720) * Math.PI * 2;
          const r = d0 + ((k * 0.618) % 1) * (d1 - d0);
          const cx = (x0 + x1) / 2;
          const cz = (z0 + z1) / 2;
          const reach = Math.hypot(x1 - x0, z1 - z0) / 2;
          const x = cx + Math.cos(a) * (reach + r);
          const z = cz + Math.sin(a) * (reach + r);
          if (inPolygon(hole.bounds, x, z)) continue;
          const h = f.heightAt(x, z);
          sum += h;
          n++;
          min = Math.min(min, h);
        }
        return { mean: sum / n, min };
      };
      const near = ring(60, 100);
      const mid = ring(400, 600);
      const far = ring(2500, 3500);
      expect(near.min).toBeGreaterThanOrEqual(0);
      expect(mid.mean).toBeGreaterThan(near.mean);
      expect(far.mean).toBeGreaterThan(mid.mean);
      expect(far.mean).toBeLessThan(1100);
    });
  }

  it('호수는 파여 있고(−1.7 m) 섬 땅은 그대로(0)', () => {
    const lakeHole = HOLES[1]!;
    const f = makeTerrainField(lakeHole);
    const lake = lakeHole.water[0]!;
    let deepest = 0;
    for (let i = 0; i < 40; i++) {
      for (let j = 0; j < 40; j++) {
        const xs = lake.map((p) => p[0]);
        const zs = lake.map((p) => p[1]);
        const x = Math.min(...xs) + ((i + 0.5) / 40) * (Math.max(...xs) - Math.min(...xs));
        const z = Math.min(...zs) + ((j + 0.5) / 40) * (Math.max(...zs) - Math.min(...zs));
        if (inPolygon(lake, x, z)) deepest = Math.min(deepest, f.heightAt(x, z));
      }
    }
    expect(deepest).toBeLessThan(-LAKE_DEPTH * 0.9);
    expect(deepest).toBeGreaterThanOrEqual(-LAKE_DEPTH - 1e-9);

    const island = HOLES[3]!;
    const fi = makeTerrainField(island);
    const land = island.land![0]!;
    const cx = land.reduce((a, p) => a + p[0], 0) / land.length;
    const cz = land.reduce((a, p) => a + p[1], 0) / land.length;
    expect(inPolygon(land, cx, cz)).toBe(true);
    expect(fi.heightAt(cx, cz)).toBe(0);
  });

  it('고리 지오메트리: 유한한 정점, 단위 법선, 색은 0–1', () => {
    const f = makeTerrainField(HOLES[0]!);
    const lv = terrainLevels(f);
    expect(lv.length).toBe(3);
    // 안쪽 고리만 (가장 촘촘): 속도를 위해 작게 다시 만든다
    const g = buildTerrainLevel(f, { ...lv[0]!, n: 48 }, 3);
    const pos = g.getAttribute('position');
    const nor = g.getAttribute('normal');
    const col = g.getAttribute('color');
    expect(pos.count).toBeGreaterThan(1000);
    let maxY = -Infinity;
    for (let i = 0; i < pos.count; i++) {
      for (const a of [pos, nor, col]) {
        expect(Number.isFinite(a.getX(i)) && Number.isFinite(a.getY(i)) && Number.isFinite(a.getZ(i))).toBe(true);
      }
      expect(Math.hypot(nor.getX(i), nor.getY(i), nor.getZ(i))).toBeCloseTo(1, 4);
      maxY = Math.max(maxY, pos.getY(i));
    }
    expect(maxY).toBeGreaterThan(5);
    const c = new Color();
    f.color(0, 0, 0, 0, 1, c);
    expect(c.r).toBeGreaterThanOrEqual(0);
    expect(c.g).toBeLessThanOrEqual(1);
    const idx = g.getIndex()!;
    for (let i = 0; i < idx.count; i++) expect(idx.getX(i)).toBeLessThan(pos.count);
  });

  it('같은 홀은 같은 지형 (결정적)', () => {
    const a = makeTerrainField(HOLES[4]!);
    const b = makeTerrainField(HOLES[4]!);
    for (const [x, z] of [[100, 400], [-300, 20], [900, -700], [2500, 1800]] as const) expect(a.heightAt(x, z)).toBe(b.heightAt(x, z));
  });
});
