/**
 * 홀 그리기: 러프(넓은 땅), 페어웨이, 티, 프린지·그린·홀컵·깃발, 벙커, 물, OB 말뚝, 경계 밖 숲.
 * 땅 위에 겹친 판은 1 cm 안쪽으로만 띄우고 polygonOffset 으로 앞뒤를 정한다(공이 떠 보이지 않게).
 */
import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';

import { CircleGeometry, DoubleSide, Object3D, PlaneGeometry, Shape, ShapeGeometry, Vector2, type InstancedMesh } from 'three';

import { LAWN_TILE_M, lawnMaps, macroOverlay, mowStripes, packedMaps, repeated, tiled } from '../../photo/surfaces.ts';
import { needleMaps, pineGeometry } from '../../photo/trees.ts';
import { Forest } from '../../photo/Forest.tsx';
import { SoftEdge } from '../../photo/SoftEdge.tsx';
import { Terrain } from '../../photo/Terrain.tsx';
import { Lakes } from '../../photo/Water.tsx';
import { M_PER_YD } from '../../engine/units.ts';
import { Label } from '../../scene/Label.tsx';
import { CUP_R, ellipsePoints, fairwayOutline, FRINGE_M, TEE_BOX, yawTo, type Hole, type Vec2 } from '../course.ts';
import { holeOf, useGame } from '../round.ts';

/** 판 높이 (m): 공은 이 위 1 cm 에 놓인다 */
export const SURFACE_Y = { water: 0.0015, fairway: 0.002, tee: 0.003, land: 0.0035, fringe: 0.004, green: 0.005, bunker: 0.006, cup: 0.0055 } as const;
export const BALL_LIFT = 0.008;

/** 월드 (x, z) 점들 → three Shape (모양 좌표는 (x, −z): rotateX(−π/2) 뒤 다시 (x, 0, z) 가 된다) */
function shapeOf(points: readonly Vec2[]): Shape {
  const s = new Shape();
  points.forEach(([x, z], i) => (i === 0 ? s.moveTo(x, -z) : s.lineTo(x, -z)));
  s.closePath();
  return s;
}

function flatShape(points: readonly Vec2[]): ShapeGeometry {
  return new ShapeGeometry(shapeOf(points), 1).rotateX(-Math.PI / 2);
}

function useDispose(...items: { dispose(): void }[]) {
  useEffect(() => () => items.forEach((i) => i.dispose()), items); // eslint-disable-line react-hooks/exhaustive-deps
}

const NORMAL_SCALE = {
  rough: new Vector2(1.5, 1.5),
  fairway: new Vector2(0.85, 0.85),
  green: new Vector2(0.6, 0.6),
  tee: new Vector2(0.75, 0.75),
  sand: new Vector2(1.3, 1.3),
} as const;

/** 큰 얼룩 덮개의 색 (밝은 풀·마른 풀 쪽 / 그늘 쪽) */
const MACRO = { fairway: ['#bcd65e', '#2c4e18'] } as const;

function Fairways({ hole }: { hole: Hole }) {
  const outlines = useMemo(() => hole.fairways.map((f) => fairwayOutline(f)), [hole]);
  const geometries = useMemo(() => outlines.map((o) => flatShape(o)), [outlines]);
  // ShapeGeometry 의 uv = 모양 좌표 (m). 줄무늬 한 쌍 = 24 m
  const maps = useMemo(() => tiled(lawnMaps('fairway'), 1 / LAWN_TILE_M.fairway, 1 / LAWN_TILE_M.fairway), []);
  const stripes = useMemo(() => repeated(mowStripes(), 1 / 24, 1), []);
  const macro = useMemo(() => repeated(macroOverlay(29, [...MACRO.fairway]), 1 / 55, 1 / 55), []);
  useEffect(() => () => geometries.forEach((g) => g.dispose()), [geometries]);
  return (
    <group>
      {geometries.map((g, i) => (
        <group key={i}>
          <SoftEdge poly={outlines[i]!} y={SURFACE_Y.fairway - 0.0006} width={1.6} order={0} />
          <mesh geometry={g} position={[0, SURFACE_Y.fairway, 0]} receiveShadow>
            <meshStandardMaterial map={maps.map} normalMap={maps.normalMap} normalScale={NORMAL_SCALE.fairway} color="#f4fbe8" roughness={0.9} polygonOffset polygonOffsetFactor={-1} polygonOffsetUnits={-2} />
          </mesh>
          <mesh geometry={g} position={[0, SURFACE_Y.fairway + 0.0003, 0]}>
            <meshBasicMaterial map={stripes} transparent depthWrite={false} polygonOffset polygonOffsetFactor={-1.2} polygonOffsetUnits={-2.4} />
          </mesh>
          <mesh geometry={g} position={[0, SURFACE_Y.fairway + 0.0005, 0]}>
            <meshBasicMaterial map={macro} transparent depthWrite={false} polygonOffset polygonOffsetFactor={-1.4} polygonOffsetUnits={-2.8} />
          </mesh>
        </group>
      ))}
    </group>
  );
}

/** 티 박스 바깥 선 (티 평면 중심 기준) */
const TEE_POLY: Vec2[] = [
  [-TEE_BOX[0], -TEE_BOX[1]],
  [TEE_BOX[0], -TEE_BOX[1]],
  [TEE_BOX[0], TEE_BOX[1]],
  [-TEE_BOX[0], TEE_BOX[1]],
];

function TeeBox({ hole }: { hole: Hole }) {
  const yaw = yawTo(hole.tee, hole.teeTarget);
  const geometry = useMemo(() => {
    const g = new PlaneGeometry(TEE_BOX[0] * 2, TEE_BOX[1] * 2).rotateX(-Math.PI / 2);
    const uv = g.getAttribute('uv');
    for (let i = 0; i < uv.count; i++) uv.setXY(i, uv.getX(i) * TEE_BOX[0] * 2, uv.getY(i) * TEE_BOX[1] * 2); // uv = m
    return g;
  }, []);
  const maps = useMemo(() => tiled(lawnMaps('tee'), 1 / LAWN_TILE_M.tee, 1 / LAWN_TILE_M.tee), []);
  useDispose(geometry);
  return (
    <group position={[hole.tee[0], 0, hole.tee[1]]} rotation={[0, -yaw, 0]}>
      <group position={[-1.5, 0, 0]}>
        <SoftEdge poly={TEE_POLY} y={SURFACE_Y.tee - 0.0006} width={1.2} order={1} />
      </group>
      <mesh geometry={geometry} position={[-1.5, SURFACE_Y.tee, 0]} receiveShadow>
        <meshStandardMaterial map={maps.map} normalMap={maps.normalMap} normalScale={NORMAL_SCALE.tee} color="#e6f2d6" roughness={0.9} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-3} />
      </mesh>
      {[-3.2, 3.2].map((z) => (
        <mesh key={z} position={[1.2, 0.09, z]} castShadow>
          <sphereGeometry args={[0.09, 16, 12]} />
          <meshStandardMaterial color="#2f6fd6" roughness={0.4} />
        </mesh>
      ))}
    </group>
  );
}

function GreenArea({ hole }: { hole: Hole }) {
  const greenMaps = useMemo(() => tiled(lawnMaps('green'), 1 / LAWN_TILE_M.green, 1 / LAWN_TILE_M.green), []);
  const fringeMaps = useMemo(() => tiled(lawnMaps('fairway'), 1 / LAWN_TILE_M.fairway, 1 / LAWN_TILE_M.fairway), []);
  const geo = useMemo(
    () => ({
      fringe: flatShape(ellipsePoints(hole.green, FRINGE_M, 64)),
      green: flatShape(ellipsePoints(hole.green, 0, 64)),
      cup: new CircleGeometry(CUP_R, 24).rotateX(-Math.PI / 2),
    }),
    [hole],
  );
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);
  const fringeLine = useMemo(() => ellipsePoints(hole.green, FRINGE_M, 64), [hole]);
  return (
    <group>
      <SoftEdge poly={fringeLine} y={SURFACE_Y.fringe - 0.0006} width={1.3} order={2} />
      <mesh geometry={geo.fringe} position={[0, SURFACE_Y.fringe, 0]} receiveShadow>
        <meshStandardMaterial map={fringeMaps.map} normalMap={fringeMaps.normalMap} normalScale={NORMAL_SCALE.fairway} color="#e6f3d4" roughness={0.92} polygonOffset polygonOffsetFactor={-3} polygonOffsetUnits={-4} />
      </mesh>
      <mesh geometry={geo.green} position={[0, SURFACE_Y.green, 0]} receiveShadow>
        <meshStandardMaterial map={greenMaps.map} normalMap={greenMaps.normalMap} normalScale={NORMAL_SCALE.green} color="#ffffff" roughness={0.8} polygonOffset polygonOffsetFactor={-4} polygonOffsetUnits={-5} />
      </mesh>
      <group position={[hole.pin[0], 0, hole.pin[1]]}>
        <mesh geometry={geo.cup} position={[0, SURFACE_Y.cup, 0]}>
          <meshBasicMaterial color="#0d140c" polygonOffset polygonOffsetFactor={-6} polygonOffsetUnits={-8} />
        </mesh>
        <mesh position={[0, 1.2, 0]} castShadow>
          <cylinderGeometry args={[0.012, 0.012, 2.4, 8]} />
          <meshStandardMaterial color="#f2f2ea" roughness={0.5} />
        </mesh>
      </group>
    </group>
  );
}

function Bunkers({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => hole.bunkers.map((b) => flatShape(ellipsePoints(b, 0, 40))), [hole]);
  const lips = useMemo(() => hole.bunkers.map((b) => flatShape(ellipsePoints(b, 0.8, 40))), [hole]);
  const sand = useMemo(() => tiled(packedMaps('sand', 1.9), 1 / 2.4, 1 / 2.4), []);
  useEffect(() => () => [...geometries, ...lips].forEach((g) => g.dispose()), [geometries, lips]);
  return (
    <group>
      {lips.map((g, i) => (
        <mesh key={`l${i}`} geometry={g} position={[0, SURFACE_Y.bunker - 0.0005, 0]} receiveShadow>
          <meshStandardMaterial color="#6f9a47" roughness={1} polygonOffset polygonOffsetFactor={-5} polygonOffsetUnits={-6} />
        </mesh>
      ))}
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} position={[0, SURFACE_Y.bunker, 0]} receiveShadow>
          <meshStandardMaterial map={sand.map} normalMap={sand.normalMap} roughnessMap={sand.roughnessMap} normalScale={NORMAL_SCALE.sand} roughness={1} polygonOffset polygonOffsetFactor={-6} polygonOffsetUnits={-7} />
        </mesh>
      ))}
    </group>
  );
}

/** OB 흰 말뚝 (경계를 따라 14 m 마다) */
function OBStakes({ hole }: { hole: Hole }) {
  const ref = useRef<InstancedMesh>(null);
  const spots = useMemo(() => {
    const out: Vec2[] = [];
    const b = hole.bounds;
    for (let i = 0; i < b.length; i++) {
      const a = b[i]!;
      const c = b[(i + 1) % b.length]!;
      const len = Math.hypot(c[0] - a[0], c[1] - a[1]);
      for (let s = 0; s < len; s += 14) out.push([a[0] + ((c[0] - a[0]) * s) / len, a[1] + ((c[1] - a[1]) * s) / len]);
    }
    return out;
  }, [hole]);
  useLayoutEffect(() => {
    const m = ref.current;
    if (!m) return;
    const o = new Object3D();
    spots.forEach(([x, z], i) => {
      o.position.set(x, 0.5, z);
      o.updateMatrix();
      m.setMatrixAt(i, o.matrix);
    });
    m.instanceMatrix.needsUpdate = true;
    m.computeBoundingSphere();
  }, [spots]);
  return (
    <instancedMesh ref={ref} args={[undefined, undefined, spots.length]} castShadow>
      <boxGeometry args={[0.1, 1, 0.1]} />
      <meshStandardMaterial color="#f4f4ee" roughness={0.6} />
    </instancedMesh>
  );
}

/** 물 위의 땅 (섬 그린 둘레) */
function Land({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => (hole.land ?? []).map((l) => flatShape(l)), [hole]);
  const maps = useMemo(() => tiled(lawnMaps('fairway'), 1 / LAWN_TILE_M.fairway, 1 / LAWN_TILE_M.fairway), []);
  useEffect(() => () => geometries.forEach((g) => g.dispose()), [geometries]);
  return (
    <group>
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} position={[0, SURFACE_Y.land, 0]} receiveShadow>
          <meshStandardMaterial map={maps.map} normalMap={maps.normalMap} normalScale={NORMAL_SCALE.fairway} color="#e8f6d4" roughness={0.95} polygonOffset polygonOffsetFactor={-2.5} polygonOffsetUnits={-3.5} />
        </mesh>
      ))}
    </group>
  );
}

/** 롱드라이브: 150–375 yd, 25 yd 마다 가로선과 글자판 */
function DriveMarkers({ hole }: { hole: Hole }) {
  const line = useMemo(() => new PlaneGeometry(0.35, 90).rotateX(-Math.PI / 2), []);
  useDispose(line);
  if (hole.kind !== 'longdrive') return null;
  const yds = [150, 175, 200, 225, 250, 275, 300, 325, 350, 375];
  return (
    <group>
      {yds.map((yd) => {
        const x = yd * M_PER_YD;
        const major = yd % 50 === 0;
        return (
          <group key={yd}>
            <mesh geometry={line} position={[x, 0.008, 0]}>
              <meshBasicMaterial color={major ? '#ffffff' : '#e8f3dc'} transparent opacity={major ? 0.85 : 0.5} polygonOffset polygonOffsetFactor={-6} polygonOffsetUnits={-7} />
            </mesh>
            {major &&
              [-1, 1].map((side) => (
                <Label key={side} lines={[{ text: `${yd}`, size: 0.6, color: '#1d2a22', weight: 800 }]} position={[x, 2.4, side * 48]} height={3.4} w={300} h={200} bg="#f5f5f0" border="rgba(0,0,0,0.2)" />
              ))}
          </group>
        );
      })}
    </group>
  );
}

/** 장애물 나무: 충돌 원기둥(r, h)에 맞춘 소나무. 땅은 평평한 채이고 날아가는 공만 막는다 (obstacles.ts) */
function ObstacleTrees({ hole }: { hole: Hole }) {
  const crown = useMemo(() => pineGeometry(11, 8, 12), []);
  const needles = useMemo(() => needleMaps(), []);
  useDispose(crown);
  const trees = hole.trees;
  if (!trees || trees.length === 0) return null;
  return (
    <group name="obstacle-trees">
      {trees.map((t, i) => (
        <group key={i} position={[t.x, 0, t.z]}>
          <mesh position={[0, t.h * 0.3, 0]} castShadow>
            <cylinderGeometry args={[0.16 + t.r * 0.05, 0.28 + t.r * 0.06, t.h * 0.6, 8]} />
            <meshStandardMaterial color="#6d5236" roughness={1} />
          </mesh>
          <mesh geometry={crown} position={[0, t.h * 0.6, 0]} scale={[t.r * 1.05, t.h * 0.8, t.r * 1.05]} castShadow receiveShadow>
            <meshStandardMaterial map={needles.map} normalMap={needles.normalMap} color="#d6ecc6" roughness={0.92} side={DoubleSide} />
          </mesh>
        </group>
      ))}
    </group>
  );
}

export function HoleScene() {
  const hole = useGame(holeOf);
  return (
    <group name={`golf-hole-${hole.number}`} key={hole.number}>
      <Terrain hole={hole} />
      <Land hole={hole} />
      <DriveMarkers hole={hole} />
      <Fairways hole={hole} />
      <TeeBox hole={hole} />
      <GreenArea hole={hole} />
      <Bunkers hole={hole} />
      <Lakes hole={hole} />
      <OBStakes hole={hole} />
      <ObstacleTrees hole={hole} />
      <Forest hole={hole} />
    </group>
  );
}
