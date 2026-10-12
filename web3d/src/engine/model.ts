/**
 * 공기역학 장 (모형을 바꿀 때는 이 파일만 고친다).
 *
 * 지금 장: '공유위기12' = 공유 포화 장 + 가파른 항력 위기 (flightbench/models.txt [공유위기12],
 * engine.rs `default_field()` = `SharedRe<12>`). $|u|=$ sp (무차원 속도), $w$ (무차원 스핀),
 * $Re = |u|\cdot$RE_PER_U, $u_0 = Re_0/$RE_PER_U 일 때
 *
 * $$k_d = |u|C_D = \Big(v + \frac{e}{1 + (Re/Re_0)^n}\Big)|u| + d\,\frac{w|u|}{|u| + l w},\qquad
 *   k_l = |u|C_L = c\,\frac{w|u|}{|u| + l w},\qquad \dot w = -\beta\,w\,|u|$$
 *
 * 매개변수는 PGA 투어표 세 양으로만 맞춘 값이고, 지수 n = 12 는 투어 23행 교차검증으로 골랐다(README 2026-10-12).
 * n = 4·12 는 engine.rs 와 같은 계산 순서(q = |u|²/u0², q·q 또는 (q³)²)를, 그 밖은 Math.pow 를 쓴다.
 * 장을 바꾸면 참값을 다시 뽑는다: flightbench 에서 `cargo run --profile quick --bin webrefs > ../web3d/test/web_refs.csv`.
 */
import { RE_PER_U } from './units.ts';

/** kk() 의 출력 칸 (할당 없이 돌려받기 위해 호출하는 쪽이 하나를 만들어 다시 쓴다) */
export interface AeroOut {
  kd: number;
  kl: number;
}

export interface AeroModel {
  /** 화면·보고용 이름 */
  readonly name: string;
  /** 스핀 감쇠 $\dot w = -\beta w|u|$ */
  readonly beta: number;
  /** (|u|, w) → out.kd = |u|C_D, out.kl = |u|C_L */
  kk(sp: number, w: number, out: AeroOut): void;
}

/** 공유위기12 매개변수 (PGA 만 훈련, engine.rs default_field). n 은 항력 위기의 가파름(지수). */
export const SHARED_RE_PARAMS = {
  v: 0.1383,
  e: 0.254,
  r0: 74003.032,
  n: 12,
  d: 1.4838,
  c: 2.8062,
  l: 6.5014,
  beta: 0.06,
} as const;

/** 이전 기본 장 '공유위기' (n = 4, engine.rs field_n4). 비교용 */
export const SHARED_RE_N4_PARAMS = {
  v: 0.133,
  e: 0.3247,
  r0: 62859.6581,
  n: 4,
  d: 1.3688,
  c: 2.7901,
  l: 5.9226,
  beta: 0.06,
} as const;

export type SharedReParams = { readonly [K in keyof typeof SHARED_RE_PARAMS]: number };

export function createSharedRe(over: Partial<SharedReParams> = {}): AeroModel {
  const p: SharedReParams = { ...SHARED_RE_PARAMS, ...over };
  const u0 = p.r0 / RE_PER_U;
  const u02 = u0 * u0;
  const { v, e, n, d, c, l } = p;
  const name = `공유위기${n}`;
  // (Re/Re0)^n = q^(n/2), q = |u|²/u0². engine.rs SharedRe::kk 와 같은 순서로 계산한다
  const crisis: (sp: number) => number =
    n === 4
      ? (sp) => {
          const q = (sp * sp) / u02;
          return q * q;
        }
      : n === 12
        ? (sp) => {
            const q = (sp * sp) / u02;
            const q3 = q * q * q;
            return q3 * q3;
          }
        : (sp) => Math.pow(sp / u0, n);
  return {
    name,
    beta: p.beta,
    kk(sp, w, out) {
      const r = (w * sp) / (sp + l * w);
      out.kd = (v + e / (1 + crisis(sp))) * sp + d * r;
      out.kl = c * r;
    },
  };
}

/** 앱 전체가 쓰는 장 */
export const MODEL: AeroModel = createSharedRe();
