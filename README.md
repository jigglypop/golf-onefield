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
(LPGA 최악 B4 76.8 yd, Z1 4.4 yd). 단 Z1 의 구조는 PGA 전체를 본 뒤 정했고, B4 는 정칙화 없이 맞췄다.

### 3차원 장 (`field3d.py`)

스핀축 기울기 $a$ 를 더했다. $\hat\omega=(0,\sin a,\cos a)$, $k=(v_{\text{ev}}|u|+h\,w)/\sqrt2$ 일 때

$$
\frac{d\mathbf u}{dt}=-k\,\mathbf u+k\,(\hat{\boldsymbol\omega}\times\mathbf u)-\hat{\mathbf y}
$$

발사 방위각은 정확한 회전 대칭이고 $a\to-a$ 는 좌우 반전이라, 펼칠 축은 $(v_0',\theta,S_0,a\ge0)$ 4개뿐이다.
$a=0$ 에서 2차원과 1.6e-13 yd 일치, 두 대칭 모두 1e-12 yd 이하로 성립.
4차원 장 809,472점(6.5 MB), 4선형 읽기 최대 오차 0.19 yd.

## 허점과 반례

- π/4 와 Z 의 상수는 훈련 자료를 본 뒤 고른 [경험식]/[가설]이다. 단순 상수 42쌍 점검에서 Z 는 특별하지 않았고, $(v_{\text{ev}}, h)$ 에 1차원 골짜기가 있어 이 자료는 조합 하나만 정한다.
- $S\to0$ 극한의 항력 $C_D(0)=0.388$ 은 자료가 정하지 않은 값이며 문헌값보다 높다.
- 스핀 감쇠($C_M=0.012\,S$)는 고정 가정이고 검증하지 않았다.
- 반례: F1(π/2 포화), F2(오일러꼴), log 저장, 진공해 배경 비율 저장.
- 레이놀즈수 의존성과 지면 접촉은 아직 없다. 사이드 스핀은 모형만 있고 실측 대조가 없다.

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

cd flightbench && cargo run --release
```
