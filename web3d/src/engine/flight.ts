/**
 * 비행 적분기. flightbench/src/harness.rs `impl Flight3 for Model { fn obs3 }` 의 τ 걸음 RK4 를 그대로 옮겼다.
 *
 *  - 걸음 변수 τ: $d\tau = |u|^{1/2}\,dt$. 상태의 도함수에 $g = |u|^{-1/2}$ 를 곱해 τ 로 적분한다(기본 h = 0.05).
 *  - 최고점: $y'$ 이 + 에서 − 로 바뀌는 걸음 안에서 에르미트 3차의 도함수(2차식) 근.
 *  - 착지: $y$ 가 0 을 지나는 걸음 안에서 에르미트 3차의 근(뉴턴 4회), 위치·속도도 같은 에르미트로 보간.
 *
 * 옮기며 더한 것 (원본 식은 바꾸지 않음):
 *  - 시간 $t$ 를 8번째 상태로 함께 적분한다 ($dt/d\tau = g$). 착지 시각도 에르미트로 보간.
 *  - 착지각은 3차원: $\operatorname{atan2}(-v_y, \sqrt{v_x^2+v_z^2})$ (webrefs.rs 와 같음).
 *  - 방위각 φ: 발사 좌표계 전체를 y 축으로 돌린다(+ 이면 +z 로 출발). 중력이 y 방향뿐이라 정확하다.
 *  - 걸음마다 궤적 표본(실단위)을 남긴다. 화면 애니메이션은 표본 사이를 시간 에르미트로 잇는다.
 *
 * 좌표: x = 타깃 방향, y = 위, z = 오른쪽(뒤에서 볼 때). 부호 약속은 convention.ts 한 곳에 있다.
 */
import { MODEL, type AeroModel, type AeroOut } from './model.ts';
import { dimless, L_M, R_BALL, T_S, U_MS, YD } from './units.ts';

/** 엔진 부호의 발사 조건 (UI 값은 convention.ts 의 toEngineLaunch 로 바꾼다) */
export interface EngineLaunch {
  mph: number;
  launchDeg: number;
  rpm: number;
  /** 엔진 스핀축 a (deg): $\hat\omega = (0, \sin a, \cos a)$. a > 0 이면 −z(왼쪽)로 휜다 */
  axisDeg: number;
  /** 방위각 (deg): + 이면 +z(오른쪽)로 출발 */
  azimuthDeg?: number;
}

/** 궤적 표본 (실단위, 월드 좌표): t s, 위치 m, 속도 m/s */
export interface TrajectorySample {
  t: number;
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
}

export interface FlightResult {
  /** 걸음 한도 안에 떨어졌는가 */
  landed: boolean;
  /** 착지점의 타깃 선(+x) 방향 거리 (yd). 방위각 0 이면 엔진의 캐리와 같다 */
  carryYd: number;
  /** 착지점의 z (yd, + = 오른쪽) */
  lateralYd: number;
  /** 최고 높이 (yd) */
  apexYd: number;
  /** 착지각 (deg, 수평 아래로) */
  landDeg: number;
  /** 체공 시간 (s) */
  timeS: number;
  /** 착지 속도 (m/s) */
  landSpeedMs: number;
  /** 착지 순간 스핀 (rpm, 비행 중 감쇠 뒤). 지면 바운드 계산에 쓴다 */
  landSpinRpm: number;
  /** 최고점 (m, 월드) */
  apex: { x: number; y: number; z: number; t: number };
  /** 처음(발사)과 끝(착지, y = 0)을 포함한 걸음별 표본 */
  samples: TrajectorySample[];
  /** τ 걸음 수 */
  steps: number;
}

export interface FlightOptions {
  /** τ 걸음 (기본 0.05, harness.rs Model::h 와 같음) */
  h?: number;
  /** 공기역학 장 (기본 model.ts 의 MODEL) */
  model?: AeroModel;
  /** τ 한도 (기본 40; harness.rs 는 12) */
  maxTau?: number;
  /** 궤적 표본을 남길지 (기본 true) */
  samples?: boolean;
  /**
   * 바람 (m/s, 수평). 좌표는 출력 표본과 같은 틀(+x = 방위각 0 의 발사 방향, +z = 오른쪽).
   * 장은 공기에 대한 속도 $\mathbf u - \mathbf u_{wind}$ 로 계산한다. 게임용 기능이며 자료로 검증하지 않았다.
   */
  wind?: { x: number; z: number };
  /**
   * 돌풍 (게임용, 자료로 검증하지 않았다). 평균 바람 `wind` 위에 시간에 따라 변하는 항을 더한다.
   * 기본값은 없음(undefined, 진폭 0): 돌풍 없이 부르면 계산 경로가 예전과 같아 web_refs.csv 대조 결과(0.05 yd)가 그대로다.
   */
  gust?: GustOptions;
}

/**
 * 돌풍 설정. 시각 $t$ 의 돌풍 벡터 (샷 틀, m/s):
 * $\mathbf g(t) = A\,(a(t)\,\hat w + 0.4\,b(t)\,\hat w_\perp)$,
 * $a(t) = [\sin(\omega_1 t+\varphi_1) + 0.6\sin(\omega_2 t+\varphi_2)]/1.6$ 이고 $b(t)$ 도 같은 꼴(다른 각주파수·위상).
 * $\hat w$ 는 평균 바람 방향(바람이 0 이면 +x), $\hat w_\perp$ 는 그 왼쪽 법선이다.
 * 각주파수·위상은 `seed` 에서 정해져 같은 씨앗이면 같은 돌풍이다 (결정적).
 */
export interface GustOptions {
  /** 진폭 A (m/s). 0 이하이면 돌풍 없음 */
  amp: number;
  /** 샷마다 다른 씨앗 (정수) */
  seed: number;
  /** 주된 변동 주기 (s). 기본 2.8 */
  period?: number;
}

/** 돌풍 벡터를 계산하는 내부 상태 */
interface GustCtx {
  amp: number;
  /** 평균 바람 단위 방향과 그 왼쪽 법선 (샷 틀) */
  ux: number;
  uz: number;
  px: number;
  pz: number;
  w: [number, number, number, number];
  p: [number, number, number, number];
  /** 샷 틀 → 적분 틀 회전 (방위각) */
  c: number;
  s: number;
  out: Float64Array;
}

/** 결정적 난수 [0, 1) */
function hash01(seed: number, k: number): number {
  let t = (Math.imul(seed | 0, 0x9e3779b1) + Math.imul(k + 1, 0x85ebca6b) + 0x6d2b79f5) | 0;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

function makeGust(g: GustOptions, wx: number, wz: number, cphi: number, sphi: number): GustCtx {
  const speed = Math.hypot(wx, wz);
  const ux = speed > 0.05 ? wx / speed : 1;
  const uz = speed > 0.05 ? wz / speed : 0;
  const base = (2 * Math.PI) / (g.period ?? 2.8);
  const w: GustCtx['w'] = [base * (0.8 + 0.4 * hash01(g.seed, 0)), base * (2.1 + 0.6 * hash01(g.seed, 1)), base * (0.9 + 0.5 * hash01(g.seed, 2)), base * (2.4 + 0.7 * hash01(g.seed, 3))];
  const p: GustCtx['p'] = [2 * Math.PI * hash01(g.seed, 4), 2 * Math.PI * hash01(g.seed, 5), 2 * Math.PI * hash01(g.seed, 6), 2 * Math.PI * hash01(g.seed, 7)];
  return { amp: g.amp, ux, uz, px: -uz, pz: ux, w, p, c: cphi, s: sphi, out: new Float64Array(2) };
}

/** 시각 t (s) 의 돌풍 벡터 (샷 틀, m/s) 를 gc.out 에 */
function gustShot(gc: GustCtx, t: number): void {
  const a = (Math.sin(gc.w[0] * t + gc.p[0]) + 0.6 * Math.sin(gc.w[1] * t + gc.p[1])) / 1.6;
  const b = (Math.sin(gc.w[2] * t + gc.p[2]) + 0.6 * Math.sin(gc.w[3] * t + gc.p[3])) / 1.6;
  gc.out[0] = gc.amp * (a * gc.ux + 0.4 * b * gc.px);
  gc.out[1] = gc.amp * (a * gc.uz + 0.4 * b * gc.pz);
}

/** 시각 t (s) 의 돌풍 벡터 (샷 틀, m/s). HUD 표시·시험용. `wind` 는 평균 바람 */
export function gustVector(g: GustOptions, wind: { x: number; z: number }, t: number): { x: number; z: number } {
  if (!(g.amp > 0)) return { x: 0, z: 0 };
  const gc = makeGust(g, wind.x, wind.z, 1, 0);
  gustShot(gc, t);
  return { x: gc.out[0]!, z: gc.out[1]! };
}

const N = 8; // x y z ux uy uz w t

/**
 * τ 에 대한 도함수. harness.rs Model::f3 와 같은 식·순서에 dt/dτ = g 를 더했다.
 * 바람 (wx, wz, 무차원, 적분 틀): 장은 공기에 대한 속도 r = u − wind 로, 걸음 재매개 g 는 땅에 대한 |u| 로 둔다.
 * 바람이 0 이면 러스트 식과 같다. 돌풍(gc)이 있으면 바람에 시각 t = s[7] 의 돌풍을 더한다 (없으면 아무것도 바뀌지 않는다).
 */
function deriv(m: AeroModel, s: Float64Array, oy: number, oz: number, out: Float64Array, kk: AeroOut, wx0 = 0, wz0 = 0, gc: GustCtx | null = null): void {
  const ux = s[3]!;
  const uy = s[4]!;
  const uz = s[5]!;
  const w = s[6]!;
  let wx = wx0;
  let wz = wz0;
  if (gc !== null) {
    gustShot(gc, s[7]! * T_S);
    // 샷 틀 → 적분 틀: 방위각만큼 거꾸로 돌리고 무차원화
    wx += (gc.out[0]! * gc.c + gc.out[1]! * gc.s) / U_MS;
    wz += (-gc.out[0]! * gc.s + gc.out[1]! * gc.c) / U_MS;
  }
  const rx = ux - wx;
  const rz = uz - wz;
  const sp = Math.sqrt(ux * ux + uy * uy + uz * uz);
  const sr = wx === 0 && wz === 0 ? sp : Math.sqrt(rx * rx + uy * uy + rz * rz);
  m.kk(sr, w, kk);
  const g = 1 / Math.sqrt(sp);
  // ω̂ × r, ω̂ = (0, oy, oz)
  const cx = oy * rz - oz * uy;
  const cy = oz * rx;
  const cz = -oy * rx;
  out[0] = ux * g;
  out[1] = uy * g;
  out[2] = uz * g;
  out[3] = (-kk.kd * rx + kk.kl * cx) * g;
  out[4] = (-kk.kd * uy + kk.kl * cy - 1) * g;
  out[5] = (-kk.kd * rz + kk.kl * cz) * g;
  out[6] = -m.beta * w * sr * g;
  out[7] = g;
}

/** 에르미트 기저 [h00, h10, h01, h11] 를 out 에 */
function herm(t: number, out: Float64Array): Float64Array {
  const t2 = t * t;
  const t3 = t2 * t;
  out[0] = 2 * t3 - 3 * t2 + 1;
  out[1] = t3 - 2 * t2 + t;
  out[2] = -2 * t3 + 3 * t2;
  out[3] = t3 - t2;
  return out;
}

/** 한 걸음 안의 에르미트 보간: 성분 i, 걸음 h */
function hermAt(hh: Float64Array, s: Float64Array, k1: Float64Array, n: Float64Array, kn: Float64Array, i: number, h: number): number {
  return hh[0]! * s[i]! + hh[1]! * k1[i]! * h + hh[2]! * n[i]! + hh[3]! * kn[i]! * h;
}

/** 실단위·월드 좌표 표본 (방위각 회전: x_w = x cosφ − z sinφ, z_w = x sinφ + z cosφ) */
function toSample(t: number, x: number, y: number, z: number, ux: number, uy: number, uz: number, c: number, sn: number): TrajectorySample {
  return {
    t: t * T_S,
    x: (x * c - z * sn) * L_M,
    y: y * L_M,
    z: (x * sn + z * c) * L_M,
    vx: (ux * c - uz * sn) * U_MS,
    vy: uy * U_MS,
    vz: (ux * sn + uz * c) * U_MS,
  };
}

export function simulate(launch: EngineLaunch, opt: FlightOptions = {}): FlightResult {
  const m = opt.model ?? MODEL;
  const h = opt.h ?? 0.05;
  const keep = opt.samples !== false;
  const maxSteps = Math.ceil((opt.maxTau ?? 40) / h);
  const [v0, th, s0] = dimless(launch.mph, launch.launchDeg, launch.rpm);
  const a = (launch.axisDeg * Math.PI) / 180;
  const oy = Math.sin(a);
  const oz = Math.cos(a);
  const phi = ((launch.azimuthDeg ?? 0) * Math.PI) / 180;
  const cphi = Math.cos(phi);
  const sphi = Math.sin(phi);
  // 바람: 출력 틀 → 적분 틀 (방위각만큼 거꾸로 돌림), 무차원
  const wxo = opt.wind?.x ?? 0;
  const wzo = opt.wind?.z ?? 0;
  const wx = (wxo * cphi + wzo * sphi) / U_MS;
  const wz = (-wxo * sphi + wzo * cphi) / U_MS;
  // 돌풍 (진폭 0 이거나 없으면 null: 예전 계산 경로 그대로)
  const gc = opt.gust && opt.gust.amp > 0 ? makeGust(opt.gust, wxo, wzo, cphi, sphi) : null;

  const kk: AeroOut = { kd: 0, kl: 0 };
  let s = new Float64Array(N);
  let n = new Float64Array(N);
  let k1 = new Float64Array(N);
  let kn = new Float64Array(N);
  const k2 = new Float64Array(N);
  const k3 = new Float64Array(N);
  const k4 = new Float64Array(N);
  const tmp = new Float64Array(N);
  const hh = new Float64Array(4);

  s[3] = v0 * Math.cos(th);
  s[4] = v0 * Math.sin(th);
  s[6] = s0 * v0;
  deriv(m, s, oy, oz, k1, kk, wx, wz, gc);

  const samples: TrajectorySample[] = [];
  if (keep) samples.push(toSample(0, 0, 0, 0, s[3]!, s[4]!, s[5]!, cphi, sphi));
  let apex = 0;
  let apexX = 0;
  let apexZ = 0;
  let apexT = 0;
  const h2 = 0.5 * h;
  const h6 = h / 6;

  for (let step = 0; step < maxSteps; step++) {
    for (let i = 0; i < N; i++) tmp[i] = s[i]! + h2 * k1[i]!;
    deriv(m, tmp, oy, oz, k2, kk, wx, wz, gc);
    for (let i = 0; i < N; i++) tmp[i] = s[i]! + h2 * k2[i]!;
    deriv(m, tmp, oy, oz, k3, kk, wx, wz, gc);
    for (let i = 0; i < N; i++) tmp[i] = s[i]! + h * k3[i]!;
    deriv(m, tmp, oy, oz, k4, kk, wx, wz, gc);
    for (let i = 0; i < N; i++) n[i] = s[i]! + h6 * (k1[i]! + 2 * k2[i]! + 2 * k3[i]! + k4[i]!);
    deriv(m, n, oy, oz, kn, kk, wx, wz, gc);

    const y0 = s[1]!;
    const y1 = n[1]!;
    const m0 = k1[1]! * h;
    const m1 = kn[1]! * h;
    // 최고점: y(τ) 에르미트의 도함수 a t² + b t + c 의 [0, 1] 근
    if (k1[1]! > 0 && kn[1]! <= 0) {
      const qa = 6 * y0 + 3 * m0 - 6 * y1 + 3 * m1;
      const qb = -6 * y0 - 4 * m0 + 6 * y1 - 2 * m1;
      let tt: number;
      if (Math.abs(qa) < 1e-14) {
        tt = -m0 / qb;
      } else {
        const d = Math.sqrt(Math.max(qb * qb - 4 * qa * m0, 0));
        const r1 = (-qb - d) / (2 * qa);
        const r2 = (-qb + d) / (2 * qa);
        tt = r1 >= 0 && r1 <= 1 ? r1 : r2;
      }
      tt = Math.min(Math.max(tt, 0), 1);
      herm(tt, hh);
      const ya = hh[0]! * y0 + hh[1]! * m0 + hh[2]! * y1 + hh[3]! * m1;
      if (ya > apex) {
        apex = ya;
        apexX = hermAt(hh, s, k1, n, kn, 0, h);
        apexZ = hermAt(hh, s, k1, n, kn, 2, h);
        apexT = hermAt(hh, s, k1, n, kn, 7, h);
      }
    }
    // 착지: y 가 0 을 지나는 걸음
    if (y1 < 0 && y0 >= 0) {
      // 첫 걸음 안에서 떨어지면(y0 = 0) t = 0 이 아닌 근을 위에서부터 찾는다
      let tt = y0 <= 0 ? 1 : y0 / (y0 - y1);
      for (let it = 0; it < 4; it++) {
        herm(tt, hh);
        const f = hh[0]! * y0 + hh[1]! * m0 + hh[2]! * y1 + hh[3]! * m1;
        const t2 = tt * tt;
        const df = (6 * t2 - 6 * tt) * (y0 - y1) + (3 * t2 - 4 * tt + 1) * m0 + (3 * t2 - 2 * tt) * m1;
        tt = Math.min(Math.max(tt - f / df, 0), 1);
      }
      herm(tt, hh);
      const x = hermAt(hh, s, k1, n, kn, 0, h);
      const z = hermAt(hh, s, k1, n, kn, 2, h);
      const vx = hermAt(hh, s, k1, n, kn, 3, h);
      const vy = hermAt(hh, s, k1, n, kn, 4, h);
      const vz = hermAt(hh, s, k1, n, kn, 5, h);
      const t = hermAt(hh, s, k1, n, kn, 7, h);
      const wLand = hermAt(hh, s, k1, n, kn, 6, h);
      const landing = toSample(t, x, 0, z, vx, vy, vz, cphi, sphi);
      if (keep) samples.push(landing);
      const xw = x * cphi - z * sphi;
      const zw = x * sphi + z * cphi;
      return {
        landed: true,
        carryYd: xw * YD,
        lateralYd: zw * YD,
        apexYd: apex * YD,
        landDeg: (Math.atan2(-vy, Math.sqrt(vx * vx + vz * vz)) * 180) / Math.PI,
        timeS: t * T_S,
        landSpeedMs: Math.sqrt(vx * vx + vy * vy + vz * vz) * U_MS,
        landSpinRpm: ((wLand * U_MS) / R_BALL) * (60 / (2 * Math.PI)),
        apex: { x: (apexX * cphi - apexZ * sphi) * L_M, y: apex * L_M, z: (apexX * sphi + apexZ * cphi) * L_M, t: apexT * T_S },
        samples,
        steps: step + 1,
      };
    }
    if (keep) samples.push(toSample(n[7]!, n[0]!, n[1]!, n[2]!, n[3]!, n[4]!, n[5]!, cphi, sphi));
    // 다음 걸음: n → s, kn → k1 (배열을 바꿔 끼워 복사를 피한다)
    const ts = s;
    s = n;
    n = ts;
    const tk = k1;
    k1 = kn;
    kn = tk;
  }
  const last = samples[samples.length - 1];
  return {
    landed: false,
    carryYd: NaN,
    lateralYd: NaN,
    apexYd: apex * YD,
    landDeg: NaN,
    timeS: last ? last.t : NaN,
    landSpeedMs: NaN,
    landSpinRpm: NaN,
    apex: { x: (apexX * cphi - apexZ * sphi) * L_M, y: apex * L_M, z: (apexX * sphi + apexZ * cphi) * L_M, t: apexT * T_S },
    samples,
    steps: maxSteps,
  };
}

/** 표본 사이 3차 에르미트(시간)로 t 초의 위치를 out 에. 범위 밖이면 끝점. 반환: 구간 번호 */
export function positionAt(samples: readonly TrajectorySample[], t: number, out: { x: number; y: number; z: number }, hint = 0): number {
  const last = samples.length - 1;
  if (last < 0) return 0;
  const first = samples[0]!;
  if (t <= first.t || last === 0) {
    out.x = first.x;
    out.y = first.y;
    out.z = first.z;
    return 0;
  }
  const end = samples[last]!;
  if (t >= end.t) {
    out.x = end.x;
    out.y = end.y;
    out.z = end.z;
    return last - 1;
  }
  // 앞 프레임의 구간에서 시작해 앞으로 찾는다 (애니메이션은 시간이 늘기만 한다)
  let i = Math.min(Math.max(hint, 0), last - 1);
  if (samples[i]!.t > t) i = 0;
  while (i < last - 1 && samples[i + 1]!.t < t) i++;
  const p = samples[i]!;
  const q = samples[i + 1]!;
  const dt = q.t - p.t;
  const u = (t - p.t) / dt;
  const u2 = u * u;
  const u3 = u2 * u;
  const h00 = 2 * u3 - 3 * u2 + 1;
  const h10 = (u3 - 2 * u2 + u) * dt;
  const h01 = -2 * u3 + 3 * u2;
  const h11 = (u3 - u2) * dt;
  out.x = h00 * p.x + h10 * p.vx + h01 * q.x + h11 * q.vx;
  out.y = h00 * p.y + h10 * p.vy + h01 * q.y + h11 * q.vy;
  out.z = h00 * p.z + h10 * p.vz + h01 * q.z + h11 * q.vz;
  return i;
}

/** 궤적을 일정한 시간 간격 dt 초로 다시 뽑는다 (그리기용). 마지막 점은 착지점. */
export function resample(samples: readonly TrajectorySample[], dt: number): { t: Float64Array; xyz: Float32Array } {
  const end = samples.length > 0 ? samples[samples.length - 1]!.t : 0;
  const count = Math.max(2, Math.ceil(end / dt) + 1);
  const t = new Float64Array(count);
  const xyz = new Float32Array(count * 3);
  const p = { x: 0, y: 0, z: 0 };
  let hint = 0;
  for (let k = 0; k < count; k++) {
    const tk = k === count - 1 ? end : k * dt;
    hint = positionAt(samples, tk, p, hint);
    t[k] = tk;
    xyz[3 * k] = p.x;
    xyz[3 * k + 1] = p.y;
    xyz[3 * k + 2] = p.z;
  }
  return { t, xyz };
}
