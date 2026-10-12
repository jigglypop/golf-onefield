/**
 * 숲 메시: 나무를 공간 칸(`forestPlan.ts`)으로 나눠 두고, 카메라 시야에 드는 칸의 나무만 모아서 그린다.
 *
 * 예전에는 구간마다 InstancedMesh 둘(소나무·활엽수)을 frustumCulled = false 로 360° 전부, 해 그림자 단계마다 다시 그렸다
 * (한 홀에 나무 2 만 그루, 삼각형 약 1000 만 × 그리기 횟수). 칸마다 InstancedMesh 를 따로 두면(한 칸 하나) 시야 밖 칸은 three 가
 * 건너뛰지만 그리기 호출이 4 배로 늘어난다 (WebGPU 는 호출당 CPU 비용이 커서 오히려 느려진다: 측정은 README 성능 절).
 * 그래서 메시는 (구간 × 종류 × 자세히/거칠게) 마다 하나만 두고, 시야에 드는 칸의 인스턴스 행렬·색을 앞에서부터 이어 붙인 뒤
 * `count` 만 줄인다 (보이는 칸 집합이 바뀔 때만 다시 채운다: 칸 → 메시로 메모리 복사 몇 십 KB).
 *
 *   가까운 구간 칸 (50 m): 칸 중심이 `HI_R` 안이면 자세한 쪽(소나무 384 삼각형, 활엽수 2352), 밖이면 거친 쪽(160 / 648).
 *                          그림자는 이 구간만 드리운다. 해 그림자는 시야 밖 나무도 시야 안에 드리우므로 칸 판정에 `SHADOW_MARGIN` 을 더한다. 줄기는 한 메시.
 *   중간 칸 (130 m)      : 거친 쪽보다 더 낮은 폴리 (128 / 384), 그림자 없음
 *   먼 칸 (450 m)        : 아주 단순 (48 / 144), 그림자 받지도 않는다 (해 그림자 지도 범위 밖)
 * 지오메트리·재질은 홀과 상관없이 모양이 같아 모듈 안에서 한 번만 만들어 모든 홀이 나눠 쓴다 (개수가 일정하다).
 */
import {
  Box3,
  BufferGeometry,
  Color,
  CylinderGeometry,
  DoubleSide,
  Frustum,
  Group,
  InstancedBufferAttribute,
  InstancedMesh,
  Matrix4,
  MeshStandardMaterial,
  Object3D,
  Quaternion,
  Vector2,
  Vector3,
  type Camera,
} from 'three';

import { CROWN, type ForestPlan, type Spot, type ZoneName } from './forestPlan.ts';
import { packedMaps, tiled } from './surfaces.ts';
import { broadleafGeometry, leafMaps, needleMaps, pineGeometry } from './trees.ts';

/** 가까운 구간에서 자세한 쪽을 쓰는 칸 중심 거리 (m)와 되돌림 폭 (경계에서 깜박이지 않게) */
export const HI_R = 80;
const HYST = 10;
/** 그림자를 드리우는 가까운 구간의 칸 판정에 더하는 여유 (m): 낮은 해(고도 27°)에서 키 20 m 나무 그림자 길이 ≈ 2 × 키 */
export const SHADOW_MARGIN = 45;

interface Assets {
  hi: { pine: BufferGeometry; broad: BufferGeometry };
  lo: { pine: BufferGeometry; broad: BufferGeometry };
  mid: { pine: BufferGeometry; broad: BufferGeometry };
  far: { pine: BufferGeometry; broad: BufferGeometry };
  trunk: BufferGeometry;
  pineMat: MeshStandardMaterial;
  broadMat: MeshStandardMaterial;
  barkMat: MeshStandardMaterial;
}

let assets: Assets | null = null;

function assetsOf(): Assets {
  if (assets) return assets;
  const needles = needleMaps();
  const leaves = leafMaps();
  const bark = tiled(packedMaps('pinebark', 1), 1, 1);
  assets = {
    hi: { pine: pineGeometry(5, 8, 12), broad: broadleafGeometry(8, 6, 14) },
    lo: { pine: pineGeometry(6, 5, 8), broad: broadleafGeometry(9, 4, 9) },
    mid: { pine: pineGeometry(6, 4, 8), broad: broadleafGeometry(9, 3, 8) },
    far: { pine: pineGeometry(7, 2, 6), broad: broadleafGeometry(10, 2, 6) },
    trunk: new CylinderGeometry(0.7, 1, 1, 7),
    pineMat: new MeshStandardMaterial({ map: needles.map, normalMap: needles.normalMap, normalScale: new Vector2(1.4, 1.4), roughness: 0.92, side: DoubleSide }),
    broadMat: new MeshStandardMaterial({ map: leaves.map, normalMap: leaves.normalMap, normalScale: new Vector2(1.2, 1.2), roughness: 0.9 }),
    barkMat: new MeshStandardMaterial({ map: bark.map, normalMap: bark.normalMap, roughnessMap: bark.roughnessMap, color: '#9a8a7a', roughness: 1 }),
  };
  return assets;
}

/** 모듈 공용 지오메트리·재질을 비운다 (월드를 통째로 내릴 때·시험용). 다음 호출에서 다시 만든다 */
export function disposeForestAssets(): void {
  if (!assets) return;
  const a = assets;
  for (const g of [a.hi, a.lo, a.mid, a.far]) {
    g.pine.dispose();
    g.broad.dispose();
  }
  a.trunk.dispose();
  a.pineMat.dispose();
  a.broadMat.dispose();
  a.barkMat.dispose();
  assets = null;
}

const PINE_COLORS = [new Color('#d6ecc6'), new Color('#ffffee')] as const;
const BROAD_COLORS = [new Color('#dcf0c0'), new Color('#ffffe6')] as const;

/** 한 칸·한 종류의 나무 인스턴스 (행렬 16 float, 색 3 float 씩) */
interface Batch {
  n: number;
  m: Float32Array;
  c: Float32Array;
}

const SCRATCH = {
  m: new Matrix4(),
  q: new Quaternion(),
  axis: new Vector3(0, 1, 0),
  pos: new Vector3(),
  scale: new Vector3(),
  col: new Color(),
  o: new Object3D(),
};

/** 나무 하나당 수관 행렬(위치·크기)과 색(dark → light 사이) */
function crownBatch(spots: readonly Spot[], dark: Color, light: Color, crown: number): Batch {
  const { m, q, axis, pos, scale, col } = SCRATCH;
  const out: Batch = { n: spots.length, m: new Float32Array(spots.length * 16), c: new Float32Array(spots.length * 3) };
  for (let i = 0; i < spots.length; i++) {
    const p = spots[i]!;
    q.setFromAxisAngle(axis, p.yaw);
    const ch = p.h * crown;
    pos.set(p.x, p.y - 0.25 + p.h - ch / 2, p.z);
    scale.set(p.r, ch, p.r);
    m.compose(pos, q, scale);
    m.toArray(out.m, i * 16);
    col.copy(dark).lerp(light, p.tint);
    out.c[i * 3] = col.r;
    out.c[i * 3 + 1] = col.g;
    out.c[i * 3 + 2] = col.b;
  }
  return out;
}

function trunkBatch(spots: readonly Spot[]): Batch {
  const o = SCRATCH.o;
  const out: Batch = { n: spots.length, m: new Float32Array(spots.length * 16), c: new Float32Array(0) };
  for (let i = 0; i < spots.length; i++) {
    const p = spots[i]!;
    const bare = p.h * (1 - (p.broad ? CROWN.broad : CROWN.pine)) + 1.2;
    const rad = Math.max(0.14, p.r * 0.07);
    o.position.set(p.x, p.y + bare / 2 - 0.3, p.z);
    o.scale.set(rad, bare, rad);
    o.rotation.set(0, p.yaw, 0);
    o.updateMatrix();
    o.matrix.toArray(out.m, i * 16);
  }
  return out;
}

/** 칸: 중심, 시야 판정용 상자, 종류별 인스턴스. 현재 상태 (0 안 보임, 1 보임·자세히, 2 보임·거칠게) */
interface Cell {
  zone: ZoneName;
  cx: number;
  cz: number;
  box: Box3;
  pine: Batch;
  broad: Batch;
  trunk: Batch | null;
  state: 0 | 1 | 2;
}

type Kind = 'pine' | 'broad' | 'trunk';

/** 메시 하나(구간·종류·자세히/거칠게)와 그 메시에 이어 붙이는 칸들 */
class Pool {
  readonly mesh: InstancedMesh;
  constructor(
    geometry: BufferGeometry,
    material: MeshStandardMaterial,
    readonly kind: Kind,
    capacity: number,
    cast: boolean,
    receive: boolean,
  ) {
    this.mesh = new InstancedMesh(geometry, material, Math.max(1, capacity));
    this.mesh.count = 0;
    this.mesh.visible = false;
    // DynamicDrawUsage 는 쓰지 않는다: three WebGPU 는 그 표시가 있으면 매 프레임·매 패스 버퍼를 다시 올린다 (우리는 바뀔 때만 needsUpdate)
    if (kind !== 'trunk') this.mesh.instanceColor = new InstancedBufferAttribute(new Float32Array(Math.max(1, capacity) * 3), 3);
    this.mesh.castShadow = cast;
    this.mesh.receiveShadow = receive;
    this.mesh.frustumCulled = false; // 칸 단위 판정을 우리가 한다 (경계 구가 구간 전체라 three 의 판정은 늘 참)
    this.mesh.matrixAutoUpdate = false;
  }

  /** `cells` 중 `want(cell)` 인 칸의 `kind` 인스턴스를 앞에서부터 이어 붙인다 */
  fill(cells: readonly Cell[], want: (c: Cell) => boolean): void {
    const mm = this.mesh.instanceMatrix.array as Float32Array;
    const cc = this.mesh.instanceColor?.array as Float32Array | undefined;
    let n = 0;
    for (const cell of cells) {
      if (!want(cell)) continue;
      const b = this.kind === 'pine' ? cell.pine : this.kind === 'broad' ? cell.broad : cell.trunk;
      if (!b || b.n === 0) continue;
      mm.set(b.m, n * 16);
      if (cc) cc.set(b.c, n * 3);
      n += b.n;
    }
    this.mesh.count = n;
    this.mesh.visible = n > 0;
    this.mesh.instanceMatrix.needsUpdate = true;
    if (this.mesh.instanceColor) this.mesh.instanceColor.needsUpdate = true;
  }
}

export interface ForestStats {
  /** 칸 수 (구간별) */
  cells: Record<ZoneName, number>;
  /** InstancedMesh 수, 나무 수 */
  meshes: number;
  trees: number;
  /** 지금 그려지는 나무 인스턴스 수 (줄기 제외), 켜진 메시 수 */
  drawnTrees: number;
  drawnMeshes: number;
}

export const DBG = { updates: 0, fills: 0, updateMs: 0, fillMs: 0 }; // TEMP
if (typeof window !== 'undefined') (window as unknown as { __forestDbg: typeof DBG }).__forestDbg = DBG; // TEMP

export class ForestMeshes {
  readonly root = new Group();
  readonly stats: ForestStats = { cells: { near: 0, mid: 0, far: 0 }, meshes: 0, trees: 0, drawnTrees: 0, drawnMeshes: 0 };
  private readonly cells: Cell[] = [];
  private readonly byZone: Record<ZoneName, Cell[]> = { near: [], mid: [], far: [] };
  private readonly pools: { zone: ZoneName; pool: Pool; want: (c: Cell) => boolean }[] = [];
  private readonly frustum = new Frustum();
  private readonly pv = new Matrix4();
  private readonly cam = new Vector3();
  private readonly pad = new Box3();

  constructor(plans: readonly ForestPlan[]) {
    const a = assetsOf();
    this.root.name = 'forest-cells';
    this.root.matrixAutoUpdate = false;
    for (const { zone, cells } of plans) {
      const isNear = zone.name === 'near';
      for (const c of cells) {
        const all = [...c.pine, ...c.broad];
        const box = new Box3();
        const v = new Vector3();
        for (const p of all) {
          box.expandByPoint(v.set(p.x - p.r, p.y - 0.3, p.z - p.r));
          box.expandByPoint(v.set(p.x + p.r, p.y + p.h, p.z + p.r));
        }
        const cell: Cell = {
          zone: zone.name,
          cx: c.cx,
          cz: c.cz,
          box,
          pine: crownBatch(c.pine, PINE_COLORS[0], PINE_COLORS[1], CROWN.pine),
          broad: crownBatch(c.broad, BROAD_COLORS[0], BROAD_COLORS[1], CROWN.broad),
          trunk: isNear ? trunkBatch(all) : null,
          state: 0,
        };
        this.cells.push(cell);
        this.byZone[zone.name].push(cell);
        this.stats.cells[zone.name]++;
        this.stats.trees += all.length;
      }
    }
    const count = (zone: ZoneName, kind: Kind): number => this.byZone[zone].reduce((s, c) => s + (kind === 'pine' ? c.pine.n : kind === 'broad' ? c.broad.n : (c.trunk?.n ?? 0)), 0);
    const add = (zone: ZoneName, geo: BufferGeometry, mat: MeshStandardMaterial, kind: Kind, want: (c: Cell) => boolean, cast: boolean, receive: boolean): void => {
      const n = count(zone, kind);
      if (n === 0) return;
      const pool = new Pool(geo, mat, kind, n, cast, receive);
      this.root.add(pool.mesh);
      this.pools.push({ zone, pool, want });
      this.stats.meshes++;
    };
    // 가까운 구간: 자세히(상태 1)·거칠게(상태 2). 줄기는 보이는 칸 전부
    add('near', a.hi.pine, a.pineMat, 'pine', (c) => c.state === 1, true, true);
    add('near', a.lo.pine, a.pineMat, 'pine', (c) => c.state === 2, true, true);
    add('near', a.hi.broad, a.broadMat, 'broad', (c) => c.state === 1, true, true);
    add('near', a.lo.broad, a.broadMat, 'broad', (c) => c.state === 2, true, true);
    add('near', a.trunk, a.barkMat, 'trunk', (c) => c.state !== 0, true, false);
    // 중간·먼 구간: 보이면 그린다. 먼 구간은 그림자를 받지도 않는다
    add('mid', a.mid.pine, a.pineMat, 'pine', (c) => c.state !== 0, false, true);
    add('mid', a.mid.broad, a.broadMat, 'broad', (c) => c.state !== 0, false, true);
    add('far', a.far.pine, a.pineMat, 'pine', (c) => c.state !== 0, false, false);
    add('far', a.far.broad, a.broadMat, 'broad', (c) => c.state !== 0, false, false);
  }

  /**
   * 카메라로 시야에 드는 칸과 가까운 칸의 자세히/거칠게를 다시 정하고, 바뀐 구간의 메시만 다시 채운다.
   * 카메라가 가만히 있거나 칸 집합이 같으면 칸 판정(상자 몇 백 개)만 하고 끝난다.
   */
  update(camera: Camera): void {
    const t0 = performance.now(); // TEMP
    DBG.updates++; // TEMP
    try { // TEMP
    camera.updateMatrixWorld();
    this.pv.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
    // 렌더러가 정한 좌표계(WebGPU 는 깊이 0..1)로 평면을 뽑는다 (gaesup GrassDriver 와 같음)
    this.frustum.setFromProjectionMatrix(this.pv, camera.coordinateSystem, (camera as Camera & { reversedDepth?: boolean }).reversedDepth ?? false);
    this.cam.setFromMatrixPosition(camera.matrixWorld);
    const dirty: Record<ZoneName, boolean> = { near: false, mid: false, far: false };
    for (const c of this.cells) {
      let next: 0 | 1 | 2;
      if (c.zone === 'near') {
        // 그림자를 드리우는 구간: 시야 밖 나무도 시야 안에 그림자를 던지므로 상자를 부풀려 판정한다
        this.pad.copy(c.box).expandByScalar(SHADOW_MARGIN);
        if (!this.frustum.intersectsBox(this.pad)) next = 0;
        else {
          const d = Math.hypot(c.cx - this.cam.x, c.cz - this.cam.z);
          const hi = c.state === 1 ? d < HI_R + HYST : d < HI_R - HYST;
          next = hi ? 1 : 2;
        }
      } else next = this.frustum.intersectsBox(c.box) ? 1 : 0;
      if (next !== c.state) {
        c.state = next;
        dirty[c.zone] = true;
      }
    }
    if (!dirty.near && !dirty.mid && !dirty.far) return;
    DBG.fills++; // TEMP
    let trees = 0;
    let meshes = 0;
    for (const { zone, pool, want } of this.pools) {
      if (dirty[zone]) pool.fill(this.byZone[zone], want);
      if (pool.kind !== 'trunk') trees += pool.mesh.count;
      if (pool.mesh.visible) meshes++;
    }
    this.stats.drawnTrees = trees;
    this.stats.drawnMeshes = meshes;
    } finally { DBG.updateMs += performance.now() - t0; } // TEMP
  }

  /** GPU 버퍼를 놓는다 (지오메트리·재질은 공용이라 건드리지 않는다) */
  dispose(): void {
    for (const { pool } of this.pools) {
      pool.mesh.dispose();
      this.root.remove(pool.mesh);
    }
    this.pools.length = 0;
    this.cells.length = 0;
    for (const z of Object.keys(this.byZone) as ZoneName[]) this.byZone[z].length = 0;
  }
}
