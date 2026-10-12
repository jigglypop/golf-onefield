/**
 * 화면 버튼 (터치·펜·마우스 공용): 조준 ◀ ▶(누르고 있으면 반복), 핀 조준, 클럽 ▲ ▼, 큰 스윙 버튼, 빨리 감기.
 * 키보드 조작은 그대로 두고 같은 store 동작(aimBy·selectClub·pressSwing …)을 부른다.
 * 보이는 조건은 game.css: 거친 포인터(손가락) 이거나 화면이 좁을 때, 또는 주소에 ?touch 가 있을 때.
 *
 * 스윙 버튼은 click 이 아니라 pointerdown 으로 받는다: 타이밍 미터를 누르는 순간이 곧 입력이라 클릭 지연이 끼면 안 된다.
 * 키보드로 초점을 옮겨 Enter 를 누른 경우(click 의 detail === 0)만 click 으로 받는다.
 */
import { useCallback, useEffect, useRef, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react';

import { CLUBS } from '../clubs.ts';
import { aimAtPin, aimBy, isBotTurn, pressSwing, selectClub, skipAnimation, toggleOverview, useGame } from '../round.ts';
import { ensureAudio } from '../sound.ts';

const DEG = Math.PI / 180;

/** 누르고 있는 동안 onStep(n) 을 되풀이한다: 처음 한 번, 0.35 s 뒤부터 점점 빠르게 (n = 몇 번째 반복) */
function HoldButton({ onStep, disabled, className, label, children }: { onStep: (n: number) => void; disabled?: boolean; className?: string; label: string; children: ReactNode }) {
  const timer = useRef<number | null>(null);
  const stop = useCallback(() => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
  }, []);
  useEffect(() => stop, [stop]);
  useEffect(() => {
    if (disabled) stop();
  }, [disabled, stop]);
  const start = (e: ReactPointerEvent<HTMLButtonElement>) => {
    if (disabled || e.button > 0) return;
    e.preventDefault();
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // 포인터 캡처를 못 쓰는 환경
    }
    stop();
    onStep(0);
    let n = 1;
    const tick = () => {
      onStep(n);
      timer.current = window.setTimeout(tick, Math.max(110 - n * 5, 35));
      n += 1;
    };
    timer.current = window.setTimeout(tick, 350);
  };
  return (
    <button type="button" className={className ?? 'g-touch-btn'} aria-label={label} disabled={disabled} onPointerDown={start} onPointerUp={stop} onPointerCancel={stop} onLostPointerCapture={stop} onContextMenu={(e) => e.preventDefault()}>
      {children}
    </button>
  );
}

export function TouchControls() {
  const phase = useGame((s) => s.phase);
  const putter = useGame((s) => CLUBS[s.clubIdx]!.putter === true);
  const bot = useGame((s) => isBotTurn(s));
  const overview = useGame((s) => s.overview);
  const canAim = phase === 'aim' && !bot;
  const canSwing = (phase === 'aim' || phase === 'swing') && !bot;
  const base = (putter ? 0.25 : 0.5) * DEG;
  // 누르고 있을수록 커진다 (반복 10 번마다 한 배씩, 최대 4 배)
  const step = (dir: -1 | 1) => (n: number) => aimBy(dir * base * (1 + Math.min(Math.floor(n / 10), 3)));
  const forced = typeof location !== 'undefined' && new URLSearchParams(location.search).has('touch');
  return (
    <div className={`g-touch${forced ? ' g-touch-force' : ''}`} data-testid="g-touch">
      <div className="g-touch-left">
        <div className="g-touch-row">
          <HoldButton label="왼쪽으로 조준" disabled={!canAim} onStep={step(-1)}>
            ◀
          </HoldButton>
          <HoldButton label="오른쪽으로 조준" disabled={!canAim} onStep={step(1)}>
            ▶
          </HoldButton>
          <button type="button" className="g-touch-btn g-touch-small" disabled={!canAim} onClick={aimAtPin} aria-label="핀 쪽으로 조준">
            핀
          </button>
        </div>
        <div className="g-touch-row">
          <button type="button" className="g-touch-btn g-touch-small" onClick={skipAnimation} aria-label="빨리 감기">
            ⏩
          </button>
          <button type="button" className="g-touch-btn g-touch-small" onClick={toggleOverview} aria-label={overview ? '공 시점으로' : '홀 전체 보기'}>
            {overview ? '공' : '전체'}
          </button>
        </div>
      </div>
      <div className="g-touch-right">
        <div className="g-touch-clubs">
          <button type="button" className="g-touch-btn" disabled={!canAim} onClick={() => selectClub(-1)} aria-label="긴 클럽으로">
            ▲
          </button>
          <button type="button" className="g-touch-btn" disabled={!canAim} onClick={() => selectClub(1)} aria-label="짧은 클럽으로">
            ▼
          </button>
        </div>
        <button
          type="button"
          className="g-touch-swing"
          disabled={!canSwing && phase !== 'intro'}
          onPointerDown={(e) => {
            if (e.button > 0) return;
            e.preventDefault();
            ensureAudio();
            pressSwing();
          }}
          onClick={(e) => {
            if (e.detail === 0) {
              ensureAudio();
              pressSwing();
            }
          }}
        >
          스윙
        </button>
      </div>
    </div>
  );
}
