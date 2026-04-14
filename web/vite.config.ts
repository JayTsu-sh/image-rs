import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// In production the bundle is served by axum's ServeDir at /ui/.
// In dev (`npm run dev`) we run on :5173 and proxy API calls to :8080.
export default defineConfig({
  plugins: [vue()],
  base: '/ui/',
  server: {
    port: 5173,
    proxy: {
      '/v1': 'http://127.0.0.1:8080',
      '/healthz': 'http://127.0.0.1:8080',
      '/readyz': 'http://127.0.0.1:8080',
      '/metrics': 'http://127.0.0.1:8080',
    },
  },
  build: {
    outDir: 'dist',
    target: 'es2022',
    sourcemap: false,
  },
});
