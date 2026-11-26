// The generated test handlers of tests/events/outbox.veld.pending (spec/events.md sections 10.9 and
// 11.2), served over node:http at one address: `Bookings` (the moves, with `Veld-Test-Emitted`) and
// `Notifications` (the event endpoint). Prints `PORT=<n>` and serves until stdin closes. The Rust
// test sends every example to it through the shared live runner. The application's handlers must
// never run: every call example hands the test handler its result or is refused by the guard.
// Expects `./server` generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createBookingsTestHandler } from './server/server/bookings-test-handler';
import { createNotificationsTestHandler } from './server/server/notifications-test-handler';
import type { VeldHandler, VeldRequest, VeldResponse } from './server/runtime/veld';

const never = async (): Promise<never> => {
  throw new Error('the application handler must not be reached');
};
const bookings = createBookingsTestHandler({ pay: never, cancel: never, refund: never } as unknown as srv.BookingsService);
const notifications = createNotificationsTestHandler();

type Json = Record<string, unknown>;

async function probe(req: VeldRequest): Promise<VeldResponse> {
  const digests: Json = {};
  const subscribes: Json = {};
  for (const handler of [bookings, notifications]) {
    const res = await handler(req);
    const body = JSON.parse(res?.body ?? '{}') as Json;
    Object.assign(digests, body['digests']);
    Object.assign(subscribes, body['subscribes']);
  }
  return {
    status: 200,
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ veldTestHandler: 1, digests, subscribes }),
  };
}

const combined: VeldHandler = Object.assign(
  async (req: VeldRequest): Promise<VeldResponse | null> => {
    const method = req.method.toUpperCase();
    if (method === 'GET' && req.path === '/__veld/test') return probe(req);
    if (method === 'POST' && req.path === '/__veld/test/event') return notifications(req);
    return bookings(req);
  },
  { ws: () => Promise.resolve(false) },
);

const server = http.createServer(srv.toNodeHttp(combined));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
