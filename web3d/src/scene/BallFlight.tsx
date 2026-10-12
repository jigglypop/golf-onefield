/**
 * 공·궤적 선·착지 표시. 비행 시각은 프레임마다 flightClock 에서 늘리고(배속 반영), 공 위치는 엔진 표본을
 * 시간 에르미트로 이어 구한다. 엔진 프레임 단계 'lateUpdate' 에서 돌아 카메라('camera' 단계)보다 먼저 움직인다.
 */
import { useEffect, useMemo, useRef } from 'react';

import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { DoubleSide, RingGeometry, Vector3, type Group, type Mesh } from 'three';

import { sideLabel } from '../engine/convention.ts';
import { positionAt } from '../engine/flight.ts';
import { R_BALL } from '../engine/units.ts';
import { flightClock, store, useApp } from '../state.ts';
import { Label } from './Label.tsx';
import { COLORS } from './layout.ts';
import { Tracer } from './tracer.ts';

/** '공 크게' 배율 */
export const BIG_BALL = 6;
/** 궤적 선 반지름 (화면 px) */
const TRACER_PX = 1.7;
/** 그리기용 경로 간격 (state.ts resample 과 같음) */
const PATH_HZ = 120;

export const ballRadius = (big: boolean): number => R_BALL * (big ? BIG_BALL : 1);

/** 공의 화면 위치 (카메라가 읽는다): 엔진 좌표 + 반지름만큼 위 */
export const ballWorld = new Vector3(0, R_BALL, 0);

export function BallFlight() {
  const shot = useApp((s) => s.shot);
  const get = useThree((s) => s.get);
  const ball = useRef<Mesh>(null);
  const trail = useRef<Group>(null);
  const tracer = useMemo(() => (shot ? new Tracer(shot.path.xyz) : null), [shot]);
  useEffect(() => () => tracer?.dispose(), [tracer]);
  const scratch = useMemo(() => ({ p: { x: 0, y: 0, z: 0 }, head: new Vector3(), cam: new Vector3(), hint: 0 }), []);

  useEngineFrame(
    'lateUpdate',
    (delta) => {
      const st = store.get();
      const r = ballRadius(st.bigBall);
      const s = st.shot;
      const p = scratch.p;
      if (!s) {
        p.x = p.y = p.z = 0;
      } else {
        if (st.phase === 'flying') {
          flightClock.t = Math.min(flightClock.t + Math.min(delta, 0.1) * st.speed, s.result.timeS);
          if (flightClock.t >= s.result.timeS) store.set({ phase: 'landed' });
        }
        scratch.hint = positionAt(s.result.samples, flightClock.t, p, scratch.hint);
      }
      ballWorld.set(p.x, p.y + r, p.z);
      if (ball.current) {
        ball.current.position.copy(ballWorld);
        ball.current.scale.setScalar(r);
      }
      if (tracer && trail.current) {
        trail.current.position.y = r;
        const three = get();
        const cam = three.camera as typeof three.camera & { fov?: number };
        const fov = ((cam.fov ?? 50) * Math.PI) / 180;
        const pxScale = ((2 * Math.tan(fov / 2)) / Math.max(1, three.size.height)) * TRACER_PX;
        scratch.cam.copy(cam.position);
        scratch.cam.y -= r;
        const landed = st.phase === 'landed';
        const upto = landed ? tracer.count - 1 : Math.floor(flightClock.t * PATH_HZ);
        tracer.update(scratch.cam, upto, landed ? null : scratch.head.set(p.x, p.y, p.z), pxScale);
      }
    },
    { label: 'golf:ball' },
  );

  return (
    <group name="golf-ball" userData={{ intangible: true }}>
      <mesh ref={ball} position={[0, R_BALL, 0]} castShadow>
        <sphereGeometry args={[1, 24, 16]} />
        <meshStandardMaterial color="#ffffff" roughness={0.35} emissive="#ffffff" emissiveIntensity={0.15} />
      </mesh>
      {tracer && (
        <group ref={trail}>
          <mesh geometry={tracer.geometry} frustumCulled={false} renderOrder={2}>
            <meshBasicMaterial color={COLORS.tracer} toneMapped={false} side={DoubleSide} />
          </mesh>
        </group>
      )}
      <LandingMarker />
    </group>
  );
}

function LandingMarker() {
  const shot = useApp((s) => s.shot);
  const phase = useApp((s) => s.phase);
  const ring = useMemo(() => new RingGeometry(1.1, 1.6, 48).rotateX(-Math.PI / 2), []);
  useEffect(() => () => ring.dispose(), [ring]);
  if (!shot || phase !== 'landed') return null;
  const end = shot.result.samples[shot.result.samples.length - 1]!;
  const { carryYd, lateralYd } = shot.result;
  return (
    <group position={[end.x, 0, end.z]}>
      <mesh geometry={ring} position={[0, 0.09, 0]}>
        <meshBasicMaterial color={COLORS.landing} toneMapped={false} polygonOffset polygonOffsetFactor={-4} polygonOffsetUnits={-8} />
      </mesh>
      <mesh position={[0, 1.6, 0]}>
        <cylinderGeometry args={[0.04, 0.04, 3.2, 8]} />
        <meshBasicMaterial color={COLORS.landing} toneMapped={false} />
      </mesh>
      <Label
        lines={[
          { text: `캐리 ${carryYd.toFixed(1)} yd`, size: 0.4, color: '#ffe680' },
          { text: `좌우 ${sideLabel(lateralYd)} yd`, size: 0.26 },
        ]}
        position={[0, 5.4, 0]}
        height={3.2}
        w={520}
        h={220}
        fog={false}
        renderOrder={3}
      />
    </group>
  );
}
