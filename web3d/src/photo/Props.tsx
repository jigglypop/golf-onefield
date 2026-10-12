/**
 * 기물: 핀 깃발(gaesup-world `FlagMesh`, 바람에 나부끼는 천), 티 안내판, 거리 말뚝(100·150·200 yd), 벙커 갈퀴, 벤치,
 * 바위(호숫가·숲 가장자리), 갈대(호숫가). 위치는 홀 데이터와 지형 높이장에서 정한다. 공 물리에는 관여하지 않는다.
 */
import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';

import { useFrame, useThree } from '@react-three/fiber';
import { positionLocal, sin, uniform, uv, vec3 } from 'three/tsl';
import { BufferAttribute, BufferGeometry, CanvasTexture, Color, DoubleSide, IcosahedronGeometry, Matrix4, PlaneGeometry, Quaternion, SRGBColorSpace, Texture, TextureLoader, Vector2, Vector3, type InstancedMesh, type Material } from 'three';
import { MeshStandardNodeMaterial } from 'three/webgpu';

import { TEE_BOX, yawTo, type Hole, type Vec2 } from '../game/course.ts';
import { holeOf, useGame } from '../game/round.ts';
import { fbm, rngOf } from './noise.ts';
import { canvas, packedMaps, tiled } from './surfaces.ts';
import { fieldOf } from './Terrain.tsx';
import { WATER_Y } from './terrain.ts';

const FLAG_COLORS = ['#d93a2f', '#f3c21a', '#2f7be0'];

function flagTexture(color: string, n: number): string {
  const [c, ctx] = canvas(256, 160);
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, 256, 160);
  ctx.fillStyle = 'rgba(255,255,255,0.92)';
  ctx.font = '800 96px system-ui, sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(String(n), 118, 86);
  ctx.fillStyle = 'rgba(0,0,0,0.18)';
  ctx.fillRect(0, 150, 256, 10);
  return c.toDataURL('image/png');
}

/**
 * 핀 깃발: 막대는 GreenArea 가 세우고, 천만 바람 방향으로 뻗어 나부끼게 한다.
 * WebGPU 에서는 정점 셰이더(TSL)로 천 물결(깃대 쪽은 고정, 끝으로 갈수록 크게)을, WebGL 에서는 같은 판을 가만히 둔다.
 * gaesup-world 의 깃발 기물(FlagMesh)은 공개 export 가 아니고 막대·조명이 정해져 있어서, 같은 물결 식을 표준 재질(조명 받음)에 옮겼다.
 */
export function PinFlag({ hole }: { hole: Hole }) {
  const wind = useGame((s) => s.wind);
  const isGpu = useThree((s) => (s.gl as unknown as { isWebGPURenderer?: boolean }).isWebGPURenderer === true);
  const geometry = useMemo(() => new PlaneGeometry(0.62, 0.4, 18, 6).translate(0.31, 0, 0), []);
  const texture = useMemo<Texture>(() => {
    const t = new TextureLoader().load(flagTexture(FLAG_COLORS[(hole.number - 1) % FLAG_COLORS.length]!, hole.number));
    t.colorSpace = SRGBColorSpace;
    return t;
  }, [hole.number]);
  const cloth = useMemo(() => {
    const time = uniform(0);
    const gust = uniform(1);
    if (!isGpu) return { material: null as Material | null, time, gust };
    const m = new MeshStandardNodeMaterial({ map: texture, side: DoubleSide, roughness: 0.85, metalness: 0 });
    const u = uv();
    const fixed = u.x.mul(u.x);
    const wave = sin(u.x.mul(5).add(time)).mul(0.1).add(sin(u.x.mul(9).add(time.mul(1.7)).add(1.3)).mul(0.05)).add(sin(u.x.mul(15).add(time.mul(2.9)).add(2.7)).mul(0.02));
    const p = positionLocal;
    m.positionNode = vec3(p.x, p.y.sub(fixed.mul(0.03).mul(gust)), p.z.add(wave.mul(fixed).mul(gust)));
    return { material: m as Material, time, gust };
  }, [isGpu, texture]);
  useEffect(
    () => () => {
      geometry.dispose();
      texture.dispose();
      cloth.material?.dispose();
    },
    [geometry, texture, cloth],
  );
  useFrame((state) => {
    cloth.time.value = state.clock.elapsedTime * 5;
    cloth.gust.value = 0.5 + wind.speed * 0.45;
  });
  return (
    <mesh geometry={geometry} position={[hole.pin[0] + 0.012, 2.18, hole.pin[1]]} rotation={[0, -wind.dir, 0]} castShadow {...(cloth.material ? { material: cloth.material } : {})}>
      {!cloth.material && <meshStandardMaterial map={texture} side={DoubleSide} roughness={0.85} />}
    </mesh>
  );
}

function signTexture(hole: Hole, yd: number): CanvasTexture {
  const [c, ctx] = canvas(512, 320);
  ctx.fillStyle = '#2b3a2f';
  ctx.fillRect(0, 0, 512, 320);
  ctx.strokeStyle = '#d9c9a0';
  ctx.lineWidth = 10;
  ctx.strokeRect(14, 14, 484, 292);
  ctx.fillStyle = '#f2e7c4';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'alphabetic';
  ctx.font = '800 150px system-ui, sans-serif';
  ctx.fillText(String(hole.number), 150, 190);
  ctx.font = '700 46px "Malgun Gothic", "Apple SD Gothic Neo", sans-serif';
  ctx.fillText(`PAR ${hole.par}`, 360, 120);
  ctx.fillText(`${yd} yd`, 360, 180);
  ctx.font = '600 38px "Malgun Gothic", "Apple SD Gothic Neo", sans-serif';
  ctx.fillText(hole.name, 256, 262);
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.anisotropy = 8;
  t.needsUpdate = true;
  return t;
}

/** 티 앞 왼쪽의 안내판 (홀 번호·파·거리·이름) */
function TeeSign({ hole }: { hole: Hole }) {
  const yaw = yawTo(hole.tee, hole.teeTarget);
  const yd = Math.round(Math.hypot(hole.pin[0] - hole.tee[0], hole.pin[1] - hole.tee[1]) / 0.9144);
  const tex = useMemo(() => signTexture(hole, yd), [hole, yd]);
  const wood = useMemo(() => tiled(packedMaps('barkbrown', 1), 1, 1), []);
  useEffect(() => () => tex.dispose(), [tex]);
  // 티 박스 앞 왼쪽 (조준 틀: x 앞, z 오른쪽)
  const ax = hole.tee[0] + Math.cos(yaw) * 9 - Math.sin(yaw) * -(TEE_BOX[1] + 3.2);
  const az = hole.tee[1] + Math.sin(yaw) * 9 + Math.cos(yaw) * -(TEE_BOX[1] + 3.2);
  return (
    <group position={[ax, 0, az]} rotation={[0, -yaw + Math.PI + 0.12, 0]}>
      {[-0.7, 0.7].map((x) => (
        <mesh key={x} position={[x, 0.6, 0]} castShadow>
          <boxGeometry args={[0.1, 1.2, 0.1]} />
          <meshStandardMaterial map={wood.map} normalMap={wood.normalMap} color="#8a7058" roughness={0.9} />
        </mesh>
      ))}
      <mesh position={[0, 1.0, 0.07]} castShadow>
        <boxGeometry args={[1.7, 1.05, 0.08]} />
        <meshStandardMaterial map={tex} roughness={0.7} />
      </mesh>
    </group>
  );
}

/** 거리 말뚝: 티에서 100 (빨강)·150 (흰색)·200 (파랑) yd, 페어웨이 가운데 선을 따라 왼쪽 가장자리 바깥 */
function YardageStakes({ hole }: { hole: Hole }) {
  const spots = useMemo(() => {
    const out: { x: number; z: number; yaw: number; color: string }[] = [];
    const f = hole.fairways[0];
    if (!f || f.path.length < 2) return out;
    const pts: Vec2[] = [hole.tee, ...f.path];
    const targets: [number, string][] = [[91.44, '#d93a2f'], [137.16, '#f4f4ee'], [182.88, '#2f7be0']];
    let acc = 0;
    let ti = 0;
    for (let i = 1; i < pts.length && ti < targets.length; i++) {
      const a = pts[i - 1]!;
      const b = pts[i]!;
      const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
      while (ti < targets.length && acc + len >= targets[ti]![0]) {
        const t = (targets[ti]![0] - acc) / len;
        const dx = (b[0] - a[0]) / len;
        const dz = (b[1] - a[1]) / len;
        const off = f.width / 2 + 1.4; // 왼쪽(−z 쪽 법선)
        out.push({ x: a[0] + (b[0] - a[0]) * t + dz * off, z: a[1] + (b[1] - a[1]) * t - dx * off, yaw: Math.atan2(dz, dx), color: targets[ti]![1] });
        ti++;
      }
      acc += len;
    }
    return out;
  }, [hole]);
  return (
    <group>
      {spots.map((s, i) => (
        <group key={i} position={[s.x, 0, s.z]} rotation={[0, -s.yaw, 0]}>
          <mesh position={[0, 0.45, 0]} castShadow>
            <cylinderGeometry args={[0.035, 0.04, 0.9, 10]} />
            <meshStandardMaterial color="#e9e6dc" roughness={0.6} />
          </mesh>
          <mesh position={[0, 0.93, 0]} castShadow>
            <cylinderGeometry args={[0.05, 0.05, 0.12, 12]} />
            <meshStandardMaterial color={s.color} roughness={0.5} />
          </mesh>
        </group>
      ))}
    </group>
  );
}

/** 벙커 갈퀴: 벙커 가장자리에 하나씩 */
function Rakes({ hole }: { hole: Hole }) {
  const rakes = useMemo(() => {
    const r = rngOf(4000 + hole.number);
    return hole.bunkers.map((b) => {
      const a = r() * Math.PI * 2;
      const rr = Math.max(b.rx, b.rz) + 0.9;
      return { x: b.c[0] + Math.cos(a) * rr, z: b.c[1] + Math.sin(a) * rr, yaw: r() * Math.PI * 2 };
    });
  }, [hole]);
  return (
    <group>
      {rakes.map((k, i) => (
        <group key={i} position={[k.x, 0, k.z]} rotation={[0, k.yaw, 0]}>
          <mesh position={[0, 0.03, 0]} rotation={[0, 0, Math.PI / 2]} castShadow>
            <cylinderGeometry args={[0.014, 0.014, 1.3, 6]} />
            <meshStandardMaterial color="#7d5a38" roughness={0.9} />
          </mesh>
          <mesh position={[0.68, 0.03, 0]} castShadow>
            <boxGeometry args={[0.07, 0.04, 0.42]} />
            <meshStandardMaterial color="#3a3f3c" roughness={0.6} metalness={0.4} />
          </mesh>
        </group>
      ))}
    </group>
  );
}

/** 티 박스 옆 벤치 */
function Bench({ hole }: { hole: Hole }) {
  const yaw = yawTo(hole.tee, hole.teeTarget);
  const wood = useMemo(() => tiled(packedMaps('barkbrown', 1), 1, 1), []);
  const ax = hole.tee[0] - Math.cos(yaw) * 0.5 - Math.sin(yaw) * -(TEE_BOX[1] + 2.2);
  const az = hole.tee[1] - Math.sin(yaw) * 0.5 + Math.cos(yaw) * -(TEE_BOX[1] + 2.2);
  return (
    <group position={[ax, 0, az]} rotation={[0, -yaw - Math.PI / 2, 0]}>
      {[-0.6, 0.6].map((x) => (
        <mesh key={x} position={[x, 0.22, 0]} castShadow>
          <boxGeometry args={[0.07, 0.44, 0.38]} />
          <meshStandardMaterial color="#3d4540" roughness={0.6} metalness={0.3} />
        </mesh>
      ))}
      <mesh position={[0, 0.46, 0]} castShadow>
        <boxGeometry args={[1.5, 0.05, 0.4]} />
        <meshStandardMaterial map={wood.map} normalMap={wood.normalMap} color="#9a8068" roughness={0.9} />
      </mesh>
      <mesh position={[0, 0.74, -0.18]} rotation={[-0.12, 0, 0]} castShadow>
        <boxGeometry args={[1.5, 0.3, 0.04]} />
        <meshStandardMaterial map={wood.map} normalMap={wood.normalMap} color="#9a8068" roughness={0.9} />
      </mesh>
    </group>
  );
}

// ------------------------------------------------------------------ 바위

function rockGeometry(seed: number): BufferGeometry {
  const g = new IcosahedronGeometry(1, 3);
  const p = g.getAttribute('position');
  const v = new Vector3();
  for (let i = 0; i < p.count; i++) {
    v.fromBufferAttribute(p, i);
    const n = fbm(v.x * 1.6 + seed, v.z * 1.6 + v.y * 1.3 - seed, 3, 61 + seed);
    const r = 0.72 + n * 0.55;
    v.multiplyScalar(r);
    v.y = Math.max(v.y, -0.35) * 0.78; // 바닥은 납작하게
    p.setXYZ(i, v.x, v.y, v.z);
  }
  g.computeVertexNormals();
  return g;
}

interface Spot3 {
  x: number;
  y: number;
  z: number;
  s: number;
  yaw: number;
  tint: number;
}

function Rocks({ hole }: { hole: Hole }) {
  const ref = useRef<InstancedMesh>(null);
  const field = useMemo(() => fieldOf(hole), [hole]);
  const geo = useMemo(() => rockGeometry(3), []);
  const maps = useMemo(() => tiled(packedMaps('rock', 1), 2, 2), []);
  const spots = useMemo(() => {
    const r = rngOf(7100 + hole.number);
    const out: Spot3[] = [];
    // 호숫가: 둘레를 따라 듬성듬성
    for (const lake of hole.water) {
      for (let i = 0; i < lake.length; i++) {
        const a = lake[i]!;
        const b = lake[(i + 1) % lake.length]!;
        const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
        for (let t = 0; t < len; t += 7 + r() * 9) {
          const u = t / len;
          const nx = (b[1] - a[1]) / len;
          const nz = -(b[0] - a[0]) / len;
          const side = r() < 0.5 ? 1 : -1;
          const x = a[0] + (b[0] - a[0]) * u + nx * side * (0.4 + r() * 1.8);
          const z = a[1] + (b[1] - a[1]) * u + nz * side * (0.4 + r() * 1.8);
          out.push({ x, y: Math.max(field.heightAt(x, z), WATER_Y - 0.25), z, s: 0.25 + r() * r() * 1.6, yaw: r() * 6.28, tint: r() });
        }
      }
    }
    // 숲 가장자리와 언덕 기슭: 플레이 구역 밖 8–120 m 에 흩어 놓는다
    const poly = hole.bounds;
    for (let i = 0; i < poly.length; i++) {
      const a = poly[i]!;
      const b = poly[(i + 1) % poly.length]!;
      const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
      for (let t = 0; t < len; t += 22 + r() * 30) {
        const u = t / len;
        const x = a[0] + (b[0] - a[0]) * u + (r() - 0.5) * 120;
        const z = a[1] + (b[1] - a[1]) * u + (r() - 0.5) * 120;
        const d = field.distOutside(x, z);
        if (d < 8 || d > 130) continue;
        out.push({ x, y: field.heightAt(x, z), z, s: 0.6 + r() * r() * 3.2, yaw: r() * 6.28, tint: r() });
      }
    }
    return out;
  }, [hole, field]);
  useLayoutEffect(() => {
    const m = ref.current;
    if (!m) return;
    const mat = new Matrix4();
    const q = new Quaternion();
    const axis = new Vector3(0, 1, 0);
    const c = new Color();
    const lo = new Color('#9a948a');
    const hi = new Color('#e0d8c8');
    spots.forEach((s, i) => {
      q.setFromAxisAngle(axis, s.yaw);
      mat.compose(new Vector3(s.x, s.y + s.s * 0.12, s.z), q, new Vector3(s.s * (1 + s.tint * 0.5), s.s * (0.7 + s.tint * 0.35), s.s));
      m.setMatrixAt(i, mat);
      m.setColorAt(i, c.copy(lo).lerp(hi, s.tint));
    });
    m.instanceMatrix.needsUpdate = true;
    if (m.instanceColor) m.instanceColor.needsUpdate = true;
    m.count = spots.length;
    m.computeBoundingSphere();
  }, [spots]);
  useEffect(() => () => geo.dispose(), [geo]);
  return (
    <instancedMesh ref={ref} args={[geo, undefined, Math.max(1, spots.length)]} castShadow receiveShadow frustumCulled={false}>
      <meshStandardMaterial map={maps.map} normalMap={maps.normalMap} roughnessMap={maps.roughnessMap} normalScale={new Vector2(1.1, 1.1)} roughness={1} />
    </instancedMesh>
  );
}

// ------------------------------------------------------------------ 갈대

let reedTex: CanvasTexture | null = null;
function reedTexture(): CanvasTexture {
  if (reedTex) return reedTex;
  const [c, ctx] = canvas(256, 512);
  const r = rngOf(31);
  ctx.clearRect(0, 0, 256, 512);
  for (let i = 0; i < 70; i++) {
    const x = 20 + r() * 216;
    const h = 260 + r() * 240;
    const lean = (r() - 0.5) * 70;
    const g = Math.round(110 + r() * 80);
    ctx.strokeStyle = `rgb(${Math.round(g * 0.78)},${g},${Math.round(g * 0.35)})`;
    ctx.lineWidth = 2.5 + r() * 3;
    ctx.beginPath();
    ctx.moveTo(x, 512);
    ctx.quadraticCurveTo(x + lean * 0.25, 512 - h * 0.55, x + lean, 512 - h);
    ctx.stroke();
  }
  // 갈대 이삭 몇 개
  for (let i = 0; i < 9; i++) {
    const x = 30 + r() * 196;
    const y = 80 + r() * 90;
    ctx.fillStyle = `rgba(${150 + Math.round(r() * 40)},${110 + Math.round(r() * 30)},70,0.95)`;
    ctx.beginPath();
    ctx.ellipse(x, y, 5, 26, (r() - 0.5) * 0.4, 0, Math.PI * 2);
    ctx.fill();
  }
  reedTex = new CanvasTexture(c);
  reedTex.colorSpace = SRGBColorSpace;
  reedTex.anisotropy = 8;
  reedTex.needsUpdate = true;
  return reedTex;
}

function Reeds({ hole }: { hole: Hole }) {
  const ref = useRef<InstancedMesh>(null);
  const field = useMemo(() => fieldOf(hole), [hole]);
  const tex = useMemo(() => reedTexture(), []);
  const geo = useMemo(() => {
    // 십자로 교차한 두 판 (아래가 원점)
    const a = new PlaneGeometry(1.1, 1.7).translate(0, 0.85, 0);
    const b = new PlaneGeometry(1.1, 1.7).translate(0, 0.85, 0).rotateY(Math.PI / 2);
    const p = [...(a.getAttribute('position').array as Float32Array), ...(b.getAttribute('position').array as Float32Array)];
    const uv = [...(a.getAttribute('uv').array as Float32Array), ...(b.getAttribute('uv').array as Float32Array)];
    const nrm = [...(a.getAttribute('normal').array as Float32Array), ...(b.getAttribute('normal').array as Float32Array)];
    const ia = a.getIndex()!.array as Uint16Array;
    const ib = b.getIndex()!.array as Uint16Array;
    const g = new BufferGeometry();
    g.setAttribute('position', new BufferAttribute(new Float32Array(p), 3));
    g.setAttribute('uv', new BufferAttribute(new Float32Array(uv), 2));
    g.setAttribute('normal', new BufferAttribute(new Float32Array(nrm), 3));
    g.setIndex([...ia, ...Array.from(ib, (v) => v + 4)]);
    return g;
  }, []);
  const spots = useMemo(() => {
    const r = rngOf(8100 + hole.number);
    const out: Spot3[] = [];
    for (const lake of hole.water) {
      for (let i = 0; i < lake.length; i++) {
        const a = lake[i]!;
        const b = lake[(i + 1) % lake.length]!;
        const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
        const nx = (b[1] - a[1]) / len;
        const nz = -(b[0] - a[0]) / len;
        for (let t = 0; t < len; t += 0.9 + r() * 1.6) {
          // 군락: 가까운 곳 몇 개를 한 번에. 가장자리 안팎으로 퍼뜨린다
          if (fbm(a[0] / 14 + t * 0.06, a[1] / 14, 2, 77) < 0.5) continue;
          const u = t / len;
          const side = (r() - 0.62) * 2.6; // 대부분 물 쪽
          const x = a[0] + (b[0] - a[0]) * u + nx * side;
          const z = a[1] + (b[1] - a[1]) * u + nz * side;
          const h = field.heightAt(x, z);
          if (h < WATER_Y - 0.55 || h > 0.25) continue; // 너무 깊거나 마른 땅은 뺀다
          out.push({ x, y: h - 0.05, z, s: 0.7 + r() * 0.7, yaw: r() * 6.28, tint: r() });
        }
      }
    }
    return out;
  }, [hole, field]);
  useLayoutEffect(() => {
    const m = ref.current;
    if (!m) return;
    const mat = new Matrix4();
    const q = new Quaternion();
    const axis = new Vector3(0, 1, 0);
    spots.forEach((s, i) => {
      q.setFromAxisAngle(axis, s.yaw);
      mat.compose(new Vector3(s.x, s.y, s.z), q, new Vector3(s.s, s.s * (0.85 + s.tint * 0.4), s.s));
      m.setMatrixAt(i, mat);
    });
    m.instanceMatrix.needsUpdate = true;
    m.count = spots.length;
    m.computeBoundingSphere();
  }, [spots]);
  useEffect(() => () => geo.dispose(), [geo]);
  return (
    <instancedMesh ref={ref} args={[geo, undefined, Math.max(1, spots.length)]} receiveShadow frustumCulled={false}>
      <meshStandardMaterial map={tex} alphaTest={0.45} side={DoubleSide} roughness={0.9} />
    </instancedMesh>
  );
}

export function Props() {
  const hole = useGame(holeOf);
  return (
    <group name="props">
      <PinFlag hole={hole} />
      {hole.kind !== 'longdrive' && <TeeSign hole={hole} />}
      <YardageStakes hole={hole} />
      <Rakes hole={hole} />
      <Bench hole={hole} />
      <Rocks hole={hole} />
      <Reeds hole={hole} />
    </group>
  );
}
