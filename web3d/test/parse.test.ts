/** screen --csv 출력 붙여넣기 해석 시험 (screen.rs csv_mode 의 println 형식). */
import { describe, expect, it } from 'vitest';

import { parseScreenOutput } from '../src/engine/parse.ts';

describe('screen --csv 출력 해석', () => {
  it('마지막 두 줄', () => {
    const p = parseScreenOutput('볼스피드 120.0 mph, 발사각 16.30°, 방위각 0.01°\n스핀 7061 rpm, 스핀축 8.0°, 캐리 168.2 yd\n');
    expect(p.values).toEqual({ mph: 120, launchDeg: 16.3, azimuthDeg: 0.01, rpm: 7061, spinAxisDeg: -8 });
    expect(p.screenAxisDeg).toBe(8);
    expect(p.screenCarryYd).toBe(168.2);
    expect(p.missing).toEqual([]);
  });

  it('전체 출력: 닫힌 꼴 첫 값 줄은 건너뛰고, 음수도 읽는다', () => {
    const text = [
      '카메라 투영 사용: 축 위치 (1.500, 0.400) m, 거리 3.000 m',
      '프레임 61개 (240 fps), 벽 3.00 m, 카이제곱/자유도 1.02',
      '볼스피드 101.7 mph, 발사각 18.02°, 방위각 -1.25°',
      '벽: k_t 0.2857 (강체 0.2857), 반발계수 e 0.600',
      '닫힌 꼴 첫 값: 스핀 5800 rpm, 축 -3.0°',
      '스핀 6012 rpm, 스핀축 -4.5°, 캐리 131.9 yd',
    ].join('\n');
    const p = parseScreenOutput(text);
    expect(p.values).toEqual({ mph: 101.7, launchDeg: 18.02, azimuthDeg: -1.25, rpm: 6012, spinAxisDeg: 4.5 });
    expect(p.screenCarryYd).toBe(131.9);
  });

  it('보정 샷(--throw) 출력처럼 스핀 줄이 없으면 빠진 항목을 알린다', () => {
    const p = parseScreenOutput('볼스피드 30.2 mph, 발사각 5.10°, 방위각 0.30°\n보정 샷: 다음 샷부터 --kt 0.2500 --kt-sd 0.01 로 쓰세요. 스핀(던진 공) 120 rpm');
    expect(p.values).toEqual({ mph: 30.2, launchDeg: 5.1, azimuthDeg: 0.3 });
    expect(p.missing).toEqual(['스핀', '스핀축']);
  });

  it('빈 글', () => {
    expect(parseScreenOutput('').missing).toEqual(['볼스피드', '발사각', '방위각', '스핀', '스핀축']);
  });
});
