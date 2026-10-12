/**
 * 결정적 값 잡음(정수 해시): 지형·숲·얼룩이 쓴다. 같은 시드면 어느 기계에서나 같은 모양이 나온다.
 * 옥타브마다 좌표를 돌려서 격자 방향 무늬를 숨긴다.
 */

function ihash(x: number, y: number, s: number): number {
  let h = (Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(s, 1442695041)) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

/** 부드러운 값 잡음 0–1 */
export function vnoise(x: number, y: number, seed = 0): number {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const fx = x - ix;
  const fy = y - iy;
  const u = fx * fx * fx * (fx * (fx * 6 - 15) + 10);
  const v = fy * fy * fy * (fy * (fy * 6 - 15) + 10);
  const a = ihash(ix, iy, seed);
  const b = ihash(ix + 1, iy, seed);
  const c = ihash(ix, iy + 1, seed);
  const d = ihash(ix + 1, iy + 1, seed);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

const COS = Math.cos(0.62);
const SIN = Math.sin(0.62);

/** 분수 브라운 운동 0–1 (옥타브 수 oct) */
export function fbm(x: number, y: number, oct: number, seed = 0): number {
  let amp = 0.5;
  let sum = 0;
  let norm = 0;
  let px = x;
  let py = y;
  for (let o = 0; o < oct; o++) {
    sum += amp * vnoise(px, py, seed + o * 31);
    norm += amp;
    amp *= 0.5;
    const nx = (px * COS - py * SIN) * 2.03 + 17.3;
    py = (px * SIN + py * COS) * 2.03 - 9.1;
    px = nx;
  }
  return sum / norm;
}

/** 능선 잡음 0–1: 봉우리 줄기가 이어진다 (산맥) */
export function ridged(x: number, y: number, oct: number, seed = 0): number {
  let amp = 0.5;
  let sum = 0;
  let norm = 0;
  let w = 1;
  let px = x;
  let py = y;
  for (let o = 0; o < oct; o++) {
    let n = 1 - Math.abs(2 * vnoise(px, py, seed + o * 17) - 1);
    n *= n;
    n *= w;
    w = Math.min(1, n * 2);
    sum += n * amp;
    norm += amp;
    amp *= 0.5;
    const nx = (px * COS - py * SIN) * 2.1 + 5.7;
    py = (px * SIN + py * COS) * 2.1 + 3.3;
    px = nx;
  }
  return sum / norm;
}

export function smooth01(t: number): number {
  const x = t < 0 ? 0 : t > 1 ? 1 : t;
  return x * x * (3 - 2 * x);
}

export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

/** 시드가 정해진 난수 (0–1) */
export function rngOf(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) / 4294967296;
  };
}
