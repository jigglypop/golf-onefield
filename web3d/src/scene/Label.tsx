/** 항상 카메라를 보는 글자판 (Sprite + 캔버스 텍스처). */
import { useEffect, useMemo } from 'react';

import { labelTexture, type LabelLine } from './textures.ts';

export interface LabelProps {
  lines: LabelLine[];
  position: [number, number, number];
  /** 월드 높이 (m) */
  height: number;
  w?: number;
  h?: number;
  bg?: string;
  border?: string;
  fog?: boolean;
  renderOrder?: number;
}

export function Label({ lines, position, height, w = 512, h = 192, bg, border, fog = true, renderOrder = 0 }: LabelProps) {
  const key = JSON.stringify([lines, w, h, bg, border]);
  // key 가 같으면 텍스처를 다시 만들지 않는다
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const texture = useMemo(() => labelTexture(lines, { w, h, ...(bg ? { bg } : {}), ...(border ? { border } : {}) }), [key]);
  useEffect(() => () => texture.dispose(), [texture]);
  return (
    <sprite position={position} scale={[(height * w) / h, height, 1]} renderOrder={renderOrder}>
      <spriteMaterial map={texture} transparent depthWrite={false} toneMapped={false} fog={fog} />
    </sprite>
  );
}
