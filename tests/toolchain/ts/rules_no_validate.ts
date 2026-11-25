// Request validation turned off (spec/rules.md section 5.3): a server and a client generated with
// `validate = false` do not enforce rules on what callers send, and the server says so once, when the
// handler is created. Expects `./server` (node) and `./client` (typescript) generated from
// tests/toolchain/contract/rules with `validate = false`.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { VeldHandler } from './server/runtime/veld';
import type * as rt from './client/runtime/veld';

const BAD = { start: '2026-01-02T00:00:00Z', end: '2026-01-01T00:00:00Z', guests: 2 };
const GOOD = { start: '2026-01-01T00:00:00Z', end: '2026-01-02T00:00:00Z', guests: 2 };

const accepted: unknown[] = [];
const impl: srv.BookingsService = {
  async create(req) {
    accepted.push(req.body);
    return GOOD;
  },
  async search(req) {
    accepted.push(req.query);
    return 1;
  },
  async report() {
    return { parts: [1, 2], total: 3 };
  },
};

function listen(handler: VeldHandler): Promise<{ url: string; close(): void }> {
  const server = http.createServer(srv.toNodeHttp(handler));
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const port = (server.address() as AddressInfo).port;
      resolve({ url: `http://127.0.0.1:${String(port)}`, close: () => server.close() });
    });
  });
}

function post(url: string): Promise<Response> {
  return fetch(`${url}/bookings/`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(BAD) });
}

async function main(): Promise<void> {
  const warnings: string[] = [];
  const warn = console.warn;
  console.warn = (m: unknown) => warnings.push(String(m));
  let off: Awaited<ReturnType<typeof listen>>;
  let on: Awaited<ReturnType<typeof listen>>;
  try {
    // Reported once, when the handler is created, and never per request.
    const handler = srv.createBookingsHandler(impl, {});
    assert.equal(warnings.length, 1, warnings.join('\n'));
    assert.match(warnings[0] ?? '', /request validation is off/);
    off = await listen(handler);
    // Switching validation back on in the hooks brings the rules back, and the report with it.
    on = await listen(srv.createBookingsHandler(impl, { validateRequests: true }));
    assert.equal(warnings.length, 1);
  } finally {
    console.warn = warn;
  }

  // A body that violates its rule is accepted: the rules are not enforced.
  const res = await post(off.url);
  assert.equal(res.status, 201);
  assert.deepEqual(accepted, [BAD]);
  const search = await fetch(`${off.url}/bookings/search?from=5&to=1`);
  assert.equal(search.status, 200);
  assert.equal(accepted.length, 2);
  assert.equal(warnings.length, 1, 'no report per request');

  // The same request is a 422 with violations where validation is on.
  const refused = await post(on.url);
  assert.equal(refused.status, 422);
  assert.equal(((await refused.json()) as { violations: unknown[] }).violations.length, 1);
  assert.equal(accepted.length, 2);

  // The client generated with `validate = false` does not check locally either.
  let sent = 0;
  const counting: rt.FetchLike = (url, init) => {
    sent += 1;
    return fetch(url, init as RequestInit) as unknown as ReturnType<rt.FetchLike>;
  };
  const client = new cli.BookingsClient({ baseUrl: off.url, fetch: counting });
  assert.equal((await client.create({ body: BAD })).guests, 2);
  assert.equal(sent, 1);
  assert.equal(accepted.length, 3);
  // Asking for validation brings the local check back.
  const strict = new cli.BookingsClient({ baseUrl: off.url, fetch: counting, validate: true });
  await assert.rejects(strict.create({ body: BAD }), (e: unknown) => {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.local, true);
    return true;
  });
  assert.equal(sent, 1);

  off.close();
  on.close();
  console.log('rules no validate ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
