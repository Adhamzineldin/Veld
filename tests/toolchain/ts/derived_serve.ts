// The generated test handler of tests/rules/endpoints/derived.veld.pending (spec/rules.md
// sections 12.4 and 17.2), served over node:http with a stub application. Prints `PORT=<n>` and
// serves until stdin closes. The Rust test sends every example to it. Expects `./server`
// generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createShopTestHandler } from './server/server/shop-test-handler';

// The application's handler must never run: every example stops after step 6 (a dry run) or hands
// the test handler its result.
const impl: srv.ShopService = {
  async pay(): Promise<never> {
    throw new Error('the application handler of pay must not be reached');
  },
};

const server = http.createServer(srv.toNodeHttp(createShopTestHandler(impl)));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
