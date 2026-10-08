#!/usr/bin/env bash
# 가짜 영상 왕복 검산: 참 궤적 → 옆 카메라 영상 → 공 추적 → 벽 반동 역보정. 실제 영상을 넣기 전 파이프라인 점검용.
# 사용: tools/e2e_synth.sh [작업 폴더=/tmp/golf_e2e]
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-/tmp/golf_e2e}"; mkdir -p "$out"
(cd "$here/flightbench" && cargo build -q --profile quick --bin screen)
B="$here/flightbench/target/quick/screen"
run() { # 이름 mph deg rpm axis [추가]
  local n=$1; shift
  echo "== $n: $($B --synth "$out/$n.csv" --mph $1 --deg $2 --rpm $3 --axis $4 --fps 240 --wall 3.0 --after 0.2)"
  python3 -B "$here/tools/make_synth_video.py" "$out/$n.csv" "$out/$n.mp4" --fps 240 >/dev/null
  python3 -B "$here/tools/track_ball.py" "$out/$n.mp4" --fps 240 --hfov 70 --wall 3.0 -o "$out/${n}_t.csv" | tail -1
  shift 4
  "$B" --csv "$out/${n}_t.csv" --wall 3.0 --cam side --sig 3 "$@" 2>/dev/null | tail -2
}
run throw 30 22 0 0 --throw
run iron7 120 16.3 7097 8
run driver 167 10.9 2686 8
run wedge 86 25.7 8403 -5
