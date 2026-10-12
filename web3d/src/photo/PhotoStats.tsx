/**
 * 렌더 통계: 프레임 시간(이동 평균)·fps·드로우콜·삼각형·텍스처·지오메트리 수를 `window.__golf.vis.stats()` 로 읽게 한다.
 * `?stats` 면 화면 구석에도 보인다. 게임 규칙·물리는 건드리지 않는다.
 */
import { useEffect, useRef } from 'react';

import { useFrame, useThree } from '@react-three/fiber';

import { renderStats, sceneRef } from './debug.ts';

export function PhotoStats() {
  const gl = useThree((s) => s.gl) as unknown as { info: { autoReset: boolean; reset(): void; render: { calls: number; triangles: number; drawCalls?: number }; memory: { geometries: number; textures: number } } };
  // 후처리가 한 프레임에 render 를 여러 번 부르므로 자동 초기화를 끄고, 읽은 뒤에 우리가 비운다 (프레임 전체 합계가 나온다)
  useEffect(() => {
    gl.info.autoReset = false;
    return () => {
      gl.info.autoReset = true;
    };
  }, [gl]);
  const scene = useThree((s) => s.scene);
  useEffect(() => {
    sceneRef.current = scene;
  }, [scene]);
  const acc = useRef({ t: 0, n: 0, worst: 0, last: 0 });
  const el = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (typeof location === 'undefined' || !new URLSearchParams(location.search).has('stats')) return;
    const d = document.createElement('div');
    d.style.cssText = 'position:fixed;left:8px;bottom:8px;z-index:50;font:11px/1.35 monospace;color:#cfe;background:rgba(0,0,0,.55);padding:4px 7px;border-radius:5px;pointer-events:none;white-space:pre';
    document.body.appendChild(d);
    el.current = d;
    return () => {
      d.remove();
      el.current = null;
    };
  }, []);
  useFrame((_, delta) => {
    const a = acc.current;
    a.t += delta;
    a.n++;
    a.worst = Math.max(a.worst, delta);
    const info = gl.info;
    if (a.t >= 0.5) {
      const calls = info.render.drawCalls ?? info.render.calls;
      renderStats.current = {
        fps: +(a.n / a.t).toFixed(1),
        frameMs: +((a.t / a.n) * 1000).toFixed(2),
        worstMs: +(a.worst * 1000).toFixed(1),
        calls,
        triangles: info.render.triangles,
        geometries: info.memory.geometries,
        textures: info.memory.textures,
      };
      if (el.current) el.current.textContent = `${renderStats.current.fps} fps  ${renderStats.current.frameMs} ms\ncalls ${calls}  tris ${(info.render.triangles / 1000).toFixed(0)}k\ngeo ${info.memory.geometries}  tex ${info.memory.textures}`;
      a.t = 0;
      a.n = 0;
      a.worst = 0;
    }
    info.reset();
  });
  return null;
}
