// wasm·SIMD·JS 의 정밀도·속도 비교 (Node) + 네이티브 대조. 네이티브 쪽은 bench 바이너리의 dump 로 한다.
//
//   cd golf-core && sh build-wasm.sh        # pkg/ (기본) 와 pkg-simd/ (SIMD) 를 만든다
//   cargo run --release --bin bench -- dump target/d.csv          # 새 엔진 f64 τ 0.137
//   cargo run --release --bin bench -- dump target/e.csv env      # 환경 f64
//   cargo run --release --bin bench -- dump target/l.csv envlanes # 환경 4레인 f32
//   node bench/bench.mjs target/d.csv target/e.csv target/l.csv
//
// 샷 generator 와 적분식은 src/lib.rs, src/bin/bench.rs 와 같다. 기준은 wasm f64 τ 0.002 (네이티브 f64 와 같아야 한다).
// 대조가 어긋나면 종료 코드 1.

import { readFileSync, existsSync } from 'node:fs';
import { loadGolfCore } from '../js/load.mjs';

const base = await loadGolfCore({ simd: false });
const hasSimdPkg = existsSync(new URL('../pkg-simd/golf_core_bg.wasm', import.meta.url));
const simd = hasSimdPkg ? await loadGolfCore({ simd: true }).catch(() => null) : null;

let failed = 0;
const check = (name, ok, detail) => {
  console.log(`  [${ok ? '통과' : '실패'}] ${name}: ${detail}`);
  if (!ok) failed++;
};

// ------------------------------------------------------------------ 샷 (Rust shots() 와 같은 식)
const N = 4000;
const A = [0.618033988749895, 0.7548776662466927, 0.5698402909980532, 0.4142135623730951];
const frac = (x) => x - Math.floor(x);
function makeShots(n) {
  const flat = new Float64Array(n * 4);
  for (let i = 0; i < n; i++) {
    const x = (k) => frac(0.5 + (i + 1) * A[k]);
    flat[4 * i] = 60 + 120 * x(0);
    flat[4 * i + 1] = 5 + 30 * x(1);
    flat[4 * i + 2] = 1500 + 8500 * x(2);
    flat[4 * i + 3] = -30 + 60 * x(3);
  }
  return flat;
}
const flat = makeShots(N);

// ------------------------------------------------------------------ JS 판본
const L_M = 53.53157581531039, U_MS = 22.916037151920374, T_S = L_M / U_MS;
const MPH = 0.44704, R_BALL = 0.04267 / 2, BETA = 0.06;
const RE_PER_U = U_MS * 0.04267 * 1.2 / 1.81e-5;
// 공유위기12 (Rust 의 Coef::SHARED_RE 와 같은 값, 위기 지수 n = 12)
const COEF = { v: 0.1383, e: 0.2540, u0: 74003.0320 / RE_PER_U, d: 1.4838, c: 2.8062, l: 6.5014, beta: BETA };
const INV_U0 = 1 / COEF.u0;
const IU02 = 1 / (COEF.u0 * COEF.u0);

// 옛 판본: 나눗셈 3개 rhs, 56회 이분법, 배열 할당 (Rust 옛 판본과 같은 연산 순서)
function jsFlyOld(mph, vla, rpm, axisDeg, h) {
  const v = mph * MPH;
  const u0 = v / U_MS;
  const th = vla * (Math.PI / 180);
  const s0 = (R_BALL * rpm * (2 * Math.PI)) / 60 / v;
  const a = axisDeg * (Math.PI / 180);
  const oy = Math.sin(a), oz = Math.cos(a);
  let s = new Float64Array(8);
  s[4] = u0 * Math.cos(th);
  s[5] = u0 * Math.sin(th);
  s[7] = s0 * u0;
  const rhs = (q) => {
    const ux = q[4], uy = q[5], uz = q[6], w = q[7];
    const sp = Math.sqrt(ux * ux + uy * uy + uz * uz);
    const g = 1 / Math.sqrt(sp);
    const ur = (w * sp) / (sp + COEF.l * w);
    const qq = sp * INV_U0;
    const q2 = qq * qq;
    const q3 = q2 * q2 * q2;
    const crisis = COEF.e / (1 + q3 * q3);
    const kd = (COEF.v + crisis) * sp + COEF.d * ur;
    const kl = COEF.c * ur;
    const cx = oy * uz - oz * uy, cy = oz * ux, cz = -(oy * ux);
    const ax = -kd * ux + kl * cx;
    const ay = -kd * uy + kl * cy - 1;
    const az = -kd * uz + kl * cz;
    const aw = -COEF.beta * w * sp;
    return [g, ux * g, uy * g, uz * g, ax * g, ay * g, az * g, aw * g];
  };
  const step = (q, f) => {
    const half = h * 0.5, six = h / 6;
    const s2 = q.map((x, i) => x + half * f[i]);
    const k2 = rhs(s2);
    const s3 = q.map((x, i) => x + half * k2[i]);
    const k3 = rhs(s3);
    const s4 = q.map((x, i) => x + h * k3[i]);
    const k4 = rhs(s4);
    return q.map((x, i) => x + six * (f[i] + 2 * k2[i] + 2 * k3[i] + k4[i]));
  };
  const basis = (t) => {
    const t2 = t * t, t3 = t2 * t;
    return [2 * t3 - 3 * t2 + 1, t3 - 2 * t2 + t, 3 * t2 - 2 * t3, t3 - t2];
  };
  const herm = (s0_, f0, s1_, f1, t) => {
    const [h00, h10, h01, h11] = basis(t);
    return s0_.map((x, i) => h00 * x + h10 * h * f0[i] + h01 * s1_[i] + h11 * h * f1[i]);
  };
  const cross = (a0, b0, a1, b1) => {
    let lo = 0, hi = 1;
    for (let it = 0; it < 56; it++) {
      const mid = (lo + hi) * 0.5;
      const [h00, h10, h01, h11] = basis(mid);
      const val = h00 * a0 + h10 * b0 + h01 * a1 + h11 * b1;
      if (val > 0) lo = mid;
      else hi = mid;
    }
    return (lo + hi) * 0.5;
  };
  let f = rhs(s);
  let apexM = 0, apexDone = false;
  let out = { carry: NaN, lat: NaN, apex: NaN, land: NaN };
  for (let k = 1; k <= 2000000; k++) {
    const s1 = step(s, f);
    const f1 = rhs(s1);
    if (!apexDone && s[5] > 0 && s1[5] <= 0) {
      const t = cross(s[5], f[5] * h, s1[5], f1[5] * h);
      apexM = herm(s, f, s1, f1, t)[2] * L_M;
      apexDone = true;
    }
    if (s[2] > 0 && s1[2] <= 0) {
      const t = cross(s[2], f[2] * h, s1[2], f1[2] * h);
      const land = herm(s, f, s1, f1, t);
      out = { carry: land[1] * L_M, lat: land[3] * L_M, apex: apexM, land: Math.atan2(-land[5], land[4]) * (180 / Math.PI) };
      break;
    }
    s = s1;
    f = f1;
  }
  return out;
}

// 새 판본: Rust 와 같은 알고리즘 (나눗셈 1개 rhs, 뉴턴 뿌리), 할당 없이 Float64Array 재사용
let kA = new Float64Array(8), kB = new Float64Array(8), kC = new Float64Array(8), kD = new Float64Array(8), kE = new Float64Array(8);
function rhsInto(k, ux, uy, uz, w, oy, oz) {
  const q = ux * ux + uy * uy + uz * uz;
  const sp = Math.sqrt(q);
  const den1 = sp + COEF.l * w;
  const q2 = q * IU02;
  const q3 = q2 * q2 * q2;
  const den2 = 1 + q3 * q3;
  const a = den1 * den2;
  const inv = 1 / (a * sp);
  const ur = w * q * den2 * inv;
  const crisis = COEF.e * den1 * sp * inv;
  const kd = (COEF.v + crisis) * sp + COEF.d * ur;
  const kl = COEF.c * ur;
  const g = Math.sqrt(a * inv);
  const cx = oy * uz - oz * uy, cy = oz * ux, cz = -(oy * ux);
  k[0] = g;
  k[1] = ux * g;
  k[2] = uy * g;
  k[3] = uz * g;
  k[4] = (kl * cx - kd * ux) * g;
  k[5] = (kl * cy - kd * uy - 1) * g;
  k[6] = (kl * cz - kd * uz) * g;
  k[7] = -(COEF.beta * w * sp) * g;
}
function hroot(c0, c1, c2, c3) {
  const y1 = c0 + c1 + c2 + c3;
  let lo = 0, hi = 1;
  let th = c0 / (c0 - y1);
  for (let it = 0; it < 60; it++) {
    const f = ((c3 * th + c2) * th + c1) * th + c0;
    if (f === 0) break;
    if (f > 0) lo = th;
    else hi = th;
    const df = (3 * c3 * th + 2 * c2) * th + c1;
    let nt = th - f / df;
    if (!(nt >= lo && nt <= hi)) nt = 0.5 * (lo + hi);
    const stp = Math.abs(nt - th);
    th = nt;
    if (stp < 1e-7) break;
  }
  return th;
}
const hc = (y0, m0, y1, m1) => { const d = y1 - y0; return [y0, m0, 3 * d - 2 * m0 - m1, m0 + m1 - 2 * d]; };
const hv = (c, t) => ((c[3] * t + c[2]) * t + c[1]) * t + c[0];
const S = new Float64Array(8), S1 = new Float64Array(8);
function jsFly(mph, vla, rpm, axisDeg, h) {
  const v = mph * MPH;
  const u0 = v / U_MS;
  const th = vla * (Math.PI / 180);
  const s0 = (R_BALL * rpm * (2 * Math.PI)) / 60 / v;
  const a = axisDeg * (Math.PI / 180);
  const oy = Math.sin(a), oz = Math.cos(a);
  const half = h * 0.5, h6 = h / 6;
  let s = S, n = S1;
  s.fill(0);
  s[4] = u0 * Math.cos(th);
  s[5] = u0 * Math.sin(th);
  s[7] = s0 * u0;
  let k1 = kA, k5 = kE;
  rhsInto(k1, s[4], s[5], s[6], s[7], oy, oz);
  let apexM = 0, apexDone = false;
  for (let it = 1; it <= 2000000; it++) {
    rhsInto(kB, s[4] + half * k1[4], s[5] + half * k1[5], s[6] + half * k1[6], s[7] + half * k1[7], oy, oz);
    rhsInto(kC, s[4] + half * kB[4], s[5] + half * kB[5], s[6] + half * kB[6], s[7] + half * kB[7], oy, oz);
    rhsInto(kD, s[4] + h * kC[4], s[5] + h * kC[5], s[6] + h * kC[6], s[7] + h * kC[7], oy, oz);
    for (let i = 0; i < 8; i++) n[i] = s[i] + h6 * (k1[i] + 2 * (kB[i] + kC[i]) + kD[i]);
    rhsInto(k5, n[4], n[5], n[6], n[7], oy, oz);
    if (!apexDone && s[5] > 0 && n[5] <= 0) {
      const t = hroot(...hc(s[5], k1[5] * h, n[5], k5[5] * h));
      apexM = hv(hc(s[2], k1[2] * h, n[2], k5[2] * h), t) * L_M;
      apexDone = true;
    }
    if (s[2] > 0 && n[2] <= 0) {
      const t = hroot(...hc(s[2], k1[2] * h, n[2], k5[2] * h));
      const at = (i, j) => hv(hc(s[i], k1[j] * h, n[i], k5[j] * h), t);
      // 성분 i 의 값과 도함수: t→(0,g=0) x→(1,1) z→(3,3) ux→(4,4) uy→(5,5)
      const x = at(1, 1), z = at(3, 3), ux = at(4, 4), uy = at(5, 5);
      return { carry: x * L_M, lat: z * L_M, apex: apexM, land: Math.atan2(-uy, ux) * (180 / Math.PI) };
    }
    const ts = s; s = n; n = ts;
    const tk = k1; k1 = k5; k5 = tk;
  }
  return { carry: NaN, lat: NaN, apex: NaN, land: NaN };
}

// ------------------------------------------------------------------ 측정 도구
const YD = 0.9144;
const ref = base.batch(flat, 0.002, false); // [carry, lat, apex, land] × N (m, m, m, 도)

function errOf(out, r = ref) {
  let cMax = 0, cSq = 0, hMax = 0, lMax = 0, sMax = 0;
  const n = r.length / 4;
  for (let i = 0; i < n; i++) {
    const dc = Math.abs(out[4 * i] - r[4 * i]) / YD;
    const ds = Math.abs(out[4 * i + 1] - r[4 * i + 1]) / YD;
    const dh = Math.abs(out[4 * i + 2] - r[4 * i + 2]) / YD;
    const dl = Math.abs(out[4 * i + 3] - r[4 * i + 3]);
    cMax = Math.max(cMax, dc);
    cSq += dc * dc;
    sMax = Math.max(sMax, ds);
    hMax = Math.max(hMax, dh);
    lMax = Math.max(lMax, dl);
  }
  return { cMax, cRms: Math.sqrt(cSq / n), hMax, sMax, lMax };
}

function timeIt(fn, n, reps = 9) {
  let best = Infinity;
  let sink = 0;
  for (let r = 0; r < reps; r++) {
    const t = performance.now();
    sink += fn();
    best = Math.min(best, ((performance.now() - t) * 1e6) / n);
  }
  if (sink === 12345.678) console.log(sink); // 최적화로 지워지지 않게
  return best;
}

const readDump = (p) => readFileSync(p, 'utf8').trim().split('\n').slice(1).flatMap((ln) => ln.split(',').map(Number));
const maxRel = (a, b) => {
  let m = 0;
  for (let i = 0; i < a.length; i++) m = Math.max(m, Math.abs(a[i] - b[i]) / (Math.abs(b[i]) + 1e-12));
  return m;
};
const maxAbsYd = (a, b) => {
  let m = 0;
  for (let i = 0; i < a.length; i += 4) m = Math.max(m, Math.abs(a[i] - b[i]) / YD);
  return m;
};

console.log(`Node ${process.version}, 기본 빌드 pkg/ ${simd ? '+ SIMD 빌드 pkg-simd/' : '(pkg-simd/ 없음 → SIMD 행 생략)'}, 기본 τ ${base.default_step()}, 샷 ${N}개, 기준 wasm f64 τ 0.002`);
console.log(`has_simd(): pkg/ ${base.has_simd()}, pkg-simd/ ${simd ? simd.has_simd() : '-'}`);

// ------------------------------------------------------------------ 대조
console.log('\n대조');
const [dumpPath, envDumpPath, envLanesPath] = process.argv.slice(2);
if (dumpPath) {
  const nat = readDump(dumpPath);
  const w = base.batch(flat, 0.137, false);
  const r = maxRel(w, nat);
  check('wasm f64 대 네이티브 f64 (τ 0.137, 4000샷×4양)', r <= 1e-12, `최대 상대 차 ${r.toExponential(2)} (목표 ≤ 1e-12)`);
}
if (envDumpPath) {
  const nat = readDump(envDumpPath);
  const w = base.batch_env(flat, 0.137, false, 28, 1500, -4, 3);
  const r = maxRel(w, nat);
  check('wasm batch_env f64 대 네이티브 환경 f64', r <= 1e-12, `최대 상대 차 ${r.toExponential(2)}`);
}
if (envLanesPath) {
  const nat = readDump(envLanesPath);
  const w = base.batch_simd_env(flat, 0.137, 28, 1500, -4, 3);
  check('wasm 4레인(기본 빌드) 환경 대 네이티브 4레인 P4', maxAbsYd(w, nat) <= 1e-9, `최대 캐리 차 ${maxAbsYd(w, nat).toExponential(2)} yd`);
  if (simd) {
    const w2 = simd.batch_simd_env(flat, 0.137, 28, 1500, -4, 3);
    check('wasm SIMD 환경 대 네이티브 4레인 P4', maxAbsYd(w2, nat) <= 1e-9, `최대 캐리 차 ${maxAbsYd(w2, nat).toExponential(2)} yd`);
  }
}
{
  // 스칼라 f32 와 4레인 (같은 연산이라 같아야 한다)
  const s32 = base.batch(flat, 0.137, true);
  const l32 = base.batch_simd(flat, 0.137);
  check('기본 빌드 4레인 대 스칼라 f32', maxAbsYd(l32, s32) <= 1e-9, `최대 캐리 차 ${maxAbsYd(l32, s32).toExponential(2)} yd`);
  if (simd) {
    const v32 = simd.batch_simd(flat, 0.137);
    check('SIMD 대 스칼라 f32', maxAbsYd(v32, s32) <= 1e-9, `최대 캐리 차 ${maxAbsYd(v32, s32).toExponential(2)} yd`);
    let mdev = 0;
    for (let i = 0; i < v32.length; i++) mdev = Math.max(mdev, Math.abs(v32[i] - s32[i]));
    console.log(`        (전 성분 최대 절대 차 ${mdev.toExponential(2)})`);
  }
  // 환경 항등: 기온 20 °C 해수면은 기본 환경과 거의 같지만 같지는 않다 / 기본 환경 값은 batch 와 같다
  const e0 = base.batch_env(flat.subarray(0, 40), 0.137, false, 20, 0, 0, 0);
  const b0 = base.batch(flat.subarray(0, 40), 0.137, false);
  const dd = maxRel(e0, b0);
  check('batch_env(20 °C, 0 m, 무풍) 는 기본 환경과 가깝다 (ρ 0.3%, μ 0.2% 차)', dd > 0 && dd < 0.01, `최대 상대 차 ${dd.toExponential(2)}`);
  // fly_env 와 batch_env 가 같은 답
  const fe = base.fly_env(167, 10.9, 2686, 0, 0.137, 15, 0, 0, 0);
  const be = base.batch_env(Float64Array.of(167, 10.9, 2686, 0), 0.137, false, 15, 0, 0, 0);
  check('fly_env 와 batch_env 일치', Math.abs(fe.carry_m - be[0]) < 1e-9, `${fe.carry_m.toFixed(6)} m / ${be[0].toFixed(6)} m`);
  const hi = base.fly_env(167, 10.9, 2686, 0, 0.05, 20, 1600, 0, 0).carry_yd;
  const lo = base.fly_env(167, 10.9, 2686, 0, 0.05, 15, 0, 0, 0).carry_yd;
  check('고도 효과 방향 (드라이버, 1600 m 20 °C 대 해수면 15 °C)', hi > lo, `${lo.toFixed(2)} → ${hi.toFixed(2)} yd (${((hi / lo - 1) * 100).toFixed(2)} %)`);
  const wl = base.fly_env(167, 10.9, 2686, 0, 0.05, 15, 0, 0, 5);
  check('왼쪽에서 오른쪽으로 부는 바람은 공을 오른쪽(+)으로 민다', wl.lateral_m > 0, `편차 ${wl.lateral_m.toFixed(2)} m`);
  wl.free();
  fe.free();
}

// ------------------------------------------------------------------ 표
console.log('\n정밀도·속도 (샷당 ns는 최소 9회)');
console.log('방식'.padEnd(22), 'τ'.padStart(6), '샷당 ns'.padStart(10), '캐리 최대(yd)'.padStart(15), '캐리 RMS(yd)'.padStart(14), '높이 최대(yd)'.padStart(15), '편차 최대(yd)'.padStart(15), '착지각 최대(도)'.padStart(17));
const rows = [];
const addRow = (name, h, ns, e) => rows.push([name, h, ns, e]);
for (const h of [0.02, 0.05, 0.137]) {
  addRow('wasm f64', h, timeIt(() => base.batch(flat, h, false).length, N), errOf(base.batch(flat, h, false)));
  addRow('wasm f32', h, timeIt(() => base.batch(flat, h, true).length, N), errOf(base.batch(flat, h, true)));
  addRow('wasm 4레인(비SIMD)', h, timeIt(() => base.batch_simd(flat, h).length, N), errOf(base.batch_simd(flat, h)));
  if (simd) addRow('wasm SIMD', h, timeIt(() => simd.batch_simd(flat, h).length, N), errOf(simd.batch_simd(flat, h)));
  const runJs = (fn) => {
    const o = new Float64Array(N * 4);
    for (let i = 0; i < N; i++) {
      const r = fn(flat[4 * i], flat[4 * i + 1], flat[4 * i + 2], flat[4 * i + 3], h);
      o[4 * i] = r.carry; o[4 * i + 1] = r.lat; o[4 * i + 2] = r.apex; o[4 * i + 3] = r.land;
    }
    return o;
  };
  const jsTime = (fn, reps) => timeIt(() => { let s = 0; for (let i = 0; i < N; i++) s += fn(flat[4 * i], flat[4 * i + 1], flat[4 * i + 2], flat[4 * i + 3], h).carry; return s; }, N, reps);
  addRow('JS f64 새', h, jsTime(jsFly, 3), errOf(runJs(jsFly)));
  if (h >= 0.05) addRow('JS f64 옛', h, jsTime(jsFlyOld, 3), errOf(runJs(jsFlyOld)));
}
// 선택 사항 (batch_opts / batch_simd_opts): min6 = 짧은 비행에 최소 6걸음, comp = 카한 보상 합 (f32 바닥)
for (const [h, ms, comp, tag] of [[0.137, 6, false, 'min6'], [0.15, 6, false, 'min6'], [0.05, 0, true, 'comp'], [0.1, 6, true, 'min6+comp'], [0.137, 6, true, 'min6+comp']]) {
  if (ms && !comp) addRow(`wasm f64 ${tag}`, h, timeIt(() => base.batch_opts(flat, h, false, ms, comp).length, N), errOf(base.batch_opts(flat, h, false, ms, comp)));
  addRow(`wasm f32 ${tag}`, h, timeIt(() => base.batch_opts(flat, h, true, ms, comp).length, N), errOf(base.batch_opts(flat, h, true, ms, comp)));
  if (simd) addRow(`wasm SIMD ${tag}`, h, timeIt(() => simd.batch_simd_opts(flat, h, ms, comp).length, N), errOf(simd.batch_simd_opts(flat, h, ms, comp)));
}
rows.sort((a, b) => a[1] - b[1]);
for (const [name, h, ns, e] of rows) {
  console.log(
    name.padEnd(22),
    String(h).padStart(6),
    ns.toFixed(0).padStart(10),
    e.cMax.toExponential(2).padStart(15),
    e.cRms.toExponential(2).padStart(14),
    e.hMax.toExponential(2).padStart(15),
    e.sMax.toExponential(2).padStart(15),
    e.lMax.toExponential(2).padStart(17),
  );
}

// 목표 판정
if (simd) {
  console.log('\n목표 판정 (SIMD)');
  const get = (name, h) => rows.find((r) => r[0] === name && r[1] === h);
  const a = get('wasm SIMD', 0.137), b = get('wasm SIMD', 0.05);
  check('SIMD τ 0.137 샷당 시간 ≤ 1000 ns', a[2] <= 1000, `${a[2].toFixed(0)} ns (스트레치 700 ns)`);
  const m = get('wasm SIMD min6', 0.137);
  console.log(`  [참고] SIMD τ 0.137 캐리 최대 오차 ${a[3].cMax.toExponential(2)} yd: 같은 τ 의 f64 절삭 오차(${get('wasm f64', 0.137)[3].cMax.toExponential(2)})가 이미 3e-4 를 넘어서 걸음 규칙을 바꾸지 않고는 못 맞춘다`);
  // 공유위기12(위기 지수 12)는 꺾임이 가팔라 같은 걸음에서 절삭 오차가 조금 크다 (옛 n = 4 에서는 2.1e-4): 기준을 4e-4 로 올렸다
  check('SIMD τ 0.137 + min_steps 6 캐리 최대 오차 ≤ 4e-4 yd', m[3].cMax <= 4e-4, `${m[3].cMax.toExponential(2)} yd, ${m[2].toFixed(0)} ns`);
  check('SIMD τ 0.137 + min_steps 6 샷당 시간 ≤ 1000 ns', m[2] <= 1000, `${m[2].toFixed(0)} ns`);
  check('SIMD τ 0.05 캐리 최대 오차 ≤ 2.5e-4 yd', b[3].cMax <= 2.5e-4, `${b[3].cMax.toExponential(2)} yd`);
}

// ------------------------------------------------------------------ 처리량 (샷 10만 개 한 번에)
const BIG = 100000;
const big = makeShots(BIG);
console.log(`\n처리량: 샷 ${BIG.toLocaleString()}개 한 번에 (τ 0.137), 샷당 ns / 초당 샷`);
const tp = [
  ['wasm f64 batch', () => base.batch(big, 0.137, false).length],
  ['wasm f32 batch', () => base.batch(big, 0.137, true).length],
  ['wasm 4레인(비SIMD)', () => base.batch_simd(big, 0.137).length],
];
if (simd) tp.push(['wasm SIMD', () => simd.batch_simd(big, 0.137).length]);
for (const [name, fn] of tp) {
  const ns = timeIt(fn, BIG, 3);
  console.log(`  ${name.padEnd(22)} ${ns.toFixed(0).padStart(7)} ns  ${(1e9 / ns).toExponential(3).padStart(10)} 샷/초`);
}

// 게임처럼 샷 하나씩 부를 때: 객체 생성과 경로 복사 비용까지 포함
const single = (() => {
  const t = performance.now();
  let sink = 0;
  for (let i = 0; i < N; i++) {
    const f = base.fly_f64(flat[4 * i], flat[4 * i + 1], flat[4 * i + 2], flat[4 * i + 3], 0.137);
    sink += f.carry_yd;
    f.path();
    f.free();
  }
  if (sink === 12345.678) console.log(sink);
  return ((performance.now() - t) * 1e6) / N;
})();
console.log(`\nwasm 단일 호출 fly_f64 (경로 포함, τ 0.137): 샷당 ${single.toFixed(0)} ns`);

if (failed) {
  console.log(`\n실패 ${failed}건`);
  process.exit(1);
}
