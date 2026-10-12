/**
 * 시각 점검용 카메라 고정: `window.__golf.vis.cam({ pos, look, fov })` 로 정한 시점을 게임 카메라 뒤에서 덮어쓴다 (null 이면 풀어 준다).
 * 게임 규칙·물리·카메라 로직은 그대로다. scripts/shots.mjs 가 쓴다.
 */
import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { Matrix4, Vector3, type PerspectiveCamera } from 'three';

import { camOverride } from './debug.ts';

const UP = new Vector3(0, 1, 0);

export function PhotoRig() {
  const get = useThree((s) => s.get);
  const state = { m: new Matrix4(), pos: new Vector3(), look: new Vector3(), applied: false, baseFov: 50 };
  useEngineFrame(
    'camera',
    () => {
      const cam = get().camera as PerspectiveCamera;
      const o = camOverride.current;
      if (!o) {
        if (state.applied) {
          state.applied = false;
          if (cam.fov !== state.baseFov) {
            cam.fov = state.baseFov;
            cam.updateProjectionMatrix();
          }
        }
        return;
      }
      if (!state.applied) {
        state.applied = true;
        state.baseFov = cam.fov;
      }
      state.pos.set(...o.pos);
      state.look.set(...o.look);
      cam.position.copy(state.pos);
      state.m.lookAt(state.pos, state.look, UP);
      cam.quaternion.setFromRotationMatrix(state.m);
      cam.updateMatrixWorld();
      const fov = o.fov ?? state.baseFov;
      if (cam.fov !== fov || cam.near > 0.06) {
        cam.fov = fov;
        cam.near = 0.05;
        cam.updateProjectionMatrix();
      }
    },
    { order: 2000, label: 'golf:photo-rig' },
  );
  return null;
}
