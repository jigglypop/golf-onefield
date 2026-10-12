/**
 * 코스 자료 이전 점검: 홀 자료를 course.ts 안의 코드에서 courses/classic9.json 으로 옮겼다.
 * test/fixtures/legacy_holes.json 은 옮기기 전 HOLES·LONG_DRIVE 를 그대로 덤프한 것이다(옮기기 전 코드로 만든 고정 사본).
 * 결과가 홀마다 똑같은지(나무 `trees` 만 새로 더해졌다) 비교한다.
 */
import { describe, expect, it } from 'vitest';

import legacy from './fixtures/legacy_holes.json';
import classic9 from '../src/game/courses/classic9.json';
import { buildHole, CTP_HOLE, HOLES, LONG_DRIVE, surfaceAt, type Hole } from '../src/game/course.ts';

/** 새 자료에서 나무만 뺀 사본 */
const withoutTrees = (h: Hole): Omit<Hole, 'trees'> => {
  const { trees: _trees, ...rest } = h;
  return rest;
};

describe('courses/classic9.json 이전', () => {
  it('홀 수와 니어핀 홀 번호가 같다', () => {
    expect(HOLES.length).toBe(legacy.holes.length);
    expect(CTP_HOLE).toBe(legacy.ctpHole);
    expect(classic9.holes.length).toBe(9);
  });

  for (let i = 0; i < legacy.holes.length; i++) {
    it(`${i + 1}번 홀: 옛 자료와 필드 하나까지 같다 (나무만 새로 있을 수 있다)`, () => {
      expect(withoutTrees(HOLES[i]!)).toEqual(legacy.holes[i]);
    });
  }

  it('롱드라이브 그리드도 같다', () => {
    expect(withoutTrees(LONG_DRIVE)).toEqual(legacy.longDrive);
  });

  it('섬 둘레(land)는 옛 ring() 호출과 같은 40점 다각형이다', () => {
    const land = HOLES[3]!.land!;
    expect(land.length).toBe(1);
    expect(land[0]!.length).toBe(40);
    expect(land).toEqual(legacy.holes[3]!.land);
  });

  it('buildHole 은 JSON 원본을 바꾸지 않는다 (landRings 는 JSON 에 남는다)', () => {
    const raw = classic9.holes[3] as { landRings?: unknown };
    expect(raw.landRings).toBeDefined();
    const built = buildHole(raw);
    expect((built as unknown as { landRings?: unknown }).landRings).toBeUndefined();
    expect(raw.landRings).toBeDefined();
  });
});

describe('나무 장애물 자료', () => {
  const all = HOLES.flatMap((h) => (h.trees ?? []).map((t) => ({ h, t })));

  it('한 코스(1번 홀)에 한두 그루가 있고, 나머지 홀은 그대로다', () => {
    expect(all.length).toBeGreaterThanOrEqual(1);
    expect(all.length).toBeLessThanOrEqual(2);
    expect(all.every(({ h }) => h.number === 1)).toBe(true);
  });

  it('나무는 플레이 구역 안의 러프·페어웨이에 서고 크기가 정상이다 (물·벙커·그린·OB 위가 아니다)', () => {
    for (const { h, t } of all) {
      expect(['rough', 'fairway']).toContain(surfaceAt(h, t.x, t.z));
      expect(t.r).toBeGreaterThan(0.5);
      expect(t.r).toBeLessThan(5);
      expect(t.h).toBeGreaterThan(5);
      expect(t.h).toBeLessThan(30);
    }
  });

  it('땅 판정은 나무와 무관하다 (평지 규칙: 나무가 높이·지면을 바꾸지 않는다)', () => {
    for (const { h, t } of all) {
      const surf = surfaceAt(h, t.x, t.z);
      const stripped = { ...h, trees: [] as Hole['trees'] };
      expect(surfaceAt(stripped as Hole, t.x, t.z)).toBe(surf);
    }
  });
});
