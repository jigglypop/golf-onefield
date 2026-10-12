/**
 * 게임 진행: 모드(스트로크·니어핀·롱드라이브), 1–4명 차례, 홀·타수·벌타, 스윙 미터, 샷 실행·결과 처리, 기록.
 * React 밖의 작은 store 에 상태를 두고 화면이 구독한다.
 *
 * 한 타: 조준(aim) → 스윙 미터(swing) → 비행(flight) → 바운드·굴림(ground) → 결과 처리 → 다음 차례.
 * 스윙 미터 (Space 세 번): 시작 → 파워(0–105 %) → 정확도. 되돌아오는 표시를 0 근처(±3.5 %)에서 누르면 정타,
 *   일찍(0 오른쪽) 누르면 오른쪽으로 휨(슬라이스), 늦게(왼쪽) 누르면 왼쪽(훅). 퍼터는 두 번(시작 → 세기),
 *   퍼터 세기 100 % 는 핀까지 거리의 1.6 배(4–24 m, 평평한 그린 기준)라 짧은 퍼트도 미터를 넉넉히 쓴다.
 * 차례 (스트로크): 티에서는 앞 홀 성적 순(아너), 그 뒤로는 핀에서 가장 먼 사람. 모두 홀아웃하면 홀이 끝난다.
 * 벌타: 물·OB 는 1벌타 후 친 자리에서 다시. 더블 파에 닿으면 그 홀은 거기서 줍는다.
 * 챌린지: 사람마다 3번씩 돌아가며 티에서 친다. 니어핀은 그린(프린지 포함)에 멈춘 공의 핀까지 거리, 롱드라이브는 OB·물이 아닌
 *   곳에 멈춘 공의 티에서 +x 거리.
 *
 * 현재 차례인 사람의 공·타수·점수는 위 칸(ball, strokes, scores)에 두고, 차례가 바뀌면 players 와 맞바꾼다.
 */
import { useSyncExternalStore } from 'react';

import { toEngineLaunch, type ShotInput } from '../engine/convention.ts';
import { resample, simulate, type FlightResult, type TrajectorySample } from '../engine/flight.ts';
import { M_PER_YD } from '../engine/units.ts';
import { createStore } from '../state.ts';
import { CLUBS, DRIVER, fullCarryYd, launchFor, PUTTER, type SwingInput } from './clubs.ts';
import { CTP_HOLE, HOLES, LONG_DRIVE, SURFACE_LABEL, holeLengthM, yawTo, type Hole, type Surface } from './course.ts';
import { bounceAndRoll, putt, puttMaxFor, puttPowerFor, type GroundResult } from './ground.ts';

export type GameMode = 'stroke' | 'ctp' | 'longdrive';
export type GamePhase = 'menu' | 'intro' | 'aim' | 'swing' | 'flight' | 'ground' | 'holed' | 'done';

export interface Ball {
  x: number;
  z: number;
  surface: Surface;
}

export interface GameShot {
  id: number;
  putt: boolean;
  origin: Ball;
  /** 조준 방향 (rad) */
  yaw: number;
  clubIdx: number;
  /** 친 사람 */
  player: number;
  /** 스윙 입력 (실측이면 null) */
  swing: SwingInput | null;
  /** 트랙맨 부호 발사 조건 (퍼트는 null) */
  input: ShotInput | null;
  /** 실측 입력으로 친 샷 */
  measured: boolean;
  flight: FlightResult | null;
  /** 비행 표본 (월드 m, 실시간 s) */
  flightSamples: TrajectorySample[];
  /** 궤적 선용 120 Hz 점 (월드 m) */
  path: Float32Array | null;
  /** 착지 뒤 (월드 m, 시간 0 = 착지) */
  ground: GroundResult;
  tFlight: number;
  tTotal: number;
}

/** 바람: 속도 m/s, 부는 쪽 (rad, 0 = +x) */
export interface Wind {
  speed: number;
  dir: number;
}

export interface SwingMeter {
  /** 0 쉼, 1 파워 올라가는 중, 2 정확도 돌아오는 중 */
  stage: 0 | 1 | 2;
  t0: number;
  power: number;
}

export interface GameMessage {
  text: string;
  tone: 'info' | 'good' | 'bad';
  id: number;
}

export interface PlayerSlot {
  name: string;
  color: string;
  ball: Ball;
  strokes: number;
  /** 이번 홀을 끝냈다 (홀아웃·줍기) */
  done: boolean;
  /** 코스 순서별 점수 (스트로크) */
  scores: (number | null)[];
  /** 챌린지 기록 (m, NaN = 기록 없음) */
  shots: number[];
}

export interface GameState {
  mode: GameMode;
  /** 홀 순서 (HOLES 번호). 롱드라이브는 비어 있다 */
  course: number[];
  /** course 안의 위치 */
  holePos: number;
  /** 지금 홀의 HOLES 번호 (롱드라이브는 −1) */
  holeIdx: number;
  players: PlayerSlot[];
  /** 지금 차례 */
  cur: number;
  /** 챌린지에서 지금까지 친 수 (모든 사람 합) */
  attempt: number;
  /** 지금 차례인 사람의 공·타수·점수 (players[cur] 의 살아 있는 값) */
  ball: Ball;
  strokes: number;
  scores: (number | null)[];
  /** 공을 놓는 차례 번호 (캐릭터를 공 옆으로 옮기고 카메라를 자를 때 쓴다) */
  ballId: number;
  clubIdx: number;
  aimYaw: number;
  wind: Wind;
  phase: GamePhase;
  meter: SwingMeter;
  shot: GameShot | null;
  shotId: number;
  message: GameMessage | null;
  /** 홀 전체를 위에서 보기 */
  overview: boolean;
  /** 애니메이션 배속 */
  timeScale: number;
  seed: number;
  /** 홀 소개(공중 촬영) 시작 시각 (performance.now ms) */
  introT0: number;
  /** 끝났을 때 새 기록 알림 */
  record: string | null;
}

// ------------------------------------------------------------------ 상수

/** 파워 표시가 0 → 1 로 가는 시간 (s) */
export const METER_UP_S = 1.0;
/** 정확도 표시가 1 만큼 되돌아오는 시간 (s) */
export const METER_DOWN_S = 1.0;
/** 정확도 1 (최대 휨)에 해당하는 표시 거리 (0.1 늦으면 휨 1/3) */
export const ACC_SPAN = 0.3;
/** 정타 구간 (±, 되돌아오는 속도로 ±35 ms) */
export const PERFECT = 0.035;
/** 이보다 더 가면 자동으로 크게 늦은 샷 */
export const MAX_LATE = -0.32;
export const MAX_POWER = 1.05;
/** 홀 소개 길이 (s) */
export const INTRO_S = 4.5;
/** 챌린지 한 사람당 시도 */
export const CHALLENGE_TRIES = 3;
export const PLAYER_COLORS = ['#ffd23f', '#ff6a8a', '#5ec8ff', '#9be37a'];

export function meterMarker(m: SwingMeter, now: number): number {
  const dt = (now - m.t0) / 1000;
  if (m.stage === 1) return Math.min(dt / METER_UP_S, MAX_POWER);
  if (m.stage === 2) return m.power - dt / METER_DOWN_S;
  return 0;
}

export function accuracyOf(marker: number): number {
  if (Math.abs(marker) <= PERFECT) return 0;
  return Math.min(Math.max(marker / ACC_SPAN, -1.2), 1.2);
}

// ------------------------------------------------------------------ store

const IDLE: SwingMeter = { stage: 0, t0: 0, power: 0 };
const now = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

function teeBall(h: Hole): Ball {
  return { x: h.tee[0], z: h.tee[1], surface: 'tee' };
}

function slot(name: string, i: number, holes: number): PlayerSlot {
  return { name, color: PLAYER_COLORS[i % PLAYER_COLORS.length]!, ball: teeBall(HOLES[0]!), strokes: 0, done: false, scores: Array.from({ length: holes }, () => null), shots: [] };
}

export const game = createStore<GameState>({
  mode: 'stroke',
  course: [0, 1, 2],
  holePos: 0,
  holeIdx: 0,
  players: [slot('플레이어 1', 0, 3)],
  cur: 0,
  attempt: 0,
  ball: teeBall(HOLES[0]!),
  strokes: 0,
  scores: [null, null, null],
  ballId: 0,
  clubIdx: DRIVER,
  aimYaw: yawTo(HOLES[0]!.tee, HOLES[0]!.teeTarget),
  wind: { speed: 0, dir: 0 },
  phase: 'menu',
  meter: IDLE,
  shot: null,
  shotId: 0,
  message: null,
  overview: false,
  timeScale: 1,
  seed: 1,
  introT0: 0,
  record: null,
});

export function useGame<S>(selector: (s: GameState) => S): S {
  return useSyncExternalStore(game.subscribe, () => selector(game.get()), () => selector(game.get()));
}

/** 샷 애니메이션 시계 (초). 프레임 루프만 바꾼다 */
export const shotClock = { t: 0 };

let messageId = 0;
function msg(text: string, tone: GameMessage['tone'] = 'info'): GameMessage {
  return { text, tone, id: ++messageId };
}

// ------------------------------------------------------------------ 보조

export const holeOf = (s: GameState = game.get()): Hole => (s.holeIdx < 0 ? LONG_DRIVE : HOLES[s.holeIdx]!);
/** 옛 이름 (장면·HUD·시험) */
export const hole = holeOf;

export function distToPinM(s: GameState = game.get()): number {
  const h = holeOf(s);
  return Math.hypot(h.pin[0] - s.ball.x, h.pin[1] - s.ball.z);
}

/** 결정적 난수 [0, 1) */
function rand(seed: number, k: number): number {
  let x = (seed * 2654435761 + k * 40503 + 12345) >>> 0;
  x ^= x << 13;
  x >>>= 0;
  x ^= x >>> 17;
  x ^= x << 5;
  return (x >>> 0) / 4294967296;
}

export function windFor(seed: number, holeIdx: number): Wind {
  return { speed: 0.5 + 5 * rand(seed, 2 * holeIdx + 1), dir: 2 * Math.PI * rand(seed, 2 * holeIdx + 2) };
}

/** 월드 바람을 조준 틀(+x = 조준 방향)로 */
export function windInAimFrame(w: Wind, yaw: number): { x: number; z: number } {
  const wx = w.speed * Math.cos(w.dir);
  const wz = w.speed * Math.sin(w.dir);
  return { x: wx * Math.cos(yaw) + wz * Math.sin(yaw), z: -wx * Math.sin(yaw) + wz * Math.cos(yaw) };
}

/** 공에서 핀까지 닿는 가장 짧은 클럽 (그린은 퍼터, 티 밖에서는 드라이버를 빼고) */
export function suggestClub(ball: Ball, h: Hole): number {
  if (h.kind === 'longdrive') return DRIVER;
  const dYd = Math.hypot(h.pin[0] - ball.x, h.pin[1] - ball.z) / M_PER_YD;
  if (ball.surface === 'green') return PUTTER;
  if (ball.surface === 'fringe' && dYd < 14) return PUTTER;
  let best = -1;
  for (let i = 0; i < PUTTER; i++) {
    if (i === DRIVER && ball.surface !== 'tee') continue;
    if (fullCarryYd(CLUBS[i]!) >= dYd * 0.98) best = i;
  }
  if (best === -1) best = ball.surface === 'tee' ? DRIVER : 1;
  return best;
}

export function scoreName(strokes: number, par: number): string {
  if (strokes === 1) return '홀인원!';
  const d = strokes - par;
  const names: Record<number, string> = { [-3]: '알바트로스!', [-2]: '이글!', [-1]: '버디!', 0: '파', 1: '보기', 2: '더블 보기', 3: '트리플 보기' };
  return names[d] ?? `${d > 0 ? '+' : ''}${d}`;
}

/** 조준 틀(공 원점, +x = yaw) 표본 → 월드 */
function toWorld(samples: TrajectorySample[], ox: number, oz: number, yaw: number): TrajectorySample[] {
  const c = Math.cos(yaw);
  const s = Math.sin(yaw);
  return samples.map((p) => ({
    t: p.t,
    x: ox + p.x * c - p.z * s,
    y: p.y,
    z: oz + p.x * s + p.z * c,
    vx: p.vx * c - p.vz * s,
    vy: p.vy,
    vz: p.vx * s + p.vz * c,
  }));
}

/** 발사 조건으로 비행만 (조준·바람 반영, 월드 표본). 미리보기에도 쓴다 */
export function flyFrom(input: ShotInput, ball: { x: number; z: number }, yaw: number, wind: Wind): { result: FlightResult; samples: TrajectorySample[] } {
  const result = simulate(toEngineLaunch(input), { wind: windInAimFrame(wind, yaw) });
  return { result, samples: toWorld(result.samples, ball.x, ball.z, yaw) };
}

/** 지금 차례의 살아 있는 값을 players 에 써 넣은 사본 */
function savedPlayers(s: GameState, patch: Partial<PlayerSlot> = {}): PlayerSlot[] {
  return s.players.map((p, i) => (i === s.cur ? { ...p, ball: s.ball, strokes: s.strokes, scores: s.scores, ...patch } : p));
}

/** 공을 놓은 자리에서 조준 기본값 (티의 파4·5 는 조준점, 나머지는 핀) */
function aimFor(ball: Ball, h: Hole): number {
  return ball.surface === 'tee' && h.par > 3 ? yawTo([ball.x, ball.z], h.teeTarget) : yawTo([ball.x, ball.z], h.pin);
}

// ------------------------------------------------------------------ 기록 (브라우저 저장소, 없어도 돈다)

export interface Records {
  stroke3?: number;
  stroke9?: number;
  ctp?: number;
  longdrive?: number;
}
const RECORD_KEY = 'golf-onefield-records';

export function loadRecords(): Records {
  try {
    return JSON.parse(localStorage.getItem(RECORD_KEY) ?? '{}') as Records;
  } catch {
    return {};
  }
}

function saveRecords(r: Records): void {
  try {
    localStorage.setItem(RECORD_KEY, JSON.stringify(r));
  } catch {
    // 저장소를 못 쓰면 기록만 남지 않는다
  }
}

// ------------------------------------------------------------------ 시작·홀

export interface GameConfig {
  mode: GameMode;
  /** 스트로크: 3 또는 9 홀 */
  holes?: number;
  players?: string[];
  seed?: number;
  /** 홀 소개(공중 촬영)를 보여 줄지 */
  intro?: boolean;
}

export function startGame(cfg: GameConfig): void {
  const names = (cfg.players && cfg.players.length > 0 ? cfg.players : ['플레이어 1']).slice(0, 4);
  const course = cfg.mode === 'stroke' ? Array.from({ length: Math.min(cfg.holes ?? 3, HOLES.length) }, (_, i) => i) : cfg.mode === 'ctp' ? [CTP_HOLE] : [];
  const seed = cfg.seed ?? (Date.now() >>> 0) % 100000;
  game.set({
    mode: cfg.mode,
    course,
    holePos: 0,
    players: names.map((n, i) => slot(n, i, course.length)),
    cur: 0,
    attempt: 0,
    seed,
    record: null,
    overview: false,
  });
  startHole(0, cfg.intro ?? false);
}

/** 예전 시작: 혼자 3홀 스트로크 (시험·콘솔) */
export function newRound(seed?: number): void {
  startGame({ mode: 'stroke', holes: 3, players: ['플레이어 1'], intro: false, ...(seed !== undefined ? { seed } : {}) });
}

/** 메뉴로 */
export function openMenu(): void {
  game.set({ phase: 'menu', overview: false, message: null });
}

/** 티 순서: 첫 홀은 이름 순, 그 뒤는 앞 홀 점수가 좋은 사람부터(같으면 앞 순서) */
function teeOrder(players: PlayerSlot[], prevPos: number): number[] {
  const idx = players.map((_, i) => i);
  if (prevPos < 0) return idx;
  return idx.sort((a, b) => (players[a]!.scores[prevPos] ?? 99) - (players[b]!.scores[prevPos] ?? 99) || a - b);
}

export function startHole(pos: number, intro = true): void {
  const s = game.get();
  const holeIdx = s.mode === 'longdrive' ? -1 : s.course[pos]!;
  const h = holeIdx < 0 ? LONG_DRIVE : HOLES[holeIdx]!;
  const tee = teeBall(h);
  const players = s.players.map((p) => ({ ...p, ball: tee, strokes: 0, done: false }));
  const first = s.mode === 'stroke' ? teeOrder(players, pos - 1)[0]! : 0;
  shotClock.t = 0;
  const title =
    s.mode === 'stroke'
      ? `${h.number}번 홀 · ${h.name} · 파 ${h.par} · ${Math.round(holeLengthM(h) / M_PER_YD)} yd`
      : s.mode === 'ctp'
        ? `니어핀 챌린지 · ${h.name} · ${Math.round(holeLengthM(h) / M_PER_YD)} yd · ${CHALLENGE_TRIES}번씩`
        : `롱드라이브 · 폭 50 m 그리드 · ${CHALLENGE_TRIES}번씩`;
  game.set({
    holePos: pos,
    holeIdx,
    players,
    cur: first,
    ball: tee,
    strokes: 0,
    scores: players[first]!.scores,
    ballId: s.ballId + 1,
    clubIdx: suggestClub(tee, h),
    aimYaw: aimFor(tee, h),
    wind: windFor(s.seed, holeIdx < 0 ? 40 : holeIdx),
    phase: intro ? 'intro' : 'aim',
    introT0: now(),
    meter: IDLE,
    shot: null,
    message: msg(players.length > 1 ? `${title} · ${players[first]!.name} 먼저` : title),
    overview: false,
  });
}

/** 홀 소개 끝내기 (시간이 다 되거나 Space·Enter) */
export function skipIntro(): void {
  if (game.get().phase === 'intro') game.set({ phase: 'aim' });
}

/** 프레임마다: 소개 시간이 다 되면 조준으로 */
export function introTick(t = now()): void {
  const s = game.get();
  if (s.phase === 'intro' && t - s.introT0 >= INTRO_S * 1000) game.set({ phase: 'aim' });
}

export function nextHole(): void {
  const s = game.get();
  if (s.phase === 'done') {
    startGame({ mode: s.mode, holes: s.course.length, players: s.players.map((p) => p.name), intro: true });
    return;
  }
  if (s.phase !== 'holed') return;
  if (s.holePos + 1 < s.course.length) startHole(s.holePos + 1, true);
  else finishGame();
}

function finishGame(): void {
  const s = game.get();
  const rec = loadRecords();
  let record: string | null = null;
  if (s.mode === 'stroke') {
    const par = s.course.reduce((a, i) => a + HOLES[i]!.par, 0);
    const best = Math.min(...s.players.map((p) => p.scores.reduce<number>((a, v) => a + (v ?? 0), 0) - par));
    const key = s.course.length >= 9 ? 'stroke9' : 'stroke3';
    if (rec[key] === undefined || best < rec[key]!) {
      record = `${s.course.length}홀 최고 기록 ${best > 0 ? '+' : ''}${best}`;
      rec[key] = best;
    }
  } else {
    const all = s.players.flatMap((p) => p.shots).filter((v) => Number.isFinite(v));
    if (all.length > 0) {
      if (s.mode === 'ctp') {
        const best = Math.min(...all);
        if (rec.ctp === undefined || best < rec.ctp) {
          record = `니어핀 최고 기록 ${best.toFixed(2)} m`;
          rec.ctp = best;
        }
      } else {
        const best = Math.max(...all) / M_PER_YD;
        if (rec.longdrive === undefined || best > rec.longdrive) {
          record = `롱드라이브 최고 기록 ${best.toFixed(1)} yd`;
          rec.longdrive = best;
        }
      }
    }
  }
  if (record) saveRecords(rec);
  game.set({ phase: 'done', record, message: msg(record ? `신기록! ${record}` : '게임 끝', 'good') });
}

// ------------------------------------------------------------------ 조작

export function selectClub(delta: number): void {
  const s = game.get();
  if (s.phase !== 'aim') return;
  game.set({ clubIdx: Math.min(Math.max(s.clubIdx + delta, 0), PUTTER) });
}

export function setClub(i: number): void {
  const s = game.get();
  if (s.phase !== 'aim') return;
  game.set({ clubIdx: Math.min(Math.max(Math.round(i), 0), PUTTER) });
}

/** 조준 돌리기 (rad, + = 오른쪽) */
export function aimBy(d: number): void {
  const s = game.get();
  if (s.phase !== 'aim') return;
  game.set({ aimYaw: s.aimYaw + d });
}

export function aimAtPin(): void {
  const s = game.get();
  if (s.phase !== 'aim') return;
  game.set({ aimYaw: yawTo(s.ball, holeOf(s).pin) });
}

export function toggleOverview(): void {
  game.set({ overview: !game.get().overview });
}

export function isPutter(s: GameState = game.get()): boolean {
  return CLUBS[s.clubIdx]!.putter === true;
}

/** Space: 소개 건너뛰기 / 미터 시작 → 파워 → 정확도 (퍼터는 시작 → 세기) */
export function pressSwing(t = now()): void {
  const s = game.get();
  if (s.phase === 'intro') {
    skipIntro();
    return;
  }
  if (s.phase === 'aim') {
    game.set({ phase: 'swing', meter: { stage: 1, t0: t, power: 0 }, message: null });
    return;
  }
  if (s.phase !== 'swing') return;
  const m = s.meter;
  if (m.stage === 1) {
    const power = Math.min(Math.max(meterMarker(m, t), 0.02), MAX_POWER);
    if (isPutter(s)) swing({ power, accuracy: 0 });
    else game.set({ meter: { stage: 2, t0: t, power } });
  } else if (m.stage === 2) {
    swing({ power: m.power, accuracy: accuracyOf(meterMarker(m, t)) });
  }
}

/** 프레임마다: 미터가 끝까지 가면 자동으로 넘긴다 */
export function meterTick(t = now()): void {
  const s = game.get();
  if (s.phase !== 'swing') return;
  const m = s.meter;
  if (m.stage === 1 && meterMarker(m, t) >= MAX_POWER) {
    if (isPutter(s)) swing({ power: MAX_POWER, accuracy: 0 });
    else game.set({ meter: { stage: 2, t0: t, power: MAX_POWER } });
  } else if (m.stage === 2 && meterMarker(m, t) <= MAX_LATE) {
    swing({ power: m.power, accuracy: -1.2 });
  }
}

export function cancelSwing(): void {
  if (game.get().phase === 'swing') game.set({ phase: 'aim', meter: IDLE });
}

/** 한 타 친다. measured 를 주면 그 발사 조건(실측)을 그대로 쓴다 */
export function swing(sw: SwingInput, measured?: ShotInput): GameShot | null {
  const s = game.get();
  if (s.phase !== 'aim' && s.phase !== 'swing') return null;
  const h = holeOf(s);
  const id = s.shotId + 1;
  let shot: GameShot;
  if (isPutter(s) && !measured) {
    const g = putt(h, s.ball.x, s.ball.z, s.aimYaw, sw.power, puttMaxFor(distToPinM(s)));
    shot = { id, putt: true, origin: s.ball, yaw: s.aimYaw, clubIdx: s.clubIdx, player: s.cur, swing: sw, input: null, measured: false, flight: null, flightSamples: [], path: null, ground: g, tFlight: 0, tTotal: g.duration };
  } else {
    const club = CLUBS[isPutter(s) ? PUTTER - 1 : s.clubIdx]!;
    const wobble = 2 * rand(s.seed + 7, id) - 1;
    const input = measured ?? launchFor(club, sw, s.ball.surface, wobble);
    const { result, samples } = flyFrom(input, s.ball, s.aimYaw, s.wind);
    if (!result.landed) {
      game.set({ phase: 'aim', meter: IDLE, message: msg('공이 떨어지지 않았습니다. 다시 쳐 주세요', 'bad') });
      return null;
    }
    const end = samples[samples.length - 1]!;
    const backspin = result.landSpinRpm * Math.cos((input.spinAxisDeg * Math.PI) / 180);
    const g = bounceAndRoll(h, { x: end.x, z: end.z, vx: end.vx, vy: end.vy, vz: end.vz, backspinRpm: backspin });
    const path = resample(samples, 1 / 120).xyz;
    shot = { id, putt: false, origin: s.ball, yaw: s.aimYaw, clubIdx: s.clubIdx, player: s.cur, swing: measured ? null : sw, input, measured: !!measured, flight: result, flightSamples: samples, path, ground: g, tFlight: result.timeS, tTotal: result.timeS + g.duration };
  }
  shotClock.t = 0;
  game.set({ phase: shot.putt ? 'ground' : 'flight', shot, shotId: id, strokes: s.strokes + 1, meter: IDLE, message: null, overview: false });
  return shot;
}

/** 애니메이션 시계를 끝으로 (시험·빨리 감기) */
export function skipAnimation(): void {
  const s = game.get();
  if (!s.shot || (s.phase !== 'flight' && s.phase !== 'ground')) return;
  shotClock.t = s.shot.tTotal;
  resolveShot();
}

/** 프레임 루프가 부른다: 시계를 늘리고 단계를 넘긴다 */
export function advanceShot(delta: number): void {
  const s = game.get();
  if (!s.shot || (s.phase !== 'flight' && s.phase !== 'ground')) return;
  shotClock.t = Math.min(shotClock.t + Math.min(delta, 0.1) * s.timeScale, s.shot.tTotal);
  if (s.phase === 'flight' && shotClock.t >= s.shot.tFlight) game.set({ phase: 'ground' });
  if (shotClock.t >= s.shot.tTotal) resolveShot();
}

function describeRest(ball: Ball, h: Hole, shot: GameShot): string {
  const d = Math.hypot(h.pin[0] - ball.x, h.pin[1] - ball.z);
  const where = ball.surface === 'green' ? `그린 온 · 핀까지 ${d.toFixed(1)} m` : `${SURFACE_LABEL[ball.surface]} · 핀까지 ${Math.round(d / M_PER_YD)} yd`;
  if (shot.putt || !shot.flight) return where;
  const carry = Math.hypot(shot.ground.samples[0]!.x - shot.origin.x, shot.ground.samples[0]!.z - shot.origin.z) / M_PER_YD;
  return `캐리 ${Math.round(carry)} yd · 런 ${Math.round(shot.ground.runM / M_PER_YD)} yd · ${where}`;
}

/** 샷이 끝난 뒤: 모드에 따라 결과 처리 → 다음 차례 */
export function resolveShot(): void {
  const s = game.get();
  const shot = s.shot;
  if (!shot || (s.phase !== 'flight' && s.phase !== 'ground')) return;
  if (s.mode === 'stroke') resolveStroke(s, shot);
  else resolveChallenge(s, shot);
}

function resolveStroke(s: GameState, shot: GameShot): void {
  const h = holeOf(s);
  const g = shot.ground;
  const name = s.players.length > 1 ? `${s.players[s.cur]!.name} · ` : '';
  let ball: Ball;
  let strokes = s.strokes;
  let done = false;
  let message: GameMessage;
  const scores = [...s.scores];
  if (g.event === 'holed') {
    ball = { x: h.pin[0], z: h.pin[1], surface: 'green' };
    done = true;
    scores[s.holePos] = strokes;
    message = msg(`${name}${scoreName(strokes, h.par)} (${strokes}타)`, strokes <= h.par ? 'good' : 'info');
  } else if (g.event === 'water' || g.event === 'ob') {
    ball = shot.origin;
    strokes += 1;
    message = msg(`${name}${g.event === 'water' ? '물에 빠졌습니다' : 'OB 입니다'} · 1벌타, 친 자리에서 다시`, 'bad');
  } else {
    ball = { x: g.rest.x, z: g.rest.z, surface: g.surface };
    message = msg(`${name}${describeRest(ball, h, shot)}`, ball.surface === 'green' || ball.surface === 'fairway' ? 'good' : 'info');
  }
  if (!done && strokes >= 2 * h.par) {
    done = true;
    strokes = 2 * h.par;
    scores[s.holePos] = strokes;
    message = msg(`${name}더블 파 · 이 홀은 여기까지 (${strokes}타)`, 'bad');
  }
  game.set({ ball, strokes, scores, players: savedPlayers({ ...s, ball, strokes, scores }, { done }) });
  nextTurn(message);
}

function nextTurn(message: GameMessage): void {
  const s = game.get();
  const h = holeOf(s);
  const players = s.players;
  const open = players.map((p, i) => ({ p, i })).filter(({ p }) => !p.done);
  if (open.length === 0) {
    game.set({ phase: 'holed', message, meter: IDLE });
    return;
  }
  // 아직 티샷 안 한 사람이 있으면 티 순서대로, 아니면 핀에서 가장 먼 사람
  const teeing = open.filter(({ p }) => p.strokes === 0);
  const dist = (b: Ball) => Math.hypot(h.pin[0] - b.x, h.pin[1] - b.z);
  const next = teeing.length > 0 ? teeing[0]!.i : open.reduce((a, b) => (dist(b.p.ball) > dist(a.p.ball) ? b : a)).i;
  const p = players[next]!;
  const turn = next !== s.cur && players.length > 1 ? ` → ${p.name} 차례` : '';
  game.set({
    cur: next,
    ball: p.ball,
    strokes: p.strokes,
    scores: p.scores,
    ballId: s.ballId + 1,
    clubIdx: suggestClub(p.ball, h),
    aimYaw: aimFor(p.ball, h),
    phase: 'aim',
    meter: IDLE,
    message: turn ? { ...message, text: message.text + turn } : message,
  });
}

function resolveChallenge(s: GameState, shot: GameShot): void {
  const h = holeOf(s);
  const g = shot.ground;
  let d = NaN;
  if (s.mode === 'ctp') {
    if (g.event === 'holed') d = 0;
    else if (g.event === 'rest' && (g.surface === 'green' || g.surface === 'fringe')) d = Math.hypot(h.pin[0] - g.rest.x, h.pin[1] - g.rest.z);
  } else if (g.event === 'rest' && g.surface !== 'ob' && g.surface !== 'water') {
    d = g.rest.x - h.tee[0];
  }
  const name = s.players.length > 1 ? `${s.players[s.cur]!.name} · ` : '';
  const text =
    s.mode === 'ctp'
      ? Number.isNaN(d)
        ? `${name}그린을 놓쳤습니다 · 기록 없음`
        : d === 0
          ? `${name}홀인원!!`
          : `${name}핀까지 ${d.toFixed(2)} m`
      : Number.isNaN(d)
        ? `${name}${g.event === 'water' ? '물' : 'OB'} · 기록 없음`
        : `${name}${(d / M_PER_YD).toFixed(1)} yd`;
  const players = s.players.map((p, i) => (i === s.cur ? { ...p, shots: [...p.shots, d] } : p));
  const attempt = s.attempt + 1;
  if (attempt >= players.length * CHALLENGE_TRIES) {
    game.set({ players, attempt, ball: { ...g.rest, surface: g.surface }, message: msg(text, Number.isNaN(d) ? 'bad' : 'good') });
    finishGame();
    return;
  }
  const next = attempt % players.length;
  const tee = teeBall(h);
  const p = players[next]!;
  game.set({
    players,
    attempt,
    cur: next,
    ball: tee,
    strokes: 0,
    scores: p.scores,
    ballId: s.ballId + 1,
    clubIdx: suggestClub(tee, h),
    aimYaw: aimFor(tee, h),
    phase: 'aim',
    meter: IDLE,
    message: msg(players.length > 1 ? `${text} → ${p.name} 차례` : text, Number.isNaN(d) ? 'bad' : 'good'),
  });
}

/** 핀까지 거리에 맞춰 알아서 친다 (브라우저 시험·데모용 봇) */
export function autoShot(): void {
  const s = game.get();
  if (s.phase === 'intro') {
    skipIntro();
    return;
  }
  if (s.phase === 'holed') {
    nextHole();
    return;
  }
  if (s.phase !== 'aim') return;
  const h = holeOf(s);
  if (h.kind === 'longdrive') {
    game.set({ clubIdx: DRIVER, aimYaw: 0 });
    swing({ power: 1, accuracy: 0 });
    return;
  }
  const d = distToPinM(s);
  const atTee = s.ball.surface === 'tee' && h.par > 3;
  game.set({ aimYaw: atTee ? yawTo(s.ball, h.teeTarget) : yawTo(s.ball, h.pin) });
  if (isPutter(s)) {
    swing({ power: (puttPowerFor(d, puttMaxFor(d)) * 1.12 + 0.012 * (24 / puttMaxFor(d))), accuracy: 0 });
    return;
  }
  const carryM = fullCarryYd(CLUBS[s.clubIdx]!) * M_PER_YD;
  const power = atTee ? 1 : Math.min(Math.max((d * 0.95) / carryM, 0.2), 1);
  swing({ power, accuracy: 0 });
}
