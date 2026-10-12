/**
 * 화면 위 HUD (DOM): 샷 입력·미리 정한 샷·screen 출력 붙여넣기, 결과, 카메라·배속·보기 설정.
 * UI 의 스핀축·방위각은 트랙맨 부호(+ = 오른쪽)다. 엔진 부호와의 변환은 engine/convention.ts 한 곳.
 */
import { useEffect, useState, type MouseEvent, type ReactNode } from 'react';

import { clampInput, INPUT_LIMITS, sideLabel, toEngineLaunch, type ShotInput } from '../engine/convention.ts';
import { simulate } from '../engine/flight.ts';
import { MODEL } from '../engine/model.ts';
import { parseScreenOutput } from '../engine/parse.ts';
import { PRESETS } from '../engine/presets.ts';
import { M_PER_YD } from '../engine/units.ts';
import { BIG_BALL } from '../scene/BallFlight.tsx';
import { flightClock, launchShot, store, useApp, type CameraMode, type Speed } from '../state.ts';

/** 버튼을 누른 뒤 포커스를 풀어 Space(점프)·Enter(샷)가 버튼을 다시 누르지 않게 한다 */
const press = (fn: () => void) => (e: MouseEvent<HTMLButtonElement>) => {
  fn();
  e.currentTarget.blur();
};

const fmt = (v: number, digits: number) => (Number.isFinite(v) ? v.toFixed(digits) : '–');
const signed = (v: number, digits: number) => `${v > 0 ? '+' : ''}${v.toFixed(digits)}`;

const FIELDS: { key: keyof ShotInput; label: string; unit: string; hint?: string }[] = [
  { key: 'mph', label: '볼스피드', unit: 'mph' },
  { key: 'launchDeg', label: '발사각', unit: '°' },
  { key: 'azimuthDeg', label: '방위각', unit: '°', hint: '+ 오른쪽으로 출발' },
  { key: 'rpm', label: '스핀', unit: 'rpm' },
  { key: 'spinAxisDeg', label: '스핀축', unit: '°', hint: '+ 오른쪽으로 휨 (트랙맨)' },
];

function NumField({ k, label, unit, hint }: { k: keyof ShotInput; label: string; unit: string; hint?: string | undefined }) {
  const value = useApp((s) => s.input[k]);
  const lim = INPUT_LIMITS[k];
  return (
    <label className="field" title={hint}>
      <span className="field-label">
        {label}
        {hint && <small>{hint}</small>}
      </span>
      <input
        type="number"
        inputMode="decimal"
        step={lim.step}
        min={lim.min}
        max={lim.max}
        value={Number.isFinite(value) ? value : ''}
        onChange={(e) => store.set({ input: { ...store.get().input, [k]: e.target.valueAsNumber } })}
        aria-label={label}
      />
      <em>{unit}</em>
    </label>
  );
}

function sameInput(a: ShotInput, b: ShotInput) {
  return (Object.keys(a) as (keyof ShotInput)[]).every((k) => a[k] === b[k]);
}

function PasteBox() {
  const [text, setText] = useState('');
  const [msg, setMsg] = useState<ReactNode>(null);
  const fill = () => {
    const p = parseScreenOutput(text);
    if (Object.keys(p.values).length === 0) {
      setMsg('읽을 줄이 없습니다. "볼스피드 … mph, 발사각 …°, 방위각 …°" 와 "스핀 … rpm, 스핀축 …°, 캐리 … yd" 줄을 붙여 넣으세요.');
      return;
    }
    const input = clampInput({ ...store.get().input, ...p.values });
    store.set({ input });
    const ours = simulate(toEngineLaunch(input), { samples: false });
    setMsg(
      <>
        <b>채움</b> {fmt(input.mph, 1)} mph · {fmt(input.launchDeg, 2)}° · 방위 {signed(input.azimuthDeg, 2)}° · {fmt(input.rpm, 0)} rpm · 축{' '}
        {signed(input.spinAxisDeg, 1)}°
        {p.screenAxisDeg !== undefined && (
          <>
            <br />
            스핀축: screen {signed(p.screenAxisDeg, 1)}° (엔진 부호, + = 왼쪽) → 트랙맨 {signed(input.spinAxisDeg, 1)}°
          </>
        )}
        {p.screenCarryYd !== undefined && (
          <>
            <br />
            캐리: screen {p.screenCarryYd.toFixed(1)} yd · 이 엔진 {fmt(ours.carryYd, 1)} yd
          </>
        )}
        {p.missing.length > 0 && (
          <>
            <br />
            <span className="warn">못 찾음: {p.missing.join(', ')} (지금 값 유지)</span>
          </>
        )}
      </>,
    );
  };
  return (
    <details className="paste">
      <summary>screen --csv 출력 붙여넣기</summary>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={4}
        spellCheck={false}
        placeholder={'볼스피드 120.0 mph, 발사각 16.30°, 방위각 0.01°\n스핀 7061 rpm, 스핀축 8.0°, 캐리 168.2 yd'}
        aria-label="screen --csv 출력"
      />
      <button type="button" className="small" onClick={press(fill)}>
        입력 채우기
      </button>
      {msg && <p className="paste-msg">{msg}</p>}
    </details>
  );
}

function ShotPanel() {
  const input = useApp((s) => s.input);
  const notice = useApp((s) => s.notice);
  return (
    <section className="panel shot-panel" aria-label="샷 입력">
      <h2>샷 입력</h2>
      <div className="fields">
        {FIELDS.map((f) => (
          <NumField key={f.key} k={f.key} label={f.label} unit={f.unit} hint={f.hint} />
        ))}
      </div>
      <div className="presets" role="group" aria-label="미리 정한 샷">
        {PRESETS.map((p) => (
          <button
            type="button"
            key={p.id}
            className="chip"
            aria-pressed={sameInput(input, p.input)}
            title={`${p.input.mph} mph · ${p.input.launchDeg}° · ${p.input.rpm} rpm`}
            onClick={press(() => store.set({ input: p.input, notice: null }))}
          >
            {p.label}
          </button>
        ))}
      </div>
      <button type="button" className="shoot" onClick={press(() => launchShot())}>
        샷 치기 <kbd>Enter</kbd>
      </button>
      {notice && <p className="warn">{notice}</p>}
      <PasteBox />
    </section>
  );
}

function FlightTimer({ total }: { total: number }) {
  const [t, setT] = useState(0);
  useEffect(() => {
    let raf = 0;
    let last = -1;
    const tick = () => {
      const now = Math.round(flightClock.t * 10) / 10;
      if (now !== last) {
        last = now;
        setT(now);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  return (
    <span className="status flying">
      비행 중 {t.toFixed(1)} / {total.toFixed(1)} s
    </span>
  );
}

function Stat({ label, value, unit, sub }: { label: string; value: string; unit: string; sub?: string }) {
  return (
    <div className="stat">
      <span className="stat-label">{label}</span>
      <span className="stat-value">
        {value}
        <small>{unit}</small>
      </span>
      {sub && <span className="stat-sub">{sub}</span>}
    </div>
  );
}

function ResultPanel() {
  const shot = useApp((s) => s.shot);
  const phase = useApp((s) => s.phase);
  const r = shot?.result;
  return (
    <section className="panel result-panel" aria-label="결과" data-phase={phase}>
      <h2>
        결과 {shot && <span className="shot-no">#{shot.id}</span>}
        {shot && (phase === 'flying' ? <FlightTimer total={r!.timeS} /> : <span className="status">착지</span>)}
      </h2>
      {r ? (
        <>
          <div className="carry">
            <span className="stat-label">캐리</span>
            <span className="carry-value" data-testid="carry">
              {fmt(r.carryYd, 1)}
              <small>yd</small>
            </span>
            <span className="stat-sub">{fmt(r.carryYd * M_PER_YD, 1)} m</span>
          </div>
          <div className="stats">
            <Stat label="좌우" value={sideLabel(r.lateralYd)} unit="yd" sub="R = 오른쪽" />
            <Stat label="최고 높이" value={fmt(r.apexYd, 1)} unit="yd" sub={`${fmt(r.apexYd * M_PER_YD, 1)} m`} />
            <Stat label="착지각" value={fmt(r.landDeg, 1)} unit="°" />
            <Stat label="체공 시간" value={fmt(r.timeS, 2)} unit="s" />
          </div>
          <p className="echo">
            {fmt(shot.input.mph, 1)} mph · 발사 {fmt(shot.input.launchDeg, 1)}° · 방위 {signed(shot.input.azimuthDeg, 1)}° · {fmt(shot.input.rpm, 0)} rpm · 축{' '}
            {signed(shot.input.spinAxisDeg, 1)}°
          </p>
        </>
      ) : (
        <p className="empty">미리 정한 샷을 고르거나 값을 넣고 <b>샷 치기</b>(Enter)를 누르세요.</p>
      )}
      <p className="note">
        총거리(바운드·굴림)는 보여 주지 않습니다. 이 저장소의 지면 모형(ground.py)이 USGA 런 검증에서 '런 평균 하나로 찍기'(6.9 yd)보다 나빴기
        때문입니다(14.8–69 yd).
      </p>
      <p className="model">
        비행 장: {MODEL.name} (PGA 투어표로만 맞춘 매개변수) · τ 걸음 RK4 · 무풍, 해수면 공기 1.2 kg/m³
      </p>
    </section>
  );
}

const CAMERAS: { mode: CameraMode; label: string; title: string }[] = [
  { mode: 'auto', label: '자동', title: '티 뒤 → 공 따라가기 → 착지점' },
  { mode: 'behind', label: '뒤', title: '티 뒤에서 타깃 쪽' },
  { mode: 'side', label: '옆', title: '오른쪽에서 본 궤적' },
  { mode: 'top', label: '위', title: '위에서 본 궤적 (화면 위 = 타깃)' },
  { mode: 'player', label: '캐릭터', title: 'gaesup-world 3인칭 카메라 (WASD 로 걷기)' },
];
const SPEEDS: Speed[] = [0.5, 1, 2];

function ViewBar() {
  const mode = useApp((s) => s.cameraMode);
  const speed = useApp((s) => s.speed);
  const big = useApp((s) => s.bigBall);
  const pinYd = useApp((s) => s.pinYd);
  return (
    <section className="panel view-bar" aria-label="보기">
      <div className="group" role="group" aria-label="카메라">
        <span className="group-label">카메라</span>
        {CAMERAS.map((c) => (
          <button type="button" key={c.mode} className="chip" aria-pressed={mode === c.mode} title={c.title} onClick={press(() => store.set({ cameraMode: c.mode }))}>
            {c.label}
          </button>
        ))}
      </div>
      <div className="group" role="group" aria-label="배속">
        <span className="group-label">배속</span>
        {SPEEDS.map((v) => (
          <button type="button" key={v} className="chip" aria-pressed={speed === v} onClick={press(() => store.set({ speed: v }))}>
            ×{v}
          </button>
        ))}
      </div>
      <label className="group check">
        <input type="checkbox" checked={big} onChange={(e) => store.set({ bigBall: e.target.checked })} />
        공 크게 (×{BIG_BALL})
      </label>
      <label className="group pin">
        핀
        <input
          type="number"
          min={50}
          max={350}
          step={1}
          value={pinYd}
          onChange={(e) => {
            const v = e.target.valueAsNumber;
            if (Number.isFinite(v)) store.set({ pinYd: Math.min(Math.max(Math.round(v), 50), 350) });
          }}
          aria-label="핀 거리 (yd)"
        />
        yd
      </label>
    </section>
  );
}

function RendererBadge() {
  const renderer = useApp((s) => s.renderer);
  return (
    <div className="badge" data-testid="renderer" title="?renderer=webgl 로 classic WebGL 강제">
      {renderer}
    </div>
  );
}

export function Hud() {
  return (
    <div className="hud">
      <header className="title">
        <h1>골프 원필드 · 3D 연습장</h1>
        <p>React Three Fiber + gaesup-world · 1 칸 = 1 m</p>
      </header>
      <RendererBadge />
      <ShotPanel />
      <ResultPanel />
      <ViewBar />
      <p className="keys">
        <kbd>WASD</kbd> 걷기 <kbd>Shift</kbd> 달리기 <kbd>Space</kbd> 점프 <kbd>Enter</kbd> 샷 · 캐릭터 시점에서 우클릭 드래그 회전, 휠 확대
      </p>
    </div>
  );
}
