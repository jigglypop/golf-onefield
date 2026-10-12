#!/bin/sh
# 진짜 wasm simd128 4레인 경로(W4)를 Node 에서 시험한다 (네이티브 cargo test 는 P4 만 본다).
#   tests.rs 의 w4_matches_scalar: W4 대 스칼라 f32 (허용 1e-6), W4 대 P4 (비트 동일), 앱 걸음·환경·퇴화 샷 포함.
# wasm-pack 이 맞는 wasm-bindgen-test-runner 를 내려받아 쓴다 (build-wasm.sh 와 같은 캐시).
set -e
cd "$(dirname "$0")"
RUSTFLAGS="-C target-feature=+simd128" wasm-pack test --node --release "$@"
