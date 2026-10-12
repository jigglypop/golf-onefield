/**
 * 효과음: 파일 없이 WebAudio 로 합성한다. 브라우저 정책상 첫 클릭·키 입력 때 켠다(ensureAudio).
 * 끄기 설정은 브라우저 저장소에 남긴다(못 쓰면 이번 실행에서만). 끄면 새 소리를 만들지 않고 마스터 음량도 0 이다.
 *
 * 시간 맞추기: 모든 소리는 `delay` (s) 를 받아 `AudioContext.currentTime + delay` 에 예약한다. 화면 프레임 시계가 아니라
 * 오디오 시계로 재므로, 타구음은 스윙 애니메이션의 임팩트 순간(delay = 임팩트까지 남은 시간)에 맞춰 울린다.
 * 예약해 둔 소리는 빨리 감기(F)·새 샷 때 `cancelScheduled()` 로 아직 시작하지 않은 것을 모두 끊는다.
 *
 * 구성: 타구(스윙 바람 + 타격), 퍼트, 착지(잔디·모래·물), 바운드, 굴림(속력에 따른 거른 잡음), 나무 맞음, 홀컵, 박수,
 * 클릭, 그리고 바람 소리 배경(속도에 따라 세기, 천천히 일렁임).
 */
import type { Surface } from './course.ts';

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let noiseBuf: AudioBuffer | null = null;
let muted = readMuted();
/** 샷에 맞춰 미리 예약한 소리 (cancelScheduled 로 끊는다). 홀컵·박수·클릭·착지음은 여기 넣지 않는다 */
const pending = new Set<AudioScheduledSourceNode>();
/** 켜져 있는 동안 만드는 소리는 pending 에 넣는다 (cue() 안) */
let cueMode = false;
/** 배경 바람 소리 */
let ambient: { gain: GainNode; filter: BiquadFilterNode } | null = null;
/** 오디오가 켜지기 전에 받은 바람 속도 */
let wantWind: number | null = null;

function readMuted(): boolean {
  try {
    return localStorage.getItem('golf-onefield-mute') === '1';
  } catch {
    return false;
  }
}

export function isMuted(): boolean {
  return muted;
}

export function setMuted(m: boolean): void {
  muted = m;
  try {
    localStorage.setItem('golf-onefield-mute', m ? '1' : '0');
  } catch {
    // 저장소가 없으면 이번 실행에서만
  }
  if (master) master.gain.value = m ? 0 : 0.8;
}

/** 사용자 입력 안에서 부른다 */
export function ensureAudio(): void {
  if (typeof window === 'undefined') return;
  const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AC) return;
  if (!ctx) {
    ctx = new AC();
    master = ctx.createGain();
    master.gain.value = muted ? 0 : 0.8;
    master.connect(ctx.destination);
    noiseBuf = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate);
    const d = noiseBuf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  }
  if (ctx.state === 'suspended') void ctx.resume();
  if (wantWind !== null) setWindAmbient(wantWind);
}

function ready(): AudioContext | null {
  return ctx && master && !muted && ctx.state === 'running' ? ctx : null;
}

/** 샷에 맞춘 예약 소리만 기록해 두었다가 끝나면 지운다 */
function track(node: AudioScheduledSourceNode): void {
  if (!cueMode) return;
  pending.add(node);
  node.onended = () => pending.delete(node);
}

/** fn 안에서 만드는 소리를 "샷 예약 소리"로 기록한다 */
function cue(fn: () => void): void {
  cueMode = true;
  try {
    fn();
  } finally {
    cueMode = false;
  }
}

/**
 * 샷에 맞춰 예약한 소리(타구·바운드·굴림·나무)를 모두 끊는다 (빨리 감기·샷이 끝났을 때·새 샷). 홀컵·박수·착지음·배경 바람은 그대로.
 */
export function cancelScheduled(): void {
  for (const n of pending) {
    try {
      n.stop();
    } catch {
      // 이미 끝난 노드
    }
  }
  pending.clear();
}

/** 감쇠하는 사인 */
function tone(freq: number, dur: number, gain: number, delay = 0, type: OscillatorType = 'sine', endFreq?: number): void {
  const c = ready();
  if (!c || !master) return;
  const t = c.currentTime + delay;
  const o = c.createOscillator();
  const g = c.createGain();
  o.type = type;
  o.frequency.setValueAtTime(freq, t);
  if (endFreq) o.frequency.exponentialRampToValueAtTime(endFreq, t + dur);
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(gain, t + 0.005);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  o.connect(g).connect(master);
  o.start(t);
  o.stop(t + dur + 0.05);
  track(o);
}

/** 거른 잡음 */
function noise(dur: number, gain: number, f0: number, f1: number, q = 1, delay = 0, type: BiquadFilterType = 'bandpass'): void {
  const c = ready();
  if (!c || !master || !noiseBuf) return;
  const t = c.currentTime + delay;
  const src = c.createBufferSource();
  src.buffer = noiseBuf;
  const f = c.createBiquadFilter();
  f.type = type;
  f.Q.value = q;
  f.frequency.setValueAtTime(f0, t);
  f.frequency.exponentialRampToValueAtTime(Math.max(f1, 20), t + dur);
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(gain, t + Math.min(0.03, dur / 3));
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  src.connect(f).connect(g).connect(master);
  src.start(t, Math.random());
  src.stop(t + dur + 0.05);
  track(src);
}

/** 스윙 바람이 임팩트 앞에서 시작하는 길이 (s) */
const WHOOSH_LEAD_S = 0.22;

/**
 * 스윙 바람 소리 + 타구음. quality 0 (빗맞음) .. 1 (정타).
 * impactIn: 지금부터 임팩트까지 (s). 타구음은 정확히 그 오디오 시각에, 바람 소리는 그 직전에 울린다.
 */
export function playSwing(quality: number, impactIn = 0.2): void {
  const at = Math.max(impactIn, 0);
  const q = Math.min(Math.max(quality, 0), 1);
  cue(() => {
    noise(0.28, 0.35, 400, 2600, 0.8, Math.max(at - WHOOSH_LEAD_S, 0));
    tone(900 + 900 * q, 0.06 + 0.05 * q, 0.5 + 0.3 * q, at, 'triangle', 500 + 400 * q);
    noise(0.05, 0.4 + 0.2 * q, 2500 + 2000 * q, 1500, 1.5, at, 'highpass');
  });
}

export function playPutt(impactIn = 0): void {
  const at = Math.max(impactIn, 0);
  cue(() => {
    tone(1400, 0.05, 0.35, at, 'triangle', 900);
    noise(0.03, 0.2, 3000, 2000, 1.2, at, 'highpass');
  });
}

export function playLand(surface: Surface, delay = 0): void {
  if (surface === 'water') {
    playSplash(delay);
    return;
  }
  if (surface === 'bunker') {
    noise(0.25, 0.4, 1800, 500, 0.7, delay);
    return;
  }
  tone(130, 0.12, 0.45, delay, 'sine', 70);
  noise(0.08, 0.2, 900, 300, 0.8, delay);
}

export function playSplash(delay = 0): void {
  noise(0.6, 0.55, 2500, 300, 0.6, delay, 'lowpass');
  for (let i = 0; i < 6; i++) tone(500 + Math.random() * 700, 0.07, 0.12, delay + 0.08 + i * 0.05, 'sine', 1200 + Math.random() * 800);
}

/** 두 번째 이후 바운드: 수직 속도 vn (m/s) 가 클수록 크고, 모래는 둔하게 */
export function playBounce(vn: number, surface: Surface, delay = 0): void {
  const k = Math.min(Math.max(vn / 6, 0.15), 1);
  cue(() => {
    if (surface === 'bunker' || surface === 'water' || surface === 'ob') {
      noise(0.1, 0.16 * k, 1400, 500, 0.7, delay);
      return;
    }
    const firm = surface === 'green' ? 1.2 : surface === 'rough' ? 0.6 : 1;
    tone(150 * firm, 0.08, 0.3 * k, delay, 'sine', 90 * firm);
    noise(0.05, 0.12 * k, 1100, 400, 0.8, delay);
  });
}

/**
 * 굴림: 거른 잡음의 세기·밝기를 공의 속력(profile: {t, v}, 착지 = 0)에 맞춘다. delay 는 profile[0].t 부터 잰다.
 * 모래·물·OB 에서는 울리지 않는다. 한 번에 노드 셋(잡음·필터·이득)만 쓴다.
 */
export function playRoll(profile: readonly { t: number; v: number }[], surface: Surface, delay = 0): void {
  const c = ready();
  if (!c || !master || !noiseBuf || profile.length === 0) return;
  if (surface === 'bunker' || surface === 'water' || surface === 'ob') return;
  const body = surface === 'rough' ? 1.5 : surface === 'green' ? 0.6 : 1;
  const t0 = c.currentTime + Math.max(delay, 0);
  const span = profile[profile.length - 1]!.t - profile[0]!.t;
  if (span <= 0.05) return;
  const src = c.createBufferSource();
  src.buffer = noiseBuf;
  src.loop = true;
  const f = c.createBiquadFilter();
  f.type = 'lowpass';
  f.Q.value = 0.7;
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, t0);
  f.frequency.setValueAtTime(300, t0);
  for (const p of profile) {
    const at = t0 + (p.t - profile[0]!.t);
    g.gain.linearRampToValueAtTime(Math.min(0.2, 0.02 * body * Math.sqrt(p.v)) + 0.0001, at);
    f.frequency.linearRampToValueAtTime(260 + 140 * Math.min(p.v, 12), at);
  }
  g.gain.linearRampToValueAtTime(0.0001, t0 + span + 0.12);
  src.connect(f).connect(g).connect(master);
  src.start(t0, Math.random());
  src.stop(t0 + span + 0.2);
  cue(() => track(src));
}

/** 나무에 맞음: 가지 스치는 소리 + 둔탁한 소리 */
export function playTree(delay = 0): void {
  cue(() => {
    noise(0.4, 0.45, 1600, 350, 0.6, delay);
    tone(120, 0.14, 0.4, delay, 'sine', 60);
  });
}

export function playCup(): void {
  tone(2400, 0.18, 0.35, 0, 'sine');
  tone(3100, 0.14, 0.25, 0.04, 'sine');
  tone(1900, 0.12, 0.25, 0.11, 'sine');
  noise(0.12, 0.15, 3500, 2500, 2, 0.05, 'bandpass');
}

/** 박수: level 0 (가볍게) .. 1 (크게) */
export function playApplause(level: number): void {
  const n = Math.round(12 + 50 * level);
  const span = 1.2 + 1.6 * level;
  for (let i = 0; i < n; i++) noise(0.05 + Math.random() * 0.04, 0.08 + 0.12 * level, 1500 + Math.random() * 1500, 1200, 1.2, Math.random() * span);
}

export function playClick(): void {
  tone(1800, 0.03, 0.15, 0, 'square', 1200);
}

/**
 * 배경 바람 소리: 거른 잡음을 되풀이하고 세기를 천천히 일렁이게 한다.
 * speed (m/s) 가 클수록 크고 밝다. 오디오가 아직 꺼져 있으면 기억했다가 켜질 때 적용한다. 끄기 설정이면 마스터 음량이 0 이다.
 */
export function setWindAmbient(speed: number): void {
  wantWind = speed;
  if (!ctx || !master || !noiseBuf) return;
  const c = ctx;
  if (!ambient) {
    const src = c.createBufferSource();
    src.buffer = noiseBuf;
    src.loop = true;
    const filter = c.createBiquadFilter();
    filter.type = 'lowpass';
    filter.Q.value = 0.6;
    const gain = c.createGain();
    gain.gain.value = 0;
    // 천천히 일렁임: 0.13 Hz 사인으로 이득을 흔든다
    const lfo = c.createOscillator();
    lfo.frequency.value = 0.13;
    const depth = c.createGain();
    depth.gain.value = 0.006;
    lfo.connect(depth).connect(gain.gain);
    src.connect(filter).connect(gain).connect(master);
    src.start(0, Math.random());
    lfo.start();
    ambient = { gain, filter };
  }
  const level = Math.min(0.012 + 0.01 * Math.max(speed, 0), 0.08);
  ambient.gain.gain.setTargetAtTime(level, c.currentTime, 0.5);
  ambient.filter.frequency.setTargetAtTime(280 + 110 * Math.max(speed, 0), c.currentTime, 0.5);
}
