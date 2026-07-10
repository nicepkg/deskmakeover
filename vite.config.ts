import path from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// Tauri sets TAURI_DEV_HOST when debugging on a physical device (mobile, later).
const host = process.env.TAURI_DEV_HOST

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  // Keep Rust compile errors visible when launched by `tauri dev`.
  clearScreen: false,
  server: {
    // Must match `build.devUrl` in ../src-tauri/tauri.conf.json.
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    // src-tauri churn shouldn't retrigger the web HMR.
    watch: { ignored: ['**/src-tauri/**'] },
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
