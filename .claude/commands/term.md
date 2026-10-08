---
description: 장의 항(수식)을 바꿔 보고 시험대에서 판정한다
argument-hint: <바꾸고 싶은 항이나 아이디어>
---
요청: $ARGUMENTS

1. `flightbench/models_search.txt` 에 새 꼴을 `[이름]` 블록으로 추가한다 (kd = |u|C_D, kl = |u|C_L, 변수 u w S Re, 매개변수 "이름 = 시작 하한 상한").
2. `cd flightbench && cargo build -q --profile quick && MODELS=models_search.txt ./target/quick/quick --cv --tour` 로 교차검증 순위를 낸다. 고르기는 CV 로만 한다.
3. 상위 꼴을 `./target/quick/quick <이름>` (PGA 만 훈련)으로 LPGA·USGA·FS 판정과 함께 보여 준다. 기준선은 '공유위기'와 OF+2.
4. 이기면 `models.txt` 맨 위에 올리고, 빠른 엔진(`src/engine.rs` FastAero)에 넣을 가치가 있는지 판단해 제안한다. 시험한 꼴은 지우지 않는다.
