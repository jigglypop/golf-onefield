// golf-core wasm 로더: SIMD 를 지원하는 브라우저/런타임이면 pkg-simd/, 아니면 기본 pkg/ 를 불러온다.
//
//   import { loadGolfCore } from './js/load.mjs';
//   const core = await loadGolfCore();           // { fly_f64, batch, batch_simd, fly_env, ..., simd: true|false }
//   const out = core.batch_simd(shots, 0.137);   // SIMD 가 없으면 같은 이름이 이식 가능한 4레인 판본이다
//
// 기본 pkg/ 는 어디서나 돈다. 안경(Meta Ray-Ban Display) 브라우저가 wasm SIMD 를 지원하는지는 아직 모른다:
// 그래서 지원 검사(WebAssembly.validate)가 실패하거나 pkg-simd/ 가 없거나 불러오다 실패하면 항상 pkg/ 로 돌아간다.

/** v128 상수 하나를 쓰는 29바이트 모듈: SIMD 를 이해하는 엔진이면 validate 가 true */
export const SIMD_PROBE = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15, 253, 98, 11]);

export function simdSupported() {
  try {
    return typeof WebAssembly === 'object' && WebAssembly.validate(SIMD_PROBE);
  } catch {
    return false;
  }
}

const isNode = typeof process !== 'undefined' && !!process.versions?.node;

async function loadFrom(dirUrl) {
  const mod = await import(new URL('golf_core.js', dirUrl).href);
  const wasmUrl = new URL('golf_core_bg.wasm', dirUrl);
  if (isNode) {
    const { readFileSync } = await import('node:fs');
    const { fileURLToPath } = await import('node:url');
    mod.initSync({ module: readFileSync(fileURLToPath(wasmUrl)) });
  } else {
    await mod.default({ module_or_path: wasmUrl });
  }
  return mod;
}

/**
 * @param {{simd?: 'auto'|boolean, base?: URL}} opt  simd: 'auto'(기본) = 지원되면 SIMD, true = SIMD 강제(실패하면 오류), false = 기본 빌드
 * @returns 모듈의 모든 내보내기 + simd(진짜 simd128 커널을 쓰는가)
 */
export async function loadGolfCore({ simd = 'auto', base = new URL('../', import.meta.url) } = {}) {
  const want = simd === true || (simd === 'auto' && simdSupported());
  if (want) {
    try {
      const mod = await loadFrom(new URL('pkg-simd/', base));
      return { ...mod, simd: mod.has_simd() };
    } catch (e) {
      if (simd === true) throw e;
      // 자동 모드: 조용히 기본 빌드로
    }
  }
  const mod = await loadFrom(new URL('pkg/', base));
  return { ...mod, simd: mod.has_simd() };
}
