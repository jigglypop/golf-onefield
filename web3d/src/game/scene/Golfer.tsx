/**
 * 캐릭터(gaesup-world GaesupController)를 공 옆 어드레스 자리에 세우고, 스윙할 때 클럽을 휘둘러 임팩트에 맞춰 공을 보낸다.
 * 오른손 골퍼는 조준선 왼쪽에 서서 공을 본다 (glTF 정면 +Z → y 축 회전 −yaw 이면 오른쪽 방향을 본다).
 *
 * 자리: 공을 새로 놓을 때와 스윙을 시작할 때 teleport 하고, 조준을 돌리는 동안에는 강체를 직접 옮겨 따라간다.
 * gaesup-world 1.7.0 의 useTeleport 는 상태(activeState)만 바꾸고 물리 강체는 옮기지 않아서, 자리는 강체를 직접 옮기고
 * (mogaesup games/teleport.ts 와 같은 방법: 이동 → 속도 0 → 깨우기) 바라보는 방향만 useTeleport 로 넘긴다.
 *
 * 스윙(절차 생성, 애니메이션 클립 없음): 어깨 높이의 피벗 P 를 중심으로 클럽이 반지름 L 의 원을 그린다.
 *   $\mathbf h(\varphi) = \mathbf P + L(\cos\varphi\,\hat u + \sin\varphi\,\hat f)$, $\hat u$ = 피벗에서 공 쪽, $\hat f$ = 조준 방향.
 *   $\varphi$ 곡선과 임팩트 시각은 game/swing.ts: 샷을 정한 뒤 0.44 s 에 $\varphi = 0$(임팩트)이고 그때 공이 떠난다(shotClock = 0).
 * 몸통은 activeState.euler.y 를 직접 돌려 비튼다 (컨트롤러가 매 프레임 이 값으로 모델 회전을 정한다).
 */
import { useEffect, useMemo, useRef } from 'react';

import { useEngineFrame, useStateSystem, useTeleport } from 'gaesup-world';
import { Euler, Matrix4, Vector3, type Group, type Mesh } from 'three';

import { playerBody } from '../../scene/player.ts';
import { CLUBS } from '../clubs.ts';
import { game, shotClock, useGame } from '../round.ts';
import { clubLengthM, swingPose, swingTotalS } from '../swing.ts';

const STILL = { x: 0, y: 0, z: 0 };
/** 골퍼 몸 기준점이 공에서 떨어진 거리: 풀스윙 / 퍼터 */
const GAP = { full: 1.05, putt: 0.75 } as const;
/** 스윙 피벗(어깨) 높이 */
const PIVOT_Y = { full: 1.3, putt: 1.0 } as const;
/** 어드레스·임팩트에서 헤드 중심이 공 중심보다 목표 반대쪽으로 떨어진 거리 (공 반지름 0.021 + 헤드 두께 절반 0.0175) */
const FACE_BACK = 0.0385;
/** 팔로스루가 끝난 뒤 클럽을 숨기기까지 (s) */
const HIDE_AFTER_S = 0.35;

/** 몸 기준점 (공에서 조준선 왼쪽 gap) */
function bodyAt(ball: { x: number; z: number }, yaw: number, gap: number): { x: number; z: number } {
  // 왼쪽 = (sin yaw, −cos yaw)
  return { x: ball.x + Math.sin(yaw) * gap, z: ball.z - Math.cos(yaw) * gap };
}

export function Golfer() {
  const { teleport, canTeleport } = useTeleport();
  const { activeState } = useStateSystem();
  const ballId = useGame((s) => s.ballId);
  const swinging = useGame((s) => s.phase === 'swing');
  // 홀 소개가 끝나 조준으로 들어설 때도 옮긴다 (소개 중에는 공 번호가 바뀌어도 건너뛰므로)
  const aiming = useGame((s) => s.phase === 'aim' || s.phase === 'swing');
  const rig = useRef<Group>(null);
  const arm = useRef<Group>(null);
  const shaft = useRef<Mesh>(null);
  const head = useRef<Mesh>(null);
  const scratch = useMemo(() => ({ f: new Vector3(), u: new Vector3(), y: new Vector3(), z: new Vector3(), m: new Matrix4(), yawBody: NaN, x: NaN, zz: NaN }), []);

  useEffect(() => {
    if (!canTeleport) return;
    const s = game.get();
    if (s.phase !== 'aim' && s.phase !== 'swing') return;
    const yaw = s.aimYaw;
    const gap = CLUBS[s.clubIdx]!.putter ? GAP.putt : GAP.full;
    const { x, z } = bodyAt(s.ball, yaw, gap);
    const body = playerBody.current;
    if (body) {
      try {
        body.setTranslation({ x, y: 0.95, z }, true);
        body.setLinvel(STILL, true);
        body.setAngvel(STILL, true);
        body.wakeUp();
      } catch {
        // 캔버스를 다시 만들며 물리 세계가 놓아 버린 강체
      }
    }
    scratch.x = x;
    scratch.zz = z;
    scratch.yawBody = yaw;
    teleport(new Vector3(x, 0.95, z), new Euler(0, -yaw, 0), { effect: false, dropHeight: 0 });
  }, [ballId, swinging, aiming, canTeleport, teleport, scratch]);

  useEngineFrame(
    'lateUpdate',
    () => {
      const g = rig.current;
      const a = arm.current;
      if (!g || !a) return;
      const s = game.get();
      const shot = s.shot;
      const inShot = (s.phase === 'flight' || s.phase === 'ground') && shot !== null;
      const inAim = s.phase === 'aim' || s.phase === 'swing';
      if (!inAim && !inShot) {
        g.visible = false;
        return;
      }
      const putter = (inShot ? CLUBS[shot.clubIdx] : CLUBS[s.clubIdx])!.putter === true;
      const clubIdx = inShot ? shot.clubIdx : s.clubIdx;
      const ball = inShot ? shot.origin : s.ball;
      const yaw = inShot ? shot.yaw : s.aimYaw;
      const power = inShot ? (shot.swing?.power ?? 1) : 1;
      // 샷 시작부터 흐른 시간: 임팩트에서 shotClock = 0 이므로 τ = shotClock + impactDelay
      const tau = inShot ? shotClock.t + shot.impactDelay : 0;
      if (inShot && tau > swingTotalS(putter) + HIDE_AFTER_S) {
        g.visible = false;
        return;
      }
      const gap = putter ? GAP.putt : GAP.full;
      const body = bodyAt(ball, yaw, gap);
      // 조준을 돌리면 강체도 따라간다 (스윙이 시작된 뒤에는 그 자리에 둔다)
      if (inAim && (Math.abs(yaw - scratch.yawBody) > 1e-4 || Math.hypot(body.x - scratch.x, body.z - scratch.zz) > 1e-3 || Number.isNaN(scratch.yawBody))) {
        const rb = playerBody.current;
        if (rb) {
          try {
            rb.setTranslation({ x: body.x, y: 0.95, z: body.z }, true);
            rb.setLinvel(STILL, true);
            rb.setAngvel(STILL, true);
          } catch {
            // 놓아 버린 강체
          }
        }
        activeState.euler.y = -yaw;
        scratch.x = body.x;
        scratch.zz = body.z;
        scratch.yawBody = yaw;
      }
      const pose = swingPose(tau, power, putter);
      if (inShot) activeState.euler.y = -yaw + pose.twist;
      // 피벗 P, 공 뒤 헤드 자리까지 단위 벡터 û, 원 반지름 L
      const f = scratch.f.set(Math.cos(yaw), 0, Math.sin(yaw));
      const py = putter ? PIVOT_Y.putt : PIVOT_Y.full;
      const tx = ball.x - f.x * FACE_BACK - body.x;
      const ty = 0.03 - py;
      const tz = ball.z - f.z * FACE_BACK - body.z;
      const L = Math.hypot(tx, ty, tz);
      scratch.u.set(tx / L, ty / L, tz / L);
      scratch.y.copy(scratch.u).negate();
      scratch.z.crossVectors(f, scratch.y);
      scratch.m.makeBasis(f, scratch.y, scratch.z);
      g.position.set(body.x, py, body.z);
      g.quaternion.setFromRotationMatrix(scratch.m);
      // φ = 0 이면 헤드 페이스가 공에 닿는다: 어드레스 때와 임팩트(τ = B + D) 때 같은 자리
      a.rotation.z = pose.phi;
      const len = clubLengthM(clubIdx, putter);
      if (shaft.current) {
        shaft.current.scale.set(1, len, 1);
        shaft.current.position.y = -(L - len / 2);
      }
      if (head.current) head.current.position.y = -L;
      g.visible = true;
    },
    { label: 'golf:golfer-swing' },
  );

  return (
    <group ref={rig} visible={false} name="golf-club-rig" userData={{ intangible: true }}>
      <group ref={arm}>
        <mesh ref={shaft} castShadow>
          <cylinderGeometry args={[0.007, 0.006, 1, 8]} />
          <meshStandardMaterial color="#c9ced3" metalness={0.8} roughness={0.35} />
        </mesh>
        <mesh ref={head} castShadow>
          <boxGeometry args={[0.035, 0.05, 0.1]} />
          <meshStandardMaterial color="#2b2e33" metalness={0.5} roughness={0.4} />
        </mesh>
      </group>
    </group>
  );
}
