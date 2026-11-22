import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// In development the API runs on :3000 (see ../server); the proxy keeps requests same-origin.
export default defineConfig({
  plugins: [react()],
  server: { proxy: { '/hello': 'http://127.0.0.1:3000' } },
});
