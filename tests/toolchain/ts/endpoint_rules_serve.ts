// The generated test handler of a service with endpoint rules (spec/rules.md section 12.4), served over
// node:http with a stub application. Prints `PORT=<n>` and serves until stdin closes. The Rust test
// sends every example of tests/rules/endpoints to it. Expects `./server` generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createBookingsTestHandler } from './server/server/bookings-test-handler';

// The application's handlers must never run: every example stops after step 6 (a dry run) or hands
// the test handler its result.
const impl: srv.BookingsService = {
  async createBooking() {
    throw new Error('the application handler of createBooking must not be reached');
  },
  async payBooking() {
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
