// The generated test handler of the contract with an audited invariant of an error
// (tests/rules/endpoints/rollout_error.veld), served over node:http with a stub application. Prints
// `PORT=<n>` and serves until stdin closes. Expects `./server` generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createTablesTestHandler } from './server/server/tables-test-handler';

const impl: srv.TablesService = {
  book() {
    return Promise.reject(new Error('the application handler of book must not be reached'));
  },
};

const server = http.createServer(srv.toNodeHttp(createTablesTestHandler(impl)));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
