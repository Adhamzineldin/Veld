// The generated test handlers of the services that subscribe (spec/events.md section 10.9),
// mounted in one server over node:http: `node out/events_serve.js <module>#<factory> ...`, for
// example `./server/server/ledger-test-handler#createLedgerTestHandler`. Prints `PORT=<n>` and
// serves until stdin closes. The Rust test sends it every event example through the shared live
// runner. Expects `./server` generated with `testing = true`.
//
// A generated test handler answers for its own service, and two of them cannot share the path
// `/__veld/test/event`, so this program is the one server of a workspace: it hands each request
// to every handler and merges the answers the way the runner reads one handler that serves
// several services by the one rule of spec/events.md section 10.9: `outcome` is `rejected` when one
// of them rejected, else `handled` when one handled, else `ignored`; `reason` is that of the first
// rejecting service in name order; `called` is the union; `violations` are joined; `deadLetters`
// are added; a `400` of any of them (the first in name order) is the answer.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import type { VeldHandler, VeldRequest, VeldResponse } from './server/runtime/veld';

const names = process.argv.slice(2);
if (names.length === 0) throw new Error('usage: events_serve <module>#<factory> ...');

// Each module is generated: the factory of a service with no endpoint rules takes nothing.
// The services in name order: the rule of spec/events.md section 10.9 takes "the first rejecting
// service in name order", and the name is the one of the factory (`create<Service>TestHandler`).
const handlers: VeldHandler[] = names
  .map((name) => {
    const [file, factory] = name.split('#');
    if (file === undefined || factory === undefined) throw new Error(`not <module>#<factory>: ${name}`);
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const loaded = require(file) as Record<string, (() => VeldHandler) | undefined>;
    const create = loaded[factory];
    if (create === undefined) throw new Error(`${file} has no ${factory}`);
    return { service: factory.replace(/^create/, '').replace(/TestHandler$/, ''), handler: create() };
  })
  .sort((a, b) => (a.service < b.service ? -1 : a.service > b.service ? 1 : 0))
  .map((h) => h.handler);

type Json = Record<string, unknown>;

const json = (status: number, value: unknown): VeldResponse => ({
  status,
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify(value),
});

async function answers(req: VeldRequest): Promise<{ status: number; body: Json }[]> {
  const out: { status: number; body: Json }[] = [];
  for (const handler of handlers) {
    const res = await handler(req);
    if (res === null) throw new Error('a test handler did not answer');
    out.push({ status: res.status, body: JSON.parse(res.body ?? '{}') as Json });
  }
  return out;
}

async function probe(req: VeldRequest): Promise<VeldResponse> {
  const digests: Json = {};
  const subscribes: Json = {};
  for (const a of await answers(req)) {
    Object.assign(digests, a.body['digests']);
    Object.assign(subscribes, a.body['subscribes']);
  }
  return json(200, { veldTestHandler: 1, digests, subscribes });
}

async function event(req: VeldRequest): Promise<VeldResponse> {
  // The body of a request can be read once: every handler gets the same text.
  const text = req.text === undefined ? '' : await req.text();
  const all = await answers({ ...req, text: () => Promise.resolve(text) });
  const bad = all.find((a) => a.status !== 200);
  if (bad !== undefined) return json(bad.status, bad.body);
  const rejected = all.find((a) => a.body['outcome'] === 'rejected');
  const outcome = rejected !== undefined ? 'rejected' : all.some((a) => a.body['outcome'] === 'handled') ? 'handled' : 'ignored';
  const called: Record<string, number> = {};
  const violations: unknown[] = [];
  let deadLetters = 0;
  for (const a of all) {
    Object.assign(called, a.body['called']);
    violations.push(...((a.body['violations'] as unknown[] | undefined) ?? []));
    deadLetters += Number(a.body['deadLetters'] ?? 0);
  }
  return json(200, { outcome, reason: rejected?.body['reason'] ?? null, called, violations, deadLetters });
}

const combined: VeldHandler = Object.assign(
  async (req: VeldRequest): Promise<VeldResponse | null> => {
    const method = req.method.toUpperCase();
    if (method === 'GET' && req.path === '/__veld/test') return probe(req);
    if (method === 'POST' && req.path === '/__veld/test/event') return event(req);
    return null;
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
