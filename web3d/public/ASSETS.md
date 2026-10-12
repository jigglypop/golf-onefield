# 외부 에셋 출처

모두 [Poly Haven](https://polyhaven.com) 의 **CC0**(퍼블릭 도메인) 텍스처다. 크레딧 의무는 없지만 출처를 남긴다.
`scripts/fetch_textures.py` 가 1k 원본(diff / nor_gl / arm)을 받아 두 장으로 묶는다 (원본은 저장소에 올리지 않는다).

- `tex/<이름>_a.webp` : RGB = 알베도 × AO(부분 곱), 1024 px
- `tex/<이름>_n.webp` : R,G = 법선(OpenGL, xy), B = 거칠기, 1024 px (`src/photo/surfaces.ts` 의 `packedMaps` 가 법선 지도·거칠기 지도로 푼다)

| 파일 이름 | Poly Haven 에셋 | 쓰는 곳 |
|---|---|---|
| `sand` | [dense_sand](https://polyhaven.com/a/dense_sand) | 벙커 모래 (밝기 1.9배) |
| `rock` | [rock_3](https://polyhaven.com/a/rock_3) | 호숫가·숲 가장자리 바위 |
| `pinebark` | [pine_bark](https://polyhaven.com/a/pine_bark) | 나무 줄기 |
| `barkbrown` | [bark_brown_02](https://polyhaven.com/a/bark_brown_02) | 안내판·벤치 나무 |

잔디(러프·페어웨이·그린·티), 솔잎·잎, 구름, 갈대, 물결, 큰 얼룩, 깎은 줄무늬, 지형·나무·바위 모양은 외부 에셋이 아니라 코드로 만든다 (`src/photo/`).
(Poly Haven 에 골프장 같은 짧게 깎은 잔디 텍스처가 없고 `leafy_grass` 는 낙엽이 섞여 어울리지 않아 잔디는 직접 그렸다.)
