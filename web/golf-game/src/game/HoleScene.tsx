import { Suspense, useRef, type RefObject } from 'react';
import type * as THREE from 'three';
import { GaesupController, GaesupWorldContent, WorldPhysics } from 'gaesup-world';

import { Course } from './Course';
import { GOLFER_START, type Vec3 } from './layout';
import { ShotBall } from './ShotBall';
import type { ShotRecord } from './shot';

interface HoleSceneProps {
  shot: ShotRecord | null;
  ballPos: Vec3;
  onLanded: (shot: ShotRecord) => void;
  onReachChange: (ok: boolean) => void;
}

/**
 * Canvas 안의 한 홀 장면. gaesup-world 는 캐릭터(골퍼)·바닥 충돌·엔진 카메라만 맡고,
 * 볼 비행은 wasm 궤적을 따른다 (ShotBall).
 */
export function HoleScene({ shot, ballPos, onLanded, onReachChange }: HoleSceneProps) {
  // 라이브러리의 outerGroupRef 는 null 이 아닌 RefObject 타입을 요구한다 (실제 값은 마운트 뒤에 채워진다)
  const golferRef = useRef<THREE.Group>(null) as RefObject<THREE.Group>;
  return (
    <>
      <color attach="background" args={['#a8dbef']} />
      <fog attach="fog" args={['#cfe9f5', 220, 650]} />
      <hemisphereLight args={['#eaf6ff', '#4f7a3a', 1.1]} />
      <directionalLight
        position={[60, 110, 40]}
        intensity={2}
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-camera-left={-70}
        shadow-camera-right={70}
        shadow-camera-top={70}
        shadow-camera-bottom={-70}
        shadow-camera-near={1}
        shadow-camera-far={300}
      />
      <Suspense fallback={null}>
        <GaesupWorldContent quality="auto">
          <WorldPhysics>
            {/* 골퍼: 모델은 GaesupWorld 의 urls.characterUrl. 시작 회전 π 는 모델이 +Z 를 본다고 가정한 값이다 (모델 정면이 다르면 modelYawOffset 으로 맞춘다) */}
            <GaesupController position={GOLFER_START} rotation={[0, Math.PI, 0]} outerGroupRef={golferRef} />
            <Course />
            <ShotBall
              shot={shot}
              ballPos={ballPos}
              golferRef={golferRef}
              onLanded={onLanded}
              onReachChange={onReachChange}
            />
          </WorldPhysics>
        </GaesupWorldContent>
      </Suspense>
    </>
  );
}
