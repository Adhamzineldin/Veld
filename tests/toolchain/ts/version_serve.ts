// The generated test handlers of tests/rules/endpoints/version.veld.pending (spec/rules.md sections
// 11.7, 11.9 and 12.4), served over node:http with a stub application: the handlers of `Rooms` and
// `Orders` side by side, because the contract has both. Prints `PORT=<n>` and serves until stdin
// closes. The Rust test sends every example to it. Expects `./server` generated with
// `testing = true` for both services.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import type { VeldHandler, VeldRequest, VeldResponse } from './server/runtime/veld';
import { createOrdersTestHandler } from './server/server/orders-test-handler';
import { createRoomsTestHandler } from './server/server/rooms-test-handler';

// The application's handlers must never run: every example stops after step 6 (a dry run) or hands
// the test handler its result.
const never = (name: string) => async (): Promise<never> => {
  throw new Error(`the application handler of ${name} must not be reached`);
};
const rooms = createRoomsTestHandler({
  rename: never('rename'),
  touch: never('touch'),
  tick: never('tick'),
  getRoom: never('getRoom'),
});
const orders = createOrdersTestHandler({ payOrder: never('payOrder') });

function isProbe(req: VeldRequest): boolean {
  return req.method.toUpperCase() === 'GET' && req.path === '/__veld/test';
}

/** One test handler for both services: the probe lists both digests, a request goes to the one that has its route. */
async function both(req: VeldRequest): Promise<VeldResponse | null> {
  if (isProbe(req)) {
    const digests: Record<string, string | null> = {};
    for (const handler of [rooms, orders]) {
      const answer = await handler(req);
      const doc = JSON.parse(answer?.body ?? '{}') as { digests?: Record<string, string | null> };
      Object.assign(digests, doc.digests);
    }
    const body = JSON.stringify({ veldTestHandler: 1, digests });
    return { status: 200, headers: { 'content-type': 'application/json' }, body };
  }
  return (await rooms(req)) ?? (await orders(req));
}

const handler: VeldHandler = Object.assign(both, { ws: rooms.ws });
const server = http.createServer(srv.toNodeHttp(handler));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
