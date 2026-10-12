// 물리 성능 점검: 헤드리스 Chromium(V8) 안에서 샷당 시간을 잰다.
//   - golf-core wasm (기본 빌드, SIMD 빌드) 와 web3d 의 TS 엔진(`src/engine`), 지면 바운드·굴림(`src/game/ground.ts`)
//   - 같은 4000샷(볼스피드 60–180 mph, 발사각 5–35°, 스핀 1500–10000 rpm, 축 ±30°, 저불일치 수열), 샷당 최소 5회
//
//   먼저 개발 서버가 떠 있어야 한다(`npm run dev`). golf-core 가 빌드돼 있어야 한다(`cd ../golf-core && sh build-wasm.sh`).
//   node scripts/perf.mjs [--url http://127.0.0.1:5190/]
//
// 측정 중에만 `public/_perf/` 에 wasm 패키지를 복사해 두고 끝나면 지운다.
import { cpSync, existsSync, rmSync } from 'node:fs';
import os from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const URL_BASE = opt('--url', 'http://127.0.0.1:5190/');
const CORE = resolve(ROOT, '..', 'golf-core');
const TMP = join(ROOT, 'public', '_perf');

for (const d of ['pkg', 'pkg-simd']) {
  if (!existsSync(join(CORE, d, 'golf_core_bg.wasm'))) throw new Error(`golf-core/${d} 가 없다: cd golf-core && sh build-wasm.sh`);
}
rmSync(TMP, { recursive: true, force: true });
for (const d of ['pkg', 'pkg-simd']) cpSync(join(CORE, d), join(TMP, d), { recursive: true });

const browser = await chromium.launch({ channel: 'chromium', headless: true, args: ['--enable-gpu', '--ignore-gpu-blocklist', '--use-angle=d3d11'] });
try {
  const page = await browser.newPage();
  page.on('pageerror', (e) => console.log(`  [pageerror] ${e.message.slice(0, 300)}`));
  await page.goto(`${URL_BASE}?mode=range`, { waitUntil: 'domcontentloaded' });
  const r = await page.evaluate(async () => {
    const origin = location.origin;
    const N = 4000;
    const shots = new Float64Array(N * 4);
    const phi = [0.7548776662466927, 0.5698402909980532, 0.3819660112501051, 0.6180339887498949];
    for (let i = 0; i < N; i++) {
      const f = (k) => (0.5 + (i + 1) * phi[k]) % 1;
      shots[4 * i] = 60 + 120 * f(0);
      shots[4 * i + 1] = 5 + 30 * f(1);
      shots[4 * i + 2] = 1500 + 8500 * f(2);
      shots[4 * i + 3] = -30 + 60 * f(3);
    }
    const best = (fn, reps = 5) => {
      fn();
      let b = 1e18;
      for (let k = 0; k < reps; k++) {
        const t0 = performance.now();
        fn();
        b = Math.min(b, performance.now() - t0);
      }
      return b;
    };
    const perShot = (ms, n) => (ms * 1e6) / n; // ns/샷
    const out = { cores: navigator.hardwareConcurrency, ua: navigator.userAgent.match(/Chrome\/[\d.]+/)?.[0] };
    const loadWasm = async (dir) => {
      const mod = await import(`${origin}/_perf/${dir}/golf_core.js`);
      await mod.default({ module_or_path: `${origin}/_perf/${dir}/golf_core_bg.wasm` });
      return mod;
    };
    const base = await loadWasm('pkg');
    const simd = await loadWasm('pkg-simd');
    out.simd = simd.has_simd();
    out.wasm = {
      'f64 τ0.137': perShot(best(() => base.batch(shots, 0.137, false)), N),
      'f64 τ0.05': perShot(best(() => base.batch(shots, 0.05, false)), N),
      'f32 τ0.137': perShot(best(() => base.batch(shots, 0.137, true)), N),
      'SIMD τ0.137': perShot(best(() => simd.batch_simd(shots, 0.137)), N),
      'SIMD τ0.05': perShot(best(() => simd.batch_simd(shots, 0.05)), N),
    };
    // 단일 호출 (경로 포함): 게임이 샷 한 번에 부르는 방식
    out.wasmSingle = perShot(best(() => { for (let i = 0; i < 500; i++) base.fly_f64(shots[4 * i], shots[4 * i + 1], shots[4 * i + 2], shots[4 * i + 3], 0.137); }), 500);

    const { simulate } = await import(`${origin}/src/engine/flight.ts`);
    const M = 2000;
    const launch = (i) => ({ mph: shots[4 * i], launchDeg: shots[4 * i + 1], rpm: shots[4 * i + 2], axisDeg: shots[4 * i + 3] });
    out.ts = {
      'τ0.05 (기본, 궤적 없음)': perShot(best(() => { for (let i = 0; i < M; i++) simulate(launch(i), { samples: false }); }), M),
      'τ0.137 (궤적 없음)': perShot(best(() => { for (let i = 0; i < M; i++) simulate(launch(i), { samples: false, h: 0.137 }); }), M),
      'τ0.05 + 궤적 표본': perShot(best(() => { for (let i = 0; i < M; i++) simulate(launch(i), { samples: true }); }), M),
    };
    // 지면 바운드·굴림 (게임 샷 한 번의 착지 뒤 계산)
    const { HOLES } = await import(`${origin}/src/game/course.ts`);
    const { bounceAndRoll } = await import(`${origin}/src/game/ground.ts`);
    const hole = HOLES[0];
    const lands = [];
    for (let i = 0; i < 400; i++) {
      const o = simulate({ mph: 90 + (i % 70), launchDeg: 8 + (i % 17), rpm: 2500 + 60 * (i % 90), axisDeg: ((i % 9) - 4) * 2 }, { samples: true });
      const s = o.samples[o.samples.length - 1];
      lands.push({ x: s.x, z: s.z, vx: s.vx, vy: -Math.abs(s.vy), vz: s.vz, backspinRpm: o.landSpinRpm });
    }
    out.ground = perShot(best(() => { for (const l of lands) bounceAndRoll(hole, l); }), lands.length);
    return out;
  });
  const us = (ns) => (ns / 1000).toFixed(2);
  console.log(`\n브라우저 ${r.ua}, 논리 코어 ${r.cores}, CPU ${os.cpus()[0]?.model.trim()}, wasm SIMD ${r.simd ? '지원' : '미지원'}`);
  console.log('\n[wasm golf-core] 샷당 (4000샷 배치)');
  for (const [k, v] of Object.entries(r.wasm)) console.log(`  ${k.padEnd(14)} ${us(v).padStart(8)} µs   ${(1e9 / v / 1e6).toFixed(2)} M샷/초`);
  console.log(`  단일 호출 fly_f64 (경로 포함, τ0.137) ${us(r.wasmSingle)} µs`);
  console.log('\n[TS 엔진 web3d/src/engine] 샷당 (2000샷)');
  for (const [k, v] of Object.entries(r.ts)) console.log(`  ${k.padEnd(24)} ${us(v).padStart(8)} µs   wasm SIMD τ0.137 대비 ${(v / r.wasm['SIMD τ0.137']).toFixed(1)}배`);
  console.log(`\n[지면 바운드·굴림 web3d/src/game/ground.ts] 샷당 ${us(r.ground)} µs (착지 400개 평균)`);
  console.log('\n한 프레임 예산 16.7 ms 대비: 비행 + 지면 계산을 한 샷에 한 번 하므로 프레임 시간에 거의 영향이 없다.');
} finally {
  await browser.close();
  rmSync(TMP, { recursive: true, force: true });
}
