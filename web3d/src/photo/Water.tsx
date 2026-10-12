/**
 * 호수 수면: 지형이 파 놓은 호수(`terrain.ts` carve) 위에 수면 판 하나. 깊이 색 지도(얕은 청록 → 깊은 남색 + 가장자리 거품)와
 * 흘러가는 잔물결 법선 지도, 하늘 환경 반사. 표준 재질이라 WebGPU·WebGL 이 같다.
 * 호수 바닥이 파여 있으므로 수면 판은 둑과 만나는 곳에서 자연스러운 물가선이 된다.
 * 메모리: 호수마다 만드는 지오메트리·잔물결 복제·깊이 지도의 GPU 쪽과 호수들이 같이 쓰는 하늘 반사 환경 지도는 호수가 내려갈 때 놓는다
 * (깊이 지도의 캔버스는 `lakeDepthMap` 이 다각형별로 캐시해 홀로 돌아와도 다시 그리지 않는다).
 */
import { useEffect, useMemo, useRef } from 'react';

import { useFrame } from '@react-three/fiber';
import { createSkyEnvironmentTexture } from 'gaesup-world';
import { DoubleSide, EquirectangularReflectionMapping, Shape, ShapeGeometry, Vector2, type Mesh, type Texture } from 'three';

import type { Hole, Vec2 } from '../game/course.ts';
import { lakeDepthMap, waterRipple } from './surfaces.ts';
import { WATER_Y } from './terrain.ts';

const RIPPLE_M = 7; // 잔물결 한 장이 덮는 길이 (m)

function Lake({ poly, ripple, env }: { poly: readonly Vec2[]; ripple: Texture; env: Texture }) {
  const { geometry, depth } = useMemo(() => {
    const shape = new Shape();
    poly.forEach(([x, z], i) => (i === 0 ? shape.moveTo(x, -z) : shape.lineTo(x, -z)));
    shape.closePath();
    const g = new ShapeGeometry(shape, 1).rotateX(-Math.PI / 2);
    const depth = lakeDepthMap(poly);
    const pos = g.getAttribute('position');
    const uv = g.getAttribute('uv');
    for (let i = 0; i < pos.count; i++) uv.setXY(i, (pos.getX(i) - depth.min[0]) / depth.size[0], (pos.getZ(i) - depth.min[1]) / depth.size[1]);
    uv.needsUpdate = true;
    return { geometry: g, depth };
  }, [poly]);
  // 잔물결은 같은 uv 에 자기 반복·이동 행렬만 따로 건다
  const rip = useMemo(() => {
    const t = ripple.clone();
    t.repeat.set(depth.size[0] / RIPPLE_M, depth.size[1] / RIPPLE_M);
    t.needsUpdate = true;
    return t;
  }, [ripple, depth]);
  useEffect(
    () => () => {
      geometry.dispose();
      depth.map.dispose();
      rip.dispose();
    },
    [geometry, depth, rip],
  );
  const mesh = useRef<Mesh>(null);
  useFrame((_, dt) => {
    rip.offset.x = (rip.offset.x + dt * 0.012) % 1;
    rip.offset.y = (rip.offset.y + dt * 0.007) % 1;
  });
  return (
    <mesh ref={mesh} geometry={geometry} position={[0, WATER_Y, 0]} renderOrder={1} receiveShadow>
      <meshStandardMaterial map={depth.map} normalMap={rip} normalScale={new Vector2(0.7, 0.7)} roughness={0.08} metalness={0} envMap={env} envMapIntensity={1.0} side={DoubleSide} transparent opacity={0.97} />
    </mesh>
  );
}

export function Lakes({ hole }: { hole: Hole }) {
  const ripple = useMemo(() => waterRipple(), []);
  const hasWater = hole.water.length > 0;
  // 수면이 비추는 하늘: 하늘색 위로 갈수록 파랗고, 지평선 쪽은 맞은편 숲이 비치는 것처럼 어둡게 (물이 없는 홀은 만들지 않는다)
  const env = useMemo(() => {
    if (!hasWater) return null;
    const t = createSkyEnvironmentTexture('#3e78c4', '#6d8187', '#1c2c20');
    t.mapping = EquirectangularReflectionMapping;
    t.needsUpdate = true;
    return t;
  }, [hasWater]);
  useEffect(() => () => env?.dispose(), [env]);
  return (
    <group name="lakes">
      {env && hole.water.map((poly, i) => <Lake key={`${hole.number}-${i}`} poly={poly} ripple={ripple} env={env} />)}
    </group>
  );
}
