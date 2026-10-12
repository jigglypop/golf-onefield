/**
 * 미니맵 (SVG): 홀 배치(경계·페어웨이·벙커·물·섬·그린), 핀, 모든 사람의 공, 지금 조준선(풀스윙 캐리 길이).
 * 티에서 핀 방향이 화면 위쪽이 되도록 돌린다.
 */
import { useMemo } from 'react';

import { M_PER_YD } from '../../engine/units.ts';
import { CLUBS, fullCarryYd } from '../clubs.ts';
import { ellipsePoints, fairwayOutline, FRINGE_M, yawTo, type Hole, type Vec2 } from '../course.ts';
import { holeOf, useGame } from '../round.ts';

const W = 170;
const H = 250;
const PAD = 8;

function useProjection(h: Hole) {
  return useMemo(() => {
    const theta = yawTo(h.tee, h.pin);
    const a = -Math.PI / 2 - theta;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const rot = ([x, z]: Vec2): [number, number] => [x * ca - z * sa, x * sa + z * ca];
    const pts = h.bounds.map(rot);
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const k = Math.min((W - 2 * PAD) / (x1 - x0), (H - 2 * PAD) / (y1 - y0));
    const ox = (W - k * (x1 - x0)) / 2 - k * x0;
    const oy = (H - k * (y1 - y0)) / 2 - k * y0;
    const map = (p: Vec2): [number, number] => {
      const [u, v] = rot(p);
      return [ox + k * u, oy + k * v];
    };
    const path = (poly: readonly Vec2[]) => `M${poly.map((p) => map(p).map((n) => n.toFixed(1)).join(',')).join('L')}Z`;
    return { map, path, k };
  }, [h]);
}

export function MiniMap() {
  const h = useGame(holeOf);
  const phase = useGame((s) => s.phase);
  const ball = useGame((s) => s.ball);
  const players = useGame((s) => s.players);
  const cur = useGame((s) => s.cur);
  const yaw = useGame((s) => s.aimYaw);
  const clubIdx = useGame((s) => s.clubIdx);
  const { map, path, k } = useProjection(h);
  const shapes = useMemo(
    () => ({
      bounds: path(h.bounds),
      fairways: h.fairways.map((f) => path(fairwayOutline(f, 6))),
      fringe: path(ellipsePoints(h.green, FRINGE_M, 32)),
      green: path(ellipsePoints(h.green, 0, 32)),
      bunkers: h.bunkers.map((b) => path(ellipsePoints(b, 0, 20))),
      water: h.water.map((w) => path(w)),
      land: (h.land ?? []).map((l) => path(l)),
    }),
    [h, path],
  );
  if (phase === 'menu') return null;
  const [bx, by] = map([ball.x, ball.z]);
  const carryM = CLUBS[clubIdx]!.putter ? 0 : fullCarryYd(CLUBS[clubIdx]!) * M_PER_YD;
  const [ax, ay] = map([ball.x + Math.cos(yaw) * carryM, ball.z + Math.sin(yaw) * carryM]);
  const [px, py] = map(h.pin);
  const aiming = phase === 'aim' || phase === 'swing';
  return (
    <svg className="g-minimap" width={W} height={H} viewBox={`0 0 ${W} ${H}`} role="img" aria-label="홀 미니맵" data-testid="g-minimap">
      <path d={shapes.bounds} fill="#4f7f33" stroke="rgba(255,255,255,0.5)" strokeWidth={1} strokeDasharray="3 2" />
      {shapes.water.map((d, i) => (
        <path key={`w${i}`} d={d} fill="#3a86c8" />
      ))}
      {shapes.land.map((d, i) => (
        <path key={`l${i}`} d={d} fill="#6f9d45" />
      ))}
      {shapes.fairways.map((d, i) => (
        <path key={`f${i}`} d={d} fill="#86c556" />
      ))}
      <path d={shapes.fringe} fill="#7cbf54" />
      <path d={shapes.green} fill="#a8e27e" />
      {shapes.bunkers.map((d, i) => (
        <path key={`b${i}`} d={d} fill="#e2cf9b" />
      ))}
      {h.kind !== 'longdrive' && (
        <g>
          <circle cx={px} cy={py} r={2} fill="#111" />
          <line x1={px} y1={py} x2={px} y2={py - 9} stroke="#fff" strokeWidth={1} />
          <path d={`M${px},${py - 9}l6,2l-6,2z`} fill="#e5484d" />
        </g>
      )}
      {aiming && carryM > 0 && <line x1={bx} y1={by} x2={ax} y2={ay} stroke="#fff" strokeWidth={1.2} strokeDasharray="3 2" opacity={0.85} />}
      {aiming && carryM > 0 && <circle cx={ax} cy={ay} r={Math.max(2.5, 8 * k)} fill="none" stroke="#fff" strokeWidth={1} opacity={0.85} />}
      {players.map((p, i) => {
        if (i === cur || p.done) return null;
        const [x, y] = map([p.ball.x, p.ball.z]);
        return <circle key={i} cx={x} cy={y} r={3} fill={p.color} stroke="#000" strokeWidth={0.6} />;
      })}
      <circle cx={bx} cy={by} r={3.6} fill={players[cur]?.color ?? '#fff'} stroke="#fff" strokeWidth={1.2} />
    </svg>
  );
}
