# golf-onefield 작업 규약

제품 요구는 `docs/PRD.md`, 연구 기록·반례는 `README.md`. 답변과 주석은 한국어, 수식은 LaTeX.

## 구조
- `flightbench/` (러스트 2024. 빠른 엔진은 x86_64+AVX-512 판, aarch64 NEON 판(애플 실리콘), 그 밖은 배열 판. `.cargo/config.toml` 의 target-cpu=native)
  - `src/engine.rs` 엔진(H2·Shared·SharedRe<N>, 기본 장 `default_field()` = 공유위기12, f64 기준 `run`, f32×16 빠른 `stream`), `src/harness.rs` 항 시험대(`fit`·`fit_mixed`·`kfold_mixed`), `src/expr.rs` 수식 해석기
  - `src/bin/quick.rs` 항 시험대 CLI, `screen.rs` 벽 반동 스핀·CSV 역보정, `spininv.rs` 비행만 역보정, `h2.rs` 속도 측정, `calib.rs` 느린 교차 검산, `webrefs.rs` 웹 엔진 참값(장을 바꾸면 다시 뽑기)
  - `models.txt` 장의 항(수식). 항을 바꿀 때는 여기만 고치고 다시 컴파일하지 않는다.
- `tools/` 영상 → 추적 → 역보정 (`run_shot.sh`), 가짜 영상 왕복 검산 (`e2e_synth.sh`)
- 파이썬 최상위 `*.py` 는 초기 연구 기록. 결과는 각 파일 docstring.

## 명령
- 빌드: `cd flightbench && cargo build --profile quick` (약 3 s). 정식 측정만 `--release` (lto, 느림)
  - 이 맥(애플 실리콘): PATH 에서 Homebrew cargo 1.84 가 먼저 잡혀 edition 2024 를 못 읽는다. `export PATH=$HOME/.cargo/bin:$PATH` (rustup 1.95) 후 빌드
- 항 시험: `./target/quick/quick` (약 1 s), 꼴 고르기 `MODELS=models_search.txt ./target/quick/quick --cv --tour`, 행별 잔차(USGA 포함) `--rows`
- 혼합 맞춤: `./target/quick/quick --joint [--tour] [--cv]` (투어 + FS 4분의 3 훈련, FS 4분의 1·USGA 판정, 고르기는 5겹 CV)
- 엔진 비교: `./target/quick/quick --engines`
- 실제 영상: `tools/run_shot.sh 영상.mp4 240 70 3.0 [--throw | --kt K --kt-sd 0.01]`
- 회귀 점검(바꾼 뒤 반드시): `cargo test --profile quick --lib`, `tools/e2e_synth.sh` (7번 캐리 ≈ 168.4 yd, 참 168.5)
  - 영상 도구는 opencv 가 필요하다(시스템 python3 에 없음): 가상환경을 만들어 PATH 앞에 둔다
- 3D 골프 게임: `cd web3d && npm run dev` (r3f + gaesup-world, 메뉴 → 스트로크 9홀 1–4명·니어핀·롱드라이브, `?mode=range` 연습장). 비행 엔진은 `web3d/src/engine/`(러스트 장과 같은 식, `npm test` 로 참값 대조),
  게임은 `web3d/src/game/`(course·clubs·ground·round·sound, scene/, ui/). 지면 물리는 검증 안 된 게임용 모형이다. 바꾼 뒤 `npm test` 와 `npm run build && npm run smoke`

## 규칙
- 판정은 숫자로: 보정은 훈련 자료로만, 보류 자료(LPGA·USGA·FS)로 고른 경우 그 사실을 기록한다. 시험한 꼴은 `models_tried.txt`/`models_search.txt` 에 남긴다.
- 결과를 바꾸면 해당 파일 상단 주석의 '결과' 블록과 README 표를 같이 고친다.
- 시뮬레이션 수치와 실측 수치를 섞어 쓰지 않는다. 실측은 `data/real/` 아래 원본 영상과 함께 둔다.
- 스크립트가 만든 영상·CSV(`*_t.csv`, `*_track.mp4`)와 `target/` 은 커밋하지 않는다.
- `pkill -f` 로 프로세스를 죽이지 말 것(자기 셸까지 죽는다).
