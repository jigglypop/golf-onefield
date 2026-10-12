/**
 * 캔버스 안: gaesup-world 의 월드 내용(프레임 단계·엔진 카메라·품질), 해, 물리(Rapier) 위의 캐릭터와 땅,
 * 그리고 모드에 따라 코스(홀·공·조준 안내·게임 카메라·캐릭터 배치) 또는 연습장(연습장·공·샷 카메라).
 */
import { Suspense, useEffect } from 'react';

import { useThree } from '@react-three/fiber';
import { CascadedSun, GaesupController, GaesupWorldContent, useEngineFrame, usePlayerPosition, WorldPhysics } from 'gaesup-world';

import { AimGuide } from '../game/scene/AimGuide.tsx';
import { GameBall } from '../game/scene/GameBall.tsx';
import { GameCamera } from '../game/scene/GameCamera.tsx';
import { GameEffects } from '../game/scene/GameEffects.tsx';
import { Golfer } from '../game/scene/Golfer.tsx';
import { HoleScene } from '../game/scene/HoleScene.tsx';
import { OtherBalls } from '../game/scene/OtherBalls.tsx';
import { playerInfo, store } from '../state.ts';
import { playerBody } from './player.ts';
import { BallFlight } from './BallFlight.tsx';
import { CameraRig } from './CameraRig.tsx';
import { SPAWN, SUN_POSITION } from './layout.ts';
import { Range, RangeColliders } from './Range.tsx';
import { SkyAndFog } from './Sky.tsx';

/** 실제로 그리는 렌더러 종류를 화면에 알린다 (gaesup-world rendering.md 의 rendererKind 판정과 같음) */
function RendererProbe() {
  const gl = useThree((s) => s.gl) as unknown as { isWebGPURenderer?: boolean; backend?: { isWebGPUBackend?: boolean } };
  useEffect(() => {
    const kind = gl.isWebGPURenderer !== true ? 'WebGL (classic)' : gl.backend?.isWebGPUBackend === true ? 'WebGPU' : 'WebGPU (WebGL2 백엔드)';
    store.set({ renderer: kind });
  }, [gl]);
  return null;
}

/** 캐릭터 위치를 playerInfo 로 (gaesup-world usePlayerPosition, 다시 그리지 않는 모드) */
function PlayerProbe() {
  const player = usePlayerPosition({ reactive: false });
  useEngineFrame(
    'snapshot',
    () => {
      playerInfo.x = player.position.x;
      playerInfo.y = player.position.y;
      playerInfo.z = player.position.z;
    },
    { throttleMs: 100, label: 'golf:player-probe' },
  );
  return null;
}

export type Mode = 'course' | 'range';


export function World({ mode }: { mode: Mode }) {
  return (
    <>
      <RendererProbe />
      <hemisphereLight args={['#e6f2ff', '#56773a', 1.15]} />
      <Suspense fallback={null}>
        <GaesupWorldContent quality="auto">
          <SkyAndFog />
          <CascadedSun position={SUN_POSITION} intensity={2.7} color="#fff3dd" />
          <WorldPhysics>
            {/* 캐릭터: 모델은 GaesupWorld urls.characterUrl 에서 온다. 회전 0 = glTF +Z 정면 = 공 쪽 */}
            <GaesupController position={SPAWN} rotation={[0, 0, 0]} enableKeyboard={mode === 'range'} clickToMove={false} rigidBodyRef={playerBody} />
            <RangeColliders />
          </WorldPhysics>
          {mode === 'course' ? (
            <>
              <HoleScene />
              <GameBall />
              <OtherBalls />
              <GameEffects />
              <AimGuide />
              <GameCamera />
              <Golfer />
            </>
          ) : (
            <>
              <Range />
              <BallFlight />
              <CameraRig />
            </>
          )}
          <PlayerProbe />
        </GaesupWorldContent>
      </Suspense>
    </>
  );
}
