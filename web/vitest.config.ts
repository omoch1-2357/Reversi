import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import wasm from 'vite-plugin-wasm'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [react(), wasm()],
  resolve: {
    alias: {
      './pkg/reversi': fileURLToPath(
        new URL('./src/test/mocks/reversi.ts', import.meta.url),
      ),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
})
