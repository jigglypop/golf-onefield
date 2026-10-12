// 샷 하나씩 부르는 경로의 경계 비용 (Node): 객체 API(Flight) 대 한 번의 호출 API(fly_flat), JS↔wasm 호출 횟수 포함.
//
//   cd golf-core && sh build-wasm.sh && node bench/single.mjs
//
// 앱(web/golf-game/src/physics/golfCore.ts)이 하는 일과 같은 일을 잰다: 샷 하나를 날려 요약값 11개 + 경로 배열을 JS 객체로 꺼내기.
//   old        = fly_f64 + getter 11개 + path() + free()  (이전 golfCore.ts, 앱 걸음 τ 0.05)
//   old@app    = 같은 객체 API, 앱 걸음 (τ 0.137, min_steps 는 객체 API 에 없어 끔)
//   flat       = fly_flat 한 번 (Float64Array 한 개: 머리 12 + 경로 7/점), 앱 걸음 + min_steps 6; 경로를 subarray 로 떼어 냄
//   flat-off   = 같은 호출, 경로를 같은 배열의 HEAD 칸 뒤에서 읽음 (subarray 할당 없음; 앱 golfCore.ts 가 이 방식)
//   flat/tau05 = fly_flat, 이전 걸음 τ 0.05 (걸음 효과와 경계 효과를 따로 보려고)
// 호출 횟수는 글루의 `wasm = instance.exports` 를 호출 세는 프록시로 바꾼 복사본으로 센다.
// 시간은 같은 세션에서 번갈아 여러 번 돌려 최소값을 쓴다 (이 기계는 다른 작업과 CPU 를 나눠 쓴다: 절대값보다 비율).

import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const which = process.argv[2] === 'simd' ? 'pkg-simd' : 'pkg';
const dir = path.resolve(here, '..', which);

// ---- 호출 세는 글루 복사본
const glue = readFileSync(path.join(dir, 'golf_core.js'), 'utf8');
if (!glue.includes('wasm = instance.exports;')) throw new Error('글루 형식이 달라 호출 세기 패치를 못 한다');
const counted = glue.replace('wasm = instance.exports;', 'wasm = globalThis.__countExports(instance.exports);');
const tmp = mkdtempSync(path.join(tmpdir(), 'golf-core-count-'));
writeFileSync(path.join(tmp, 'golf_core.mjs'), counted);
let crossings = 0;
const perName = new Map();
// 내보내기 객체는 얼려 있어서 Proxy 를 못 씌운다: 함수만 세는 래퍼로 바꾼 새 객체를 돌려준다 (memory 같은 비함수는 그대로)
globalThis.__countExports = (ex) => {
  const o = {};
  for (const k of Object.keys(ex)) {
    const v = ex[k];
    o[k] =
      typeof v === 'function'
        ? (...a) => {
            crossings++;
            perName.set(k, (perName.get(k) ?? 0) + 1);
            return v(...a);
          }
        : v;
  }
  return o;
};
const bytes = readFileSync(path.join(dir, 'golf_core_bg.wasm'));

const cnt = await import(pathToFileURL(path.join(tmp, 'golf_core.mjs')).href);
cnt.initSync({ module: bytes }); // 호출 세는 래퍼 exports 를 쓴다
const plain = await import(pathToFileURL(path.join(dir, 'golf_core.js')).href);
plain.initSync({ module: bytes });

const H_APP = plain.app_step();
const MIN_STEPS = plain.app_min_steps();
const HEAD = 12;
const STRIDE = 7;

// ---- 샷 4000개 (bench.mjs 와 같은 식)
const A = [0.618033988749895, 0.7548776662466927, 0.5698402909980532, 0.4142135623730951];
const frac = (x) => x - Math.floor(x);
const N = 4000;
const shots = new Float64Array(N * 4);
for (let i = 0; i < N; i++) {
  const x = (k) => frac(0.5 + (i + 1) * A[k]);
  shots[4 * i] = 60 + 120 * x(0);
  shots[4 * i + 1] = 5 + 30 * x(1);
  shots[4 * i + 2] = 1500 + 8500 * x(2);
  shots[4 * i + 3] = -30 + 60 * x(3);
}

// ---- 변종 (각각 요약 객체와 경로 배열을 만든다)
const YARD = 0.9144;
function oldApi(mod, h) {
  return (i) => {
    const f = mod.fly_f64(shots[4 * i], shots[4 * i + 1], shots[4 * i + 2], shots[4 * i + 3], h);
    try {
      const stats = {
        carryYd: f.carry_yd, carryM: f.carry_m, apexM: f.apex_m, landDeg: f.land_deg, lateralM: f.lateral_m, timeS: f.time_s,
        steps: f.steps, landMph: f.land_mph, rollYd: f.roll_yd, totalYd: f.total_yd, totalReliable: f.total_reliable,
      };
      return { stats, path: f.path() };
    } finally {
      f.free();
    }
  };
}
function stats11(a) {
  if (a[11] !== STRIDE) throw new Error('경로 배치 불일치');
  return {
    carryYd: a[0] / YARD, carryM: a[0], apexM: a[2], landDeg: a[3], lateralM: a[1], timeS: a[4],
    steps: a[5], landMph: a[6], rollYd: a[7], totalYd: a[8], totalReliable: a[9] !== 0,
  };
}
// 글루가 Vec<f64> 를 Float64Array 로 복사해 돌려준다 (할당 1) + 경로만 따로 뗀 subarray (할당 1)
function flatSub(mod, h, ms) {
  return (i) => {
    const a = mod.fly_flat(shots[4 * i], shots[4 * i + 1], shots[4 * i + 2], shots[4 * i + 3], h, ms);
    return { stats: stats11(a), path: a.subarray(HEAD) };
  };
}
// 같은 호출, 경로는 같은 배열에서 HEAD 칸 뒤부터 읽는다 (할당 1)
function flatOff(mod, h, ms) {
  return (i) => {
    const a = mod.fly_flat(shots[4 * i], shots[4 * i + 1], shots[4 * i + 2], shots[4 * i + 3], h, ms);
    return { stats: stats11(a), path: a };
  };
}
// 경계 없이 계산만 (바닥): 같은 걸음의 f64 스칼라 일괄 → 샷당 ns. 샷 하나 변종과 같은 모양을 맞추려고 4000샷 한 번 부른 비용을 샷당으로 나눠 낸다
const floorNs = (mod, h, ms) => {
  let best = Infinity;
  for (let r = 0; r < 20; r++) {
    const t = performance.now();
    mod.batch_opts(shots, h, false, ms, false);
    best = Math.min(best, ((performance.now() - t) * 1e6) / N);
  }
  return best;
};

const variants = [
  ['old (τ0.05, 객체 API)', oldApi(plain, 0.05), oldApi(cnt, 0.05)],
  [`old@app (τ${H_APP}, 객체 API)`, oldApi(plain, H_APP), oldApi(cnt, H_APP)],
  ['flat/τ0.05 (fly_flat, sub)', flatSub(plain, 0.05, 0), flatSub(cnt, 0.05, 0)],
  [`flat (τ${H_APP}+min${MIN_STEPS}, sub)`, flatSub(plain, H_APP, MIN_STEPS), flatSub(cnt, H_APP, MIN_STEPS)],
  [`flat-off (τ${H_APP}+min${MIN_STEPS})`, flatOff(plain, H_APP, MIN_STEPS), flatOff(cnt, H_APP, MIN_STEPS)],
];

console.log(`${which}/ (has_simd ${plain.has_simd()}), wasm ${bytes.length} B`);
console.log('\n[샷당 JS↔wasm 호출 횟수]');
for (const [name, , counting] of variants) {
  crossings = 0;
  perName.clear();
  for (let i = 0; i < 100; i++) counting(i);
  const detail = name.startsWith('old (') ? ' ' + JSON.stringify(Object.fromEntries([...perName].map(([k, v]) => [k, v / 100]))) : '';
  console.log(`  ${name.padEnd(30)} ${(crossings / 100).toFixed(2)} 번${detail}`);
}

// ---- 시간
// 이 기계는 보통 CPU 100% 라 벽시계가 크게 흔들린다. 우선순위를 올리고, 짧은 반복(한 번 약 10–50 ms)을 많이 번갈아 돌려
// 조용한 틈에 걸린 최소값을 쓴다. 프로세스 CPU 시간(선점을 빼 준다)도 같이 낸다.
try {
  os.setPriority(os.constants.priority.PRIORITY_HIGH);
} catch {
  /* 권한이 없으면 그대로 */
}
const WARM = 30000;
const PER = Number(process.env.PER ?? 4000);
const REPS = Number(process.env.REPS ?? 60);
const sink = { v: 0 };
function timeOne(fn) {
  const t = performance.now();
  for (let k = 0; k < PER; k++) {
    const r = fn(k % N);
    sink.v += r.stats.carryM + r.path.length;
  }
  return ((performance.now() - t) * 1e6) / PER;
}
const wallAll = variants.map(() => []);
for (const [, fn] of variants) for (let k = 0; k < WARM; k++) fn(k % N);
for (let rep = 0; rep < REPS; rep++) {
  variants.forEach(([, fn], vi) => {
    wallAll[vi].push(timeOne(fn));
  });
}
const stat = (xs) => {
  const s = xs.slice().sort((a, b) => a - b);
  return { min: s[0], med: s[(s.length / 2) | 0] };
};
console.log(`
[샷당 ns: 요약 객체 + 경로 배열까지 꺼내기. 벽시계 최소 / 중앙값 (${REPS}회 × ${PER}샷)]`);
const W = variants.map((_, vi) => stat(wallAll[vi]));
variants.forEach(([name], vi) => {
  console.log(`  ${name.padEnd(34)} ${W[vi].min.toFixed(0).padStart(7)} / ${W[vi].med.toFixed(0).padStart(7)}`);
});
const fl = floorNs(plain, H_APP, MIN_STEPS);
console.log(`  ${'(바닥) 경계 없는 f64 스칼라 일괄'.padEnd(34)} ${fl.toFixed(0).padStart(7)}   ← 같은 걸음, 샷 4000개 한 번에 / 4000`);
console.log(`
  old 대비 (벽 최소): ${variants.map(([n], vi) => `${n.split(' ')[0]} ${(W[0].min / W[vi].min).toFixed(2)}배`).join(', ')}`);
if (sink.v === 1234.5) console.log(sink.v);
