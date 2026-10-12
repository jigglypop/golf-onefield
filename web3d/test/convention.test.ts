/** 부호 약속 시험: UI 는 트랙맨(스핀축 + = 오른쪽으로 휨), 엔진은 축 + = −z(왼쪽)로 휨, 화면 +z = 오른쪽. */
import { PerspectiveCamera, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';

import { engineAxisFromTrackman, sideLabel, toEngineLaunch, trackmanAxisFromScreen, type ShotInput } from '../src/engine/convention.ts';
import { simulate } from '../src/engine/flight.ts';
import { parseScreenOutput } from '../src/engine/parse.ts';

const SEVEN: ShotInput = { mph: 120, launchDeg: 16.3, rpm: 7097, azimuthDeg: 0, spinAxisDeg: 0 };

describe('스핀축·방위각 부호', () => {
  it('트랙맨 스핀축 + 는 오른쪽(+z), − 는 왼쪽(−z)으로 휜다', () => {
    const fade = simulate(toEngineLaunch({ ...SEVEN, spinAxisDeg: 10 }), { samples: false });
    const draw = simulate(toEngineLaunch({ ...SEVEN, spinAxisDeg: -10 }), { samples: false });
    expect(fade.lateralYd).toBeGreaterThan(10);
    expect(draw.lateralYd).toBeLessThan(-10);
    // 참값: 엔진 축 +10 → 좌우 -14.201131 yd (web_refs.csv, 공유위기12). 트랙맨 −10 이 같은 샷이다.
    expect(draw.lateralYd).toBeCloseTo(-14.201131, 1);
    expect(fade.lateralYd).toBeCloseTo(14.201131, 1);
    expect(engineAxisFromTrackman(10)).toBe(-10);
  });

  it('방위각 + 는 오른쪽으로 출발한다 (스핀축 0 이면 착지도 오른쪽)', () => {
    const o = simulate(toEngineLaunch({ ...SEVEN, azimuthDeg: 3 }));
    expect(o.samples[1]!.z).toBeGreaterThan(0);
    expect(o.lateralYd).toBeGreaterThan(0);
    // 방위각은 발사 좌표계 전체를 돌릴 뿐이라 직선 거리는 그대로다
    const straight = simulate(toEngineLaunch(SEVEN), { samples: false });
    expect(Math.hypot(o.carryYd, o.lateralYd)).toBeCloseTo(straight.carryYd, 6);
  });

  it('티 뒤 카메라에서 +z 는 화면 오른쪽에 보인다', () => {
    const camera = new PerspectiveCamera(50, 16 / 9, 0.5, 4000);
    camera.position.set(-10, 2, 0);
    camera.lookAt(100, 2, 0);
    camera.updateMatrixWorld();
    const right = new Vector3(60, 2, 5).project(camera);
    const left = new Vector3(60, 2, -5).project(camera);
    expect(right.x).toBeGreaterThan(0);
    expect(left.x).toBeLessThan(0);
  });

  it('screen --csv 의 스핀축(엔진 부호)은 뒤집어 넣고, 엔진으로 갈 때 다시 뒤집혀 같은 샷이 된다', () => {
    expect(trackmanAxisFromScreen(8)).toBe(-8);
    const parsed = parseScreenOutput('볼스피드 120.0 mph, 발사각 16.30°, 방위각 0.01°\n스핀 7061 rpm, 스핀축 8.0°, 캐리 168.2 yd');
    const input = { ...SEVEN, ...parsed.values };
    expect(input.spinAxisDeg).toBe(-8);
    expect(toEngineLaunch(input).axisDeg).toBe(8);
    expect(toEngineLaunch(input).azimuthDeg).toBe(0.01);
  });

  it('좌우 표시', () => {
    expect(sideLabel(12.34)).toBe('R 12.3');
    expect(sideLabel(-4.56)).toBe('L 4.6');
    expect(sideLabel(0.04)).toBe('0.0');
  });
});
