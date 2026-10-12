#!/bin/sh
# wasm 패키지를 둘 만든다 (브라우저용 ES 모듈). 생성물 pkg/, pkg-simd/ 는 커밋하지 않는다.
#   pkg/       기본 빌드: wasm SIMD 없이 어디서나 돈다. 내보내기는 기존 그대로(fly_f64, fly_f32, batch, ...) + 환경·4레인 함수
#   pkg-simd/  SIMD 빌드 (-C target-feature=+simd128): 같은 내보내기, batch_simd 가 진짜 v128 커널.
#              지원 검사와 선택은 js/load.mjs 가 한다 (기본값은 항상 pkg/).
# 뒤의 node 한 줄: wasm-pack 이 쓰는 package.json 에 "type": "module" 을 넣는다 (Node 에서 bench/bench.mjs 가 pkg 를 불러오려면 필요).
set -e
cd "$(dirname "$0")"
fix_pkg() {
  node -e 'const f=process.argv[1]+"/package.json",p=require("./"+f);p.type="module";require("fs").writeFileSync(f,JSON.stringify(p,null,2))' "$1"
}
wasm-pack build --target web --release --out-dir pkg
fix_pkg pkg
RUSTFLAGS="-C target-feature=+simd128" wasm-pack build --target web --release --out-dir pkg-simd
fix_pkg pkg-simd
echo "wasm 준비 완료: golf-core/pkg/ (기본), golf-core/pkg-simd/ (SIMD)"
