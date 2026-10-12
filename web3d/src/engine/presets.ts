/** 미리 정한 샷 (TrackMan 투어 평균표의 입력 세 개, harness.rs PGA12·LPGA11 과 같은 값). 방위각·스핀축 0. */
import type { ShotInput } from './convention.ts';

export interface Preset {
  id: string;
  label: string;
  input: ShotInput;
}

const shot = (mph: number, launchDeg: number, rpm: number): ShotInput => ({ mph, launchDeg, rpm, azimuthDeg: 0, spinAxisDeg: 0 });

export const PRESETS: readonly Preset[] = [
  { id: 'pga-dr', label: 'PGA 드라이버', input: shot(167, 10.9, 2686) },
  { id: 'pga-7i', label: 'PGA 7번', input: shot(120, 16.3, 7097) },
  { id: 'pga-pw', label: 'PGA PW', input: shot(102, 24.2, 9304) },
  { id: 'lpga-7i', label: 'LPGA 7번', input: shot(104, 19.0, 6699) },
  { id: 'am-dr', label: '아마추어 드라이버', input: shot(135, 13, 3000) },
];

export const DEFAULT_INPUT: ShotInput = PRESETS[1]!.input;
