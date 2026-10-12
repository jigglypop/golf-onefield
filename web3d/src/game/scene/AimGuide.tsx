/**
 * 조준 안내 (조준·스윙 중에만):
 *   풀스윙 샷 — 지금 클럽·라이·바람으로 파워 100 %·정타일 때의 궤적(반투명 선), 낙하 고리, 캐리 글자판.
 *   퍼팅 — 조준선과 그린 경사 화살표(내리막 쪽, 길이는 기울기에 비례).
 */
import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';

import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { DoubleSide, Object3D, Quaternion, RingGeometry, Vector3, type InstancedMesh } from 'three';

import { resample } from '../../engine/flight.ts';
import { M_PER_YD } from '../../engine/units.ts';
import { Label } from '../../scene/Label.tsx';
import { Tracer } from '../../scene/tracer.ts';
import { CLUBS, launchFor } from '../clubs.ts';
import { inEllipse } from '../course.ts';
import { flyFrom, holeOf, useGame } from '../round.ts';
import { BALL_LIFT } from './HoleScene.tsx';

function FullShotPreview() {
  const ballX = useGame((s) => s.ball.x);
  const ballZ = useGame((s) => s.ball.z);
  const surface = useGame((s) => s.ball.surface);
  const clubIdx = useGame((s) => s.clubIdx);
  const yaw = useGame((s) => s.aimYaw);
  const wind = useGame((s) => s.wind);
  const get = useThree((s) => s.get);
  const preview = useMemo(() => {
    const input = launchFor(CLUBS[clubIdx]!, { power: 1, accuracy: 0 }, surface);
    const { result, samples } = flyFrom(input, { x: ballX, z: ballZ }, yaw, wind);
    if (!result.landed) return null;
    const end = samples[samples.length - 1]!;
    return { tracer: new Tracer(resample(samples, 1 / 60).xyz), end, carryYd: Math.hypot(end.x - ballX, end.z - ballZ) / M_PER_YD };
  }, [ballX, ballZ, surface, clubIdx, yaw, wind]);
  useEffect(() => () => preview?.tracer.dispose(), [preview]);
  const ring = useMemo(() => new RingGeometry(1.6, 2.3, 48).rotateX(-Math.PI / 2), []);
  useEffect(() => () => ring.dispose(), [ring]);
  const cam = useMemo(() => new Vector3(), []);
  useEngineFrame(
    'lateUpdate',
    () => {
      if (!preview) return;
      const three = get();
      const c = three.camera as typeof three.camera & { fov?: number };
      const fov = ((c.fov ?? 50) * Math.PI) / 180;
      const px = ((2 * Math.tan(fov / 2)) / Math.max(1, three.size.height)) * 1.1;
      cam.copy(c.position);
      preview.tracer.update(cam, preview.tracer.count - 1, null, px, 0.008, 1.5);
    },
    { label: 'golf:aim-preview' },
  );
  if (!preview) return null;
  return (
    <group name="golf-aim-preview" userData={{ intangible: true }}>
      <mesh geometry={preview.tracer.geometry} position={[0, BALL_LIFT, 0]} frustumCulled={false} renderOrder={2}>
        <meshBasicMaterial color="#ffffff" toneMapped={false} transparent opacity={0.55} side={DoubleSide} depthWrite={false} />
      </mesh>
      <mesh geometry={ring} position={[preview.end.x, 0.02, preview.end.z]} renderOrder={3}>
        <meshBasicMaterial color="#ffffff" toneMapped={false} transparent opacity={0.8} depthWrite={false} polygonOffset polygonOffsetFactor={-8} polygonOffsetUnits={-10} />
      </mesh>
      <Label lines={[{ text: `풀스윙 ${Math.round(preview.carryYd)} yd`, size: 0.42 }]} position={[preview.end.x, 4.2, preview.end.z]} height={2.6} w={460} h={130} fog={false} renderOrder={4} />
    </group>
  );
}

function PuttGuide() {
  const ballX = useGame((s) => s.ball.x);
  const ballZ = useGame((s) => s.ball.z);
  const yaw = useGame((s) => s.aimYaw);
  const hole = useGame(holeOf);
  const arrows = useRef<InstancedMesh>(null);
  const spots = useMemo(() => {
    const out: [number, number][] = [];
    const g = hole.green;
    const R = Math.max(g.rx, g.rz) + 2;
    for (let x = g.c[0] - R; x <= g.c[0] + R; x += 2) for (let z = g.c[1] - R; z <= g.c[1] + R; z += 2) if (inEllipse(g, x, z, -0.6)) out.push([x, z]);
    return out;
  }, [hole]);
  useLayoutEffect(() => {
    const m = arrows.current;
    if (!m) return;
    const [sx, sz] = hole.green.slope;
    const mag = Math.hypot(sx, sz);
    const dir = new Vector3(-sx, 0, -sz).normalize();
    const q = new Quaternion().setFromUnitVectors(new Vector3(0, 1, 0), dir);
    const o = new Object3D();
    const len = 0.18 + mag * 12;
    spots.forEach(([x, z], i) => {
      o.position.set(x, 0.012, z);
      o.quaternion.copy(q);
      o.scale.set(0.035, len, 0.035);
      o.updateMatrix();
      m.setMatrixAt(i, o.matrix);
    });
    m.instanceMatrix.needsUpdate = true;
    m.computeBoundingSphere();
  }, [spots, hole]);
  const c = Math.cos(yaw);
  const s = Math.sin(yaw);
  const LINE = 7;
  return (
    <group name="golf-putt-guide" userData={{ intangible: true }}>
      <mesh position={[ballX + (c * LINE) / 2, 0.012, ballZ + (s * LINE) / 2]} rotation={[0, -yaw, 0]} renderOrder={5}>
        <boxGeometry args={[LINE, 0.002, 0.018]} />
        <meshBasicMaterial color="#ffffff" toneMapped={false} transparent opacity={0.9} depthWrite={false} depthTest={false} />
      </mesh>
      <instancedMesh key={hole.number} ref={arrows} args={[undefined, undefined, spots.length]} renderOrder={4}>
        <coneGeometry args={[1, 1, 3]} />
        <meshBasicMaterial color="#ffffff" toneMapped={false} transparent opacity={0.2} depthWrite={false} depthTest={false} />
      </instancedMesh>
    </group>
  );
}

export function AimGuide() {
  const phase = useGame((s) => s.phase);
  const putter = useGame((s) => CLUBS[s.clubIdx]!.putter === true);
  if (phase !== 'aim' && phase !== 'swing') return null;
  return putter ? <PuttGuide /> : <FullShotPreview />;
}
