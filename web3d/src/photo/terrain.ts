/**
 * 지형: 홀의 플레이 구역(`hole.bounds`)은 y = 0 으로 평평하게 두고(공 물리가 평지를 가정한다), 그 밖은 완만한 언덕에서
 * 능선·산으로 올라가는 높이장. 같은 함수가 높이·색·숲 밀도를 모두 정해서 나무와 땅이 어긋나지 않는다.
 *
 * 높이 H(d), d = 플레이 구역 밖 거리(m):
 *   d ≤ 6 m: 0 (평지).  그 뒤로 잔 기복 + 언덕 포락선 (1 − e^{−(d−6)/170}) · (24..76 m) + 산맥 (d 600 m 부터 능선 잡음, 최대 약 900 m).
 * 정점은 3단 고리(가까이 촘촘, 멀리 성김)로 깔고, 이음매는 치마(skirt)로 가린다.
 */
import { BufferAttribute, BufferGeometry, Color } from 'three';

import { inPolygon, type Hole, type Vec2 } from '../game/course.ts';
import { fbm, lerp, ridged, smooth01 } from './noise.ts';

function segDist(ax: number, az: number, bx: number, bz: number, px: number, pz: number): number {
  const dx = bx - ax;
  const dz = bz - az;
  const l2 = dx * dx + dz * dz;
  const t = l2 > 0 ? Math.min(Math.max(((px - ax) * dx + (pz - az) * dz) / l2, 0), 1) : 0;
  return Math.hypot(px - (ax + t * dx), pz - (az + t * dz));
}

export interface TerrainField {
  /** 플레이 구역 외곽 상자의 중심과 반 크기 */
  cx: number;
  cz: number;
  hx: number;
  hz: number;
  /** 플레이 구역 밖 거리 (안은 0) */
  distOutside(x: number, z: number): number;
  heightAt(x: number, z: number): number;
  /** 숲 밀도 0–1 (나무 심기와 땅 색이 같이 쓴다). ny = 법선의 y 성분 */
  forest(x: number, z: number, d: number, h: number, ny: number): number;
  /** 정점 색 (선형 공간) */
  color(x: number, z: number, d: number, h: number, ny: number, out: Color): Color;
}

/** 호수 깊이 (m)와 둑 경사 폭 (m), 물 높이는 0 보다 조금 아래 */
export const LAKE_DEPTH = 1.7;
export const LAKE_BANK = 5.5;
export const WATER_Y = -0.14;

const C = {
  meadowDark: new Color('#4f7a2f'),
  meadowLight: new Color('#7fa646'),
  dry: new Color('#a49a52'),
  forestDark: new Color('#22401f'),
  forestLight: new Color('#3b5e2a'),
  rock: new Color('#7d7a70'),
  rockDark: new Color('#5a5750'),
  alpine: new Color('#8a8e76'),
  mud: new Color('#5b5a44'),
};

export function makeTerrainField(hole: Hole): TerrainField {
  const poly: readonly Vec2[] = hole.bounds;
  const xs = poly.map((p) => p[0]);
  const zs = poly.map((p) => p[1]);
  const cx = (Math.min(...xs) + Math.max(...xs)) / 2;
  const cz = (Math.min(...zs) + Math.max(...zs)) / 2;
  const hx = (Math.max(...xs) - Math.min(...xs)) / 2;
  const hz = (Math.max(...zs) - Math.min(...zs)) / 2;

  const distOutside = (x: number, z: number): number => {
    if (inPolygon(poly, x, z)) return 0;
    let d = Infinity;
    for (let i = 0; i < poly.length; i++) {
      const a = poly[i]!;
      const b = poly[(i + 1) % poly.length]!;
      d = Math.min(d, segDist(a[0], a[1], b[0], b[1], x, z));
    }
    return d;
  };

  // 호수: 물 다각형 안은 가장자리에서 안쪽으로 5.5 m 에 걸쳐 1.7 m 파인다. 섬(land 다각형)은 그대로 두고 섬 둘레만 경사진다.
  const lakes: readonly (readonly Vec2[])[] = hole.water;
  const lands: readonly (readonly Vec2[])[] = hole.land ?? [];
  const edgeDist = (poly: readonly Vec2[], x: number, z: number): number => {
    let d = Infinity;
    for (let i = 0; i < poly.length; i++) {
      const a = poly[i]!;
      const b = poly[(i + 1) % poly.length]!;
      d = Math.min(d, segDist(a[0], a[1], b[0], b[1], x, z));
    }
    return d;
  };
  const carve = (x: number, z: number): number => {
    let depth = 0;
    for (const lake of lakes) {
      if (!inPolygon(lake, x, z)) continue;
      let d = edgeDist(lake, x, z);
      for (const land of lands) {
        if (inPolygon(land, x, z)) {
          d = 0;
          break;
        }
        d = Math.min(d, edgeDist(land, x, z));
      }
      depth = Math.max(depth, LAKE_DEPTH * smooth01(d / LAKE_BANK));
    }
    return depth;
  };

  const heightAt = (x: number, z: number): number => {
    if (lakes.length > 0) {
      const c = carve(x, z);
      if (c > 0) return -c;
    }
    const d = distOutside(x, z);
    const t = d - 6;
    if (t <= 0) return 0;
    const s1 = smooth01(t / 70);
    // 잔 기복: 양수만 (아래로 파지 않는다)
    const bump = s1 * (fbm(x / 58, z / 58, 3, 11) * 5.2 + fbm(x / 17, z / 17, 2, 13) * 1.1);
    const env = 1 - Math.exp(-t / 170);
    const s2 = smooth01(t / 130);
    const hills = env * s2 * (22 + 54 * fbm(x / 390 + 3, z / 390 - 7, 4, 5)) * (0.45 + 0.55 * fbm(x / 950, z / 950, 3, 9));
    const mt = smooth01((d - 550) / 2300);
    const mount = mt * (110 + 800 * Math.pow(ridged(x / 3300 + 9, z / 3300 - 4, 5, 21), 1.3));
    return bump + hills + mount;
  };

  const forest = (x: number, z: number, d: number, h: number, ny: number): number => {
    const patch = fbm(x / 310 + 4.2, z / 310 - 1.7, 3, 51);
    let m = smooth01((patch - 0.3) / 0.22);
    // 플레이 구역 바로 바깥은 끊김 없는 숲 띠
    m = Math.max(m, 0.9 * (1 - smooth01((d - 40) / 60)));
    const edge = smooth01((d - 7) / 22);
    const treeline = 1 - smooth01((h - 360) / 150);
    const steep = 1 - smooth01((0.74 - ny) / 0.16);
    return m * edge * treeline * steep;
  };

  const color = (x: number, z: number, d: number, h: number, ny: number, out: Color): Color => {
    const n1 = fbm(x / 72, z / 72, 3, 41);
    const n2 = fbm(x / 9, z / 9, 2, 43);
    const dry = smooth01((fbm(x / 230 - 8, z / 230 + 3, 3, 47) - 0.55) / 0.2) * (1 - smooth01((d - 20) / 200));
    out.copy(C.meadowDark).lerp(C.meadowLight, n1 * 0.85 + n2 * 0.15).lerp(C.dry, dry * 0.5);
    const f = forest(x, z, d, h, ny);
    const forestTone = C.forestDark.clone().lerp(C.forestLight, fbm(x / 40, z / 40, 2, 53));
    out.lerp(forestTone, Math.min(1, f * 1.15));
    // 암반: 가파르거나 높다. 높이에 따라 회녹색 → 바위색
    const alp = smooth01((h - 380) / 260);
    const rockMask = Math.max(smooth01((0.84 - ny) / 0.2), alp * 0.85);
    const rock = C.rock.clone().lerp(C.rockDark, n2).lerp(C.alpine, alp * 0.55);
    out.lerp(rock, Math.min(1, rockMask));
    if (h < 0.05) {
      // 호수 둑·바닥: 물 높이 근처는 젖은 진흙, 깊이 들어가면 바닥 색
      const wet = smooth01((0.05 - h) / 0.5);
      out.lerp(C.mud, wet * 0.85);
    }
    // 아주 멀리는 살짝 푸르스름한 숲 느낌 (대기 원근은 안개가 맡는다)
    void lerp;
    return out;
  };

  return { cx, cz, hx, hz, distOutside, heightAt, forest, color };
}

export interface TerrainLevel {
  half: number;
  n: number;
  /** 안쪽 고리가 덮는 반 길이 (이 안의 칸은 뺀다). 0 이면 빼지 않는다 */
  inner: number;
}

/** 플레이 구역 크기에 맞춘 3단 고리 */
export function terrainLevels(f: TerrainField): TerrainLevel[] {
  const half0 = Math.max(f.hx, f.hz) + 230;
  return [
    { half: half0, n: 220, inner: 0 },
    { half: 3000, n: 256, inner: half0 },
    { half: 15000, n: 256, inner: 3000 },
  ];
}

/** 고리 하나의 지오메트리 (위치·법선·정점 색·uv = 월드 m / uvTile) */
export function buildTerrainLevel(f: TerrainField, lv: TerrainLevel, uvTile: number): BufferGeometry {
  const { half, n, inner } = lv;
  const cell = (2 * half) / n;
  const nv = n + 1;
  const x0 = f.cx - half;
  const z0 = f.cz - half;
  const hs = new Float32Array(nv * nv);
  const ds = new Float32Array(nv * nv);
  for (let j = 0; j < nv; j++) {
    for (let i = 0; i < nv; i++) {
      const x = x0 + i * cell;
      const z = z0 + j * cell;
      ds[j * nv + i] = f.distOutside(x, z);
      hs[j * nv + i] = f.heightAt(x, z);
    }
  }
  const at = (i: number, j: number) => hs[Math.min(nv - 1, Math.max(0, j)) * nv + Math.min(nv - 1, Math.max(0, i))]!;
  // 안쪽 고리가 덮는 칸은 뺀다
  const skip = (qi: number, qj: number) => {
    if (inner <= 0) return false;
    const mx = x0 + (qi + 0.5) * cell - f.cx;
    const mz = z0 + (qj + 0.5) * cell - f.cz;
    return Math.abs(mx) < inner - cell * 0.5 && Math.abs(mz) < inner - cell * 0.5;
  };
  const included = new Uint8Array(n * n);
  for (let qj = 0; qj < n; qj++) for (let qi = 0; qi < n; qi++) included[qj * n + qi] = skip(qi, qj) ? 0 : 1;
  const inc = (qi: number, qj: number) => qi >= 0 && qj >= 0 && qi < n && qj < n && included[qj * n + qi] === 1;

  const pos: number[] = [];
  const nor: number[] = [];
  const col: number[] = [];
  const uv: number[] = [];
  const idx: number[] = [];
  const tmp = new Color();
  const vmap = new Int32Array(nv * nv).fill(-1);
  const vertex = (i: number, j: number): number => {
    const k = j * nv + i;
    const hit = vmap[k]!;
    if (hit >= 0) return hit;
    const x = x0 + i * cell;
    const z = z0 + j * cell;
    const h = hs[k]!;
    const dhx = (at(i + 1, j) - at(i - 1, j)) / (2 * cell);
    const dhz = (at(i, j + 1) - at(i, j - 1)) / (2 * cell);
    const l = Math.hypot(dhx, 1, dhz);
    const ny = 1 / l;
    f.color(x, z, ds[k]!, h, ny, tmp);
    const v = pos.length / 3;
    pos.push(x, h, z);
    nor.push(-dhx / l, ny, -dhz / l);
    col.push(tmp.r, tmp.g, tmp.b);
    uv.push(x / uvTile, z / uvTile);
    vmap[k] = v;
    return v;
  };
  const skirt = (a: number, b: number, drop: number) => {
    // 모서리 (a, b) 아래로 drop m 떨어지는 치마. 위 정점의 법선·색을 그대로 쓴다
    const mk = (src: number) => {
      const v = pos.length / 3;
      pos.push(pos[3 * src]!, pos[3 * src + 1]! - drop, pos[3 * src + 2]!);
      nor.push(nor[3 * src]!, nor[3 * src + 1]!, nor[3 * src + 2]!);
      col.push(col[3 * src]!, col[3 * src + 1]!, col[3 * src + 2]!);
      uv.push(uv[2 * src]!, uv[2 * src + 1]!);
      return v;
    };
    const a2 = mk(a);
    const b2 = mk(b);
    idx.push(a, b, b2, a, b2, a2, a, b2, b, a, a2, b2); // 양면 (안쪽에서도 보이게)
  };
  const drop = Math.max(40, cell * 2.5);
  for (let qj = 0; qj < n; qj++) {
    for (let qi = 0; qi < n; qi++) {
      if (!inc(qi, qj)) continue;
      const a = vertex(qi, qj);
      const b = vertex(qi + 1, qj);
      const c = vertex(qi, qj + 1);
      const d = vertex(qi + 1, qj + 1);
      idx.push(a, c, b, b, c, d);
      if (!inc(qi, qj - 1)) skirt(a, b, drop);
      if (!inc(qi, qj + 1)) skirt(c, d, drop);
      if (!inc(qi - 1, qj)) skirt(a, c, drop);
      if (!inc(qi + 1, qj)) skirt(b, d, drop);
    }
  }
  const g = new BufferGeometry();
  g.setAttribute('position', new BufferAttribute(new Float32Array(pos), 3));
  g.setAttribute('normal', new BufferAttribute(new Float32Array(nor), 3));
  g.setAttribute('color', new BufferAttribute(new Float32Array(col), 3));
  g.setAttribute('uv', new BufferAttribute(new Float32Array(uv), 2));
  g.setIndex(idx);
  g.computeBoundingSphere();
  g.computeBoundingBox();
  return g;
}
