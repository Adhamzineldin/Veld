// The generated test handler of one service that subscribes (spec/events.md section 10.9), served
// over node:http. `node out/events_serve.js <module> <factory>`, for example
// `./server/server/ledger-test-handler createLedgerTestHandler`. Prints `PORT=<n>` and serves until
// stdin closes. The Rust test sends it every event example through the shared live runner.
// Expects `./server` generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import type { VeldHandler } from './server/runtime/veld';

const [file, factory] = process.argv.slice(2);
if (file === undefined || factory === undefined) throw new Error('usage: events_serve <module> <factory>');

// The module is generated: its factory takes nothing for a service that has no endpoint rules.
// eslint-disable-next-line @typescript-eslint/no-require-imports
const loaded = require(file) as Record<string, () => VeldHandler>;
const create = loaded[factory];
if (create === undefined) throw new Error(`${file} has no ${factory}`);

const server = http.createServer(srv.toNodeHttp(create()));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
