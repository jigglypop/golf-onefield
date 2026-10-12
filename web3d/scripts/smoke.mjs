// 브라우저 연기 시험: vite preview 를 띄우고(또는 주어진 URL), 헤드리스 Chromium 으로 연다.
//   코스(기본): 메뉴 → 2명·9홀 → 홀 소개 → Space 세 번 스윙(판정) → F 빨리 감기 → 다음 사람 → V 홀 전체 → 봇 9홀 완주 → 니어핀 → 롱드라이브.
//   연습장(?mode=range): 첫 화면·비행 중·착지·옆·위·screen 출력 붙여넣기·캐릭터 걷기·Enter 샷.
// 화면을 찍고 콘솔 오류·경고를 모은다. 먼저 `npm run build`.
//
//   npm run smoke                                  # 두 번: gpu(새 헤드리스, WebGPU 가 되면 WebGPU) + swiftshader(classic WebGL)
//   npm run smoke -- --out /tmp/shots --pass gpu   # 찍은 그림 위치, 한 번만
//   npm run smoke -- --url http://127.0.0.1:5190/  # 이미 떠 있는 dev 서버
//
// 띄운 preview 서버는 끝날 때 PID 로 끈다.
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const OUT = resolve(opt('--out', join(ROOT, 'screenshots')));
const PASSES = opt('--pass', 'gpu,swiftshader').split(',');
const PORT = Number(opt('--port', '4174'));
let url = opt('--url', '');

const LAUNCH = {
  // 새 헤드리스 Chromium: 이 맥에서는 Metal GPU 로 WebGL·WebGPU 가 된다
  gpu: { channel: 'chromium', headless: true, args: ['--enable-unsafe-webgpu', '--enable-gpu'] },
  // 헤드리스 셸 + SwiftShader: WebGPU 어댑터가 없어 gaesup-world createRenderer 가 classic WebGL 로 간다
  swiftshader: { headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] },
};

mkdirSync(OUT, { recursive: true });

async function waitHttp(target, ms) {
  const end = Date.now() + ms;
  let last = '';
  while (Date.now() < end) {
    try {
      const r = await fetch(target);
      if (r.ok) return;
      last = `HTTP ${r.status}`;
    } catch (e) {
      last = String(e.cause?.code ?? e.message); // 아직 안 떴다
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`${target} 가 ${ms} ms 안에 뜨지 않음 (${last})`);
}

let server = null;
if (!url) {
  url = `http://127.0.0.1:${PORT}/`;
  server = spawn(process.execPath, [join(ROOT, 'node_modules/vite/bin/vite.js'), 'preview', '--host', '127.0.0.1', '--port', String(PORT), '--strictPort'], {
    cwd: ROOT,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  server.stdout.on('data', () => {});
  server.stderr.on('data', (d) => process.stderr.write(`[preview] ${d}`));
  try {
    await waitHttp(url, 30_000);
  } catch (e) {
    server.kill('SIGTERM');
    throw e;
  }
  console.log(`preview 서버 PID ${server.pid} ${url}`);
}

const report = { url, passes: {} };
let failed = false;

async function runPass(name) {
  const launch = LAUNCH[name];
  if (!launch) throw new Error(`모르는 pass: ${name}`);
  const browser = await chromium.launch(launch);
  const context = await browser.newContext({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  const messages = [];
  const problems = [];
  page.on('console', (m) => {
    if (m.type() === 'error' || m.type() === 'warning') messages.push(`${m.type()}: ${m.text().slice(0, 400)}`);
  });
  page.on('pageerror', (e) => messages.push(`pageerror: ${e.message.slice(0, 400)}`));
  page.on('requestfailed', (r) => messages.push(`requestfailed: ${r.url()} ${r.failure()?.errorText ?? ''}`));
  page.on('response', (r) => {
    if (r.status() >= 400) messages.push(`http ${r.status()}: ${r.url()}`);
  });
  const shots = [];
  const shoot = async (label) => {
    const path = join(OUT, `${name}-${label}.png`);
    await page.screenshot({ path, timeout: 60_000 });
    shots.push(path);
    console.log(`  찍음 ${path}`);
  };
  const step = async (label, fn) => {
    const t0 = Date.now();
    try {
      await fn();
      console.log(`  ok   ${label} (${Date.now() - t0} ms)`);
    } catch (e) {
      problems.push(`${label}: ${String(e.message ?? e).split('\n')[0]}`);
      console.log(`  FAIL ${label}: ${String(e.message ?? e).split('\n')[0]}`);
    }
  };
  const state = () => page.evaluate(() => window.__golf.state());
  const info = {};
  console.log(`[${name}] ${browser.version()}`);

  // ---------------------------------------------------------------- 코스 (기본 모드: 메뉴 → 게임)
  const game = () =>
    page.evaluate(() => {
      const s = window.__golf.game.state();
      const players = s.players.map((p, i) => (i === s.cur ? { ...p, ball: s.ball, strokes: s.strokes, scores: s.scores } : p));
      return { phase: s.phase, mode: s.mode, hole: s.holeIdx + 1, cur: s.cur, strokes: s.strokes, surface: s.ball.surface, x: s.ball.x, z: s.ball.z, players, msg: s.message?.text ?? null, record: s.record };
    });
  const playOut = async (limit) => {
    for (let i = 0; i < limit; i++) {
      if ((await game()).phase === 'done') break;
      await page.evaluate(() => {
        window.__golf.game.auto();
        window.__golf.game.skip();
      });
      await page.waitForTimeout(60);
    }
  };

  await step('코스: 첫 화면 (메뉴)', async () => {
    const glb = page.waitForResponse((r) => r.url().includes('trainer_green.glb'), { timeout: 60_000 });
    await page.goto(url);
    await page.getByTestId('g-menu').waitFor({ timeout: 60_000 });
    await page.waitForFunction(() => window.__golf?.game && window.__golf.state().renderer !== '…', undefined, { timeout: 60_000 });
    info.renderer = (await state()).renderer;
    info.glb = (await glb).status();
    await page.waitForTimeout(3000);
    await shoot('g0-menu');
  });

  await step('코스: 2명·9홀 시작 → 홀 소개 → Space 로 건너뛰기', async () => {
    await page.getByRole('button', { name: '2명' }).click();
    await page.getByRole('button', { name: '9홀' }).click();
    await page.getByTestId('g-start').click();
    await page.waitForFunction(() => window.__golf.game.state().phase === 'intro', undefined, { timeout: 5000 });
    await page.waitForTimeout(1600);
    await shoot('g1-intro');
    await page.keyboard.press('Space');
    await page.waitForFunction(() => window.__golf.game.state().phase === 'aim', undefined, { timeout: 5000 });
    const g = await game();
    if (g.hole !== 1 || g.players.length !== 2 || g.surface !== 'tee') throw new Error(`첫 상태 ${JSON.stringify(g)}`);
    await page.waitForTimeout(1200);
    await shoot('g2-tee');
  });

  await step('코스: Space 세 번으로 티샷 (스윙 미터·판정)', async () => {
    await page.keyboard.press('Space'); // 시작
    await page.waitForTimeout(900); // 파워 ≈ 0.9
    await page.keyboard.press('Space'); // 파워 정함
    await page.waitForTimeout(700); // 표시가 0 근처로 돌아올 때
    await page.keyboard.press('Space'); // 정확도
    await page.waitForFunction(() => window.__golf.game.state().phase === 'flight', undefined, { timeout: 5000 });
    await page.getByTestId('g-rating').waitFor({ timeout: 2000 });
    if ((await game()).strokes !== 1) throw new Error('타수가 1 이 아님');
    await page.waitForFunction(() => window.__golf.game.clock() >= 2.5, undefined, { timeout: 30_000, polling: 50 });
    await shoot('g3-flight');
  });

  await step('코스: F 로 빨리 감기 → 다음 사람 차례', async () => {
    await page.keyboard.press('KeyF');
    await page.waitForFunction(() => window.__golf.game.state().phase === 'aim', undefined, { timeout: 10_000 });
    const g = await game();
    info.teeShot = g.players[0].ball;
    if (g.cur !== 1 || !String(g.msg).includes('차례')) throw new Error(`차례가 넘어가지 않음 ${JSON.stringify(g)}`);
    if (Math.hypot(g.players[0].ball.x, g.players[0].ball.z) < 150) throw new Error('티샷이 짧음');
    await page.waitForTimeout(1500);
    await shoot('g4-next-player');
  });

  await step('코스: V 홀 전체 보기', async () => {
    await page.keyboard.press('KeyV');
    await page.waitForTimeout(2500);
    await shoot('g5-overview');
    await page.keyboard.press('KeyV');
  });

  await step('코스: 봇이 9홀을 끝까지 (2명, 홀마다 더블 파 이하)', async () => {
    await playOut(600);
    await page.waitForTimeout(2500);
    const g = await game();
    info.round = g.players.map((p) => p.scores);
    if (g.phase !== 'done') throw new Error(`라운드가 끝나지 않음 ${JSON.stringify(g.phase)}`);
    const pars = [4, 3, 5, 3, 4, 4, 3, 4, 5];
    for (const p of g.players) {
      p.scores.forEach((v, i) => {
        if (v === null || v > 2 * pars[i]) throw new Error(`${p.name} ${i + 1}번 홀 점수 ${v}`);
      });
    }
    await page.getByTestId('g-card').waitFor({ timeout: 5000 });
    await shoot('g6-round-done');
  });

  await step('코스: 니어핀 챌린지 (섬 그린, 봇)', async () => {
    await page.evaluate(() => window.__golf.game.start({ mode: 'ctp', players: ['가', '나'], intro: false }));
    await page.waitForTimeout(1500);
    await shoot('g7-ctp');
    await playOut(30);
    await page.waitForTimeout(1500);
    const g = await game();
    info.ctp = g.players.map((p) => p.shots);
    if (g.phase !== 'done' || g.players.some((p) => p.shots.length !== 3)) throw new Error(`니어핀 ${JSON.stringify(info.ctp)}`);
    await shoot('g8-ctp-result');
  });

  await step('코스: 롱드라이브 (봇)', async () => {
    await page.evaluate(() => window.__golf.game.start({ mode: 'longdrive', players: ['가'], intro: false }));
    await page.waitForTimeout(1500);
    await playOut(30);
    await page.waitForTimeout(1500);
    const g = await game();
    info.longdrive = g.players.map((p) => p.shots);
    if (g.phase !== 'done' || g.players[0].shots.filter((v) => Number.isFinite(v)).length === 0) throw new Error(`롱드라이브 ${JSON.stringify(info.longdrive)}`);
    await shoot('g9-longdrive-result');
  });

  // ---------------------------------------------------------------- 연습장 (?mode=range)
  await step('첫 화면: 캔버스·렌더러·캐릭터 모델', async () => {
    const glb = page.waitForResponse((r) => r.url().includes('trainer_green.glb'), { timeout: 60_000 });
    await page.goto(`${url}?mode=range`);
    await page.locator('canvas').first().waitFor({ timeout: 60_000 });
    await page.waitForFunction(() => window.__golf && window.__golf.state().renderer !== '…', undefined, { timeout: 60_000 });
    info.renderer = (await state()).renderer;
    info.glb = (await glb).status();
    await page.waitForTimeout(3500); // 모델·그림자·첫 프레임들
    info.playerAtLoad = await page.evaluate(() => window.__golf.player());
    await shoot('1-initial');
  });

  await step('PGA 7번 → 샷 → 비행 중', async () => {
    await page.getByRole('button', { name: 'PGA 7번', exact: true }).click();
    await page.getByRole('button', { name: /샷 치기/ }).click();
    await page.waitForFunction(() => window.__golf.state().phase === 'flying', undefined, { timeout: 10_000 });
    await page.waitForFunction(
      () => {
        const s = window.__golf.state();
        return s.shot && window.__golf.clock() >= 0.45 * s.shot.result.timeS;
      },
      undefined,
      { timeout: 60_000, polling: 50 },
    );
    info.midFlightClock = await page.evaluate(() => window.__golf.clock());
    await shoot('2-midflight');
  });

  await step('착지 → 착지 보기', async () => {
    await page.waitForFunction(() => window.__golf.state().phase === 'landed', undefined, { timeout: 90_000 });
    await page.waitForTimeout(2200);
    const s = await state();
    info.pga7 = { carryYd: s.shot.result.carryYd, apexYd: s.shot.result.apexYd, landDeg: s.shot.result.landDeg, timeS: s.shot.result.timeS };
    info.carryText = (await page.getByTestId('carry').innerText()).replace(/\s+/g, ' ');
    // 참값은 test/web_refs.csv (flightbench webrefs 가 뽑은 기본 장)에서 읽는다: 장을 바꿔도 여기는 그대로
    const ref = Number(readFileSync(join(ROOT, 'test/web_refs.csv'), 'utf8').split('\n').find((l) => l.startsWith('120,16.3,7097,0,'))?.split(',')[4]);
    if (!(Math.abs(s.shot.result.carryYd - ref) <= 0.05)) throw new Error(`PGA 7번 캐리 ${s.shot.result.carryYd} (참값 ${ref})`);
    await shoot('3-landing');
  });

  await step('옆 시점', async () => {
    await page.getByRole('button', { name: '옆', exact: true }).click();
    await page.waitForTimeout(2500);
    await shoot('4-side');
  });

  await step('위 시점', async () => {
    await page.getByRole('button', { name: '위', exact: true }).click();
    await page.waitForTimeout(2500);
    await shoot('5-top');
  });

  await step('screen 출력 붙여넣기 → 드로·페이드 부호 → 샷', async () => {
    await page.getByRole('button', { name: '자동', exact: true }).click();
    await page.locator('details.paste summary').click();
    await page.getByLabel('screen --csv 출력').fill('볼스피드 120.0 mph, 발사각 16.30°, 방위각 0.01°\n스핀 7061 rpm, 스핀축 8.0°, 캐리 168.2 yd');
    await page.getByRole('button', { name: '입력 채우기' }).click();
    const s0 = await state();
    if (s0.input.mph !== 120 || s0.input.spinAxisDeg !== -8 || s0.input.rpm !== 7061) throw new Error(`붙여넣기 값 ${JSON.stringify(s0.input)}`);
    info.pasteMessage = (await page.locator('.paste-msg').innerText()).replace(/\s+/g, ' ');
    await page.getByRole('button', { name: /샷 치기/ }).click();
    await page.waitForFunction(() => window.__golf.state().phase === 'landed', undefined, { timeout: 90_000 });
    await page.waitForTimeout(2200);
    const s = await state();
    info.paste = { carryYd: s.shot.result.carryYd, lateralYd: s.shot.result.lateralYd };
    // screen 축 +8° (엔진 부호) = 트랙맨 −8° = 왼쪽으로 휨
    if (!(s.shot.result.lateralYd < 0)) throw new Error(`드로인데 좌우 ${s.shot.result.lateralYd}`);
    await shoot('6-paste-draw-landing');
  });

  await step('캐릭터 시점에서 걷기 (W 1.2 s)', async () => {
    const before = await page.evaluate(() => window.__golf.player());
    await page.getByRole('button', { name: '캐릭터', exact: true }).click();
    await page.waitForTimeout(1500);
    await page.locator('canvas').first().click({ position: { x: 800, y: 300 } });
    await page.keyboard.down('KeyW');
    await page.waitForTimeout(1200);
    await page.keyboard.up('KeyW');
    await page.waitForTimeout(1200);
    const after = await page.evaluate(() => window.__golf.player());
    info.walk = { before, after };
    if (!(after.y > -0.5 && after.y < 3)) throw new Error(`캐릭터 높이 ${after.y} (땅을 뚫었거나 떠 있음)`);
    if (!(Math.hypot(after.x - before.x, after.z - before.z) > 1)) throw new Error('캐릭터가 걷지 않음');
    await shoot('7-player-walk');
  });

  await step('Enter 키로 샷 (캐릭터 시점 → 자동 시점)', async () => {
    const n = (await state()).shots;
    await page.keyboard.press('Enter');
    await page.waitForFunction((k) => window.__golf.state().shots === k + 1, n, { timeout: 5000 });
    if ((await state()).cameraMode !== 'auto') throw new Error('샷 뒤 카메라가 자동으로 돌아가지 않음');
  });

  await browser.close();
  report.passes[name] = { info, problems, messages, shots };
  if (problems.length > 0 || messages.some((m) => m.startsWith('pageerror') || m.startsWith('error'))) failed = true;
}

try {
  for (const p of PASSES) await runPass(p);
} finally {
  if (server) {
    server.kill('SIGTERM');
    console.log(`preview 서버 PID ${server.pid} 끔`);
  }
}

writeFileSync(join(OUT, 'smoke-report.json'), JSON.stringify(report, null, 2));
for (const [name, p] of Object.entries(report.passes)) {
  console.log(`\n[${name}] 렌더러 ${p.info.renderer}, 문제 ${p.problems.length}, 콘솔 오류·경고 ${p.messages.length}`);
  for (const m of p.messages) console.log(`  ${m}`);
  for (const m of p.problems) console.log(`  문제: ${m}`);
}
console.log(`\n보고서 ${join(OUT, 'smoke-report.json')}`);
process.exit(failed ? 1 : 0);
