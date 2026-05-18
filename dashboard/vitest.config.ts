import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    globals: true,
    root: fileURLToPath(new URL('./', import.meta.url)),
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,vue}'],
      exclude: ['src/main.ts', 'src/types/**'],
      thresholds: { statements: 100, branches: 100, functions: 100, lines: 100 },
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      // Workaround for cntm-labs/nucleus#102 — mirrors the alias in
      // vite.config.ts so tests can import the SDK.
      '@cntm-labs/nucleus-js': fileURLToPath(
        new URL('./node_modules/@cntm-labs/nucleus-js/dist/index.js', import.meta.url),
      ),
    },
  },
})
