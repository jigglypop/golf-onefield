/** 9홀 코스 형상 점검: 핀·티·조준점·페어웨이 중심선·섬 그린·롱드라이브 그리드 */
import { describe, expect, it } from 'vitest';

import { HOLES, LONG_DRIVE, holeLengthM, surfaceAt } from '../src/game/course.ts';

describe('9홀 형상', () => {
  it('홀 9개, 파 합 35', () => {
    expect(HOLES.length).toBe(9);
    expect(HOLES.reduce((a, h) => a + h.par, 0)).toBe(35);
  });
  for (const h of HOLES) {
    it(`${h.number}번 ${h.name}: 티·핀·조준점·중심선`, () => {
      expect(surfaceAt(h, h.tee[0], h.tee[1])).toBe('tee');
      expect(surfaceAt(h, h.pin[0], h.pin[1])).toBe('green');
      expect(['water', 'ob', 'bunker']).not.toContain(surfaceAt(h, h.teeTarget[0], h.teeTarget[1]));
      for (const f of h.fairways) for (const [x, z] of f.path) expect(['fairway', 'tee', 'fringe', 'green']).toContain(surfaceAt(h, x, z));
      const yd = holeLengthM(h) / 0.9144;
      const range = h.par === 3 ? [120, 230] : h.par === 4 ? [280, 470] : [470, 600];
      expect(yd).toBeGreaterThan(range[0]!);
      expect(yd).toBeLessThan(range[1]!);
    });
  }
  it('섬 그린: 그린·둘레 땅은 물이 아니고, 그 밖은 물', () => {
    const h = HOLES[3]!;
    expect(surfaceAt(h, 134, 4)).toBe('green');
    expect(surfaceAt(h, 134 + 16, 4 + 2)).not.toBe('water');
    expect(surfaceAt(h, 100, 0)).toBe('water');
    expect(surfaceAt(h, 170, 30)).toBe('water');
  });
  it('롱드라이브 그리드: 가운데는 페어웨이, 옆은 OB', () => {
    expect(surfaceAt(LONG_DRIVE, 250, 0)).toBe('fairway');
    expect(surfaceAt(LONG_DRIVE, 250, 40)).toBe('rough');
    expect(surfaceAt(LONG_DRIVE, 250, 60)).toBe('ob');
  });
});
