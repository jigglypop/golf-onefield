/**
 * 게임 카메라 (엔진 카메라 단계 뒤 order 1000 에서 덮어쓴다):
 *   조준 — 공 뒤 6.5 m·위 1.9 m 에서 공 앞 바닥 쪽(공이 HUD 위 70 % 높이), 퍼팅 — 공 뒤 3.4 m·위 1.3 m 에서 낮게,
 *   비행 — 공을 뒤에서 따라감, 굴림 — 공 뒤에서 내려다봄, 홀아웃 — 깃대 둘레를 천천히 돎,
 *   전체 보기(V)·라운드 끝 — 홀을 위에서 (화면 위쪽 = 티에서 핀 쪽), 메뉴 — 홀 위를 높이 돎, 홀 소개 — 티에서 그린까지 날아감.
 * 공을 새 자리에 놓으면(ballId) 조준 시점으로 바로 자른다.
 */
import { useRef } from 'react';

import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { Matrix4, Vector3, type PerspectiveCamera } from 'three';

import { CLUBS } from '../clubs.ts';
import { yawTo, type Hole } from '../course.ts';
import { game, holeOf, INTRO_S } from '../round.ts';
import { ballPos } from './GameBall.tsx';

const UP_Y = new Vector3(0, 1, 0);

/** 홀 꺾은선 (티 → 조준점 → 핀, 파3 은 티 → 핀) 위의 거리 d 지점 */
function along(h: Hole, d: number): [number, number, number] {
  const pts = h.par > 3 ? [h.tee, h.teeTarget, h.pin] : [h.tee, h.pin];
  let rest = Math.max(d, 0);
  for (let i = 0; i + 1 < pts.length; i++) {
    const [ax, az] = pts[i]!;
    const [bx, bz] = pts[i + 1]!;
    const l = Math.hypot(bx - ax, bz - az);
    if (rest <= l || i + 2 === pts.length) {
      const t = Math.min(rest / l, 1);
      return [ax + (bx - ax) * t, az + (bz - az) * t, Math.atan2(bz - az, bx - ax)];
    }
    rest -= l;
  }
  return [h.pin[0], h.pin[1], 0];
}

function pathLength(h: Hole): number {
  const pts = h.par > 3 ? [h.tee, h.teeTarget, h.pin] : [h.tee, h.pin];
  let l = 0;
  for (let i = 0; i + 1 < pts.length; i++) l += Math.hypot(pts[i + 1]![0] - pts[i]![0], pts[i + 1]![1] - pts[i]![1]);
  return l;
}

export function GameCamera() {
  const get = useThree((s) => s.get);
  const rig = useRef({
    pos: new Vector3(),
    look: new Vector3(),
    up: new Vector3(0, 1, 0),
    dPos: new Vector3(),
    dLook: new Vector3(),
    dUp: new Vector3(0, 1, 0),
    m: new Matrix4(),
    orbit: 0,
    lastBall: -1,
    init: false,
  });

  useEngineFrame(
    'camera',
    (delta) => {
      const s = game.get();
      const cam = get().camera as PerspectiveCamera;
      const r = rig.current;
      const h = holeOf(s);
      const yaw = s.phase === 'flight' || s.phase === 'ground' ? (s.shot?.yaw ?? s.aimYaw) : s.aimYaw;
      const fx = Math.cos(yaw);
      const fz = Math.sin(yaw);
      const rx = -fz;
      const rz = fx;
      const b = ballPos;
      const putter = CLUBS[s.clubIdx]!.putter === true;
      let posRate = 2.6;
      let lookRate = 4;
      let near = 0.1;
      r.dUp.copy(UP_Y);
      if (s.phase === 'menu') {
        // 메뉴: 홀 위를 높이 천천히 돈다
        r.orbit += Math.min(delta, 0.1) * 0.05;
        const cx = (h.tee[0] + h.pin[0]) / 2;
        const cz = (h.tee[1] + h.pin[1]) / 2;
        // 낮게 천천히 도는 항공 촬영: 능선과 하늘이 같이 보이게
        r.dPos.set(cx + Math.cos(r.orbit) * 215, 48, cz + Math.sin(r.orbit) * 215);
        r.dLook.set(cx, 10, cz);
        posRate = 1.2;
        lookRate = 1.5;
        near = 0.5;
      } else if (s.phase === 'intro') {
        // 홀 소개: 티 뒤 높이에서 꺾은선을 따라 그린까지 날아간다
        const p = Math.min(Math.max((performance.now() - s.introT0) / (INTRO_S * 1000), 0), 1);
        const e = p * p * (3 - 2 * p);
        const L = h.kind === 'longdrive' ? 380 : pathLength(h);
        const [x, z, dir] = h.kind === 'longdrive' ? [e * L, 0, 0] : along(h, e * L);
        const [lx, lz] = h.kind === 'longdrive' ? [Math.min(e * L + 90, L), 0] : along(h, Math.min(e * L + 80, L));
        r.dPos.set(x - Math.cos(dir) * 35, 42 - 18 * e, z - Math.sin(dir) * 35);
        r.dLook.set(lx, 0, lz);
        posRate = 3;
        lookRate = 3;
        near = 0.5;
      } else if (s.overview || s.phase === 'done') {
        // 위에서: 티–핀 가운데, 화면 위쪽이 홀 방향
        const hy = yawTo(h.tee, h.pin);
        const cx = (h.tee[0] + h.pin[0]) / 2;
        const cz = (h.tee[1] + h.pin[1]) / 2;
        const len = Math.hypot(h.pin[0] - h.tee[0], h.pin[1] - h.tee[1]) + 90;
        const tanV = Math.tan((cam.fov * Math.PI) / 360);
        const height = (len / 2 / tanV) * 1.02;
        r.dPos.set(cx, height, cz);
        r.dLook.set(cx, 0, cz);
        r.dUp.set(Math.cos(hy), 0, Math.sin(hy));
        near = Math.max(0.5, height * 0.02);
        posRate = 2;
        lookRate = 3;
      } else {
        switch (s.phase) {
          case 'aim':
          case 'swing':
            // 공이 화면 아래 약 70 % 높이(HUD 위)에 오도록: 낮게 뒤에서, 시선은 공 앞 바닥
            if (putter) {
              r.dPos.set(b.x - fx * 3.4, 1.3, b.z - fz * 3.4);
              r.dLook.set(b.x + fx * 4, 0, b.z + fz * 4);
              near = 0.05;
            } else {
              // 어깨 너머: 공 오른쪽으로 조금 비켜서 왼쪽에 선 골퍼가 화면 왼쪽에 오게
              r.dPos.set(b.x - fx * 6.5 + rx * 1.1, 2.0, b.z - fz * 6.5 + rz * 1.1);
              r.dLook.set(b.x + fx * 10 + rx * 0.4, 0.2, b.z + fz * 10 + rz * 0.4);
            }
            posRate = 5;
            lookRate = 5;
            break;
          case 'flight':
            r.dPos.set(b.x - fx * 16 + rx * 6, Math.max(2.5, b.y * 0.75 + 3.5), b.z - fz * 16 + rz * 6);
            r.dLook.copy(b);
            posRate = 2.2;
            lookRate = 10;
            break;
          case 'ground':
            if (s.shot?.putt) {
              r.dPos.set(b.x - fx * 4 + rx * 1.5, 1.6, b.z - fz * 4 + rz * 1.5);
              near = 0.05;
            } else {
              r.dPos.set(b.x - fx * 11 + rx * 4, 5, b.z - fz * 11 + rz * 4);
            }
            r.dLook.copy(b);
            posRate = 2;
            lookRate = 8;
            break;
          case 'holed': {
            r.orbit += Math.min(delta, 0.1) * 0.35;
            const a = yaw + Math.PI + r.orbit;
            r.dPos.set(h.pin[0] + Math.cos(a) * 6, 2.4, h.pin[1] + Math.sin(a) * 6);
            r.dLook.set(h.pin[0], 0.6, h.pin[1]);
            posRate = 1.6;
            lookRate = 3;
            break;
          }
        }
      }
      if (s.phase !== 'holed' && s.phase !== 'menu') r.orbit = 0;
      // 새 자리·첫 프레임: 조준 시점으로 바로
      if (!r.init || (s.ballId !== r.lastBall && (s.phase === 'aim' || s.phase === 'swing'))) {
        r.init = true;
        r.lastBall = s.ballId;
        r.pos.copy(r.dPos);
        r.look.copy(r.dLook);
        r.up.copy(r.dUp);
      }
      const dt = Math.min(delta, 0.1);
      r.pos.lerp(r.dPos, 1 - Math.exp(-posRate * dt));
      r.look.lerp(r.dLook, 1 - Math.exp(-lookRate * dt));
      r.up.lerp(r.dUp, 1 - Math.exp(-4 * dt)).normalize();
      cam.position.copy(r.pos);
      r.m.lookAt(r.pos, r.look, r.up);
      cam.quaternion.setFromRotationMatrix(r.m);
      cam.updateMatrixWorld();
      if (Math.abs(cam.near - near) > 1e-3) {
        cam.near = near;
        cam.updateProjectionMatrix();
      }
    },
    { order: 1000, label: 'golf:game-camera' },
  );
  return null;
}
