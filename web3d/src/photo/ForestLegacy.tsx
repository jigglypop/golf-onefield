// 임시: 이전 판 숲(칸 나누기 전)과 나란히 재기 위한 파일. 측정이 끝나면 지운다.
import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';

import { Color, DoubleSide, Matrix4, Object3D, Quaternion, Vector2, Vector3, type BufferGeometry, type InstancedMesh } from 'three';

import type { Hole } from '../game/course.ts';
import { CROWN, scatter, ZONES, type Spot } from './forestPlan.ts';
import { packedMaps, tiled } from './surfaces.ts';
import { fieldOf } from './Terrain.tsx';
import { broadleafGeometry, leafMaps, needleMaps, pineGeometry } from './trees.ts';

function fill(mesh: InstancedMesh | null, spots: readonly Spot[], dark: string, light: string, crown: number): void {
  if (!mesh) return;
  const m = new Matrix4();
  const q = new Quaternion();
  const axis = new Vector3(0, 1, 0);
  const col = new Color();
  const d = new Color(dark);
  const l = new Color(light);
  spots.forEach((p, i) => {
    q.setFromAxisAngle(axis, p.yaw);
    const ch = p.h * crown;
    m.compose(new Vector3(p.x, p.y - 0.25 + p.h - ch / 2, p.z), q, new Vector3(p.r, ch, p.r));
    mesh.setMatrixAt(i, m);
    mesh.setColorAt(i, col.copy(d).lerp(l, p.tint));
  });
  mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  mesh.computeBoundingSphere();
  mesh.count = spots.length;
}

const NS = new Vector2(1.4, 1.4);
const NS_LEAF = new Vector2(1.2, 1.2);

function TreeLayer({ spots, pineGeo, broadGeo, needles, leaves, shadows }: { spots: readonly Spot[]; pineGeo: BufferGeometry; broadGeo: BufferGeometry; needles: ReturnType<typeof needleMaps>; leaves: ReturnType<typeof leafMaps>; shadows: boolean }) {
  const pines = useRef<InstancedMesh>(null);
  const broads = useRef<InstancedMesh>(null);
  const pineSpots = useMemo(() => spots.filter((s) => !s.broad), [spots]);
  const broadSpots = useMemo(() => spots.filter((s) => s.broad), [spots]);
  useLayoutEffect(() => {
    fill(pines.current, pineSpots, '#d6ecc6', '#ffffee', CROWN.pine);
    fill(broads.current, broadSpots, '#dcf0c0', '#ffffe6', CROWN.broad);
  }, [pineSpots, broadSpots]);
  return (
    <group>
      <instancedMesh ref={pines} args={[pineGeo, undefined, Math.max(1, pineSpots.length)]} castShadow={shadows} receiveShadow frustumCulled={false}>
        <meshStandardMaterial map={needles.map} normalMap={needles.normalMap} normalScale={NS} roughness={0.92} side={DoubleSide} />
      </instancedMesh>
      <instancedMesh ref={broads} args={[broadGeo, undefined, Math.max(1, broadSpots.length)]} castShadow={shadows} receiveShadow frustumCulled={false}>
        <meshStandardMaterial map={leaves.map} normalMap={leaves.normalMap} normalScale={NS_LEAF} roughness={0.9} />
      </instancedMesh>
    </group>
  );
}

function Trunks({ spots }: { spots: readonly Spot[] }) {
  const ref = useRef<InstancedMesh>(null);
  const bark = useMemo(() => tiled(packedMaps('pinebark', 1), 1, 1), []);
  useLayoutEffect(() => {
    const t = ref.current;
    if (!t) return;
    const o = new Object3D();
    spots.forEach((p, i) => {
      const bare = p.h * (1 - (p.broad ? CROWN.broad : CROWN.pine)) + 1.2;
      const rad = Math.max(0.14, p.r * 0.07);
      o.position.set(p.x, p.y + bare / 2 - 0.3, p.z);
      o.scale.set(rad, bare, rad);
      o.rotation.set(0, p.yaw, 0);
      o.updateMatrix();
      t.setMatrixAt(i, o.matrix);
    });
    t.instanceMatrix.needsUpdate = true;
    t.computeBoundingSphere();
    t.count = spots.length;
  }, [spots]);
  return (
    <instancedMesh ref={ref} args={[undefined, undefined, Math.max(1, spots.length)]} castShadow frustumCulled={false}>
      <cylinderGeometry args={[0.7, 1, 1, 7]} />
      <meshStandardMaterial map={bark.map} normalMap={bark.normalMap} roughnessMap={bark.roughnessMap} color="#9a8a7a" roughness={1} />
    </instancedMesh>
  );
}

export function ForestLegacy({ hole }: { hole: Hole }) {
  const field = useMemo(() => fieldOf(hole), [hole]);
  const layers = useMemo(() => ZONES.map((z, i) => ({ zone: z, spots: scatter(field, z, 9100 + hole.number * 17 + i * 101, 1) })), [field, hole.number]);
  const needles = useMemo(() => needleMaps(), []);
  const leaves = useMemo(() => leafMaps(), []);
  const geos = useMemo(() => ({ near: { pine: pineGeometry(5, 8, 12), broad: broadleafGeometry(8, 6, 14) }, mid: { pine: pineGeometry(6, 5, 8), broad: broadleafGeometry(9, 4, 9) }, far: { pine: pineGeometry(7, 3, 6), broad: broadleafGeometry(10, 3, 7) } }), []);
  useEffect(() => () => { for (const g of Object.values(geos)) { g.pine.dispose(); g.broad.dispose(); } }, [geos]);
  return (
    <group name="forest">
      {layers.map(({ zone, spots }) => (
        <group key={`${hole.number}-${zone.name}`}>
          <TreeLayer spots={spots} pineGeo={geos[zone.name].pine} broadGeo={geos[zone.name].broad} needles={needles} leaves={leaves} shadows={zone.name === 'near'} />
          {zone.name === 'near' && <Trunks spots={spots} />}
        </group>
      ))}
    </group>
  );
}
