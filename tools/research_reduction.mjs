/**
 * 공유위기12 축약 연구용 독립 시험. 제품 엔진을 변경하지 않는다.
 * node tools/research_reduction.mjs --out docs/research/reduction-20261013.json
 * 기준: 현재 WASM f64 및 웹 JS의 hτ=0.002. 새 적분기는 동일 장의 무풍/고정축에 한정.
 * 결과는 이 모형의 수치 오차이며, 실측 정확도/폰 성능을 의미하지 않는다.
 */
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { simulate } from '../web3d/src/engine/flight.ts';
import { MODEL } from '../web3d/src/engine/model.ts';
import { dimless, L_M, T_S, U_MS, YD } from '../web3d/src/engine/units.ts';
import { loadGolfCore } from '../golf-core/js/load.mjs';

const root = resolve(dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1')), '..');
const N = 4000;
const A = [0.618033988749895, 0.7548776662466927, 0.5698402909980532, 0.4142135623730951];
const shots = Array.from({ length: N }, (_, i) => {
  const x = (k) => (0.5 + (i + 1) * A[k]) % 1;
  return { mph: 60 + 120 * x(0), launchDeg: 5 + 30 * x(1), rpm: 1500 + 8500 * x(2), axisDeg: -30 + 60 * x(3) };
});
const edges = [
  { mph: 33, launchDeg: 6, rpm: 0, axisDeg: 0 },
  { mph: 45, launchDeg: 55, rpm: 12000, axisDeg: 45 },
  { mph: 180, launchDeg: 5, rpm: 0, axisDeg: 80 },
  { mph: 60, launchDeg: 10, rpm: 500, axisDeg: -80 },
];
const flat = Float64Array.from(shots.flatMap(s => [s.mph, s.launchDeg, s.rpm, s.axisDeg]));
const base = await loadGolfCore({ simd: false });
const simd = await loadGolfCore({ simd: true });
const ref = base.batch(flat, 0.002, false);
let sink = 0;
function timing(fn, reps = 12) {
  for (let i = 0; i < 3; i++) sink += fn()[0];
  const runs = [];
  for (let i = 0; i < 7; i++) {
    const start = performance.now();
    for (let r = 0; r < reps; r++) sink += fn()[0];
    runs.push((performance.now() - start) * 1000 / (N * reps));
  }
  runs.sort((a, b) => a - b);
  return { median_us: runs[3], min_us: runs[0], max_us: runs[6] };
}
function errors(v, truth) {
  const names = ['carry_yd', 'lateral_yd', 'apex_yd', 'land_deg'];
  const out = {};
  for (let c = 0; c < 4; c++) {
    let max = 0, sq = 0, bad = 0;
    for (let i = c; i < v.length; i += 4) {
      const d = (v[i] - truth[i]) / (c < 3 ? 0.9144 : 1);
      if (!Number.isFinite(d)) { bad++; continue; }
      max = Math.max(max, Math.abs(d)); sq += d * d;
    }
    out[names[c]] = { max, rms: Math.sqrt(sq / (v.length / 4 - bad)), nonfinite: bad };
  }
  return out;
}
const batch = [];
for (const h of [0.05, 0.137, 0.2, 0.274, 0.35, 0.45]) {
  for (const mode of ['f64', 'simd_f32']) {
    const fn = () => mode === 'f64' ? base.batch(flat, h, false) : simd.batch_simd(flat, h);
    batch.push({ mode, h_tau: h, ...timing(fn), error: errors(fn(), ref) });
  }
}

// 고정 계수의 정확한 흐름. 기저 e1=x, e2=(0,cos a,-sin a), e3=ω.
// z'=λz+f, λ=-kd+i kl。φ1=(exp(λh)-1)/λ, φ2=(φ1-h)/λ。
function frozen(s, h, kd, kl, speed, ca, sa) {
  const ed = Math.exp(-kd * h), er = ed * Math.cos(kl * h), ei = ed * Math.sin(kl * h);
  const den = kd * kd + kl * kl;
  const p1r = ((er - 1) * -kd + ei * kl) / den;
  const p1i = (ei * -kd - (er - 1) * kl) / den;
  const p2r = ((p1r - h) * -kd + p1i * kl) / den;
  const p2i = (p1i * -kd - (p1r - h) * kl) / den;
  const b1 = -Math.expm1(-kd * h) / kd, b2 = (h - b1) / kd;
  return [
    s[0] + p1r * s[3] - p1i * s[4] + p2i * ca,
    s[1] + p1i * s[3] + p1r * s[4] - p2r * ca,
    s[2] + b1 * s[5] - b2 * sa,
    er * s[3] - ei * s[4] + p1i * ca,
    ei * s[3] + er * s[4] - p1r * ca,
    ed * s[5] - b1 * sa,
    s[6] * Math.exp(-MODEL.beta * speed * h),
  ];
}
function hval(y0, y1, d0, d1, h, t) {
  const t2 = t * t, t3 = t2 * t;
  return (2*t3-3*t2+1)*y0+(t3-2*t2+t)*h*d0+(-2*t3+3*t2)*y1+(t3-t2)*h*d1;
}
function expFly(launch, h) {
  const [v, th, spin] = dimless(launch.mph, launch.launchDeg, launch.rpm);
  const a = launch.axisDeg * Math.PI / 180, ca = Math.cos(a), sa = Math.sin(a);
  let s = [0, 0, 0, v*Math.cos(th), ca*v*Math.sin(th), sa*v*Math.sin(th), spin*v];
  const coeff = { kd: 0, kl: 0 };
  let apex = 0;
  for (let k = 0; k < 10000; k++) {
    const q = Math.hypot(s[3], s[4], s[5]);
    MODEL.kk(q, s[6], coeff);
    const mid = frozen(s, h/2, coeff.kd, coeff.kl, q, ca, sa);
    const qm = Math.hypot(mid[3], mid[4], mid[5]);
    MODEL.kk(qm, mid[6], coeff);
    const n = frozen(s, h, coeff.kd, coeff.kl, qm, ca, sa);
    const y0 = ca*s[1]+sa*s[2], y1 = ca*n[1]+sa*n[2];
    const vy0 = ca*s[4]+sa*s[5], vy1 = ca*n[4]+sa*n[5];
    if (vy0 > 0 && vy1 <= 0) {
      // 에르미트 높이의 도함수 근. 이분법은 연구용 사건 검산에만 사용.
      let lo = 0, hi = 1;
      for (let j = 0; j < 40; j++) {
        const t = (lo+hi)/2, t2=t*t;
        const dy = (6*t2-6*t)*(y0-y1)+(3*t2-4*t+1)*h*vy0+(3*t2-2*t)*h*vy1;
        if (dy > 0) lo=t; else hi=t;
      }
      apex = Math.max(apex, hval(y0,y1,vy0,vy1,h,(lo+hi)/2));
    }
    if (y1 < 0 && y0 >= 0) {
      let lo = y0 === 0 ? 1e-12 : 0, hi = 1;
      for (let j=0;j<40;j++) {
        const t=(lo+hi)/2;
        if (hval(y0,y1,vy0,vy1,h,t)>0) lo=t; else hi=t;
      }
      const t=(lo+hi)/2;
      const p = [0,1,2].map(i => hval(s[i],n[i],s[i+3],n[i+3],h,t));
      // 착지 속도는 위치 에르미트의 시간 도함수로 일관되게 복원.
      const vv=[0,1,2].map(i=>((6*t*t-6*t)*(s[i]-n[i])+(3*t*t-4*t+1)*h*s[i+3]+(3*t*t-2*t)*h*n[i+3])/h);
      const vy=ca*vv[1]+sa*vv[2], vz=-sa*vv[1]+ca*vv[2];
      return { carryYd:p[0]*YD,lateralYd:(-sa*p[1]+ca*p[2])*YD,apexYd:apex*YD,
        landDeg:Math.atan2(-vy,Math.hypot(vv[0],vz))*180/Math.PI, steps:k+1 };
    }
    s=n;
  }
  return {carryYd:NaN,lateralYd:NaN,apexYd:NaN,landDeg:NaN,steps:10000};
}
const jsRef = shots.map(s => simulate(s, {h:0.002,samples:false}));
const flattenJs = results => Float64Array.from(results.flatMap(o => [o.carryYd*0.9144,o.lateralYd*0.9144,o.apexYd*0.9144,o.landDeg]));
const jsTruth=flattenJs(jsRef);
const exp = [];
for (const h of [0.02,0.05,0.1,0.15,0.2]) {
  const fn=()=>flattenJs(shots.map(s=>expFly(s,h)));
  const vals=shots.map(s=>expFly(s,h));
  exp.push({h_time_dimless:h,step_seconds:h*T_S, ...timing(fn,3),
    mean_steps:vals.reduce((a,b)=>a+b.steps,0)/N,error:errors(flattenJs(vals),jsTruth)});
}
const js=[];
for (const h of [0.05,0.137,0.274,0.35]) {
  for (const samples of [false,true]) {
    const fn=()=>flattenJs(shots.map(s=>simulate(s,{h,samples})));
    js.push({h_tau:h,samples,...timing(fn,3),error:errors(fn(),jsTruth)});
  }
}
const edge = edges.map(s=>({shot:s,reference:simulate(s,{h:0.002,samples:false}),
  rk4:simulate(s,{h:0.35,samples:false}),exp:expFly(s,0.05)})).map(({reference,rk4,...v})=>({
    ...v,reference:{...reference,samples:undefined},rk4:{...rk4,samples:undefined}}));

// 이미 사용한 공개 실측 자료에서 비교 집합을 기준 해로 한 번만 고정한다.
let observed=null;
const tmPath=resolve(root,'data/real/external/trackman_public/data.csv');
if (existsSync(tmPath)) {
  const lines=readFileSync(tmPath,'utf8').trim().split(/\r?\n/), header=lines[0].split(',');
  const ix=k=>header.indexOf(k);
  const cols=['Ball Speed','Launch Angle','Launch Direction','Spin Rate','Spin Axis','Carry Flat - Length','Carry Flat - Side','Max Height - Height','Carry Flat - Land. Angle','Last data Point - Length'];
  const rows=lines.slice(2).map(line=>{
    const v=line.split(',');return {ball:v[ix('Ball')],x:cols.map(k=>v[ix(k)]?.trim()?Number(v[ix(k)]):NaN)};
  }).filter(r=>r.x.every(Number.isFinite)&&r.x[9]>=0.9*r.x[5]&&r.x[5]>100&&['Premium','Limited Distance Soft'].includes(r.ball));
  const selected=rows.map(r=>{
    const x=r.x,s={mph:x[0],launchDeg:x[1],rpm:x[3],axisDeg:-x[4],azimuthDeg:x[2]};
    return {s,x,ref:simulate(s,{h:0.002,samples:false})};
  }).filter(r=>Math.abs(r.ref.carryYd-r.x[5])<=25);
  const methods=[];
  for(const h of [0.002,0.137,0.274,0.35,0.45]){
    const v=selected.map(r=>simulate(r.s,{h,samples:false}));
    const truth=selected.flatMap(r=>[r.x[5]*0.9144,r.x[6]*0.9144,r.x[7]/3*0.9144,r.x[8]]);
    methods.push({h_tau:h,vs_observed:errors(flattenJs(v),truth),vs_fine_model:errors(flattenJs(v),flattenJs(selected.map(r=>r.ref)))});
  }
  observed={source:'tim-blackmore/launch-monitor-regression/data.csv',sha256:createHash('sha256').update(readFileSync(tmPath)).digest('hex'),before_trim:rows.length,n:selected.length,
    selection:'기준 hτ=0.002의 캐리 잔차 |오차|≤25 yd에서 고정. 후보별 재선별 없음. 이미 모델 선택에 사용된 자료로 새 독립 holdout이 아님.',methods};
}
// 순서 효과와 WASM JIT 준비 비용을 줄인 교차 측정. 여전히 다른 프로세스 부하를 격리하지 않는다.
const perfCases=[0.137,0.274,0.35].flatMap(h=>[
  {mode:'f64',h,fn:()=>base.batch(flat,h,false)},
  {mode:'simd_f32',h,fn:()=>simd.batch_simd(flat,h)},
]);
for(let warm=0;warm<15;warm++)for(const c of perfCases)sink+=c.fn()[0];
const perfRuns=perfCases.map(()=>[]);
for(let round=0;round<11;round++){
  const order=round%2?[...perfCases.keys()].reverse():[...perfCases.keys()];
  for(const i of order){const c=perfCases[i],t=performance.now();for(let r=0;r<20;r++)sink+=c.fn()[0];perfRuns[i].push((performance.now()-t)*1000/(N*20));}
}
const pairedPerformance=perfCases.map((c,i)=>{const v=[...perfRuns[i]].sort((a,b)=>a-b);return {mode:c.mode,h_tau:c.h,median_us:v[5],min_us:v[0],max_us:v[10],runs_us:perfRuns[i]};});
let frozenSemigroupMax=0;
for(const kl of [0,0.2,1.2]){
  const s=[0.1,0.2,-0.05,2,0.4,0.1,0.25],full=frozen(s,0.1,0.3,kl,2.1,0.8,0.6);
  const half=frozen(frozen(s,0.05,0.3,kl,2.1,0.8,0.6),0.05,0.3,kl,2.1,0.8,0.6);
  frozenSemigroupMax=Math.max(frozenSemigroupMax,...full.map((v,i)=>Math.abs(v-half[i])));
}
if(frozenSemigroupMax>1e-12)throw new Error(`고정계수 흐름의 합성 검산 실패: ${frozenSemigroupMax}`);
const artifacts=['golf-core/pkg/golf_core_bg.wasm','golf-core/pkg-simd/golf_core_bg.wasm','golf-core/src/lib.rs','web3d/src/engine/flight.ts','web3d/src/engine/model.ts','tools/research_reduction.mjs'].map(p=>{
  const b=readFileSync(resolve(root,p));return {path:p,bytes:b.length,sha256:createHash('sha256').update(b).digest('hex')};
});
const result={date:'2026-10-13',runtime:process.version,platform:process.platform,arch:process.arch,n:N,
  domain:{mph:[60,180],launch_deg:[5,35],rpm:[1500,10000],axis_deg:[-30,30]},
  caveats:['시간은 Node CPU 일괄 호출 중앙값. 폰·브라우저 FPS·단일샷 API 시간이 아님.',
    'WASM과 웹 JS의 착지각 정의가 다름. 각 후보는 자기 경로의 f64 hτ=0.002에 대조.',
    '지수 적분기는 두 번 계수 평가의 2차 방법이며 event bisection/배열 할당이 포함된 연구용 구현. 생산 성능 비교가 아님.',
    '수치 오차는 고정 모형 기준이며 실측 모형 오차를 없애지 않음.'],
  artifacts,wasm_batch:batch,paired_performance:pairedPerformance,js,exponential_midpoint:exp,frozen_semigroup_max:frozenSemigroupMax,edges:edge,observed};
const at=process.argv.indexOf('--out');
if(at>=0){const p=resolve(root,process.argv[at+1]);mkdirSync(dirname(p),{recursive:true});writeFileSync(p,JSON.stringify(result,null,2)+'\n');}
console.log(JSON.stringify({n:N,wasm_batch:batch.map(v=>({mode:v.mode,h:v.h_tau,us:v.median_us,carry_max:v.error.carry_yd.max,apex_max:v.error.apex_yd.max,angle_max:v.error.land_deg.max})),
  js:js.map(v=>({h:v.h_tau,samples:v.samples,us:v.median_us,carry_max:v.error.carry_yd.max})),
  exp:exp.map(v=>({h:v.h_time_dimless,us:v.median_us,steps:v.mean_steps,carry_max:v.error.carry_yd.max,apex_max:v.error.apex_yd.max,angle_max:v.error.land_deg.max})),paired_performance:pairedPerformance,frozen_semigroup_max:frozenSemigroupMax,observed:observed&&{n:observed.n,methods:observed.methods.map(v=>({h:v.h_tau,carry_rmse:v.vs_observed.carry_yd.rms,numeric_max:v.vs_fine_model.carry_yd.max}))},sink},null,2));
