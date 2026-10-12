// Rust(wasm) 비행 코어 래퍼. 물리 모델은 golf-core 에 있고, 여기서는 호출과 단위 정리만 한다.
// 출력 좌표계(wasm): x = 앞(목표 방향, m), y = 위(m), z = 오른쪽(m, 오른손잡이 골퍼 기준 +).
//
// 경계 비용: 샷 하나는 `fly_flat` 한 번(+ 글루의 해제 한 번 = JS↔wasm 호출 2번, 결과 복사 1번)이다.
// 예전의 `fly_f64` 객체 API 는 값마다 getter 를 불러(11개) + `path()` + `free()` 까지 샷당 15번 경계를 넘었다.
// 걸음은 golf-core 의 앱 걸음(`app_step()`, `app_min_steps()`)을 읽어 쓴다: τ 0.137 + 최소 6걸음 (web3d 참값 52샷 대비 캐리 오차 5e-4 yd).
// 걸음이 성겨(샷당 약 21점) 경로는 점마다 위치와 속도를 담고, 소비자(ShotBall.tsx)가 3차 에르미트로 잇는다 (flat.ts).
//
// SIMD: 브라우저가 wasm SIMD 를 이해하면(`WebAssembly.validate` 로 검사) pkg-simd 를, 아니면 pkg 를 불러온다.
// 샷 하나(`fly_flat`)는 스칼라 f64 라 어느 쪽이든 같은 값이다. 차이는 일괄 계산(클럽표 `batchCarryYd`)에만 있다:
// SIMD 가 없으면 `batch_simd_opts` 가 같은 연산의 스칼라 f32 로 돌아서 샷당 1 µs 목표가 성립하지 않는다(`CoreInfo.simd`).
import type * as CoreModule from './wasm/golf_core.js';
import { M_PER_YD, parseStats, pathOf, type ShotStats } from './flat';

export { M_PER_YD } from './flat';
export type { ShotStats } from './flat';

/** 샷 입력. hlaDeg 는 wasm 에 넘기지 않는다 (출발 방향 회전은 장면에서 적용) */
export interface ShotParams {
  /** 볼 스피드, mph */
  mph: number;
  /** 발사각, 도 */
  launchDeg: number;
  /** 백스핀, rpm */
  spinRpm: number;
  /** 스핀축 기울기, 도. 양수 = 훅(왼쪽으로 휨), 음수 = 슬라이스(오른쪽으로 휨) */
  axisDeg: number;
  /** 수평 출발 방향(HLA), 도. 양수 = 오른쪽 */
  hlaDeg: number;
}

export interface SimulatedShot {
  stats: ShotStats;
  /** 점마다 [t s, x m, y m, z m, vx m/s, vy m/s, vz m/s] (PATH_STRIDE = 7칸). 마지막 점이 착지점. 점이 없을 수 있다(위로 나가지 않는 샷) */
  path: Float64Array;
}

/** 불러온 코어의 상태 */
export interface CoreInfo {
  /** 진짜 wasm simd128 4레인 커널을 쓰는가. false 면 일괄 계산이 스칼라 f32 라서 샷당 1 µs 목표가 성립하지 않는다 */
  simd: boolean;
  /** 앱 걸음 τ (golf-core `app_step()`) */
  tau: number;
  /** 최소 걸음 수 (golf-core `app_min_steps()`) */
  minSteps: number;
}

type Core = typeof CoreModule;
let loaded: { mod: Core; info: CoreInfo } | null = null;
let ready: Promise<CoreInfo> | null = null;

/** v128 상수 하나를 쓰는 29바이트 모듈: SIMD 를 이해하는 엔진이면 validate 가 true (golf-core/js/load.mjs 와 같다) */
const SIMD_PROBE = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15, 253, 98, 11]);

/** 이 브라우저가 wasm SIMD 를 지원하는가 */
export function simdSupported(): boolean {
  try {
    return typeof WebAssembly === 'object' && WebAssembly.validate(SIMD_PROBE);
  } catch {
    return false;
  }
}

async function instantiate(): Promise<{ mod: Core; simd: boolean }> {
  if (simdSupported()) {
    try {
      const mod: Core = await import('./wasm-simd/golf_core.js');
      await mod.default();
      if (mod.has_simd()) return { mod, simd: true };
    } catch (error) {
      // 검사는 통과했지만 불러오기·컴파일에 실패하면 기본 빌드로 돌아간다 (조용히 죽지 않도록 콘솔에는 남긴다)
      console.warn('[golf-core] SIMD 빌드를 불러오지 못해 기본 빌드로 돌아갑니다:', error);
    }
  }
  const mod: Core = await import('./wasm/golf_core.js');
  await mod.default();
  return { mod, simd: mod.has_simd() };
}

/** wasm 을 한 번만 불러온다 (SIMD 가 되면 pkg-simd). 실패하면 다음 호출에서 다시 시도한다. */
export function loadGolfCore(): Promise<CoreInfo> {
  ready ??= instantiate().then(
    ({ mod, simd }) => {
      const info: CoreInfo = { simd, tau: mod.app_step(), minSteps: mod.app_min_steps() };
      loaded = { mod, info };
      return info;
    },
    (error: unknown) => {
      ready = null;
      throw error;
    },
  );
  return ready;
}

function core(): { mod: Core; info: CoreInfo } {
  if (!loaded) throw new Error('물리 코어가 아직 준비되지 않았습니다 (loadGolfCore 가 끝난 뒤에 부르세요)');
  return loaded;
}

/** 샷 하나를 날린다: wasm 호출 한 번으로 요약값과 경로가 한 배열에 온다 (경로는 같은 메모리를 보는 뷰). */
export function simulateShot(params: ShotParams): SimulatedShot {
  const { mod, info } = core();
  const a = mod.fly_flat(params.mph, params.launchDeg, params.spinRpm, params.axisDeg, info.tau, info.minSteps);
  return { stats: parseStats(a), path: pathOf(a) };
}

/** 일괄 계산의 입력 한 줄 (HLA 는 캐리에 영향이 없다) */
export type BatchInput = Pick<ShotParams, 'mph' | 'launchDeg' | 'spinRpm' | 'axisDeg'>;

/**
 * 샷 여러 개의 캐리(yd)를 일괄 계산 한 번으로 구한다 (클럽표). 앱 걸음 τ + 최소 걸음 수, 4레인 f32 커널
 * (SIMD 가 없으면 같은 연산의 스칼라 f32 — `CoreInfo.simd` 로 구분). 단일 샷 f64 와의 차이는 캐리 5e-4 yd 안쪽.
 */
export function batchCarryYd(inputs: readonly BatchInput[]): Float64Array {
  const { mod, info } = core();
  const flat = new Float64Array(inputs.length * 4);
  inputs.forEach((s, i) => {
    flat[4 * i] = s.mph;
    flat[4 * i + 1] = s.launchDeg;
    flat[4 * i + 2] = s.spinRpm;
    flat[4 * i + 3] = s.axisDeg;
  });
  const out = mod.batch_simd_opts(flat, info.tau, info.minSteps, false); // [캐리 m, 편차 m, 높이 m, 착지각] × n
  const carry = new Float64Array(inputs.length);
  for (let i = 0; i < inputs.length; i++) carry[i] = out[4 * i]! / M_PER_YD;
  return carry;
}
