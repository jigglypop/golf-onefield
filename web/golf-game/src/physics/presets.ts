import type { ShotParams } from './golfCore';

/** 프리셋 값. 드라이버·7번·웨지는 PRD 기준의 대표 샷 (스핀축 0, 조준 0) */
export type PresetId = 'driver' | 'iron7' | 'wedge';

export const PRESETS: { id: PresetId; label: string; params: Omit<ShotParams, 'hlaDeg'> }[] = [
  { id: 'driver', label: '드라이버', params: { mph: 167, launchDeg: 10.9, spinRpm: 2686, axisDeg: 0 } },
  { id: 'iron7', label: '7번', params: { mph: 120, launchDeg: 16.3, spinRpm: 7097, axisDeg: 0 } },
  { id: 'wedge', label: '웨지', params: { mph: 86, launchDeg: 25.7, spinRpm: 8403, axisDeg: 0 } },
];

/** 조작판 항목 (입력 범위와 한국어 라벨) */
export interface ParamField {
  key: keyof ShotParams;
  label: string;
  unit: string;
  min: number;
  max: number;
  step: number;
  hint: string;
}

export const PARAM_FIELDS: ParamField[] = [
  { key: 'mph', label: '볼 스피드', unit: 'mph', min: 40, max: 190, step: 1, hint: '' },
  { key: 'launchDeg', label: '발사각', unit: '°', min: 0, max: 45, step: 0.1, hint: '' },
  { key: 'spinRpm', label: '스핀', unit: 'rpm', min: 500, max: 12000, step: 10, hint: '' },
  { key: 'axisDeg', label: '스핀축', unit: '°', min: -45, max: 45, step: 0.5, hint: '음수 = 슬라이스' },
  { key: 'hlaDeg', label: '조준 (HLA)', unit: '°', min: -20, max: 20, step: 0.5, hint: '양수 = 오른쪽' },
];

export const DEFAULT_PARAMS: ShotParams = { ...PRESETS[0]!.params, hlaDeg: 0 };

/**
 * 클럽별 발사 조건: PGA 투어 평균(공개 TrackMan 표, flightbench 의 PGA 12행·web3d 참값 표와 같은 입력).
 * 입력은 실측 평균이고, 클럽표의 캐리는 우리 엔진이 계산한 시뮬레이션 값이다 (둘을 섞어 쓰지 않는다).
 */
export const CLUBS: { id: string; label: string; params: Omit<ShotParams, 'hlaDeg'> }[] = [
  { id: 'dr', label: '드라이버', params: { mph: 167, launchDeg: 10.9, spinRpm: 2686, axisDeg: 0 } },
  { id: 'w3', label: '3번 우드', params: { mph: 158, launchDeg: 9.2, spinRpm: 3655, axisDeg: 0 } },
  { id: 'w5', label: '5번 우드', params: { mph: 152, launchDeg: 9.4, spinRpm: 4350, axisDeg: 0 } },
  { id: 'hy', label: '하이브리드', params: { mph: 146, launchDeg: 10.2, spinRpm: 4437, axisDeg: 0 } },
  { id: 'i3', label: '3번 아이언', params: { mph: 142, launchDeg: 10.4, spinRpm: 4630, axisDeg: 0 } },
  { id: 'i4', label: '4번 아이언', params: { mph: 137, launchDeg: 11, spinRpm: 4836, axisDeg: 0 } },
  { id: 'i5', label: '5번 아이언', params: { mph: 132, launchDeg: 12.1, spinRpm: 5361, axisDeg: 0 } },
  { id: 'i6', label: '6번 아이언', params: { mph: 127, launchDeg: 14.1, spinRpm: 6231, axisDeg: 0 } },
  { id: 'i7', label: '7번 아이언', params: { mph: 120, launchDeg: 16.3, spinRpm: 7097, axisDeg: 0 } },
  { id: 'i8', label: '8번 아이언', params: { mph: 115, launchDeg: 18.1, spinRpm: 7998, axisDeg: 0 } },
  { id: 'i9', label: '9번 아이언', params: { mph: 109, launchDeg: 20.4, spinRpm: 8647, axisDeg: 0 } },
  { id: 'pw', label: '피칭웨지', params: { mph: 102, launchDeg: 24.2, spinRpm: 9304, axisDeg: 0 } },
];
