import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Tauri 期望固定的开发端口与较保守的构建目标（WebView2 / WKWebView）
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'baseline-widely-available',
    minify: 'esbuild',
    sourcemap: false,
  },
});
