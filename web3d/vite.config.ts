/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

import { gaesupAssets } from './vite/gaesupAssets.ts';

export default defineConfig({
  plugins: [react(), gaesupAssets()],
  server: { host: '127.0.0.1', port: 5190 },
  preview: { host: '127.0.0.1', port: 4174 },
  build: {
    // three·rapier·gaesup-world 묶음이 크다 (rapier WASM 이 JS 안에 들어 있다)
    chunkSizeWarningLimit: 6000,
  },
  test: {
    environment: 'node',
    include: ['test/**/*.test.ts'],
    // 엔진 최대 오차·샷당 µs 를 늘 보이게 한다
    silent: false,
    reporters: ['verbose'],
  },
});
