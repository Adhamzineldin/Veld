// The generated test handler of tests/rules/endpoints/tenancy.veld.pending (spec/platform.md
// section 3; spec/rules.md section 12.4), served over node:http with a stub application. Prints
// `PORT=<n>` and serves until stdin closes. The Rust test sends every example to it. Expects
// `./server` generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createBookingsTestHandler } from './server/server/bookings-test-handler';

// The application's handler must never run: every example hands the test handler its result or
// stops before the handler.
const never = (name: string) => async (): Promise<never> => {
  throw new Error(`the application handler of ${name} must not be reached`);
};
const impl: srv.BookingsService = {
  getBooking: never('getBooking'),
  seen: never('seen'),
  listBookings: never('listBookings'),
  pageBookings: never('pageBookings'),
  me: never('me'),
  copyBooking: never('copyBooking'),
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
