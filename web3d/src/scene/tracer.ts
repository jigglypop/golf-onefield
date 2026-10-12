/**
 * 궤적 선 (튜브). 굵기를 카메라 거리에 비례시켜 화면에서 거의 일정한 폭(몇 px)으로 보이게 한다.
 * three 의 기본 재질만 쓰므로 WebGPU·WebGL 모두에서 같다 (Line2 같은 ShaderMaterial 이 필요 없다).
 * 링 i 는 경로 점 i 를 둘러싼 꼭짓점 RAD 개. drawRange 로 공이 지나간 데까지만 그린다.
 */
import { BufferAttribute, BufferGeometry, DynamicDrawUsage, Vector3 } from 'three';

const RAD = 6;

export class Tracer {
  readonly geometry = new BufferGeometry();
  readonly count: number;
  private readonly pts: Float32Array;
  /** 링마다 법선 N, 종법선 B */
  private readonly frames: Float32Array;
  private readonly pos: Float32Array;
  private readonly cos = new Float32Array(RAD);
  private readonly sin = new Float32Array(RAD);

  constructor(path: Float32Array) {
    this.pts = path;
    const n = (this.count = path.length / 3);
    this.frames = new Float32Array(n * 6);
    this.pos = new Float32Array(n * RAD * 3);
    for (let j = 0; j < RAD; j++) {
      this.cos[j] = Math.cos((2 * Math.PI * j) / RAD);
      this.sin[j] = Math.sin((2 * Math.PI * j) / RAD);
    }
    // 평행 이동 틀: 접선 T, 처음 법선은 T × 위, 다음부터 앞 법선을 T 에 수직으로 투영
    const p = (i: number) => new Vector3(path[3 * i]!, path[3 * i + 1]!, path[3 * i + 2]!);
    const T = new Vector3();
    const N = new Vector3();
    const B = new Vector3();
    for (let i = 0; i < n; i++) {
      T.copy(p(Math.min(i + 1, n - 1))).sub(p(Math.max(i - 1, 0)));
      if (T.lengthSq() < 1e-12) T.set(1, 0, 0);
      T.normalize();
      if (i === 0) {
        N.crossVectors(T, new Vector3(0, 1, 0));
        if (N.lengthSq() < 1e-8) N.set(0, 0, 1);
      } else {
        N.addScaledVector(T, -N.dot(T));
      }
      N.normalize();
      B.crossVectors(T, N).normalize();
      this.frames.set([N.x, N.y, N.z, B.x, B.y, B.z], 6 * i);
    }
    const index: number[] = [];
    for (let i = 0; i < n - 1; i++) {
      for (let j = 0; j < RAD; j++) {
        const a = i * RAD + j;
        const b = (i + 1) * RAD + j;
        const c = (i + 1) * RAD + ((j + 1) % RAD);
        const d = i * RAD + ((j + 1) % RAD);
        index.push(a, b, d, b, c, d);
      }
    }
    const attr = new BufferAttribute(this.pos, 3);
    attr.setUsage(DynamicDrawUsage);
    this.geometry.setAttribute('position', attr);
    this.geometry.setIndex(index);
    this.geometry.setDrawRange(0, 0);
  }

  /**
   * 링 0..upto 를 경로 점에, 링 upto+1 을 공 위치 head 에 두고 그 앞까지 그린다.
   * 반지름 = 카메라 거리 × pxScale (rMin..rMax 로 자름).
   */
  update(camera: Vector3, upto: number, head: Vector3 | null, pxScale: number, rMin = 0.012, rMax = 3): void {
    const n = this.count;
    const last = Math.min(upto + (head ? 1 : 0), n - 1);
    if (last < 1) {
      this.geometry.setDrawRange(0, 0);
      return;
    }
    const { pts, frames, pos, cos, sin } = this;
    for (let i = 0; i <= last; i++) {
      const useHead = head !== null && i === last && i > upto;
      const cx = useHead ? head.x : pts[3 * i]!;
      const cy = useHead ? head.y : pts[3 * i + 1]!;
      const cz = useHead ? head.z : pts[3 * i + 2]!;
      const dx = cx - camera.x;
      const dy = cy - camera.y;
      const dz = cz - camera.z;
      const r = Math.min(Math.max(Math.sqrt(dx * dx + dy * dy + dz * dz) * pxScale, rMin), rMax);
      const f = 6 * i;
      for (let j = 0; j < RAD; j++) {
        const k = 3 * (i * RAD + j);
        const c = cos[j]! * r;
        const s = sin[j]! * r;
        pos[k] = cx + c * frames[f]! + s * frames[f + 3]!;
        pos[k + 1] = cy + c * frames[f + 1]! + s * frames[f + 4]!;
        pos[k + 2] = cz + c * frames[f + 2]! + s * frames[f + 5]!;
      }
    }
    const attr = this.geometry.getAttribute('position') as BufferAttribute;
    attr.clearUpdateRanges();
    attr.addUpdateRange(0, (last + 1) * RAD * 3);
    attr.needsUpdate = true;
    this.geometry.setDrawRange(0, last * RAD * 6);
  }

  dispose(): void {
    this.geometry.dispose();
  }
}
