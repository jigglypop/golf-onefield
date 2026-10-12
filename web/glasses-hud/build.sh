#!/bin/sh
# golf-core/pkg 를 ./wasm/ 으로 복사해 이 폴더를 그대로 정적 호스팅할 수 있게 한다 (복사본은 커밋하지 않는다).
# 안경에는 URL 로 올린다 (Ray-Ban Display 웹 앱은 개발자 모드에서 URL 로 불러옴, 2026-05 미리보기 기준 보도).
set -e
cd "$(dirname "$0")"
test -f ../../golf-core/pkg/golf_core_bg.wasm || { echo "먼저 golf-core/build-wasm.sh"; exit 1; }
rm -rf wasm && mkdir wasm
cp ../../golf-core/pkg/golf_core.js ../../golf-core/pkg/golf_core_bg.wasm wasm/
echo "준비: web/glasses-hud/ (index.html + wasm/). 예: python -m http.server 8000 로 미리보기"
