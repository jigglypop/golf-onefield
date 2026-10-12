// golf-core 의 wasm-bindgen 산출물(JS 글루 + .wasm + d.ts)을 src/physics/ 으로 복사한다.
//   golf-core/pkg      -> src/physics/wasm       (기본 빌드: 어디서나 돈다)
//   golf-core/pkg-simd -> src/physics/wasm-simd  (SIMD 빌드: golfCore.ts 가 WebAssembly.validate 로 지원을 확인하면 이쪽을 불러온다)
// 원본은 golf-core 가 관리하며 이 스크립트는 읽기만 한다. predev / prebuild 에서 자동 실행된다.
// 교차 디렉터리 import 를 피하려고 복사하며, 복사본은 .gitignore 로 커밋하지 않는다.
// 둘 다 golf-core/build-wasm.sh 가 만든다. 하나라도 없으면 멈춘다 (golfCore.ts 의 동적 import 가 둘 다 필요하다).
import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const pairs = [
  ['pkg', 'wasm'],
  ['pkg-simd', 'wasm-simd'],
].map(([from, to]) => ({
  src: path.resolve(here, '../../../golf-core', from),
  dst: path.resolve(here, '../src/physics', to),
}));

for (const { src } of pairs) {
  if (!existsSync(path.join(src, 'golf_core_bg.wasm'))) {
    console.error(`[copy-wasm] 물리 코어 산출물을 찾지 못했습니다: ${src}\n  golf-core/build-wasm.sh 로 pkg 와 pkg-simd 를 먼저 만드세요.`);
    process.exit(1);
  }
}

for (const { src, dst } of pairs) {
  rmSync(dst, { recursive: true, force: true });
  mkdirSync(dst, { recursive: true });
  // pkg/.gitignore 는 원본 저장소용이라 복사하지 않는다
  cpSync(src, dst, { recursive: true, filter: (p) => path.basename(p) !== '.gitignore' });
  console.log(`[copy-wasm] ${src} -> ${dst}`);
}
