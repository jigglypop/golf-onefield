/**
 * 게임 진행 보강 시험: 아너(티 순서), 클럽 제한, 바운드 중 홀인, 빨리 감기의 단계 전환, 홀 소개 타이머, 임팩트 지연,
 * 돌풍·굴림 바람, 나무 장애물, 봇, 핸디캡·라운드 저장(저장소가 막혀도 도는지), 직전 샷 정보.
 * 지면 물리 수치는 게임용 목표라 실측 검증이 아니다.
 */
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { positionAt, type TrajectorySample } from '../src/engine/flight.ts';
import { CLUBS, DRIVER, PUTTER } from '../src/game/clubs.ts';
import { HOLES, surfaceAt, type Hole } from '../src/game/course.ts';
import { bounceAndRoll, putt, puttPowerFor, rollProfile } from '../src/game/ground.ts';
import { aimDeltaFromDrag, DRAG_DEG_PER_PX } from '../src/game/input.ts';
import { cutAtTree, firstTreeHit } from '../src/game/obstacles.ts';
import {
  advanceShot,
  aimBy,
  autoShot,
  BOT_NOISE,
  botPlan,
  botTurn,
  clubAllowed,
  flyFrom,
  game,
  handicapAllowance,
  impactDelayS,
  INTRO_S,
  isBotTurn,
  loadRounds,
  nextHole,
  pressSwing,
  ROUNDS_KEPT,
  roundSummary,
  selectClub,
  setClub,
  shotCarryYd,
  shotClock,
  skipAnimation,
  skipIntro,
  startGame,
  startHole,
  swing,
  teeOrderFor,
  type PlayerSlot,
  type PlayerSpec,
} from '../src/game/round.ts';
import { swingPose, swingTotalS } from '../src/game/swing.ts';
import { GameHud } from '../src/game/ui/GameHud.tsx';
import { drawPanel, panelModel, type Ctx2D } from '../src/game/ui/panelModel.ts';

function flatHole(kind: 'fairway' | 'green' | 'rough', pin: [number, number] = [5000, 0], slope: [number, number] = [0, 0]): Hole {
  return {
    number: 0, name: '시험', par: 4, tee: [-500, 0], teeTarget: [-400, 0], pin,
    fairways: kind === 'fairway' ? [{ path: [[-900, 0], [1900, 0]], width: 4000 }] : [],
    green: kind === 'green' ? { c: [0, 0], rx: 3000, rz: 3000, rot: 0, slope } : { c: [9000, 9000], rx: 1, rz: 1, rot: 0, slope },
    bunkers: [], water: [], bounds: [[-1000, -1000], [2000, -1000], [2000, 1000], [-1000, 1000]],
  };
}

/** 봇·사람 가리지 않고 한 사람 한 타씩 끝까지 */
function playOut(limit = 2500): void {
  for (let i = 0; i < limit && game.get().phase !== 'done'; i++) {
    const s = game.get();
    if (s.phase === 'intro') skipIntro();
    else if (s.phase === 'holed') nextHole();
    else if (s.phase === 'aim') {
      if (isBotTurn(s)) botTurn();
      else autoShot();
      skipAnimation();
    } else break;
  }
}

/** 스트로크 게임에서 1번 홀이 끝난 상태를 만든다 (점수를 직접 적는다) */
function finishHoleWith(pos: number, scores: number[]): void {
  const s = game.get();
  game.set({
    players: s.players.map((p, i) => ({ ...p, done: true, scores: p.scores.map((v, k) => (k === pos ? scores[i]! : v)) })),
    scores: s.scores.map((v, k) => (k === pos ? scores[s.cur]! : v)),
    phase: 'holed',
  });
}

// ------------------------------------------------------------------ G1 아너

describe('G1 티 순서 (아너)', () => {
  it('앞 홀 점수가 좋은 사람부터, 동점이면 앞 홀의 순서를 유지하고, 홀마다 다시 정한다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가', '나', '다', '라'], intro: false, seed: 11 });
    expect(game.get().teeOrder).toEqual([0, 1, 2, 3]);
    // 1번 홀: 가 5, 나 3, 다 4, 라 3 → 나·라(동점, 앞 순서 나 먼저), 다, 가
    finishHoleWith(0, [5, 3, 4, 3]);
    nextHole();
    expect(game.get().holePos).toBe(1);
    expect(game.get().teeOrder).toEqual([1, 3, 2, 0]);
    expect(game.get().cur).toBe(1);
    skipIntro();
    // 티샷 네 번: 인덱스 순서(0,1,2,3)가 아니라 아너 순서 → 1, 3, 2, 0
    const order: number[] = [];
    for (let k = 0; k < 4; k++) {
      order.push(game.get().cur);
      autoShot();
      skipAnimation();
    }
    expect(order).toEqual([1, 3, 2, 0]);
    // 2번 홀: 네 명이 모두 4 타 (전원 동점) → 3번 홀 순서는 2번 홀 순서 그대로
    finishHoleWith(1, [4, 4, 4, 4]);
    nextHole();
    expect(game.get().holePos).toBe(2);
    expect(game.get().teeOrder).toEqual([1, 3, 2, 0]);
    expect(game.get().cur).toBe(1);
  });

  it('teeOrderFor: 첫 홀은 번호 순, 점수가 없는 사람은 맨 뒤', () => {
    const mk = (scores: (number | null)[]): PlayerSlot => ({ name: 'x', color: '#fff', ball: { x: 0, z: 0, surface: 'tee' }, strokes: 0, done: false, scores, shots: [] });
    const ps = [mk([4, null]), mk([3, null]), mk([null, null])];
    expect(teeOrderFor(ps, -1, [2, 1, 0])).toEqual([0, 1, 2]);
    expect(teeOrderFor(ps, 0, [0, 1, 2])).toEqual([1, 0, 2]);
  });
});

// ------------------------------------------------------------------ G2 클럽 제한

describe('G2 클럽 제한은 store 에서도 지킨다', () => {
  it('티 밖에서는 드라이버를 못 고르고, 선택 이동은 건너뛴다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 3 });
    expect(clubAllowed(DRIVER)).toBe(true);
    game.set({ ball: { x: 120, z: 0, surface: 'fairway' }, clubIdx: 3 });
    expect(clubAllowed(DRIVER)).toBe(false);
    setClub(DRIVER);
    expect(game.get().clubIdx).toBe(3);
    expect(game.get().message?.text).toContain('쓸 수 없');
    selectClub(-1);
    expect(game.get().clubIdx).toBe(2);
    selectClub(-1);
    expect(game.get().clubIdx).toBe(1);
    selectClub(-1); // 위에는 드라이버뿐이고 막혀 있다
    expect(game.get().clubIdx).toBe(1);
    // 티로 돌아오면 다시 된다
    game.set({ ball: { x: 0, z: 0, surface: 'tee' } });
    setClub(DRIVER);
    expect(game.get().clubIdx).toBe(DRIVER);
  });

  it('막힌 클럽으로는 칠 수 없다 (콘솔로 clubIdx 를 바꿔도)', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 3 });
    game.set({ ball: { x: 100, z: 0, surface: 'rough' }, clubIdx: DRIVER });
    expect(swing({ power: 1, accuracy: 0 })).toBeNull();
    expect(game.get().phase).toBe('aim');
    expect(game.get().strokes).toBe(0);
  });

  it('롱드라이브는 드라이버만', () => {
    startGame({ mode: 'longdrive', players: ['가'], intro: false, seed: 3 });
    for (let i = 1; i <= PUTTER; i++) expect(clubAllowed(i)).toBe(false);
    setClub(5);
    expect(game.get().clubIdx).toBe(DRIVER);
    selectClub(1);
    expect(game.get().clubIdx).toBe(DRIVER);
  });
});

// ------------------------------------------------------------------ G3 바운드 중 홀인

describe('G3 바운드 중 컵에 들어간다', () => {
  const land = { vx: 1.5, vy: -5, vz: 0, backspinRpm: 0 };

  it('첫 바운드가 떨어지는 자리에 컵이 있으면 두 번째 착지에서 들어간다', () => {
    const probe = bounceAndRoll(flatHole('green', [5000, 0]), { x: 0, z: 0, ...land });
    expect(probe.hits.length).toBeGreaterThanOrEqual(1);
    const h1 = probe.hits[0]!;
    const at = probe.samples.find((s) => s.y === 0 && Math.abs(s.t - h1.t) < 1e-9)!;
    expect(at.x).toBeGreaterThan(0.05);
    const g = bounceAndRoll(flatHole('green', [at.x, at.z]), { x: 0, z: 0, ...land });
    expect(g.event).toBe('holed');
    expect(g.bounces).toBe(1);
    expect(g.rest.x).toBeCloseTo(at.x, 9);
    // 컵으로 떨어지는 짧은 표본 (y < 0) 이 이어진다
    expect(g.samples[g.samples.length - 1]!.y).toBeLessThan(0);
  });

  it('너무 빠르게 떨어지면 들어가지 않는다', () => {
    const fast = { vx: 12, vy: -5, vz: 0, backspinRpm: 0 };
    const probe = bounceAndRoll(flatHole('green', [5000, 0]), { x: 0, z: 0, ...fast });
    const h1 = probe.hits[0]!;
    const at = probe.samples.find((s) => s.y === 0 && Math.abs(s.t - h1.t) < 1e-9)!;
    const g = bounceAndRoll(flatHole('green', [at.x, at.z]), { x: 0, z: 0, ...fast });
    expect(g.event).not.toBe('holed');
  });

  it('컵 위에 곧바로 떨어져도 (홀인원) 느리면 들어가고, 컵 밖이면 안 들어간다', () => {
    const h = flatHole('green', [10, 0]);
    expect(bounceAndRoll(h, { x: 10.01, z: 0, vx: 1, vy: -8, vz: 0, backspinRpm: 0 }).event).toBe('holed');
    expect(bounceAndRoll(h, { x: 10.3, z: 0, vx: 0.2, vy: -8, vz: 0, backspinRpm: 0 }).event).toBe('rest');
  });

  it('바운드 기록(hits)과 굴림 곡선: 바운드마다 시각·수직 속도가 있고, 굴림 속력은 줄어든다', () => {
    const g = bounceAndRoll(flatHole('fairway'), { x: 0, z: 0, vx: 20, vy: -25, vz: 0, backspinRpm: 3000 });
    expect(g.hits.length).toBeGreaterThanOrEqual(1);
    for (let i = 1; i < g.hits.length; i++) expect(g.hits[i]!.t).toBeGreaterThan(g.hits[i - 1]!.t);
    expect(g.rollStart).toBeGreaterThanOrEqual(g.hits[g.hits.length - 1]!.t - 1e-9);
    const prof = rollProfile(g, 12);
    expect(prof.length).toBeGreaterThan(3);
    expect(prof.length).toBeLessThanOrEqual(12);
    expect(prof[0]!.v).toBeGreaterThan(prof[prof.length - 1]!.v);
    expect(prof.every((p) => p.t >= g.rollStart - 1e-9)).toBe(true);
  });
});

// ------------------------------------------------------------------ G4 빨리 감기

describe('G4 빨리 감기도 비행 → 지면 전환을 거친다', () => {
  it('skipAnimation 은 flight → ground → 다음 단계를 모두 지나며 구독자에게 알린다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 5 });
    const seen: string[] = [];
    const unsub = game.subscribe(() => {
      const p = game.get().phase;
      if (seen[seen.length - 1] !== p) seen.push(p);
    });
    const shot = swing({ power: 0.8, accuracy: 0 });
    expect(shot).not.toBeNull();
    skipAnimation();
    unsub();
    expect(seen.slice(0, 2)).toEqual(['flight', 'ground']);
    expect(['aim', 'holed']).toContain(seen[seen.length - 1]);
  });

  it('스윙 중(임팩트 전)에 빨리 감아도 끝까지 간다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 5 });
    const shot = swing({ power: 0.8, accuracy: 0 })!;
    expect(shotClock.t).toBeLessThan(0);
    skipAnimation();
    expect(game.get().phase).not.toBe('flight');
    expect(game.get().strokes).toBeGreaterThanOrEqual(1);
    expect(shotClock.t).toBeCloseTo(shot.tTotal, 9);
  });
});

// ------------------------------------------------------------------ G5 홀 소개 타이머

describe('G5 홀 소개는 프레임 고리 없이도 시간으로 끝난다', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance', 'Date'] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it(`${INTRO_S} s 뒤 조준으로 (introTick 을 부르는 rAF 가 없어도)`, () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: true, seed: 1 });
    expect(game.get().phase).toBe('intro');
    vi.advanceTimersByTime(INTRO_S * 1000 - 600);
    expect(game.get().phase).toBe('intro');
    vi.advanceTimersByTime(800);
    expect(game.get().phase).toBe('aim');
  });

  it('Space 로 건너뛰면 타이머가 풀리고, 다음 홀 소개는 다시 건다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: true, seed: 1 });
    pressSwing();
    expect(game.get().phase).toBe('aim');
    vi.advanceTimersByTime(INTRO_S * 1000 + 500);
    expect(game.get().phase).toBe('aim');
    startHole(1, true);
    expect(game.get().phase).toBe('intro');
    vi.advanceTimersByTime(INTRO_S * 1000 + 100);
    expect(game.get().phase).toBe('aim');
  });
});

// ------------------------------------------------------------------ S1 임팩트

describe('S1 공은 임팩트에 떠난다', () => {
  it('샷을 정한 뒤 0.44 s 동안 공은 제자리이고, 시계가 0 을 지나면 움직인다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 8 });
    const origin = { ...game.get().ball };
    const shot = swing({ power: 1, accuracy: 0 })!;
    expect(shot.impactDelay).toBeCloseTo(impactDelayS(false), 12);
    expect(shot.impactDelay).toBeCloseTo(0.44, 9);
    expect(shotClock.t).toBeCloseTo(-0.44, 9);
    const p = { x: 0, y: 0, z: 0 };
    positionAt(shot.flightSamples, shotClock.t, p);
    expect(p).toEqual({ x: origin.x, y: 0, z: origin.z });
    for (let i = 0; i < 4; i++) advanceShot(0.1); // −0.04
    expect(shotClock.t).toBeLessThan(0);
    positionAt(shot.flightSamples, shotClock.t, p);
    expect(p.x).toBe(origin.x);
    expect(game.get().phase).toBe('flight');
    advanceShot(0.1); // +0.06
    expect(shotClock.t).toBeGreaterThan(0);
    positionAt(shot.flightSamples, shotClock.t, p);
    expect(Math.hypot(p.x - origin.x, p.z - origin.z)).toBeGreaterThan(1);
    // 비행·굴림 전체 길이는 임팩트 뒤부터 센다
    expect(shot.tFlight).toBeCloseTo(shot.flight!.timeS, 9);
  });

  it('퍼트도 같은 규칙: 임팩트까지 제자리', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 8 });
    game.set({ ball: { x: 330, z: 60, surface: 'green' }, clubIdx: PUTTER });
    const shot = swing({ power: 0.3, accuracy: 0 })!;
    expect(shot.putt).toBe(true);
    expect(shot.impactDelay).toBeCloseTo(impactDelayS(true), 12);
    expect(shotClock.t).toBeCloseTo(-shot.impactDelay, 9);
    expect(game.get().phase).toBe('ground');
  });

  it('스윙 자세: 백스윙(−) → 임팩트에서 정확히 0 → 팔로스루(+), 끊김 없이 이어진다', () => {
    const imp = impactDelayS(false);
    expect(swingPose(-0.1, 1, false).phi).toBe(0);
    expect(swingPose(imp, 1, false).phi).toBeCloseTo(0, 12);
    expect(swingPose(imp - 1e-9, 1, false).phi).toBeCloseTo(0, 6);
    let min = 0;
    let max = 0;
    let prev = 0;
    for (let t = 0; t <= swingTotalS(false) + 0.2; t += 0.002) {
      const { phi } = swingPose(t, 1, false);
      min = Math.min(min, phi);
      max = Math.max(max, phi);
      expect(Math.abs(phi - prev)).toBeLessThan(0.2); // 2 ms 에 0.2 rad 넘게 뛰지 않는다
      prev = phi;
    }
    expect(min).toBeLessThan(-2);
    expect(max).toBeGreaterThan(1.5);
    expect(swingTotalS(false)).toBeCloseTo(0.64, 9);
    // 퍼터는 작게
    const pmin = Math.min(...Array.from({ length: 100 }, (_, i) => swingPose(i * 0.005, 1, true).phi));
    expect(pmin).toBeGreaterThan(-0.6);
    // 파워가 작으면 백스윙도 작다
    expect(swingPose(0.3, 0.2, false).phi).toBeGreaterThan(swingPose(0.3, 1, false).phi);
  });
});

// ------------------------------------------------------------------ D1 돌풍·굴림 바람

describe('D1 돌풍과 굴림 바람', () => {
  it('flyFrom: gustSeed 가 없으면 평균 바람만 (미리보기), 있으면 돌풍이 더해지고 결정적이다', () => {
    const input = { mph: 120, launchDeg: 16.3, azimuthDeg: 0, rpm: 7097, spinAxisDeg: 0 };
    const w = { speed: 5, dir: 1, gust: 2 };
    const a = flyFrom(input, { x: 0, z: 0 }, 0, w);
    const b = flyFrom(input, { x: 0, z: 0 }, 0, w, 11);
    const b2 = flyFrom(input, { x: 0, z: 0 }, 0, w, 11);
    const c = flyFrom(input, { x: 0, z: 0 }, 0, { ...w, gust: 0 }, 11);
    expect(c.result.carryYd).toBe(a.result.carryYd);
    expect(b2.result.carryYd).toBe(b.result.carryYd);
    expect(Math.hypot(b.result.carryYd - a.result.carryYd, b.result.lateralYd - a.result.lateralYd)).toBeGreaterThan(0.2);
  });

  it('한 홀의 바람에는 돌풍 진폭이 붙고, 같은 씨앗·같은 샷이면 같은 결과다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 21 });
    expect(game.get().wind.gust).toBeGreaterThan(0);
    const s0 = game.get();
    const shot = swing({ power: 1, accuracy: 0 })!;
    expect(shot.gust).not.toBeNull();
    const landing = shot.flightSamples[shot.flightSamples.length - 1]!;
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 21 });
    expect(game.get().wind).toEqual(s0.wind);
    const shot2 = swing({ power: 1, accuracy: 0 })!;
    const landing2 = shot2.flightSamples[shot2.flightSamples.length - 1]!;
    expect(landing2.x).toBe(landing.x);
    expect(landing2.z).toBe(landing.z);
  });

  it('굴림: 순풍이면 더 구르고 맞바람이면 덜 구른다, 바람 없음과 wind 생략은 같다', () => {
    const land = { x: 0, z: 0, vx: 8, vy: -8, vz: 0, backspinRpm: 0 };
    const h = flatHole('green');
    const none = bounceAndRoll(h, land);
    const same = bounceAndRoll(h, land, { x: 0, z: 0 });
    const tail = bounceAndRoll(h, land, { x: 6, z: 0 });
    const head = bounceAndRoll(h, land, { x: -6, z: 0 });
    expect(same.runM).toBe(none.runM);
    expect(tail.runM).toBeGreaterThan(none.runM);
    expect(head.runM).toBeLessThan(none.runM);
    // 옆바람은 옆으로 민다
    const cross = bounceAndRoll(h, land, { x: 0, z: 6 });
    expect(cross.rest.z).toBeGreaterThan(0.005);
  });

  it('퍼트에도 바람 (작게): 그린 스팀프 10 의 3.05 m 퍼트는 바람 없을 때 그대로', () => {
    const h = flatHole('green');
    expect(putt(h, 0, 0, 0, puttPowerFor(3.05)).runM).toBeCloseTo(3.05, 2);
    const tail = putt(h, 0, 0, 0, puttPowerFor(3.05), undefined, { x: 5, z: 0 }).runM;
    const head = putt(h, 0, 0, 0, puttPowerFor(3.05), undefined, { x: -5, z: 0 }).runM;
    expect(tail).toBeGreaterThan(3.05);
    expect(head).toBeLessThan(3.05);
    expect(tail - head).toBeLessThan(0.6); // 5 m/s 에서도 수십 cm 안
  });

  it('바람이 멎은 공을 밀지는 못한다 (정지 판정에 바람 가속이 들어간다)', () => {
    const r = bounceAndRoll(flatHole('rough'), { x: 0, z: 0, vx: 0, vy: -3, vz: 0, backspinRpm: 0 }, { x: 8, z: 0 });
    expect(r.event).toBe('rest');
    expect(r.runM).toBeLessThan(0.5);
  });
});

// ------------------------------------------------------------------ D5 나무

describe('D5 나무 장애물', () => {
  /** 일정한 높이 y0 로 +x 로 가는 가짜 비행 */
  const line = (y0: number, y1 = y0): TrajectorySample[] =>
    Array.from({ length: 21 }, (_, i) => ({ t: i * 0.5, x: 10 * i, y: y0 + ((y1 - y0) * i) / 20, z: 0, vx: 20, vy: (y1 - y0) / 10, vz: 0 }));

  it('꼭대기보다 낮게 원기둥을 지나면 들어가는 자리에서 멈춘다', () => {
    const hit = firstTreeHit([{ x: 100, z: 0.5, r: 2, h: 10 }], line(5));
    expect(hit).not.toBeNull();
    expect(hit!.x).toBeCloseTo(100 - Math.sqrt(4 - 0.25), 1);
    expect(hit!.y).toBeCloseTo(5, 1);
    expect(hit!.t).toBeCloseTo(hit!.x / 20, 1);
  });

  it('꼭대기 위로 지나가거나 옆으로 비껴가면 맞지 않는다', () => {
    expect(firstTreeHit([{ x: 100, z: 0, r: 2, h: 10 }], line(12))).toBeNull();
    expect(firstTreeHit([{ x: 100, z: 2.5, r: 2, h: 10 }], line(5))).toBeNull();
    expect(firstTreeHit(undefined, line(5))).toBeNull();
    expect(firstTreeHit([], line(5))).toBeNull();
  });

  it('위에서 내려오다 원기둥 안에서 꼭대기 높이를 지나면 그 높이에서 맞는다', () => {
    // 높이 20 → 0 으로 내려가는 선: x = 100 의 나무 (반지름 2) 안에서 y 가 h = 10 보다 낮아진다
    const samples: TrajectorySample[] = Array.from({ length: 21 }, (_, i) => ({ t: i * 0.5, x: 10 * i, y: 20 - i, z: 0, vx: 20, vy: -2, vz: 0 }));
    const hit = firstTreeHit([{ x: 150, z: 0, r: 3, h: 10 }], samples);
    expect(hit).not.toBeNull();
    // x = 147–153 에서 y = 20 − x/10 = 5.3–... 이미 낮다: 원기둥 입구(147)에서 y = 5.3 < 10 이므로 입구에서 맞는다
    expect(hit!.x).toBeCloseTo(147, 0);
    const high = firstTreeHit([{ x: 50, z: 0, r: 3, h: 10 }], samples);
    // x = 47–53 에서 y = 15.3–14.7 이라 꼭대기(10) 위: 맞지 않는다
    expect(high).toBeNull();
  });

  it('처음부터 원기둥 안에 있던 공은 그 나무를 벗어나기 전까지 맞지 않는다', () => {
    expect(firstTreeHit([{ x: 1, z: 0, r: 5, h: 10 }], line(2))).toBeNull();
    // 벗어났다가 다른 쪽에서 다시 들어오면 맞는다 (다른 나무)
    expect(firstTreeHit([{ x: 1, z: 0, r: 5, h: 10 }, { x: 100, z: 0, r: 2, h: 10 }], line(2))!.index).toBe(1);
  });

  it('cutAtTree: 맞은 곳에서 끊고 수직으로 떨어뜨린다 (끝은 y = 0, 속도는 중력 낙하)', () => {
    const hit = firstTreeHit([{ x: 100, z: 0, r: 2, h: 10 }], line(8))!;
    const cut = cutAtTree(line(8), hit);
    const end = cut[cut.length - 1]!;
    expect(end.y).toBe(0);
    expect(end.x).toBeCloseTo(hit.x, 9);
    expect(end.vx).toBe(0);
    const fall = Math.sqrt((2 * hit.y) / 9.81);
    expect(end.t).toBeCloseTo(hit.t + fall, 9);
    expect(end.vy).toBeCloseTo(-9.81 * fall, 6);
    for (let i = 1; i < cut.length; i++) expect(cut[i]!.t).toBeGreaterThan(cut[i - 1]!.t);
    expect(cut.every((s) => s.y >= 0)).toBe(true);
  });

  it('1번 홀 나무: 낮은 실측 샷이 나무에 맞고, 높이 넘기는 샷은 맞지 않는다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 2 });
    const tree = HOLES[0]!.trees![0]!;
    // 나무 62 m 앞에서 나무 쪽으로 낮게 (발사각 8°: 나무까지 약 9 m 높이 < 15 m)
    game.set({ ball: { x: tree.x - 62, z: tree.z, surface: 'fairway' }, aimYaw: 0, wind: { speed: 0, dir: 0 } });
    const low = swing({ power: 1, accuracy: 0 }, { mph: 100, launchDeg: 8, rpm: 3000, azimuthDeg: 0, spinAxisDeg: 0 })!;
    expect(low.tree).not.toBeNull();
    expect(Math.hypot(low.tree!.x - tree.x, low.tree!.z - tree.z)).toBeLessThan(tree.r + 0.05);
    expect(low.tree!.y).toBeLessThan(tree.h);
    expect(low.tFlight).toBeGreaterThan(low.tree!.t);
    expect(low.tFlight).toBeLessThan(low.flight!.timeS);
    expect(low.ground.samples[0]!.x).toBeCloseTo(low.tree!.x, 6);
    expect(shotCarryYd(low)).toBeLessThan(75);
    skipAnimation();
    expect(game.get().message?.text).toContain('나무');
    expect(game.get().ball.x).toBeLessThan(tree.x + tree.r + 1);
    // 땅은 평평하다: 지면 표본은 모두 y ≥ −0.07 (컵 표본만 음수) 이고 최대 바운드 높이는 몇 m 안
    expect(low.ground.samples.every((s) => s.y > -0.07 && s.y < 3)).toBe(true);
    // 같은 자리에서 높이 넘기는 샷 (발사각 40°)
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 2 });
    game.set({ ball: { x: tree.x - 62, z: tree.z, surface: 'fairway' }, aimYaw: 0, wind: { speed: 0, dir: 0 } });
    const high = swing({ power: 1, accuracy: 0 }, { mph: 100, launchDeg: 40, rpm: 3000, azimuthDeg: 0, spinAxisDeg: 0 })!;
    expect(high.tree).toBeNull();
  });

  it('나무가 없는 홀은 비행이 그대로다 (tree = null, tFlight = 체공 시간)', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 2 });
    startHole(1, false);
    const shot = swing({ power: 0.6, accuracy: 0 })!;
    expect(shot.tree).toBeNull();
    expect(shot.tFlight).toBe(shot.flight!.timeS);
  });

  it('땅 판정은 나무와 무관하다', () => {
    const t = HOLES[0]!.trees![0]!;
    expect(surfaceAt(HOLES[0]!, t.x, t.z)).toBe('fairway');
  });
});

// ------------------------------------------------------------------ D2 봇

describe('D2 봇', () => {
  const specs: PlayerSpec[] = [
    { name: '봇 가', bot: true, level: 'normal' },
    { name: '봇 나', bot: true, level: 'hard' },
  ];

  it('봇 두 명이 9홀을 끝까지 돌고 (홀마다 더블 파 이하), 같은 씨앗이면 점수가 똑같다', () => {
    const run = () => {
      startGame({ mode: 'stroke', holes: 9, players: specs, intro: false, seed: 2024 });
      playOut();
      const s = game.get();
      expect(s.phase).toBe('done');
      return s.players.map((p, i) => (i === s.cur ? s.scores : p.scores));
    };
    const a = run();
    const b = run();
    expect(b).toEqual(a);
    for (const scores of a) {
      expect(scores.length).toBe(9);
      scores.forEach((v, i) => {
        expect(v).not.toBeNull();
        expect(v!).toBeLessThanOrEqual(2 * HOLES[i]!.par);
        expect(v!).toBeGreaterThanOrEqual(1);
      });
    }
    console.log(`[봇 9홀] ${a.map((s) => s.join(',')).join(' | ')} (파 ${HOLES.map((h) => h.par).join(',')})`);
  });

  it('봇 차례에는 사람 입력이 막힌다', () => {
    startGame({ mode: 'stroke', holes: 3, players: specs, intro: false, seed: 5 });
    expect(isBotTurn()).toBe(true);
    const yaw = game.get().aimYaw;
    aimBy(0.1);
    setClub(3);
    selectClub(1);
    pressSwing();
    expect(game.get().aimYaw).toBe(yaw);
    expect(game.get().phase).toBe('aim');
    expect(swing({ power: 1, accuracy: 0 })).toBeNull();
    expect(botTurn()).not.toBeNull();
    expect(game.get().phase).toBe('flight');
  });

  it('사람과 봇이 섞이면 사람 차례에는 botTurn 이 아무것도 안 한다', () => {
    startGame({ mode: 'stroke', holes: 3, players: [{ name: '사람' }, { name: '봇', bot: true }], intro: false, seed: 5 });
    expect(isBotTurn()).toBe(false);
    expect(botTurn()).toBeNull();
    expect(game.get().phase).toBe('aim');
  });

  it('난이도가 높을수록 흔들림이 작다 (같은 상태에서 샷 번호만 바꿔 평균 비교)', () => {
    const avg = (level: 'easy' | 'normal' | 'hard') => {
      startGame({ mode: 'stroke', holes: 3, players: [{ name: 'b', bot: true, level }], intro: false, seed: 4 });
      let acc = 0;
      let pw = 0;
      const base = botPlan(game.get());
      for (let k = 0; k < 300; k++) {
        game.set({ shotId: k });
        const p = botPlan(game.get());
        acc += Math.abs(p.swing.accuracy);
        pw += Math.abs(p.swing.power - base.swing.power);
      }
      return { acc: acc / 300, pw: pw / 300 };
    };
    const easy = avg('easy');
    const normal = avg('normal');
    const hard = avg('hard');
    expect(easy.acc).toBeGreaterThan(normal.acc);
    expect(normal.acc).toBeGreaterThan(hard.acc);
    expect(BOT_NOISE.easy.aim).toBeGreaterThan(BOT_NOISE.hard.aim);
  });

  it('봇 계획: 클럽은 거리로 고르고, 그린에서는 퍼터, 티샷은 풀파워', () => {
    startGame({ mode: 'stroke', holes: 3, players: [{ name: 'b', bot: true, level: 'hard' }], intro: false, seed: 4 });
    const tee = botPlan(game.get());
    expect(tee.clubIdx).toBe(DRIVER);
    expect(tee.swing.power).toBeGreaterThan(0.9);
    game.set({ ball: { x: 330, z: 60, surface: 'green' } });
    expect(CLUBS[botPlan(game.get()).clubIdx]!.putter).toBe(true);
    game.set({ ball: { x: 250, z: 30, surface: 'fairway' } });
    const approach = botPlan(game.get());
    expect(approach.clubIdx).not.toBe(DRIVER);
    expect(approach.clubIdx).not.toBe(PUTTER);
  });
});

// ------------------------------------------------------------------ D3 핸디캡·저장

describe('D3 핸디캡과 라운드 저장', () => {
  /** 메모리 저장소 */
  function memoryStorage() {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v), removeItem: (k: string) => void m.delete(k), m };
  }

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('허용 타수 = round(핸디캡 × 홀 수 / 18), 순타수는 총타수에서 뺀다', () => {
    expect(handicapAllowance(18, 9)).toBe(9);
    expect(handicapAllowance(10, 9)).toBe(5);
    expect(handicapAllowance(10, 3)).toBe(2);
    expect(handicapAllowance(0, 9)).toBe(0);
    expect(handicapAllowance(-3, 9)).toBe(0);
    startGame({ mode: 'stroke', holes: 3, players: [{ name: '가', handicap: 18 }, { name: '나' }], intro: false, seed: 3 });
    expect(game.get().players[0]!.handicap).toBe(18);
    expect(game.get().players[1]!.handicap).toBeUndefined();
    game.set({ players: game.get().players.map((p, i) => ({ ...p, scores: i === 0 ? [5, 4, 6] : [4, 4, 5] })), scores: [5, 4, 6], phase: 'holed' });
    const r = roundSummary(game.get(), '2026-01-01T00:00:00.000Z');
    expect(r.players[0]).toMatchObject({ name: '가', handicap: 18, gross: 15, net: 12 });
    expect(r.players[1]).toMatchObject({ name: '나', handicap: 0, gross: 13, net: 13 });
    expect(r.par).toBe(12);
  });

  it('끝난 라운드를 저장소에 남기고 (최근 N 개), 불러온다', () => {
    const st = memoryStorage();
    vi.stubGlobal('localStorage', st);
    startGame({ mode: 'stroke', holes: 3, players: [{ name: '봇', bot: true, handicap: 12 }], intro: false, seed: 31 });
    playOut();
    expect(game.get().phase).toBe('done');
    const rounds = loadRounds();
    expect(rounds.length).toBe(1);
    expect(rounds[0]!.mode).toBe('stroke');
    expect(rounds[0]!.holes).toBe(3);
    expect(rounds[0]!.players[0]).toMatchObject({ name: '봇', bot: true, handicap: 12 });
    expect(rounds[0]!.players[0]!.net).toBe(rounds[0]!.players[0]!.gross - handicapAllowance(12, 3));
    // 봇만 있으면 신기록 알림은 없다
    expect(game.get().record).toBeNull();
    // 상한
    for (let i = 0; i < ROUNDS_KEPT + 3; i++) {
      startGame({ mode: 'stroke', holes: 3, players: [{ name: '봇', bot: true }], intro: false, seed: 100 + i });
      playOut();
    }
    expect(loadRounds().length).toBe(ROUNDS_KEPT);
  });

  it('사람이 이기면 신기록이 저장되고, 챌린지 라운드도 남는다', () => {
    const st = memoryStorage();
    vi.stubGlobal('localStorage', st);
    startGame({ mode: 'ctp', players: ['가'], intro: false, seed: 9 });
    playOut();
    expect(game.get().phase).toBe('done');
    expect(loadRounds()[0]!.mode).toBe('ctp');
    expect(loadRounds()[0]!.players[0]!.shots.length).toBe(3);
  });

  it('저장소가 막혀 있어도 게임은 끝까지 돈다', () => {
    const boom = () => {
      throw new Error('storage blocked');
    };
    vi.stubGlobal('localStorage', { getItem: boom, setItem: boom, removeItem: boom });
    startGame({ mode: 'stroke', holes: 3, players: [{ name: '가', handicap: 5 }], intro: false, seed: 31 });
    expect(() => playOut()).not.toThrow();
    expect(game.get().phase).toBe('done');
    expect(loadRounds()).toEqual([]);
  });
});

// ------------------------------------------------------------------ D6 직전 샷 정보 · HUD 그리기

describe('D6 직전 샷 정보', () => {
  it('shotCarryYd 는 비행 결과의 캐리·좌우로 잰 거리와 같다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 6 });
    const shot = swing({ power: 0.9, accuracy: 0.2 })!;
    expect(shotCarryYd(shot)).toBeCloseTo(Math.hypot(shot.flight!.carryYd, shot.flight!.lateralYd), 1);
    expect(Number.isNaN(shotCarryYd({ ...shot, putt: true }))).toBe(true);
  });

  it('HUD 정보 카드에 발사각·스핀·캐리가 나온다 (서버 렌더)', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 6 });
    const before = renderToString(createElement(GameHud, { onRange: () => {} }));
    expect(before).not.toContain('g-last-carry');
    const shot = swing({ power: 1, accuracy: 0 })!;
    skipAnimation();
    const html = renderToString(createElement(GameHud, { onRange: () => {} }));
    expect(html).toContain('data-testid="g-last-launch"');
    expect(html).toContain(`${shot.input!.launchDeg.toFixed(1)}°`);
    expect(html).toContain(`${Math.round(shot.input!.rpm)} rpm`);
    expect(html).toContain(`${Math.round(shotCarryYd(shot))} yd`);
  });

  it('메뉴에 봇·핸디캡 입력이 있다', () => {
    game.set({ phase: 'menu' });
    const html = renderToString(createElement(GameHud, { onRange: () => {} }));
    expect(html).toContain('aria-label="플레이어 1 봇"');
    expect(html).toContain('aria-label="플레이어 1 핸디캡"');
  });
});

// ------------------------------------------------------------------ I1 입력 · I3 패널

describe('I1 끌어서 조준', () => {
  it('오른쪽으로 끌면 + (오른쪽), 퍼터는 더 곱게', () => {
    expect(aimDeltaFromDrag(100, false)).toBeCloseTo((100 * DRAG_DEG_PER_PX.full * Math.PI) / 180, 12);
    expect(aimDeltaFromDrag(-50, false)).toBeLessThan(0);
    expect(Math.abs(aimDeltaFromDrag(100, true))).toBeLessThan(Math.abs(aimDeltaFromDrag(100, false)));
    expect(aimDeltaFromDrag(0, false)).toBe(0);
  });

  it('조준 단계에서만 돌아간다 (aimBy 규칙)', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 1 });
    const y0 = game.get().aimYaw;
    aimBy(aimDeltaFromDrag(100, false));
    expect(game.get().aimYaw).toBeCloseTo(y0 + aimDeltaFromDrag(100, false), 12);
    pressSwing(); // swing 단계
    const y1 = game.get().aimYaw;
    aimBy(0.3);
    expect(game.get().aimYaw).toBe(y1);
  });
});

describe('I3 월드 공간 패널 (XR)', () => {
  it('상태에서 제목·직전 메시지·직전 샷 수치를 만들고 캔버스에 그린다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 6 });
    swing({ power: 1, accuracy: 0 });
    skipAnimation();
    const model = panelModel(game.get(), 0);
    expect(model.title).toContain('1번 홀');
    expect(model.headline.length).toBeGreaterThan(0);
    expect(model.lines.some((l) => l.includes('직전 샷') && l.includes('캐리'))).toBe(true);
    const texts: string[] = [];
    const fake: Ctx2D = {
      fillStyle: '', strokeStyle: '', font: '', textAlign: 'left', textBaseline: 'top', lineWidth: 1,
      clearRect() {},
      fillRect() {},
      strokeRect() {},
      fillText(t) {
        texts.push(t);
      },
    };
    drawPanel(fake, model, 1024, 576);
    expect(texts).toContain(model.title);
    expect(texts).toContain(model.headline);
  });

  it('스윙 중에는 미터를 그린다', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가'], intro: false, seed: 6 });
    pressSwing(1000);
    expect(panelModel(game.get(), 1500).meter).not.toBeNull();
    expect(panelModel(game.get(), 1500).meter!.stage).toBe(1);
    expect(panelModel({ ...game.get(), phase: 'aim' }, 1500).meter).toBeNull();
  });
});
