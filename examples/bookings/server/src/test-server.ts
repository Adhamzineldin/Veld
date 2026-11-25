// THE TEST SERVER. NEVER DEPLOY IT, AND NEVER MOUNT THE TEST HANDLER IN PRODUCTION.
//
// The generated test handler lets whoever sends a request choose the facts, the principal and
// the clock of that request (`Veld-Test-*` headers). That switches rules off in practice. It
// exists so `veld test --live` can run the `example` blocks of the contract against the real
// handler code in `service.ts`. Start it on a developer machine or in CI only.
import express from 'express';
import { toExpress } from '@veld/api';
import type { Booking, BookingsService, veld } from '@veld/api';
import { createBookingsTestHandler } from '@veld/api/server/bookings-test-handler';
import { createService, createStore } from './service';

const store = createStore();
const service = createService(store);

// The test handler takes the facts from the request headers and never calls the loaders, so the
// booking an example describes is not in the store. The handlers that move a booking write
// conditionally into the store (see `move` in service.ts) and would find nothing. Before each of
// them runs, put the booking the example gave into the store, which is what the real loader
// would have found.
function seeded<Req extends { booking: Booking }, Res>(call: (req: Req, ctx: veld.RequestContext) => Promise<Res>) {
  return (req: Req, ctx: veld.RequestContext): Promise<Res> => {
    store.bookings.set(req.booking.id, req.booking);
    return call(req, ctx);
  };
}

const testService: BookingsService = {
  ...service,
  payBooking: seeded(service.payBooking),
  cancelBooking: seeded(service.cancelBooking),
  checkIn: seeded(service.checkIn),
  refundBooking: seeded(service.refundBooking),
};

const app = express();
app.use(toExpress(createBookingsTestHandler(testService)));

const port = Number(process.env.PORT ?? 3001);
app.listen(port, '127.0.0.1', () => {
  console.log(`bookings TEST server listening on http://127.0.0.1:${port} (never for production)`);
});
