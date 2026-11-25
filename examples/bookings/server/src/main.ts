// The bookings server: the generated handler mounted on Express.
import express from 'express';
import { createBookingsHandler, toExpress } from '@veld/api';
import { createLoaders, createService, createStore } from './service';

const store = createStore();

const app = express();
app.use(
  toExpress(
    createBookingsHandler(createService(store), {
      // How the handler finds the room a booking names.
      facts: createLoaders(store),
      // Say when a rule fired (rejected requests, and a result that broke an `ensure`).
      onRuleViolation: (event) => console.warn(`rule ${event.rule} ${event.outcome} (${event.position})`),
    }),
  ),
);

const port = Number(process.env.PORT ?? 3000);
app.listen(port, '127.0.0.1', () => {
  console.log(`bookings server listening on http://127.0.0.1:${port}`);
});
