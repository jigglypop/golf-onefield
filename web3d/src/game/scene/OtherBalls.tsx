/** 다른 사람의 공 (스트로크 여럿일 때): 색 공 + 이름 글자판. 지금 차례의 공은 GameBall 이 그린다 */
import { Label } from '../../scene/Label.tsx';
import { useGame } from '../round.ts';
import { BALL_LIFT } from './HoleScene.tsx';

export function OtherBalls() {
  const s = useGame((x) => x);
  if (s.mode !== 'stroke' || s.players.length < 2 || s.phase === 'menu' || s.phase === 'intro') return null;
  return (
    <group name="golf-other-balls" userData={{ intangible: true }}>
      {s.players.map((p, i) => {
        // 지금 공 바로 옆(티를 같이 쓰는 등)은 숨기고, 이름표는 30 m 넘게 떨어졌을 때만
        const d = Math.hypot(p.ball.x - s.ball.x, p.ball.z - s.ball.z);
        if (i === s.cur || p.done || d < 8) return null;
        return (
          <group key={i} position={[p.ball.x, 0, p.ball.z]}>
            <mesh position={[0, BALL_LIFT + 0.08, 0]}>
              <sphereGeometry args={[0.08, 16, 12]} />
              <meshStandardMaterial color={p.color} emissive={p.color} emissiveIntensity={0.35} />
            </mesh>
            {d > 30 && <Label lines={[{ text: p.name, size: 0.5, color: p.color, weight: 800 }]} position={[0, 2.2, 0]} height={1.4} w={300} h={110} fog={false} />}
          </group>
        );
      })}
    </group>
  );
}
