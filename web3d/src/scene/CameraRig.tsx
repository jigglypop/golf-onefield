/**
 * 샷 카메라. 'player' 모드에서는 아무것도 하지 않아 gaesup-world 3인칭 카메라가 캐릭터를 따라간다.
 * 다른 모드에서는 엔진 카메라 단계 뒤(order 1000)에 카메라 자세를 덮어쓴다. 엔진 카메라는 매 프레임 지금 카메라
 * 위치에서 보간을 시작하므로, 'player' 로 돌아가면 마지막 샷 카메라 자리에서 캐릭터 쪽으로 부드럽게 돌아간다.
 *
 * auto: 샷 전 티 뒤 → 비행 중 공 따라가기 → 착지 뒤 착지점 보기. behind/side/top: 고정 시점(궤적에 맞춰 거리 조절).
 */
import { useRef } from 'react';

import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { Matrix4, Vector3, type PerspectiveCamera } from 'three';

import { store, type CameraMode } from '../state.ts';
import { ballWorld } from './BallFlight.tsx';

const BEHIND_POS = new Vector3(-9.5, 2.4, 1.8);
const BEHIND_LOOK = new Vector3(110, 7, 0);
const UP_Y = new Vector3(0, 1, 0);
const UP_X = new Vector3(1, 0, 0);
const NEAR = 0.3;

type View = 'behind' | 'follow' | 'landing' | 'side' | 'top';

function viewOf(mode: Exclude<CameraMode, 'player'>, phase: 'idle' | 'flying' | 'landed'): View {
  if (mode !== 'auto') return mode;
  return phase === 'flying' ? 'follow' : phase === 'landed' ? 'landing' : 'behind';
}

function setNear(cam: PerspectiveCamera, near: number) {
  if (Math.abs(cam.near - near) > 1e-3) {
    cam.near = near;
    cam.updateProjectionMatrix();
  }
}

export function CameraRig() {
  const get = useThree((s) => s.get);
  const rig = useRef({
    active: false,
    lastShot: 0,
    pos: new Vector3(),
    look: new Vector3(),
    up: new Vector3(0, 1, 0),
    dPos: new Vector3(),
    dLook: new Vector3(),
    tmp: new Vector3(),
    m: new Matrix4(),
  });

  useEngineFrame(
    'camera',
    (delta) => {
      const app = store.get();
      const cam = get().camera as PerspectiveCamera;
      const r = rig.current;
      if (app.cameraMode === 'player') {
        if (r.active) {
          r.active = false;
          setNear(cam, NEAR);
        }
        return;
      }
      if (!r.active) {
        r.active = true;
        r.pos.copy(cam.position);
        r.look.copy(cam.getWorldDirection(r.tmp)).multiplyScalar(60).add(cam.position);
        r.up.copy(UP_Y);
      }
      const shot = app.shot;
      // 새 샷: 자동 카메라는 티 뒤로 바로 자른다
      if (shot && shot.id !== r.lastShot) {
        r.lastShot = shot.id;
        if (app.cameraMode === 'auto') {
          r.pos.copy(BEHIND_POS);
          r.look.copy(BEHIND_LOOK);
          r.up.copy(UP_Y);
        }
      }
      const view = viewOf(app.cameraMode, app.phase);
      const end = shot ? shot.result.samples[shot.result.samples.length - 1]! : null;
      const carry = end ? Math.max(end.x, 40) : 180;
      const lat = end ? end.z : 0;
      const apex = shot ? shot.result.apex.y : 28;
      const vfov = (cam.fov * Math.PI) / 180;
      const tanV = Math.tan(vfov / 2);
      const tanH = tanV * cam.aspect;
      let posRate = 2.5;
      let lookRate = 3;
      let up = UP_Y;
      let near = NEAR;
      switch (view) {
        case 'behind':
          r.dPos.copy(BEHIND_POS);
          r.dLook.copy(BEHIND_LOOK);
          break;
        case 'follow': {
          const b = ballWorld;
          r.dPos.set(b.x - 17, Math.max(1.6, b.y * 0.85 + 3.5), b.z + 8);
          r.dLook.copy(b);
          posRate = 2.2;
          lookRate = 10;
          break;
        }
        case 'landing':
          r.dPos.set((end?.x ?? 0) - 26, 8.5, (end?.z ?? 0) + 15);
          r.dLook.set(end?.x ?? 0, 1.2, end?.z ?? 0);
          posRate = 1.4;
          lookRate = 2.2;
          break;
        case 'side': {
          // 오른쪽(+z)에서 보면 공이 화면 왼쪽 → 오른쪽으로 난다
          const span = carry + 40;
          const d = Math.max((span / 2) / tanH, (apex * 0.6 + 6) / tanV) + 4;
          r.dPos.set(carry / 2, apex * 0.45 + 3, lat / 2 + d);
          r.dLook.set(carry / 2, apex * 0.4, lat / 2);
          break;
        }
        case 'top': {
          // 화면 위쪽 = 타깃 방향(+x)
          const span = carry + 50;
          const h = Math.max((span / 2) / tanV, (Math.abs(lat) + 30) / tanH) * 1.05;
          r.dPos.set(carry / 2, h, lat / 2);
          r.dLook.set(carry / 2, 0, lat / 2);
          up = UP_X;
          near = Math.max(NEAR, h * 0.02);
          break;
        }
      }
      const dt = Math.min(delta, 0.1);
      r.pos.lerp(r.dPos, 1 - Math.exp(-posRate * dt));
      r.look.lerp(r.dLook, 1 - Math.exp(-lookRate * dt));
      r.up.lerp(up, 1 - Math.exp(-4 * dt)).normalize();
      cam.position.copy(r.pos);
      r.m.lookAt(r.pos, r.look, r.up);
      cam.quaternion.setFromRotationMatrix(r.m);
      cam.updateMatrixWorld();
      setNear(cam, near);
    },
    { order: 1000, label: 'golf:camera' },
  );
  return null;
}
