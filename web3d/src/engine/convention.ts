/**
 * 부호 약속. UI·엔진·화면 사이의 부호는 이 파일에서만 바꾼다.
 *
 * 좌표 (엔진 = 화면 three.js 월드, 1 단위 = 1 m):
 *   x = 타깃 방향(사거리), y = 위, z = 오른쪽 (티 뒤에서 타깃을 볼 때).
 *   오른손 좌표계에서 앞 x, 위 y 이면 오른쪽 = x × y = +z 이다. tools/track_ball.py 의 CSV 도 'z 오른쪽'.
 *
 * 엔진 스핀축 a: $\hat\omega = (0, \sin a, \cos a)$ 이고 $\hat\omega\times\hat x = (0, \cos a, -\sin a)$ 이므로
 *   a > 0 이면 마그누스 힘이 −z 쪽 → 공이 왼쪽으로 휜다 (훅·드로). 참값 web_refs.csv 의 축 +10° → 좌우 −20.7 yd.
 * 엔진 방위각 φ: 발사 방향 (cos φ, 0, sin φ) → φ > 0 이면 오른쪽으로 출발 (screen.rs `sample` 와 같음).
 *
 * UI 는 트랙맨 약속을 쓴다.
 *   스핀축 + = 오른쪽으로 휨 (페이드·슬라이스), − = 왼쪽 (드로·훅)  → 엔진 a = −(UI 스핀축)
 *   방위각(Launch Direction) + = 오른쪽으로 출발                     → 엔진 φ = UI 방위각
 *   좌우(Side) + = 오른쪽                                            → UI 좌우 = 엔진 z
 *
 * flightbench `screen --csv` 의 출력은 엔진 부호다 (스핀축 + = 왼쪽으로 휨, 방위각 + = 오른쪽).
 *   붙여 넣을 때 스핀축만 부호를 뒤집는다.
 */
import type { EngineLaunch } from './flight.ts';

/** UI 입력 (트랙맨 부호) */
export interface ShotInput {
  /** 볼스피드 mph */
  mph: number;
  /** 발사각 deg */
  launchDeg: number;
  /** 방위각 deg, + = 오른쪽으로 출발 */
  azimuthDeg: number;
  /** 스핀 rpm */
  rpm: number;
  /** 스핀축 deg, + = 오른쪽으로 휨 */
  spinAxisDeg: number;
}

/** 트랙맨 스핀축 → 엔진 축 a */
export const engineAxisFromTrackman = (deg: number): number => -deg;
/** 엔진 축 a → 트랙맨 스핀축 */
export const trackmanAxisFromEngine = (deg: number): number => -deg;
/** 트랙맨 방위각 → 엔진 방위각 (같은 부호) */
export const engineAzimuthFromTrackman = (deg: number): number => deg;
/** screen --csv 출력의 스핀축(엔진 부호) → UI(트랙맨) */
export const trackmanAxisFromScreen = (deg: number): number => trackmanAxisFromEngine(deg);
/** screen --csv 출력의 방위각 → UI (같은 부호) */
export const trackmanAzimuthFromScreen = (deg: number): number => deg;
/** 엔진·월드 z (yd 또는 m) → UI 좌우 (+ = 오른쪽) */
export const rightFromWorldZ = (z: number): number => z;

export function toEngineLaunch(s: ShotInput): EngineLaunch {
  return {
    mph: s.mph,
    launchDeg: s.launchDeg,
    rpm: s.rpm,
    axisDeg: engineAxisFromTrackman(s.spinAxisDeg),
    azimuthDeg: engineAzimuthFromTrackman(s.azimuthDeg),
  };
}

/** 좌우 표시: 'R 12.3' / 'L 4.5' / '0.0' (yd) */
export function sideLabel(rightYd: number, digits = 1): string {
  const r = Number(rightYd.toFixed(digits));
  if (r === 0) return (0).toFixed(digits);
  return `${r > 0 ? 'R' : 'L'} ${Math.abs(r).toFixed(digits)}`;
}

/** 입력 범위 (엔진이 다룰 수 있는 곳으로 자른다: 스핀 음수·발사각 0 이하·수직 발사는 받지 않는다) */
export const INPUT_LIMITS: { [K in keyof ShotInput]: { min: number; max: number; step: number } } = {
  mph: { min: 20, max: 230, step: 0.1 },
  launchDeg: { min: 0.5, max: 70, step: 0.1 },
  azimuthDeg: { min: -30, max: 30, step: 0.1 },
  rpm: { min: 0, max: 15000, step: 10 },
  spinAxisDeg: { min: -60, max: 60, step: 0.1 },
};

export function clampInput(s: ShotInput): ShotInput {
  const out = { ...s };
  for (const key of Object.keys(INPUT_LIMITS) as (keyof ShotInput)[]) {
    const { min, max } = INPUT_LIMITS[key];
    const v = Number.isFinite(out[key]) ? out[key] : min;
    out[key] = Math.min(Math.max(v, min), max);
  }
  return out;
}
