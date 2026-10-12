/**
 * 효과 입자: 착지 흙먼지(잔디)·모래(벙커)·물보라(물), 홀인 축하(꽃가루). 상태가 바뀌는 순간 store 구독으로 뿌리고,
 * 프레임마다 중력·끌림으로 움직인다. 입자는 인스턴스 하나(최대 MAX 개)로 그린다.
 */
import { useEffect, useMemo, useRef } from 'react';

import { useEngineFrame } from 'gaesup-world';
import { Color, Object3D, type InstancedMesh } from 'three';

import type { Surface } from '../course.ts';
import { game, holeOf, shotClock } from '../round.ts';

const MAX = 420;

interface P {
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  life: number;
  max: number;
  size: number;
  g: number;
  drag: number;
}

const DUST: Partial<Record<Surface, string[]>> = {
  bunker: ['#e8d6a4', '#d9c18a', '#f2e5c0'],
  water: ['#e8f6ff', '#bfe3ff', '#ffffff'],
  rough: ['#5c8f3c', '#7a5a35', '#6e9e45'],
};
const GRASS = ['#86c556', '#6c4f30', '#9bd36b'];
const LEAVES = ['#3f7a34', '#5a9a3f', '#2f5a2a', '#7aa84a'];
const CONFETTI = ['#ffd23f', '#ff6a8a', '#5ec8ff', '#9be37a', '#ffffff', '#ff8b2a'];

export function GameEffects() {
  const mesh = useRef<InstancedMesh>(null);
  const pool = useMemo<P[]>(() => [], []);
  const tmp = useMemo(() => ({ o: new Object3D(), c: new Color() }), []);
  const next = useRef(0);
  // 나무에 맞은 순간(비행 시계가 맞은 시각에 닿을 때) 잎을 흩뿌린다: effect 안의 spawn 을 프레임 고리에서도 쓰려고 ref 로 건넨다
  const burst = useRef<((x: number, y: number, z: number) => void) | null>(null);
  const treeDone = useRef<unknown>(null);

  useEffect(() => {
    const spawn = (n: number, x: number, z: number, colors: string[], f: (k: number) => Partial<P>) => {
      const m = mesh.current;
      for (let k = 0; k < n; k++) {
        const i = next.current;
        next.current = (i + 1) % MAX;
        const p: P = { x, y: 0.05, z, vx: 0, vy: 0, vz: 0, life: 1, max: 1, size: 0.08, g: 9.81, drag: 0.6, ...f(k) };
        pool[i] = p;
        if (m) m.setColorAt(i, tmp.c.set(colors[k % colors.length]!));
      }
      if (m?.instanceColor) m.instanceColor.needsUpdate = true;
    };
    const rnd = (a: number, b: number) => a + Math.random() * (b - a);
    burst.current = (x, y, z) =>
      spawn(34, x, z, LEAVES, () => {
        const a = rnd(0, 2 * Math.PI);
        const r = rnd(0.4, 2.4);
        return { y, vx: Math.cos(a) * r, vz: Math.sin(a) * r, vy: rnd(-0.5, 2.5), max: rnd(0.9, 1.6), size: rnd(0.08, 0.16), g: 4.5, drag: 1.1 };
      });
    let prev = game.get();
    return game.subscribe(() => {
      const s = game.get();
      const sh = s.shot;
      // 착지
      if (sh && !sh.putt && prev.phase === 'flight' && s.phase === 'ground') {
        const p0 = sh.ground.samples[0]!;
        const surf = sh.ground.firstSurface;
        if (surf === 'water') {
          spawn(46, p0.x, p0.z, DUST.water!, () => {
            const a = rnd(0, 2 * Math.PI);
            const r = rnd(0.5, 2.2);
            return { vx: Math.cos(a) * r, vz: Math.sin(a) * r, vy: rnd(3, 7.5), max: rnd(0.8, 1.3), size: rnd(0.18, 0.36), drag: 0.3 };
          });
        } else {
          const sand = surf === 'bunker';
          spawn(sand ? 40 : 26, p0.x, p0.z, DUST[surf] ?? GRASS, () => {
            const a = rnd(0, 2 * Math.PI);
            const r = rnd(0.4, sand ? 2 : 1.3);
            return { vx: Math.cos(a) * r, vz: Math.sin(a) * r, vy: rnd(1.5, sand ? 4.2 : 3), max: rnd(0.6, 1.0), size: rnd(0.1, sand ? 0.26 : 0.18), drag: 1.2 };
          });
        }
      }
      // 굴러서 물에 빠짐
      if (sh && prev.phase === 'ground' && s.phase !== 'ground' && sh.ground.event === 'water' && sh.ground.firstSurface !== 'water') {
        const end = sh.ground.samples[sh.ground.samples.length - 1]!;
        spawn(34, end.x, end.z, DUST.water!, () => ({ vx: rnd(-1.2, 1.2), vz: rnd(-1.2, 1.2), vy: rnd(2.5, 5.5), max: rnd(0.7, 1.1), size: rnd(0.15, 0.3), drag: 0.3 }));
      }
      // 홀인: 꽃가루 (버디 이상·챌린지는 많이)
      if (sh && (prev.phase === 'flight' || prev.phase === 'ground') && sh.ground.event === 'holed') {
        const [px, pz] = holeOf(s).pin;
        const big = s.mode !== 'stroke' || prev.strokes <= holeOf(s).par - 1;
        spawn(big ? 160 : 50, px, pz, CONFETTI, () => {
          const a = rnd(0, 2 * Math.PI);
          const r = rnd(0.5, big ? 3.5 : 2);
          return { y: 0.3, vx: Math.cos(a) * r, vz: Math.sin(a) * r, vy: rnd(3, big ? 8 : 5), max: rnd(1.4, 2.6), size: rnd(0.07, 0.15), g: 3.2, drag: 1.4 };
        });
      }
      prev = s;
    });
  }, [pool, tmp]);

  useEngineFrame(
    'lateUpdate',
    (delta) => {
      const m = mesh.current;
      if (!m) return;
      const gs = game.get();
      const sh = gs.shot;
      if (sh?.tree && (gs.phase === 'flight' || gs.phase === 'ground') && treeDone.current !== sh && shotClock.t >= sh.tree.t) {
        treeDone.current = sh;
        burst.current?.(sh.tree.x, sh.tree.y, sh.tree.z);
      }
      const dt = Math.min(delta, 0.05);
      const o = tmp.o;
      let any = false;
      for (let i = 0; i < MAX; i++) {
        const p = pool[i];
        if (!p || p.life <= 0) {
          o.scale.setScalar(0);
          o.position.set(0, -100, 0);
        } else {
          any = true;
          p.vy -= p.g * dt;
          const k = Math.exp(-p.drag * dt);
          p.vx *= k;
          p.vz *= k;
          p.x += p.vx * dt;
          p.y = Math.max(p.y + p.vy * dt, 0.01);
          p.z += p.vz * dt;
          p.life -= dt / p.max;
          o.position.set(p.x, p.y, p.z);
          o.rotation.set(p.life * 9, p.life * 7, 0);
          o.scale.setScalar(p.size * Math.max(p.life, 0) ** 0.5);
        }
        o.updateMatrix();
        m.setMatrixAt(i, o.matrix);
      }
      m.instanceMatrix.needsUpdate = true;
      m.visible = any;
    },
    { label: 'golf:effects' },
  );

  return (
    <instancedMesh ref={mesh} args={[undefined, undefined, MAX]} frustumCulled={false} renderOrder={6}>
      <boxGeometry args={[1, 1, 1]} />
      <meshBasicMaterial toneMapped={false} />
    </instancedMesh>
  );
}
