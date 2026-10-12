/**
 * 부드러운 가장자리: 페어웨이·티·프린지 다각형 바깥에 폭 1.4 m 의 띠를 둘러, 깎은 잔디색이 러프로 번지게 한다 (정점 알파).
 * 칼로 자른 듯한 경계선이 게임 같아 보이는 것을 줄인다. 띠는 지면 판보다 아래에 깔려 바깥쪽에만 보인다.
 */
import { useEffect, useMemo } from 'react';

import { BufferAttribute, BufferGeometry, Color } from 'three';

import { inPolygon, type Vec2 } from '../game/course.ts';

export function edgeBandGeometry(poly: readonly Vec2[], width: number, color: Color, alphaIn = 0.92): BufferGeometry {
  const n = poly.length;
  const pos: number[] = [];
  const col: number[] = [];
  const idx: number[] = [];
  const nrm: number[] = [];
  const offsets: Vec2[] = [];
  for (let i = 0; i < n; i++) {
    const prev = poly[(i - 1 + n) % n]!;
    const cur = poly[i]!;
    const next = poly[(i + 1) % n]!;
    const unit = (a: Vec2, b: Vec2): Vec2 => {
      const dx = b[0] - a[0];
      const dz = b[1] - a[1];
      const l = Math.hypot(dx, dz) || 1;
      return [dx / l, dz / l];
    };
    const e1 = unit(prev, cur);
    const e2 = unit(cur, next);
    let n1: Vec2 = [e1[1], -e1[0]];
    let n2: Vec2 = [e2[1], -e2[0]];
    // 바깥 쪽으로: 한 걸음 가 본 점이 안이면 뒤집는다
    const mid: Vec2 = [(n1[0] + n2[0]) / 2, (n1[1] + n2[1]) / 2];
    if (inPolygon(poly, cur[0] + mid[0] * 0.3, cur[1] + mid[1] * 0.3)) {
      n1 = [-n1[0], -n1[1]];
      n2 = [-n2[0], -n2[1]];
    }
    const m: Vec2 = [n1[0] + n2[0], n1[1] + n2[1]];
    const ml = Math.hypot(m[0], m[1]) || 1;
    const dir: Vec2 = [m[0] / ml, m[1] / ml];
    const cosHalf = Math.max(0.45, dir[0] * n1[0] + dir[1] * n1[1]); // 뾰족한 모서리에서 너무 길어지지 않게
    offsets.push([(dir[0] * width) / cosHalf, (dir[1] * width) / cosHalf]);
  }
  for (let i = 0; i < n; i++) {
    const p = poly[i]!;
    const o = offsets[i]!;
    pos.push(p[0], 0, p[1], p[0] + o[0], 0, p[1] + o[1]);
    col.push(color.r, color.g, color.b, alphaIn, color.r, color.g, color.b, 0);
    nrm.push(0, 1, 0, 0, 1, 0);
  }
  for (let i = 0; i < n; i++) {
    const a = 2 * i;
    const b = 2 * ((i + 1) % n);
    idx.push(a, a + 1, b, b, a + 1, b + 1);
  }
  const g = new BufferGeometry();
  g.setAttribute('position', new BufferAttribute(new Float32Array(pos), 3));
  g.setAttribute('normal', new BufferAttribute(new Float32Array(nrm), 3));
  g.setAttribute('color', new BufferAttribute(new Float32Array(col), 4));
  g.setIndex(idx);
  return g;
}

const BAND = new Color('#6b9a3a');

export function SoftEdge({ poly, y, width = 1.4, order = 0 }: { poly: readonly Vec2[]; y: number; width?: number; order?: number }) {
  const geometry = useMemo(() => edgeBandGeometry(poly, width, BAND), [poly, width]);
  useEffect(() => () => geometry.dispose(), [geometry]);
  return (
    <mesh geometry={geometry} position={[0, y, 0]} receiveShadow renderOrder={order}>
      <meshStandardMaterial vertexColors transparent depthWrite={false} roughness={0.95} polygonOffset polygonOffsetFactor={-0.6 - order * 0.1} polygonOffsetUnits={-1} />
    </mesh>
  );
}
