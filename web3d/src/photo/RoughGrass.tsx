/**
 * 러프의 풀잎: gaesup-world 의 `Grass`(바람에 흔들리는 인스턴스 풀잎)를 공 둘레의 러프 칸에만 깐다.
 * 안쪽 고리는 짧고 촘촘하게(공이 놓이는 곳), 바깥 고리는 길고 성기게. 칸은 공이 새로 놓일 때마다 다시 고른다
 * (페어웨이·그린·티·벙커·물은 빼고 러프와 경계 밖만). 풀잎 높이는 지형 높이장 위에 선다.
 *
 * 마운트 키는 (홀, 고리) 뿐이다. 공이 새로 놓여도 `Grass` 를 새로 올리지 않고 `position`·`center`·`cells` 만 바꾼다:
 * 풀잎 메시·재질·블레이드 지오메트리는 그대로고 인스턴스 오프셋만 다시 깔린다 (예전에는 키에 ballId 가 있어 샷마다
 * 세 겹(최대 12 만/9 만/7 만 장)을 통째로 내리고 다시 올렸다).
 */
import { useMemo } from 'react';

import { Grass, GrassDriver } from 'gaesup-world';

import { surfaceAt, type Hole } from '../game/course.ts';
import { game, holeOf, useGame } from '../game/round.ts';
import { currentProfile } from './quality.ts';
import type { TerrainField } from './terrain.ts';
import { fieldOf } from './Terrain.tsx';

type Cell = [number, number, number];

interface Ring {
  /** r0 이상 r1 미만 (공에서 m) */
  r0: number;
  r1: number;
  cell: number;
  height: number;
  width: number;
  density: number;
  max: number;
}

const RINGS: Ring[] = [
  { r0: 0, r1: 11, cell: 1.5, height: 0.09, width: 0.045, density: 480, max: 120000 },
  { r0: 11, r1: 26, cell: 2, height: 0.15, width: 0.06, density: 150, max: 90000 },
  { r0: 26, r1: 55, cell: 3, height: 0.26, width: 0.085, density: 55, max: 70000 },
];

function cellsFor(hole: Hole, bx: number, bz: number, ring: Ring, field: TerrainField): Cell[] {
  const out: Cell[] = [];
  const n = Math.ceil(ring.r1 / ring.cell);
  const gx = Math.round(bx / ring.cell) * ring.cell;
  const gz = Math.round(bz / ring.cell) * ring.cell;
  for (let j = -n; j <= n; j++) {
    for (let i = -n; i <= n; i++) {
      const x = gx + i * ring.cell;
      const z = gz + j * ring.cell;
      const r = Math.hypot(x - bx, z - bz);
      if (r < ring.r0 || r >= ring.r1) continue;
      const s = surfaceAt(hole, x, z);
      if (s !== 'rough' && s !== 'ob') continue;
      out.push([x - bx, z - bz, field.heightAt(x, z)]); // 칸 = [x, z, y] (gaesup GrassCell 순서)
    }
  }
  return out;
}

export function RoughGrass() {
  const hole = useGame(holeOf);
  const ballId = useGame((s) => s.ballId);
  // 풀잎 밀도·최대 수 배율: 품질 프로필 (?quality=low|medium, 모바일·XR)
  const q = currentProfile().grassDensity;
  const rings = useMemo(() => {
    const field = fieldOf(hole);
    // 공이 놓인 자리 (비행 중에는 마지막으로 놓였던 자리를 유지: ballId 가 바뀔 때만 다시 고른다)
    const b = game.get().ball;
    return RINGS.map((ring) => ({ ring, cells: cellsFor(hole, b.x, b.z, ring, field), center: [b.x, 0, b.z] as [number, number, number] }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hole, ballId]);
  return (
    <group name="rough-grass">
      <GrassDriver />
      {rings.map(({ ring, cells, center }, i) =>
        cells.length === 0 ? null : (
          <Grass
            key={`${hole.number}-${i}`}
            position={[center[0], 0, center[2]]}
            cells={cells}
            cellSize={ring.cell}
            width={ring.r1 * 2}
            center={center}
            ground={false}
            toon={false}
            lod={{ near: ring.r1 * 0.62, far: ring.r1 * 1.12, strength: 2 }}
            density={ring.density * q}
            maxInstances={Math.round(ring.max * q)}
            options={{ bW: ring.width, bH: ring.height, joints: 4 }}
            bladeTipColor="#a9b95c"
            bladeBottomColor="#587a3a"
          />
        ),
      )}
    </group>
  );
}
