import { useEffect, useMemo, useRef, type RefObject } from 'react';
import { useFrame } from '@react-three/fiber';
import { useGaesupStoreApi } from 'gaesup-world';
import * as THREE from 'three';

import { hermiteAt, PATH_STRIDE, segmentAt } from '../physics/flat';
import { aimRotationY } from './coords';
import { BALL_RADIUS, REACH_M, TEE, type Vec3 } from './layout';
import type { ShotRecord } from './shot';

/** 비행 중 카메라가 볼에서 떨어져 있는 거리 (m). 엔진 카메라가 골퍼 쪽에서 이만큼 떨어져 따라간다 */
const FOCUS_DISTANCE = 14;
/** 카메라 추적 속도 (엔진의 focusLerpSpeed, 1 이상이면 초당 감쇠 계수) */
const FOCUS_LERP = 3;
/** 카메라 목표(볼 위치)를 갱신하는 주기, 초. 매 프레임 store 를 바꾸지 않으려고 둔다 */
const FOCUS_UPDATE_S = 0.1;

interface ShotBallProps {
  shot: ShotRecord | null;
  /** 다음 샷을 칠 볼 위치 (월드) */
  ballPos: Vec3;
  golferRef: RefObject<THREE.Object3D | null>;
  onLanded: (shot: ShotRecord) => void;
  onReachChange: (ok: boolean) => void;
}

/**
 * 경로 점 사이를 이은 소점 수. 앱 걸음은 샷당 약 21점이라 점끼리 직선으로 이으면 정점·착지 근처에 모서리가 보인다.
 * 점마다 위치와 속도가 있으므로 구간을 3차 에르미트(flat.ts hermiteAt)로 잇고, 선은 구간당 이만큼의 소점으로 그린다.
 */
const TRAIL_SUBDIV = 6;

/**
 * 궤적 선 (샷 루트 기준 로컬 좌표). 샷마다 한 번 만들고, 매 프레임은 drawRange 만 바꾼다.
 * 구간마다 에르미트 곡선을 TRAIL_SUBDIV 등분한 소점을 쓴다. 마지막 점은 착지점 그대로다.
 */
function buildTrail(path: Float64Array): THREE.BufferGeometry {
  const n = path.length / PATH_STRIDE;
  const count = n >= 2 ? (n - 1) * TRAIL_SUBDIV + 1 : n;
  const pos = new Float32Array(count * 3);
  const p = new Float64Array(3);
  const put = (index: number) => {
    // wasm (x 앞, y 위, z 오른쪽) → three 로컬 (z, y, -x). 자세한 설명은 coords.ts 의 flightToLocal
    pos[3 * index] = p[2]!;
    pos[3 * index + 1] = p[1]! + BALL_RADIUS;
    pos[3 * index + 2] = -p[0]!;
  };
  for (let i = 0; i < n - 1; i++) {
    const t0 = path[PATH_STRIDE * i]!;
    const t1 = path[PATH_STRIDE * (i + 1)]!;
    for (let j = 0; j < TRAIL_SUBDIV; j++) {
      hermiteAt(path, i, t0 + ((t1 - t0) * j) / TRAIL_SUBDIV, p);
      put(i * TRAIL_SUBDIV + j);
    }
  }
  if (n >= 1) {
    const last = PATH_STRIDE * (n - 1);
    p[0] = path[last + 1]!;
    p[1] = path[last + 2]!;
    p[2] = path[last + 3]!;
    put(count - 1);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  geometry.setDrawRange(0, 0);
  return geometry;
}

/**
 * 볼과 궤적. 샷이 바뀌면 루트 그룹을 출발점·조준 방향으로 옮기고, 매 프레임 wasm 경로를 시간(t)으로 3차 에르미트 보간한다.
 * 볼은 Rapier 로 시뮬레이션하지 않는다. 착지하면 onLanded 를 한 번 부른다.
 */
export function ShotBall({ shot, ballPos, golferRef, onLanded, onReachChange }: ShotBallProps) {
  const storeApi = useGaesupStoreApi();
  const rootRef = useRef<THREE.Group>(null);
  const ballRef = useRef<THREE.Mesh>(null);
  const trail = useMemo(() => (shot ? buildTrail(shot.path) : null), [shot]);
  const trailMaterial = useMemo(
    () => new THREE.LineBasicMaterial({ color: '#ffffff', transparent: true, opacity: 0.55, depthWrite: false }),
    [],
  );
  // <line> 은 JSX 에서 SVG 로 해석되므로 THREE.Line 객체를 primitive 로 붙인다
  const trailLine = useMemo(() => (trail ? new THREE.Line(trail, trailMaterial) : null), [trail, trailMaterial]);
  useEffect(() => () => trailMaterial.dispose(), [trailMaterial]);
  const anim = useRef({ id: -1, t0: 0, idx: 0, done: false, focusAt: -Infinity, focusing: false });
  const reach = useRef(true);
  const scratch = useMemo(() => new THREE.Vector3(), []);
  const hermite = useMemo(() => new Float64Array(3), []);

  useEffect(() => () => trail?.dispose(), [trail]);

  // 리셋(샷 없음): 볼을 루트 원점에 놓고, 비행 중이었다면 카메라 추적을 끈다
  useEffect(() => {
    if (shot) return;
    ballRef.current?.position.set(0, BALL_RADIUS, 0);
    if (anim.current.focusing) {
      anim.current.focusing = false;
      storeApi.getState().setCameraOption({ focus: false });
    }
  }, [shot, storeApi]);

  useFrame((state) => {
    // 골퍼가 볼 근처에 있는지 (바뀔 때만 알린다)
    const golfer = golferRef.current;
    if (golfer) {
      golfer.getWorldPosition(scratch);
      const ok = Math.hypot(scratch.x - ballPos[0], scratch.z - ballPos[2]) <= REACH_M;
      if (ok !== reach.current) {
        reach.current = ok;
        onReachChange(ok);
      }
    }

    const ball = ballRef.current;
    if (!shot || !trail || !ball) return;
    const a = anim.current;
    if (a.id !== shot.id) {
      a.id = shot.id;
      a.t0 = state.clock.elapsedTime;
      a.idx = 0;
      a.done = false;
      a.focusAt = -Infinity;
      ball.position.set(0, BALL_RADIUS, 0);
    }
    if (a.done) return;

    const path = shot.path;
    const n = path.length / PATH_STRIDE;
    const t = state.clock.elapsedTime - a.t0;

    // 점이 둘보다 적으면(위로 나가지 않는 샷) 날아갈 곡선이 없다: 바로 착지로 친다
    const last = PATH_STRIDE * (n - 1);
    if (n < 2 || t >= path[last]!) {
      // 착지: 마지막 점에 놓고 궤적을 전부 보인다
      if (n >= 1) ball.position.set(path[last + 3]!, path[last + 2]! + BALL_RADIUS, -path[last + 1]!);
      trail.setDrawRange(0, trail.getAttribute('position').count);
      a.done = true;
      if (a.focusing) {
        a.focusing = false;
        storeApi.getState().setCameraOption({ focus: false });
      }
      onLanded(shot);
      return;
    }

    // 현재 시간을 감싸는 구간 찾기 (시간은 단조 증가하므로 직전 구간에서 이어서 찾는다)
    const i = segmentAt(path, t, a.idx);
    a.idx = i;
    const u = hermiteAt(path, i, t, hermite);
    // 로컬 (z, y, -x) 에 놓는다 (flightToLocal 과 같은 식)
    ball.position.set(hermite[2]!, hermite[1]! + BALL_RADIUS, -hermite[0]!);
    // 궤적 선은 볼이 지나온 소점까지만 (구간 i 의 u 만큼)
    trail.setDrawRange(0, i * TRAIL_SUBDIV + Math.floor(u * TRAIL_SUBDIV) + 1);

    // 카메라: 엔진 카메라의 focus 로 볼을 쫓는다 (골퍼 쪽에서 FOCUS_DISTANCE 떨어진 자리에서 볼을 본다)
    if (t - a.focusAt >= FOCUS_UPDATE_S) {
      a.focusAt = t;
      ball.getWorldPosition(scratch);
      a.focusing = true;
      storeApi.getState().setCameraOption({
        focus: true,
        focusTarget: scratch.clone(),
        focusDistance: FOCUS_DISTANCE,
        focusLerpSpeed: FOCUS_LERP,
      });
    }
  });

  return (
    // 루트 원점 = 출발점, Y 회전 = 조준(HLA). 볼과 궤적은 로컬 좌표로 그린다.
    <group ref={rootRef} position={shot ? shot.start : TEE} rotation={[0, shot ? aimRotationY(shot.hlaDeg) : 0, 0]}>
      <mesh ref={ballRef} position={[0, BALL_RADIUS, 0]} castShadow>
        <sphereGeometry args={[BALL_RADIUS, 24, 16]} />
        <meshStandardMaterial color="#ffffff" roughness={0.35} />
      </mesh>
      {trailLine && <primitive object={trailLine} />}
    </group>
  );
}
