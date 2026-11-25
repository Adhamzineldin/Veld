// The generated test handler of the audited-rules contract (spec/rules.md sections 12.4 and 17.1), served
// over node:http with a stub application. Prints `PORT=<n>` and serves until stdin closes. The Rust test
// sends every example of tests/rules/endpoints/rollout.veld to it; `veld test --live` compares the
// `Veld-Test-Audited` header of each answer with what the reference evaluator reports.
// Expects `./server` generated with `testing = true`.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import { createPartiesTestHandler } from './server/server/parties-test-handler';

// The application's handlers must never run: every example stops after step 6 (a dry run) or hands the
// test handler its result.
function never(name: string): () => Promise<never> {
  return () => Promise.reject(new Error(`the application handler of ${name} must not be reached`));
}

const impl: srv.PartiesService = {
  watched: never('watched'),
  enforced: never('enforced'),
  mixed: never('mixed'),
  booking: never('booking'),
  receipt: never('receipt'),
};

const server = http.createServer(srv.toNodeHttp(createPartiesTestHandler(impl)));
server.listen(0, '127.0.0.1', () => {
  console.log(`PORT=${String((server.address() as AddressInfo).port)}`);
});
process.stdin.resume();
process.stdin.on('end', () => {
  server.close();
  process.exit(0);
});
