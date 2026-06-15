import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const octoviaPkg = JSON.parse(
  readFileSync(fileURLToPath(new URL('./src/octovia/package.json', import.meta.url)), 'utf-8'),
) as { version: string }

export default defineConfig({
  base: process.env.BASE_PATH ?? '/',
  define: {
    __OCTOVIA_VERSION__: JSON.stringify(octoviaPkg.version),
  },
  plugins: [
    tailwindcss(),
    svelte(),
    {
      name: 'wasm-mime',
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          if (req.url?.endsWith('.wasm')) {
            res.setHeader('Content-Type', 'application/wasm');
          }
          next();
        });
      },
    },
  ],
  optimizeDeps: {
    exclude: ['src/octovia/octovia.js'],
  },
  build: {
    target: 'esnext',
  },
})
