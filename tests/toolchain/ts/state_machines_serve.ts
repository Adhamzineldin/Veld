// The generated test handler of tests/rules/endpoints/transitions.veld (spec/rules.md sections
// 10 and 12.4), served over node:http with a stub application. Prints `PORT=<n>` and serves until
// stdin closes. The Rust test sends every example to it. Expects `./server` generated with
// `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createBookingsTestHandler } from './server/server/bookings-test-handler';

// The application's handlers must never run: every example stops after step 6 (a dry run) or hands
// the test handler its result.
const never = (name: string) => async (): Promise<never> => {
  throw new Error(`the application handler of ${name} must not be reached`);
};
const impl: srv.BookingsService = {
  payBooking: never('payBooking'),
  cancelBooking: never('cancelBooking'),
  checkIn: never('checkIn'),
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
