#!/usr/bin/env bash
# 공개 TrackMan 드라이버 자료(약 1만 샷)를 내려받는다. 라이선스 표기가 없어 저장소에는 넣지 않고 data/real/external/ (gitignore) 에 둔다.
# 출처: https://github.com/tim-blackmore/launch-monitor-regression  (data.csv)
# 사용 뒤: TM_CSV=data/real/external/trackman_public/data.csv MODELS=models_search.txt (cd flightbench && ./target/quick/real)
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
out="$here/data/real/external/trackman_public"; mkdir -p "$out"
curl -fsSL -o "$out/data.csv" "https://raw.githubusercontent.com/tim-blackmore/launch-monitor-regression/main/data.csv"
echo "받음: $out/data.csv ($(wc -l < "$out/data.csv") 줄)"
