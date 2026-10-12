/**
 * 캔버스로 그리는 텍스처 (잔디, 페어웨이 줄무늬, 글자판). WebGPU·WebGL 둘 다에서 같은 기본 재질로 쓴다.
 */
import { CanvasTexture, RepeatWrapping, SRGBColorSpace } from 'three';

const FONT = '"Apple SD Gothic Neo", "Noto Sans KR", "Malgun Gothic", system-ui, sans-serif';

function canvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  const ctx = c.getContext('2d');
  if (!ctx) throw new Error('2d canvas 를 만들 수 없음');
  return [c, ctx];
}

/** 결정적 난수 (같은 그림이 매번 나오게) */
function rng(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) / 4294967296;
  };
}

function finish(c: HTMLCanvasElement, repeat: [number, number] | null): CanvasTexture {
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.anisotropy = 8;
  if (repeat) {
    t.wrapS = RepeatWrapping;
    t.wrapT = RepeatWrapping;
    t.repeat.set(repeat[0], repeat[1]);
  }
  t.needsUpdate = true;
  return t;
}

/** 잔디 얼룩: 밝기를 조금씩 바꾼 점과 짧은 획 */
export function grassTexture(base: string, repeat: [number, number], seed = 7): CanvasTexture {
  const [c, ctx] = canvas(256, 256);
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, 256, 256);
  const r = rng(seed);
  for (let i = 0; i < 2600; i++) {
    const x = r() * 256;
    const y = r() * 256;
    const light = r() < 0.5;
    ctx.fillStyle = light ? `rgba(255,255,220,${0.03 + r() * 0.05})` : `rgba(0,30,0,${0.04 + r() * 0.07})`;
    ctx.fillRect(x, y, 1 + r() * 3, 1 + r() * 3);
  }
  return finish(c, repeat);
}

/** 페어웨이 깎은 줄무늬: u 방향으로 밝은 띠·어두운 띠 한 쌍 */
export function stripeTexture(light: string, dark: string, repeat: [number, number]): CanvasTexture {
  const [c, ctx] = canvas(128, 64);
  ctx.fillStyle = light;
  ctx.fillRect(0, 0, 64, 64);
  ctx.fillStyle = dark;
  ctx.fillRect(64, 0, 64, 64);
  const r = rng(11);
  for (let i = 0; i < 500; i++) {
    ctx.fillStyle = r() < 0.5 ? 'rgba(255,255,230,0.05)' : 'rgba(0,25,0,0.06)';
    ctx.fillRect(r() * 128, r() * 64, 1 + r() * 2, 1 + r() * 2);
  }
  return finish(c, repeat);
}

export interface LabelLine {
  text: string;
  /** 글자 높이 (캔버스 높이에 대한 비율) */
  size: number;
  color?: string;
  weight?: number;
}

/** 글자판: 둥근 사각형 바탕에 줄 몇 개. 반환 텍스처의 가로세로비는 w:h */
export function labelTexture(lines: LabelLine[], opts: { w?: number; h?: number; bg?: string; border?: string } = {}): CanvasTexture {
  const w = opts.w ?? 512;
  const h = opts.h ?? 192;
  const [c, ctx] = canvas(w, h);
  const radius = h * 0.18;
  ctx.beginPath();
  ctx.roundRect(4, 4, w - 8, h - 8, radius);
  ctx.fillStyle = opts.bg ?? 'rgba(18, 28, 22, 0.78)';
  ctx.fill();
  if (opts.border) {
    ctx.lineWidth = 6;
    ctx.strokeStyle = opts.border;
    ctx.stroke();
  }
  const total = lines.reduce((acc, l) => acc + l.size, 0);
  let y = (h - total * h * 1.08) / 2;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  for (const l of lines) {
    const px = Math.round(l.size * h);
    ctx.font = `${l.weight ?? 700} ${px}px ${FONT}`;
    ctx.fillStyle = l.color ?? '#ffffff';
    ctx.fillText(l.text, w / 2, y + px * 0.04);
    y += px * 1.08;
  }
  return finish(c, null);
}

/** 해 원판 (가운데가 밝은 방사형) */
export function sunTexture(): CanvasTexture {
  const [c, ctx] = canvas(128, 128);
  const g = ctx.createRadialGradient(64, 64, 0, 64, 64, 64);
  g.addColorStop(0, 'rgba(255,255,245,1)');
  g.addColorStop(0.18, 'rgba(255,250,225,0.95)');
  g.addColorStop(0.4, 'rgba(255,240,200,0.25)');
  g.addColorStop(1, 'rgba(255,240,200,0)');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, 128, 128);
  return finish(c, null);
}
