/**
 * 공: 놓인 자리 → 비행(엔진 표본을 시간 에르미트로) → 바운드·굴림(지면 표본을 직선으로) → 홀컵.
 * 프레임마다 라운드 시계를 늘리고(advanceShot) 단계가 넘어가면 store 가 바뀐다.
 * 멀리서도 보이게 공을 카메라 거리에 비례해 키우되(최대 6배), 가까이서는 실제 크기다.
 */
import { useEffect, useMemo, useRef } from 'react';

import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { DoubleSide, Vector3, type Group, type Mesh } from 'three';

import { positionAt } from '../../engine/flight.ts';
import { R_BALL } from '../../engine/units.ts';
import { COLORS } from '../../scene/layout.ts';
import { Tracer } from '../../scene/tracer.ts';
import type { GroundSample } from '../ground.ts';
import { advanceShot, game, holeOf, shotClock, useGame } from '../round.ts';
import { BALL_LIFT } from './HoleScene.tsx';

/** 공의 월드 위치 (중심). 카메라·조준선이 읽는다 */
export const ballPos = new Vector3();
const TRACER_PX = 1.7;

function groundAt(samples: readonly GroundSample[], t: number, out: Vector3, hint: { i: number }): void {
  const n = samples.length;
  if (n === 0) return;
  if (t <= samples[0]!.t) {
    const p = samples[0]!;
    out.set(p.x, p.y, p.z);
    return;
  }
  if (t >= samples[n - 1]!.t) {
    const p = samples[n - 1]!;
    out.set(p.x, p.y, p.z);
    return;
  }
  let i = Math.min(Math.max(hint.i, 0), n - 2);
  if (samples[i]!.t > t) i = 0;
  while (i < n - 2 && samples[i + 1]!.t < t) i++;
  hint.i = i;
  const a = samples[i]!;
  const b = samples[i + 1]!;
  const u = (t - a.t) / Math.max(b.t - a.t, 1e-9);
  out.set(a.x + (b.x - a.x) * u, a.y + (b.y - a.y) * u, a.z + (b.z - a.z) * u);
}

export function GameBall() {
  const shot = useGame((s) => s.shot);
  const get = useThree((s) => s.get);
  const ball = useRef<Mesh>(null);
  const trail = useRef<Group>(null);
  const tracer = useMemo(() => (shot?.path ? new Tracer(shot.path) : null), [shot]);
  // 공이 컵 바로 옆을 지나가는 샷인가 (지나갈 때 슬로모션)
  const nearCup = useMemo(() => {
    if (!shot) return false;
    const [px, pz] = holeOf().pin;
    return shot.ground.samples.some((p) => Math.hypot(p.x - px, p.z - pz) < 0.35);
  }, [shot]);
  useEffect(() => () => tracer?.dispose(), [tracer]);
  const scratch = useMemo(() => ({ p: { x: 0, y: 0, z: 0 }, cam: new Vector3(), head: new Vector3(), hint: 0, gh: { i: 0 } }), []);

  useEngineFrame(
    'lateUpdate',
    (delta) => {
      const s0 = game.get();
      const [px, pz] = holeOf(s0).pin;
      const slow = nearCup && s0.phase === 'ground' && Math.hypot(ballPos.x - px, ballPos.z - pz) < 1.6 ? 0.35 : 1;
      advanceShot(delta * slow);
      const s = game.get();
      const sh = s.shot;
      const t = shotClock.t;
      if (sh && (s.phase === 'flight' || s.phase === 'ground')) {
        if (s.phase === 'flight' && sh.flightSamples.length > 0) {
          scratch.hint = positionAt(sh.flightSamples, t, scratch.p, scratch.hint);
          ballPos.set(scratch.p.x, scratch.p.y, scratch.p.z);
        } else {
          groundAt(sh.ground.samples, t - sh.tFlight, ballPos, scratch.gh);
        }
      } else if (s.phase === 'holed') {
        ballPos.set(s.ball.x, -0.05, s.ball.z);
      } else {
        ballPos.set(s.ball.x, 0, s.ball.z);
        scratch.hint = 0;
        scratch.gh.i = 0;
      }
      const three = get();
      const cam = three.camera as typeof three.camera & { fov?: number };
      const dist = cam.position.distanceTo(ballPos);
      const scale = Math.min(Math.max(dist * 0.018, 1), 6);
      const r = R_BALL * scale;
      if (ball.current) {
        ball.current.position.set(ballPos.x, ballPos.y + BALL_LIFT + r, ballPos.z);
        ball.current.scale.setScalar(r);
      }
      if (trail.current) trail.current.visible = s.phase === 'flight' || s.phase === 'ground' || s.phase === 'holed';
      if (tracer && trail.current && sh) {
        trail.current.position.y = BALL_LIFT + R_BALL;
        const fov = ((cam.fov ?? 50) * Math.PI) / 180;
        const pxScale = ((2 * Math.tan(fov / 2)) / Math.max(1, three.size.height)) * TRACER_PX;
        scratch.cam.copy(cam.position);
        const flying = s.phase === 'flight' && s.shot?.id === sh.id;
        const upto = flying ? Math.floor(t * 120) : tracer.count - 1;
        tracer.update(scratch.cam, upto, flying ? scratch.head.set(ballPos.x, ballPos.y, ballPos.z) : null, pxScale);
      }
    },
    { label: 'golf:game-ball' },
  );

  return (
    <group name="golf-game-ball" userData={{ intangible: true }}>
      <mesh ref={ball} castShadow>
        <sphereGeometry args={[1, 24, 16]} />
        <meshStandardMaterial color="#ffffff" roughness={0.35} emissive="#ffffff" emissiveIntensity={0.18} />
      </mesh>
      {tracer && (
        <group ref={trail}>
          <mesh geometry={tracer.geometry} frustumCulled={false} renderOrder={2}>
            <meshBasicMaterial color={COLORS.tracer} toneMapped={false} side={DoubleSide} transparent opacity={0.95} />
          </mesh>
        </group>
      )}
    </group>
  );
}
