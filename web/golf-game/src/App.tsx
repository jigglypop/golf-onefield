import { useCallback, useEffect, useRef, useState } from 'react';
import { Canvas } from '@react-three/fiber';
import { createGaesupRuntime, createRenderer, GaesupWorld } from 'gaesup-world';

import { HoleScene } from './game/HoleScene';
import { REACH_M, TEE, type Vec3 } from './game/layout';
import { fireShot, type ShotRecord } from './game/shot';
import { batchCarryYd, loadGolfCore, type CoreInfo, type ShotParams } from './physics/golfCore';
import { CLUBS, DEFAULT_PARAMS, PARAM_FIELDS, PRESETS, type PresetId } from './physics/presets';
import { ResultCard } from './ui/ResultCard';
import { ShotPanel } from './ui/ShotPanel';

/** 캐릭터 모델은 public/gltf 아래 (gaesup-world 의 trainer_green.glb, 예제와 같은 파일) */
const WORLD_URLS = { characterUrl: '/gltf/trainer_green.glb' };

/**
 * 엔진 카메라 설정. 엔진의 thirdPerson 오프셋은 월드 기준 (-x, y, -z) 이다 (캐릭터 방향과 무관).
 * 조준 방향은 목표(-Z)이므로 zDistance 를 음수로 두어 카메라를 +Z 쪽, 즉 골퍼의 등 뒤에 둔다.
 */
const CAMERA_OPTION = { type: 'thirdPerson', xDistance: 0, yDistance: 7, zDistance: -15, fov: 50 } as const;

const MOBILE_QUERY = '(max-width: 720px)';

export function App() {
  // 한 페이지에 월드가 하나라 런타임 하나를 쓴다 (store 가 전역이 아니라 이 월드 소속이 되어 경고가 없다)
  const [runtime] = useState(() => createGaesupRuntime({ worldId: 'golf-hole-1' }));
  const [wasmReady, setWasmReady] = useState(false);
  /** 불러온 코어의 상태 (SIMD 여부) 와 클럽별 캐리 (일괄 계산 한 번) */
  const [coreInfo, setCoreInfo] = useState<CoreInfo | null>(null);
  const [clubCarries, setClubCarries] = useState<Float64Array | null>(null);
  const [worldReady, setWorldReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [params, setParams] = useState<ShotParams>(DEFAULT_PARAMS);
  /** 장면에 넘기는 현재 샷 (비행 중이거나 마지막으로 날린 샷). null 이면 볼은 티에 있다. */
  const [shot, setShot] = useState<ShotRecord | null>(null);
  /** 결과 카드용: 착지가 끝난 마지막 샷 */
  const [lastShot, setLastShot] = useState<ShotRecord | null>(null);
  const [flying, setFlying] = useState(false);
  const [reachable, setReachable] = useState(true);
  const [shotCount, setShotCount] = useState(0);
  const [panelOpen, setPanelOpen] = useState(() => !window.matchMedia(MOBILE_QUERY).matches);

  // wasm 코어 로드 (한 번)
  useEffect(() => {
    let alive = true;
    loadGolfCore().then(
      (info) => {
        if (!alive) return;
        setCoreInfo(info);
        setClubCarries(batchCarryYd(CLUBS.map((c) => c.params)));
        setWasmReady(true);
      },
      (e: unknown) => alive && setError(`물리 코어를 불러오지 못했습니다: ${String(e)}`),
    );
    return () => {
      alive = false;
    };
  }, []);

  // 월드 런타임 설정. StrictMode 의 setup → dispose → setup 도 런타임이 줄 세워 처리한다.
  useEffect(() => {
    let alive = true;
    runtime.setup().then(
      () => alive && setWorldReady(true),
      (e: unknown) => alive && setError(`월드를 준비하지 못했습니다: ${String(e)}`),
    );
    return () => {
      alive = false;
      runtime.dispose().catch((e: unknown) => console.error(e));
    };
  }, [runtime]);

  const ready = wasmReady && worldReady && !error;
  // 볼 위치: 비행 중이면 출발점, 착지 뒤에는 착지점, 샷이 없으면 티
  const ballPos: Vec3 = shot ? (flying ? shot.start : shot.landing) : TEE;
  const canShoot = ready && !flying && reachable;

  const shoot = () => {
    if (!canShoot) return;
    const id = shotCount + 1;
    setShotCount(id);
    setShot(fireShot(ballPos, params, id));
    setFlying(true);
  };

  // Enter 로 샷. 장면과 입력 상태가 바뀌어도 최신 shoot 을 부르도록 ref 에 둔다.
  const shootRef = useRef(shoot);
  useEffect(() => {
    shootRef.current = shoot;
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Enter' || e.repeat) return;
      // 입력칸과 버튼에서의 Enter 는 원래 동작(값 반영, 버튼 누름)을 그대로 둔다
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'BUTTON' || tag === 'SELECT' || tag === 'TEXTAREA') return;
      e.preventDefault();
      shootRef.current();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const onLanded = useCallback((rec: ShotRecord) => {
    setLastShot(rec);
    setFlying(false);
  }, []);
  const onReachChange = useCallback((ok: boolean) => setReachable(ok), []);

  const reset = () => {
    setShot(null);
    setFlying(false);
  };

  const applyPreset = (id: PresetId) => {
    const preset = PRESETS.find((p) => p.id === id);
    if (preset) setParams((prev) => ({ ...prev, ...preset.params }));
  };

  const applyClub = (id: string) => {
    const club = CLUBS.find((c) => c.id === id);
    if (club) setParams((prev) => ({ ...prev, ...club.params }));
  };

  const setField = (key: keyof ShotParams, value: number) => {
    const field = PARAM_FIELDS.find((f) => f.key === key);
    if (!field) return;
    const clamped = Math.min(field.max, Math.max(field.min, value));
    setParams((prev) => ({ ...prev, [key]: clamped }));
  };

  const hint = error
    ? error
    : !ready
      ? '불러오는 중…'
      : flying
        ? '비행 중…'
        : !reachable
          ? `공에서 ${REACH_M} m 안으로 걸어가 주세요`
          : `Enter 또는 샷 버튼 · 조준 ${params.hlaDeg > 0 ? '+' : ''}${params.hlaDeg.toFixed(1)}°`;

  return (
    <GaesupWorld runtime={runtime} urls={WORLD_URLS} cameraOption={CAMERA_OPTION}>
      <Canvas
        shadows="percentage"
        gl={createRenderer}
        camera={{ position: [-3, 6, 14], fov: 50 }}
        style={{ position: 'fixed', inset: 0 }}
      >
        <HoleScene shot={shot} ballPos={ballPos} onLanded={onLanded} onReachChange={onReachChange} />
      </Canvas>

      <div className="hud">
        <header className="title">
          <h1>골프 한 홀</h1>
          <p>
            티에서 깃대까지 약 230 m (250 yd) · 샷 {shotCount}회
          </p>
        </header>

        <div className="result-wrap">
          <ResultCard shot={lastShot} />
        </div>

        <ShotPanel
          params={params}
          onField={setField}
          onPreset={applyPreset}
          core={coreInfo}
          clubCarries={clubCarries}
          onClub={applyClub}
          onShoot={shoot}
          onReset={reset}
          canShoot={canShoot}
          hint={hint}
          open={panelOpen}
          onToggle={() => setPanelOpen((v) => !v)}
        />
      </div>

      {!ready && (
        <div className="overlay" role="status">
          <p>{error ?? '물리 코어와 월드를 불러오는 중…'}</p>
        </div>
      )}
    </GaesupWorld>
  );
}
