/**
 * 게임 HUD (DOM): 시작 메뉴(사람·봇·핸디캡), 홀 소개, 홀·타수, 사람별 점수, 핀까지·라이·바람·직전 샷, 클럽 줄, 스윙 미터, 샷 판정, 알림,
 * 실측 샷 넣기, 스코어카드(순타수 포함)·챌린지 순위, 화면 버튼(TouchControls). 스윙 미터는 requestAnimationFrame 고리에서 넘긴다.
 * 홀 소개 시간은 rAF 고리에서도 보지만 배경 탭에서 rAF 가 멈춰도 round.ts 의 타이머가 넘긴다 (performance.now 기준).
 * 효과음은 상태가 바뀌는 순간(스윙·착지·바운드·굴림·홀컵·박수)에 낸다: 타구음은 임팩트 시각에 맞춰 오디오 시계로 예약한다.
 * 봇 차례는 useBotDriver 가 잠깐 생각한 뒤 친다.
 * 구독은 필요한 칸만 고른다(useGame 에 통째로 넘기지 않는다): 미터·바람·메시지가 바뀔 때마다 모든 카드가 다시 그려지지 않게.
 */
import { useEffect, useRef, useState } from 'react';

import { parseScreenOutput } from '../../engine/parse.ts';
import { M_PER_YD } from '../../engine/units.ts';
import { CLUBS, fullCarryYd, PUTTER } from '../clubs.ts';
import { HOLES, holeLengthM, SURFACE_LABEL } from '../course.ts';
import { puttMaxFor, rollProfile } from '../ground.ts';
import {
  ACC_SPAN,
  BOT_LEVEL_LABEL,
  botTurn,
  CHALLENGE_TRIES,
  clubAllowed,
  distToPinM,
  game,
  handicapAllowance,
  holeOf,
  INTRO_S,
  introTick,
  isBotTurn,
  loadRecords,
  loadRounds,
  MAX_LATE,
  MAX_POWER,
  meterMarker,
  meterTick,
  nextHole,
  openMenu,
  PERFECT,
  pressSwing,
  scoreName,
  setClub,
  shotCarryYd,
  startGame,
  swing,
  toggleOverview,
  useGame,
  type BotLevel,
  type GameMode,
  type PlayerSlot,
  type PlayerSpec,
} from '../round.ts';
import { cancelScheduled, ensureAudio, isMuted, playApplause, playBounce, playClick, playCup, playLand, playPutt, playRoll, playSplash, playSwing, playTree, setMuted, setWindAmbient } from '../sound.ts';
import { MiniMap } from './MiniMap.tsx';
import { TouchControls } from './TouchControls.tsx';
import './game.css';

/** 코스 순서(course)와 점수(scores)로 파 대비 합 ('E', '+3', '-2') */
function toPar(scores: (number | null)[], course: number[]): string {
  let strokes = 0;
  let par = 0;
  scores.forEach((v, i) => {
    if (v !== null) {
      strokes += v;
      par += HOLES[course[i]!]!.par;
    }
  });
  const d = strokes - par;
  return d === 0 ? 'E' : d > 0 ? `+${d}` : `${d}`;
}

const total = (p: PlayerSlot) => p.scores.reduce<number>((a, v) => a + (v ?? 0), 0);
const bestShot = (p: PlayerSlot, mode: GameMode) => {
  const v = p.shots.filter((x) => Number.isFinite(x));
  if (v.length === 0) return NaN;
  return mode === 'ctp' ? Math.min(...v) : Math.max(...v);
};
const fmtShot = (v: number, mode: GameMode) => (Number.isNaN(v) ? '—' : mode === 'ctp' ? `${v.toFixed(2)} m` : `${(v / M_PER_YD).toFixed(1)} yd`);
const fmtSigned = (d: number) => (d === 0 ? 'E' : d > 0 ? `+${d}` : `${d}`);

// ------------------------------------------------------------------ 메뉴

interface Row {
  name: string;
  bot: boolean;
  level: BotLevel;
  handicap: number;
}

const DEFAULT_ROWS: Row[] = [0, 1, 2, 3].map((i) => ({ name: `플레이어 ${i + 1}`, bot: false, level: 'normal', handicap: 0 }));
const PLAYER_COLOR_HEX = ['#ffd23f', '#ff6a8a', '#5ec8ff', '#9be37a'];

function Menu({ onRange }: { onRange: () => void }) {
  const [mode, setMode] = useState<GameMode>('stroke');
  const [holes, setHoles] = useState(9);
  const [rows, setRows] = useState<Row[]>(DEFAULT_ROWS);
  const [count, setCount] = useState(1);
  const rec = loadRecords();
  const last = loadRounds().slice(-1)[0];
  const patch = (i: number, p: Partial<Row>) => setRows((r) => r.map((x, j) => (j === i ? { ...x, ...p } : x)));
  const start = () => {
    ensureAudio();
    playClick();
    const players: PlayerSpec[] = rows.slice(0, count).map((r, i) => ({
      name: r.name.trim() || (r.bot ? `봇 ${i + 1}` : `플레이어 ${i + 1}`),
      ...(r.bot ? { bot: true, level: r.level } : {}),
      ...(r.handicap > 0 ? { handicap: r.handicap } : {}),
    }));
    startGame({ mode, holes, players, intro: true });
  };
  const modes: [GameMode, string, string][] = [
    ['stroke', '스트로크 플레이', '9홀(또는 3홀)을 돌며 타수를 겨룬다. 여럿이면 먼 사람부터'],
    ['ctp', '니어핀 챌린지', `섬 그린 파3, ${CHALLENGE_TRIES}번씩 쳐서 핀에 가장 가깝게`],
    ['longdrive', '롱드라이브', `폭 50 m 그리드, ${CHALLENGE_TRIES}번씩 쳐서 가장 멀리 (OB 는 기록 없음)`],
  ];
  return (
    <div className="g-menu" data-testid="g-menu" onKeyDown={(e) => e.stopPropagation()}>
      <div className="g-menu-card panel">
        <h1>
          골프 원필드 <small>3D</small>
        </h1>
        <p className="g-menu-sub">장 하나 비행 엔진으로 날아가는 홈 스크린골프</p>
        <div className="g-menu-modes" role="radiogroup" aria-label="모드">
          {modes.map(([m, label, desc]) => (
            <button key={m} type="button" role="radio" aria-checked={mode === m} className={`g-mode-card${mode === m ? ' on' : ''}`} onClick={() => (setMode(m), playClick())}>
              <b>{label}</b>
              <span>{desc}</span>
            </button>
          ))}
          <button type="button" className="g-mode-card" onClick={onRange}>
            <b>연습장</b>
            <span>볼스피드·발사각·스핀을 넣어 쳐 보기</span>
          </button>
        </div>
        {mode === 'stroke' && (
          <div className="g-menu-row">
            <span>홀</span>
            {[3, 9].map((n) => (
              <button key={n} type="button" className={`g-pill${holes === n ? ' on' : ''}`} onClick={() => setHoles(n)}>
                {n}홀
              </button>
            ))}
          </div>
        )}
        <div className="g-menu-row">
          <span>사람</span>
          {[1, 2, 3, 4].map((n) => (
            <button key={n} type="button" className={`g-pill${count === n ? ' on' : ''}`} onClick={() => setCount(n)}>
              {n}명
            </button>
          ))}
        </div>
        <div className="g-menu-players" data-testid="g-menu-players">
          {rows.slice(0, count).map((r, i) => (
            <div key={i} className="g-menu-player">
              <input
                aria-label={`플레이어 ${i + 1} 이름`}
                value={r.name}
                maxLength={10}
                onChange={(e) => patch(i, { name: e.target.value })}
                style={{ borderColor: PLAYER_COLOR_HEX[i] }}
              />
              <button
                type="button"
                className={`g-pill g-bot-toggle${r.bot ? ' on' : ''}`}
                aria-pressed={r.bot}
                aria-label={`플레이어 ${i + 1} 봇`}
                onClick={() => patch(i, { bot: !r.bot, name: /^(플레이어|봇) \d$/.test(r.name) ? `${r.bot ? '플레이어' : '봇'} ${i + 1}` : r.name })}
              >
                봇
              </button>
              {r.bot && (
                <select aria-label={`봇 ${i + 1} 난이도`} value={r.level} onChange={(e) => patch(i, { level: e.target.value as BotLevel })}>
                  {(Object.keys(BOT_LEVEL_LABEL) as BotLevel[]).map((l) => (
                    <option key={l} value={l}>
                      {BOT_LEVEL_LABEL[l]}
                    </option>
                  ))}
                </select>
              )}
              {mode === 'stroke' && (
                <label className="g-hc">
                  핸디
                  <input
                    type="number"
                    inputMode="numeric"
                    min={0}
                    max={54}
                    aria-label={`플레이어 ${i + 1} 핸디캡`}
                    value={r.handicap}
                    onChange={(e) => patch(i, { handicap: Math.min(Math.max(Math.round(Number(e.target.value) || 0), 0), 54) })}
                  />
                </label>
              )}
            </div>
          ))}
        </div>
        <button type="button" className="g-start" onClick={start} data-testid="g-start">
          시작
        </button>
        <div className="g-menu-records">
          기록 · 3홀 {rec.stroke3 === undefined ? '—' : fmtSigned(rec.stroke3)} · 9홀 {rec.stroke9 === undefined ? '—' : fmtSigned(rec.stroke9)} · 니어핀{' '}
          {rec.ctp === undefined ? '—' : `${rec.ctp.toFixed(2)} m`} · 롱드라이브 {rec.longdrive === undefined ? '—' : `${rec.longdrive.toFixed(1)} yd`}
        </div>
        {last && (
          <div className="g-menu-records" data-testid="g-menu-last">
            지난 라운드 · {new Date(last.when).toLocaleDateString('ko-KR')} ·{' '}
            {last.players.map((p) => (last.mode === 'stroke' ? `${p.name} ${p.gross}${p.handicap > 0 ? ` (순 ${p.net})` : ''}` : p.name)).join(' / ')}
          </div>
        )}
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ 홀 소개

function IntroCard() {
  const phase = useGame((s) => s.phase);
  const s = game.get();
  if (phase !== 'intro') return null;
  const h = holeOf(s);
  const title = s.mode === 'longdrive' ? '롱드라이브' : s.mode === 'ctp' ? '니어핀 챌린지' : `${h.number}번 홀`;
  return (
    <div className="g-intro" data-testid="g-intro">
      <div className="g-intro-no">{title}</div>
      <div className="g-intro-name">{h.name}</div>
      {h.kind !== 'longdrive' && (
        <div className="g-intro-meta">
          파 {h.par} · {Math.round(holeLengthM(h) / M_PER_YD)} yd · 바람 {s.wind.speed.toFixed(1)} m/s
        </div>
      )}
      <div className="g-intro-skip">Space 로 건너뛰기</div>
      <div className="g-intro-bar" style={{ animationDuration: `${INTRO_S}s` }} />
    </div>
  );
}

// ------------------------------------------------------------------ 위 카드

function HoleCard() {
  const mode = useGame((s) => s.mode);
  const h = useGame(holeOf);
  const holePos = useGame((s) => s.holePos);
  const course = useGame((s) => s.course);
  const multi = useGame((s) => s.players.length > 1);
  const p = useGame((s) => s.players[s.cur]!);
  const strokes = useGame((s) => s.strokes);
  const scores = useGame((s) => s.scores);
  const tries = useGame((s) => Math.min(Math.floor(s.attempt / s.players.length) + 1, CHALLENGE_TRIES));
  const who = multi && (
    <b className="g-who" style={{ color: p.color }}>
      {p.isBot ? '🤖 ' : ''}
      {p.name}{' '}
    </b>
  );
  return (
    <div className="panel g-hole" data-testid="g-hole">
      <div className="g-hole-no">
        {mode === 'stroke' ? h.number : mode === 'ctp' ? 'CTP' : 'LD'}
        {mode === 'stroke' && <span>번 홀</span>}
      </div>
      <div>
        <div className="g-hole-name">{mode === 'stroke' ? h.name : mode === 'ctp' ? `니어핀 · ${h.name}` : '롱드라이브 그리드'}</div>
        {mode === 'stroke' ? (
          <>
            <div className="g-hole-meta">
              파 {h.par} · {Math.round(holeLengthM(h) / M_PER_YD)} yd · {holePos + 1}/{course.length}
            </div>
            <div className="g-hole-meta">
              {who}
              타수 <b data-testid="g-strokes">{strokes}</b> · 합계 <b>{toPar(scores, course)}</b>
            </div>
          </>
        ) : (
          <div className="g-hole-meta">
            {who}
            {tries}/{CHALLENGE_TRIES}번째 · 최고 <b>{fmtShot(bestShot(p, mode), mode)}</b>
          </div>
        )}
      </div>
    </div>
  );
}

function PlayersPanel() {
  const mode = useGame((s) => s.mode);
  const h = useGame(holeOf);
  const course = useGame((s) => s.course);
  const slots = useGame((s) => s.players);
  const cur = useGame((s) => s.cur);
  const ball = useGame((s) => s.ball);
  const strokes = useGame((s) => s.strokes);
  const scores = useGame((s) => s.scores);
  if (slots.length < 2 && mode === 'stroke') return null;
  const players = slots.map((p, i) => (i === cur ? { ...p, ball, strokes, scores } : p));
  return (
    <div className="panel g-players" data-testid="g-players">
      {players.map((p, i) => (
        <div key={i} className={`g-player${i === cur ? ' on' : ''}`}>
          <i style={{ background: p.color }} />
          <span className="g-player-name">
            {p.isBot ? '🤖 ' : ''}
            {p.name}
          </span>
          {mode === 'stroke' ? (
            <span className="g-player-val">
              {p.done ? '홀아웃' : p.strokes === 0 ? '티' : `${p.strokes}타 · ${Math.round(Math.hypot(h.pin[0] - p.ball.x, h.pin[1] - p.ball.z) / M_PER_YD)} yd`} · {toPar(p.scores, course)}
            </span>
          ) : (
            <span className="g-player-val">{p.shots.map((v) => fmtShot(v, mode)).join(' · ') || '—'}</span>
          )}
        </div>
      ))}
    </div>
  );
}

function windText(rel: number, speed: number): string {
  const tail = Math.cos(rel) * speed;
  const cross = Math.sin(rel) * speed;
  const parts: string[] = [];
  if (tail > 0.6) parts.push('뒷바람');
  else if (tail < -0.6) parts.push('앞바람');
  if (cross > 0.6) parts.push('오른쪽으로');
  else if (cross < -0.6) parts.push('왼쪽으로');
  return parts.length ? parts.join(' · ') : '거의 없음';
}

/** 직전 샷: 볼스피드·발사각·스핀·캐리 (퍼트는 굴린 거리) */
function LastShot() {
  const shot = useGame((s) => s.shot);
  if (!shot) return null;
  if (shot.putt) {
    return (
      <div className="g-last" data-testid="g-last">
        <div className="g-row">
          <span className="g-label">직전 퍼트</span>
          <span>{shot.ground.runM.toFixed(1)} m</span>
        </div>
      </div>
    );
  }
  const input = shot.input;
  if (!input) return null;
  const carry = shotCarryYd(shot);
  return (
    <div className="g-last" data-testid="g-last">
      <div className="g-row">
        <span className="g-label">직전 샷</span>
        <span className="g-last-club">{shot.measured ? '실측' : CLUBS[shot.clubIdx]!.short}</span>
      </div>
      <div className="g-last-grid">
        <span>
          <small>발사각</small>
          <b data-testid="g-last-launch">{`${input.launchDeg.toFixed(1)}°`}</b>
        </span>
        <span>
          <small>스핀</small>
          <b data-testid="g-last-spin">{`${Math.round(input.rpm)} rpm`}</b>
        </span>
        <span>
          <small>볼스피드</small>
          <b>{`${input.mph.toFixed(0)} mph`}</b>
        </span>
        <span>
          <small>캐리</small>
          <b data-testid="g-last-carry">{`${Math.round(carry)} yd`}</b>
        </span>
      </div>
    </div>
  );
}

function InfoCard() {
  const ball = useGame((s) => s.ball);
  const wind = useGame((s) => s.wind);
  const yaw = useGame((s) => s.aimYaw);
  const h = useGame(holeOf);
  const d = Math.hypot(h.pin[0] - ball.x, h.pin[1] - ball.z);
  const rel = wind.dir - yaw;
  return (
    <div className="panel g-info" data-testid="g-info">
      <div className="g-row">
        <span className="g-label">{h.kind === 'longdrive' ? '목표' : '핀까지'}</span>
        <span className="g-big" data-testid="g-dist">
          {h.kind === 'longdrive' ? '최대한 멀리' : ball.surface === 'green' ? `${d.toFixed(1)} m` : `${Math.round(d / M_PER_YD)} yd`}
        </span>
      </div>
      <div className="g-row">
        <span className="g-label">라이</span>
        <span className={`g-lie g-lie-${ball.surface}`}>{SURFACE_LABEL[ball.surface]}</span>
      </div>
      <div className="g-row">
        <span className="g-label">바람</span>
        <span className="g-wind">
          <span className="g-wind-arrow" style={{ transform: `rotate(${(rel * 180) / Math.PI}deg)` }}>
            ↑
          </span>
          {wind.speed.toFixed(1)} m/s <small>{windText(rel, wind.speed)}</small>
          {(wind.gust ?? 0) > 0.05 && <small> · 돌풍 ±{(wind.gust ?? 0).toFixed(1)}</small>}
        </span>
      </div>
      <LastShot />
    </div>
  );
}

function Toast() {
  const message = useGame((s) => s.message);
  const [shown, setShown] = useState<number | null>(null);
  useEffect(() => {
    if (!message) return;
    setShown(message.id);
    const t = window.setTimeout(() => setShown((v) => (v === message.id ? null : v)), 3400);
    return () => window.clearTimeout(t);
  }, [message]);
  if (!message || shown !== message.id) return null;
  return (
    <div className={`g-toast g-toast-${message.tone}`} data-testid="g-toast">
      {message.text}
    </div>
  );
}

/** 스윙 판정 (정타·훅·슬라이스·오버스윙) */
function Rating() {
  const shot = useGame((s) => s.shot);
  const [shown, setShown] = useState<{ id: number; text: string; sub: string; tone: string } | null>(null);
  useEffect(() => {
    if (!shot || shot.putt || !shot.swing) return;
    const { power, accuracy } = shot.swing;
    let text = '굿 샷';
    let tone = 'good';
    if (accuracy === 0 && power >= 0.95 && power <= 1.0001) text = 'PERFECT!';
    else if (accuracy === 0) text = '나이스 샷!';
    else if (accuracy > 0.15) ((text = '슬라이스'), (tone = 'bad'));
    else if (accuracy < -0.15) ((text = '훅'), (tone = 'bad'));
    if (power > 1.0001) ((text = `오버스윙! ${text === 'PERFECT!' ? '' : text}`), (tone = 'bad'));
    setShown({ id: shot.id, text, sub: `파워 ${Math.round(power * 100)} %`, tone });
    const t = window.setTimeout(() => setShown((v) => (v?.id === shot.id ? null : v)), 1600);
    return () => window.clearTimeout(t);
  }, [shot]);
  if (!shown) return null;
  return (
    <div className={`g-rating g-rating-${shown.tone}`} data-testid="g-rating" key={shown.id}>
      {shown.text}
      <small>{shown.sub}</small>
    </div>
  );
}

function ClubBar() {
  const clubIdx = useGame((s) => s.clubIdx);
  const phase = useGame((s) => s.phase);
  // 클럽마다 쓸 수 있는지를 한 문자열로 (원시값이라 바뀔 때만 다시 그린다)
  const allowed = useGame((s) => CLUBS.map((_, i) => (clubAllowed(i, s) ? '1' : '0')).join(''));
  const bot = useGame((s) => isBotTurn(s));
  return (
    <div className="g-clubs" role="radiogroup" aria-label="클럽">
      {CLUBS.map((c, i) => {
        const off = phase !== 'aim' || bot || allowed[i] === '0';
        return (
          <button
            key={c.id}
            type="button"
            role="radio"
            aria-checked={i === clubIdx}
            className={`g-club${i === clubIdx ? ' on' : ''}`}
            disabled={off && i !== clubIdx}
            onClick={() => setClub(i)}
            title={c.name}
          >
            <b>{c.short}</b>
            <span>{c.putter ? '—' : Math.round(fullCarryYd(c))}</span>
          </button>
        );
      })}
    </div>
  );
}

/** 미터 눈금: 값 v → 막대 안 % */
function meterPct(v: number, putter: boolean): number {
  const lo = putter ? 0 : MAX_LATE;
  return ((v - lo) / (MAX_POWER - lo)) * 100;
}

function SwingMeterView() {
  const putter = useGame((s) => CLUBS[s.clubIdx]!.putter === true);
  const phase = useGame((s) => s.phase);
  const bot = useGame((s) => isBotTurn(s));
  const marker = useRef<HTMLDivElement>(null);
  const fill = useRef<HTMLDivElement>(null);
  const label = useRef<HTMLSpanElement>(null);
  const shown = useRef({ v: 0, p: 0 });
  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const t = performance.now();
      meterTick(t);
      introTick(t);
      const s = game.get();
      const pt = CLUBS[s.clubIdx]!.putter === true;
      if (s.phase === 'swing') {
        const v = meterMarker(s.meter, t);
        shown.current = { v, p: s.meter.stage === 2 ? s.meter.power : v };
      } else if (s.phase === 'aim') {
        shown.current = { v: 0, p: 0 };
      }
      const { v, p } = shown.current;
      const zero = meterPct(0, pt);
      if (marker.current) marker.current.style.left = `${meterPct(v, pt)}%`;
      if (fill.current) {
        fill.current.style.left = `${zero}%`;
        fill.current.style.width = `${Math.max(meterPct(p, pt) - zero, 0)}%`;
      }
      if (label.current) label.current.textContent = pt ? `${(p * puttMaxFor(distToPinM(s))).toFixed(1)} m` : `${Math.round(p * 100)} %`;
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);
  const zero = meterPct(0, putter);
  const full = meterPct(1, putter);
  const zone = putter ? 0 : meterPct(PERFECT, false) - zero;
  const hint = bot
    ? '봇이 칩니다'
    : phase === 'swing'
      ? putter
        ? 'Space: 세기 정하기'
        : '한 번 더 Space: 파워 → 정확도 (가운데 흰 띠에서)'
      : putter
        ? `Space 두 번: 시작 → 세기 (100 % = 평평한 그린 ${puttMaxFor(distToPinM(game.get())).toFixed(1)} m)`
        : 'Space 세 번: 시작 → 파워 → 정확도';
  return (
    <div className="g-meter-wrap">
      <div className={`g-meter${putter ? ' putt' : ''}`} data-testid="g-meter">
        <div className="g-meter-fill" ref={fill} />
        {!putter && <div className="g-meter-zone" style={{ left: `${zero - zone}%`, width: `${2 * zone}%` }} />}
        <div className="g-meter-full" style={{ left: `${full}%` }} />
        <div className="g-meter-marker" ref={marker} />
        {!putter && (
          <>
            <span className="g-meter-tag hook" style={{ left: `${meterPct(-ACC_SPAN * 0.6, false)}%` }}>
              훅
            </span>
            <span className="g-meter-tag slice" style={{ left: `${meterPct(ACC_SPAN * 0.6, false)}%` }}>
              슬라이스
            </span>
          </>
        )}
      </div>
      <div className="g-meter-text">
        <span className="g-meter-value" ref={label} data-testid="g-meter-value" />
        <span className="g-meter-hint">{hint}</span>
        <button type="button" className="g-swing" onClick={() => (ensureAudio(), pressSwing())} disabled={(phase !== 'aim' && phase !== 'swing') || bot}>
          스윙
        </button>
      </div>
    </div>
  );
}

function MeasuredShot() {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState('');
  const [note, setNote] = useState<string | null>(null);
  const phase = useGame((s) => s.phase);
  const bot = useGame((s) => isBotTurn(s));
  const shoot = () => {
    const p = parseScreenOutput(text);
    const v = p.values;
    if (v.mph === undefined || v.launchDeg === undefined || v.rpm === undefined) {
      setNote(`못 읽음: ${p.missing.join(', ')}`);
      return;
    }
    const shot = swing({ power: 1, accuracy: 0 }, { mph: v.mph, launchDeg: v.launchDeg, rpm: v.rpm, azimuthDeg: v.azimuthDeg ?? 0, spinAxisDeg: v.spinAxisDeg ?? 0 });
    setNote(shot ? `실측으로 침: ${v.mph} mph · ${v.launchDeg}° · ${v.rpm} rpm · 축 ${v.spinAxisDeg ?? 0}° (트랙맨 부호)` : '지금은 칠 수 없습니다');
  };
  return (
    <div className="panel g-measured">
      <button type="button" className="g-link" onClick={() => setOpen(!open)} aria-expanded={open}>
        {open ? '▾' : '▸'} 실측 샷으로 치기 (벽 반동 screen 출력)
      </button>
      {open && (
        <>
          <textarea
            aria-label="screen 출력"
            rows={3}
            placeholder={'볼스피드 120.0 mph, 발사각 16.30°, 방위각 0.01°\n스핀 7061 rpm, 스핀축 8.0°, 캐리 168.2 yd'}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.stopPropagation()}
          />
          <button type="button" className="g-small" onClick={shoot} disabled={phase !== 'aim' || bot}>
            이 값으로 치기
          </button>
          {note && <div className="g-note">{note}</div>}
        </>
      )}
    </div>
  );
}

/** 스코어카드: 홀·끝 단계에서만 그린다 (바깥은 phase 만 구독해 평소에는 다시 그리지 않는다) */
function ScoreCard() {
  const phase = useGame((s) => s.phase);
  if (phase !== 'holed' && phase !== 'done') return null;
  return <ScoreCardBody />;
}

function ScoreCardBody() {
  const s = useGame((x) => x);
  const done = s.phase === 'done';
  const players = s.players.map((p, i) => (i === s.cur ? { ...p, scores: s.scores } : p));
  if (s.mode !== 'stroke') {
    const ranked = players
      .map((p) => ({ p, best: bestShot(p, s.mode) }))
      .sort((a, b) => (Number.isNaN(a.best) ? 1 : Number.isNaN(b.best) ? -1 : s.mode === 'ctp' ? a.best - b.best : b.best - a.best));
    return (
      <div className="panel g-card" data-testid="g-card">
        <h2>{s.mode === 'ctp' ? '니어핀 결과' : '롱드라이브 결과'}</h2>
        <table>
          <tbody>
            {ranked.map(({ p, best }, i) => (
              <tr key={p.name + i}>
                <td>{i + 1}위</td>
                <td style={{ color: p.color, fontWeight: 800 }}>{p.name}</td>
                <td>{p.shots.map((v) => fmtShot(v, s.mode)).join(' · ')}</td>
                <td className="under">{fmtShot(best, s.mode)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {s.record && <div className="g-record">신기록! {s.record}</div>}
        <div className="g-card-buttons">
          <button type="button" className="g-next" onClick={nextHole} autoFocus>
            다시 하기 (Enter)
          </button>
          <button type="button" className="g-small" onClick={openMenu}>
            메뉴
          </button>
        </div>
      </div>
    );
  }
  const holes = s.course.map((i) => HOLES[i]!);
  const sumPar = holes.reduce((a, h) => a + h.par, 0);
  const h = holeOf(s);
  const last = s.holePos === s.course.length - 1;
  const hasHc = players.some((p) => (p.handicap ?? 0) > 0);
  return (
    <div className="panel g-card" data-testid="g-card">
      <h2>{done ? '라운드 끝' : `${h.number}번 홀 끝`}</h2>
      <div className="g-card-scroll">
        <table>
          <thead>
            <tr>
              <th>홀</th>
              {holes.map((x) => (
                <th key={x.number}>{x.number}</th>
              ))}
              <th>합계</th>
              {hasHc && <th>순타수</th>}
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>파</td>
              {holes.map((x) => (
                <td key={x.number}>{x.par}</td>
              ))}
              <td>{sumPar}</td>
              {hasHc && <td />}
            </tr>
            {players.map((p, pi) => {
              const gross = total(p);
              const net = gross - handicapAllowance(p.handicap ?? 0, s.course.length);
              return (
                <tr key={pi}>
                  <td style={{ color: p.color, fontWeight: 800 }}>
                    {p.isBot ? '🤖 ' : ''}
                    {p.name}
                    {(p.handicap ?? 0) > 0 && <small> H{p.handicap}</small>}
                  </td>
                  {p.scores.map((v, i) => (
                    <td key={i} className={v === null ? '' : v < holes[i]!.par ? 'under' : v > holes[i]!.par ? 'over' : 'even'}>
                      {v ?? '·'}
                    </td>
                  ))}
                  <td>
                    {gross} <small>({toPar(p.scores, s.course)})</small>
                  </td>
                  {hasHc && (
                    <td data-testid="g-net">
                      {net} <small>({fmtSigned(net - sumPar)})</small>
                    </td>
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {!done &&
        players.map((p, i) => {
          const v = p.scores[s.holePos];
          return v === null || v === undefined ? null : (
            <div key={i} className="g-card-line">
              <b style={{ color: p.color }}>{p.name}</b> {scoreName(v, h.par)} ({v}타)
            </div>
          );
        })}
      {s.record && <div className="g-record">신기록! {s.record}</div>}
      <div className="g-card-buttons">
        <button type="button" className="g-next" onClick={nextHole} autoFocus>
          {done ? '다시 하기 (Enter)' : last ? '결과 보기 (Enter)' : '다음 홀 (Enter)'}
        </button>
        {done && (
          <button type="button" className="g-small" onClick={openMenu}>
            메뉴
          </button>
        )}
      </div>
    </div>
  );
}

/**
 * 상태가 바뀌는 순간 효과음.
 *   새 샷: 타구음을 임팩트 시각(shot.impactDelay)에 오디오 시계로 예약 (+ 나무에 맞는 소리). 앞 샷의 남은 예약 소리는 끊는다.
 *   착지(flight → ground): 착지음 + 바운드 소리들 + 굴림 소리를 지면 표본의 시각에 예약. 빨리 감기로 넘어가도 이 전환이 한 번 거친다.
 *   샷이 끝남: 예약 소리를 끊고 홀컵·박수·물 소리를 낸다.
 */
function useGameAudio() {
  useEffect(() => {
    let prev = game.get();
    const unsub = game.subscribe(() => {
      const s = game.get();
      const sh = s.shot;
      // 배속이 있으면 같은 비율로 당긴다 (지금은 1)
      const k = 1 / Math.max(s.timeScale, 0.1);
      if (sh && sh.id !== prev.shot?.id) {
        cancelScheduled();
        const at = sh.impactDelay * k;
        if (sh.putt) {
          playPutt(at);
          const prof = rollProfile(sh.ground);
          if (prof.length > 0) playRoll(prof, sh.ground.surface, at + prof[0]!.t * k);
        } else {
          playSwing(sh.swing ? 1 - Math.min(Math.abs(sh.swing.accuracy), 1) : 0.8, at);
          if (sh.tree) playTree((sh.impactDelay + sh.tree.t) * k);
        }
      }
      if (sh && !sh.putt && prev.phase === 'flight' && s.phase === 'ground') {
        playLand(sh.ground.firstSurface);
        for (const hit of sh.ground.hits) playBounce(hit.vn, sh.ground.firstSurface, hit.t * k);
        const prof = rollProfile(sh.ground);
        if (prof.length > 0) playRoll(prof, sh.ground.surface, prof[0]!.t * k);
      }
      if (sh && (prev.phase === 'flight' || prev.phase === 'ground') && s.phase !== 'flight' && s.phase !== 'ground') {
        cancelScheduled();
        if (sh.ground.event === 'holed') {
          playCup();
          if (s.mode === 'stroke') {
            const h = holeOf(s);
            const strokes = prev.strokes;
            playApplause(strokes <= h.par - 1 ? 1 : strokes === h.par ? 0.45 : 0.15);
          } else playApplause(1);
        } else if (sh.ground.event === 'water') {
          // 물에 바로 떨어졌으면 착지 때 이미 났고, 굴러서 빠졌으면 여기서
          if (sh.ground.firstSurface !== 'water') playSplash();
        } else if (s.mode === 'ctp' && sh.ground.event === 'rest' && Math.hypot(holeOf(s).pin[0] - sh.ground.rest.x, holeOf(s).pin[1] - sh.ground.rest.z) < 3) {
          playApplause(0.6);
        }
      }
      if (s.phase === 'done' && prev.phase !== 'done' && s.record) playApplause(1);
      prev = s;
    });
    const wake = () => ensureAudio();
    window.addEventListener('pointerdown', wake);
    window.addEventListener('keydown', wake);
    return () => {
      unsub();
      window.removeEventListener('pointerdown', wake);
      window.removeEventListener('keydown', wake);
    };
  }, []);
}

/** 배경 바람 소리: 메뉴에서는 약하게, 코스에서는 이 홀의 바람 속도에 맞춘다 */
function useWindAmbient() {
  const speed = useGame((s) => (s.phase === 'menu' ? 1 : s.wind.speed));
  useEffect(() => {
    setWindAmbient(speed);
  }, [speed]);
}

/**
 * 봇 차례를 진행한다: 조준 단계에 봇 차례가 되면 잠깐(0.9 s) 생각한 뒤 친다.
 * 모두 봇이면 홀이 끝난 뒤에도 2.6 s 뒤 스스로 다음 홀로 넘어간다 (사람이 있으면 사람이 누른다).
 * 시간은 setTimeout (배경 탭에서도 돈다). 같은 차례에서 상태가 여러 번 바뀌어도 타이머는 한 번만 건다.
 */
function useBotDriver() {
  useEffect(() => {
    let timer: number | undefined;
    let armedFor = '';
    const arm = () => {
      const s = game.get();
      const allBots = s.players.length > 0 && s.players.every((p) => p.isBot);
      let key = '';
      let wait = 0;
      let run: (() => void) | null = null;
      if (s.phase === 'aim' && isBotTurn(s)) {
        key = `aim:${s.holePos}:${s.cur}:${s.ballId}:${s.attempt}`;
        wait = 900;
        run = () => void botTurn();
      } else if (allBots && s.phase === 'holed') {
        key = `holed:${s.holePos}`;
        wait = 2600;
        run = nextHole;
      }
      if (key === armedFor) return;
      armedFor = key;
      window.clearTimeout(timer);
      timer = undefined;
      if (run) {
        const fn = run;
        timer = window.setTimeout(fn, wait);
      }
    };
    arm();
    const unsub = game.subscribe(arm);
    return () => {
      unsub();
      window.clearTimeout(timer);
    };
  }, []);
}

function MuteButton() {
  const [m, setM] = useState(isMuted());
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code === 'KeyM' && !(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) {
        setMuted(!isMuted());
        setM(isMuted());
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  return (
    <button type="button" className="g-mute" onClick={(e) => (setMuted(!m), setM(!m), e.currentTarget.blur())} title="소리 (M)" aria-label={m ? '소리 켜기' : '소리 끄기'}>
      {m ? '🔇' : '🔊'}
    </button>
  );
}

export function GameHud({ onRange }: { onRange: () => void }) {
  const phase = useGame((s) => s.phase);
  const overview = useGame((s) => s.overview);
  const clubIdx = useGame((s) => s.clubIdx);
  useGameAudio();
  useWindAmbient();
  useBotDriver();
  if (phase === 'menu') {
    return (
      <div className="hud g-hud">
        <Menu onRange={onRange} />
        <MuteButton />
      </div>
    );
  }
  return (
    <div className="hud g-hud">
      <HoleCard />
      <PlayersPanel />
      <InfoCard />
      <MiniMap />
      <MeasuredShot />
      <IntroCard />
      <Toast />
      <Rating />
      <ScoreCard />
      <MuteButton />
      <TouchControls />
      <div className="g-bottom panel">
        <ClubBar />
        <SwingMeterView />
        <div className="g-keys">
          <span>
            <kbd>←</kbd>
            <kbd>→</kbd> 조준{clubIdx === PUTTER ? ' (Shift 크게)' : ''}
          </span>
          <span>
            <kbd>↑</kbd>
            <kbd>↓</kbd> 클럽
          </span>
          <span>
            <kbd>P</kbd> 핀 조준
          </span>
          <span>
            <kbd>V</kbd> {overview ? '공 시점' : '홀 전체'}
          </span>
          <span>
            <kbd>F</kbd> 빨리 감기
          </span>
          <span>
            <kbd>M</kbd> 소리
          </span>
          <span className="g-keys-touch">화면을 좌우로 끌면 조준</span>
          <button type="button" className="g-link" onClick={toggleOverview}>
            {overview ? '공 시점으로' : '홀 전체 보기'}
          </button>
          <button type="button" className="g-link" onClick={openMenu}>
            메뉴
          </button>
        </div>
      </div>
    </div>
  );
}
