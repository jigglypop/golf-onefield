/**
 * gaesup-world npm 패키지에 들어 있는 파일 중 이 앱이 쓰는 것만 서빙·배포한다 (저장소에 바이너리를 넣지 않는다).
 *   /gltf/trainer_green.glb : 캐릭터 (패키지 public/gltf, idle·walk·run 클립)
 *   /wasm/gaesup_core.wasm  : 엔진 코어 WASM (없으면 JS 경로로 돌지만 요청이 404 로 남는다)
 * dev 는 미들웨어로 바로 보내고, build 는 dist 에 같은 경로로 넣는다 (preview 는 dist 를 그대로 서빙).
 */
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import type { Plugin } from 'vite';

const FILES: Record<string, string> = {
  'gltf/trainer_green.glb': 'public/gltf/trainer_green.glb',
  'wasm/gaesup_core.wasm': 'dist/wasm/gaesup_core.wasm',
};
const TYPES: Record<string, string> = { glb: 'model/gltf-binary', wasm: 'application/wasm' };

function packageRoot(): string {
  return resolve(dirname(fileURLToPath(import.meta.resolve('gaesup-world'))), '..');
}

export function gaesupAssets(): Plugin {
  const root = packageRoot();
  const source = (published: string) => join(root, FILES[published]!);
  return {
    name: 'golf:gaesup-assets',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const path = decodeURIComponent((request.url ?? '').split('?')[0] ?? '').replace(/^\/+/, '');
        if (!(path in FILES)) return next();
        response.setHeader('Content-Type', TYPES[path.split('.').pop() ?? ''] ?? 'application/octet-stream');
        response.end(readFileSync(source(path)));
      });
    },
    generateBundle() {
      for (const fileName of Object.keys(FILES)) {
        this.emitFile({ type: 'asset', fileName, source: readFileSync(source(fileName)) });
      }
    },
  };
}
