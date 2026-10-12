/**
 * WebXR 시험(스파이크, 주소에 ?xr 가 있을 때만 App 이 붙인다). 실제 헤드셋에서는 아직 시험하지 못했다.
 *
 * 하는 일:
 *   1. three 의 VRButton 을 화면에 붙인다 ("ENTER VR"). 렌더러는 classic WebGL 이어야 한다 (App 이 ?xr 이면 고른다).
 *   2. 몰입 세션이 열리면 카메라를 "돌리(dolly)" 그룹 밑에 넣는다. 헤드셋 자세는 돌리 기준이므로 돌리를 공 뒤 2.2 m,
 *      조준 방향(−Z 가 조준 방향)에 놓으면 선 채로 샷을 바라본다. 위치는 샷이 바뀔 때만 옮긴다 (멀미 방지: 비행 중 카메라 이동 없음).
 *   3. DOM HUD 는 몰입 중 보이지 않으므로 캔버스 질감 한 장(PANEL_W × PANEL_H)을 월드 공간 평면에 붙여 같은 정보를 보인다
 *      (직전 결과 메시지, 핀까지·바람·클럽, 직전 샷 수치, 스윙 미터). 글은 ui/panelModel.ts.
 *   4. 컨트롤러: 트리거(select) = 스페이스 (소개 건너뛰기·스윙 단계·다음 홀), 스틱 좌우 = 조준, 스틱 위/아래 = 클럽(한 번씩),
 *      그립(squeeze) = 빨리 감기.
 *
 * 한계: 컨트롤러 모델·손 추적·텔레포트 이동·몰입 안의 메뉴는 없다.
 */
import { useEffect, useMemo, useRef } from 'react';

import { useFrame, useThree } from '@react-three/fiber';
import { CanvasTexture, LinearFilter, SRGBColorSpace, type Group, type Mesh, type WebGLRenderer } from 'three';
import { VRButton } from 'three/examples/jsm/webxr/VRButton.js';

import { aimBy, game, nextHole, pressSwing, selectClub, skipAnimation } from '../round.ts';
import { drawPanel, panelModel } from '../ui/panelModel.ts';

const PANEL_W = 1024;
const PANEL_H = 576;
/** 패널 크기 (m) */
const PANEL_M = [1.6, 0.9] as const;
/** 돌리가 샷 자리에서 떨어지는 거리 (공 뒤, m) */
const BACK_M = 2.2;

export function XrBridge() {
  const gl = useThree((s) => s.gl) as unknown as WebGLRenderer;
  const camera = useThree((s) => s.camera);
  const dolly = useRef<Group>(null);
  const panel = useRef<Mesh>(null);
  const state = useMemo(() => ({ dirty: true, lastDraw: 0, key: '', stickAim: 0, stickClub: 0 }), []);
  const canvas = useMemo(() => {
    const c = document.createElement('canvas');
    c.width = PANEL_W;
    c.height = PANEL_H;
    return c;
  }, []);
  const texture = useMemo(() => {
    const t = new CanvasTexture(canvas);
    t.colorSpace = SRGBColorSpace;
    t.minFilter = LinearFilter;
    t.generateMipmaps = false;
    return t;
  }, [canvas]);

  // VR 버튼 + 세션 시작/끝 처리
  useEffect(() => {
    const xr = gl.xr;
    if (!xr) return;
    xr.enabled = true;
    const button = VRButton.createButton(gl);
    button.setAttribute('data-testid', 'xr-button');
    document.body.appendChild(button);

    const onSelect = () => {
      const s = game.get();
      if (s.phase === 'holed' || s.phase === 'done') nextHole();
      else pressSwing();
    };
    const onSqueeze = () => skipAnimation();
    const onStart = () => {
      const d = dolly.current;
      if (d) d.add(camera);
      state.key = '';
      const session = xr.getSession();
      session?.addEventListener('select', onSelect);
      session?.addEventListener('squeeze', onSqueeze);
    };
    const onEnd = () => {
      camera.removeFromParent();
      const d = dolly.current;
      if (d) {
        d.position.set(0, 0, 0);
        d.rotation.set(0, 0, 0);
      }
    };
    xr.addEventListener('sessionstart', onStart);
    xr.addEventListener('sessionend', onEnd);
    const unsub = game.subscribe(() => {
      state.dirty = true;
    });
    return () => {
      unsub();
      xr.removeEventListener('sessionstart', onStart);
      xr.removeEventListener('sessionend', onEnd);
      button.remove();
    };
  }, [gl, camera, state]);

  useEffect(
    () => () => {
      texture.dispose();
    },
    [texture],
  );

  useFrame((_, delta) => {
    const xr = gl.xr;
    if (!xr?.isPresenting) return;
    const s = game.get();
    const d = dolly.current;
    if (d) {
      // 샷 자리가 바뀔 때만 옮긴다: 조준·스윙 때는 공 + 조준 방향, 홀이 끝나면 핀 뒤쪽
      const key = `${s.ballId}:${s.phase === 'holed' || s.phase === 'done' ? 'h' : s.phase === 'intro' || s.phase === 'menu' ? 'i' : 'a'}`;
      if (key !== state.key && (s.phase === 'aim' || s.phase === 'holed' || s.phase === 'done' || s.phase === 'intro' || s.phase === 'menu')) {
        state.key = key;
        const yaw = s.aimYaw;
        d.position.set(s.ball.x - Math.cos(yaw) * BACK_M, 0, s.ball.z - Math.sin(yaw) * BACK_M);
        d.rotation.y = -yaw - Math.PI / 2;
      }
    }
    // 컨트롤러 스틱: 좌우 = 조준, 위/아래 = 클럽 (한 번씩)
    const session = xr.getSession();
    if (session) {
      for (const src of session.inputSources) {
        const ax = src.gamepad?.axes;
        if (!ax || ax.length < 4) continue;
        const x = ax[2] ?? 0;
        const y = ax[3] ?? 0;
        if (Math.abs(x) > 0.2) aimBy(x * Math.min(delta, 0.1) * 0.35);
        if (Math.abs(y) > 0.7 && state.stickClub === 0) {
          selectClub(y > 0 ? 1 : -1);
          state.stickClub = y > 0 ? 1 : -1;
        } else if (Math.abs(y) < 0.4) state.stickClub = 0;
      }
    }
    // 패널: 상태가 바뀌었거나, 스윙 미터가 도는 중이면 20 Hz 로 다시 그린다
    const now = performance.now();
    if (state.dirty || (s.phase === 'swing' && now - state.lastDraw > 50)) {
      state.dirty = false;
      state.lastDraw = now;
      const ctx = canvas.getContext('2d');
      if (ctx) {
        drawPanel(ctx, panelModel(s, now), PANEL_W, PANEL_H);
        texture.needsUpdate = true;
      }
    }
  });

  return (
    <group ref={dolly} name="xr-dolly">
      {/* 사용자의 왼쪽 앞: 돌리 기준 −Z 가 조준 방향 */}
      <mesh ref={panel} position={[-0.9, 1.5, -2.2]} rotation={[0, 0.35, 0]} renderOrder={10}>
        <planeGeometry args={[PANEL_M[0], PANEL_M[1]]} />
        <meshBasicMaterial map={texture} transparent toneMapped={false} depthWrite={false} />
      </mesh>
    </group>
  );
}
