/**
 * 스윙 애니메이션의 순수 수학 (그리기는 scene/Golfer.tsx). 공이 임팩트 순간에 떠나도록 시각만 정한다.
 *
 * 시간 $\tau$ = 샷을 정한 뒤 흐른 시간 (shotClock.t + impactDelay). 구간:
 *   백스윙 $[0, B]$: $\varphi = -\Phi_b\,s(u)$, $s(u) = u^2(3-2u)$ (부드럽게 올라간다)
 *   다운스윙 $[B, B+D]$: $\varphi = -\Phi_b\,(1-u^2)$ (가속해서 내려온다; 임팩트 $\tau = B+D$ 에서 $\varphi = 0$, 속도 최대)
 *   팔로스루 $[B+D, B+D+F]$: $\varphi = \Phi_f\,(1-(1-u)^2)$ (감속하며 멈춘다)
 * $\varphi$ 는 클럽이 임팩트 위치(공)에서 목표 쪽으로 쓸고 가는 각 (+ = 목표 쪽, − = 뒤로 들어 올린 쪽).
 * 몸통 비틀림 $\psi$ 도 같은 곡선 (뒤로 돌았다가 앞으로 돈다).
 */
import { SWING_TIMING } from './round.ts';

export interface SwingPose {
  /** 클럽 진자 각 (rad). 0 = 임팩트 자리, − = 백스윙, + = 팔로스루 */
  phi: number;
  /** 몸통 비틀림 (rad). − = 목표 반대쪽으로 돈 쪽, + = 목표 쪽 */
  twist: number;
  /** 0 = 어드레스, 1 = 백스윙 중, 2 = 다운스윙, 3 = 팔로스루 */
  stage: 0 | 1 | 2 | 3;
}

/** 스윙 크기: 풀스윙은 파워에 따라 백스윙 0.9–2.5 rad, 퍼팅은 0.15–0.5 rad */
export function swingAmplitude(power: number, putt: boolean): { back: number; follow: number; twistBack: number; twistFollow: number } {
  const p = Math.min(Math.max(power, 0), 1);
  if (putt) return { back: 0.15 + 0.35 * p, follow: 0.12 + 0.28 * p, twistBack: 0.04, twistFollow: 0.05 };
  return { back: 0.9 + 1.6 * p, follow: 1.3 + 0.8 * p, twistBack: 0.35 + 0.55 * p, twistFollow: 0.9 + 0.4 * p };
}

/** 시각 tau (s) 의 자세. 끝난 뒤에는 팔로스루 자세를 유지한다 */
export function swingPose(tau: number, power: number, putt: boolean): SwingPose {
  const t = putt ? SWING_TIMING.putt : SWING_TIMING.full;
  const a = swingAmplitude(power, putt);
  if (tau <= 0) return { phi: 0, twist: 0, stage: 0 };
  if (tau < t.back) {
    const u = tau / t.back;
    const e = u * u * (3 - 2 * u);
    return { phi: -a.back * e, twist: -a.twistBack * e, stage: 1 };
  }
  if (tau < t.back + t.down) {
    const u = (tau - t.back) / t.down;
    const e = 1 - u * u;
    return { phi: -a.back * e, twist: -a.twistBack * e, stage: 2 };
  }
  const u = Math.min((tau - t.back - t.down) / t.follow, 1);
  const e = 1 - (1 - u) * (1 - u);
  return { phi: a.follow * e, twist: a.twistFollow * e, stage: 3 };
}

/** 스윙 전체 길이 (s): 백스윙 + 다운스윙 + 팔로스루 */
export function swingTotalS(putt: boolean): number {
  const t = putt ? SWING_TIMING.putt : SWING_TIMING.full;
  return t.back + t.down + t.follow;
}

/** 클럽 길이 (m): 퍼터 0.85, 드라이버 1.15, 그 사이는 번호에 따라 짧아진다 (장면에서 막대 길이로만 쓴다) */
export function clubLengthM(idx: number, putter: boolean): number {
  if (putter) return 0.85;
  return Math.max(1.15 - 0.022 * idx, 0.88);
}
