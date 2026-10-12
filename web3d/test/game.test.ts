/**
 * 게임 논리: 코스 지면 판정, 클럽, 지면 물리(손맛 범위), 스윙 미터, 라운드 진행(봇으로 끝까지).
 * 지면 물리의 기준값은 게임용 목표이고 실측 검증이 아니다.
 */
import { beforeEach, describe, expect, it } from 'vitest';

import { toEngineLaunch } from '../src/engine/convention.ts';
import { simulate } from '../src/engine/flight.ts';
import { M_PER_YD } from '../src/engine/units.ts';
import { CLUBS, fullCarryYd, launchFor, PUTTER } from '../src/game/clubs.ts';
import { HOLES, surfaceAt, type Hole } from '../src/game/course.ts';
import { bounceAndRoll, putt, puttPowerFor } from '../src/game/ground.ts';
import { accuracyOf, autoShot, game, meterMarker, newRound, pressSwing, skipAnimation, skipIntro, startGame, swing, setClub, ACC_SPAN, CHALLENGE_TRIES, METER_UP_S } from '../src/game/round.ts';

const [H1, H2, H3] = HOLES as [Hole, Hole, Hole];

function flatHole(kind: 'fairway' | 'green' | 'rough', pin: [number, number] = [5000, 0], slope: [number, number] = [0, 0]): Hole {
  return {
    number: 0, name: '시험', par: 4, tee: [-500, 0], teeTarget: [-400, 0], pin,
    fairways: kind === 'fairway' ? [{ path: [[-900, 0], [1900, 0]], width: 4000 }] : [],
    green: kind === 'green' ? { c: [0, 0], rx: 3000, rz: 3000, rot: 0, slope } : { c: [9000, 9000], rx: 1, rz: 1, rot: 0, slope },
    bunkers: [], water: [], bounds: [[-1000, -1000], [2000, -1000], [2000, 1000], [-1000, 1000]],
  };
}

/** 클럽 풀스윙의 착지 → 지면 결과 */
function runYd(clubId: string, kind: 'fairway' | 'green' | 'rough'): number {
  const club = CLUBS.find((c) => c.id === clubId)!;
  const r = simulate(toEngineLaunch(launchFor(club, { power: 1, accuracy: 0 }, 'fairway')));
  const e = r.samples[r.samples.length - 1]!;
  return bounceAndRoll(flatHole(kind), { x: e.x, z: e.z, vx: e.vx, vy: e.vy, vz: e.vz, backspinRpm: r.landSpinRpm }).runM / M_PER_YD;
}

describe('코스', () => {
  it('지면 판정', () => {
    expect(surfaceAt(H1, 0, 0)).toBe('tee');
    expect(surfaceAt(H1, 150, 4)).toBe('fairway');
    expect(surfaceAt(H1, 150, 30)).toBe('rough');
    expect(surfaceAt(H1, 338, 62)).toBe('green');
    expect(surfaceAt(H1, 222, -12)).toBe('bunker');
    expect(surfaceAt(H1, 150, -80)).toBe('ob');
    expect(surfaceAt(H2, 60, 0)).toBe('water');
    expect(surfaceAt(H2, 150, -8)).toBe('green');
    expect(surfaceAt(H3, 410, 10)).toBe('water');
    expect(surfaceAt(H3, 468, 36)).toBe('green');
  });
  it('티·핀·조준점은 경계 안, 핀은 그린 위', () => {
    for (const h of HOLES) {
      expect(surfaceAt(h, h.pin[0], h.pin[1])).toBe('green');
      expect(surfaceAt(h, h.tee[0], h.tee[1])).toBe('tee');
      expect(surfaceAt(h, h.teeTarget[0], h.teeTarget[1])).not.toBe('ob');
    }
  });
});

describe('클럽', () => {
  it('풀스윙 캐리는 드라이버부터 샌드 웨지까지 줄어든다', () => {
    const c = CLUBS.slice(0, PUTTER).map(fullCarryYd);
    for (let i = 1; i < c.length; i++) expect(c[i]!).toBeLessThan(c[i - 1]!);
    expect(c[0]!).toBeGreaterThan(260);
    expect(c[c.length - 1]!).toBeGreaterThan(80);
  });
  it('정확도 + 는 오른쪽, − 는 왼쪽으로 휜다 (트랙맨 부호)', () => {
    const club = CLUBS.find((c) => c.id === '7i')!;
    const fade = simulate(toEngineLaunch(launchFor(club, { power: 1, accuracy: 0.6 }, 'fairway')));
    const hook = simulate(toEngineLaunch(launchFor(club, { power: 1, accuracy: -0.6 }, 'fairway')));
    expect(fade.lateralYd).toBeGreaterThan(8);
    expect(hook.lateralYd).toBeLessThan(-8);
  });
});

describe('지면 물리 (게임용 목표 범위)', () => {
  it('런: 드라이버 페어웨이 12–28 yd, 7번 그린 1–6 yd, PW 그린 3 yd 아래, 러프는 페어웨이보다 짧다', () => {
    const d = runYd('1w', 'fairway');
    expect(d).toBeGreaterThan(12);
    expect(d).toBeLessThan(28);
    const i7 = runYd('7i', 'green');
    expect(i7).toBeGreaterThan(1);
    expect(i7).toBeLessThan(6);
    expect(runYd('pw', 'green')).toBeLessThan(3);
    expect(runYd('1w', 'rough')).toBeLessThan(d / 2);
  });
  it('스팀프 10: 평평한 그린에서 3.05 m', () => {
    expect(putt(flatHole('green'), 0, 0, 0, puttPowerFor(3.05)).runM).toBeCloseTo(3.05, 2);
  });
  it('홀컵: 알맞은 세기는 들어가고, 아주 세면 안 들어간다', () => {
    const h = flatHole('green', [4, 0]);
    expect(putt(h, 0, 0, 0, puttPowerFor(4.4)).event).toBe('holed');
    expect(putt(h, 0, 0, 0, puttPowerFor(14)).event).toBe('rest');
  });
  it('경사: 공은 낮은 쪽(−기울기)으로 휜다', () => {
    const g = putt(flatHole('green', [5000, 0], [0, 0.02]), 0, 0, 0, puttPowerFor(6));
    expect(g.rest.z).toBeLessThan(-0.2);
  });
  it('물·OB 에 떨어지면 그 사건으로 끝난다', () => {
    expect(bounceAndRoll(H2, { x: 60, z: 0, vx: 10, vy: -15, vz: 0, backspinRpm: 6000 }).event).toBe('water');
    expect(bounceAndRoll(H1, { x: 150, z: -80, vx: 10, vy: -15, vz: 0, backspinRpm: 6000 }).event).toBe('ob');
  });
});

describe('스윙 미터', () => {
  it('정확도: 정타 구간과 끝값', () => {
    expect(accuracyOf(0.01)).toBe(0);
    expect(accuracyOf(ACC_SPAN)).toBeCloseTo(1, 6);
    expect(accuracyOf(-ACC_SPAN)).toBeCloseTo(-1, 6);
  });
  it('세 번 누르기: 파워는 둘째 누름의 표시값', () => {
    newRound(7);
    pressSwing(1000);
    expect(game.get().phase).toBe('swing');
    pressSwing(1000 + 0.8 * METER_UP_S * 1000);
    expect(game.get().meter.power).toBeCloseTo(0.8, 6);
    expect(meterMarker(game.get().meter, game.get().meter.t0)).toBeCloseTo(0.8, 6);
    pressSwing(game.get().meter.t0 + 0.8 * 0.8 * 1000);
    expect(game.get().phase).toBe('flight');
    expect(game.get().strokes).toBe(1);
  });
});

describe('라운드', () => {
  beforeEach(() => newRound(42));
  it('물에 빠지면 1벌타 후 친 자리에서 다시', () => {
    game.set({ holeIdx: 1 });
    newRound(42);
    // 2번 홀(호수)로: 1번 홀을 봇으로 끝낸다
    for (let i = 0; i < 40 && game.get().holeIdx === 0; i++) {
      autoShot();
      skipAnimation();
    }
    expect(game.get().holeIdx).toBe(1);
    if (game.get().phase === 'intro') skipIntro();
    expect(game.get().phase).toBe('aim');
    const tee = { ...game.get().ball };
    setClub(CLUBS.findIndex((c) => c.id === 'sw'));
    swing({ power: 0.55, accuracy: 0 });
    skipAnimation();
    expect(game.get().strokes).toBe(2);
    expect(game.get().ball.x).toBeCloseTo(tee.x, 6);
    expect(game.get().message?.text).toContain('물');
  });
  it('봇이 3홀을 끝까지 돈다 (홀마다 더블 파 이하)', () => {
    for (let i = 0; i < 200 && game.get().phase !== 'done'; i++) {
      autoShot();
      skipAnimation();
    }
    const s = game.get();
    expect(s.phase).toBe('done');
    s.scores.forEach((v, i) => {
      expect(v).not.toBeNull();
      expect(v!).toBeLessThanOrEqual(2 * HOLES[i]!.par);
    });
    console.log(`[봇 라운드] ${s.scores.join(', ')} (파 ${HOLES.slice(0, s.scores.length).map((h) => h.par).join(', ')})`);
  });
});

/** 봇으로 끝까지 */
function playOut(limit = 400): void {
  for (let i = 0; i < limit && game.get().phase !== 'done'; i++) {
    autoShot();
    skipAnimation();
  }
}

describe('모드', () => {
  it('2인 스트로크: 둘 다 끝까지, 티샷 뒤에는 핀에서 먼 사람부터', () => {
    startGame({ mode: 'stroke', holes: 3, players: ['가', '나'], intro: false, seed: 5 });
    // 1번 홀 티샷 두 번
    autoShot();
    skipAnimation();
    expect(game.get().cur).toBe(1);
    autoShot();
    skipAnimation();
    const s = game.get();
    const pin = HOLES[0]!.pin;
    const d = s.players.map((p, i) => (i === s.cur ? Math.hypot(pin[0] - s.ball.x, pin[1] - s.ball.z) : Math.hypot(pin[0] - p.ball.x, pin[1] - p.ball.z)));
    expect(d[s.cur]!).toBeGreaterThanOrEqual(Math.min(...d) - 1e-9);
    playOut();
    const end = game.get();
    expect(end.phase).toBe('done');
    end.players.forEach((p, pi) => {
      const scores = pi === end.cur ? end.scores : p.scores;
      scores.forEach((v, i) => {
        expect(v).not.toBeNull();
        expect(v!).toBeLessThanOrEqual(2 * HOLES[i]!.par);
      });
    });
  });
  it(`니어핀: 사람마다 ${CHALLENGE_TRIES}번`, () => {
    startGame({ mode: 'ctp', players: ['가', '나'], intro: false, seed: 9 });
    playOut();
    const s = game.get();
    expect(s.phase).toBe('done');
    s.players.forEach((p) => expect(p.shots.length).toBe(CHALLENGE_TRIES));
    console.log(`[니어핀] ${s.players.map((p) => p.shots.map((v) => (Number.isNaN(v) ? '미스' : v.toFixed(2))).join('/')).join(' | ')}`);
  });
  it('롱드라이브: 곧게 친 드라이버는 기록이 남는다', () => {
    startGame({ mode: 'longdrive', players: ['가'], intro: false, seed: 3 });
    playOut();
    const s = game.get();
    expect(s.phase).toBe('done');
    expect(s.players[0]!.shots.length).toBe(CHALLENGE_TRIES);
    const yd = s.players[0]!.shots.filter(Number.isFinite).map((v) => v / M_PER_YD);
    expect(yd.length).toBeGreaterThan(0);
    for (const v of yd) expect(v).toBeGreaterThan(240);
    console.log(`[롱드라이브] ${yd.map((v) => v.toFixed(1)).join(', ')} yd`);
  });
});
