"""
Poly Haven (CC0) PBR 텍스처를 내려받아 게임용으로 묶는다 (PYTHONUTF8=1 python scripts/fetch_textures.py [작업폴더]).
  <이름>_a.webp : RGB = 알베도 × AO(부분 곱), 1024 px
  <이름>_n.webp : R,G = 법선(OpenGL, xy), B = 거칠기, 1024 px
원본(diff / nor_gl / arm 1k jpg)은 작업폴더에만 두고 저장소에는 올리지 않는다. 출처·라이선스는 public/ASSETS.md.
"""
import io
import json
import sys
import urllib.request
from pathlib import Path

from PIL import Image

ASSETS = {
    # 이름: (Poly Haven id, 알베도 쪽 AO 곱 세기, 한 변 px)
    'sand': ('dense_sand', 0.7, 1024),
    'rock': ('rock_3', 0.8, 1024),
    'pinebark': ('pine_bark', 0.8, 1024),
    'barkbrown': ('bark_brown_02', 0.8, 1024),
}
UA = {'User-Agent': 'golf-onefield-asset-fetch/1.0'}
OUT = Path(__file__).resolve().parent.parent / 'public' / 'tex'
WORK = Path(sys.argv[1]) if len(sys.argv) > 1 else Path('ph_work')
WORK.mkdir(parents=True, exist_ok=True)
OUT.mkdir(parents=True, exist_ok=True)


def get(url: str) -> bytes:
    return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120).read()


def main() -> None:
    total = 0
    for key, (pid, ao_k, px) in ASSETS.items():
        files = json.loads(get(f'https://api.polyhaven.com/files/{pid}'))
        imgs = {}
        for m in ('Diffuse', 'nor_gl', 'arm'):
            cache = WORK / f'{pid}_{m}.jpg'
            if not cache.exists():
                cache.write_bytes(get(files[m]['1k']['jpg']['url']))
            imgs[m] = Image.open(cache).convert('RGB').resize((px, px), Image.LANCZOS)
        diff, nor, arm = imgs['Diffuse'], imgs['nor_gl'], imgs['arm']
        ao = arm.getchannel('R').point(lambda v: int(255 * (1 - ao_k + ao_k * v / 255)))
        r, g, b = diff.split()
        from PIL import ImageChops

        albedo = Image.merge('RGB', (ImageChops.multiply(r, ao), ImageChops.multiply(g, ao), ImageChops.multiply(b, ao)))
        nr, ng, _ = nor.split()
        nrm = Image.merge('RGB', (nr, ng, arm.getchannel('G')))
        for suffix, im, q in (('a', albedo, 82), ('n', nrm, 85)):
            buf = io.BytesIO()
            im.save(buf, 'WEBP', quality=q, method=6)
            (OUT / f'{key}_{suffix}.webp').write_bytes(buf.getvalue())
            total += len(buf.getvalue())
        print(f'{key:10s} {pid:20s} ok')
    print(f'합계 {total / 1e6:.2f} MB')


if __name__ == '__main__':
    main()
