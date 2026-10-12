/**
 * 지면 재질 텍스처 (WebGPU·WebGL 공통, 표준 재질만 쓴다).
 *  - 잔디: 캔버스로 풀잎 수만 개를 그려 알베도와 높이 지도를 만들고, 높이 지도에서 법선 지도를 뽑는다.
 *    러프(길고 거침), 페어웨이(짧고 촘촘), 그린(아주 짧고 곱게) 세 가지.
 *  - 모래·흙 같은 자연 재질은 Poly Haven(CC0) 에서 받아 `public/tex/*_a.webp` (알베도×AO), `*_n.webp` (R,G = 법선 xy, B = 거칠기)
 *    로 묶어 둔 것을 읽어 법선 지도(+거칠기 지도)로 푼다 (`scripts/fetch_textures.py`, 출처는 `public/ASSETS.md`).
 *  - 큰 규모의 얼룩(macro)과 깎은 줄무늬는 알파 판으로 따로 만들어 지면 위에 한 겹 덮는다 (타일 반복을 가린다).
 *
 * 품질 프로필(`quality.ts`): 이방성 필터는 기본 4(모바일·XR 2), 캔버스 큰 질감 한 변은 기본 1024 px(모바일·XR 512 px,
 * 풀잎 수·길이·굵기도 같은 비율로 줄여 한 장이 덮는 모양은 같다). 만든 질감은 모듈 안에 캐시해 홀이 바뀌어도 다시 그리지 않는다.
 */
import { CanvasTexture, ClampToEdgeWrapping, LinearMipmapLinearFilter, RepeatWrapping, SRGBColorSpace, NoColorSpace, type Texture } from 'three';

import { currentProfile } from './quality.ts';

export interface PbrMaps {
  map: Texture;
  normalMap: Texture;
  /** 있으면 거칠기 지도 (G 채널을 쓰는 three 규약에 맞춰 G 에 넣어 둔다) */
  roughnessMap?: Texture;
}

export function canvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  const ctx = c.getContext('2d', { willReadFrequently: true });
  if (!ctx) throw new Error('2d canvas 를 만들 수 없음');
  return [c, ctx];
}

export function rng(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) / 4294967296;
  };
}

export function tex(c: HTMLCanvasElement, srgb: boolean): CanvasTexture {
  const t = new CanvasTexture(c);
  t.colorSpace = srgb ? SRGBColorSpace : NoColorSpace;
  t.wrapS = RepeatWrapping;
  t.wrapT = RepeatWrapping;
  t.anisotropy = currentProfile().anisotropy;
  t.generateMipmaps = true;
  t.minFilter = LinearMipmapLinearFilter;
  t.needsUpdate = true;
  return t;
}

/** 높이 지도(회색조 캔버스)에서 접선 공간 법선 지도 (OpenGL 규약, 반복 경계 포함). strength 가 클수록 거칠다 */
export function normalFromHeight(h: HTMLCanvasElement, strength: number): HTMLCanvasElement {
  const w = h.width;
  const hh = h.height;
  const src = h.getContext('2d')!.getImageData(0, 0, w, hh).data;
  const [c, ctx] = canvas(w, hh);
  const out = ctx.createImageData(w, hh);
  const at = (x: number, y: number) => src[(((y + hh) % hh) * w + ((x + w) % w)) * 4]! / 255;
  for (let y = 0; y < hh; y++) {
    for (let x = 0; x < w; x++) {
      const dx = (at(x + 1, y) - at(x - 1, y)) * strength;
      const dy = (at(x, y + 1) - at(x, y - 1)) * strength;
      const l = Math.hypot(dx, dy, 1);
      const i = (y * w + x) * 4;
      out.data[i] = ((-dx / l) * 0.5 + 0.5) * 255;
      out.data[i + 1] = ((dy / l) * 0.5 + 0.5) * 255; // OpenGL: +Y 위
      out.data[i + 2] = ((1 / l) * 0.5 + 0.5) * 255;
      out.data[i + 3] = 255;
    }
  }
  ctx.putImageData(out, 0, 0);
  return c;
}

interface LawnStyle {
  /** 풀잎 수, 길이 범위 (px), 굵기 (px), 기울기 폭 (rad) */
  blades: number;
  len: [number, number];
  width: number;
  spread: number;
  /** 풀 색 팔레트 (어두움 → 밝음) */
  palette: string[];
  /** 바닥(뿌리 쪽) 색 */
  base: string;
  /** 법선 세기 */
  bump: number;
  seed: number;
}

const LAWN: Record<'rough' | 'fairway' | 'green' | 'tee', LawnStyle> = {
  rough: { blades: 30000, len: [10, 26], width: 2.1, spread: 1.1, palette: ['#4c7a2e', '#5d8e36', '#6f9f3f', '#84ae4a', '#98b957', '#a8b560'], base: '#3a5c25', bump: 3.4, seed: 101 },
  fairway: { blades: 50000, len: [4, 10], width: 1.45, spread: 0.9, palette: ['#5f9a33', '#6faa3b', '#80ba46', '#92c855', '#a3d466'], base: '#4a7c2a', bump: 2.0, seed: 202 },
  green: { blades: 70000, len: [2.5, 6], width: 1.2, spread: 0.8, palette: ['#69a840', '#78b84a', '#88c656', '#98d266'], base: '#55933a', bump: 1.0, seed: 303 },
  tee: { blades: 50000, len: [3.5, 8], width: 1.3, spread: 0.85, palette: ['#58903a', '#669f44', '#75ae4e', '#85bb5b'], base: '#477a2f', bump: 1.5, seed: 404 },
};

const lawnCache = new Map<string, PbrMaps>();

/** 잔디 알베도·법선 (기본 1024 px, 모바일·XR 512 px. 한 변이 `LAWN_TILE_M` m 로 반복되게 쓴다) */
export function lawnMaps(kind: keyof typeof LAWN): PbrMaps {
  const hit = lawnCache.get(kind);
  if (hit) return hit;
  performance.mark(`lawn-${kind}`);
  const st = LAWN[kind];
  const N = currentProfile().texSize;
  const k = N / 1024; // 픽셀 단위 값(풀잎 길이·굵기)과 풀잎 수(넓이)를 한 변 비율에 맞춰 줄인다
  const [ac, actx] = canvas(N, N);
  const [hc, hctx] = canvas(N, N);
  actx.fillStyle = st.base;
  actx.fillRect(0, 0, N, N);
  hctx.fillStyle = '#303030';
  hctx.fillRect(0, 0, N, N);
  const r = rng(st.seed);
  actx.lineCap = 'round';
  hctx.lineCap = 'round';
  const blades = Math.round(st.blades * k * k);
  for (let i = 0; i < blades; i++) {
    const x = r() * N;
    const y = r() * N;
    const ang = -Math.PI / 2 + (r() - 0.5) * 2 * st.spread * (0.4 + r());
    const len = (st.len[0] + r() * (st.len[1] - st.len[0])) * k;
    const col = st.palette[Math.min(st.palette.length - 1, Math.floor(r() * r() * st.palette.length * 1.4))]!;
    const x2 = x + Math.cos(ang) * len;
    const y2 = y + Math.sin(ang) * len;
    // 타일 가장자리에서 이음매가 보이지 않게 네 번(가장자리 근처에서만) 그린다
    const offs: [number, number][] = [[0, 0]];
    if (x < len || x > N - len) offs.push([x < N / 2 ? N : -N, 0]);
    if (y < len || y > N - len) offs.push([0, y < N / 2 ? N : -N]);
    const shade = 0.55 + r() * 0.45;
    for (const [ox, oy] of offs) {
      actx.strokeStyle = col;
      actx.globalAlpha = 0.45 + r() * 0.4;
      actx.lineWidth = st.width * k * (0.7 + r() * 0.7);
      actx.beginPath();
      actx.moveTo(x + ox, y + oy);
      actx.lineTo(x2 + ox, y2 + oy);
      actx.stroke();
      const g = Math.round(110 + shade * 140);
      hctx.strokeStyle = `rgb(${g},${g},${g})`;
      hctx.globalAlpha = 0.7;
      hctx.lineWidth = st.width * k * 0.9;
      hctx.beginPath();
      hctx.moveTo(x + ox, y + oy);
      hctx.lineTo(x2 + ox, y2 + oy);
      hctx.stroke();
    }
  }
  actx.globalAlpha = 1;
  const maps: PbrMaps = { map: tex(ac, true), normalMap: tex(normalFromHeight(hc, st.bump), false) };
  lawnCache.set(kind, maps);
  performance.measure(`lawn texture ${kind}`, `lawn-${kind}`);
  return maps;
}

/**
 * 지형용 흑백 잔디 질감: 러프 잔디의 밝기 얼룩만 평균 0.8 로 맞춰 남긴다. 색은 정점 색이 정하므로 곱해도 색이 틀어지지 않는다.
 * 법선 지도는 러프 잔디의 것을 그대로 쓴다.
 */
export function terrainDetail(): PbrMaps {
  const hit = lawnCache.get('terrain');
  if (hit) return hit;
  const src = lawnMaps('rough');
  const img = src.map.image as HTMLCanvasElement;
  const d = img.getContext('2d')!.getImageData(0, 0, img.width, img.height);
  const lum = new Float32Array(img.width * img.height);
  let sum = 0;
  for (let i = 0, k = 0; i < d.data.length; i += 4, k++) {
    const l = 0.2126 * d.data[i]! + 0.7152 * d.data[i + 1]! + 0.0722 * d.data[i + 2]!;
    lum[k] = l;
    sum += l;
  }
  const gain = (0.8 * 255) / (sum / lum.length);
  const [c, ctx] = canvas(img.width, img.height);
  const out = ctx.createImageData(img.width, img.height);
  for (let i = 0, k = 0; i < out.data.length; i += 4, k++) {
    const g = Math.min(255, lum[k]! * gain);
    out.data[i] = g;
    out.data[i + 1] = g;
    out.data[i + 2] = g;
    out.data[i + 3] = 255;
  }
  ctx.putImageData(out, 0, 0);
  const maps: PbrMaps = { map: tex(c, true), normalMap: src.normalMap };
  lawnCache.set('terrain', maps);
  return maps;
}

/** 잔디 텍스처 한 장이 덮는 길이 (m) */
export const LAWN_TILE_M: Record<keyof typeof LAWN, number> = { rough: 2.2, fairway: 1.6, green: 1.2, tee: 1.4 };

const macroCache = new Map<string, CanvasTexture>();

/** 큰 규모 얼룩: 알파 판에 씌워 타일 반복을 가린다. 밝은 풀·마른 풀·그늘 얼룩이 섞인 부드러운 값 잡음 (같은 인수면 같은 텍스처) */
export function macroOverlay(seed: number, tones: [string, string]): CanvasTexture {
  const key = `${seed}:${tones[0]}:${tones[1]}`;
  const hit = macroCache.get(key);
  if (hit) return hit;
  const N = currentProfile().texSize / 2; // 기본 512, 모바일·XR 256
  const k = N / 512;
  const [c, ctx] = canvas(N, N);
  const r = rng(seed);
  ctx.clearRect(0, 0, N, N);
  for (let i = 0; i < 260; i++) {
    const x = r() * N;
    const y = r() * N;
    const rad = (14 + r() * 64) * k;
    const g = ctx.createRadialGradient(x, y, 0, x, y, rad);
    const col = tones[r() < 0.5 ? 0 : 1];
    g.addColorStop(0, `${col}${Math.round(10 + r() * 22).toString(16).padStart(2, '0')}`);
    g.addColorStop(1, `${col}00`);
    ctx.fillStyle = g;
    for (const [ox, oy] of [[0, 0], [N, 0], [-N, 0], [0, N], [0, -N]] as const) {
      ctx.beginPath();
      ctx.arc(x + ox, y + oy, rad, 0, Math.PI * 2);
      ctx.fill();
    }
  }
  const t = tex(c, true);
  macroCache.set(key, t);
  return t;
}

let stripesCache: CanvasTexture | null = null;

/** 깎은 줄무늬: 알파 판. 한 주기에 밝은 띠(흰색 약하게)와 어두운 띠(검정 약하게). u 방향이 주기 (한 번만 만든다) */
export function mowStripes(): CanvasTexture {
  if (stripesCache) return stripesCache;
  const [c, ctx] = canvas(256, 8);
  const g = ctx.createLinearGradient(0, 0, 256, 0);
  g.addColorStop(0, 'rgba(255,255,230,0.06)');
  g.addColorStop(0.46, 'rgba(255,255,230,0.06)');
  g.addColorStop(0.5, 'rgba(0,0,0,0)');
  g.addColorStop(0.54, 'rgba(10,30,0,0.08)');
  g.addColorStop(0.96, 'rgba(10,30,0,0.08)');
  g.addColorStop(1, 'rgba(255,255,230,0.06)');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, 256, 8);
  const t = tex(c, true);
  t.wrapT = ClampToEdgeWrapping;
  stripesCache = t;
  return t;
}

// ------------------------------------------------------------------ Poly Haven 묶음 읽기

const packedCache = new Map<string, PbrMaps>();

/** `public/tex/<name>_a.webp`, `_n.webp` 를 읽어 알베도·법선·거칠기 지도로 푼다 (기본 1024 px, 모바일·XR 512 px). 읽는 동안은 빈 지도가 돌아가고, 읽히면 같은 텍스처가 갱신된다 */
export function packedMaps(name: string, gain = 1, base = import.meta.env.BASE_URL): PbrMaps {
  const key = `${name}@${gain}`;
  const hit = packedCache.get(key);
  if (hit) return hit;
  const N = currentProfile().texSize;
  const [ac, actx] = canvas(N, N);
  const [nc, nctx] = canvas(N, N);
  const [rc, rctx] = canvas(N, N);
  actx.fillStyle = '#808080';
  actx.fillRect(0, 0, N, N);
  nctx.fillStyle = '#8080ff';
  nctx.fillRect(0, 0, N, N);
  rctx.fillStyle = '#c0c0c0';
  rctx.fillRect(0, 0, N, N);
  const maps: PbrMaps = { map: tex(ac, true), normalMap: tex(nc, false), roughnessMap: tex(rc, false) };
  const load = (url: string) =>
    new Promise<HTMLImageElement>((ok, bad) => {
      const im = new Image();
      im.onload = () => ok(im);
      im.onerror = () => bad(new Error(url));
      im.src = url;
    });
  void load(`${base}tex/${name}_a.webp`).then((im) => {
    actx.drawImage(im, 0, 0, N, N);
    if (gain !== 1) {
      // 알베도 밝기 보정 (벙커 모래를 밝게)
      const d = actx.getImageData(0, 0, N, N);
      for (let i = 0; i < d.data.length; i += 4) {
        d.data[i] = Math.min(255, d.data[i]! * gain);
        d.data[i + 1] = Math.min(255, d.data[i + 1]! * gain);
        d.data[i + 2] = Math.min(255, d.data[i + 2]! * gain * 0.94);
      }
      actx.putImageData(d, 0, 0);
    }
    maps.map.needsUpdate = true;
  });
  void load(`${base}tex/${name}_n.webp`).then((im) => {
    const [tc, tctx] = canvas(N, N);
    tctx.drawImage(im, 0, 0, N, N);
    const d = tctx.getImageData(0, 0, N, N);
    const n = nctx.createImageData(N, N);
    const rr = rctx.createImageData(N, N);
    for (let i = 0; i < d.data.length; i += 4) {
      const x = d.data[i]! / 255 * 2 - 1;
      const y = d.data[i + 1]! / 255 * 2 - 1;
      const z = Math.sqrt(Math.max(0, 1 - x * x - y * y));
      n.data[i] = d.data[i]!;
      n.data[i + 1] = d.data[i + 1]!;
      n.data[i + 2] = (z * 0.5 + 0.5) * 255;
      n.data[i + 3] = 255;
      const rough = d.data[i + 2]!;
      rr.data[i] = rough;
      rr.data[i + 1] = rough;
      rr.data[i + 2] = rough;
      rr.data[i + 3] = 255;
    }
    nctx.putImageData(n, 0, 0);
    rctx.putImageData(rr, 0, 0);
    maps.normalMap.needsUpdate = true;
    maps.roughnessMap!.needsUpdate = true;
    void tc;
  });
  packedCache.set(key, maps);
  return maps;
}

const tiledCache = new Map<string, PbrMaps>();

/** 같은 그림을 다른 반복 횟수로 쓴다 (복제는 원본과 이미지를 공유한다). 같은 인수면 같은 복제를 돌려줘 홀이 바뀌어도 새로 만들지 않는다 */
export function tiled(src: PbrMaps, rx: number, ry: number): PbrMaps {
  const key = `${src.map.uuid}:${rx}:${ry}`;
  const hit = tiledCache.get(key);
  if (hit) return hit;
  const one = (t: Texture): Texture => {
    const c = t.clone();
    c.repeat.set(rx, ry);
    c.needsUpdate = true;
    return c;
  };
  const out: PbrMaps = { map: one(src.map), normalMap: one(src.normalMap), roughnessMap: src.roughnessMap ? one(src.roughnessMap) : undefined };
  tiledCache.set(key, out);
  return out;
}

/** 텍스처 하나를 반복 횟수만 바꿔 쓴다 */
export function repeated(t: Texture, rx: number, ry: number): Texture {
  const key = `${t.uuid}:${rx}:${ry}:single`;
  const hit = tiledCache.get(key);
  if (hit) return hit.map;
  const c = t.clone();
  c.repeat.set(rx, ry);
  c.needsUpdate = true;
  tiledCache.set(key, { map: c, normalMap: c });
  return c;
}

// ------------------------------------------------------------------ 공

let ballCache: Texture | null = null;

/** 골프공 딤플 법선 지도 (equirect 1024×512): 피보나치 구 위 336개 점에 오목한 원. 극 쪽은 가로로 늘려 그린다 */
export function ballDimples(): Texture {
  if (ballCache) return ballCache;
  const W = 1024;
  const H = 512;
  const [hc, hctx] = canvas(W, H);
  hctx.fillStyle = 'rgb(205,205,205)';
  hctx.fillRect(0, 0, W, H);
  const N = 336;
  const golden = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < N; i++) {
    const y = 1 - (2 * (i + 0.5)) / N;
    const lat = Math.asin(y);
    const lon = ((i * golden + Math.PI) % (2 * Math.PI)) - Math.PI;
    const cx = ((lon + Math.PI) / (2 * Math.PI)) * W;
    const cy = (0.5 - lat / Math.PI) * H;
    const r = 7.4;
    const stretch = 1 / Math.max(0.22, Math.cos(lat));
    for (const ox of [0, -W, W]) {
      const x = cx + ox;
      if (x < -r * stretch * 2 || x > W + r * stretch * 2) continue;
      hctx.save();
      hctx.translate(x, cy);
      hctx.scale(stretch, 1);
      const g = hctx.createRadialGradient(0, 0, 0, 0, 0, r);
      g.addColorStop(0, 'rgb(40,40,40)');
      g.addColorStop(0.7, 'rgb(120,120,120)');
      g.addColorStop(1, 'rgba(205,205,205,0)');
      hctx.fillStyle = g;
      hctx.beginPath();
      hctx.arc(0, 0, r, 0, Math.PI * 2);
      hctx.fill();
      hctx.restore();
    }
  }
  const t = tex(normalFromHeight(hc, 5), false);
  t.wrapT = ClampToEdgeWrapping;
  ballCache = t;
  return t;
}

// ------------------------------------------------------------------ 물

function pnoise(x: number, y: number, period: number, seed: number): number {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const fx = x - ix;
  const fy = y - iy;
  const u = fx * fx * (3 - 2 * fx);
  const v = fy * fy * (3 - 2 * fy);
  const w = (i: number) => ((i % period) + period) % period;
  const h = (a: number, b: number) => {
    let n = (Math.imul(w(a), 374761393) + Math.imul(w(b), 668265263) + Math.imul(seed, 1442695041)) | 0;
    n = Math.imul(n ^ (n >>> 13), 1274126177);
    return ((n ^ (n >>> 16)) >>> 0) / 4294967296;
  };
  const a = h(ix, iy);
  const b = h(ix + 1, iy);
  const c = h(ix, iy + 1);
  const d = h(ix + 1, iy + 1);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

let rippleCache: Texture | null = null;

/** 잔물결 법선 지도 (타일, 512 px): 크고 작은 물결 세 겹. 모든 호수가 같이 쓰고 시간에 따라 offset 만 흘린다 */
export function waterRipple(): Texture {
  if (rippleCache) return rippleCache;
  const N = 512;
  const [hc, hctx] = canvas(N, N);
  const img = hctx.createImageData(N, N);
  for (let y = 0; y < N; y++) {
    for (let x = 0; x < N; x++) {
      let sum = 0;
      let amp = 1;
      let norm = 0;
      for (let o = 0; o < 3; o++) {
        const f = 4 * 2 ** o;
        // 한 방향으로 길쭉한 물결: 가로·세로 주파수를 다르게
        sum += amp * pnoise((x / N) * f, (y / N) * f * 1.7, f, 7 + o * 13);
        norm += amp;
        amp *= 0.5;
      }
      const g = (sum / norm) * 255;
      const i = (y * N + x) * 4;
      img.data[i] = g;
      img.data[i + 1] = g;
      img.data[i + 2] = g;
      img.data[i + 3] = 255;
    }
  }
  hctx.putImageData(img, 0, 0);
  rippleCache = tex(normalFromHeight(hc, 6), false);
  return rippleCache;
}

export interface LakeDepth {
  map: CanvasTexture;
  min: [number, number];
  size: [number, number];
}

const lakeCache = new WeakMap<readonly (readonly [number, number])[], Map<number, LakeDepth>>();

/**
 * 호수 깊이 색 지도: 다각형 안쪽 픽셀마다 가장자리까지 거리로 얕은 청록 → 깊은 남색, 둑 가장자리에는 연한 거품 띠.
 * uv 는 다각형의 외곽 상자 (x 방향 u, z 방향 v) 에 맞춘다.
 * 같은 다각형·깊이면 캔버스를 다시 그리지 않는다 (홀로 돌아와도). GPU 쪽은 `map.dispose()` 로 놓아도 다음에 쓰면 다시 올라간다.
 */
export function lakeDepthMap(poly: readonly (readonly [number, number])[], depthM = 9): LakeDepth {
  let byDepth = lakeCache.get(poly);
  if (!byDepth) lakeCache.set(poly, (byDepth = new Map()));
  const hit = byDepth.get(depthM);
  if (hit) return hit;
  const out = buildLakeDepth(poly, depthM);
  byDepth.set(depthM, out);
  return out;
}

function buildLakeDepth(poly: readonly (readonly [number, number])[], depthM: number): LakeDepth {
  const xs = poly.map((p) => p[0]);
  const zs = poly.map((p) => p[1]);
  const min: [number, number] = [Math.min(...xs), Math.min(...zs)];
  const size: [number, number] = [Math.max(...xs) - min[0], Math.max(...zs) - min[1]];
  const N = 256;
  const [c, ctx] = canvas(N, N);
  const img = ctx.createImageData(N, N);
  const sh = [0.40, 0.66, 0.62]; // 얕은 곳 (선형에 가까운 값, 아래에서 sRGB 로 쓴다)
  const dp = [0.06, 0.24, 0.34];
  const mid = [0.15, 0.46, 0.5];
  const inside = (x: number, z: number) => {
    let ins = false;
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      const a = poly[i]!;
      const b = poly[j]!;
      if (a[1] > z !== b[1] > z && x < ((b[0] - a[0]) * (z - a[1])) / (b[1] - a[1]) + a[0]) ins = !ins;
    }
    return ins;
  };
  for (let j = 0; j < N; j++) {
    for (let i = 0; i < N; i++) {
      const x = min[0] + ((i + 0.5) / N) * size[0];
      const z = min[1] + ((j + 0.5) / N) * size[1];
      const k = (j * N + i) * 4;
      if (!inside(x, z)) {
        img.data[k + 3] = 0;
        continue;
      }
      let d = Infinity;
      for (let e = 0; e < poly.length; e++) {
        const a = poly[e]!;
        const b = poly[(e + 1) % poly.length]!;
        const dx = b[0] - a[0];
        const dz = b[1] - a[1];
        const l2 = dx * dx + dz * dz;
        const t = l2 > 0 ? Math.min(Math.max(((x - a[0]) * dx + (z - a[1]) * dz) / l2, 0), 1) : 0;
        d = Math.min(d, Math.hypot(x - (a[0] + t * dx), z - (a[1] + t * dz)));
      }
      const t = Math.min(1, d / depthM);
      const s = t * t * (3 - 2 * t);
      const col = s < 0.5 ? sh.map((v, q) => v + (mid[q]! - v) * (s * 2)) : mid.map((v, q) => v + (dp[q]! - v) * ((s - 0.5) * 2));
      const foam = Math.max(0, 1 - d / 1.4);
      for (let q = 0; q < 3; q++) img.data[k + q] = Math.min(255, (col[q]! + foam * foam * 0.35) ** (1 / 2.2) * 255);
      img.data[k + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.wrapS = ClampToEdgeWrapping;
  t.wrapT = ClampToEdgeWrapping;
  t.anisotropy = currentProfile().anisotropy;
  t.needsUpdate = true;
  return { map: t, min, size };
}
