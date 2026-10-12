/**
 * 월드 공간 패널(XR)에 적을 글: HUD 는 DOM 이라 몰입 세션 안에서 보이지 않으므로, 같은 정보를 캔버스 질감 한 장에 그린다.
 * 여기는 순수 함수(글 만들기)와 2D 컨텍스트에 그리는 함수만 있다 (장면 연결은 scene/XrBridge.tsx).
 */
import { M_PER_YD } from '../../engine/units.ts';
import { CLUBS } from '../clubs.ts';
import { HOLES, SURFACE_LABEL } from '../course.ts';
import { distToPinM, holeOf, meterMarker, shotCarryYd, type GameState } from '../round.ts';

export interface PanelModel {
  /** 맨 위 줄: 홀·타수 */
  title: string;
  /** 가운데 큰 줄: 직전 결과 메시지 (없으면 지금 할 일) */
  headline: string;
  tone: 'info' | 'good' | 'bad';
  /** 아래 작은 줄들 */
  lines: string[];
  /** 스윙 미터 (0..1.05, 스윙 중일 때만) */
  meter: { marker: number; power: number; stage: 1 | 2 } | null;
}

const PHASE_HINT: Record<GameState['phase'], string> = {
  menu: '메뉴',
  intro: '홀 소개 · 트리거로 건너뛰기',
  aim: '트리거: 스윙 시작 · 스틱 좌우: 조준 · 스틱 상하: 클럽',
  swing: '트리거: 파워 → 정확도',
  flight: '',
  ground: '',
  holed: '트리거: 다음 홀',
  done: '라운드 끝 · 트리거: 다시 하기',
};

/** 상태 → 패널 글 (순수) */
export function panelModel(s: GameState, now: number): PanelModel {
  const h = holeOf(s);
  const p = s.players[s.cur];
  const who = s.players.length > 1 && p ? `${p.name} · ` : '';
  const title =
    s.mode === 'stroke'
      ? `${who}${h.number}번 홀 ${h.name} · 파 ${h.par} · ${s.strokes}타`
      : s.mode === 'ctp'
        ? `${who}니어핀 · ${s.attempt + 1}번째`
        : `${who}롱드라이브 · ${s.attempt + 1}번째`;
  const lines: string[] = [];
  if (s.phase !== 'menu' && s.phase !== 'intro') {
    const d = distToPinM(s);
    lines.push(h.kind === 'longdrive' ? `라이 ${SURFACE_LABEL[s.ball.surface]}` : `핀까지 ${s.ball.surface === 'green' ? `${d.toFixed(1)} m` : `${Math.round(d / M_PER_YD)} yd`} · 라이 ${SURFACE_LABEL[s.ball.surface]}`);
    const rel = s.wind.dir - s.aimYaw;
    const tail = Math.cos(rel) * s.wind.speed;
    const cross = Math.sin(rel) * s.wind.speed;
    lines.push(`바람 ${s.wind.speed.toFixed(1)} m/s (${tail >= 0 ? '뒤' : '앞'} ${Math.abs(tail).toFixed(1)} · ${cross >= 0 ? '오른쪽' : '왼쪽'} ${Math.abs(cross).toFixed(1)}) · 클럽 ${CLUBS[s.clubIdx]!.name}`);
  }
  const sh = s.shot;
  if (sh && !sh.putt && sh.input && s.phase !== 'intro') {
    lines.push(`직전 샷: 캐리 ${Math.round(shotCarryYd(sh))} yd · 발사각 ${sh.input.launchDeg.toFixed(1)}° · 스핀 ${Math.round(sh.input.rpm)} rpm · ${Math.round(sh.input.mph)} mph`);
  }
  if (s.phase === 'holed' || s.phase === 'done') {
    const par = (i: number) => HOLES[s.course[i]!]!.par;
    const sum = s.scores.reduce<number>((a, v, i) => a + (v === null ? 0 : v - par(i)), 0);
    if (s.mode === 'stroke') lines.push(`이번 라운드 ${sum === 0 ? 'E' : sum > 0 ? `+${sum}` : sum}`);
  }
  const hint = PHASE_HINT[s.phase];
  if (hint) lines.push(hint);
  const m = s.meter;
  const meter = s.phase === 'swing' && m.stage > 0 ? { marker: meterMarker(m, now), power: m.stage === 2 ? m.power : meterMarker(m, now), stage: m.stage as 1 | 2 } : null;
  return { title, headline: s.message?.text ?? (s.phase === 'aim' ? '조준하고 스윙하세요' : ''), tone: s.message?.tone ?? 'info', lines, meter };
}

/** 2D 컨텍스트의 필요한 부분만 (시험에서 가짜로 갈아 끼운다) */
export interface Ctx2D {
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  font: string;
  textAlign: CanvasTextAlign;
  textBaseline: CanvasTextBaseline;
  lineWidth: number;
  clearRect(x: number, y: number, w: number, h: number): void;
  fillRect(x: number, y: number, w: number, h: number): void;
  strokeRect(x: number, y: number, w: number, h: number): void;
  fillText(text: string, x: number, y: number, maxWidth?: number): void;
}

const FONT = "'Pretendard', 'Apple SD Gothic Neo', 'Noto Sans KR', 'Malgun Gothic', sans-serif";
const TONE_COLOR = { info: '#ffffff', good: '#ffd23f', bad: '#ff8b6b' } as const;

/** 패널 한 장을 w × h 캔버스에 그린다 */
export function drawPanel(ctx: Ctx2D, model: PanelModel, w: number, h: number): void {
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = 'rgba(10, 18, 14, 0.82)';
  ctx.fillRect(0, 0, w, h);
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.35)';
  ctx.lineWidth = 4;
  ctx.strokeRect(2, 2, w - 4, h - 4);
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillStyle = '#b6c4b9';
  ctx.font = `600 34px ${FONT}`;
  ctx.fillText(model.title, 32, 24, w - 64);
  ctx.fillStyle = TONE_COLOR[model.tone];
  ctx.font = `800 60px ${FONT}`;
  ctx.fillText(model.headline, 32, 84, w - 64);
  ctx.fillStyle = '#f5f8f2';
  ctx.font = `500 32px ${FONT}`;
  model.lines.forEach((t, i) => ctx.fillText(t, 32, 180 + i * 48, w - 64));
  if (model.meter) {
    // 스윙 미터: [-0.32, 1.05] 구간, 0 = 정타 위치
    const x0 = 32;
    const bw = w - 64;
    const y = h - 70;
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 3;
    ctx.strokeRect(x0, y, bw, 34);
    const pos = (v: number) => x0 + ((Math.min(Math.max(v, -0.32), 1.05) + 0.32) / 1.37) * bw;
    ctx.fillStyle = '#f6c744';
    ctx.fillRect(pos(0), y + 3, Math.max(pos(model.meter.power) - pos(0), 0), 28);
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(pos(model.meter.marker) - 3, y - 8, 6, 50);
  }
}
