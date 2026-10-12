/**
 * 비행 엔진 대 참값 (web_refs.csv: 같은 식을 dt = 2e-5 RK4 로 푼 값, scratchpad fb/src/bin/webrefs.rs 로 만듦).
 * 허용 오차: 캐리·좌우·최고 높이 0.05 yd, 착지각 0.05°, 체공 시간 0.01 s. CSV 의 축은 엔진 부호다.
 */
import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { positionAt, resample, simulate } from '../src/engine/flight.ts';
import { createSharedRe, MODEL } from '../src/engine/model.ts';

interface Ref {
  mph: number;
  deg: number;
  rpm: number;
  axis: number;
  carry: number;
  lateral: number;
  apex: number;
  land: number;
  time: number;
}

const REFS: Ref[] = readFileSync(new URL('./web_refs.csv', import.meta.url), 'utf8')
  .trim()
  .split(/\r?\n/)
  .slice(1)
  .map((line) => {
    const v = line.split(',').map(Number);
    const at = (i: number) => v[i]!;
    return { mph: at(0), deg: at(1), rpm: at(2), axis: at(3), carry: at(4), lateral: at(5), apex: at(6), land: at(7), time: at(8) };
  });

const TOL = { carry: 0.05, lateral: 0.05, apex: 0.05, land: 0.05, time: 0.01 };

describe('비행 엔진 대 web_refs.csv', () => {
  it('참값 52샷을 읽었다', () => {
    expect(REFS.length).toBe(52);
  });

  it('모든 샷이 허용 오차 안 (캐리·좌우·높이 0.05 yd, 착지각 0.05°, 시간 0.01 s)', () => {
    const max = { carry: 0, lateral: 0, apex: 0, land: 0, time: 0 };
    const fails: string[] = [];
    for (const r of REFS) {
      const o = simulate({ mph: r.mph, launchDeg: r.deg, rpm: r.rpm, axisDeg: r.axis }, { samples: false });
      expect(o.landed).toBe(true);
      const err = {
        carry: Math.abs(o.carryYd - r.carry),
        lateral: Math.abs(o.lateralYd - r.lateral),
        apex: Math.abs(o.apexYd - r.apex),
        land: Math.abs(o.landDeg - r.land),
        time: Math.abs(o.timeS - r.time),
      };
      for (const k of Object.keys(err) as (keyof typeof err)[]) {
        max[k] = Math.max(max[k], err[k]);
        if (!(err[k] <= TOL[k])) fails.push(`${r.mph}/${r.deg}/${r.rpm}/${r.axis} ${k} 오차 ${err[k].toExponential(2)}`);
      }
    }
    console.log(
      `[엔진 최대 오차] 캐리 ${max.carry.toExponential(2)} yd, 좌우 ${max.lateral.toExponential(2)} yd, ` +
        `최고 높이 ${max.apex.toExponential(2)} yd, 착지각 ${max.land.toExponential(2)}°, 시간 ${max.time.toExponential(2)} s`,
    );
    expect(fails).toEqual([]);
  });

  it('속도 (node, 샷당 µs)', () => {
    const run = (samples: boolean) => {
      for (const r of REFS) simulate({ mph: r.mph, launchDeg: r.deg, rpm: r.rpm, axisDeg: r.axis }, { samples });
    };
    for (let i = 0; i < 20; i++) run(true); // 덥히기
    const measure = (samples: boolean) => {
      let shots = 0;
      const start = performance.now();
      while (performance.now() - start < 300) {
        run(samples);
        shots += REFS.length;
      }
      return ((performance.now() - start) * 1000) / shots;
    };
    const plain = measure(false);
    const withSamples = measure(true);
    const steps = REFS.reduce((acc, r) => acc + simulate({ mph: r.mph, launchDeg: r.deg, rpm: r.rpm, axisDeg: r.axis }).steps, 0) / REFS.length;
    console.log(`[엔진 속도] 샷당 ${plain.toFixed(2)} µs (표본 없이), ${withSamples.toFixed(2)} µs (궤적 표본 포함), 평균 ${steps.toFixed(1)} τ 걸음`);
    expect(plain).toBeLessThan(1000);
  });

  it('궤적 표본: 처음은 원점, 끝은 착지점(y = 0), 시간은 늘기만 한다', () => {
    const o = simulate({ mph: 120, launchDeg: 16.3, rpm: 7097, axisDeg: 0 });
    const first = o.samples[0]!;
    const last = o.samples[o.samples.length - 1]!;
    expect(first.t).toBe(0);
    expect(Math.hypot(first.x, first.y, first.z)).toBe(0);
    expect(last.y).toBe(0);
    expect(last.t).toBeCloseTo(o.timeS, 12);
    expect(last.x / 0.9144).toBeCloseTo(o.carryYd, 9);
    for (let i = 1; i < o.samples.length; i++) expect(o.samples[i]!.t).toBeGreaterThan(o.samples[i - 1]!.t);
    // 표본 사이 에르미트 보간은 표본 점을 지나고, 최고점 근처에서 최고 높이를 넘지 않는다
    const p = { x: 0, y: 0, z: 0 };
    const mid = o.samples[10]!;
    positionAt(o.samples, mid.t, p);
    expect(p.x).toBeCloseTo(mid.x, 9);
    positionAt(o.samples, o.apex.t, p);
    expect(p.y).toBeCloseTo(o.apex.y, 2);
    const dense = resample(o.samples, 1 / 120);
    expect(dense.t[dense.t.length - 1]).toBeCloseTo(o.timeS, 12);
  });

  it('항력 위기 지수 n 은 model.ts 매개변수다 (기본 12, 바꾸면 느린 공 캐리가 바뀐다)', () => {
    const slow = { mph: 60, launchDeg: 30, rpm: 6000, axisDeg: 0 };
    const base = simulate(slow, { samples: false }).carryYd;
    expect(simulate(slow, { model: createSharedRe({ n: 12 }), samples: false }).carryYd).toBe(base);
    expect(createSharedRe({ n: 12 }).name).toBe(MODEL.name);
    const n4 = simulate(slow, { model: createSharedRe({ n: 4 }), samples: false }).carryYd;
    const n12pow = simulate(slow, { model: createSharedRe({ n: 12.000000001 }), samples: false }).carryYd;
    expect(Math.abs(n4 - base)).toBeGreaterThan(0.1);
    expect(Math.abs(n12pow - base)).toBeLessThan(1e-4);
  });
});
