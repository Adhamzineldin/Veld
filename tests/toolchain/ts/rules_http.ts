// Rules over real HTTP (spec/rules.md sections 5.3, 5.4 and 7): a generated Node server and a generated client.
// Expects `./server` (node, role server) and `./client` (typescript) generated from
// tests/toolchain/contract/rules.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';
import * as rt from './client/runtime/veld';

const events: RuleViolationEvent[] = [];
let served = 0;

const impl: srv.BookingsService = {
  async create(req) {
    served += 1;
    return req.body;
  },
  async search() {
    served += 1;
    return 1;
  },
  async report(req) {
    served += 1;
    return { parts: [1, 2], total: req.broken ? 5 : 3 };
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

const BAD = { start: '2026-01-02T00:00:00Z', end: '2026-01-01T00:00:00Z', guests: 2 };
const GOOD = { start: '2026-01-01T00:00:00Z', end: '2026-01-02T00:00:00Z', guests: 2 };

async function main(): Promise<void> {
  const strict = await listen(
    srv.createBookingsHandler(impl, { onRuleViolation: (e) => events.push(e) }),
  );
  const audit = await listen(
    srv.createBookingsHandler(impl, { rules: { output: 'audit' }, onRuleViolation: (e) => events.push(e) }),
  );

  // The 422 problem document has both data.issues and the top-level violations member.
  const raw = await fetch(`${strict.url}/bookings/`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(BAD),
  });
  assert.equal(raw.status, 422);
  const doc = (await raw.json()) as {
    title: string;
    data: { issues: { path: string; code: string; message: string }[] };
    violations: { rule: string; path: string; fields: string[]; message: string }[];
  };
  assert.equal(doc.title, 'ValidationError');
  assert.deepEqual(doc.data.issues, [{ path: 'body', code: 'rule', message: 'A booking must end after it starts' }]);
  assert.equal(doc.violations.length, 1);
  assert.deepEqual(doc.violations[0]?.fields, ['end', 'start']);
  assert.equal(doc.violations[0]?.path, 'body');
  assert.match(doc.violations[0]?.rule ?? '', /^veld:.*Booking\.endAfterStart@/);
  assert.equal(served, 0);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.position, 'input');
  assert.equal(events[0]?.outcome, 'rejected');

  // A client with the local check off receives and decodes the same violations.
  const remote = new cli.BookingsClient({ baseUrl: strict.url, rules: false });
  try {
    await remote.create({ body: BAD });
    assert.fail('expected a ValidationError');
  } catch (e) {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.local, false);
    assert.deepEqual(e.violations, doc.violations);
    assert.equal(e.data.issues[0]?.code, 'rule');
  }

  // With the local check on it fails locally with the same violations and sends nothing.
  let sent = 0;
  const counting: rt.FetchLike = (url, init) => {
    sent += 1;
    return fetch(url, init as RequestInit) as unknown as ReturnType<rt.FetchLike>;
  };
  const local = new cli.BookingsClient({ baseUrl: strict.url, fetch: counting });
  // The client trusts its checks only once a response carried its own rules digest (section 9.7).
  assert.equal(await local.search({ query: { from: 1, to: 5 } }), 1);
  assert.equal(sent, 1);
  sent = 0;
  try {
    await local.create({ body: BAD });
    assert.fail('expected a ValidationError');
  } catch (e) {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.local, true);
    assert.deepEqual(e.violations, doc.violations);
  }
  assert.equal(sent, 0);

  // A valid booking goes through; the query object is checked in both places.
  assert.equal((await local.create({ body: GOOD })).guests, 2);
  assert.equal(await local.search({ query: { from: 1, to: 5 } }), 1);
  await assert.rejects(local.search({ query: { from: 5, to: 1 } }), (e: unknown) => {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.violations[0]?.path, 'query');
    return true;
  });
  await assert.rejects(remote.search({ query: { from: 5, to: 1 } }), (e: unknown) => {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.local, false);
    return true;
  });

  // An output violation: 500 in strict mode with no detail, a hook event; sent unchanged in audit mode.
  events.length = 0;
  const strictClient = new cli.BookingsClient({ baseUrl: strict.url });
  assert.deepEqual(await strictClient.report({ broken: false }), { parts: [1, 2], total: 3 });
  assert.equal(events.length, 0);
  await assert.rejects(strictClient.report({ broken: true }), (e: unknown) => {
    const err = e as { status: number; data?: unknown; body?: unknown };
    assert.equal(err.status, 500);
    assert.ok(!JSON.stringify(err.data ?? err.body).includes('sums'));
    return true;
  });
  assert.equal(events.length, 1);
  assert.equal(events[0]?.position, 'output');
  assert.equal(events[0]?.outcome, 'rejected');
  assert.ok(events[0]?.operation.startsWith('veld:'));
  assert.ok(events[0]?.elapsedMicros !== undefined && events[0].elapsedMicros >= 0);

  events.length = 0;
  const auditClient = new cli.BookingsClient({ baseUrl: audit.url });
  assert.deepEqual(await auditClient.report({ broken: true }), { parts: [1, 2], total: 5 });
  assert.equal(events.length, 1);
  assert.equal(events[0]?.outcome, 'audited');

  // The partial check of a form.
  const early = cli.Booking.partial({ start: '2026-01-02T00:00:00Z', end: '2026-01-01T00:00:00Z' });
  assert.deepEqual(early.map((v) => v.fields), [['end', 'start']]);
  assert.deepEqual(cli.Booking.partial({ start: '2026-01-02T00:00:00Z' }), []);
  assert.equal(cli.Booking.partial({ guests: 11 })[0]?.message, 'violates rule party');
  assert.ok(cli.Booking.ruleIds.party.includes('Booking.party@'));

  await digestTests(strict.url);

  strict.close();
  audit.close();
  console.log('rules http ok');
}

/** A fetch that counts the requests and can change the digest the server appears to send. */
function digestFetch(counter: { n: number }, digest?: string | null): rt.FetchLike {
  return async (url, init) => {
    counter.n += 1;
    const res = await fetch(url, init as RequestInit);
    if (digest === undefined) return res;
    return {
      status: res.status,
      text: () => res.text(),
      body: res.body as unknown as undefined,
      headers: { get: (name: string) => (name.toLowerCase() === 'veld-rules' ? digest : res.headers.get(name)) },
    };
  };
}

async function failure(call: Promise<unknown>): Promise<cli.ValidationError> {
  const e = await call.then(
    () => undefined,
    (err: unknown) => err,
  );
  assert.ok(cli.ValidationError.matches(e));
  return e as cli.ValidationError;
}

/** The rules digest of a contract with invariants only (spec/rules.md section 9.7). */
async function digestTests(url: string): Promise<void> {
  // The server sends it on every response of the service, errors included, and on nothing else.
  const ok = await fetch(`${url}/bookings/search?from=1&to=5`);
  const digest = ok.headers.get('veld-rules');
  assert.match(digest ?? '', /^[0-9a-f]{16}$/);
  const bad = await fetch(`${url}/bookings/`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(BAD),
  });
  assert.equal(bad.status, 422);
  assert.equal(bad.headers.get('veld-rules'), digest, 'errors carry the digest too');
  assert.equal((await fetch(`${url}/nowhere`)).headers.get('veld-rules'), null);

  // Same digest: the client rejects locally only after its first response.
  const same = { n: 0 };
  const mismatches: rt.ContractMismatch[] = [];
  const a = new cli.BookingsClient({ baseUrl: url, fetch: digestFetch(same), onContractMismatch: (m) => mismatches.push(m) });
  assert.equal((await failure(a.create({ body: BAD }))).local, false, 'the server decides before the first response');
  assert.equal(same.n, 1);
  assert.equal((await failure(a.create({ body: BAD }))).local, true, 'with the same digest the client rejects locally');
  assert.equal(same.n, 1, 'a local rejection sends nothing');
  assert.deepEqual(mismatches, []);

  // A different digest: everything is sent, and onContractMismatch is called once.
  const other = { n: 0 };
  const seen: rt.ContractMismatch[] = [];
  const b = new cli.BookingsClient({ baseUrl: url, fetch: digestFetch(other, 'ffffffffffffffff'), onContractMismatch: (m) => seen.push(m) });
  await b.create({ body: GOOD });
  assert.equal(seen.length, 1);
  assert.equal(seen[0]?.service, 'Bookings');
  assert.equal(seen[0]?.serverDigest, 'ffffffffffffffff');
  assert.equal(seen[0]?.clientDigest, digest);
  assert.equal((await failure(b.create({ body: BAD }))).local, false, 'the local check stops when the digest differs');
  assert.equal(other.n, 2, 'the request was sent');
  await b.create({ body: GOOD });
  assert.equal(seen.length, 1, 'once per digest');

  // A response without the header: the client sends anyway.
  const bare = { n: 0 };
  const c = new cli.BookingsClient({ baseUrl: url, fetch: digestFetch(bare, null) });
  await c.create({ body: GOOD });
  assert.equal((await failure(c.create({ body: BAD }))).local, false);
  assert.equal(bare.n, 2);
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
