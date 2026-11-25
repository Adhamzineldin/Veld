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

  strict.close();
  audit.close();
  console.log('rules http ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
