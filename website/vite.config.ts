import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react-swc'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 2501,
    host: '0.0.0.0',
    allowedHosts: ['app2501.maayn.com'],
  },
})
