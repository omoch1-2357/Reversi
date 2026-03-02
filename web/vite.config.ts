import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasm from 'vite-plugin-wasm'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), wasm()],
  base: '/Reversi/',
  build: {
    target: ['chrome80', 'edge80', 'firefox114', 'safari15'],
  },
  worker: {
    format: 'es',
  },
})
