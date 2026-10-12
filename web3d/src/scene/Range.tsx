/**
 * 연습장: 러프(넓은 땅), 페어웨이 줄무늬, 티, 50 yd 마다 거리 선과 표지판, 핀이 꽂힌 그린, 둘레 나무.
 * 땅 위에 겹친 판은 높이를 조금씩 띄우고 polygonOffset 으로 z-fighting 을 막는다.
 */
import { useEffect, useMemo } from 'react';

import { CuboidCollider, RigidBody } from '@react-three/rapier';
import { CircleGeometry, DoubleSide, PlaneGeometry, RingGeometry, Shape, ShapeGeometry, Vector2 } from 'three';

import { useApp } from '../state.ts';
import { Label } from './Label.tsx';
import { COLORS, FAIRWAY, GREEN_RADIUS, LINE_LENGTH, MARKER_COLORS, MARKERS_YD, ydToM } from './layout.ts';
import { Forest } from '../photo/Forest.tsx';
import { LAWN_TILE_M, lawnMaps, mowStripes, repeated, tiled } from '../photo/surfaces.ts';
import { RANGE_HOLE } from '../photo/rangeHole.ts';
import { Terrain } from '../photo/Terrain.tsx';

const flat = (g: PlaneGeometry | CircleGeometry | RingGeometry | ShapeGeometry) => g.rotateX(-Math.PI / 2);

function roundedRect(x0: number, x1: number, halfWidth: number, r: number): Shape {
  const s = new Shape();
  // 모양의 y 는 rotateX(−π/2) 뒤 −z 가 된다 (폭이 대칭이라 상관없다)
  s.moveTo(x0 + r, -halfWidth);
  s.lineTo(x1 - r, -halfWidth);
  s.quadraticCurveTo(x1, -halfWidth, x1, -halfWidth + r);
  s.lineTo(x1, halfWidth - r);
  s.quadraticCurveTo(x1, halfWidth, x1 - r, halfWidth);
  s.lineTo(x0 + r, halfWidth);
  s.quadraticCurveTo(x0, halfWidth, x0, halfWidth - r);
  s.lineTo(x0, -halfWidth + r);
  s.quadraticCurveTo(x0, -halfWidth, x0 + r, -halfWidth);
  return s;
}

/** 캐릭터가 걸을 땅 (Rapier 고정 몸체). WorldPhysics 안에 둔다. 윗면 y = 0. */
export function RangeColliders() {
  return (
    <RigidBody type="fixed" colliders={false} name="range-ground">
      <CuboidCollider args={[1200, 1, 1200]} position={[300, -1, 0]} />
    </RigidBody>
  );
}

function Fairway() {
  const geometry = useMemo(() => flat(new ShapeGeometry(roundedRect(FAIRWAY.x0, FAIRWAY.x1, FAIRWAY.width / 2, 16), 12)), []);
  // ShapeGeometry 의 uv = 꼭짓점 (x, y) m. 잔디 질감 한 장 = 1.6 m, 줄무늬 한 쌍 = 24 m
  const maps = useMemo(() => tiled(lawnMaps('fairway'), 1 / LAWN_TILE_M.fairway, 1 / LAWN_TILE_M.fairway), []);
  const stripes = useMemo(() => repeated(mowStripes(), 1 / 24, 1), []);
  useEffect(() => () => geometry.dispose(), [geometry]);
  return (
    <group>
      <mesh geometry={geometry} position={[0, 0.03, 0]} receiveShadow>
        <meshStandardMaterial map={maps.map} normalMap={maps.normalMap} normalScale={new Vector2(1.1, 1.1)} color="#f4fbe8" roughness={0.9} polygonOffset polygonOffsetFactor={-1} polygonOffsetUnits={-2} />
      </mesh>
      <mesh geometry={geometry} position={[0, 0.0305, 0]}>
        <meshBasicMaterial map={stripes} transparent depthWrite={false} polygonOffset polygonOffsetFactor={-1.2} polygonOffsetUnits={-2.4} />
      </mesh>
    </group>
  );
}

function Tee() {
  const geometry = useMemo(() => flat(new ShapeGeometry(roundedRect(-6, 5, 7, 1.5), 4)), []);
  const maps = useMemo(() => tiled(lawnMaps('tee'), 1 / LAWN_TILE_M.tee, 1 / LAWN_TILE_M.tee), []);
  useEffect(() => () => geometry.dispose(), [geometry]);
  return (
    <group>
      <mesh geometry={geometry} position={[0, 0.035, 0]} receiveShadow>
        <meshStandardMaterial map={maps.map} normalMap={maps.normalMap} normalScale={new Vector2(0.9, 0.9)} color="#e6f2d6" roughness={0.9} polygonOffset polygonOffsetFactor={-1} polygonOffsetUnits={-2} />
      </mesh>
      {/* 티 마커 */}
      {[-3.2, 3.2].map((z) => (
        <mesh key={z} position={[1.2, 0.09, z]} castShadow>
          <sphereGeometry args={[0.09, 16, 12]} />
          <meshStandardMaterial color="#2f6fd6" roughness={0.4} />
        </mesh>
      ))}
    </group>
  );
}

function DistanceMarkers() {
  const line = useMemo(() => flat(new PlaneGeometry(0.35, LINE_LENGTH)), []);
  useEffect(() => () => line.dispose(), [line]);
  return (
    <group>
      {MARKERS_YD.map((yd) => {
        const x = ydToM(yd);
        const color = MARKER_COLORS[yd] ?? '#ffffff';
        const dark = yd === 100 ? '#1d2a22' : '#ffffff';
        return (
          <group key={yd}>
            <mesh geometry={line} position={[x, 0.06, 0]} receiveShadow>
              <meshStandardMaterial color={COLORS.line} roughness={0.9} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-4} />
            </mesh>
            {[-1, 1].map((side) => (
              <group key={side} position={[x, 0, side * (LINE_LENGTH / 2 + 3)]}>
                <mesh position={[0, 0.7, 0]} castShadow>
                  <cylinderGeometry args={[0.07, 0.07, 1.4, 8]} />
                  <meshStandardMaterial color="#d9d9d2" roughness={0.6} />
                </mesh>
                <Label
                  lines={[
                    { text: String(yd), size: 0.56, color: dark, weight: 800 },
                    { text: 'yd', size: 0.22, color: dark, weight: 600 },
                  ]}
                  position={[0, 3.4, 0]}
                  height={4}
                  w={320}
                  h={240}
                  bg={color}
                  border="rgba(0,0,0,0.25)"
                />
              </group>
            ))}
          </group>
        );
      })}
    </group>
  );
}

function Green() {
  const pinYd = useApp((s) => s.pinYd);
  const x = ydToM(pinYd);
  // 착지점이 핀 가까이(20 m 안)면 착지 라벨과 겹치므로 핀 라벨을 숨긴다
  const nearLanding = useApp((s) => {
    if (s.phase !== 'landed' || !s.shot) return false;
    const end = s.shot.result.samples[s.shot.result.samples.length - 1]!;
    return Math.hypot(end.x - ydToM(s.pinYd), end.z) < 20;
  });
  const geo = useMemo(() => {
    const scaleUv = (g: CircleGeometry | RingGeometry, r: number) => {
      const p = g.getAttribute('position');
      const uv = g.getAttribute('uv');
      for (let i = 0; i < uv.count; i++) uv.setXY(i, p.getX(i) / 1, p.getY(i) / 1); // 만들 때 xy 평면: uv = m (위에서 눕힌다)
      void r;
      return g;
    };
    return {
      green: flat(scaleUv(new CircleGeometry(GREEN_RADIUS, 64), GREEN_RADIUS)),
      fringe: flat(scaleUv(new RingGeometry(GREEN_RADIUS, GREEN_RADIUS + 2.5, 64), GREEN_RADIUS)),
      hole: flat(new CircleGeometry(0.11, 24)),
    };
  }, []);
  const greenMaps = useMemo(() => tiled(lawnMaps('green'), 1 / LAWN_TILE_M.green, 1 / LAWN_TILE_M.green), []);
  const fringeMaps = useMemo(() => tiled(lawnMaps('fairway'), 1 / LAWN_TILE_M.fairway, 1 / LAWN_TILE_M.fairway), []);
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);
  return (
    <group position={[x, 0, 0]}>
      <mesh geometry={geo.fringe} position={[0, 0.045, 0]} receiveShadow>
        <meshStandardMaterial map={fringeMaps.map} normalMap={fringeMaps.normalMap} normalScale={new Vector2(1.1, 1.1)} color="#e6f3d4" roughness={0.92} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-3} />
      </mesh>
      <mesh geometry={geo.green} position={[0, 0.05, 0]} receiveShadow>
        <meshStandardMaterial map={greenMaps.map} normalMap={greenMaps.normalMap} normalScale={new Vector2(0.7, 0.7)} color="#ffffff" roughness={0.8} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-4} />
      </mesh>
      <mesh geometry={geo.hole} position={[0, 0.07, 0]}>
        <meshBasicMaterial color="#10160f" polygonOffset polygonOffsetFactor={-3} polygonOffsetUnits={-6} />
      </mesh>
      {/* 깃대와 깃발 (멀리서도 보이게 실물보다 굵고 크게) */}
      <mesh position={[0, 1.35, 0]} castShadow>
        <cylinderGeometry args={[0.035, 0.035, 2.7, 10]} />
        <meshStandardMaterial color="#f2f2ea" roughness={0.5} />
      </mesh>
      <mesh position={[0.55, 2.32, 0]} castShadow>
        <planeGeometry args={[1.1, 0.7]} />
        <meshStandardMaterial color="#e5484d" roughness={0.7} side={DoubleSide} />
      </mesh>
      {!nearLanding && <Label lines={[{ text: `핀 ${Math.round(x)} m · ${pinYd} yd`, size: 0.42 }]} position={[0, 5.2, 0]} height={2.2} w={560} h={150} />}
    </group>
  );
}

export function Range() {
  return (
    <group name="golf-range">
      <Terrain hole={RANGE_HOLE} />
      <Fairway />
      <Tee />
      <DistanceMarkers />
      <Green />
      <Forest hole={RANGE_HOLE} />
    </group>
  );
}
