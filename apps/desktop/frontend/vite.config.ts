import path from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  build: {
    // The bundle is served from a WebView2 virtual host on the local disk —
    // inline small assets, keep chunk names stable for cache-busting hashes.
    assetsInlineLimit: 8192,
    rollupOptions: {
      output: {
        // Vendor split: app-code edits stay tiny, framework bytes cache across builds.
        manualChunks: (id: string) => (id.includes('node_modules') ? 'vendor' : undefined),
      },
    },
  },
})
