/**
 * 게임 HUD (DOM): 시작 메뉴, 홀 소개, 홀·타수, 사람별 점수, 핀까지·라이·바람, 클럽 줄, 스윙 미터, 샷 판정, 알림,
 * 실측 샷 넣기, 스코어카드·챌린지 순위. 스윙 미터·소개 시간은 requestAnimationFrame 고리에서 넘긴다.
 * 효과음은 상태가 바뀌는 순간(스윙·착지·홀컵·박수)에 낸다.
 */
import { useEffect, useRef, useState } from 'react';

import { parseScreenOutput } from '../../engine/parse.ts';
import { M_PER_YD } from '../../engine/units.ts';
import { CLUBS, fullCarryYd, PUTTER } from '../clubs.ts';
import { HOLES, holeLengthM, SURFACE_LABEL } from '../course.ts';
import { puttMaxFor } from '../ground.ts';
import {
  ACC_SPAN,
  CHALLENGE_TRIES,
  distToPinM,
  game,
  holeOf,
  INTRO_S,
  introTick,
  loadRecords,
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
  startGame,
  swing,
  toggleOverview,
  useGame,
  type GameMode,
  type GameState,
  type PlayerSlot,
} from '../round.ts';
import { ensureAudio, isMuted, playApplause, playClick, playCup, playLand, playPutt, playSwing, setMuted } from '../sound.ts';
import { MiniMap } from './MiniMap.tsx';
import './game.css';

const parOf = (s: GameState, pos: number) => HOLES[s.course[pos]!]!.par;

function toPar(scores: (number | null)[], s: GameState): string {
  let strokes = 0;
  let par = 0;
  scores.forEach((v, i) => {
    if (v !== null) {
      strokes += v;
      par += parOf(s, i);
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

// ------------------------------------------------------------------ 메뉴

function Menu({ onRange }: { onRange: () => void }) {
  const [mode, setMode] = useState<GameMode>('stroke');
  const [holes, setHoles] = useState(9);
  const [names, setNames] = useState(['플레이어 1', '플레이어 2', '플레이어 3', '플레이어 4']);
  const [count, setCount] = useState(1);
  const rec = loadRecords();
  const start = () => {
    ensureAudio();
    playClick();
    startGame({ mode, holes, players: names.slice(0, count).map((n, i) => n.trim() || `플레이어 ${i + 1}`), intro: true });
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
        <div className="g-menu-names">
          {names.slice(0, count).map((n, i) => (
            <input
              key={i}
              aria-label={`플레이어 ${i + 1} 이름`}
              value={n}
              maxLength={10}
              onChange={(e) => setNames(names.map((x, j) => (j === i ? e.target.value : x)))}
              style={{ borderColor: ['#ffd23f', '#ff6a8a', '#5ec8ff', '#9be37a'][i] }}
            />
          ))}
        </div>
        <button type="button" className="g-start" onClick={start} data-testid="g-start">
          시작
        </button>
        <div className="g-menu-records">
          기록 · 3홀 {rec.stroke3 === undefined ? '—' : rec.stroke3 > 0 ? `+${rec.stroke3}` : rec.stroke3 === 0 ? 'E' : rec.stroke3} · 9홀{' '}
          {rec.stroke9 === undefined ? '—' : rec.stroke9 > 0 ? `+${rec.stroke9}` : rec.stroke9 === 0 ? 'E' : rec.stroke9} · 니어핀{' '}
          {rec.ctp === undefined ? '—' : `${rec.ctp.toFixed(2)} m`} · 롱드라이브 {rec.longdrive === undefined ? '—' : `${rec.longdrive.toFixed(1)} yd`}
        </div>
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
  const s = useGame((x) => x);
  const h = holeOf(s);
  const p = s.players[s.cur]!;
  const multi = s.players.length > 1;
  return (
    <div className="panel g-hole" data-testid="g-hole">
      <div className="g-hole-no">
        {s.mode === 'stroke' ? h.number : s.mode === 'ctp' ? 'CTP' : 'LD'}
        {s.mode === 'stroke' && <span>번 홀</span>}
      </div>
      <div>
        <div className="g-hole-name">{s.mode === 'stroke' ? h.name : s.mode === 'ctp' ? `니어핀 · ${h.name}` : '롱드라이브 그리드'}</div>
        {s.mode === 'stroke' ? (
          <>
            <div className="g-hole-meta">
              파 {h.par} · {Math.round(holeLengthM(h) / M_PER_YD)} yd · {s.holePos + 1}/{s.course.length}
            </div>
            <div className="g-hole-meta">
              {multi && (
                <b className="g-who" style={{ color: p.color }}>
                  {p.name}{' '}
                </b>
              )}
              타수 <b data-testid="g-strokes">{s.strokes}</b> · 합계 <b>{toPar(s.scores, s)}</b>
            </div>
          </>
        ) : (
          <div className="g-hole-meta">
            {multi && (
              <b className="g-who" style={{ color: p.color }}>
                {p.name}{' '}
              </b>
            )}
            {Math.min(Math.floor(s.attempt / s.players.length) + 1, CHALLENGE_TRIES)}/{CHALLENGE_TRIES}번째 · 최고 <b>{fmtShot(bestShot(p, s.mode), s.mode)}</b>
          </div>
        )}
      </div>
    </div>
  );
}

function PlayersPanel() {
  const s = useGame((x) => x);
  if (s.players.length < 2 && s.mode === 'stroke') return null;
  const players = s.players.map((p, i) => (i === s.cur ? { ...p, ball: s.ball, strokes: s.strokes, scores: s.scores } : p));
  const h = holeOf(s);
  return (
    <div className="panel g-players" data-testid="g-players">
      {players.map((p, i) => (
        <div key={i} className={`g-player${i === s.cur ? ' on' : ''}`}>
          <i style={{ background: p.color }} />
          <span className="g-player-name">{p.name}</span>
          {s.mode === 'stroke' ? (
            <span className="g-player-val">
              {p.done ? '홀아웃' : p.strokes === 0 ? '티' : `${p.strokes}타 · ${Math.round(Math.hypot(h.pin[0] - p.ball.x, h.pin[1] - p.ball.z) / M_PER_YD)} yd`} · {toPar(p.scores, s)}
            </span>
          ) : (
            <span className="g-player-val">
              {p.shots.map((v) => fmtShot(v, s.mode)).join(' · ') || '—'}
            </span>
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
        </span>
      </div>
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
  const surface = useGame((s) => s.ball.surface);
  const longdrive = useGame((s) => s.mode === 'longdrive');
  return (
    <div className="g-clubs" role="radiogroup" aria-label="클럽">
      {CLUBS.map((c, i) => {
        const off = phase !== 'aim' || (c.id === '1w' && surface !== 'tee') || (longdrive && i !== 0);
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
  const hint =
    phase === 'swing'
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
        <button type="button" className="g-swing" onClick={() => (ensureAudio(), pressSwing())} disabled={phase !== 'aim' && phase !== 'swing'}>
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
          <button type="button" className="g-small" onClick={shoot} disabled={phase !== 'aim'}>
            이 값으로 치기
          </button>
          {note && <div className="g-note">{note}</div>}
        </>
      )}
    </div>
  );
}

function ScoreCard() {
  const s = useGame((x) => x);
  if (s.phase !== 'holed' && s.phase !== 'done') return null;
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
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>파</td>
              {holes.map((x) => (
                <td key={x.number}>{x.par}</td>
              ))}
              <td>{sumPar}</td>
            </tr>
            {players.map((p, pi) => (
              <tr key={pi}>
                <td style={{ color: p.color, fontWeight: 800 }}>{p.name}</td>
                {p.scores.map((v, i) => (
                  <td key={i} className={v === null ? '' : v < holes[i]!.par ? 'under' : v > holes[i]!.par ? 'over' : 'even'}>
                    {v ?? '·'}
                  </td>
                ))}
                <td>
                  {total(p)} <small>({toPar(p.scores, s)})</small>
                </td>
              </tr>
            ))}
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

/** 상태가 바뀌는 순간 효과음 */
function useGameAudio() {
  useEffect(() => {
    let prev = game.get();
    const unsub = game.subscribe(() => {
      const s = game.get();
      const sh = s.shot;
      if (sh && sh.id !== prev.shot?.id) {
        if (sh.putt) playPutt();
        else playSwing(sh.swing ? 1 - Math.min(Math.abs(sh.swing.accuracy), 1) : 0.8);
      }
      if (sh && prev.phase === 'flight' && s.phase === 'ground') playLand(sh.ground.firstSurface);
      if (sh && (prev.phase === 'flight' || prev.phase === 'ground') && s.phase !== 'flight' && s.phase !== 'ground') {
        if (sh.ground.event === 'holed') {
          playCup();
          if (s.mode === 'stroke') {
            const h = holeOf(s);
            const strokes = prev.strokes;
            playApplause(strokes <= h.par - 1 ? 1 : strokes === h.par ? 0.45 : 0.15);
          } else playApplause(1);
        } else if (sh.ground.event === 'water') {
          // 물소리는 착지 때 이미 냈다
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

