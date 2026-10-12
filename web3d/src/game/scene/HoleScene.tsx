/**
 * 홀 그리기: 러프(넓은 땅), 페어웨이, 티, 프린지·그린·홀컵·깃발, 벙커, 물, OB 말뚝, 경계 밖 숲.
 * 땅 위에 겹친 판은 1 cm 안쪽으로만 띄우고 polygonOffset 으로 앞뒤를 정한다(공이 떠 보이지 않게).
 */
import { useEffect, useLayoutEffect, useMemo, useRef } from 'react';

import { CircleGeometry, Color, DoubleSide, Matrix4, Object3D, PlaneGeometry, Quaternion, Shape, ShapeGeometry, Vector3, type InstancedMesh } from 'three';

import { COLORS } from '../../scene/layout.ts';
import { grassTexture, stripeTexture } from '../../scene/textures.ts';
import { M_PER_YD } from '../../engine/units.ts';
import { Label } from '../../scene/Label.tsx';
import { CUP_R, ellipsePoints, fairwayOutline, FRINGE_M, inPolygon, TEE_BOX, yawTo, type Hole, type Vec2 } from '../course.ts';
import { holeOf, useGame } from '../round.ts';

/** 판 높이 (m): 공은 이 위 1 cm 에 놓인다 */
export const SURFACE_Y = { water: 0.0015, fairway: 0.002, tee: 0.003, land: 0.0035, fringe: 0.004, green: 0.005, bunker: 0.006, cup: 0.0055 } as const;
export const BALL_LIFT = 0.008;

const FLAG_COLORS = ['#e5484d', '#f5c518', '#3e8ef7'];

/** 월드 (x, z) 점들 → three Shape (모양 좌표는 (x, −z): rotateX(−π/2) 뒤 다시 (x, 0, z) 가 된다) */
function shapeOf(points: readonly Vec2[]): Shape {
  const s = new Shape();
  points.forEach(([x, z], i) => (i === 0 ? s.moveTo(x, -z) : s.lineTo(x, -z)));
  s.closePath();
  return s;
}

function flatShape(points: readonly Vec2[]): ShapeGeometry {
  return new ShapeGeometry(shapeOf(points), 1).rotateX(-Math.PI / 2);
}

function useDispose(...items: { dispose(): void }[]) {
  useEffect(() => () => items.forEach((i) => i.dispose()), items); // eslint-disable-line react-hooks/exhaustive-deps
}

function center(h: Hole): [number, number] {
  const xs = h.bounds.map((p) => p[0]);
  const zs = h.bounds.map((p) => p[1]);
  return [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...zs) + Math.max(...zs)) / 2];
}

function Rough({ hole }: { hole: Hole }) {
  const texture = useMemo(() => grassTexture(COLORS.rough, [600, 600]), []);
  const geometry = useMemo(() => new PlaneGeometry(3000, 3000).rotateX(-Math.PI / 2), []);
  useDispose(texture, geometry);
  const [cx, cz] = center(hole);
  return (
    <mesh geometry={geometry} position={[cx, 0, cz]} receiveShadow>
      <meshStandardMaterial map={texture} roughness={1} metalness={0} />
    </mesh>
  );
}

function Fairways({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => hole.fairways.map((f) => flatShape(fairwayOutline(f))), [hole]);
  // ShapeGeometry 의 uv = 모양 좌표 (m). 줄무늬 한 쌍 = 24 m
  const texture = useMemo(() => stripeTexture(COLORS.fairwayLight, COLORS.fairwayDark, [1 / 24, 1 / 24]), []);
  useEffect(() => () => geometries.forEach((g) => g.dispose()), [geometries]);
  useDispose(texture);
  return (
    <group>
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} position={[0, SURFACE_Y.fairway, 0]} receiveShadow>
          <meshStandardMaterial map={texture} roughness={0.95} polygonOffset polygonOffsetFactor={-1} polygonOffsetUnits={-2} />
        </mesh>
      ))}
    </group>
  );
}

function TeeBox({ hole }: { hole: Hole }) {
  const yaw = yawTo(hole.tee, hole.teeTarget);
  const geometry = useMemo(() => new PlaneGeometry(TEE_BOX[0] * 2, TEE_BOX[1] * 2).rotateX(-Math.PI / 2), []);
  useDispose(geometry);
  return (
    <group position={[hole.tee[0], 0, hole.tee[1]]} rotation={[0, -yaw, 0]}>
      <mesh geometry={geometry} position={[-1.5, SURFACE_Y.tee, 0]} receiveShadow>
        <meshStandardMaterial color={COLORS.tee} roughness={0.95} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-3} />
      </mesh>
      {[-3.2, 3.2].map((z) => (
        <mesh key={z} position={[1.2, 0.09, z]} castShadow>
          <sphereGeometry args={[0.09, 16, 12]} />
          <meshStandardMaterial color="#2f6fd6" roughness={0.4} />
        </mesh>
      ))}
    </group>
  );
}

function GreenArea({ hole }: { hole: Hole }) {
  const geo = useMemo(
    () => ({
      fringe: flatShape(ellipsePoints(hole.green, FRINGE_M, 64)),
      green: flatShape(ellipsePoints(hole.green, 0, 64)),
      cup: new CircleGeometry(CUP_R, 24).rotateX(-Math.PI / 2),
    }),
    [hole],
  );
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);
  const flag = FLAG_COLORS[(hole.number - 1) % FLAG_COLORS.length]!;
  return (
    <group>
      <mesh geometry={geo.fringe} position={[0, SURFACE_Y.fringe, 0]} receiveShadow>
        <meshStandardMaterial color={COLORS.fringe} roughness={0.95} polygonOffset polygonOffsetFactor={-3} polygonOffsetUnits={-4} />
      </mesh>
      <mesh geometry={geo.green} position={[0, SURFACE_Y.green, 0]} receiveShadow>
        <meshStandardMaterial color={COLORS.green} roughness={0.85} polygonOffset polygonOffsetFactor={-4} polygonOffsetUnits={-5} />
      </mesh>
      <group position={[hole.pin[0], 0, hole.pin[1]]}>
        <mesh geometry={geo.cup} position={[0, SURFACE_Y.cup, 0]}>
          <meshBasicMaterial color="#0d140c" polygonOffset polygonOffsetFactor={-6} polygonOffsetUnits={-8} />
        </mesh>
        <mesh position={[0, 1.2, 0]} castShadow>
          <cylinderGeometry args={[0.012, 0.012, 2.4, 8]} />
          <meshStandardMaterial color="#f2f2ea" roughness={0.5} />
        </mesh>
        <mesh position={[0.3, 2.15, 0]} castShadow>
          <planeGeometry args={[0.6, 0.4]} />
          <meshStandardMaterial color={flag} roughness={0.7} side={DoubleSide} />
        </mesh>
      </group>
    </group>
  );
}

function Bunkers({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => hole.bunkers.map((b) => flatShape(ellipsePoints(b, 0, 40))), [hole]);
  const lips = useMemo(() => hole.bunkers.map((b) => flatShape(ellipsePoints(b, 0.8, 40))), [hole]);
  const sand = useMemo(() => grassTexture('#e2cf9b', [1 / 6, 1 / 6], 23), []);
  useEffect(() => () => [...geometries, ...lips].forEach((g) => g.dispose()), [geometries, lips]);
  useDispose(sand);
  return (
    <group>
      {lips.map((g, i) => (
        <mesh key={`l${i}`} geometry={g} position={[0, SURFACE_Y.bunker - 0.0005, 0]} receiveShadow>
          <meshStandardMaterial color="#6f9a47" roughness={1} polygonOffset polygonOffsetFactor={-5} polygonOffsetUnits={-6} />
        </mesh>
      ))}
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} position={[0, SURFACE_Y.bunker, 0]} receiveShadow>
          <meshStandardMaterial map={sand} roughness={1} polygonOffset polygonOffsetFactor={-6} polygonOffsetUnits={-7} />
        </mesh>
      ))}
    </group>
  );
}

function Water({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => hole.water.map((w) => flatShape(w)), [hole]);
  useEffect(() => () => geometries.forEach((g) => g.dispose()), [geometries]);
  return (
    <group>
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} position={[0, SURFACE_Y.water, 0]}>
          <meshStandardMaterial color="#3a86c8" roughness={0.12} metalness={0.15} emissive="#0b3b66" emissiveIntensity={0.25} polygonOffset polygonOffsetFactor={-0.5} polygonOffsetUnits={-1} />
        </mesh>
      ))}
    </group>
  );
}

/** 경계 바깥쪽 법선 (점이 경계 밖으로 나가는 쪽) */
function outward(h: Hole, a: Vec2, b: Vec2): Vec2 {
  const dx = b[0] - a[0];
  const dz = b[1] - a[1];
  const l = Math.hypot(dx, dz) || 1;
  const n: Vec2 = [dz / l, -dx / l];
  const mx = (a[0] + b[0]) / 2 + n[0] * 3;
  const mz = (a[1] + b[1]) / 2 + n[1] * 3;
  return inPolygon(h.bounds, mx, mz) ? [-n[0], -n[1]] : n;
}

/** OB 흰 말뚝 (경계를 따라 14 m 마다) */
function OBStakes({ hole }: { hole: Hole }) {
  const ref = useRef<InstancedMesh>(null);
  const spots = useMemo(() => {
    const out: Vec2[] = [];
    const b = hole.bounds;
    for (let i = 0; i < b.length; i++) {
      const a = b[i]!;
      const c = b[(i + 1) % b.length]!;
      const len = Math.hypot(c[0] - a[0], c[1] - a[1]);
      for (let s = 0; s < len; s += 14) out.push([a[0] + ((c[0] - a[0]) * s) / len, a[1] + ((c[1] - a[1]) * s) / len]);
    }
    return out;
  }, [hole]);
  useLayoutEffect(() => {
    const m = ref.current;
    if (!m) return;
    const o = new Object3D();
    spots.forEach(([x, z], i) => {
      o.position.set(x, 0.5, z);
      o.updateMatrix();
      m.setMatrixAt(i, o.matrix);
    });
    m.instanceMatrix.needsUpdate = true;
    m.computeBoundingSphere();
  }, [spots]);
  return (
    <instancedMesh ref={ref} args={[undefined, undefined, spots.length]} castShadow>
      <boxGeometry args={[0.1, 1, 0.1]} />
      <meshStandardMaterial color="#f4f4ee" roughness={0.6} />
    </instancedMesh>
  );
}

/** 경계 밖 숲: 경계를 따라 바깥쪽 4–34 m 에 원뿔 나무 */
function Trees({ hole }: { hole: Hole }) {
  const crowns = useRef<InstancedMesh>(null);
  const trunks = useRef<InstancedMesh>(null);
  const spots = useMemo(() => {
    const out: { x: number; z: number; h: number; r: number; tint: number }[] = [];
    let s = 977 * hole.number + 13;
    const rnd = () => ((s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
    const b = hole.bounds;
    for (let i = 0; i < b.length; i++) {
      const a = b[i]!;
      const c = b[(i + 1) % b.length]!;
      const n = outward(hole, a, c);
      const len = Math.hypot(c[0] - a[0], c[1] - a[1]);
      for (let t = 0; t < len; t += 6.5) {
        for (let k = 0; k < 2; k++) {
          const d = 4 + rnd() * 30;
          const x = a[0] + ((c[0] - a[0]) * t) / len + n[0] * d + (rnd() - 0.5) * 4;
          const z = a[1] + ((c[1] - a[1]) * t) / len + n[1] * d + (rnd() - 0.5) * 4;
          if (inPolygon(hole.bounds, x, z)) continue;
          out.push({ x, z, h: 8 + rnd() * 9, r: 2.3 + rnd() * 2.2, tint: rnd() });
        }
      }
    }
    return out;
  }, [hole]);
  useLayoutEffect(() => {
    const c = crowns.current;
    const t = trunks.current;
    if (!c || !t) return;
    const m = new Matrix4();
    const q = new Quaternion();
    const o = new Object3D();
    const dark = new Color('#2f5a2a');
    const light = new Color('#4d7d34');
    const col = new Color();
    spots.forEach((p, i) => {
      m.compose(new Vector3(p.x, 1.6 + p.h / 2, p.z), q, new Vector3(p.r, p.h, p.r));
      c.setMatrixAt(i, m);
      c.setColorAt(i, col.copy(dark).lerp(light, p.tint));
      o.position.set(p.x, 1, p.z);
      o.scale.set(0.35, 2, 0.35);
      o.updateMatrix();
      t.setMatrixAt(i, o.matrix);
    });
    c.instanceMatrix.needsUpdate = true;
    if (c.instanceColor) c.instanceColor.needsUpdate = true;
    t.instanceMatrix.needsUpdate = true;
    c.computeBoundingSphere();
    t.computeBoundingSphere();
  }, [spots]);
  return (
    <group>
      <instancedMesh key={`c${hole.number}`} ref={crowns} args={[undefined, undefined, spots.length]} castShadow>
        <coneGeometry args={[1, 1, 8]} />
        <meshStandardMaterial roughness={0.9} />
      </instancedMesh>
      <instancedMesh key={`t${hole.number}`} ref={trunks} args={[undefined, undefined, spots.length]} castShadow>
        <cylinderGeometry args={[0.5, 0.6, 1, 6]} />
        <meshStandardMaterial color="#6b4a2f" roughness={0.9} />
      </instancedMesh>
    </group>
  );
}

/** 물 위의 땅 (섬 그린 둘레) */
function Land({ hole }: { hole: Hole }) {
  const geometries = useMemo(() => (hole.land ?? []).map((l) => flatShape(l)), [hole]);
  const texture = useMemo(() => grassTexture('#6f9d45', [1 / 8, 1 / 8], 31), []);
  useEffect(() => () => geometries.forEach((g) => g.dispose()), [geometries]);
  useDispose(texture);
  return (
    <group>
      {geometries.map((g, i) => (
        <mesh key={i} geometry={g} position={[0, SURFACE_Y.land, 0]} receiveShadow>
          <meshStandardMaterial map={texture} roughness={1} polygonOffset polygonOffsetFactor={-2.5} polygonOffsetUnits={-3.5} />
        </mesh>
      ))}
    </group>
  );
}

/** 롱드라이브: 150–375 yd, 25 yd 마다 가로선과 글자판 */
function DriveMarkers({ hole }: { hole: Hole }) {
  const line = useMemo(() => new PlaneGeometry(0.35, 90).rotateX(-Math.PI / 2), []);
  useDispose(line);
  if (hole.kind !== 'longdrive') return null;
  const yds = [150, 175, 200, 225, 250, 275, 300, 325, 350, 375];
  return (
    <group>
      {yds.map((yd) => {
        const x = yd * M_PER_YD;
        const major = yd % 50 === 0;
        return (
          <group key={yd}>
            <mesh geometry={line} position={[x, 0.008, 0]}>
              <meshBasicMaterial color={major ? '#ffffff' : '#e8f3dc'} transparent opacity={major ? 0.85 : 0.5} polygonOffset polygonOffsetFactor={-6} polygonOffsetUnits={-7} />
            </mesh>
            {major &&
              [-1, 1].map((side) => (
                <Label key={side} lines={[{ text: `${yd}`, size: 0.6, color: '#1d2a22', weight: 800 }]} position={[x, 2.4, side * 48]} height={3.4} w={300} h={200} bg="#f5f5f0" border="rgba(0,0,0,0.2)" />
              ))}
          </group>
        );
      })}
    </group>
  );
}

export function HoleScene() {
  const hole = useGame(holeOf);
  return (
    <group name={`golf-hole-${hole.number}`} key={hole.number}>
      <Rough hole={hole} />
      <Land hole={hole} />
      <DriveMarkers hole={hole} />
      <Fairways hole={hole} />
      <TeeBox hole={hole} />
      <GreenArea hole={hole} />
      <Bunkers hole={hole} />
      <Water hole={hole} />
      <OBStakes hole={hole} />
      <Trees hole={hole} />
    </group>
  );
}
