// Audited rules over real HTTP, SSE and WebSocket (spec/rules.md section 17.1): a generated Node server.
// An audited rule is evaluated where the enforced one would be, goes to `onRuleViolation` with the outcome
// `audited`, and never rejects, never answers and never shows in a response. Expects `./server` (node)
// generated from tests/toolchain/contract/rollout.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { WebSocket as RawWebSocket, WebSocketServer } from 'ws';
import * as cli from './client/index';
import { AuditClient } from './client/bookings/client';
import * as srv from './server/index';
import type { RuleViolationEvent } from './server/runtime/veld';
import { attachWebSocket } from './server/server/ws';

const ROOM = '00000000-0000-4000-8000-000000000001';
let capacity = 100;

const loaders: srv.AuditFactLoaders = {
  async room() {
    return { id: ROOM, capacity };
  },
};

const impl: srv.AuditService = {
  // Five guests come back as six, which breaks the audited `ensure`.
  async book(req) {
    return { ...req.body, guests: req.body.guests === 5 ? 6 : req.body.guests };
  },
  async *ticks(req) {
    for (let n = 0; n < req.count; n += 1) yield { n };
  },
  async announce(_req, _ctx, ws) {
    for (let n = 0; n < 3; n += 1) ws.send({ n });
  },
};

interface Running {
  url: string;
  events: RuleViolationEvent[];
  close(): Promise<void>;
}

async function start(extra: Record<string, unknown> = {}): Promise<Running> {
  const events: RuleViolationEvent[] = [];
  const hooks = { facts: loaders, onRuleViolation: (e: RuleViolationEvent) => events.push(e), ...extra };
  const handler = srv.createAuditHandler(impl, hooks);
  const server = http.createServer(srv.toNodeHttp(handler));
  const wss = new WebSocketServer({ server });
  attachWebSocket(wss, handler);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const port = (server.address() as AddressInfo).port;
  return {
    url: `http://127.0.0.1:${String(port)}`,
    events,
    close: () =>
      new Promise<void>((resolve) => {
        for (const c of wss.clients) c.terminate();
        server.closeAllConnections();
        server.close(() => resolve());
      }),
  };
}

async function book(url: string, guests: number): Promise<{ status: number; text: string }> {
  const r = await fetch(`${url}/audit/book`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ roomId: ROOM, guests }),
  });
  return { status: r.status, text: await r.text() };
}

/** What the hook heard, as `outcome position rule path`, in order. */
function heard(events: RuleViolationEvent[]): string[] {
  return events.map((e) => `${e.outcome} ${e.position} ${e.violation.rule.replace(/^veld:[^/]+\//, '').replace(/@.*$/, '')} ${e.violation.path}`);
}

async function invariantAndRequire(): Promise<void> {
  const strict = await start();
  // An audited invariant of the body and of the result: reported, and the request goes on.
  capacity = 100;
  const a = await book(strict.url, 7);
  assert.equal(a.status, 201);
  assert.deepEqual(JSON.parse(a.text), { roomId: ROOM, guests: 7 });
  assert.ok(!a.text.includes('violations') && !a.text.includes('smallParty'));
  assert.deepEqual(heard(strict.events), ['audited input Party.smallParty body', 'audited output Party.smallParty result']);
  assert.ok(strict.events.every((e) => e.violation.fields.join() === 'guests' || e.violation.fields.length > 0));
  assert.ok(strict.events.every((e) => e.operation.includes('Audit.book')));

  // An audited `require ... else E` that fails is not answered: the payload divides by zero and is never built.
  strict.events.length = 0;
  capacity = 2;
  const b = await book(strict.url, 3);
  assert.equal(b.status, 201, b.text);
  assert.ok(!b.text.includes('RoomFull') && !b.text.includes('"violations"'));
  assert.deepEqual(heard(strict.events), ['audited input Audit.book.require request']);
  assert.equal(strict.events[0]?.violation.error, undefined);

  // An audited `ensure` that the result breaks leaves the outcome ok.
  strict.events.length = 0;
  capacity = 100;
  const c = await book(strict.url, 5);
  assert.equal(c.status, 201);
  assert.equal((JSON.parse(c.text) as { guests: number }).guests, 6);
  assert.deepEqual(heard(strict.events), ['audited output Audit.book.ensure request']);

  // A rule that holds is not reported.
  strict.events.length = 0;
  assert.equal((await book(strict.url, 2)).status, 201);
  assert.deepEqual(heard(strict.events), []);
  await strict.close();
}

async function modes(): Promise<void> {
  // Output mode `off` skips an audited output rule like every output rule; an audited input rule is evaluated.
  const off = await start({ rules: { output: 'off' } });
  capacity = 100;
  assert.equal((await book(off.url, 7)).status, 201);
  assert.deepEqual(heard(off.events), ['audited input Party.smallParty body']);
  off.events.length = 0;
  assert.equal((await book(off.url, 5)).status, 201);
  assert.deepEqual(heard(off.events), []);
  await off.close();

  // `audit` evaluates and reports them too.
  const audit = await start({ rules: { output: 'audit' } });
  assert.equal((await book(audit.url, 5)).status, 201);
  assert.deepEqual(heard(audit.events), ['audited output Audit.book.ensure request']);
  await audit.close();

  // With request validation off an audited rule is off exactly where the enforced one would be: the
  // invariants of the body, `require` and `ensure` are not evaluated, the invariants of the result are.
  const loose = await start({ validateRequests: false });
  capacity = 2;
  assert.equal((await book(loose.url, 7)).status, 201);
  assert.deepEqual(heard(loose.events), ['audited output Party.smallParty result']);
  loose.events.length = 0;
  assert.equal((await book(loose.url, 5)).status, 201);
  assert.deepEqual(heard(loose.events), []);
  await loose.close();

  // A hook that throws changes nothing.
  const boom = await start({ onRuleViolation: () => { throw new Error('boom'); } });
  capacity = 100;
  assert.equal((await book(boom.url, 7)).status, 201);
  await boom.close();
}

async function streams(): Promise<void> {
  const sse = await start();
  const r = await fetch(`${sse.url}/audit/ticks?count=4`);
  assert.equal(r.status, 200);
  const text = await r.text();
  assert.equal(text.split('\n').filter((l) => l.startsWith('data:')).length, 4, text);
  assert.ok(!text.includes('event: error'));
  // The odd items break the audited invariant: they are sent all the same, and reported.
  assert.deepEqual(heard(sse.events), ['audited output Tick.even result', 'audited output Tick.even result']);
  await sse.close();

  const ws = await start();
  const sock = new RawWebSocket(`${ws.url.replace(/^http/, 'ws')}/audit/announce`);
  const frames: string[] = [];
  sock.on('message', (d) => frames.push(String(d)));
  await new Promise((resolve) => sock.on('close', resolve));
  assert.equal(frames.length, 3, frames.join('\n'));
  assert.deepEqual(heard(ws.events), ['audited output Tick.even result']);
  await ws.close();
}

/** A client skips audited rules: in the local check before a send and in the partial check of a form. */
async function client(): Promise<void> {
  const draft = { roomId: ROOM, guests: 7 };
  assert.deepEqual(cli.Party.check(draft), []);
  assert.deepEqual(cli.Party.partial({ guests: 7 }), []);
  assert.deepEqual(cli.Tick.check({ n: 1 }), []);
  const running = await start();
  capacity = 100;
  // The local check does not block the send, and the server accepts it.
  const sent = await new AuditClient({ baseUrl: running.url }).book({ body: draft });
  assert.deepEqual(sent, draft);
  assert.deepEqual(heard(running.events), ['audited input Party.smallParty body', 'audited output Party.smallParty result']);
  await running.close();
}

async function main(): Promise<void> {
  await client();
  await invariantAndRequire();
  await modes();
  await streams();
  console.log('rollout http ok');
}

main().then(
  () => process.exit(0),
  (e: unknown) => {
    console.error(e);
    process.exit(1);
  },
);
