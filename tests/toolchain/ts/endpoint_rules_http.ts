// Endpoint rules over real HTTP (spec/rules.md section 9): the production handler of a generated Node
// server and a generated client. Expects `./server` (node, role server) and `./client` (typescript)
// generated from tests/rules/endpoints/bookings.veld.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';
import type { ContractMismatch, FetchLike } from './client/runtime/veld';

const ROOM_ID = '00000000-0000-4000-8000-000000000001';
const BOOKING_ID = '00000000-0000-4000-8000-000000000002';
const NOW = new Date('2026-01-01T00:00:00Z');

const rooms = new Map<string, srv.Room>();
const coupons = new Map<string, srv.Coupon>();
const bookings = new Map<string, srv.Booking>();
const log: string[] = [];
let served = 0;

const loaders: srv.BookingsFactLoaders = {
  async room(key) {
    log.push(`room:${key}`);
    return rooms.get(key);
  },
  async coupon(key) {
    log.push(`coupon:${key}`);
    return coupons.get(key);
  },
  async booking(key) {
    log.push(`booking:${key}`);
    return bookings.get(key);
  },
};

type CreateReq = srv.CreateBookingRequest & srv.CreateBookingFacts;
type PayReq = srv.PayBookingRequest & srv.PayBookingFacts;

let created: (req: CreateReq) => srv.Booking = (req) => ({
  ...req.body,
  id: BOOKING_ID,
  status: srv.Status.pending,
  total: '100.00',
});
let paid: (req: PayReq) => srv.Booking = (req) => ({ ...req.booking, status: srv.Status.paid });

const impl: srv.BookingsService = {
  async createBooking(req) {
    served += 1;
    return created(req);
  },
  async payBooking(req) {
    served += 1;
    return paid(req);
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
  data: Record<string, unknown> & { issues?: { path: string; code: string; message: string }[] };
  violations?: { rule: string; path: string; fields: string[]; message: string; error?: string }[];
}

async function post(url: string, path: string, body: unknown): Promise<{ status: number; digest: string | null; doc: Doc }> {
  const res = await fetch(`${url}${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  return { status: res.status, digest: res.headers.get('veld-rules'), doc: (await res.json()) as Doc };
}

const GOOD = { roomId: ROOM_ID, guests: 2, start: '2026-06-01T00:00:00Z', end: '2026-06-03T00:00:00Z' };
const BAD = { roomId: ROOM_ID, guests: 2, start: '2026-06-03T00:00:00Z', end: '2026-06-01T00:00:00Z' };

function reset(): void {
  rooms.clear();
  coupons.clear();
  bookings.clear();
  log.length = 0;
  served = 0;
  rooms.set(ROOM_ID, { id: ROOM_ID, capacity: 2, closed: false, nightly: '50.00' });
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  let clockReads = 0;
  const strict = await listen(
    srv.createBookingsHandler(impl, {
      facts: loaders,
      clock: () => {
        clockReads += 1;
        return NOW;
      },
      onRuleViolation: (e) => events.push(e),
    }),
  );
  const audit = await listen(
    srv.createBookingsHandler(impl, { facts: loaders, clock: () => NOW, rules: { output: 'audit' }, onRuleViolation: (e) => events.push(e) }),
  );
  const off = await listen(
    srv.createBookingsHandler(impl, { facts: loaders, clock: () => NOW, rules: { output: 'off' }, onRuleViolation: (e) => events.push(e) }),
  );

  // A success carries the digest, and the clock was read once.
  reset();
  const ok = await post(strict.url, '/bookings', GOOD);
  assert.equal(ok.status, 201);
  assert.equal(clockReads, 1);
  const digest = srv.bookingsEndpoints(impl, loaders)[0]?.digest;
  assert.equal(typeof digest, 'string');
  assert.equal(ok.digest, digest);
  assert.deepEqual(log, [`room:${ROOM_ID}`]);

  // A required fact that is not found: 404 NotFound with the fact's name, and nothing after it runs.
  reset();
  rooms.clear();
  let r = await post(strict.url, '/bookings', { ...GOOD, couponCode: 'HALF' });
  assert.equal(r.status, 404);
  assert.equal(r.doc.title, 'NotFound');
  assert.equal(r.doc.data['resource'], 'room');
  assert.deepEqual(log, [`room:${ROOM_ID}`], 'the coupon loader must not run after a missing room');
  assert.equal(served, 0);
  assert.equal(r.digest, digest, 'errors carry the digest too');

  // An optional fact the loader does not find is absent, not an error; its key is the coupon code.
  reset();
  r = await post(strict.url, '/bookings', { ...GOOD, couponCode: 'GONE' });
  assert.equal(r.status, 201);
  assert.deepEqual(log, [`room:${ROOM_ID}`, 'coupon:GONE']);

  // An optional fact whose key is absent: the loader is not called.
  reset();
  coupons.set('BIG', { code: 'BIG', percent: 60 });
  r = await post(strict.url, '/bookings', GOOD);
  assert.equal(r.status, 201);
  assert.deepEqual(log, [`room:${ROOM_ID}`]);

  // The handler gets the facts with the parameters.
  reset();
  let seen: CreateReq | undefined;
  created = (req) => {
    seen = req;
    return { ...req.body, id: BOOKING_ID, status: srv.Status.pending, total: '1.00' };
  };
  coupons.set('HALF', { code: 'HALF', percent: 50 });
  r = await post(strict.url, '/bookings', { ...GOOD, couponCode: 'HALF' });
  assert.equal(r.status, 201);
  assert.equal(seen?.room.capacity, 2);
  assert.equal(seen?.coupon?.percent, 50);
  assert.equal(seen?.body.guests, 2);
  created = (req) => ({ ...req.body, id: BOOKING_ID, status: srv.Status.pending, total: '100.00' });

  // `require .. else E { .. }`: the declared error with its payload and one violation.
  reset();
  r = await post(strict.url, '/bookings', { ...GOOD, guests: 3 });
  assert.equal(r.status, 409);
  assert.equal(r.doc.title, 'RoomFull');
  assert.deepEqual(r.doc.data, { capacity: 2 });
  assert.equal(r.doc.violations?.length, 1);
  assert.equal(r.doc.violations?.[0]?.path, 'request');
  assert.deepEqual(r.doc.violations?.[0]?.fields, ['body']);
  assert.match(r.doc.violations?.[0]?.rule ?? '', /^veld:bookings\/Bookings\.createBooking\.require@/);
  assert.equal(served, 0);

  // The first failing require wins.
  reset();
  rooms.set(ROOM_ID, { id: ROOM_ID, capacity: 1, closed: true, nightly: '50.00' });
  r = await post(strict.url, '/bookings', { ...GOOD, guests: 5 });
  assert.equal(r.status, 409);
  assert.equal(r.doc.title, 'RoomClosed');

  // Step 5 comes before step 6: an invalid body is rejected before any fact is loaded.
  reset();
  r = await post(strict.url, '/bookings', BAD);
  assert.equal(r.status, 422);
  assert.deepEqual(log, []);
  assert.equal(r.doc.violations?.[0]?.path, 'body');

  // A require without `else`: 422 ValidationError with one issue and one violation; `now` is the clock.
  reset();
  r = await post(strict.url, '/bookings', { ...GOOD, start: '2025-12-31T00:00:00Z', end: '2026-01-02T00:00:00Z' });
  assert.equal(r.status, 422);
  assert.equal(r.doc.title, 'ValidationError');
  assert.deepEqual(r.doc.data.issues, [{ path: 'request', code: 'rule', message: 'A booking cannot start in the past' }]);
  assert.equal(r.doc.violations?.length, 1);
  assert.match(r.doc.violations?.[0]?.rule ?? '', /\.createBooking\.notInPast@/);
  r = await post(strict.url, '/bookings', { ...GOOD, start: '2026-01-01T00:00:00Z', end: '2026-01-02T00:00:00Z' });
  assert.equal(r.status, 201, 'starting exactly now is allowed');

  // An optional fact is read under has().
  reset();
  coupons.set('BIG', { code: 'BIG', percent: 60 });
  r = await post(strict.url, '/bookings', { ...GOOD, couponCode: 'BIG' });
  assert.equal(r.status, 422);
  assert.match(r.doc.violations?.[0]?.rule ?? '', /\.couponWorks@/);

  // ensure, strict: 500 Internal, no rule detail for the caller; the hook gets the violation.
  reset();
  events.length = 0;
  created = (req) => ({ ...req.body, id: BOOKING_ID, status: srv.Status.paid, total: '1.00' });
  r = await post(strict.url, '/bookings', GOOD);
  assert.equal(r.status, 500);
  assert.equal(r.doc.title, 'Internal');
  assert.equal(r.doc.violations, undefined);
  assert.deepEqual(r.doc.data, {});
  assert.equal(events.length, 1);
  assert.equal(events[0]?.position, 'output');
  assert.equal(events[0]?.outcome, 'rejected');
  assert.match(events[0]?.rule ?? '', /\.startsPending@/);

  // ensure, audit: the response is sent unchanged and the hook is told.
  events.length = 0;
  r = await post(audit.url, '/bookings', GOOD);
  assert.equal(r.status, 201);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.outcome, 'audited');

  // ensure, off: not evaluated.
  events.length = 0;
  r = await post(off.url, '/bookings', GOOD);
  assert.equal(r.status, 201);
  assert.equal(events.length, 0);
  created = (req) => ({ ...req.body, id: BOOKING_ID, status: srv.Status.pending, total: '100.00' });

  // A handler that keeps the room: the second ensure.
  reset();
  events.length = 0;
  created = (req) => ({ ...req.body, roomId: '00000000-0000-4000-8000-0000000000ff', id: BOOKING_ID, status: srv.Status.pending, total: '1.00' });
  r = await post(strict.url, '/bookings', GOOD);
  assert.equal(r.status, 500);
  assert.match(events[0]?.rule ?? '', /\.keepsRoom@/);
  created = (req) => ({ ...req.body, id: BOOKING_ID, status: srv.Status.pending, total: '100.00' });

  // payBooking: a declared error with a decimal payload and one violation.
  reset();
  bookings.set(BOOKING_ID, { ...GOOD, id: BOOKING_ID, status: srv.Status.pending, total: '120.00' });
  r = await post(strict.url, `/bookings/${BOOKING_ID}/pay`, { amount: '100.00' });
  assert.equal(r.status, 422);
  assert.equal(r.doc.title, 'AmountMismatch');
  assert.deepEqual(r.doc.data, { expected: '120.00' });
  assert.equal(r.doc.violations?.length, 1);
  assert.deepEqual(r.doc.violations?.[0]?.fields, ['body']);

  // payBooking: an unknown booking.
  r = await post(strict.url, `/bookings/${ROOM_ID}/pay`, { amount: '100.00' });
  assert.equal(r.status, 404);
  assert.equal(r.doc.data['resource'], 'booking');

  // old() survives a handler that changes the loaded object in place.
  events.length = 0;
  paid = (req) => {
    req.booking.total = '99.00';
    return { ...req.booking, status: srv.Status.paid };
  };
  r = await post(strict.url, `/bookings/${BOOKING_ID}/pay`, { amount: '120.00' });
  assert.equal(r.status, 500);
  assert.match(events[0]?.rule ?? '', /\.payBooking\.ensure@/);
  // The loaded object was changed by the handler above; load a fresh one and keep the total.
  bookings.set(BOOKING_ID, { ...GOOD, id: BOOKING_ID, status: srv.Status.pending, total: '120.00' });
  paid = (req) => ({ ...req.booking, status: srv.Status.paid });
  events.length = 0;
  r = await post(strict.url, `/bookings/${BOOKING_ID}/pay`, { amount: '120.00' });
  assert.equal(r.status, 201);
  assert.equal(events.length, 0);
  // 120 and 120.0 are the same amount.
  r = await post(strict.url, `/bookings/${BOOKING_ID}/pay`, { amount: '120.0' });
  assert.equal(r.status, 201);

  // The digest is on every response of the service: 404 from the router has no endpoint, so none.
  const miss = await fetch(`${strict.url}/nowhere`);
  assert.equal(miss.status, 404);

  await clientBehaviour(strict.url, digest);
  strict.close();
  audit.close();
  off.close();
  console.log('endpoint rules over HTTP: ok');
}

/** A fetch that counts the requests and can change the digest the server appears to send. */
function counting(counter: { n: number }, digest?: string | null): FetchLike {
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

async function clientBehaviour(url: string, serverDigest: string | undefined): Promise<void> {
  reset();
  created = (req) => ({ ...req.body, id: BOOKING_ID, status: srv.Status.pending, total: '100.00' });

  // Same digest: it rejects locally only after its first response.
  const same = { n: 0 };
  const mismatches: ContractMismatch[] = [];
  const a = new cli.BookingsClient({ baseUrl: url, fetch: counting(same), onContractMismatch: (m) => mismatches.push(m) });
  const first = await a.createBooking({ body: BAD }).then(
    () => undefined,
    (e: unknown) => e,
  );
  assert.ok(cli.ValidationError.matches(first), 'the server decides before the first response');
  assert.equal((first as cli.ValidationError).local, false);
  assert.equal(same.n, 1);
  await a.createBooking({ body: GOOD });
  assert.equal(same.n, 2);
  const second = await a.createBooking({ body: BAD }).then(
    () => undefined,
    (e: unknown) => e,
  );
  assert.ok(cli.ValidationError.matches(second));
  assert.equal((second as cli.ValidationError).local, true, 'with the same digest the client rejects locally');
  assert.equal(same.n, 2, 'a local rejection sends nothing');
  assert.deepEqual(mismatches, []);

  // A different digest: everything is sent, and onContractMismatch is called once.
  const other = { n: 0 };
  const seen: ContractMismatch[] = [];
  const b = new cli.BookingsClient({ baseUrl: url, fetch: counting(other, 'ffffffffffffffff'), onContractMismatch: (m) => seen.push(m) });
  await b.createBooking({ body: GOOD });
  assert.equal(seen.length, 1);
  assert.equal(seen[0]?.service, 'Bookings');
  assert.equal(seen[0]?.serverDigest, 'ffffffffffffffff');
  assert.equal(seen[0]?.clientDigest, serverDigest);
  const sent = await b.createBooking({ body: BAD }).then(
    () => undefined,
    (e: unknown) => e,
  );
  assert.ok(cli.ValidationError.matches(sent));
  assert.equal((sent as cli.ValidationError).local, false, 'the local check stops when the digest differs');
  assert.equal(other.n, 2, 'the request was sent');
  await b.createBooking({ body: GOOD });
  assert.equal(seen.length, 1, 'once per digest');

  // A response without the header: the client sends anyway.
  const bare = { n: 0 };
  const c = new cli.BookingsClient({ baseUrl: url, fetch: counting(bare, null) });
  await c.createBooking({ body: GOOD });
  const unsure = await c.createBooking({ body: BAD }).then(
    () => undefined,
    (e: unknown) => e,
  );
  assert.equal((unsure as cli.ValidationError).local, false);
  assert.equal(bare.n, 2);
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
