/** 캐릭터의 Rapier 강체. World.tsx 가 GaesupController 에 넘기고, 게임이 공 옆으로 옮길 때 쓴다. */
import type { RefObject } from 'react';

import type { RapierRigidBody } from '@react-three/rapier';

export const playerBody = { current: null } as unknown as RefObject<RapierRigidBody>;
