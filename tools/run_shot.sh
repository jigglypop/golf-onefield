#!/usr/bin/env bash
# 영상 한 개 → 공 추적 → 역보정.  사용: tools/run_shot.sh shot.mp4 [fps=240] [hfov=70] [wall_m] [추가 인수...]
#   wall_m 을 주면 벽 반동 역보정(screen), 없으면 비행만(spininv).
#   보정 던지기: tools/run_shot.sh throw.mp4 240 70 3.0 --throw
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
vid="$1"; fps="${2:-240}"; hfov="${3:-70}"; wall="${4:-}"; shift $(( $# < 4 ? $# : 4 )) || true
csv="${vid%.*}.csv"
(cd "$here/flightbench" && cargo build -q --profile quick --bin screen --bin spininv)
if [[ -n "$wall" ]]; then
  python3 -B "$here/tools/track_ball.py" "$vid" --fps "$fps" --hfov "$hfov" --wall "$wall" -o "$csv" --debug "${vid%.*}_track.mp4"
  "$here/flightbench/target/quick/screen" --csv "$csv" --wall "$wall" --cam side "$@"
else
  python3 -B "$here/tools/track_ball.py" "$vid" --fps "$fps" --hfov "$hfov" -o "$csv" --debug "${vid%.*}_track.mp4"
  "$here/flightbench/target/quick/spininv" --csv "$csv" --cam side "$@"
fi
