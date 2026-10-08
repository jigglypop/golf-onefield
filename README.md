# golf-onefield

골프공 비행을 **장(field) 하나**로 계산하는 실험 엔진. Clarus Equation(CE)의 OBJ-04 구조
"세 힘 = 공통장의 투영, 중력 = 배경"을 골프공에 옮겼다.

공 표면 응력장 $\mathbf t(\mathbf x)$ 하나에서 항력·양력·스핀 감쇠 토크가 모두 투영으로 나온다.

$$
\mathbf F=\oint \mathbf t\,dA,\quad D=-\mathbf F\cdot\hat{\mathbf v},\quad L=\mathbf F\cdot\hat{\mathbf e}_L,\quad T_\omega=-\hat{\boldsymbol\omega}\cdot\oint \mathbf r\times\mathbf t\,dA
$$

## 모형

2차원(순수 백스핀)에서 장을 복소수 $\Gamma=C_D+iC_L$ 하나로 둔다. 무차원화
($L=2m/\rho A$, $U=\sqrt{gL}$, 시간 $U/g$)하면 공기밀도·질량·크기·중력이 모두 빠진다.

$$
\frac{du}{dt}=-|u|\,\overline{\Gamma(S)}\,u-i,\qquad \frac{dw}{dt}=-0.06\,w\,|u|,\qquad S=\frac{w}{|u|}
$$

힉스꼴 장 (각도 잠금):

$$
\Gamma(S)=(v_{\text{ev}}+h\,S)\,e^{i\pi/4},\qquad v_{\text{ev}}=0.388,\ h=0.271
\quad\Longrightarrow\quad |u|\,\overline{\Gamma}=(v_{\text{ev}}|u|+h\,w)\,e^{-i\pi/4}
$$

즉 $C_D=C_L$ 이고, 장 읽기에 나눗셈·초월함수가 없다.

## 결과 요약 (2026-10-08)

자료: TrackMan 투어 평균 아이언 행 (GolfMagic 게재본). 훈련 PGA 3i–9i 7행, 보류 LPGA 4i–9i 6행. 캐리 RMSE(yd).

| 모형 | 매개변수 | 훈련 | 보류 |
|---|---|---|---|
| B 분리 법칙 $C_D=d_0+d_1S,\ C_L=l_0S^p$ | 4 | 0.71 | 3.16 |
| F3 장 하나, 각도 π/4 | 실효 2 | 0.72 | 3.67 |
| Z 장 하나, $v_{\text{ev}}=\pi/8,\ h/v_{\text{ev}}=1/\sqrt2$ | 0 | 1.48 | 3.10 |

속도 (Xeon 2.8 GHz, 오차 0.01 yd 이내):

| 방식 | 샷당 |
|---|---|
| 파이썬 solve_ivp | 약 15,000 µs |
| 러스트 직접 적분, 스칼라 | 3.1 µs |
| 러스트 직접 적분, 8샷 묶음 | 0.68 µs |
| 펼친 장 3선형 읽기 (12.5 MB) | 약 20 ns (메모리 병목) |
| 펼친 장 3차 읽기 (0.67 MB) | 약 42 ns (계산 병목, 코어 수에 비례) |

### 적은 샷 보정 (`fewshot.py`)

PGA 7행에서 k개만 골라 보정한 뒤, 보정에 안 쓴 PGA 행으로 채점 (중앙값/최악, yd).

| k | B4 (4매개) | F2 (2매개) | Z1 (1매개: $h/v_{\text{ev}}=1/\sqrt2$ 고정) |
|---|---|---|---|
| 1 | – | – | 0.85 / 1.90 |
| 2 | – | 1.74 / 6.12 | 0.89 / 1.23 |
| 4 | 2.13 / 11.3 | 1.06 / 2.26 | 0.89 / 1.24 |
| 6 | 1.18 / 2.72 | 0.67 / 1.81 | 0.41 / 1.71 |

1매개 장 하나는 1–2샷으로 4매개 분리 모형의 6샷 보정과 대등하고, 최악 사례가 훨씬 안정적이다
(LPGA 최악 B4 76.8 yd, Z1 4.4 yd). 단 Z1 의 구조는 PGA 전체를 본 뒤 정했다.

정칙화 판본(`fewshot_reg.py`): B4·F2 에 일반적인 크기의 사전분포를 주면 B4 의 파국은 사라진다
(LPGA 최악 6.4 yd 이하). 그래도 k ≤ 4 에서는 Z1 의 중앙값·최악값이 가장 작고, k = 6 에서 셋이 비슷해진다.

| k | B4r | F2r | Z1 |
|---|---|---|---|
| 1 | 4.97 / 8.00 | 1.83 / 2.89 | 0.85 / 1.90 |
| 2 | 1.42 / 6.87 | 1.66 / 4.31 | 0.89 / 1.23 |
| 4 | 1.19 / 5.77 | 1.07 / 2.26 | 0.89 / 1.24 |
| 6 | 0.41 / 2.31 | 0.64 / 1.81 | 0.41 / 1.71 |

### 3차원 장 (`field3d.py`)

스핀축 기울기 $a$ 를 더했다. $\hat\omega=(0,\sin a,\cos a)$, $k=(v_{\text{ev}}|u|+h\,w)/\sqrt2$ 일 때

$$
\frac{d\mathbf u}{dt}=-k\,\mathbf u+k\,(\hat{\boldsymbol\omega}\times\mathbf u)-\hat{\mathbf y}
$$

발사 방위각은 정확한 회전 대칭이고 $a\to-a$ 는 좌우 반전이라, 펼칠 축은 $(v_0',\theta,S_0,a\ge0)$ 4개뿐이다.
$a=0$ 에서 2차원과 1.6e-13 yd 일치, 두 대칭 모두 1e-12 yd 이하로 성립.
4차원 장 809,472점(6.5 MB), 4선형 읽기 최대 오차 0.19 yd.

러스트 판본(`flightbench/src/bin/field4.rs`, 파이썬과 7.7e-10 yd 일치):

| 방식 | 크기 | 최대 오차 (x / z) | 샷당 |
|---|---|---|---|
| 직접 적분 f64×8 | – | 0.0004 yd | 1,025 ns |
| 4차원 장, 4축 3차 | 7.3 MB | 0.012 / 0.002 yd | 333 ns (코어 확장 없음) |
| **각도 모드 전개 K=4** | **1.6 MB** | **0.014 / 0.005 yd** | **87 ns (2코어 1.9e7 샷/초)** |

기울기 축을 펼치는 대신 대칭을 꼴에 넣은 모드로 전개했다:
$x=c_0+c_2s^2+c_4s^4+c_6s^6$, $z=c_1s+c_3s^3+c_5s^5+c_7s^7$, $s=\sin a$.
계수 8개를 담은 3차원 장 하나로 4차원 장과 같은 정확도를 4.5배 작은 크기, 3.8배 빠른 속도로 낸다.
차원이 늘수록 펼치기의 이점은 줄어든다(3차원: 직접 적분 대비 16–34배, 사이드 스핀 포함: 약 12배).

### 독립 자료 보류 검증: USGA R22-01 (`usga_holdout.py`)

USGA "Launch conditions of male and female golfers of different skill levels"(성별 × 드라이버·6번 × 실력 5단계, 20행)로,
투어 자료로 동결한 모형을 다시 맞추지 않고 채점했다. **장 하나가 졌다.**

| 캐리 RMSE (yd) | B4 (4매개) | F3 (2매개) | Z0 (0매개) |
|---|---|---|---|
| 프로 4행 | 12.3 | 12.1 | 12.6 |
| 전체 20행 | **11.6** | 14.7 | 14.4 |
| 드라이버 10행 | **14.0** | 17.3 | 17.1 |

남자 프로 드라이버($S=0.067$, 훈련 범위 밖)에서 B4 -0.6, F3 -4.7 yd. 각도 π/4 잠금은 저스핀에서도
$C_L=C_D$ 를 강제하는데, 스핀이 0으로 가면 양력은 0이어야 한다. 잠금은 아이언 범위 밖에서 깨진다.

### CE 최신 힉스 원리 반영 (`usga_v2.py`)

CE `ce-dimension-qgt-bridge` 브랜치의 2026-10-02 힉스 문서에서 두 원리를 가져왔다.

1. **$q[\langle\Psi\rangle]\neq\langle q[\Psi]\rangle$**: USGA 행은 샷 평균이므로, 보고된 입력 표준편차로 캐리의 분포 평균을 낸다.
2. **반경과 영향 결합** $R=\sqrt{2\Phi^\dagger\Phi}$, $g=\partial m/\partial R|_v$: 장을 배경(항력)과 스핀 들뜸(양력)으로 나누고 양력을 반경 노름으로 포화시킨 H2.

$$
C_D=v+d\,S,\qquad C_L=\frac{c\,S}{\sqrt{1+\lambda^2S^2}}
$$

$S\to0$ 에서 양력이 0 이 되고(각도 잠금의 반례 해소), 초월함수 없이 sqrt 하나로 계산된다. 매개변수는 PGA 로만 맞췄다.

| 캐리 RMSE (yd), USGA 20행 | B4 | F3 | Z0 | H2 |
|---|---|---|---|---|
| 평균 입력 하나 | 11.60 | 14.68 | 14.42 | 11.85 |
| **입력 분포 평균** | **8.96** | 12.09 | 11.92 | 9.93 |
| 분포 평균, 아마추어 16행 | 7.27 | 11.84 | 11.44 | **5.85** |
| 분포 평균, 평균 부호 오차 | +3.97 | +7.64 | +6.97 | **+1.65** |

원리 1은 모든 모형의 오차를 16–23% 줄였다. H2 는 아마추어·아이언에서 가장 좋고 치우침이 가장 작지만,
남자 프로 드라이버에서 분포 평균 -22 yd 로 무너진다(보고된 스핀 산포가 커서 저스핀 표본이 많다). 각도 잠금(F3, Z0)은 기본 장에서 내린다.

### 지면 접촉 (`ground.py`) — 검증 실패

Penner 강체 바운스(브리스톨 적합 계수) + 미끄럼 + 구름 저항 1매개. USGA 런(총거리 − 캐리)에서 모든 조합이
"런 평균 하나로 찍기"(보류 RMSE 6.9 yd)보다 나빴다(14.8–69 yd). 맞춘 구름 저항도 비물리적이다.
티 구역 잔디 계수와 투어 페어웨이의 차이, 가파른 착지각, 총거리 산출 방법 미상이 원인 후보다.

### WebGPU 측정 페이지 (`web/`)

같은 모드 장(1.6 MB)과 직접 적분을 WGSL 컴퓨트 셰이더와 JS로 구현해, 브라우저에서 GPU·CPU 속도와 정확도를 잰다.
셰이더는 naga 로 검증했고, JS 판본의 정확도는 러스트와 같다(장 읽기 최대 0.013 / 0.004 yd, 직접 적분 0.0004 yd).
`cargo run --release --bin field4 -- --dump` 후 `python -B web/build_bench.py` 로 `web/onefield_bench.html` 을 만든다.
게시본: https://claude.ai/artifact/4kAmDTbhGT4NtruM7ocdiB (비공개)

## 허점과 반례

- π/4 와 Z 의 상수는 훈련 자료를 본 뒤 고른 [경험식]/[가설]이다. 단순 상수 42쌍 점검에서 Z 는 특별하지 않았고, $(v_{\text{ev}}, h)$ 에 1차원 골짜기가 있어 이 자료는 조합 하나만 정한다.
- $S\to0$ 극한의 항력 $C_D(0)=0.388$ 은 자료가 정하지 않은 값이며 문헌값보다 높다.
- 스핀 감쇠($C_M=0.012\,S$)는 고정 가정이고 검증하지 않았다.
- 반례: F1(π/2 포화), F2(오일러꼴), log 저장, 진공해 배경 비율 저장, 4차원 장의 기울기 축 펼치기(캐시 초과),
  **각도 잠금의 저스핀 외삽(USGA 드라이버)**, **Penner + 브리스톨 계수 지면 모형(USGA 런)**.
- 레이놀즈수 의존성이 없다. 사이드 스핀은 모형만 있고 실측 대조가 없다. 지면 모형은 검증되지 않았다.

## 파일

| 파일 | 내용 |
|---|---|
| `onefield_carry.py` | 첫 검산: B·F1–F4 모형 적합 (solve_ivp) |
| `higgs_engine.py` | 무차원 배치 RK4 엔진, 힉스꼴 장, 펼친 장 |
| `run_higgs.py` | 엔진 검산: 일치·재피팅·ψ 프로파일·보편성 |
| `zero_param.py` | 매개변수 0개 가설과 눈길 효과 점검 |
| `compress_field.py` | 장 압축 실험 (반례) |
| `fewshot.py` | 적은 샷 보정 실험 |
| `field3d.py` | 3차원 장 (사이드 스핀), 대칭 축소, 4차원 펼치기 |
| `fewshot_reg.py` | 정칙화한 적은 샷 비교 |
| `refs3d.csv` | 러스트 3차원 일치 검사용 기준값 |
| `flightbench/src/bin/field4.rs` | 러스트 3차원 장: 직접 적분, 4차원 장, 각도 모드 전개, 웹용 내보내기 |
| `usga_holdout.py` | USGA 독립 자료 보류 검증 |
| `ground.py` | 지면 접촉 모형과 USGA 런 검증 (반례) |
| `usga_v2.py` | CE 최신 힉스 원리 반영: 분포 평균 채점과 힉스 이중항 장 H2 |
| `web/bench_template.html`, `web/build_bench.py` | WebGPU 측정 페이지 원본과 빌드 스크립트 |
| `refs.csv` | 러스트 일치 검사용 파이썬 기준값 |
| `flightbench/` | 러스트 엔진과 병목 측정 |

각 스크립트의 결과는 파일 상단 docstring/주석에 기록되어 있다.

## 실행

```sh
pip install numpy scipy
python -B onefield_carry.py
python -B run_higgs.py          # unfolded_field.npz 생성
python -B zero_param.py
python -B compress_field.py
python -B fewshot.py            # 약 20분
python -B field3d.py

python -B fewshot_reg.py        # 약 20분

cd flightbench && cargo run --release
cargo run --release --bin field4
```
