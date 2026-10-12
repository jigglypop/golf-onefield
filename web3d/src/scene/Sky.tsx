/**
 * 하늘: 꼭짓점 색으로 그린 그러데이션 돔 + 해 원판 + 안개. ShaderMaterial 을 쓰지 않아 WebGPU·WebGL 모두에서 같다.
 * 돔과 해는 카메라를 따라간다 (엔진 카메라 단계 뒤 effects 단계).
 */
import { useEffect, useMemo, useRef } from 'react';

import { useThree } from '@react-three/fiber';
import { useEngineFrame } from 'gaesup-world';
import { BackSide, BufferAttribute, Color, Fog, SphereGeometry, Vector3, type Mesh, type Sprite } from 'three';

import { COLORS, SUN_POSITION } from './layout.ts';
import { sunTexture } from './textures.ts';

const DOME_R = 3000;
const SUN_DIR = new Vector3(...SUN_POSITION).normalize();

export function SkyAndFog() {
  const scene = useThree((s) => s.scene);
  const get = useThree((s) => s.get);

  useEffect(() => {
    const prevFog = scene.fog;
    const prevBg = scene.background;
    scene.fog = new Fog(COLORS.fog, 320, 2600);
    scene.background = new Color(COLORS.skyHorizon);
    return () => {
      scene.fog = prevFog;
      scene.background = prevBg;
    };
  }, [scene]);

  const geometry = useMemo(() => {
    const g = new SphereGeometry(DOME_R, 48, 24);
    const pos = g.getAttribute('position');
    const colors = new Float32Array(pos.count * 3);
    const zenith = new Color(COLORS.skyZenith);
    const horizon = new Color(COLORS.skyHorizon);
    const c = new Color();
    for (let i = 0; i < pos.count; i++) {
      const h = Math.max(0, pos.getY(i) / DOME_R);
      const k = Math.min(1, Math.pow(h / 0.55, 0.7));
      c.copy(horizon).lerp(zenith, k);
      colors[3 * i] = c.r;
      colors[3 * i + 1] = c.g;
      colors[3 * i + 2] = c.b;
    }
    g.setAttribute('color', new BufferAttribute(colors, 3));
    return g;
  }, []);
  const sun = useMemo(() => sunTexture(), []);
  useEffect(
    () => () => {
      geometry.dispose();
      sun.dispose();
    },
    [geometry, sun],
  );

  const dome = useRef<Mesh>(null);
  const disc = useRef<Sprite>(null);
  useEngineFrame(
    'effects',
    () => {
      const cam = get().camera;
      dome.current?.position.copy(cam.position);
      disc.current?.position.copy(cam.position).addScaledVector(SUN_DIR, DOME_R * 0.85);
    },
    { label: 'golf:sky' },
  );

  return (
    <group userData={{ intangible: true }}>
      <mesh ref={dome} geometry={geometry} renderOrder={-10} frustumCulled={false}>
        <meshBasicMaterial vertexColors side={BackSide} fog={false} depthWrite={false} toneMapped={false} />
      </mesh>
      <sprite ref={disc} scale={[240, 240, 1]} renderOrder={-9} frustumCulled={false}>
        <spriteMaterial map={sun} fog={false} depthWrite={false} toneMapped={false} transparent />
      </sprite>
    </group>
  );
}
