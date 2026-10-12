/**
 * 효과음 시간 맞추기·범위: 가짜 AudioContext 로 "언제 시작하라고 예약했는지"를 읽는다.
 *  - 타구음은 임팩트까지 남은 시간(delay)만큼 뒤의 오디오 시각에 예약한다 (예전에는 고정 0.2 s).
 *  - 바운드·굴림·나무·바람 배경이 만들어지고, 끄기 설정이면 새 소리가 안 만들어진다.
 *  - cancelScheduled 는 샷에 맞춘 소리만 끊고 홀컵·박수 같은 자유 소리는 두고 간다.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

interface FakeNode {
  kind: 'osc' | 'src' | 'other';
  starts: number[];
  stops: (number | undefined)[];
  loop?: boolean;
  [k: string]: unknown;
}

const created: FakeNode[] = [];

class FakeParam {
  value = 0;
  setValueAtTime() {
    return this;
  }
  exponentialRampToValueAtTime() {
    return this;
  }
  linearRampToValueAtTime() {
    return this;
  }
  setTargetAtTime() {
    return this;
  }
}

function node(kind: FakeNode['kind']): FakeNode {
  const n: FakeNode = {
    kind,
    starts: [],
    stops: [],
    connect(next: unknown) {
      return next;
    },
    start(t?: number) {
      n.starts.push(t ?? 0);
    },
    stop(t?: number) {
      n.stops.push(t);
    },
  };
  return n;
}

class FakeAC {
  currentTime = 10;
  state = 'running';
  sampleRate = 8000;
  destination = {};
  createGain() {
    const n = node('other');
    n.gain = new FakeParam();
    return n;
  }
  createOscillator() {
    const n = node('osc');
    n.frequency = new FakeParam();
    created.push(n);
    return n;
  }
  createBufferSource() {
    const n = node('src');
    created.push(n);
    return n;
  }
  createBiquadFilter() {
    const n = node('other');
    n.frequency = new FakeParam();
    n.Q = new FakeParam();
    return n;
  }
  createBuffer(_c: number, len: number) {
    return { getChannelData: () => new Float32Array(len) };
  }
  resume() {
    return Promise.resolve();
  }
}

type Sound = typeof import('../src/game/sound.ts');
let sound: Sound;

beforeEach(async () => {
  created.length = 0;
  vi.resetModules();
  vi.stubGlobal('window', { AudioContext: FakeAC });
  vi.stubGlobal('localStorage', { getItem: () => null, setItem: () => {} });
  sound = await import('../src/game/sound.ts');
  sound.ensureAudio();
  created.length = 0; // 켤 때 만든 것(없음)은 버린다
});

/** 시작 시각이 t 인 노드 개수 */
const startedAt = (t: number, kind?: FakeNode['kind']) => created.filter((n) => (kind ? n.kind === kind : true) && n.starts.some((s) => Math.abs(s - t) < 1e-9)).length;

describe('S2 타구음은 임팩트 시각에 예약한다', () => {
  it('playSwing(q, 0.44): 타구 톤은 currentTime + 0.44, 바람 소리는 그 0.22 s 앞', () => {
    sound.playSwing(1, 0.44);
    expect(startedAt(10.44, 'osc')).toBe(1); // 타구 톤
    expect(startedAt(10.44, 'src')).toBe(1); // 타구 잡음
    expect(startedAt(10.22, 'src')).toBe(1); // 스윙 바람
    // 예전 고정 지연 0.2 s 에는 아무것도 없다
    expect(startedAt(10.2)).toBe(0);
  });

  it('임팩트가 바람 길이보다 가까우면 바람 소리는 바로 시작한다', () => {
    sound.playSwing(0.5, 0.1);
    expect(startedAt(10.1, 'osc')).toBe(1);
    expect(startedAt(10, 'src')).toBe(1);
  });

  it('퍼트도 같다', () => {
    sound.playPutt(0.48);
    expect(startedAt(10.48, 'osc')).toBe(1);
  });
});

describe('S3 바운드·굴림·나무·바람', () => {
  it('바운드: delay 만큼 뒤에 톤과 잡음', () => {
    sound.playBounce(5, 'fairway', 0.3);
    expect(startedAt(10.3, 'osc')).toBe(1);
    expect(startedAt(10.3, 'src')).toBe(1);
    created.length = 0;
    sound.playBounce(5, 'bunker', 0.3); // 모래는 잡음만
    expect(created.filter((n) => n.kind === 'osc').length).toBe(0);
    expect(startedAt(10.3, 'src')).toBe(1);
  });

  it('굴림: 되풀이 잡음 한 개가 delay 에 시작하고, 모래·물에서는 안 난다', () => {
    const prof = [{ t: 0, v: 5 }, { t: 0.5, v: 2 }, { t: 1, v: 0.1 }];
    sound.playRoll(prof, 'fairway', 0.4);
    expect(created.length).toBe(1);
    expect(created[0]!.starts[0]).toBeCloseTo(10.4, 9);
    expect(created[0]!.loop).toBe(true);
    created.length = 0;
    sound.playRoll(prof, 'bunker', 0.4);
    sound.playRoll([{ t: 0, v: 1 }], 'fairway', 0);
    sound.playRoll([], 'fairway', 0);
    expect(created.length).toBe(0);
  });

  it('나무 맞는 소리, 착지음(잔디·모래·물), 홀컵이 만들어진다', () => {
    sound.playTree(0.7);
    expect(startedAt(10.7, 'osc')).toBe(1);
    created.length = 0;
    sound.playLand('fairway');
    sound.playLand('bunker');
    sound.playLand('water');
    sound.playCup();
    expect(created.length).toBeGreaterThan(8);
  });

  it('바람 배경: 켜면 되풀이 잡음과 천천히 흔드는 LFO 가 하나씩, 속도를 바꿔도 새로 만들지 않는다', () => {
    sound.setWindAmbient(3);
    expect(created.filter((n) => n.kind === 'src').length).toBe(1);
    expect(created.filter((n) => n.kind === 'src')[0]!.loop).toBe(true);
    expect(created.filter((n) => n.kind === 'osc').length).toBe(1);
    sound.setWindAmbient(6);
    expect(created.length).toBe(2);
  });

  it('오디오가 켜지기 전에 받은 바람 속도는 켜질 때 적용된다', async () => {
    vi.resetModules();
    created.length = 0;
    const fresh = await import('../src/game/sound.ts');
    fresh.setWindAmbient(2); // 아직 ensureAudio 전: 만들 수 없다
    expect(created.length).toBe(0);
    fresh.ensureAudio();
    expect(created.length).toBe(2);
  });
});

describe('끄기 설정과 예약 취소', () => {
  it('끄면 새 소리를 만들지 않는다 (끄기 설정은 저장소에 남긴다)', () => {
    const set = vi.fn();
    vi.stubGlobal('localStorage', { getItem: () => null, setItem: set });
    sound.setMuted(true);
    expect(set).toHaveBeenCalledWith('golf-onefield-mute', '1');
    sound.playSwing(1, 0.44);
    sound.playBounce(5, 'fairway', 0.1);
    sound.playRoll([{ t: 0, v: 5 }, { t: 1, v: 1 }], 'fairway', 0);
    sound.playTree(0);
    sound.playCup();
    sound.playApplause(1);
    expect(created.length).toBe(0);
    sound.setMuted(false);
    sound.playClick();
    expect(created.length).toBe(1);
  });

  it('저장소가 막혀 있어도 끄기·켜기가 돈다', () => {
    const boom = () => {
      throw new Error('blocked');
    };
    vi.stubGlobal('localStorage', { getItem: boom, setItem: boom });
    expect(() => sound.setMuted(true)).not.toThrow();
    expect(sound.isMuted()).toBe(true);
    expect(() => sound.setMuted(false)).not.toThrow();
  });

  it('cancelScheduled: 샷에 맞춘 소리는 끊고, 홀컵·박수·착지음은 그대로 둔다', () => {
    sound.playSwing(1, 0.44);
    sound.playBounce(5, 'fairway', 0.3);
    const shotCues = created.slice();
    created.length = 0;
    sound.playCup();
    sound.playLand('fairway');
    const free = created.slice();
    sound.cancelScheduled();
    // 샷 소리: 예약한 stop(시각) 뒤에 stop() 한 번 더 불렸다
    for (const n of shotCues) expect(n.stops.length).toBe(2);
    for (const n of free) expect(n.stops.length).toBe(1);
    // 두 번 불러도 안전하다
    expect(() => sound.cancelScheduled()).not.toThrow();
  });
});
