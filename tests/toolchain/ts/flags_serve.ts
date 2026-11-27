// The generated test handler of tests/rules/endpoints/flags.veld.pending (spec/platform.md section 11; spec/rules.md
// section 12.4), served over node:http with a stub application. Prints `PORT=<n>` and
// serves until stdin closes. The Rust test sends every example to it. Expects `./server`
// generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createRoomsTestHandler } from './server/server/rooms-test-handler';

// The application's handler must never run: every example stops after step 6 (a dry run) or hands
// the test handler its result.
const impl: srv.RoomsService = {
  async bookRoom(): Promise<never> {
    throw new Error('the application handler of bookRoom must not be reached');
  },
  async publishRoom(): Promise<never> {
    throw new Error('the application handler of publishRoom must not be reached');
  },
  async archiveRoom(): Promise<never> {
    throw new Error('the application handler of archiveRoom must not be reached');
  },
};

const server = http.createServer(srv.toNodeHttp(createRoomsTestHandler(impl)));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
