/**
 * 클럽과 스윙 → 발사 조건.
 *
 * 풀스윙 값은 TrackMan PGA 투어 평균표(flightbench harness.rs PGA12)를 그대로 쓰고, 샌드 웨지만 따로 정했다.
 * 그래서 이 게임의 골퍼는 투어 프로만큼 멀리 친다.
 *
 * 스윙 → 발사:
 *   볼스피드 = 풀스윙 × 파워 × 라이 배율, 스핀 = 풀스윙 × (0.55 + 0.45·파워) × 라이 배율, 발사각 = 풀스윙.
 *   정확도 a (−1..1, 0 = 정타): a > 0 → 오른쪽으로 휨(슬라이스), a < 0 → 왼쪽(훅). 트랙맨 스핀축 = 20·a°, 방위각 = 2.5·a°.
 *   오버스윙(파워 > 1)은 빗나감을 키운다.
 */
import type { ShotInput } from '../engine/convention.ts';
import { toEngineLaunch } from '../engine/convention.ts';
import { simulate } from '../engine/flight.ts';
import type { Surface } from './course.ts';

export interface Club {
  id: string;
  name: string;
  short: string;
  mph: number;
  deg: number;
  rpm: number;
  putter?: true;
}

export const CLUBS: Club[] = [
  { id: '1w', name: '드라이버', short: '1W', mph: 167, deg: 10.9, rpm: 2686 },
  { id: '3w', name: '3번 우드', short: '3W', mph: 158, deg: 9.2, rpm: 3655 },
  { id: '5w', name: '5번 우드', short: '5W', mph: 152, deg: 9.4, rpm: 4350 },
  { id: '4h', name: '4번 유틸', short: '4H', mph: 146, deg: 10.2, rpm: 4437 },
  { id: '4i', name: '4번 아이언', short: '4I', mph: 137, deg: 11.0, rpm: 4836 },
  { id: '5i', name: '5번 아이언', short: '5I', mph: 132, deg: 12.1, rpm: 5361 },
  { id: '6i', name: '6번 아이언', short: '6I', mph: 127, deg: 14.1, rpm: 6231 },
  { id: '7i', name: '7번 아이언', short: '7I', mph: 120, deg: 16.3, rpm: 7097 },
  { id: '8i', name: '8번 아이언', short: '8I', mph: 115, deg: 18.1, rpm: 7998 },
  { id: '9i', name: '9번 아이언', short: '9I', mph: 109, deg: 20.4, rpm: 8647 },
  { id: 'pw', name: '피칭 웨지', short: 'PW', mph: 102, deg: 24.2, rpm: 9304 },
  { id: 'sw', name: '샌드 웨지', short: 'SW', mph: 86, deg: 30, rpm: 9800 },
  { id: 'pt', name: '퍼터', short: 'PT', mph: 0, deg: 0, rpm: 0, putter: true },
];

export const PUTTER = CLUBS.length - 1;
export const DRIVER = 0;

/** 라이 배율: [볼스피드, 스핀, 정확도 흔들림] */
const LIE: Record<Surface, [number, number, number]> = {
  tee: [1, 1, 0],
  fairway: [1, 1, 0],
  fringe: [0.97, 0.9, 0],
  green: [0.97, 0.9, 0],
  rough: [0.9, 0.65, 0.12],
  bunker: [0.72, 0.75, 0.15],
  water: [0.5, 0.5, 0.3],
  ob: [0.9, 0.65, 0.12],
};
/** 벙커에서 웨지는 덜 잃는다 */
const BUNKER_WEDGE_SPEED = 0.86;

export interface SwingInput {
  /** 0.05..1.05 */
  power: number;
  /** −1.2..1.2, 0 = 정타, + = 오른쪽으로 휨 */
  accuracy: number;
}

/** 클럽·스윙·라이 → 트랙맨 부호의 발사 조건 (방위각·스핀축은 조준 방향 기준) */
export function launchFor(club: Club, swing: SwingInput, lie: Surface, wobble = 0): ShotInput {
  const [kSpeed0, kSpin, kWobble] = LIE[lie];
  const wedge = club.id === 'sw' || club.id === 'pw';
  const kSpeed = lie === 'bunker' && wedge ? BUNKER_WEDGE_SPEED : kSpeed0;
  const p = Math.min(Math.max(swing.power, 0.05), 1.05);
  const over = Math.max(0, p - 1);
  const a = Math.min(Math.max(swing.accuracy + kWobble * wobble, -1.2), 1.2) * (1 + 6 * over);
  return {
    mph: club.mph * p * kSpeed,
    launchDeg: club.deg,
    azimuthDeg: 2.5 * a,
    rpm: club.rpm * (0.55 + 0.45 * Math.min(p, 1)) * kSpin,
    spinAxisDeg: 20 * a,
  };
}

const carryMemo = new Map<string, number>();
/** 풀스윙(파워 1, 정타, 무풍, 페어웨이) 캐리 (yd) */
export function fullCarryYd(club: Club): number {
  if (club.putter) return 0;
  const hit = carryMemo.get(club.id);
  if (hit !== undefined) return hit;
  const r = simulate(toEngineLaunch(launchFor(club, { power: 1, accuracy: 0 }, 'fairway')), { samples: false });
  carryMemo.set(club.id, r.carryYd);
  return r.carryYd;
}
