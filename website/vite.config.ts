import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react-swc'

// The worker registers after WASM has loaded. Precache the built files on installation so
// the first visit also works offline, including assets requested by the WASM worker itself.
function offlinePlayground(): Plugin {
  return {
    name: 'veld-offline-playground',
    apply: 'build',
    generateBundle(_, bundle) {
      const paths = ['/index.html', '/veld-icon.svg', ...Object.keys(bundle).filter((p) => p.startsWith('assets/')).sort().map((p) => '/' + p)]
      const version = createHash('sha256').update(JSON.stringify(paths)).digest('hex').slice(0, 16)
      const worker = readFileSync(new URL('./scripts/playground-sw.js', import.meta.url), 'utf8')
      this.emitFile({
        type: 'asset',
        fileName: 'sw.js',
        source: `const CACHE = ${JSON.stringify('veld-site-' + version)};\nconst ASSETS = ${JSON.stringify(paths)};\n${worker}`,
      })
    },
  }
}

export default defineConfig({
  plugins: [react(), offlinePlayground()],
  // The WebAssembly engine runs in a module worker (src/playground/engine.worker.ts).
  worker: { format: 'es' },
  server: {
    port: 2501,
    host: '0.0.0.0',
    allowedHosts: ['app2501.maayn.com'],
  },
})
