/**
 * 품질 프로필: 데스크톱(기본)·모바일·XR(Quest 등)에서 같은 장면을 어느 수준으로 그릴지 한곳에서 정한다.
 *
 *   기본            dpr [1, 2], 후처리(TRAA·GTAO) 켬, 그림자는 gaesup-world 자동 단계(= 예전 그대로)
 *   모바일/XR       dpr 낮춤, 후처리 끔, 그림자 단계 2·512², 숲·풀잎 밀도 축소, 텍스처 512², 이방성 2
 *
 * 판정(이 순서):
 *   mobile = `?profile=mobile` 이거나 matchMedia('(pointer: coarse)') 가 참  (`?profile=desktop` 이면 강제로 아님)
 *   xr     = `?xr` 가 있다 (WebXR 세션 안의 헤드셋 브라우저에서 켠다)
 *   `?quality=low|medium|high` 는 예전 뜻 그대로 숲·풀잎 밀도 배율만 바꾼다 (low 0.35·0.3, medium 0.65·0.6).
 *   `?postfx=0|off` 로 후처리만, `?dpr=1.25` 로 dpr 상한만 따로 덮어쓸 수 있다 (점검용).
 *
 * `shadowCascades`·`shadowMapSize` 가 0 이면 gaesup-world 가 GPU 를 보고 고르는 자동 단계를 그대로 쓴다 (예전 동작).
 * 프로필은 페이지가 열릴 때 한 번 정해진다: `currentProfile()` 는 같은 객체를 돌려준다.
 */

export interface QualityProfile {
  /** R3F Canvas dpr 범위 */
  dpr: [number, number];
  /** 후처리 스택(TRAA·GTAO·블룸·채도) */
  postFx: boolean;
  /** 해 그림자 단계 수. 0 = gaesup 자동 */
  shadowCascades: number;
  /** 그림자 지도 한 변 (px). 0 = gaesup 자동 */
  shadowMapSize: number;
  /** 숲 격자 밀도 배율 (1 = 전부) */
  forestDensity: number;
  /** 러프 풀잎 밀도·최대 수 배율 */
  grassDensity: number;
  /** 텍스처 이방성 필터 (기본 4, 모바일 2) */
  anisotropy: number;
  /** 캔버스로 만드는 큰 질감의 한 변 (px) */
  texSize: number;
  mobile: boolean;
  xr: boolean;
}

export interface ProfileOpts {
  mobile: boolean;
  xr: boolean;
  query: URLSearchParams;
}

/** `?quality=` 의 숲·풀잎 배율 (예전 Forest.tsx·RoughGrass.tsx 의 값 그대로) */
function densityOf(q: string | null): { forest: number; grass: number } {
  return q === 'low' ? { forest: 0.35, grass: 0.3 } : q === 'medium' ? { forest: 0.65, grass: 0.6 } : { forest: 1, grass: 1 };
}

export function profileFor(opts: ProfileOpts): QualityProfile {
  const { mobile, xr, query } = opts;
  const light = mobile || xr;
  const d = densityOf(query.get('quality'));
  // 모바일·XR 에서는 기본 밀도 위에 한 번 더 줄인다 (?quality 와 곱하지 않고 더 작은 쪽)
  const forestDensity = light ? Math.min(d.forest, 0.5) : d.forest;
  const grassDensity = light ? Math.min(d.grass, 0.5) : d.grass;
  const fx = query.get('postfx');
  const postFx = fx === '0' || fx === 'off' ? false : fx === '1' || fx === 'on' ? true : !light;
  const dprCap = Number(query.get('dpr'));
  let dpr: [number, number] = xr ? [1, 1] : mobile ? [1, 1.5] : [1, 2];
  if (Number.isFinite(dprCap) && dprCap >= 0.5 && query.has('dpr')) dpr = [Math.min(1, dprCap), dprCap];
  return {
    dpr,
    postFx,
    shadowCascades: light ? 2 : 0,
    shadowMapSize: light ? 512 : 0,
    forestDensity,
    grassDensity,
    anisotropy: light ? 2 : 4,
    texSize: light ? 512 : 1024,
    mobile,
    xr,
  };
}

function detect(): ProfileOpts {
  if (typeof location === 'undefined') return { mobile: false, xr: false, query: new URLSearchParams() };
  const query = new URLSearchParams(location.search);
  const forced = query.get('profile');
  let coarse = false;
  try {
    coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches;
  } catch {
    coarse = false;
  }
  return { mobile: forced === 'mobile' || (forced !== 'desktop' && coarse), xr: query.has('xr'), query };
}

let cached: QualityProfile | null = null;

/** 이 페이지의 프로필 (URL·포인터 종류를 처음 부를 때 한 번 읽는다) */
export function currentProfile(): QualityProfile {
  cached ??= profileFor(detect());
  return cached;
}
