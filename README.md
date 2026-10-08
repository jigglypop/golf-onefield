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

## 허점과 반례

- π/4 와 Z 의 상수는 훈련 자료를 본 뒤 고른 [경험식]/[가설]이다. 단순 상수 42쌍 점검에서 Z 는 특별하지 않았고, $(v_{\text{ev}}, h)$ 에 1차원 골짜기가 있어 이 자료는 조합 하나만 정한다.
- $S\to0$ 극한의 항력 $C_D(0)=0.388$ 은 자료가 정하지 않은 값이며 문헌값보다 높다.
- 스핀 감쇠($C_M=0.012\,S$)는 고정 가정이고 검증하지 않았다.
- 반례: F1(π/2 포화), F2(오일러꼴), log 저장, 진공해 배경 비율 저장.
- 레이놀즈수 의존성, 사이드 스핀, 지면 접촉은 아직 없다.

## 파일

| 파일 | 내용 |
|---|---|
| `onefield_carry.py` | 첫 검산: B·F1–F4 모형 적합 (solve_ivp) |
| `higgs_engine.py` | 무차원 배치 RK4 엔진, 힉스꼴 장, 펼친 장 |
| `run_higgs.py` | 엔진 검산: 일치·재피팅·ψ 프로파일·보편성 |
| `zero_param.py` | 매개변수 0개 가설과 눈길 효과 점검 |
| `compress_field.py` | 장 압축 실험 (반례) |
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

cd flightbench && cargo run --release
```
