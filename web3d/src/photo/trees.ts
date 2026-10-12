/**
 * 나무: 소나무 수관(여러 층의 처진 치마 + 잔가지 요철)과 활엽수 수관(울퉁불퉁한 덩어리) 지오메트리, 솔잎·잎 텍스처.
 * 수관 지오메트리는 높이 1, 밑 반지름 1, 가운데가 원점이라 기존 원뿔(`coneGeometry(1, 1)`)을 그대로 바꿔 끼운다.
 * 텍스처는 캔버스로 그린다 (알베도 + 높이에서 뽑은 법선). 표준 재질만 쓰므로 WebGPU·WebGL 공통.
 */
import { BufferAttribute, BufferGeometry, CanvasTexture, type Texture } from 'three';

import { canvas, normalFromHeight, rng, tex } from './surfaces.ts';

/** 소나무 수관: tiers 층의 처진 원뿔 치마를 쌓는다. 층마다 반지름·높이·위상을 흔들고 아랫단이 넓다 */
export function pineGeometry(seed = 1, tiers = 9, seg = 14): BufferGeometry {
  const r = rng(seed);
  const pos: number[] = [];
  const uv: number[] = [];
  const idx: number[] = [];
  let base = 0;
  for (let t = 0; t < tiers; t++) {
    const k = t / (tiers - 1); // 0 = 맨 아래, 1 = 맨 위
    const yBase = -0.5 + k * 0.66 + (r() - 0.5) * 0.02;
    const h = 0.34 - k * 0.12 + r() * 0.04;
    const radius = (1 - k * 0.86) * (0.93 + r() * 0.12);
    const phase = r() * Math.PI * 2;
    // 아래 가장자리 고리(들쭉날쭉) 와 위 꼭짓점. 가장자리 바로 안쪽에도 고리를 하나 더 둬 처진 모양을 만든다
    const ringOuter = base;
    for (let i = 0; i <= seg; i++) {
      const a = phase + (i / seg) * Math.PI * 2;
      const j = 1 + (r() - 0.5) * 0.22 + (i % 2 === 0 ? 0.07 : -0.05);
      pos.push(Math.cos(a) * radius * j, yBase - h * 0.09, Math.sin(a) * radius * j);
      uv.push(i / seg, 0);
    }
    const ringMid = base + seg + 1;
    for (let i = 0; i <= seg; i++) {
      const a = phase + (i / seg) * Math.PI * 2;
      const j = 0.62 + (r() - 0.5) * 0.08;
      pos.push(Math.cos(a) * radius * j, yBase + h * 0.42, Math.sin(a) * radius * j);
      uv.push(i / seg, 0.5);
    }
    const apex = base + 2 * (seg + 1);
    pos.push(0, yBase + h, 0);
    uv.push(0.5, 1);
    base += 2 * (seg + 1) + 1;
    for (let i = 0; i < seg; i++) {
      idx.push(ringOuter + i, ringOuter + i + 1, ringMid + i);
      idx.push(ringOuter + i + 1, ringMid + i + 1, ringMid + i);
      idx.push(ringMid + i, ringMid + i + 1, apex);
    }
    // 아랫면 덮개 (아래에서 올려다볼 때 속이 비지 않게)
    const center = base;
    pos.push(0, yBase - h * 0.05, 0);
    uv.push(0.5, 0.2);
    base += 1;
    for (let i = 0; i < seg; i++) idx.push(ringOuter + i + 1, ringOuter + i, center);
  }
  const g = new BufferGeometry();
  g.setAttribute('position', new BufferAttribute(new Float32Array(pos), 3));
  g.setAttribute('uv', new BufferAttribute(new Float32Array(uv), 2));
  g.setIndex(idx);
  g.computeVertexNormals();
  return g;
}

/** 활엽수 수관: 구를 소음으로 일그러뜨린 덩어리 여러 개를 합친다 (높이 1, 반지름 1 에 맞춤) */
export function broadleafGeometry(seed = 3, blobs = 7, seg = 18): BufferGeometry {
  const r = rng(seed);
  const pos: number[] = [];
  const uv: number[] = [];
  const idx: number[] = [];
  let base = 0;
  for (let b = 0; b < blobs; b++) {
    const cx = (r() - 0.5) * 1.1;
    const cz = (r() - 0.5) * 1.1;
    const cy = (r() - 0.5) * 0.45 + (b === 0 ? 0.05 : 0);
    const rad = 0.45 + r() * 0.2;
    const lumps = 5 + Math.floor(r() * 4);
    const ph = r() * 6.28;
    for (let i = 0; i <= seg; i++) {
      const v = i / seg;
      const th = v * Math.PI;
      for (let j = 0; j <= seg; j++) {
        const u = j / seg;
        const ph2 = u * Math.PI * 2;
        const bump = 1 + 0.13 * Math.sin(lumps * ph2 + ph) * Math.sin(th * 3 + ph) + 0.07 * Math.sin(th * 7 + b);
        const rr = rad * bump;
        pos.push(cx + Math.cos(ph2) * Math.sin(th) * rr, cy + Math.cos(th) * rr * 0.82, cz + Math.sin(ph2) * Math.sin(th) * rr);
        uv.push(u * 2, v * 2);
      }
    }
    for (let i = 0; i < seg; i++) {
      for (let j = 0; j < seg; j++) {
        const a = base + i * (seg + 1) + j;
        const c = a + seg + 1;
        idx.push(a, c, a + 1, a + 1, c, c + 1);
      }
    }
    base += (seg + 1) * (seg + 1);
  }
  const g = new BufferGeometry();
  g.setAttribute('position', new BufferAttribute(new Float32Array(pos), 3));
  g.setAttribute('uv', new BufferAttribute(new Float32Array(uv), 2));
  g.setIndex(idx);
  g.computeVertexNormals();
  // 높이 1, 가운데 원점으로 맞춘다
  g.computeBoundingBox();
  const bb = g.boundingBox!;
  const sy = 1 / (bb.max.y - bb.min.y);
  const sxz = 1 / Math.max(bb.max.x - bb.min.x, bb.max.z - bb.min.z) * 2;
  const p = g.getAttribute('position');
  const my = (bb.max.y + bb.min.y) / 2;
  for (let i = 0; i < p.count; i++) p.setXYZ(i, p.getX(i) * sxz, (p.getY(i) - my) * sy, p.getZ(i) * sxz);
  g.computeVertexNormals();
  return g;
}

export interface TreeMaps {
  map: Texture;
  normalMap: Texture;
}

const cache = new Map<string, TreeMaps>();

/** 솔잎 텍스처: 어두운 바탕에 가는 잎 획 수천 개 (바늘 방향은 아래로 처진 쪽) */
export function needleMaps(): TreeMaps {
  const hit = cache.get('needle');
  if (hit) return hit;
  const N = 512;
  const [ac, actx] = canvas(N, N);
  const [hc, hctx] = canvas(N, N);
  actx.fillStyle = '#2a4a22';
  actx.fillRect(0, 0, N, N);
  hctx.fillStyle = '#383838';
  hctx.fillRect(0, 0, N, N);
  const r = rng(77);
  const pal = ['#1d3d19', '#2a5222', '#386a2c', '#4a7f36', '#5f9640', '#78a64a'];
  actx.lineCap = 'round';
  hctx.lineCap = 'round';
  for (let i = 0; i < 16000; i++) {
    const x = r() * N;
    const y = r() * N;
    const a = Math.PI / 2 + (r() - 0.5) * 1.6;
    const len = 7 + r() * 16;
    const x2 = x + Math.cos(a) * len;
    const y2 = y + Math.sin(a) * len;
    const col = pal[Math.floor(r() * r() * pal.length)]!;
    const offs: [number, number][] = [[0, 0]];
    if (x < len || x > N - len) offs.push([x < N / 2 ? N : -N, 0]);
    if (y < len || y > N - len) offs.push([0, y < N / 2 ? N : -N]);
    const g = Math.round(90 + r() * 150);
    for (const [ox, oy] of offs) {
      actx.strokeStyle = col;
      actx.globalAlpha = 0.5 + r() * 0.4;
      actx.lineWidth = 1.3 + r() * 1.2;
      actx.beginPath();
      actx.moveTo(x + ox, y + oy);
      actx.lineTo(x2 + ox, y2 + oy);
      actx.stroke();
      hctx.strokeStyle = `rgb(${g},${g},${g})`;
      hctx.globalAlpha = 0.8;
      hctx.lineWidth = 1.6;
      hctx.beginPath();
      hctx.moveTo(x + ox, y + oy);
      hctx.lineTo(x2 + ox, y2 + oy);
      hctx.stroke();
    }
  }
  actx.globalAlpha = 1;
  const out = { map: tex(ac, true), normalMap: tex(normalFromHeight(hc, 3.2), false) };
  cache.set('needle', out);
  return out;
}

/** 잎 덩어리 텍스처: 둥근 잎 점·획 (활엽수) */
export function leafMaps(): TreeMaps {
  const hit = cache.get('leaf');
  if (hit) return hit;
  const N = 512;
  const [ac, actx] = canvas(N, N);
  const [hc, hctx] = canvas(N, N);
  actx.fillStyle = '#2f5a22';
  actx.fillRect(0, 0, N, N);
  hctx.fillStyle = '#383838';
  hctx.fillRect(0, 0, N, N);
  const r = rng(91);
  const pal = ['#264a1c', '#33602a', '#427a33', '#559040', '#6aa34a', '#86b350', '#a3b955'];
  for (let i = 0; i < 9000; i++) {
    const x = r() * N;
    const y = r() * N;
    const rad = 3 + r() * 5;
    const col = pal[Math.floor(r() * r() * pal.length)]!;
    const rot = r() * Math.PI;
    const g = Math.round(90 + r() * 150);
    for (const [ox, oy] of [[0, 0], [N, 0], [-N, 0], [0, N], [0, -N]] as const) {
      if ((ox !== 0 && x > rad && x < N - rad) || (oy !== 0 && y > rad && y < N - rad)) continue;
      actx.globalAlpha = 0.6 + r() * 0.3;
      actx.fillStyle = col;
      actx.beginPath();
      actx.ellipse(x + ox, y + oy, rad, rad * 0.55, rot, 0, Math.PI * 2);
      actx.fill();
      hctx.globalAlpha = 0.8;
      hctx.fillStyle = `rgb(${g},${g},${g})`;
      hctx.beginPath();
      hctx.ellipse(x + ox, y + oy, rad, rad * 0.55, rot, 0, Math.PI * 2);
      hctx.fill();
    }
  }
  actx.globalAlpha = 1;
  const out = { map: tex(ac, true), normalMap: tex(normalFromHeight(hc, 2.6), false) };
  cache.set('leaf', out);
  return out;
}

export type { CanvasTexture };
