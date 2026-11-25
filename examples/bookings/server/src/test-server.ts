// THE TEST SERVER. NEVER DEPLOY IT, AND NEVER MOUNT THE TEST HANDLER IN PRODUCTION.
//
// The generated test handler lets whoever sends a request choose the facts, the principal and
// the clock of that request (`Veld-Test-*` headers). That switches rules off in practice. It
// exists so `veld test --live` can run the `example` blocks of the contract against the real
// handler code in `service.ts`. Start it on a developer machine or in CI only.
import express from 'express';
import { toExpress } from '@veld/api';
import { createBookingsTestHandler } from '@veld/api/server/bookings-test-handler';
import { createService, createStore } from './service';

const app = express();
app.use(toExpress(createBookingsTestHandler(createService(createStore()))));

const port = Number(process.env.PORT ?? 3001);
app.listen(port, '127.0.0.1', () => {
  console.log(`bookings TEST server listening on http://127.0.0.1:${port} (never for production)`);
});
