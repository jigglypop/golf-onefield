/**
 * 골프 원필드 3D: gaesup-world 런타임과 월드(GaesupWorld) 아래에 R3F 캔버스와 DOM HUD 를 둔다.
 * 모드: 코스(기본: 메뉴 → 스트로크 1–4명·니어핀·롱드라이브) / 연습장(값을 넣고 쳐 보기). ?mode=range 로 연습장부터.
 * 렌더러는 gaesup-world createRenderer (WebGPU 가 되면 WebGPU, 아니면 classic WebGL). ?renderer=webgl 이면 classic 강제.
 */
import { useEffect, useState } from 'react';

import { Canvas } from '@react-three/fiber';
import { createGaesupRuntime, createLegacyRenderer, createRenderer, GaesupWorld } from 'gaesup-world';

import { CLUBS } from './game/clubs.ts';
import {
  aimAtPin,
  aimBy,
  autoShot,
  cancelSwing,
  game,
  newRound,
  nextHole,
  openMenu,
  pressSwing,
  selectClub,
  setClub,
  shotClock,
  skipAnimation,
  skipIntro,
  startGame,
  swing,
  toggleOverview,
} from './game/round.ts';
import { GameHud } from './game/ui/GameHud.tsx';
import { World, type Mode } from './scene/World.tsx';
import { launchShot, store } from './state.ts';
import { Hud } from './ui/Hud.tsx';

const URLS = { characterUrl: `${import.meta.env.BASE_URL}gltf/trainer_green.glb` };
/** 캐릭터 카메라: 캐릭터 뒤(−x) 7.5 m, 위 3.4 m 에서 타깃 쪽을 본다. 넓은 들판이라 카메라 충돌은 끈다 */
const CAMERA = { type: 'thirdPerson', xDistance: 7.5, yDistance: 3.4, zDistance: 0, fov: 50, enableCollision: false } as const;
const PARAMS = typeof location !== 'undefined' ? new URLSearchParams(location.search) : new URLSearchParams();
const FORCE_WEBGL = PARAMS.get('renderer') === 'webgl';
const START_MODE: Mode = PARAMS.get('mode') === 'range' ? 'range' : 'course';

const MOVE_KEYS = new Set(['KeyW', 'KeyA', 'KeyS', 'KeyD', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);
const DEG = Math.PI / 180;

function typingIn(e: KeyboardEvent): { typing: boolean; tag: string } {
  const t = e.target instanceof HTMLElement ? e.target : null;
  const tag = t?.tagName ?? '';
  return { typing: tag === 'TEXTAREA' || tag === 'SELECT' || tag === 'INPUT' || t?.isContentEditable === true, tag };
}

/** 연습장: Enter = 샷 (gaesup-world 가 쓰지 않는 키). 걷기 키를 누르면 캐릭터 카메라로 바꾼다. */
function useRangeKeys(active: boolean) {
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      const { typing, tag } = typingIn(e);
      if (e.key === 'Enter') {
        // 버튼 위의 Enter 는 그 버튼을 누르고, 글상자에서는 줄을 바꾼다
        if (typing || tag === 'BUTTON' || e.repeat || e.isComposing) return;
        e.preventDefault();
        launchShot();
        return;
      }
      if (typing || e.ctrlKey || e.metaKey || e.altKey) return;
      if (MOVE_KEYS.has(e.code) && store.get().cameraMode !== 'player') store.set({ cameraMode: 'player' });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [active]);
}

/** 코스: Space 스윙(소개 중이면 건너뛰기), ←/→ 조준, ↑/↓ 클럽, P 핀 조준, V 홀 전체, F 빨리 감기, Esc 스윙 취소, Enter 다음 홀 */
function useGameKeys(active: boolean) {
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      const { typing, tag } = typingIn(e);
      if (typing || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return;
      const s = game.get();
      if (s.phase === 'menu') return;
      const putter = CLUBS[s.clubIdx]!.putter === true;
      const step = (putter ? 0.25 : 0.5) * (e.shiftKey ? 4 : 1) * DEG;
      switch (e.code) {
        case 'Space':
          // 버튼에 초점이 있어도 Space 는 스윙 (버튼을 누르지 않게 막는다)
          e.preventDefault();
          if (!e.repeat) pressSwing();
          break;
        case 'ArrowLeft':
        case 'KeyA':
          e.preventDefault();
          aimBy(-step);
          break;
        case 'ArrowRight':
        case 'KeyD':
          e.preventDefault();
          aimBy(step);
          break;
        case 'ArrowUp':
        case 'KeyW':
          e.preventDefault();
          if (!e.repeat) selectClub(-1);
          break;
        case 'ArrowDown':
        case 'KeyS':
          e.preventDefault();
          if (!e.repeat) selectClub(1);
          break;
        case 'KeyV':
          toggleOverview();
          break;
        case 'KeyP':
          aimAtPin();
          break;
        case 'KeyF':
          skipAnimation();
          break;
        case 'Escape':
          cancelSwing();
          break;
        case 'Enter':
          if (tag === 'BUTTON' || e.repeat) return;
          if (s.phase === 'intro') {
            e.preventDefault();
            skipIntro();
          } else if (s.phase === 'holed' || s.phase === 'done') {
            e.preventDefault();
            nextHole();
          }
          break;
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [active]);
}

/** 브라우저 시험(scripts/smoke.mjs)·콘솔용 게임 창 */
const gameApi = {
  state: game.get,
  clock: () => shotClock.t,
  swing: (power: number, accuracy = 0) => swing({ power, accuracy }) !== null,
  press: (now?: number) => pressSwing(now),
  auto: autoShot,
  skip: skipAnimation,
  next: nextHole,
  newRound,
  start: startGame,
  skipIntro,
  menu: openMenu,
  club: setClub,
  aimDeg: (d: number) => aimBy(d * DEG),
  overview: toggleOverview,
};

export function App() {
  const [runtime] = useState(() => createGaesupRuntime({ worldId: 'golf-onefield' }));
  const [mode, setMode] = useState<Mode>(START_MODE);
  useEffect(() => {
    runtime.setup().catch((error: unknown) => console.error(error));
    return () => {
      runtime.dispose().catch((error: unknown) => console.error(error));
    };
  }, [runtime]);
  useEffect(() => {
    if (window.__golf) window.__golf.game = gameApi;
  }, []);
  useRangeKeys(mode === 'range');
  useGameKeys(mode === 'course');
  return (
    <GaesupWorld runtime={runtime} urls={URLS} cameraOption={CAMERA}>
      <div className="app">
        <Canvas
          className="world"
          shadows="percentage"
          gl={FORCE_WEBGL ? createLegacyRenderer : createRenderer}
          camera={{ position: [-9.5, 2.4, 1.8], fov: 50, near: 0.1, far: 4000 }}
        >
          <World mode={mode} />
        </Canvas>
        {mode === 'course' ? <GameHud onRange={() => setMode('range')} /> : <Hud />}
        <div className="g-mode" role="tablist" aria-label="모드">
          {(
            [
              ['course', '코스'],
              ['range', '연습장'],
            ] as const
          ).map(([m, label]) => (
            <button key={m} type="button" role="tab" aria-selected={mode === m} className={mode === m ? 'on' : ''} onClick={(e) => (setMode(m), e.currentTarget.blur())}>
              {label}
            </button>
          ))}
        </div>
        {mode === 'course' && (
          <div className="g-physics-note">
            비행은 공유위기12 장(PGA 투어표로 보정), 바운드·굴림·퍼팅은 검증하지 않은 게임용 모형입니다. 클럽 거리는 투어 프로 기준.
          </div>
        )}
      </div>
    </GaesupWorld>
  );
}
