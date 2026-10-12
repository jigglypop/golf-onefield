import { CuboidCollider, CylinderCollider, RigidBody } from '@react-three/rapier';
import { DoubleSide } from 'three';

import { BUNKERS, FAIRWAY, FLAG, GREEN, TEE, TREES } from './layout';

/** 페어웨이 줄무늬 수 (번갈아 두 가지 녹색) */
const STRIPES = 5;
const STRIPE_LEN = (FAIRWAY.zNear - FAIRWAY.zFar) / STRIPES;

/**
 * 한 홀 지형. 바닥과 나무 줄기는 Rapier 고정 충돌체로 두어 gaesup-world 캐릭터가 설 수 있게 한다.
 * 볼은 Rapier 로 시뮬레이션하지 않는다 (wasm 궤적을 따른다). 잔디·모래 등 시각 요소는 충돌 없이 얇은 판으로 깐다.
 */
export function Course() {
  return (
    <>
      <RigidBody type="fixed" colliders={false}>
        {/* 윗면이 y = 0 인 넓은 바닥 */}
        <CuboidCollider args={[400, 0.5, 400]} position={[0, -0.5, 0]} />
        {TREES.map(([x, z]) => (
          <CylinderCollider key={`trunk-${x}-${z}`} args={[2, 0.5]} position={[x, 2, z]} />
        ))}
      </RigidBody>

      {/* 러프 (바닥 면) */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} receiveShadow>
        <planeGeometry args={[800, 800]} />
        <meshStandardMaterial color="#3d7a36" />
      </mesh>

      {/* 티잉 그라운드 */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[TEE[0], 0.01, TEE[2] - 1.5]} receiveShadow>
        <planeGeometry args={[10, 7]} />
        <meshStandardMaterial color="#7cc45a" />
      </mesh>
      <mesh position={[TEE[0], 0.02, TEE[2]]}>
        <cylinderGeometry args={[0.02, 0.02, 0.04, 8]} />
        <meshStandardMaterial color="#f5f5f5" />
      </mesh>

      {/* 페어웨이 줄무늬 */}
      {Array.from({ length: STRIPES }, (_, i) => {
        const zCenter = FAIRWAY.zNear - (i + 0.5) * STRIPE_LEN;
        return (
          <mesh key={`stripe-${i}`} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.01, zCenter]} receiveShadow>
            <planeGeometry args={[FAIRWAY.halfWidth * 2, STRIPE_LEN]} />
            <meshStandardMaterial color={i % 2 ? '#6aae4a' : '#78bb55'} />
          </mesh>
        );
      })}

      {/* 벙커 (모래 원반). 걸어 들어갈 수 있는 얇은 판이다 */}
      {BUNKERS.map(({ pos: [x, z], scale: [sx, sz] }) => (
        <mesh key={`bunker-${x}-${z}`} rotation={[-Math.PI / 2, 0, 0]} position={[x, 0.03, z]} scale={[sx, sz, 1]} receiveShadow>
          <circleGeometry args={[1, 32]} />
          <meshStandardMaterial color="#e8d49a" />
        </mesh>
      ))}

      {/* 그린과 컵 */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.02, GREEN.z]} receiveShadow>
        <circleGeometry args={[GREEN.radius, 48]} />
        <meshStandardMaterial color="#8ed46a" />
      </mesh>
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[FLAG[0], 0.03, FLAG[2]]}>
        <circleGeometry args={[0.054, 24]} />
        <meshStandardMaterial color="#111111" />
      </mesh>

      {/* 깃대와 깃발 */}
      <mesh position={[FLAG[0], 1.2, FLAG[2]]} castShadow>
        <cylinderGeometry args={[0.025, 0.025, 2.4, 8]} />
        <meshStandardMaterial color="#e8e8e8" />
      </mesh>
      <mesh position={[FLAG[0] + 0.36, 2.1, FLAG[2]]} castShadow>
        <planeGeometry args={[0.72, 0.42]} />
        <meshStandardMaterial color="#d8362b" side={DoubleSide} />
      </mesh>

      {/* 거리 표지 (100 m, 200 m) */}
      {[100, 200].map((d) => (
        <mesh key={`marker-${d}`} position={[FAIRWAY.halfWidth + 1, 0.4, -d]} castShadow>
          <boxGeometry args={[0.3, 0.8, 0.3]} />
          <meshStandardMaterial color="#ffffff" />
        </mesh>
      ))}

      {/* 나무: 줄기와 수관 */}
      {TREES.map(([x, z]) => (
        <group key={`tree-${x}-${z}`} position={[x, 0, z]}>
          <mesh position={[0, 2, 0]} castShadow>
            <cylinderGeometry args={[0.5, 0.6, 4, 8]} />
            <meshStandardMaterial color="#6b4a2b" />
          </mesh>
          <mesh position={[0, 5.6, 0]} castShadow>
            <sphereGeometry args={[3.2, 14, 10]} />
            <meshStandardMaterial color="#2f6b2a" />
          </mesh>
        </group>
      ))}
    </>
  );
}
