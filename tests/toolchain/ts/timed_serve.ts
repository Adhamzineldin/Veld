// The generated test handler of tests/rules/endpoints/timed.veld.pending (spec/platform.md section 10; spec/rules.md
// section 12.4), served over node:http with a stub application. Prints `PORT=<n>` and
// serves until stdin closes. The Rust test sends every example to it. Expects `./server`
// generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createBookingsTestHandler } from './server/server/bookings-test-handler';

// The application's handler must never run: every example stops after step 6 (a dry run) or hands
// the test handler its result.
const impl: srv.BookingsService = {
  async expireBooking(): Promise<never> {
    throw new Error('the application handler of expireBooking must not be reached');
  },
  async archiveBooking(): Promise<never> {
    throw new Error('the application handler of archiveBooking must not be reached');
  },
  async payBooking(): Promise<never> {
    throw new Error('the application handler of payBooking must not be reached');
  },
};

const server = http.createServer(srv.toNodeHttp(createBookingsTestHandler(impl)));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
