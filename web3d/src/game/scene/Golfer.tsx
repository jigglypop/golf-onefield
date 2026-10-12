/**
 * 캐릭터(gaesup-world GaesupController)를 공 옆 어드레스 자리로 옮긴다: 공을 새로 놓을 때와 스윙을 시작할 때.
 * 오른손 골퍼는 조준선 왼쪽에 서서 공을 본다 (glTF 정면 +Z → y 축 회전 −yaw 이면 오른쪽 방향을 본다).
 *
 * gaesup-world 1.7.0 의 useTeleport 는 상태(activeState)만 바꾸고 물리 강체는 옮기지 않아서, 자리는 강체를 직접 옮기고
 * (mogaesup games/teleport.ts 와 같은 방법: 이동 → 속도 0 → 깨우기) 바라보는 방향만 useTeleport 로 넘긴다.
 */
import { useEffect } from 'react';

import { useTeleport } from 'gaesup-world';
import { Euler, Vector3 } from 'three';

import { playerBody } from '../../scene/player.ts';
import { CLUBS } from '../clubs.ts';
import { game, useGame } from '../round.ts';

const STILL = { x: 0, y: 0, z: 0 };

export function Golfer() {
  const { teleport, canTeleport } = useTeleport();
  const ballId = useGame((s) => s.ballId);
  const swinging = useGame((s) => s.phase === 'swing');
  // 홀 소개가 끝나 조준으로 들어설 때도 옮긴다 (소개 중에는 공 번호가 바뀌어도 건너뛰므로)
  const aiming = useGame((s) => s.phase === 'aim' || s.phase === 'swing');
  useEffect(() => {
    if (!canTeleport) return;
    const s = game.get();
    if (s.phase !== 'aim' && s.phase !== 'swing') return;
    const yaw = s.aimYaw;
    const gap = CLUBS[s.clubIdx]!.putter ? 0.75 : 1.05;
    // 왼쪽 = (sin yaw, −cos yaw)
    const x = s.ball.x + Math.sin(yaw) * gap;
    const z = s.ball.z - Math.cos(yaw) * gap;
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
    teleport(new Vector3(x, 0.95, z), new Euler(0, -yaw, 0), { effect: false, dropHeight: 0 });
  }, [ballId, swinging, aiming, canTeleport, teleport]);
  return null;
}
