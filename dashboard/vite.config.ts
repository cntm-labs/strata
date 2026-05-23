import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      // Workaround for cntm-labs/nucleus#102 — the SDK's package.json
      // `exports` map points at dist/esm/ and dist/cjs/ paths that don't
      // exist in the published tarball (the actual files are dist/index.js
      // and dist/index.cjs). Remove this alias once the SDK is republished.
      '@cntm-labs/nucleus-js': fileURLToPath(
        new URL('./node_modules/@cntm-labs/nucleus-js/dist/index.js', import.meta.url),
      ),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
    },
  },
})
