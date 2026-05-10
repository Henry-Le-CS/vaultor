import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri's dev host (set when running inside Tauri)
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],

  // Prevent Vite from clearing the terminal
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: 'ws', host, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust source — Tauri handles that separately
      ignored: ['**/src-tauri/**'],
    },
  },

  // Expose VITE_ and TAURI_ env vars to the frontend
  envPrefix: ['VITE_', 'TAURI_'],

  build: {
    // Tauri supports es2021 on macOS
    target: 'es2021',
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
