/**
 * 효과음: 파일 없이 WebAudio 로 합성한다. 브라우저 정책상 첫 클릭·키 입력 때 켠다(ensureAudio).
 * 끄기 설정은 브라우저 저장소에 남긴다(못 쓰면 이번 실행에서만).
 */
import type { Surface } from './course.ts';

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let noiseBuf: AudioBuffer | null = null;
let muted = readMuted();

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
}

function ready(): AudioContext | null {
  return ctx && master && !muted && ctx.state === 'running' ? ctx : null;
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
}

/** 스윙 바람 소리 + 타구음. quality 0 (빗맞음) .. 1 (정타) */
export function playSwing(quality: number): void {
  noise(0.28, 0.35, 400, 2600, 0.8);
  const q = Math.min(Math.max(quality, 0), 1);
  tone(900 + 900 * q, 0.06 + 0.05 * q, 0.5 + 0.3 * q, 0.2, 'triangle', 500 + 400 * q);
  noise(0.05, 0.4 + 0.2 * q, 2500 + 2000 * q, 1500, 1.5, 0.2, 'highpass');
}

export function playPutt(): void {
  tone(1400, 0.05, 0.35, 0, 'triangle', 900);
  noise(0.03, 0.2, 3000, 2000, 1.2, 0, 'highpass');
}

export function playLand(surface: Surface): void {
  if (surface === 'water') {
    playSplash();
    return;
  }
  if (surface === 'bunker') {
    noise(0.25, 0.4, 1800, 500, 0.7);
    return;
  }
  tone(130, 0.12, 0.45, 0, 'sine', 70);
  noise(0.08, 0.2, 900, 300, 0.8);
}

export function playSplash(): void {
  noise(0.6, 0.55, 2500, 300, 0.6, 0, 'lowpass');
  for (let i = 0; i < 6; i++) tone(500 + Math.random() * 700, 0.07, 0.12, 0.08 + i * 0.05, 'sine', 1200 + Math.random() * 800);
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
