/**
 * 연습장을 홀 하나처럼 다루는 자리표시 홀: 지형(`Terrain`)과 숲(`Forest`)이 `hole.bounds`(평지로 둘 구역)만 있으면 되므로
 * 연습장 구역을 그 모양으로 준다. 게임 코스와 규칙·물리가 겹치지 않는다.
 */
import type { Hole } from '../game/course.ts';

export const RANGE_HOLE: Hole = {
  number: 99,
  name: '연습장',
  par: 0,
  tee: [0, 0],
  teeTarget: [200, 0],
  pin: [200, 0],
  fairways: [],
  green: { c: [200, 0], rx: 14, rz: 14, rot: 0, slope: [0, 0] },
  bunkers: [],
  water: [],
  bounds: [
    [-45, -72],
    [425, -72],
    [475, -40],
    [475, 40],
    [425, 72],
    [-45, 72],
  ],
};
