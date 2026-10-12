/**
 * flightbench `screen --csv` 출력 붙여넣기 해석.
 *
 * 쓰는 줄 (screen.rs csv_mode 의 마지막 줄들):
 *   볼스피드 120.0 mph, 발사각 16.30°, 방위각 0.01°
 *   스핀 7061 rpm, 스핀축 8.0°, 캐리 168.2 yd
 * '닫힌 꼴 첫 값: 스핀 … rpm, 축 …°' 줄은 '스핀축' 이 없어 건너뛴다. 같은 이름이 여러 번 나오면 마지막 값을 쓴다.
 * 스핀축은 엔진 부호(+ = 왼쪽으로 휨)라 UI(트랙맨)로 넣을 때 부호를 뒤집는다 (convention.ts).
 */
import { trackmanAxisFromScreen, trackmanAzimuthFromScreen, type ShotInput } from './convention.ts';

export interface ParsedScreen {
  /** 찾은 값 (UI 부호) */
  values: Partial<ShotInput>;
  /** screen 출력에 적힌 스핀축 (엔진 부호) */
  screenAxisDeg?: number;
  /** screen 출력에 적힌 캐리 (yd) */
  screenCarryYd?: number;
  /** 못 찾은 항목 이름 */
  missing: string[];
}

const NUM = String.raw`([-−+]?\d+(?:\.\d+)?)`;
const num = (s: string): number => Number(s.replace('−', '-'));

function last(text: string, re: RegExp, lineFilter?: (line: string) => boolean): number | undefined {
  let found: number | undefined;
  for (const line of text.split(/\r?\n/)) {
    if (lineFilter && !lineFilter(line)) continue;
    const m = re.exec(line);
    if (m?.[1] !== undefined) found = num(m[1]);
  }
  return found;
}

export function parseScreenOutput(text: string): ParsedScreen {
  const spinLine = (line: string) => line.includes('스핀축');
  const mph = last(text, new RegExp(`볼스피드\\s*${NUM}\\s*mph`));
  const launchDeg = last(text, new RegExp(`발사각\\s*${NUM}`));
  const azimuth = last(text, new RegExp(`방위각\\s*${NUM}`));
  const rpm = last(text, new RegExp(`스핀\\s*${NUM}\\s*rpm`), spinLine);
  const axis = last(text, new RegExp(`스핀축\\s*${NUM}`), spinLine);
  const carry = last(text, new RegExp(`캐리\\s*${NUM}\\s*yd`), spinLine);

  const values: Partial<ShotInput> = {};
  const missing: string[] = [];
  if (mph !== undefined) values.mph = mph;
  else missing.push('볼스피드');
  if (launchDeg !== undefined) values.launchDeg = launchDeg;
  else missing.push('발사각');
  if (azimuth !== undefined) values.azimuthDeg = trackmanAzimuthFromScreen(azimuth);
  else missing.push('방위각');
  if (rpm !== undefined) values.rpm = rpm;
  else missing.push('스핀');
  if (axis !== undefined) values.spinAxisDeg = trackmanAxisFromScreen(axis);
  else missing.push('스핀축');

  const out: ParsedScreen = { values, missing };
  if (axis !== undefined) out.screenAxisDeg = axis;
  if (carry !== undefined) out.screenCarryYd = carry;
  return out;
}
