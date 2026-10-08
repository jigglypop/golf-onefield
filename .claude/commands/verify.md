---
description: 바꾼 뒤 회귀 점검 (빌드·단위 시험·시험대·가짜 영상 왕복)
---
차례로 실행하고 결과를 한 표로 보고한다. 하나라도 기준을 벗어나면 원인을 찾는다.

1. `cd flightbench && cargo build --profile quick 2>&1 | grep -E "^(warning|error)"` → 없어야 함
2. `cargo test --profile quick --lib` → 전부 통과
3. `./target/quick/quick` → 공유위기 LPGA 캐리 ≈ 2.85, USGA ≈ 7.34 (±0.05)
4. `cd .. && tools/e2e_synth.sh` → 7번 캐리 167.9 (참 168.2), 던지기 k_t 0.285 ±0.002
