// State machines over real HTTP (spec/rules.md section 10): the production handler of a generated
// Node server. Expects `./server` and `./client` generated from
// tests/rules/endpoints/transitions.veld.pending.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000002';
const bookings = new Map<string, srv.Booking>();
const seen: { name: string; transition: unknown; keys: string[] }[] = [];
let move: (req: srv.PayBookingRequest & srv.PayBookingFacts & srv.PayBookingTransition) => srv.Booking = (req) => ({
  ...req.booking,
  status: srv.Status.paid,
});

const loaders: srv.BookingsFactLoaders = {
  async booking(key) {
    return bookings.get(key);
  },
};

const impl: srv.BookingsService = {
  async payBooking(req) {
    seen.push({ name: 'payBooking', transition: req.transition, keys: Object.keys(req).sort() });
    return move(req);
  },
  async cancelBooking(req) {
    seen.push({ name: 'cancelBooking', transition: req.transition, keys: Object.keys(req).sort() });
    return { ...req.booking, status: srv.Status.cancelled };
  },
  async checkIn(req) {
    seen.push({ name: 'checkIn', transition: req.transition, keys: Object.keys(req).sort() });
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

interface Doc {
  type: string;
  title: string;
  status: number;
  data: Record<string, unknown>;
  violations?: { rule: string; path: string; fields: string[]; message: string }[];
}

async function post(url: string, path: string, body?: unknown): Promise<{ status: number; type: string | null; doc: Doc }> {
  const res = await fetch(`${url}${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const type = res.headers.get('content-type');
  const text = await res.text();
  return { status: res.status, type, doc: (text === '' ? {} : JSON.parse(text)) as Doc };
}

function put(status: srv.Status, total = '50.00'): void {
  bookings.clear();
  seen.length = 0;
  bookings.set(ID, { id: ID, status, total });
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  const strict = await listen(srv.createBookingsHandler(impl, { facts: loaders, onRuleViolation: (e) => events.push(e) }));
  const audit = await listen(
    srv.createBookingsHandler(impl, { facts: loaders, rules: { output: 'audit' }, onRuleViolation: (e) => events.push(e) }),
  );
  const off = await listen(srv.createBookingsHandler(impl, { facts: loaders, rules: { output: 'off' } }));

  // The helpers: the states that can follow a state, in declaration order of the edges, and finals.
  assert.deepEqual(srv.Status.nextStates('pending'), ['paid', 'cancelled']);
  assert.deepEqual(srv.Status.nextStates('paid'), ['checkedIn', 'cancelled', 'refunded']);
  assert.deepEqual(srv.Status.nextStates('refunded'), []);
  assert.equal(srv.Status.isFinal('refunded'), true);
  assert.equal(srv.Status.isFinal('paid'), false);
  assert.deepEqual(cli.Status.nextStates('pending'), srv.Status.nextStates('pending'));

  // A legal move: the handler receives the transition as data, next to the parameters and the facts.
  put('pending');
  let r = await post(strict.url, `/bookings/${ID}/pay`, { amount: '50.00' });
  assert.equal(r.status, 201);
  assert.equal(seen.length, 1);
  assert.deepEqual(seen[0]?.transition, { field: 'status', from: 'pending', to: 'paid' });
  assert.deepEqual(seen[0]?.keys, ['body', 'booking', 'id', 'transition']);

  // An illegal move: 409 with field, from and to, one violation, and the handler never runs.
  put('paid');
  events.length = 0;
  r = await post(strict.url, `/bookings/${ID}/pay`, { amount: '50.00' });
  assert.equal(r.status, 409);
  assert.match(r.type ?? '', /^application\/problem\+json/);
  assert.equal(r.doc.title, 'InvalidTransition');
  assert.deepEqual(r.doc.data, { field: 'status', from: 'paid', to: 'paid' });
  assert.equal(r.doc.violations?.length, 1);
  const v = r.doc.violations?.[0];
  assert.match(v?.rule ?? '', /^veld:bookings\/Bookings\.payBooking\.transition@[0-9a-f]{16}$/);
  assert.equal(v?.path, 'request');
  assert.deepEqual(v?.fields, []);
  assert.equal(v?.message, 'violates rule transition');
  assert.equal(seen.length, 0);
  assert.equal(events[0]?.position, 'input');
  assert.equal(events[0]?.outcome, 'rejected');

  // Several sources, and a final state has no way out.
  put('paid');
  r = await post(strict.url, `/bookings/${ID}/cancel`);
  assert.equal(r.status, 201);
  assert.deepEqual(seen[0]?.transition, { field: 'status', from: 'paid', to: 'cancelled' });
  put('refunded');
  r = await post(strict.url, `/bookings/${ID}/cancel`);
  assert.equal(r.status, 409);
  assert.deepEqual(r.doc.data, { field: 'status', from: 'refunded', to: 'cancelled' });

  // The order: a missing fact first, then the transition, then require.
  bookings.clear();
  r = await post(strict.url, `/bookings/${ID}/pay`, { amount: '1.00' });
  assert.equal(r.status, 404);
  put('cancelled');
  r = await post(strict.url, `/bookings/${ID}/pay`, { amount: '1.00' });
  assert.equal(r.status, 409, 'the transition is checked before require');
  put('pending');
  r = await post(strict.url, `/bookings/${ID}/pay`, { amount: '1.00' });
  assert.equal(r.status, 422);
  assert.equal(r.doc.title, 'AmountMismatch');

  // An endpoint without a result has no target check.
  put('paid');
  r = await post(strict.url, `/bookings/${ID}/check-in`);
  assert.equal(r.status, 204);

  // The target check follows the output mode.
  const stay = (req: srv.PayBookingRequest & srv.PayBookingFacts): srv.Booking => ({ ...req.booking });
  put('pending');
  move = stay;
  events.length = 0;
  r = await post(strict.url, `/bookings/${ID}/pay`, { amount: '50.00' });
  assert.equal(r.status, 500);
  assert.equal(r.doc.title, 'Internal');
  assert.deepEqual(r.doc.data, {});
  assert.equal(events.length, 1);
  assert.equal(events[0]?.position, 'output');
  assert.equal(events[0]?.outcome, 'rejected');
  assert.equal(events[0]?.violation.message, 'the result must hold the state `paid` in `status`, found `pending`');
  events.length = 0;
  r = await post(audit.url, `/bookings/${ID}/pay`, { amount: '50.00' });
  assert.equal(r.status, 201);
  assert.equal(events[0]?.outcome, 'audited');
  assert.equal(events[0]?.violation.message, 'the result must hold the state `paid` in `status`, found `pending`');
  events.length = 0;
  r = await post(off.url, `/bookings/${ID}/pay`, { amount: '50.00' });
  assert.equal(r.status, 201);
  assert.equal(events.length, 0);

  strict.close();
  audit.close();
  off.close();
  console.log('state machines over HTTP: ok');
}

void main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
