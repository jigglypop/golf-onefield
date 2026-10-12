/**
 * 시각 점검용 창 (scripts/shots.mjs · 콘솔): 공 놓기, 카메라 직접 놓기, 렌더 통계.
 * 게임 규칙·물리는 건드리지 않는다. 공을 옮길 때는 round.ts 의 상태만 바꾼다.
 */
import { surfaceAt, yawTo } from '../game/course.ts';
import { game, holeOf } from '../game/round.ts';

export interface CamOverride {
  pos: [number, number, number];
  look: [number, number, number];
  fov?: number;
}

/** PhotoRig 가 읽는다 */
export const camOverride: { current: CamOverride | null } = { current: null };
/** 매 프레임 PhotoStats 가 채운다 */
export const renderStats: { current: Record<string, number | string> } = { current: {} };

export function placeBall(x: number, z: number): void {
  const s = game.get();
  const h = holeOf(s);
  const ball = { x, z, surface: surfaceAt(h, x, z) };
  game.set({ ball, ballId: s.ballId + 1, aimYaw: yawTo({ x, z }, h.pin) });
}

/** 점검용: R3F 장면 (콘솔에서 `__golf.vis.scene` 로 연다) */
export const sceneRef: { current: unknown } = { current: null };

export const visApi = {
  get scene() {
    return sceneRef.current;
  },
  cam(o: CamOverride | null) {
    camOverride.current = o;
  },
  stats: () => renderStats.current,
};
