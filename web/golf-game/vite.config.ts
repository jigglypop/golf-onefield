import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// wasm 글루(src/physics/wasm/golf_core.js)는 new URL('golf_core_bg.wasm', import.meta.url) 로 .wasm 을 찾는다.
// Vite 가 이 패턴을 그대로 자산으로 내보내므로 별도 플러그인이 필요 없다.
export default defineConfig({
  plugins: [react()],
});
