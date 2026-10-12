// 시각 점검용 캡처: 개발 서버(또는 preview)를 헤드리스 Chromium 으로 열어 정해 둔 시점들을 찍는다.
//   node scripts/shots.mjs --url http://127.0.0.1:5190/ --q "quality=high" --pass gpu --only tee,green --out screens
// 시점은 window.__golf.vis.cam(pos, look, fov) 로 카메라를 직접 놓아 찍는다 (게임 카메라 로직은 건드리지 않는다).
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : fallback;
};
const URL_BASE = opt('--url', 'http://127.0.0.1:5190/');
const QUERY = opt('--q', '');
const OUT = resolve(opt('--out', join(ROOT, 'screens')));
const PASS = opt('--pass', 'gpu');
const ONLY = opt('--only', '').split(',').filter(Boolean);
const TAG = opt('--tag', '');
const W = Number(opt('--w', '1280'));
const H = Number(opt('--h', '720'));
const WAIT = Number(opt('--wait', '3500'));
mkdirSync(OUT, { recursive: true });

const LAUNCH = {
  gpu: { channel: 'chromium', headless: true, args: ['--enable-gpu', '--ignore-gpu-blocklist', '--use-angle=d3d11'] },
  swiftshader: { headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] },
  // 프레임 한도·수직 동기화를 꺼서 GPU 가 실제로 낼 수 있는 프레임 시간을 본다 (--pass perf)
  perf: { channel: 'chromium', headless: true, args: ['--enable-gpu', '--ignore-gpu-blocklist', '--use-angle=d3d11', '--disable-gpu-vsync', '--disable-frame-rate-limit'] },
};

// 각 시점: hole = 코스 안의 홀 위치(0 기준), ball = [x, z] (없으면 티), cam = [x,y,z, lx,ly,lz, fov?], phase = 'aim' 로 둔다
const SHOTS = {
  tee: { hole: 0, aim: true },
  fairway: { hole: 0, ball: [150, 4], aim: true },
  roughclose: { hole: 0, ball: [120, 24], cam: [117.6, 0.45, 23.2, 126, 0.05, 24.2], fov: 45 },
  roughaim: { hole: 0, ball: [120, 24], aim: true },
  fairway2: { hole: 5, ball: [120, 3], aim: true },
  approach: { hole: 0, ball: [300, 52], aim: true },
  green: { hole: 0, ball: [330, 56], cam: [326, 0.45, 53.5, 338, 0.0, 62], fov: 40 },
  puttcam: { hole: 0, ball: [334, 58], aim: true, club: 'putter' },
  bunker: { hole: 0, ball: [222, -12], aim: true },
  bunkerclose: { hole: 0, ball: [222, -12], cam: [219.2, 0.28, -10.6, 222, 0, -12], fov: 45 },
  lake: { hole: 1, ball: [0, 0], cam: [8, 1.8, -6, 70, 0, -4], fov: 55 },
  lake2: { hole: 1, ball: [8, 0], aim: true },
  lakecam: { hole: 1, ball: [8, 0], cam: [4, 3.2, -10, 80, 0, 0], fov: 55 },
  island: { hole: 3, ball: [0, 0], aim: true },
  ballclose: { hole: 0, ball: [150, 4], cam: [149.7, 0.1, 3.8, 150, 0.03, 4], fov: 30 },
  forest: { hole: 0, ball: [200, 6], cam: [190, 2.2, 6, 240, 4, 40], fov: 60 },
  // 숲 가장자리 가까이 (경계 z = −45 밖의 나무: 가까운 구간의 자세한 쪽(80 m 안)과 거친 쪽이 같이 보인다)
  treeline: { hole: 0, ball: [100, 0], cam: [100, 1.8, -38, 135, 7, -100], fov: 55 },
  treewide: { hole: 0, ball: [100, 0], cam: [60, 3, -30, 200, 9, -120], fov: 60 },
  overview: { hole: 0, ball: [0, 0], overview: true },
  intro: { hole: 4, ball: [0, 0], cam: [-30, 40, -20, 120, 0, 0], fov: 50 },
};

const browser = await chromium.launch(LAUNCH[PASS]);
const context = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
const page = await context.newPage();
page.on('console', (m) => {
  if (m.type() === 'error' || m.type() === 'warning') console.log(`  [${m.type()}] ${m.text().slice(0, 300)}`);
});
page.on('pageerror', (e) => console.log(`  [pageerror] ${e.message.slice(0, 400)}`));
await page.goto(`${URL_BASE}${URL_BASE.includes('?') ? '&' : '?'}${QUERY}`);
await page.getByTestId('g-menu').waitFor({ timeout: 90_000 });
const gl = await page.evaluate(() => {
  const c = document.createElement('canvas');
  const g = c.getContext('webgl2');
  const e = g?.getExtension('WEBGL_debug_renderer_info');
  return e ? g.getParameter(e.UNMASKED_RENDERER_WEBGL) : 'n/a';
});
console.log(`[${PASS}] ${browser.version()} GL=${gl}`);
await page.evaluate(() => window.__golf.game.start({ mode: 'stroke', holes: 9, players: ['가'], intro: false, seed: 7 }));
await page.waitForTimeout(500);

for (const [name, s] of Object.entries(SHOTS)) {
  if (ONLY.length > 0 && !ONLY.includes(name)) continue;
  await page.evaluate((s) => {
    const g = window.__golf.game;
    g.hole(s.hole);
    if (s.ball) g.place(s.ball[0], s.ball[1]);
    if (s.club === 'putter') g.club(99);
    if (s.overview) g.overview();
    window.__golf.vis?.cam(s.cam ? { pos: s.cam.slice(0, 3), look: s.cam.slice(3, 6), fov: s.fov } : null);
  }, s);
  await page.waitForTimeout(WAIT);
  const path = join(OUT, `${TAG}${name}.png`);
  await page.screenshot({ path, timeout: 120_000 });
  const stats = await page.evaluate(() => window.__golf.vis?.stats?.() ?? null);
  console.log(`  찍음 ${path}${stats ? ' ' + JSON.stringify(stats) : ''}`);
  await page.evaluate(() => {
    const g = window.__golf.game;
    if (g.state().overview) g.overview();
  });
}
await browser.close();
