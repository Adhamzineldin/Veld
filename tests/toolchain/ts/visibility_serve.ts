// The generated test handlers of tests/rules/endpoints/visibility.veld.pending (spec/platform.md
// section 2; spec/rules.md section 12.4), served over node:http with a stub application: the
// handlers of `Bookings` and `Notices` side by side, because the contract has both. Prints
// `PORT=<n>` and serves until stdin closes. The Rust test sends every example to it. Expects
// `./server` generated with `testing = true` for both services.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import type { VeldHandler, VeldRequest, VeldResponse } from './server/runtime/veld';
import { createBookingsTestHandler } from './server/server/bookings-test-handler';
import { createNoticesTestHandler } from './server/server/notices-test-handler';

// The application's handlers must never run: every example hands the test handler its result.
const never = (name: string) => async (): Promise<never> => {
  throw new Error(`the application handler of ${name} must not be reached`);
};
const bookings = createBookingsTestHandler({
  getBooking: never('getBooking'),
  listBookings: never('listBookings'),
  pageBookings: never('pageBookings'),
  checkedBooking: never('checkedBooking'),
  cancelBooking: never('cancelBooking'),
});
const notices = createNoticesTestHandler({ publicBooking: never('publicBooking') });

function isProbe(req: VeldRequest): boolean {
  return req.method.toUpperCase() === 'GET' && req.path === '/__veld/test';
}

/** One test handler for both services: the probe lists both digests, a request goes to the one that has its route. */
async function both(req: VeldRequest): Promise<VeldResponse | null> {
  if (isProbe(req)) {
    const digests: Record<string, string | null> = {};
    for (const handler of [bookings, notices]) {
      const answer = await handler(req);
      const doc = JSON.parse(answer?.body ?? '{}') as { digests?: Record<string, string | null> };
      Object.assign(digests, doc.digests);
    }
    const body = JSON.stringify({ veldTestHandler: 1, digests });
    return { status: 200, headers: { 'content-type': 'application/json' }, body };
  }
  return (await bookings(req)) ?? (await notices(req));
}

const handler: VeldHandler = Object.assign(both, { ws: bookings.ws });
const server = http.createServer(srv.toNodeHttp(handler));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
