/**
 * 게임 진행: 모드(스트로크·니어핀·롱드라이브), 1–4명 차례, 홀·타수·벌타, 스윙 미터, 샷 실행·결과 처리, 기록.
 * React 밖의 작은 store 에 상태를 두고 화면이 구독한다.
 *
 * 한 타: 조준(aim) → 스윙 미터(swing) → 비행(flight) → 바운드·굴림(ground) → 결과 처리 → 다음 차례.
 * 스윙 미터 (Space 세 번): 시작 → 파워(0–105 %) → 정확도. 되돌아오는 표시를 0 근처(±3.5 %)에서 누르면 정타,
 *   일찍(0 오른쪽) 누르면 오른쪽으로 휨(슬라이스), 늦게(왼쪽) 누르면 왼쪽(훅). 퍼터는 두 번(시작 → 세기),
 *   퍼터 세기 100 % 는 핀까지 거리의 1.6 배(4–24 m, 평평한 그린 기준)라 짧은 퍼트도 미터를 넉넉히 쓴다.
 * 스윙 → 임팩트: 정확도를 정하는 순간 샷이 정해지지만 공은 임팩트(`impactDelayS`, 풀스윙 0.44 s)에 떠난다.
 *   그동안 애니메이션 시계(shotClock)는 음수로 시작해 0 이 되는 때가 임팩트다 (공은 제자리, 골퍼가 스윙).
 * 차례 (스트로크): 티에서는 앞 홀 성적 순(아너, 동점이면 앞 홀 순서 유지), 그 뒤로는 핀에서 가장 먼 사람. 모두 홀아웃하면 홀이 끝난다.
 * 벌타: 물·OB 는 1벌타 후 친 자리에서 다시. 더블 파에 닿으면 그 홀은 거기서 줍는다. 나무에 맞은 공은 그 자리에서 수직으로 떨어진다.
 * 챌린지: 사람마다 3번씩 돌아가며 티에서 친다. 니어핀은 그린(프린지 포함)에 멈춘 공의 핀까지 거리, 롱드라이브는 OB·물이 아닌
 *   곳에 멈춘 공의 티에서 +x 거리.
 * 봇: 사람 슬롯에 isBot 을 켜면 조준·클럽·세기를 알아서 고른다 (`botPlan`). 난이도가 높을수록 세기·방향 흔들림이 작다.
 * 핸디캡: 선택 사항. 순타수 = 총타수 − round(핸디캡 × 홀 수 / 18) (스코어카드에 같이 보인다).
 *
 * 현재 차례인 사람의 공·타수·점수는 위 칸(ball, strokes, scores)에 두고, 차례가 바뀌면 players 와 맞바꾼다.
 */
import { useSyncExternalStore } from 'react';

import { toEngineLaunch, type ShotInput } from '../engine/convention.ts';
import { resample, simulate, type FlightResult, type GustOptions, type TrajectorySample } from '../engine/flight.ts';
import { M_PER_YD } from '../engine/units.ts';
import { createStore } from '../state.ts';
import { CLUBS, DRIVER, fullCarryYd, launchFor, PUTTER, type SwingInput } from './clubs.ts';
import { CTP_HOLE, HOLES, LONG_DRIVE, SURFACE_LABEL, holeLengthM, yawTo, type Hole, type Surface } from './course.ts';
import { bounceAndRoll, putt, puttMaxFor, puttPowerFor, type GroundResult, type GroundWind } from './ground.ts';
import { cutAtTree, firstTreeHit } from './obstacles.ts';

export type GameMode = 'stroke' | 'ctp' | 'longdrive';
export type GamePhase = 'menu' | 'intro' | 'aim' | 'swing' | 'flight' | 'ground' | 'holed' | 'done';

export interface Ball {
  x: number;
  z: number;
  surface: Surface;
}

/** 나무에 맞은 곳 (월드 m, 비행 시각 s) */
export interface TreeStrike {
  x: number;
  y: number;
  z: number;
  t: number;
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
  /** 비행 표본 (월드 m, 실시간 s). 나무에 맞았으면 맞은 곳에서 수직 낙하 표본까지 */
  flightSamples: TrajectorySample[];
  /** 궤적 선용 120 Hz 점 (월드 m) */
  path: Float32Array | null;
  /** 착지 뒤 (월드 m, 시간 0 = 착지) */
  ground: GroundResult;
  /** 임팩트에서 착지(나무에 맞았으면 낙하 끝)까지 (s) */
  tFlight: number;
  /** 임팩트에서 멈출 때까지 (s) */
  tTotal: number;
  /** 샷 결정에서 임팩트까지 (s): 이 동안 공은 제자리에서 골퍼가 스윙한다 */
  impactDelay: number;
  /** 비행 중 나무에 맞았다 */
  tree: TreeStrike | null;
  /** 이 샷에 쓴 돌풍 (없으면 null) */
  gust: GustOptions | null;
}

/** 바람: 속도 m/s, 부는 쪽 (rad, 0 = +x), 돌풍 진폭 m/s */
export interface Wind {
  speed: number;
  dir: number;
  /** 돌풍 진폭 (m/s). 없거나 0 이면 돌풍 없음 */
  gust?: number;
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

/** 봇 난이도 */
export type BotLevel = 'easy' | 'normal' | 'hard';

export const BOT_LEVEL_LABEL: Record<BotLevel, string> = { easy: '쉬움', normal: '보통', hard: '어려움' };

/**
 * 봇의 흔들림 (표준편차): power = 세기에 곱하는 비율, putt = 퍼트 세기 비율, acc = 스윙 정확도 값(±1.2 범위),
 * aim = 조준 방향 (rad).
 */
export const BOT_NOISE: Record<BotLevel, { power: number; putt: number; acc: number; aim: number }> = {
  easy: { power: 0.1, putt: 0.12, acc: 0.3, aim: 0.012 },
  normal: { power: 0.05, putt: 0.06, acc: 0.15, aim: 0.006 },
  hard: { power: 0.02, putt: 0.025, acc: 0.05, aim: 0.002 },
};

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
  /** 봇이면 조준·클럽·스윙을 알아서 한다 */
  isBot?: boolean;
  /** 봇 난이도 (기본 normal) */
  level?: BotLevel;
  /** 핸디캡 (18홀 기준 타수, 0–54). 없으면 0 */
  handicap?: number;
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
  /** 지금 홀의 티샷 순서 (players 번호). 첫 홀은 이름 순, 그 뒤는 앞 홀 성적 순 */
  teeOrder: number[];
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
/** 돌풍 진폭 / 평균 바람 속도. 홀마다 정해지는 평균 바람 위에 이만큼의 일렁임이 얹힌다 (게임용 값) */
export const GUST_RATIO = 0.3;

/**
 * 스윙 애니메이션 길이 (s): 백스윙 → 다운스윙 → 임팩트(= 둘의 합) → 팔로스루.
 * 풀스윙은 합 0.64 s. 공은 임팩트에 떠난다 (shotClock 이 0 이 되는 때).
 */
export const SWING_TIMING = {
  full: { back: 0.3, down: 0.14, follow: 0.2 },
  putt: { back: 0.32, down: 0.16, follow: 0.14 },
} as const;

/** 샷 결정에서 임팩트까지 (s) */
export function impactDelayS(isPutt: boolean): number {
  const t = isPutt ? SWING_TIMING.putt : SWING_TIMING.full;
  return t.back + t.down;
}

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

/** 사람 슬롯 설정 (메뉴·시험). 문자열이면 이름만 */
export interface PlayerSpec {
  name: string;
  bot?: boolean;
  level?: BotLevel;
  handicap?: number;
}

function slot(spec: string | PlayerSpec, i: number, holes: number): PlayerSlot {
  const sp = typeof spec === 'string' ? { name: spec } : spec;
  const p: PlayerSlot = { name: sp.name, color: PLAYER_COLORS[i % PLAYER_COLORS.length]!, ball: teeBall(HOLES[0]!), strokes: 0, done: false, scores: Array.from({ length: holes }, () => null), shots: [] };
  if (sp.bot) {
    p.isBot = true;
    p.level = sp.level ?? 'normal';
  }
  if (sp.handicap !== undefined && sp.handicap > 0) p.handicap = Math.min(Math.round(sp.handicap), 54);
  return p;
}

export const game = createStore<GameState>({
  mode: 'stroke',
  course: [0, 1, 2],
  holePos: 0,
  holeIdx: 0,
  players: [slot('플레이어 1', 0, 3)],
  cur: 0,
  teeOrder: [0],
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

/**
 * 샷 애니메이션 시계 (초). 프레임 루프만 바꾼다.
 * 샷을 정하면 −impactDelay 로 시작하고, 0 이 되는 때가 임팩트(공이 떠남)다.
 */
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

/** 결정적 정규 난수 (표준편차 1, 평균 0) */
function gauss(seed: number, k: number): number {
  const u1 = Math.max(rand(seed, 2 * k), 1e-9);
  const u2 = rand(seed, 2 * k + 1);
  return Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
}

export function windFor(seed: number, holeIdx: number): Wind {
  const speed = 0.5 + 5 * rand(seed, 2 * holeIdx + 1);
  return { speed, dir: 2 * Math.PI * rand(seed, 2 * holeIdx + 2), gust: GUST_RATIO * speed };
}

/** 월드 바람을 조준 틀(+x = 조준 방향)로 */
export function windInAimFrame(w: Wind, yaw: number): { x: number; z: number } {
  const wx = w.speed * Math.cos(w.dir);
  const wz = w.speed * Math.sin(w.dir);
  return { x: wx * Math.cos(yaw) + wz * Math.sin(yaw), z: -wx * Math.sin(yaw) + wz * Math.cos(yaw) };
}

/** 월드 틀 바람 (굴림에 쓴다) */
export function windOnGround(w: Wind): GroundWind {
  return { x: w.speed * Math.cos(w.dir), z: w.speed * Math.sin(w.dir) };
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

/**
 * 이 자리에서 이 클럽을 쓸 수 있는가: 드라이버는 티에서만, 롱드라이브는 드라이버만.
 * 화면 버튼 비활성과 같은 규칙을 store 에서도 지킨다 (키보드·터치·콘솔 어디로 와도 막는다).
 */
export function clubAllowed(idx: number, s: GameState = game.get()): boolean {
  if (idx < 0 || idx >= CLUBS.length) return false;
  if (s.mode === 'longdrive' || s.holeIdx < 0) return idx === DRIVER;
  if (idx === DRIVER && s.ball.surface !== 'tee') return false;
  return true;
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

/**
 * 발사 조건으로 비행만 (조준·바람 반영, 월드 표본). 미리보기에도 쓴다.
 * gustSeed 를 주면 wind.gust 진폭의 돌풍을 그 씨앗으로 더한다 (미리보기는 주지 않아 평균 바람만 본다).
 */
export function flyFrom(input: ShotInput, ball: { x: number; z: number }, yaw: number, wind: Wind, gustSeed?: number): { result: FlightResult; samples: TrajectorySample[] } {
  const amp = wind.gust ?? 0;
  const result = simulate(toEngineLaunch(input), gustSeed !== undefined && amp > 0 ? { wind: windInAimFrame(wind, yaw), gust: { amp, seed: gustSeed } } : { wind: windInAimFrame(wind, yaw) });
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

/** 샷의 캐리 거리 (yd): 임팩트 자리에서 첫 착지(또는 나무에 맞은 자리)까지. 퍼트는 NaN */
export function shotCarryYd(shot: GameShot): number {
  if (shot.putt) return NaN;
  const land = shot.ground.samples[0]!;
  return Math.hypot(land.x - shot.origin.x, land.z - shot.origin.z) / M_PER_YD;
}

export function isBotTurn(s: GameState = game.get()): boolean {
  return s.players[s.cur]?.isBot === true;
}

// ------------------------------------------------------------------ 기록 (브라우저 저장소, 없어도 돈다)

export interface Records {
  stroke3?: number;
  stroke9?: number;
  ctp?: number;
  longdrive?: number;
}
const RECORD_KEY = 'golf-onefield-records';
const ROUNDS_KEY = 'golf-onefield-rounds';
/** 저장하는 지난 라운드 수 */
export const ROUNDS_KEPT = 30;

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

/** 한 사람의 라운드 결과 */
export interface RoundPlayerResult {
  name: string;
  bot: boolean;
  handicap: number;
  /** 스트로크: 홀별 타수 */
  scores: (number | null)[];
  /** 챌린지: 샷 기록 (m, 기록 없음은 null) */
  shots: (number | null)[];
  /** 총타수 (스트로크) */
  gross: number;
  /** 순타수 = 총타수 − 핸디캡 허용 타수 */
  net: number;
}

/** 끝난 라운드 한 건 (브라우저 저장소에 남긴다) */
export interface RoundRecord {
  /** ISO 시각 */
  when: string;
  mode: GameMode;
  holes: number;
  seed: number;
  par: number;
  players: RoundPlayerResult[];
}

/** 핸디캡 허용 타수 = round(핸디캡 × 친 홀 수 / 18) */
export function handicapAllowance(handicap: number, holes: number): number {
  return Math.round((Math.max(handicap, 0) * holes) / 18);
}

/** 끝난 게임 → 저장용 요약 (순수 함수) */
export function roundSummary(s: GameState, when = new Date().toISOString()): RoundRecord {
  const par = s.course.reduce((a, i) => a + HOLES[i]!.par, 0);
  const players = s.players.map((p, i) => {
    const scores = i === s.cur ? s.scores : p.scores;
    const gross = scores.reduce<number>((a, v) => a + (v ?? 0), 0);
    const hc = p.handicap ?? 0;
    return {
      name: p.name,
      bot: p.isBot === true,
      handicap: hc,
      scores: [...scores],
      shots: p.shots.map((v) => (Number.isFinite(v) ? v : null)),
      gross,
      net: s.mode === 'stroke' ? gross - handicapAllowance(hc, s.course.length) : gross,
    };
  });
  return { when, mode: s.mode, holes: s.course.length, seed: s.seed, par, players };
}

export function loadRounds(): RoundRecord[] {
  try {
    const v = JSON.parse(localStorage.getItem(ROUNDS_KEY) ?? '[]') as unknown;
    return Array.isArray(v) ? (v as RoundRecord[]) : [];
  } catch {
    return [];
  }
}

/** 끝난 라운드를 저장한다 (저장소가 막혀 있어도 게임은 그대로 돈다) */
function saveRound(r: RoundRecord): void {
  try {
    const list = [...loadRounds(), r].slice(-ROUNDS_KEPT);
    localStorage.setItem(ROUNDS_KEY, JSON.stringify(list));
  } catch {
    // 저장소를 못 쓰면 라운드 기록만 남지 않는다
  }
}

// ------------------------------------------------------------------ 시작·홀

export interface GameConfig {
  mode: GameMode;
  /** 스트로크: 3 또는 9 홀 */
  holes?: number;
  /** 이름만 주거나 봇·핸디캡을 같이 준다 */
  players?: (string | PlayerSpec)[];
  seed?: number;
  /** 홀 소개(공중 촬영)를 보여 줄지 */
  intro?: boolean;
}

export function startGame(cfg: GameConfig): void {
  const specs = (cfg.players && cfg.players.length > 0 ? cfg.players : ['플레이어 1']).slice(0, 4);
  const course = cfg.mode === 'stroke' ? Array.from({ length: Math.min(cfg.holes ?? 3, HOLES.length) }, (_, i) => i) : cfg.mode === 'ctp' ? [CTP_HOLE] : [];
  const seed = cfg.seed ?? (Date.now() >>> 0) % 100000;
  game.set({
    mode: cfg.mode,
    course,
    holePos: 0,
    players: specs.map((sp, i) => slot(sp, i, course.length)),
    cur: 0,
    teeOrder: specs.map((_, i) => i),
    attempt: 0,
    // 샷 번호도 새 게임에서 다시 센다: 같은 씨앗이면 같은 게임이 되게 (샷마다의 흔들림·돌풍 씨앗이 샷 번호에서 나온다)
    shotId: 0,
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
  clearIntroTimer();
  game.set({ phase: 'menu', overview: false, message: null });
}

/**
 * 티 순서 (아너): 앞 홀 점수가 낮은 사람부터, 같으면 앞 홀의 티 순서를 유지한다. 첫 홀(prevPos < 0)은 players 번호 순.
 * prevOrder 는 앞 홀의 티 순서.
 */
export function teeOrderFor(players: readonly PlayerSlot[], prevPos: number, prevOrder: readonly number[]): number[] {
  const idx = players.map((_, i) => i);
  if (prevPos < 0) return idx;
  const rank = (i: number) => {
    const r = prevOrder.indexOf(i);
    return r < 0 ? i : r;
  };
  return idx.sort((a, b) => (players[a]!.scores[prevPos] ?? 99) - (players[b]!.scores[prevPos] ?? 99) || rank(a) - rank(b));
}

export function startHole(pos: number, intro = true): void {
  const s = game.get();
  const holeIdx = s.mode === 'longdrive' ? -1 : s.course[pos]!;
  const h = holeIdx < 0 ? LONG_DRIVE : HOLES[holeIdx]!;
  const tee = teeBall(h);
  const players = s.players.map((p) => ({ ...p, ball: tee, strokes: 0, done: false }));
  const order = s.mode === 'stroke' ? teeOrderFor(players, pos - 1, s.teeOrder) : players.map((_, i) => i);
  const first = order[0]!;
  shotClock.t = 0;
  const title =
    s.mode === 'stroke'
      ? `${h.number}번 홀 · ${h.name} · 파 ${h.par} · ${Math.round(holeLengthM(h) / M_PER_YD)} yd`
      : s.mode === 'ctp'
        ? `니어핀 챌린지 · ${h.name} · ${Math.round(holeLengthM(h) / M_PER_YD)} yd · ${CHALLENGE_TRIES}번씩`
        : `롱드라이브 · 폭 50 m 그리드 · ${CHALLENGE_TRIES}번씩`;
  const t0 = now();
  game.set({
    holePos: pos,
    holeIdx,
    players,
    cur: first,
    teeOrder: order,
    ball: tee,
    strokes: 0,
    scores: players[first]!.scores,
    ballId: s.ballId + 1,
    clubIdx: suggestClub(tee, h),
    aimYaw: aimFor(tee, h),
    wind: windFor(s.seed, holeIdx < 0 ? 40 : holeIdx),
    phase: intro ? 'intro' : 'aim',
    introT0: t0,
    meter: IDLE,
    shot: null,
    message: msg(players.length > 1 ? `${title} · ${players[first]!.name} 먼저` : title),
    overview: false,
  });
  if (intro) armIntroTimer();
  else clearIntroTimer();
}

// 홀 소개 시간: 화면 프레임 고리(HUD 의 rAF)가 introTick 을 부르지만, 배경 탭에서는 rAF 가 멈춘다.
// 그래서 실시간(performance.now) 기준으로 같은 점검을 타이머로도 건다 (배경 탭 타이머는 느려도 돈다).
let introTimer: ReturnType<typeof setTimeout> | null = null;

function clearIntroTimer(): void {
  if (introTimer !== null) {
    clearTimeout(introTimer);
    introTimer = null;
  }
}

function armIntroTimer(): void {
  clearIntroTimer();
  const s = game.get();
  const wait = Math.max(INTRO_S * 1000 - (now() - s.introT0), 0) + 30;
  introTimer = setTimeout(() => {
    introTimer = null;
    introTick();
    if (game.get().phase === 'intro') armIntroTimer();
  }, wait);
  // 노드(시험)에서 타이머가 프로세스를 붙잡지 않게
  (introTimer as unknown as { unref?: () => void }).unref?.();
}

/** 홀 소개 끝내기 (시간이 다 되거나 Space·Enter) */
export function skipIntro(): void {
  clearIntroTimer();
  if (game.get().phase === 'intro') game.set({ phase: 'aim' });
}

/** 프레임마다(또는 타이머로): 소개 시간이 다 되면 조준으로. 시간은 performance.now 기준이라 프레임이 멈춰도 맞다 */
export function introTick(t = now()): void {
  const s = game.get();
  if (s.phase === 'intro' && t - s.introT0 >= INTRO_S * 1000) {
    clearIntroTimer();
    game.set({ phase: 'aim' });
  }
}

export function nextHole(): void {
  const s = game.get();
  if (s.phase === 'done') {
    startGame({
      mode: s.mode,
      holes: s.course.length,
      players: s.players.map((p) => ({ name: p.name, ...(p.isBot ? { bot: true, level: p.level ?? 'normal' } : {}), ...(p.handicap ? { handicap: p.handicap } : {}) })),
      intro: true,
    });
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
  // 기록(신기록)은 사람만 센다. 봇만 있으면 남기지 않는다
  const humans = s.players.filter((p) => !p.isBot);
  if (s.mode === 'stroke') {
    const par = s.course.reduce((a, i) => a + HOLES[i]!.par, 0);
    const best = humans.length > 0 ? Math.min(...humans.map((p) => p.scores.reduce<number>((a, v) => a + (v ?? 0), 0) - par)) : NaN;
    const key = s.course.length >= 9 ? 'stroke9' : 'stroke3';
    if (!Number.isNaN(best) && (rec[key] === undefined || best < rec[key]!)) {
      record = `${s.course.length}홀 최고 기록 ${best > 0 ? '+' : ''}${best}`;
      rec[key] = best;
    }
  } else {
    const all = humans.flatMap((p) => p.shots).filter((v) => Number.isFinite(v));
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
  saveRound(roundSummary(s));
  game.set({ phase: 'done', record, message: msg(record ? `신기록! ${record}` : '게임 끝', 'good') });
}

// ------------------------------------------------------------------ 조작

const NO_CLUB = '이 자리에서는 쓸 수 없는 클럽입니다';

/** 클럽 바꾸기: 쓸 수 없는 클럽(티 밖의 드라이버, 롱드라이브의 다른 클럽)은 건너뛴다 */
export function selectClub(delta: number): void {
  const s = game.get();
  if (s.phase !== 'aim' || isBotTurn(s) || delta === 0) return;
  const dir = delta > 0 ? 1 : -1;
  let i = s.clubIdx + delta;
  while (i >= 0 && i <= PUTTER && !clubAllowed(i, s)) i += dir;
  if (i < 0 || i > PUTTER) return;
  game.set({ clubIdx: i });
}

export function setClub(i: number): void {
  const s = game.get();
  if (s.phase !== 'aim' || isBotTurn(s)) return;
  const idx = Math.min(Math.max(Math.round(i), 0), PUTTER);
  if (!clubAllowed(idx, s)) {
    game.set({ message: msg(NO_CLUB, 'bad') });
    return;
  }
  game.set({ clubIdx: idx });
}

/** 조준 돌리기 (rad, + = 오른쪽) */
export function aimBy(d: number): void {
  const s = game.get();
  if (s.phase !== 'aim' || isBotTurn(s)) return;
  game.set({ aimYaw: s.aimYaw + d });
}

export function aimAtPin(): void {
  const s = game.get();
  if (s.phase !== 'aim' || isBotTurn(s)) return;
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
  if (isBotTurn(s) && (s.phase === 'aim' || s.phase === 'swing')) return;
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

/** 한 타 친다. measured 를 주면 그 발사 조건(실측)을 그대로 쓴다. 봇 차례에는 사람 입력으로 칠 수 없다 */
export function swing(sw: SwingInput, measured?: ShotInput): GameShot | null {
  if (isBotTurn()) return null;
  return performSwing(sw, measured);
}

/** 샷을 정해 비행/굴림을 미리 계산하고 애니메이션을 시작한다 (공은 임팩트에 떠난다) */
function performSwing(sw: SwingInput, measured?: ShotInput): GameShot | null {
  const s = game.get();
  if (s.phase !== 'aim' && s.phase !== 'swing') return null;
  const h = holeOf(s);
  const id = s.shotId + 1;
  const gw = windOnGround(s.wind);
  const isPutt = isPutter(s) && !measured;
  let shot: GameShot;
  if (isPutt) {
    const g = putt(h, s.ball.x, s.ball.z, s.aimYaw, sw.power, puttMaxFor(distToPinM(s)), gw);
    shot = { id, putt: true, origin: s.ball, yaw: s.aimYaw, clubIdx: s.clubIdx, player: s.cur, swing: sw, input: null, measured: false, flight: null, flightSamples: [], path: null, ground: g, tFlight: 0, tTotal: g.duration, impactDelay: impactDelayS(true), tree: null, gust: null };
  } else {
    if (!measured && !clubAllowed(s.clubIdx, s)) {
      game.set({ message: msg(NO_CLUB, 'bad') });
      return null;
    }
    const club = CLUBS[isPutter(s) ? PUTTER - 1 : s.clubIdx]!;
    const wobble = 2 * rand(s.seed + 7, id) - 1;
    const input = measured ?? launchFor(club, sw, s.ball.surface, wobble);
    const gust: GustOptions | null = (s.wind.gust ?? 0) > 0 ? { amp: s.wind.gust!, seed: (Math.imul(s.seed, 131) + Math.imul(id, 7919)) | 0 } : null;
    const { result, samples } = flyFrom(input, s.ball, s.aimYaw, s.wind, gust?.seed);
    if (!result.landed) {
      game.set({ phase: 'aim', meter: IDLE, message: msg('공이 떨어지지 않았습니다. 다시 쳐 주세요', 'bad') });
      return null;
    }
    // 나무: 맞으면 그 자리에서 비행이 끝나고 수직으로 떨어진다
    const hit = firstTreeHit(h.trees, samples);
    const flightSamples = hit ? cutAtTree(samples, hit) : samples;
    const end = flightSamples[flightSamples.length - 1]!;
    const backspin = hit ? 0 : result.landSpinRpm * Math.cos((input.spinAxisDeg * Math.PI) / 180);
    const g = bounceAndRoll(h, { x: end.x, z: end.z, vx: end.vx, vy: end.vy, vz: end.vz, backspinRpm: backspin }, gw);
    const path = resample(flightSamples, 1 / 120).xyz;
    const tFlight = hit ? end.t : result.timeS;
    shot = {
      id,
      putt: false,
      origin: s.ball,
      yaw: s.aimYaw,
      clubIdx: s.clubIdx,
      player: s.cur,
      swing: measured ? null : sw,
      input,
      measured: !!measured,
      flight: result,
      flightSamples,
      path,
      ground: g,
      tFlight,
      tTotal: tFlight + g.duration,
      impactDelay: impactDelayS(false),
      tree: hit ? { x: hit.x, y: hit.y, z: hit.z, t: hit.t } : null,
      gust,
    };
  }
  shotClock.t = -shot.impactDelay;
  game.set({ phase: shot.putt ? 'ground' : 'flight', shot, shotId: id, strokes: s.strokes + 1, meter: IDLE, message: null, overview: false });
  return shot;
}

/**
 * 애니메이션 시계를 끝으로 (시험·빨리 감기).
 * 비행 중이면 먼저 ground 로 넘겨 착지 효과(먼지·소리·물보라)가 평소와 같은 경로로 한 번 나오게 하고 나서 결과를 처리한다.
 */
export function skipAnimation(): void {
  const s = game.get();
  if (!s.shot || (s.phase !== 'flight' && s.phase !== 'ground')) return;
  if (s.phase === 'flight') game.set({ phase: 'ground' });
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
  if (shot.tree) return `나무에 맞음 · 캐리 ${Math.round(shotCarryYd(shot))} yd · ${where}`;
  return `캐리 ${Math.round(shotCarryYd(shot))} yd · 런 ${Math.round(shot.ground.runM / M_PER_YD)} yd · ${where}`;
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
  // 아직 티샷 안 한 사람이 있으면 이 홀의 티 순서(아너)대로, 아니면 핀에서 가장 먼 사람
  const rank = (i: number) => {
    const r = s.teeOrder.indexOf(i);
    return r < 0 ? i : r;
  };
  const teeing = open.filter(({ p }) => p.strokes === 0).sort((a, b) => rank(a.i) - rank(b.i));
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

// ------------------------------------------------------------------ 봇

/** 봇이 정한 한 타 */
export interface BotPlan {
  aimYaw: number;
  clubIdx: number;
  swing: SwingInput;
}

/**
 * 봇의 한 타 계획 (순수 함수: 같은 상태·씨앗이면 같은 계획):
 *   조준 — 티의 파4·5 는 조준점, 나머지는 핀 (+ 난이도별 방향 흔들림).
 *   클럽 — suggestClub (핀에 닿는 가장 짧은 클럽; 그린은 퍼터).
 *   세기 — 풀스윙이 모자라지 않으면 거리/캐리 비율 (티샷은 100 %), 퍼트는 굴림 거리에 1.12 배(지나치게 굴려 홀 뒤를 노린다).
 *   흔들림 — 난이도별 표준편차(BOT_NOISE)의 결정적 정규 난수, 키는 (씨앗, 샷 번호).
 */
export function botPlan(s: GameState = game.get()): BotPlan {
  const h = holeOf(s);
  const level = s.players[s.cur]?.level ?? 'normal';
  const noise = BOT_NOISE[level];
  const k = 4 * (s.shotId + 1);
  const nz = (j: number) => gauss(s.seed + 977, k + j);
  if (h.kind === 'longdrive') {
    return { aimYaw: noise.aim * nz(0), clubIdx: DRIVER, swing: { power: Math.min(Math.max(1 + noise.power * nz(1), 0.5), 1), accuracy: clampAcc(noise.acc * nz(2)) } };
  }
  const clubIdx = suggestClub(s.ball, h);
  const d = distToPinM(s);
  const atTee = s.ball.surface === 'tee' && h.par > 3;
  const aimYaw = (atTee ? yawTo(s.ball, h.teeTarget) : yawTo(s.ball, h.pin)) + noise.aim * nz(0);
  if (CLUBS[clubIdx]!.putter) {
    const max = puttMaxFor(d);
    const base = puttPowerFor(d, max) * 1.12 + 0.012 * (24 / max);
    return { aimYaw, clubIdx, swing: { power: Math.max(base * (1 + noise.putt * nz(1)), 0.02), accuracy: 0 } };
  }
  const carryM = fullCarryYd(CLUBS[clubIdx]!) * M_PER_YD;
  const base = atTee ? 1 : Math.min(Math.max((d * 0.95) / carryM, 0.2), 1);
  const power = Math.min(Math.max(base * (1 + noise.power * nz(1)), 0.2), 1);
  return { aimYaw, clubIdx, swing: { power, accuracy: clampAcc(noise.acc * nz(2)) } };
}

const clampAcc = (a: number) => Math.min(Math.max(a, -1.2), 1.2);

/** 봇 차례의 조준 단계이면 한 타 친다 (소개·홀 끝 넘김은 화면이 맡는다). 친 샷을 돌려준다 */
export function botTurn(): GameShot | null {
  const s = game.get();
  if (s.phase !== 'aim' || !isBotTurn(s)) return null;
  const plan = botPlan(s);
  game.set({ aimYaw: plan.aimYaw, clubIdx: plan.clubIdx });
  return performSwing(plan.swing);
}

/** 핀까지 거리에 맞춰 알아서 친다 (브라우저 시험·데모용 봇, 흔들림 없음) */
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
    performSwing({ power: 1, accuracy: 0 });
    return;
  }
  const d = distToPinM(s);
  const atTee = s.ball.surface === 'tee' && h.par > 3;
  game.set({ aimYaw: atTee ? yawTo(s.ball, h.teeTarget) : yawTo(s.ball, h.pin) });
  if (isPutter(s)) {
    performSwing({ power: (puttPowerFor(d, puttMaxFor(d)) * 1.12 + 0.012 * (24 / puttMaxFor(d))), accuracy: 0 });
    return;
  }
  const carryM = fullCarryYd(CLUBS[s.clubIdx]!) * M_PER_YD;
  const power = atTee ? 1 : Math.min(Math.max((d * 0.95) / carryM, 0.2), 1);
  performSwing({ power, accuracy: 0 });
}
