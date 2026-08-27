import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  base: '/token-monitor/',
  plugins: [react(), tailwindcss()],
  build: { sourcemap: false, minify: true },
});
