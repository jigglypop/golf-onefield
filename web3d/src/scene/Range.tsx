/**
 * 연습장: 러프(넓은 땅), 페어웨이 줄무늬, 티, 50 yd 마다 거리 선과 표지판, 핀이 꽂힌 그린, 둘레 나무.
 * 땅 위에 겹친 판은 높이를 조금씩 띄우고 polygonOffset 으로 z-fighting 을 막는다.
 */
import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';

import { CuboidCollider, RigidBody } from '@react-three/rapier';
import { CircleGeometry, Color, DoubleSide, Matrix4, Object3D, PlaneGeometry, Quaternion, RingGeometry, Shape, ShapeGeometry, Vector3, type InstancedMesh } from 'three';

import { useApp } from '../state.ts';
import { Label } from './Label.tsx';
import { COLORS, FAIRWAY, GREEN_RADIUS, LINE_LENGTH, MARKER_COLORS, MARKERS_YD, ydToM } from './layout.ts';
import { grassTexture, stripeTexture } from './textures.ts';

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

function Ground() {
  const texture = useMemo(() => grassTexture(COLORS.rough, [500, 500]), []);
  const geometry = useMemo(() => flat(new PlaneGeometry(4000, 4000)), []);
  useEffect(() => () => (texture.dispose(), geometry.dispose()), [texture, geometry]);
  return (
    <mesh geometry={geometry} position={[300, 0, 0]} receiveShadow>
      <meshStandardMaterial map={texture} roughness={1} metalness={0} />
    </mesh>
  );
}

function Fairway() {
  const geometry = useMemo(() => flat(new ShapeGeometry(roundedRect(FAIRWAY.x0, FAIRWAY.x1, FAIRWAY.width / 2, 16), 12)), []);
  // ShapeGeometry 의 uv = 꼭짓점 (x, y) m. 줄무늬 한 쌍 = 24 m
  const texture = useMemo(() => stripeTexture(COLORS.fairwayLight, COLORS.fairwayDark, [1 / 24, 1 / 24]), []);
  useEffect(() => () => (texture.dispose(), geometry.dispose()), [texture, geometry]);
  return (
    <mesh geometry={geometry} position={[0, 0.03, 0]} receiveShadow>
      <meshStandardMaterial map={texture} roughness={0.95} polygonOffset polygonOffsetFactor={-1} polygonOffsetUnits={-2} />
    </mesh>
  );
}

function Tee() {
  const geometry = useMemo(() => flat(new ShapeGeometry(roundedRect(-6, 5, 7, 1.5), 4)), []);
  useEffect(() => () => geometry.dispose(), [geometry]);
  return (
    <group>
      <mesh geometry={geometry} position={[0, 0.035, 0]} receiveShadow>
        <meshStandardMaterial color={COLORS.tee} roughness={0.95} polygonOffset polygonOffsetFactor={-1} polygonOffsetUnits={-2} />
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
  const geo = useMemo(
    () => ({
      green: flat(new CircleGeometry(GREEN_RADIUS, 64)),
      fringe: flat(new RingGeometry(GREEN_RADIUS, GREEN_RADIUS + 2.5, 64)),
      hole: flat(new CircleGeometry(0.11, 24)),
    }),
    [],
  );
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);
  return (
    <group position={[x, 0, 0]}>
      <mesh geometry={geo.fringe} position={[0, 0.045, 0]} receiveShadow>
        <meshStandardMaterial color={COLORS.fringe} roughness={0.95} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-3} />
      </mesh>
      <mesh geometry={geo.green} position={[0, 0.05, 0]} receiveShadow>
        <meshStandardMaterial color={COLORS.green} roughness={0.85} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-4} />
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

/** 둘레 나무: 원뿔 + 줄기, 인스턴스 두 묶음 */
function Trees() {
  const crowns = useRef<InstancedMesh>(null);
  const trunks = useRef<InstancedMesh>(null);
  const spots = useMemo(() => {
    const out: { x: number; z: number; h: number; r: number; tint: number }[] = [];
    let s = 12345;
    const rnd = () => ((s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
    for (let x = -60; x <= 470; x += 9) {
      for (const side of [-1, 1]) {
        const z = side * (78 + rnd() * 70);
        out.push({ x: x + rnd() * 6, z, h: 8 + rnd() * 8, r: 2.4 + rnd() * 2, tint: rnd() });
      }
    }
    for (let z = -160; z <= 160; z += 8) out.push({ x: 450 + rnd() * 60, z: z + rnd() * 5, h: 9 + rnd() * 9, r: 2.6 + rnd() * 2, tint: rnd() });
    return out;
  }, []);
  useLayoutEffect(() => {
    const c = crowns.current;
    const t = trunks.current;
    if (!c || !t) return;
    const m = new Matrix4();
    const q = new Quaternion();
    const o = new Object3D();
    const dark = new Color('#2f5a2a');
    const light = new Color('#4d7d34');
    const col = new Color();
    spots.forEach((p, i) => {
      m.compose(new Vector3(p.x, 1.6 + p.h / 2, p.z), q, new Vector3(p.r, p.h, p.r));
      c.setMatrixAt(i, m);
      c.setColorAt(i, col.copy(dark).lerp(light, p.tint));
      o.position.set(p.x, 1, p.z);
      o.scale.set(0.35, 2, 0.35);
      o.updateMatrix();
      t.setMatrixAt(i, o.matrix);
    });
    c.instanceMatrix.needsUpdate = true;
    if (c.instanceColor) c.instanceColor.needsUpdate = true;
    t.instanceMatrix.needsUpdate = true;
    c.computeBoundingSphere();
    t.computeBoundingSphere();
  }, [spots]);
  return (
    <group>
      <instancedMesh ref={crowns} args={[undefined, undefined, spots.length]} castShadow>
        <coneGeometry args={[1, 1, 8]} />
        <meshStandardMaterial roughness={0.9} />
      </instancedMesh>
      <instancedMesh ref={trunks} args={[undefined, undefined, spots.length]} castShadow>
        <cylinderGeometry args={[0.5, 0.6, 1, 6]} />
        <meshStandardMaterial color="#6b4a2f" roughness={0.9} />
      </instancedMesh>
    </group>
  );
}

export function Range() {
  return (
    <group name="golf-range">
      <Ground />
      <Fairway />
      <Tee />
      <DistanceMarkers />
      <Green />
      <Trees />
    </group>
  );
}
