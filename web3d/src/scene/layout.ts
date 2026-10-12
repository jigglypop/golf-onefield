/**
 * 연습장 배치 (1 단위 = 1 m). 티는 원점, 공은 +x 로 난다, +z 는 오른쪽 (convention.ts).
 */
import { M_PER_YD } from '../engine/units.ts';

/** 거리 표시 (yd) */
export const MARKERS_YD = [50, 100, 150, 200, 250, 300, 350] as const;
export const MARKER_COLORS: Record<number, string> = {
  50: '#e5484d',
  100: '#f5f5f0',
  150: '#f5c518',
  200: '#3e8ef7',
  250: '#f08c2e',
  300: '#8e6cf0',
  350: '#e5484d',
};
export const ydToM = (yd: number): number => yd * M_PER_YD;

/** 페어웨이: x 범위와 폭 (m) */
export const FAIRWAY = { x0: 28, x1: 340, width: 46 } as const;
/** 거리 선 길이 (z 방향, m) */
export const LINE_LENGTH = 74;
/** 그린 반지름 (m) */
export const GREEN_RADIUS = 14;

/** 캐릭터 시작 위치: 오른손 골퍼가 공 왼쪽(−z)에 서서 공(+z)을 본다 */
export const SPAWN: [number, number, number] = [-0.2, 1.0, -1.15];

/** 해 방향 (빛 위치, 방향만 쓴다): 앞 왼쪽 낮은 오후 해 (고도 약 27°). 나무 그림자가 페어웨이를 가로지르고 능선이 역광으로 떠 보인다 */
export const SUN_POSITION: [number, number, number] = [130, 75, -65];

export const COLORS = {
  rough: '#5c8f3c',
  fairwayLight: '#86c556',
  fairwayDark: '#74b248',
  tee: '#8fd05e',
  green: '#93d86c',
  fringe: '#7cbf54',
  line: '#f4f4ee',
  skyZenith: '#2f78cf',
  skyHorizon: '#d6e5ef',
  fog: '#cddbe6',
  tracer: '#ff6a2a',
  landing: '#ffd23f',
} as const;
