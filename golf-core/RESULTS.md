# golf-core 결과: 속도·정밀도·환경 입력

측정 환경: Windows 11, AMD Ryzen 5 7500F, Rust 1.95 (`--release`, lto fat, 기본 x86-64 타깃 = FMA·AVX 없음), Node 24.15 (V8).
**주의: 측정하는 동안 이 기계는 CPU 100% 사용 중이었다**(브라우저·빌드 등 다른 작업과 SMT 형제 스레드 공유). 같은 행이 실행마다 ±20–30% 흔들린다.
아래 표의 시간은 여러 번 돌린 **최소값**이고, 괄호는 실행들의 범위다. 절대값보다 같은 세션 안의 비율을 믿을 것.
샷 4000개(저불일치 수열), 기준 = 새 스칼라 f64 τ 0.002 (얼린 옛 판본으로 만든 같은 기준과 캐리 2.2e-13 yd 차이).

## 1. 목표 대비

| 목표 | 결과 |
|---|---|
| 1. 스칼라 f64 τ 0.137 ≤ 2.0 µs, 옛 출력과 ≤ 1e-9 yd | **달성**: 1.66 µs (1.60–1.93), 옛 3.34 µs 의 **2.0배**. 옛 dump 와 최대 차 캐리 1.9e-13 yd, 상대 2.0e-15 |
| 2. wasm simd128 4레인 커널 ≤ 1.0 µs, 스트레치 0.7 | **달성**: Node 에서 τ 0.137 샷당 **589 ns** (589–715), 10만 샷 일괄 0.68–0.73 µs. 옛 wasm f64 2.90 µs 의 **4.9배**. 스트레치 0.7 µs 는 최소값만 넘고 중앙값은 0.7 µs 근처 |
| 2. 정확도: τ 0.137 캐리 ≤ 3e-4 yd, τ 0.05 ≲ 2.5e-4 yd | τ 0.05 **달성**(1.90e-4). τ 0.137 은 **기본 설정에서 못 맞춘다**(4.35e-4): f64 로 풀어도 같은 τ 에서 4.37e-4 라 f32 바닥이 아니라 RK4 절삭 오차다. 선택 사항 `min_steps = 6` 을 켜면 **2.10e-4**(비용 +3%) |
| 3. 환경 입력 (`Env`, 바람·밀도·점성) | **달성**. 시험 4종 + SI 단위 직접 적분 대조(캐리 오차 2e-8 m). 고도 효과 +4.78% (§5) |
| 4. 벤치 갱신, wasm 대 네이티브 f64 상대 ≤ 1e-12 | **달성**: 9.0e-16 (옛 9.4e-16 과 같은 수준) |
| 5. 스트레치 | 양성 2건(최소 걸음 수, 보상 합), 음성 3건(재매개 지수, 착지 부분 걸음, 묶음 인터리브) |

못한 것·확인 못한 것은 §9.

## 2. 전/후 (샷당 ns, τ 0.137 / τ 0.05)

| 방식 | 전 | 후 | 배 |
|---|---|---|---|
| 네이티브 f64 스칼라 (`fly`) | 3335 / 6362 | **1664 / 4258** | 2.0 / 1.5 |
| 네이티브 f32 스칼라 | 2772 / 5363 | 1520 / 3919 | 1.8 / 1.4 |
| 네이티브 f32 4레인 (`P4`, SSE 자동 벡터화) | – | 635 / 1357 | – |
| wasm f64 (`batch`) | 2897 / 5832 | **1630 / 4180** | 1.8 / 1.4 |
| wasm f32 (`batch`, single) | 2383 / 5078 | 1531 / 3933 | 1.6 / 1.3 |
| wasm 비SIMD `batch_simd` (= 스칼라 f32 위임) | – | 1583 / 4226 | – |
| **wasm SIMD `batch_simd`** | – | **589 / 1315** | 옛 f64 대비 4.9 / 4.4 |
| 순수 JS f64 | 16300–19400 / 38600–54900 | 3300–3500 / 6900–9950 | 5–6 |

두 τ 의 시간 차로 걸음당 비용과 샷당 고정 비용을 나누면(근사):

| | 걸음당 | 샷당 고정 (초기화·정점·착지) |
|---|---|---|
| 네이티브 f64 옛 | 89 ns | 약 1470 ns |
| 네이티브 f64 새 | 76 ns | 약 70 ns |
| wasm f64 옛 | 86 ns | 약 1090 ns |
| wasm f64 새 | 75 ns | 약 55 ns |

τ 0.137 의 큰 개선(2배)은 거의 전부 **샷당 고정 비용**에서 왔다: 옛 판본의 56회 이분법(정점·착지 각 56회, 분기 예측 실패, 매 반복 에르미트 기저 계산)이 뉴턴 4–5회로 바뀐 몫이다.
걸음당 비용은 나눗셈 3 → 1, 위치 성분 제거, 배열 복사 제거로 **15% 줄었을 뿐**이라 긴 비행(τ 0.05)의 개선은 1.4–1.5배다.
걸음당 rhs 의 지연 사슬(sqrt → div → 곱셈)은 남아 있다. 스칼라 한 샷의 사슬 길이가 한계라 더 줄이려면 샷을 나란히 돌려야 하고(4레인), 그 결과가 SIMD 행이다.
`-C target-cpu=native` 로 빌드해도 차이가 없다(FMA 를 컴파일러가 쓰지 않음, 1.61 µs).

## 3. 정확도 (캐리 yd, 4000샷, 기준 f64 τ 0.002)

| 방식 | τ | 걸음 | 캐리 최대 | 캐리 RMS | 높이 최대 | 편차 최대 | 착지각 최대(°) |
|---|---|---|---|---|---|---|---|
| f64 (옛과 새 동일) | 0.02 | 138 | 2.00e-7 | 3.11e-8 | 1.53e-6 | 6.49e-8 | 1.32e-7 |
| f64 | 0.05 | 55 | 7.07e-6 | 1.22e-6 | 4.45e-5 | 2.57e-6 | 5.01e-6 |
| f64 | 0.137 | 21 | 4.37e-4 | 7.00e-5 | 2.46e-3 | 1.51e-4 | 2.73e-4 |
| f32 옛 | 0.05 | 55 | 2.14e-4 | 3.07e-5 | 5.66e-5 | 3.47e-5 | 2.48e-5 |
| f32 새 (= 4레인 = SIMD) | 0.05 | 55 | 1.90e-4 | 2.97e-5 | 5.76e-5 | 3.76e-5 | 2.11e-5 |
| f32 새 (= 4레인 = SIMD) | 0.137 | 21 | 4.35e-4 | 7.36e-5 | 2.44e-3 | 1.72e-4 | 2.72e-4 |
| f32 + 보상 합 | 0.05 | 55 | **2.80e-5** | 7.18e-6 | 4.47e-5 | 1.30e-5 | 1.30e-5 |
| f32 + min6 | 0.137 | 21 | 2.10e-4 | 5.99e-5 | 2.44e-3 | 1.72e-4 | 1.88e-4 |
| f32 + min6 + 보상 합 | 0.1 | 28 | **7.77e-5** | 1.96e-5 | 8.19e-4 | 5.11e-5 | 6.30e-5 |
| f32 + min6 + 보상 합 | 0.137 | 21 | 1.84e-4 | 5.69e-5 | 2.46e-3 | 1.58e-4 | 1.90e-4 |

- f32 새가 옛보다 나은 것(2.14e-4 → 1.90e-4)은 착지·정점 보간을 f32 가 아니라 f64 로 하기 때문이다.
- **4레인/SIMD 는 스칼라 f32 와 샷마다 비트까지 같다**(wasm SIMD 대 스칼라 f32 최대 차 0). 같은 연산을 같은 순서로 하고 wasm f32x4 는 IEEE 정확하다.
- wasm 대 네이티브: f64 최대 상대 차 9.0e-16, 환경 f64 7.1e-15, 4레인(환경) 대 네이티브 `P4` 최대 차 0.

## 4. 바꾼 것

### 4.1 스칼라 (`src/lib.rs`)
- **뿌리**: 56회 이분법 → 3차 에르미트를 단항식 계수로 바꾼 뒤 선형 보간에서 출발하는 뉴턴(구간 보호, 걸음 < 1e-7 에서 종료). 보통 4–5회. 56회 이분법과 임의 3차식 5000개에서 1e-11 안쪽으로 일치.
- **rhs**: 나눗셈 3개·sqrt 2개 사슬 → 나눗셈 1개·sqrt 2개. 역수 하나 `inv = 1/(den1·den2·sp)` 에서 1/den1, 1/den2, 1/sp 를 모두 곱으로 얻고,
  `g = √sp · (1/sp)` 의 `√sp` 를 나눗셈과 나란히 계산(사슬 sqrt → div → sqrt 가 sqrt → div). 스핀 감쇠 `-β w sp g = -β w √sp`.
  `|u|C_D = v·sp + q·inv·(e·den1 + d·w·den2)`, `|u|C_L = c·w·q·den2·inv`.
- 위치 성분은 단계 상태에서 빼고(rhs 가 `ux·g` 를 돌려준다) 배열 복사를 없앴다 (`S`, `K` 구조체).
- 착지·정점 보간은 항상 f64 (`touchdown`, `apex_height`): f32 경로도 상태만 f32.
- **고친 결함**: 첫 걸음 안에 착지하는 아주 짧은 비행(예: 33 mph 6° 스핀 0 에 τ 0.137)은 `y > 0` 에서 출발해야 한다는 조건 때문에 옛 판본이 2백만 걸음을 돌고 0 을 냈다.
  새 판본은 `H(θ)/θ` 의 뿌리를 찾아 정상 착지시킨다 (`lands_within_first_step`). 옛 판본(`oracle`)은 얼린 채 두었으므로 이 경우에는 다르다.
- 방어: 걸음 ≤ 0·NaN 입력은 NaN 결과(옛 판본은 2백만 걸음 후 0), 위로 나가지 않는 샷은 캐리 0 으로 즉시 반환.
- 얼린 옛 판본: `src/oracle.rs` (글자 그대로). 시험과 벤치가 대조한다.

### 4.2 4레인 커널 (`src/lanes.rs`)
- `rhs`·`rk4g` 를 `Lane` 트레이트 제네릭으로 공유: 스칼라(f32/f64)와 `P4`(이식 가능 배열), `W4`(wasm simd128, `cfg(all(target_arch = "wasm32", target_feature = "simd128"))`)가 같은 식.
- 레인마다 끝나는 걸음이 다르다: 착지·정점 사건이 나면 그 묶음을 배열로 내려(`v128` 32–40개) 레인별 스칼라 f64 로 처리하고 다음 샷을 같은 레인에 채운다.
  빈 레인은 무해한 샷으로 채워 계속 돌린다. 걸음 h 도 레인마다 벡터다.
- 사건 감지는 비교 4개 + 비트마스크 1번(`i32x4_bitmask`).
- **묶음 수 G**: 한 고리에서 번갈아 돌리는 묶음 수. 지연 사슬을 숨기려 G = 2, 3, 4 를 시도했으나 wasm (V8)에서 G = 1 이 가장 빠르다(§7).
- SIMD 가 없는 wasm 에서는 `batch_simd*` 가 같은 연산의 스칼라 f32 로 위임한다(`P4` 배열 판본은 벡터가 없는 곳에서 스칼라보다 느리다: 측정).

### 4.3 빌드·JS
- `build-wasm.sh`: `pkg/` (기본, SIMD 없음) 와 `pkg-simd/` (`RUSTFLAGS="-C target-feature=+simd128"`) 를 모두 만들고 둘 다 `"type": "module"` 후처리.
  `.gitignore` 에 `pkg-simd/` 추가.
- `js/load.mjs`: `loadGolfCore({simd: 'auto'|true|false})`. `WebAssembly.validate` 로 29바이트 프로브 검사 → 지원이면 `pkg-simd/`, 아니면(또는 불러오기에 실패하면) `pkg/`. Node 와 브라우저 경로 모두 있음(브라우저는 미시험).
- wasm 크기: `pkg/` 62 KB (옛 30 KB), `pkg-simd/` 97 KB.
- 기존 내보내기(`fly_f64`, `fly_f32`, `batch`, `default_step`, `Flight` 와 getter, `path()`)는 이름·동작 그대로다(`.d.ts` 비교: 추가만 있음).
  `batch(.., single = true)` 의 결과는 옛보다 약간 낫다(착지 보간이 f64 라서).

## 5. 환경 입력 (`Env`)

### 5.1 물리
`Env { wind: [x 앞, y 위, z 오른쪽] m/s, rho: kg/m³, mu: Pa·s }`, `Env::default()` = ρ 1.2, μ 1.81e-5, 무풍(= 지금까지의 동작).
`Env::from_conditions(temp_c, altitude_m, wind_forward_ms, wind_right_ms)`: ISA 기압 `p = 101325 (1 − 0.0065 h/288.15)^5.25588`, 이상기체 `ρ = p/(287.058 T)`(습도 무시), 서덜랜드 `μ = 1.458e-6 T^1.5/(T + 110.4)`.
바람은 공기가 움직이는 속도다: `wind_forward_ms` > 0 이면 뒷바람, `wind_right_ms` > 0 이면 공을 오른쪽으로 미는 바람(왼쪽에서 분다).

- 무차원 식은 ρ 에 길이 단위로만 들어간다: `L = 2m/(ρA) = L_M · (1.2/ρ)`, `U = √(gL)`, 시간 단위 `L/U`. 기본 환경에서 인자가 정확히 1.0 이라 `L_M`, `U_MS`, `T_S` 가 비트까지 그대로다.
- 항력 위기는 Re 로 정의돼 있어 `Coef::SHARED_RE.u0` 를 Re 비 `(Re/|u|)_환경 / (Re/|u|)_기본 = √(ρ/ρ0) · (μ0/μ)` 로 나눈다. `Coef` 값은 그대로 읽는다(단일 출처).
- 바람 w⃗/U 는 공기에 대한 상대속도 `u_rel = u − w⃗/U` 로 항력, 양력 방향 `ω̂×u_rel`, `S = w/|u_rel|`, Re, 스핀 감쇠, 재매개 `g = |u_rel|^{-1/2}` 에 들어가고, 중력과 운동학은 지면 기준 속도다. 지면 y = 0 그대로. `Landing` 의 길이·편차·시간·착지각은 지면 기준.
- 바람이 0 이면 바람 없는 코드 길(`WIND = false` 상수 제네릭)로 가서 비용이 없다.
- wasm: `fly_env(mph, vla, rpm, axis, h, temp_c, altitude_m, wind_forward_ms, wind_right_ms) -> Flight`, `batch_env(shots, h, single, temp_c, altitude_m, wf, wr)`, `batch_simd_env(shots, h, temp_c, altitude_m, wf, wr)`.

### 5.2 시험
- (a) `default_env_is_bitwise_legacy_api`: `fly(..)` 와 `fly_with(.., &Env::default())` 가 48샷 × 2걸음에서 `carry/lateral/apex/land/time/vel` 전부 **비트까지** 같고 `units()` 가 `L_M`, `U_MS`, `T_S`, `re_scale = 1.0` 과 비트 동일.
  **얼린 옛 판본과의 비트 일치는 불가능하다**: rhs 의 연산을 재배열하고 이분법을 뉴턴으로 바꿨으니 마지막 자리가 달라진다. 대신 `new_engine_matches_frozen_oracle`: 같은 걸음 수에서 캐리·편차·높이·각·시간 차이 < 1e-11 (실측 2.2e-13 yd), 그리고 이 변경 이전에 저장한 옛 f64 τ 0.137 dump(4000샷×4양)와의 최대 차 2.2e-13 yd, 상대 1.0e-15.
- (b) `galilean_invariance_horizontal_wind`: 5가지 (샷, 기온·고도, 바람, τ 0.01–0.137) 에서 `캐리_풍 = 캐리_무풍(u0 − w) + w_앞·T`, `편차_풍 = 편차_무풍 + w_우·T`, 시간·정점·수직 착지속도 같음, 이동 좌표계 착지각 같음을 상대 1e-9 로 확인(통과).
- (c) `environment_monotonic`: 고도↑, 기온↑ → 더 멀리(빠른 공), 맞바람 < 무풍 < 뒷바람, 맞바람 손실 > 뒷바람 이득, 왼쪽에서 오는 횡풍 → 오른쪽(+) 편차(좌우 대칭).
  **관찰**: 느린 공(100 mph 로브)은 기온 20 → 40 °C 에서 캐리가 오히려 0.3 yd 줄어든다. 따뜻하면 공기가 얇아지지만 점성이 올라 Re 가 내려가 항력 위기 속도가 올라가기 때문이다(Re 연동의 결과). 빠른 공에서는 밀도 효과가 이긴다.
- (d) `altitude_magnitude`: 드라이버 167 mph / 10.9° / 2686 rpm, 해수면 15 °C **269.11 yd** → 1600 m 20 °C **281.99 yd**, **+12.88 yd = +4.78%** (요청 범위 +4…+9% 안, 맞추지 않음).
- 독립 검산 `si_reference_integrator_matches_env`: m·s·kg 단위로 운동방정식을 직접(Re 로 정의한 C_D, `L = 2m/(ρA)`, 바람 포함, 수직 바람·임의 ρ/μ 포함) RK4 적분(dt 2e-4 s)해 5가지 환경과 비교: 캐리·편차 차이 **≤ 2.5e-8 m**(선형 보간 한계). 같은 시험이 `L_M`, `U_MS`, `RE_PER_U` 상수가 m, A, ρ0, g 에서 나온 값과 1e-9 안쪽으로 맞는지도 본다.

### 5.3 환경별 캐리 (f64 τ 0.05, 참고)
드라이버 / 7번(120 mph 16.3° 7097 rpm):

| 조건 | 드라이버 yd | 7번 yd |
|---|---|---|
| 해수면 15 °C 무풍 | 269.11 | 168.44 |
| 고도 500 m | 273.95 (+1.8%) | 171.45 (+1.8%) |
| 고도 1000 m | 278.15 (+3.4%) | 174.06 (+3.3%) |
| 고도 1600 m | 282.21 (+4.9%) | 176.61 (+4.9%) |
| 고도 2500 m | 286.03 (+6.3%) | 179.15 (+6.4%) |
| −10 °C / 0 / 20 / 40 °C (해수면) | 263.77 / 266.29 / 269.81 / 271.39 (−2.0 / −1.1 / +0.3 / +0.9%) | 165.92 / 167.16 / 168.73 / 169.20 |
| 맞바람 5 m/s / 10 m/s | 248.41 (−7.7%) / 223.07 (−17.1%) | 148.86 (−11.6%) / 122.71 (−27.2%) |
| 뒷바람 5 m/s / 10 m/s | 284.95 (+5.9%) / 295.76 (+9.9%) | 181.34 (+7.7%) / 187.81 (+11.5%) |
| 횡풍 5 m/s (왼쪽에서) | 편차 +17.3 m, 캐리 268.94 | 편차 +14.8 m, 캐리 168.24 |

위기 속도를 Re 에 묶지 않고 무차원 속도에 고정했다면 고도 효과는 드라이버 +7.7%, 7번 +8.6%, 웨지 +7.1%로 훨씬 컸다(`exp_altitude_without_re_coupling`). Re 연동이 고도 효과를 약 3%p 깎는다.
온도 효과(0 → 20 °C 에 드라이버 +1.3%)는 흔한 어림(10 °C 당 약 1%)보다 약하다: 모형에 볼 압축 등 온도 효과가 없고 Re 연동이 일부를 상쇄한다.

## 6. 선택 사항 `Opts` (기본값 = 지금까지의 동작)

### 6.1 `min_steps` (최소 걸음 수) — 양성
τ 0.137 에서 최대 오차(4.37e-4)를 만드는 것은 빠른 공이 아니라 **3–5걸음에 끝나는 낮고 느린 샷**(60 mph 5°…)이다(`exp_error_vs_shot`: 가장 나쁜 10샷 중 8개가 60 mph 대).
진공 비행 시간 × √속도로 τ 길이를 어림해 걸음을 `min(h, 0.8·τ_추정/min_steps)` 로 줄이면:

| | 평균 걸음 | 캐리 최대(f64) |
|---|---|---|
| τ 0.137 | 20.55 | 4.37e-4 |
| τ 0.137 + min6 | 21.26 (+3.5%) | **1.77e-4** |
| τ 0.12 (규칙 없음) | 23.39 | 2.55e-4 |
| τ 0.15 + min6 | 19.68 | 2.75e-4 |

같은 오차(2.6e-4)에서 걸음이 16% 적고(23.4 → 19.7), 같은 걸음 수에서 오차가 2.5배 작다. SIMD 에서는 τ 0.137 + min6 이 캐리 최대 2.10e-4(3e-4 목표 달성), 755 ns.
wasm: `batch_opts`, `batch_env_opts`, `batch_simd_opts`, `batch_simd_env_opts` 의 `min_steps` 인수(0 = 끔). **기본값은 꺼져 있다**: 켜면 짧은 샷의 값이 4e-4 yd 수준에서 바뀌므로 옛 출력과 ≤ 1e-9 요구를 지키려고 선택 사항으로 두었다. 권장 값 6.

### 6.2 `compensated` (카한 보상 합) — 양성
f32 바닥의 대부분은 걸음마다 상태에 더하는 반올림이 쌓인 것이다(위치만 보상하면 1.9 → 1.1e-4, 전부 보상하면 **2.8e-5**). 비용은 측정 불가능한 수준(SIMD 1344 대 1315 ns, 잡음 안).
τ 0.05 f32 캐리 최대 1.90e-4 → 2.80e-5. 4레인 커널도 같은 연산이라 스칼라 f32 와 비트까지 같다. 조합 `min6 + 보상 합` 은 τ 0.1 에서 캐리 최대 7.8e-5 를 28걸음(SIMD 약 850 ns)으로 낸다.
f64 에서는 효과가 없다.

### 6.3 음성 결과
- **재매개 지수 p** (dτ = |u|^p dt; `exp_reparam_exponent`, 평균 걸음이 같을 때 f64 캐리 최대 오차): p = 0.5 가 최선이다. 약 28걸음에서 p = 0.25: 1.9e-4, **0.5: 7.5e-5**, 0.75: 2.6e-4, 1.0: 7.8e-4. 약 14걸음에서 0.25: 3.2e-3, **0.5: 1.2e-3**, 0.75: 5.4e-3.
- **착지를 보간 대신 부분 걸음 RK4 로** (`exp_landing_substep`): 4.37e-4 → 4.46e-4. 오차는 보간이 아니라 RK4 전역 절삭 오차다. y 잔차로 한 번 보정해도 3.16e-4.
- **묶음 인터리브 G > 1**: §7.

## 7. SIMD 커널 속도 조사

| 실험 (wasm, τ 0.137, 샷당 ns) | 결과 |
|---|---|
| G = 1 / 2 / 3 / 4 | **588** / 725 / 818 / 772 (G = 2–4 가 느리다) |
| RK4 반복만 (사건 처리 없음) | 4레인 한 반복 약 65 ns (16 ns/샷·걸음) |
| 같은 반복에서 sqrt/div 를 모두 없애도 | 65 ns 그대로 → sqrt/div 처리량이 병목이 아니다 |
| G = 2 반복 | 126 ns = 정확히 2배 → 지연이 아니라 명령 수(처리량) 병목, 인터리브 이득 없음 |
| `--no-liftoff` / `--liftoff-only` / wasm-opt -O3 / wasm-opt 없음 | 모두 같은 범위(잡음 안) |
| 사건 처리를 `#[inline(never)]` 함수로 분리 | 변화 없음 (오히려 약간 느림) → 되돌림 |
| 뿌리 뉴턴을 1회로 줄여 보기 | 시간 변화 없음 → 사건 처리는 병목이 아니다 |

한 반복(4샷, rhs 4회 + 갱신)의 wasm 명령은 (조사 시점 빌드 기준) f32x4 연산 256개, sqrt 8, div 4, 상수 85. 이 기계가 SMT 형제와 코어를 나눠 쓰는 상태에서는 명령 처리량이 한계로 보인다. 연산 수를 더 줄이는 것 외에는 길이 없어 보였고, 남은 줄임 후보는 3% 안팎이라 두었다.
`f32x4.relaxed_madd` 는 기기마다 결과가 달라져 쓰지 않았다.

## 8. 시험 (`cargo test --lib`, 28개 통과 + 실험 8개 `--ignored`)

기존 5: `carry_is_in_golf_range`, `step_converges`, `f32_tracks_f64`, `lateral_symmetry`, `path_ends_on_ground`.
새로: `path_ends_on_ground_with_wind`, `default_env_is_bitwise_legacy_api`, `new_engine_matches_frozen_oracle`, `hroot_matches_bisection`, `from_conditions_values`, `units_scaling`,
`galilean_invariance_horizontal_wind`, `environment_monotonic`, `altitude_magnitude`, `viscosity_moves_crisis`, `si_reference_integrator_matches_env`, `degenerate_inputs`, `lands_within_first_step`,
`lanes_match_scalar_f32`, `lanes_track_f64`, `lanes_with_environment`, `lanes_reject_bad_step`, `lanes_wild_shots`, `lanes_group_count_independent`,
`min_steps_option`, `lanes_min_steps_match_scalar`, `compensated_lowers_f32_floor`, `lanes_compensated_match_scalar`.
실험(`cargo test --release --lib exp_ -- --ignored --nocapture`): `exp_landing_substep`, `exp_reparam_exponent`, `exp_error_vs_shot`, `exp_min_steps_rule`, `exp_min_steps_with_larger_h`, `exp_f32_compensated`, `exp_altitude_without_re_coupling`, `exp_env_table`.
디버그(`cargo test --lib`)와 릴리스 모두 통과.

wasm 쪽 대조는 `bench/bench.mjs` 가 한다(실패하면 종료 코드 1): wasm f64 대 네이티브 f64, 환경 f64 대 네이티브, 4레인 대 스칼라 f32 (비트 동일), SIMD 대 네이티브 `P4`, `fly_env` 대 `batch_env`, 고도·횡풍 방향.

## 9. 못한 것·주의

- **시간 측정이 시끄럽다**(§머리말). 같은 코드가 실행마다 ±20–30%. 스트레치 0.7 µs 는 최소값(589 ns)에서만 넘었다. 조용한 기계에서 다시 재야 한다.
- **폰·안경 CPU 에서는 재지 못했다.** PRD 의 "폰 CPU 에서 샷당 ≤ 1 µs" 는 이 데스크톱의 0.6–0.7 µs 로 볼 때 여유가 크지 않다(ARM NEON 에서는 f32x4 sqrt/div 가 느릴 수 있다).
- **브라우저 SIMD 는 미시험**: Node (V8) 에서만 돌렸다. `js/load.mjs` 의 브라우저 분기, Meta Ray-Ban Display 브라우저의 wasm SIMD 지원 여부는 모른다. 기본은 항상 `pkg/`.
- τ 0.137 캐리 ≤ 3e-4 yd 는 기본 설정에서 못 맞춘다(§1). `min_steps` 를 켜야 한다.
- "옛 판본과 비트 일치"는 불가(§5.2 a). 같은 길(`Env::default()` 대 기존 API)은 비트 일치.
- `batch(.., single = true)` 의 값은 옛 f32 경로와 약간 다르다(착지 보간 f64). 옛 판본의 첫 걸음 착지 결함(§4.1)도 달라진 점이다.
- 샷 한 개씩 `fly_f64` 를 부르는 경로(경로 배열 포함)는 Node 에서 샷당 약 5.3–5.4 µs (옛 13.2 µs). 대부분 JS↔wasm 객체 생성·`path()` 복사 비용이라 더 줄이지 않았다.
- f64 SIMD(f64x2) 커널은 만들지 않았다. SIMD 가 없는 환경의 f64 일괄 처리는 스칼라(1.6–1.9 µs)다.
- 순수 JS 이식(`bench/bench.mjs`)은 새 알고리즘으로 바꿨지만 첫 걸음 착지 처리는 넣지 않았다(벤치용).

## 10. 사용 요약

```sh
cd golf-core
cargo test --lib                       # 단위 시험
cargo run --release --bin bench        # 네이티브 표 (옛/새, f32, 4레인, 10만 샷)
cargo run --release --bin bench -- dump target/d.csv                # f64 τ 0.137 (wasm 대조용)
cargo run --release --bin bench -- dump target/e.csv env            # 환경 f64
cargo run --release --bin bench -- dump target/l.csv envlanes       # 환경 4레인 f32
sh build-wasm.sh                       # pkg/ (기본) + pkg-simd/ (SIMD)
node bench/bench.mjs target/d.csv target/e.csv target/l.csv         # wasm·SIMD·JS 표와 대조
```

```js
import { loadGolfCore } from './js/load.mjs';
const core = await loadGolfCore();                  // SIMD 지원이면 pkg-simd/, 아니면 pkg/
core.batch_simd(shots, 0.137);                      // [mph, 발사각, rpm, 축]×n → [캐리 m, 편차 m, 최고 m, 착지각]×n
core.batch_simd_env(shots, 0.137, 28, 1500, -4, 3); // 기온 °C, 고도 m, 앞뒤 바람(+뒷바람), 좌우 바람(+오른쪽)
core.batch_simd_opts(shots, 0.137, 6, false);       // min_steps = 6
core.fly_env(167, 10.9, 2686, 0, 0.05, 15, 0, -5, 0).carry_yd; // 맞바람 5 m/s
```

바꾼 파일: `src/lib.rs`, `src/lanes.rs`(새), `src/oracle.rs`(새), `src/tests.rs`(새), `src/bin/bench.rs`, `bench/bench.mjs`, `js/load.mjs`(새), `build-wasm.sh`, `.gitignore`, `RESULTS.md`(새). `Cargo.toml` 은 그대로.
