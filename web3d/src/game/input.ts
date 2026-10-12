/**
 * 포인터 끌기 → 조준 변화 (App.tsx 의 usePointerAim 이 쓴다). 순수 함수라 시험할 수 있다.
 * 캔버스를 오른쪽으로 끌면 오른쪽으로 돌린다(+). 퍼터는 더 곱게 돈다.
 */
export const DRAG_DEG_PER_PX = { full: 0.22, putt: 0.1 } as const;

/** 가로로 dxPx (CSS px) 끈 만큼의 조준 변화 (rad) */
export function aimDeltaFromDrag(dxPx: number, putter: boolean): number {
  return (dxPx * (putter ? DRAG_DEG_PER_PX.putt : DRAG_DEG_PER_PX.full) * Math.PI) / 180;
}
