/**
 * 숲: 지형 높이장 위에 소나무·활엽수를 거리 구간별로 심는다 (가까이 촘촘·자세히, 멀리 성기게·단순하게).
 * 배치는 `forestPlan.ts`(나무 위치·키·색), 칸별 InstancedMesh·LOD 는 `forestMeshes.ts`. 여기는 두 가지를 홀에 붙이는 얇은 층이다.
 *
 * 예전에는 구간마다 InstancedMesh 둘(소나무·활엽수)을 frustumCulled = false 로 360° 전부 그렸다 (한 홀에 나무 2 만 그루,
 * 삼각형 약 1000 만). 지금은 100–900 m 칸마다 메시를 나눠 시야 밖 칸은 그리지 않고, 그림자 단계에서도 그 단계의 시야 밖 칸은 건너뛴다.
 * 심는 위치·밀도는 지형의 `forest()` 와 같은 함수라서 땅 색(숲 바닥)과 나무가 어긋나지 않는다.
 * 밀도는 품질 프로필(`quality.ts`: ?quality=low|medium, 모바일·XR)이 정한다.
 */
import { useEffect, useMemo } from 'react';

import { useFrame } from '@react-three/fiber';

import type { Hole } from '../game/course.ts';
import { ForestMeshes } from './forestMeshes.ts';
import { planForest } from './forestPlan.ts';
import { currentProfile } from './quality.ts';
import { fieldOf } from './Terrain.tsx';
import { ForestLegacy } from './ForestLegacy.tsx'; // TEMP

export function Forest({ hole }: { hole: Hole }) {
  if (typeof location !== 'undefined' && new URLSearchParams(location.search).get('forest') === 'legacy') return <ForestLegacy hole={hole} />; // TEMP
  return <ForestNew hole={hole} />;
}

function ForestNew({ hole }: { hole: Hole }) {
  const field = useMemo(() => fieldOf(hole), [hole]);
  const density = currentProfile().forestDensity;
  const forest = useMemo(() => {
    performance.mark('forest-start');
    const plans = planForest(field, hole.number, density);
    const out = new ForestMeshes(plans);
    performance.measure(`forest scatter (${plans.map((p) => p.total).join('/')}) cells ${out.stats.cells.near}/${out.stats.cells.mid}/${out.stats.cells.far}`, 'forest-start');
    return out;
  }, [field, hole.number, density]);
  useEffect(() => () => forest.dispose(), [forest]);
  useFrame(({ camera }) => forest.update(camera));
  return (
    <group name="forest">
      <primitive object={forest.root} />
    </group>
  );
}
