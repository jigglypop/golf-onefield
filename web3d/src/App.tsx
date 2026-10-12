/**
 * 골프 원필드 3D: gaesup-world 런타임과 월드(GaesupWorld) 아래에 R3F 캔버스와 DOM HUD 를 둔다.
 * 모드: 코스(기본: 메뉴 → 스트로크 1–4명(사람·봇)·니어핀·롱드라이브) / 연습장(값을 넣고 쳐 보기). ?mode=range 로 연습장부터.
 * 렌더러는 gaesup-world createRenderer (WebGPU 가 되면 WebGPU, 아니면 classic WebGL). ?renderer=webgl 이면 classic 강제.
 * 입력: 키보드 + 캔버스 위 포인터 끌기(마우스·터치·펜 공용, 조준) + 화면 버튼(TouchControls).
 * ?xr: WebXR 시험 — classic WebGL 렌더러로 VR 버튼을 붙이고 월드 공간 패널에 결과를 보인다 (game/scene/XrBridge.tsx, 실기기 미시험).
 * 해상도 상한은 photo/quality.ts 의 품질 프로필(dpr)을 따른다.
 */
import { useEffect, useState } from 'react';

import { Canvas } from '@react-three/fiber';
import { createGaesupRuntime, createLegacyRenderer, createRenderer, GaesupWorld } from 'gaesup-world';

import { CLUBS } from './game/clubs.ts';
import { aimDeltaFromDrag } from './game/input.ts';
import {
  aimAtPin,
  aimBy,
  autoShot,
  botTurn,
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
  startHole,
  swing,
  toggleOverview,
} from './game/round.ts';
import { XrBridge } from './game/scene/XrBridge.tsx';
import { placeBall, visApi } from './photo/debug.ts';
import { currentProfile } from './photo/quality.ts';
import { GameHud } from './game/ui/GameHud.tsx';
import { World, type Mode } from './scene/World.tsx';
import { launchShot, store } from './state.ts';
import { Hud } from './ui/Hud.tsx';

const URLS = { characterUrl: `${import.meta.env.BASE_URL}gltf/trainer_green.glb` };
/** 캐릭터 카메라: 캐릭터 뒤(−x) 7.5 m, 위 3.4 m 에서 타깃 쪽을 본다. 넓은 들판이라 카메라 충돌은 끈다 */
const CAMERA = { type: 'thirdPerson', xDistance: 7.5, yDistance: 3.4, zDistance: 0, fov: 50, enableCollision: false } as const;
const PARAMS = typeof location !== 'undefined' ? new URLSearchParams(location.search) : new URLSearchParams();
const XR = PARAMS.has('xr');
/** WebXR 는 classic WebGL 렌더러에서만 시험한다 */
const FORCE_WEBGL = PARAMS.get('renderer') === 'webgl' || XR;
const START_MODE: Mode = PARAMS.get('mode') === 'range' ? 'range' : 'course';
const PROFILE = currentProfile();

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

/**
 * 코스: 캔버스를 좌우로 끌면 조준 (포인터 이벤트라 마우스·터치·펜 모두 된다).
 * HUD 패널·버튼 위에서 시작한 끌기는 받지 않는다 (이벤트 대상이 캔버스일 때만). 조준 단계가 아니면 aimBy 가 무시한다.
 * 한 번에 한 포인터만 (두 번째 손가락은 무시), 끌기가 끝나면 풀어 준다.
 */
function usePointerAim(active: boolean) {
  useEffect(() => {
    if (!active) return;
    let drag: { id: number; x: number } | null = null;
    const onDown = (e: PointerEvent) => {
      if (drag || !(e.target instanceof HTMLCanvasElement)) return;
      if (e.pointerType === 'mouse' && e.button !== 0) return;
      if (game.get().phase !== 'aim') return;
      drag = { id: e.pointerId, x: e.clientX };
      try {
        e.target.setPointerCapture(e.pointerId);
      } catch {
        // 포인터 캡처를 못 쓰는 환경
      }
    };
    const onMove = (e: PointerEvent) => {
      if (!drag || e.pointerId !== drag.id) return;
      const dx = e.clientX - drag.x;
      drag.x = e.clientX;
      aimBy(aimDeltaFromDrag(dx, CLUBS[game.get().clubIdx]!.putter === true));
    };
    const onEnd = (e: PointerEvent) => {
      if (drag && e.pointerId === drag.id) drag = null;
    };
    window.addEventListener('pointerdown', onDown);
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onEnd);
    window.addEventListener('pointercancel', onEnd);
    return () => {
      window.removeEventListener('pointerdown', onDown);
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onEnd);
      window.removeEventListener('pointercancel', onEnd);
    };
  }, [active]);
}

/** 브라우저 시험(scripts/smoke.mjs)·콘솔용 게임 창 */
const gameApi = {
  state: game.get,
  clock: () => shotClock.t,
  swing: (power: number, accuracy = 0) => swing({ power, accuracy }) !== null,
  press: (now?: number) => pressSwing(now),
  auto: autoShot,
  /** 봇 차례이면 한 타 (봇 슬롯: start({ players: [{ name, bot: true, level }] })) */
  bot: botTurn,
  skip: skipAnimation,
  next: nextHole,
  newRound,
  start: startGame,
  skipIntro,
  menu: openMenu,
  club: setClub,
  aimDeg: (d: number) => aimBy(d * DEG),
  overview: toggleOverview,
  /** 시각 점검용: 코스 안의 위치(0 기준)의 홀로 소개 없이 간다 */
  hole: (pos: number) => startHole(pos, false),
  /** 시각 점검용: 공을 (x, z) 에 놓는다 (조준 상태로) */
  place: placeBall,
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
    if (window.__golf) {
      window.__golf.game = gameApi;
      window.__golf.vis = visApi;
    }
  }, []);
  useRangeKeys(mode === 'range');
  useGameKeys(mode === 'course');
  usePointerAim(mode === 'course');
  return (
    <GaesupWorld runtime={runtime} urls={URLS} cameraOption={CAMERA}>
      <div className="app">
        <Canvas
          className="world"
          shadows="percentage"
          dpr={PROFILE.dpr}
          gl={FORCE_WEBGL ? createLegacyRenderer : createRenderer}
          camera={{ position: [-9.5, 2.4, 1.8], fov: 50, near: 0.1, far: 30000 }}
        >
          <World mode={mode} />
          {XR && <XrBridge />}
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
