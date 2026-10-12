/**
 * 앱 상태: 작은 외부 store (useSyncExternalStore). 프레임마다 바뀌는 비행 시각은 React 상태 밖(flightClock)에 둔다.
 */
import { useSyncExternalStore } from 'react';

import { clampInput, toEngineLaunch, type ShotInput } from './engine/convention.ts';
import { resample, simulate, type FlightResult } from './engine/flight.ts';
import { DEFAULT_INPUT } from './engine/presets.ts';

export type Phase = 'idle' | 'flying' | 'landed';
/** auto: 티 뒤 → 공 따라가기 → 착지 보기. player: gaesup-world 3인칭 카메라(캐릭터) */
export type CameraMode = 'auto' | 'behind' | 'side' | 'top' | 'player';
export type Speed = 0.5 | 1 | 2;

export interface Shot {
  id: number;
  input: ShotInput;
  result: FlightResult;
  /** 그리기용: 1/120 s 간격 점 (m) */
  path: { t: Float64Array; xyz: Float32Array };
}

export interface AppState {
  input: ShotInput;
  shot: Shot | null;
  phase: Phase;
  cameraMode: CameraMode;
  speed: Speed;
  bigBall: boolean;
  /** 핀 거리 (yd) */
  pinYd: number;
  /** 실제로 그리는 렌더러 */
  renderer: string;
  /** 붙여넣기 결과·경고 */
  notice: string | null;
  /** 친 샷 수 */
  shots: number;
}

type Listener = () => void;

export function createStore<T extends object>(initial: T) {
  let state = initial;
  const listeners = new Set<Listener>();
  return {
    get: () => state,
    set(patch: Partial<T>) {
      state = { ...state, ...patch };
      for (const l of listeners) l();
    },
    subscribe(l: Listener) {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },
  };
}

export const store = createStore<AppState>({
  input: DEFAULT_INPUT,
  shot: null,
  phase: 'idle',
  cameraMode: 'auto',
  speed: 1,
  bigBall: true,
  pinYd: 164,
  renderer: '…',
  notice: null,
  shots: 0,
});

/** selector 는 원시값이나 store 안의 같은 객체를 돌려줘야 한다 (새 객체를 만들면 매번 다시 그린다) */
export function useApp<S>(selector: (s: AppState) => S): S {
  return useSyncExternalStore(store.subscribe, () => selector(store.get()), () => selector(store.get()));
}

/** 비행 시계 (초, 공이 실제로 난 시간). 프레임 루프만 바꾼다. */
export const flightClock = { t: 0 };

/** 캐릭터 위치 (m). World.tsx 의 PlayerProbe 가 0.1 s 마다 채운다 (시험·디버깅용) */
export const playerInfo = { x: NaN, y: NaN, z: NaN };

/** 지금 입력으로 친다 */
export function launchShot(input?: ShotInput): Shot | null {
  const s = store.get();
  const inp = clampInput(input ?? s.input);
  const result = simulate(toEngineLaunch(inp));
  if (!result.landed) {
    store.set({ input: inp, notice: '공이 떨어지지 않았습니다 (입력 범위를 확인하세요).' });
    return null;
  }
  const shot: Shot = { id: s.shots + 1, input: inp, result, path: resample(result.samples, 1 / 120) };
  flightClock.t = 0;
  store.set({
    input: inp,
    shot,
    phase: 'flying',
    shots: shot.id,
    // 캐릭터 카메라로 걷다가 치면 공을 보여 준다
    cameraMode: s.cameraMode === 'player' ? 'auto' : s.cameraMode,
  });
  return shot;
}

// 브라우저 시험(scripts/smoke.mjs)과 콘솔 디버깅용 읽기 창
declare global {
  interface Window {
    __golf?: { state: () => AppState; clock: () => number; player: () => { x: number; y: number; z: number }; game?: unknown };
  }
}
if (typeof window !== 'undefined') {
  window.__golf = { state: store.get, clock: () => flightClock.t, player: () => ({ ...playerInfo }) };
}
