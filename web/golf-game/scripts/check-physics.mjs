// 복사된 wasm 코어를 Node 에서 직접 불러 앱이 쓰는 경로(fly_flat + flat.ts 해석 + 에르미트 보간 + 일괄 클럽표)를 점검한다 (브라우저 없이 연결 확인).
// 실행 전에 `npm run copy-wasm` 으로 src/physics/wasm/ 과 src/physics/wasm-simd/ 이 있어야 한다.
// flat.ts 는 앱과 같은 파일을 Node 의 타입 제거로 그대로 불러 쓴다 (wasm 도 DOM 도 import 하지 않는 순수 함수).
// 점검 항목:
//   1. 프리셋 3종: fly_flat 의 머리·경로 일관성 (끝점 = 캐리, 속도 방향)
//   2. 객체 API(fly_f64) 와 같은 걸음에서 같은 값 (옛 API 호환)
//   3. web3d/test/web_refs.csv 52샷 대비 앱 걸음 오차 (web3d 와 같은 허용 오차)
//   4. 성긴 경로의 에르미트 보간 오차 (촘촘한 기준 경로 대비) 와 직선 이음 비교
//   5. 클럽표: 일괄 계산(SIMD 빌드의 batch_simd_opts, 기본 빌드의 스칼라 f32) 대 단일 샷
//   6. 두 빌드의 SIMD 상태 (has_simd) 와 이 런타임의 SIMD 지원
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import * as base from '../src/physics/wasm/golf_core.js';
import * as simd from '../src/physics/wasm-simd/golf_core.js';
import { FLAT_HEAD, M_PER_YD, PATH_STRIDE, hermiteAt, parseStats, pathOf, segmentAt } from '../src/physics/flat.ts';

const here = path.dirname(fileURLToPath(import.meta.url));
base.initSync({ module: readFileSync(path.resolve(here, '../src/physics/wasm/golf_core_bg.wasm')) });
simd.initSync({ module: readFileSync(path.resolve(here, '../src/physics/wasm-simd/golf_core_bg.wasm')) });

let bad = 0;
const check = (name, ok, detail = '') => {
  console.log(`  [${ok ? '통과' : '실패'}] ${name}${detail ? ': ' + detail : ''}`);
  if (!ok) bad++;
};

const TAU = base.app_step();
const MIN_STEPS = base.app_min_steps();
console.log(`앱 걸음 τ ${TAU}, 최소 걸음 ${MIN_STEPS} (golf-core app_step / app_min_steps)`);
check('두 빌드의 앱 걸음이 같다', simd.app_step() === TAU && simd.app_min_steps() === MIN_STEPS);

// ---- 1. 프리셋
const presets = [
  { name: '드라이버', mph: 167, launchDeg: 10.9, spinRpm: 2686, axisDeg: 0 },
  { name: '7번 아이언', mph: 120, launchDeg: 16.3, spinRpm: 7097, axisDeg: 0 },
  { name: '웨지', mph: 86, launchDeg: 25.7, spinRpm: 8403, axisDeg: 0 },
];
console.log('\n프리셋 (fly_flat, 앱 걸음)');
for (const p of presets) {
  const a = base.fly_flat(p.mph, p.launchDeg, p.spinRpm, p.axisDeg, TAU, MIN_STEPS);
  const s = parseStats(a);
  const pa = pathOf(a);
  const n = pa.length / PATH_STRIDE;
  const last = PATH_STRIDE * (n - 1);
  const row = {
    carry_yd: s.carryYd.toFixed(2), carry_m: s.carryM.toFixed(2), apex_m: s.apexM.toFixed(2), land_deg: s.landDeg.toFixed(2),
    lateral_m: s.lateralM.toFixed(3), time_s: s.timeS.toFixed(2), steps: s.steps, points: n,
    last_xyz_m: [pa[last + 1], pa[last + 2], pa[last + 3]].map((v) => v.toFixed(2)).join(' / '),
  };
  const endOk = Math.abs(pa[last + 1] - s.carryM) < 1e-9 && Math.abs(pa[last + 2]) < 1e-9 && Math.abs(pa[last] - s.timeS) < 1e-9;
  const fwd = pa[4] > 0 && pa[5] > 0; // 출발 속도: 앞으로, 위로
  const down = pa[last + 4] > 0 && pa[last + 5] < 0; // 착지 속도: 앞으로, 아래로
  console.log(`  [${p.name}] ${p.mph} mph / ${p.launchDeg}° / ${p.spinRpm} rpm`, row);
  check(`${p.name}: 끝점 = 캐리·시간, 속도 방향`, endOk && fwd && down && Number.isFinite(s.carryYd));
}

// ---- 2. 옛 객체 API 호환
console.log('\n객체 API (fly_f64) 호환');
for (const p of presets) {
  const f = base.fly_f64(p.mph, p.launchDeg, p.spinRpm, p.axisDeg, TAU);
  const g = parseStats(base.fly_flat(p.mph, p.launchDeg, p.spinRpm, p.axisDeg, TAU, 0));
  const same = f.carry_m === g.carryM && f.apex_m === g.apexM && f.lateral_m === g.lateralM && f.land_deg === g.landDeg && f.steps === g.steps && f.roll_yd === g.rollYd;
  const legacyPath = f.path();
  const pa = pathOf(base.fly_flat(p.mph, p.launchDeg, p.spinRpm, p.axisDeg, TAU, 0));
  let pathSame = legacyPath.length / 4 === pa.length / PATH_STRIDE;
  for (let i = 0; pathSame && i < legacyPath.length / 4; i++) for (let j = 0; j < 4; j++) pathSame &&= legacyPath[4 * i + j] === Math.fround(pa[PATH_STRIDE * i + j]);
  check(`${p.name}: 같은 걸음에서 요약값·경로(f32 4칸)가 같다`, same && pathSame);
  f.free();
}

// ---- 3. web_refs.csv
console.log('\nweb3d 참값 (web3d/test/web_refs.csv, 52샷) 대비 앱 걸음');
const csv = readFileSync(path.resolve(here, '../../../web3d/test/web_refs.csv'), 'utf8').trim().split(/\r?\n/).slice(1);
const TOL = { carry: 0.05, lateral: 0.05, apex: 0.05, land: 0.05, time: 0.01 };
const max = { carry: 0, lateral: 0, apex: 0, land: 0, time: 0 };
for (const line of csv) {
  const [mph, deg, rpm, axis, carry, lateral, apex, land, time] = line.split(',').map(Number);
  const a = base.fly_flat(mph, deg, rpm, axis, TAU, MIN_STEPS);
  const s = parseStats(a);
  const pa = pathOf(a);
  const last = PATH_STRIDE * (pa.length / PATH_STRIDE - 1);
  // web3d 의 착지각 정의: 내려가는 속도 대 수평 속력 (착지 순간 경로 점의 속도로 계산)
  const landWeb = (Math.atan2(-pa[last + 5], Math.hypot(pa[last + 4], pa[last + 6])) * 180) / Math.PI;
  const err = {
    carry: Math.abs(s.carryYd - carry), lateral: Math.abs(s.lateralM / M_PER_YD - lateral), apex: Math.abs(s.apexM / M_PER_YD - apex),
    land: Math.abs(landWeb - land), time: Math.abs(s.timeS - time),
  };
  for (const k of Object.keys(max)) max[k] = Math.max(max[k], err[k]);
}
console.log(`  최대 오차: 캐리 ${max.carry.toExponential(2)} yd, 좌우 ${max.lateral.toExponential(2)} yd, 높이 ${max.apex.toExponential(2)} yd, 착지각 ${max.land.toExponential(2)}°, 시간 ${max.time.toExponential(2)} s`);
check('52샷 모두 web3d 허용 오차 안 (캐리·좌우·높이 0.05 yd, 착지각 0.05°, 시간 0.01 s)', csv.length === 52 && Object.keys(TOL).every((k) => max[k] <= TOL[k]));

// ---- 4. 에르미트 대 직선
console.log('\n성긴 경로의 에르미트 보간 (기준: τ 0.005 경로의 점)');
let worstH = 0;
let worstL = 0;
const sample = [...presets, { mph: 158, launchDeg: 9.2, spinRpm: 3655, axisDeg: 10 }, { mph: 60, launchDeg: 30, spinRpm: 6000, axisDeg: -8 }];
const p3 = new Float64Array(3);
for (const p of sample) {
  const coarse = pathOf(base.fly_flat(p.mph, p.launchDeg, p.spinRpm, p.axisDeg, TAU, MIN_STEPS));
  const fine = pathOf(base.fly_flat(p.mph, p.launchDeg, p.spinRpm, p.axisDeg, 0.005, 0));
  const nf = fine.length / PATH_STRIDE;
  const tEnd = coarse[coarse.length - PATH_STRIDE];
  let hint = 0;
  for (let i = 0; i < nf; i++) {
    const t = fine[PATH_STRIDE * i];
    if (t > tEnd) break;
    hint = segmentAt(coarse, t, hint);
    hermiteAt(coarse, hint, t, p3);
    // 직선 이음
    const a = PATH_STRIDE * hint;
    const b = a + PATH_STRIDE;
    const u = (t - coarse[a]) / (coarse[b] - coarse[a]);
    for (let k = 0; k < 3; k++) {
      const ref = fine[PATH_STRIDE * i + 1 + k];
      worstH = Math.max(worstH, Math.abs(p3[k] - ref));
      worstL = Math.max(worstL, Math.abs(coarse[a + 1 + k] + (coarse[b + 1 + k] - coarse[a + 1 + k]) * u - ref));
    }
  }
}
console.log(`  위치 오차 최대: 에르미트 ${(worstH * 1000).toFixed(2)} mm, 직선 ${(worstL * 1000).toFixed(1)} mm`);
check('에르미트 오차 < 2 cm 이고 직선의 5% 미만', worstH < 0.02 && worstH < 0.05 * worstL);

// ---- 5. 클럽표 (일괄 대 단일)
console.log('\n클럽표: 일괄 계산 대 단일 샷');
const clubs = [
  [167, 10.9, 2686], [158, 9.2, 3655], [152, 9.4, 4350], [146, 10.2, 4437], [142, 10.4, 4630], [137, 11, 4836],
  [132, 12.1, 5361], [127, 14.1, 6231], [120, 16.3, 7097], [115, 18.1, 7998], [109, 20.4, 8647], [102, 24.2, 9304],
];
const flat = new Float64Array(clubs.length * 4);
clubs.forEach(([m, v, r], i) => flat.set([m, v, r, 0], 4 * i));
for (const [label, mod] of [['기본 빌드 (스칼라 f32)', base], ['SIMD 빌드 (4레인 f32)', simd]]) {
  const out = mod.batch_simd_opts(flat, TAU, MIN_STEPS, false);
  let d = 0;
  clubs.forEach(([m, v, r], i) => {
    const single = parseStats(base.fly_flat(m, v, r, 0, TAU, MIN_STEPS)).carryYd;
    d = Math.max(d, Math.abs(out[4 * i] / M_PER_YD - single));
  });
  check(`${label}: 12클럽 캐리가 단일 샷(f64)과 1e-3 yd 안`, d < 1e-3, `최대 차이 ${d.toExponential(2)} yd`);
}
const outB = base.batch_simd_opts(flat, TAU, MIN_STEPS, false);
const outS = simd.batch_simd_opts(flat, TAU, MIN_STEPS, false);
check('SIMD 빌드와 기본 빌드의 일괄 결과가 비트까지 같다', outB.every((x, i) => Object.is(x, outS[i])));
console.log('  클럽 캐리(yd):', Array.from({ length: clubs.length }, (_, i) => (outS[4 * i] / M_PER_YD).toFixed(0)).join(' '));

// ---- 6. SIMD 상태
console.log('\nSIMD');
const SIMD_PROBE = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15, 253, 98, 11]);
check('기본 빌드는 SIMD 없음, SIMD 빌드는 있음', base.has_simd() === false && simd.has_simd() === true);
check('이 런타임이 wasm SIMD 를 이해한다 (WebAssembly.validate)', WebAssembly.validate(SIMD_PROBE) === true);
void FLAT_HEAD;

if (bad) {
  console.error(`[check-physics] 실패 ${bad}건`);
  process.exit(1);
}
console.log('\n[check-physics] wasm 코어 연결 확인 완료');
