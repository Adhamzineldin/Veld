import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// In development every service runs locally (see ../README.md); the proxy keeps the browser on
// one origin, like nginx does in docker compose.
const target = (port: number) => `http://127.0.0.1:${port}`;

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api/iam': target(3001),
      '/api/accounts': target(3002),
      '/api/transactions': target(3003),
      '/api/cards': target(3004),
      '/api/notifications': target(3005),
    },
  },
});
