/**
 * 구름 텍스처: 위아래로 이음매 없이 도는 값 잡음(fbm)으로 알파 구름을 그린다 (equirect, 하늘 돔 안쪽에 씌운다).
 * 아랫면은 회색 그림자, 윗면은 흰색으로 칠해서 납작하지 않게 한다.
 */
import { CanvasTexture, ClampToEdgeWrapping, RepeatWrapping, SRGBColorSpace } from 'three';

function hash(ix: number, iy: number, seed: number): number {
  let h = (ix * 374761393 + iy * 668265263 + seed * 144665) | 0;
  h = (h ^ (h >>> 13)) * 1274126177;
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

/** 가로로 W 칸에서 이음매 없이 도는 값 잡음 */
function vnoise(x: number, y: number, period: number, seed: number): number {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const fx = x - ix;
  const fy = y - iy;
  const sx = fx * fx * (3 - 2 * fx);
  const sy = fy * fy * (3 - 2 * fy);
  const wrap = (i: number) => ((i % period) + period) % period;
  const a = hash(wrap(ix), iy, seed);
  const b = hash(wrap(ix + 1), iy, seed);
  const c = hash(wrap(ix), iy + 1, seed);
  const d = hash(wrap(ix + 1), iy + 1, seed);
  return a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy;
}

/** 구름 텍스처 (W×H). cover = 구름 양(0–1), 클수록 많다 */
export function cloudTexture(cover = 0.5, seed = 5, W = 1024, H = 512): CanvasTexture {
  const c = document.createElement('canvas');
  c.width = W;
  c.height = H;
  const ctx = c.getContext('2d')!;
  const img = ctx.createImageData(W, H);
  const base = 6; // 가로 기본 주기 (칸)
  const dens = new Float32Array(W * H);
  for (let y = 0; y < H; y++) {
    const v = 1 - y / H; // 1 = 천정, 0 = 바닥
    for (let x = 0; x < W; x++) {
      let amp = 0.5;
      let freq = base;
      let sum = 0;
      let norm = 0;
      for (let o = 0; o < 6; o++) {
        // 위로 갈수록(천정 근처) 가로 주기가 같아도 길어 보이므로 세로 주파수를 약간 줄인다
        sum += amp * vnoise((x / W) * freq, (y / H) * freq * 0.55, freq, seed + o * 17);
        norm += amp;
        amp *= 0.52;
        freq *= 2;
      }
      dens[y * W + x] = sum / norm;
      void v;
    }
  }
  // 알파: 임계값 위만 구름. 아랫면 그림자는 위쪽 이웃 밀도의 차로 낸다
  const lo = 0.62 - cover * 0.2;
  const hi = lo + 0.13;
  for (let y = 0; y < H; y++) {
    const v = 1 - y / H;
    const horizonFade = Math.min(1, Math.max(0, (v - 0.5) / 0.14)); // 지평선 근처는 옅게
    for (let x = 0; x < W; x++) {
      const d = dens[y * W + x]!;
      const t = Math.min(1, Math.max(0, (d - lo) / (hi - lo)));
      const a = t * t * (3 - 2 * t) * horizonFade;
      const above = dens[Math.max(0, y - 6) * W + x]!;
      const shade = Math.min(1, Math.max(0, (d - above) * 5 + 0.5)); // 위쪽이 더 옅으면(경계 위) 밝게
      const g = 205 + shade * 50;
      const i = (y * W + x) * 4;
      img.data[i] = g;
      img.data[i + 1] = g + 2;
      img.data[i + 2] = Math.min(255, g + 8);
      img.data[i + 3] = a * 235;
    }
  }
  ctx.putImageData(img, 0, 0);
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.wrapS = RepeatWrapping;
  t.wrapT = ClampToEdgeWrapping;
  t.anisotropy = 4;
  t.needsUpdate = true;
  return t;
}
