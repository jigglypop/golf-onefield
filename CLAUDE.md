# golf-onefield 작업 규약

제품 요구는 `docs/PRD.md`, 연구 기록·반례는 `README.md`. 답변과 주석은 한국어, 수식은 LaTeX.

## 구조
- `flightbench/` (러스트 2024, AVX-512 필요: `.cargo/config.toml` 의 target-cpu=native)
  - `src/engine.rs` 엔진(H2·Shared·SharedRe, f64 기준 `run`, f32×16 빠른 `stream`), `src/harness.rs` 항 시험대, `src/expr.rs` 수식 해석기
  - `src/bin/quick.rs` 항 시험대 CLI, `screen.rs` 벽 반동 스핀·CSV 역보정, `spininv.rs` 비행만 역보정, `h2.rs` 속도 측정, `calib.rs` 느린 교차 검산
  - `models.txt` 장의 항(수식). 항을 바꿀 때는 여기만 고치고 다시 컴파일하지 않는다.
- `tools/` 영상 → 추적 → 역보정 (`run_shot.sh`), 가짜 영상 왕복 검산 (`e2e_synth.sh`)
- 파이썬 최상위 `*.py` 는 초기 연구 기록. 결과는 각 파일 docstring.

## 명령
- 빌드: `cd flightbench && cargo build --profile quick` (약 3 s). 정식 측정만 `--release` (lto, 느림)
- 항 시험: `./target/quick/quick` (약 2 s), 꼴 고르기 `MODELS=models_search.txt ./target/quick/quick --cv --tour`
- 엔진 비교: `./target/quick/quick --engines`
- 실제 영상: `tools/run_shot.sh 영상.mp4 240 70 3.0 [--throw | --kt K --kt-sd 0.01]`
- 회귀 점검(바꾼 뒤 반드시): `cargo test --profile quick --lib`, `tools/e2e_synth.sh` (7번 캐리 ≈ 167.9 yd 유지)

## 규칙
- 판정은 숫자로: 보정은 훈련 자료로만, 보류 자료(LPGA·USGA·FS)로 고른 경우 그 사실을 기록한다. 시험한 꼴은 `models_tried.txt`/`models_search.txt` 에 남긴다.
- 결과를 바꾸면 해당 파일 상단 주석의 '결과' 블록과 README 표를 같이 고친다.
- 시뮬레이션 수치와 실측 수치를 섞어 쓰지 않는다. 실측은 `data/real/` 아래 원본 영상과 함께 둔다.
- 스크립트가 만든 영상·CSV(`*_t.csv`, `*_track.mp4`)와 `target/` 은 커밋하지 않는다.
- `pkill -f` 로 프로세스를 죽이지 말 것(자기 셸까지 죽는다).
