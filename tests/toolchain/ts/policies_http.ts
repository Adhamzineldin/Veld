// Policies over real HTTP (spec/rules.md section 11): the production handler of a generated Node
// server and the `can<Endpoint>` of a generated client. Expects `./server` and `./client` generated
// from tests/rules/endpoints/policies.veld.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';

const GUEST = '00000000-0000-4000-8000-0000000000aa';
const OTHER = '00000000-0000-4000-8000-0000000000bb';
const ID = '00000000-0000-4000-8000-000000000002';

const users = new Map<string, srv.User>([
  ['guest', { id: GUEST, roles: [], suspended: false }],
  ['member', { id: GUEST, roles: ['member'], suspended: false }],
  ['other', { id: OTHER, roles: [], suspended: false }],
  ['support', { id: OTHER, roles: ['support'], suspended: false }],
  ['bad', { id: GUEST, roles: ['support'], suspended: true }],
  ['badmember', { id: OTHER, roles: ['member'], suspended: true }],
]);
const bookings = new Map<string, srv.Booking>();
const callers: srv.User[] = [];
const loaded: string[] = [];
let served = 0;

const loaders: srv.BookingsFactLoaders = {
  async booking(key) {
    loaded.push(key);
    return bookings.get(key);
  },
};

const impl: srv.BookingsService = {
  async payBooking(req, ctx) {
    served += 1;
    callers.push(ctx.principal as srv.User);
    return req.booking;
  },
  async getBooking(req) {
    served += 1;
    return req.booking;
  },
};

// The auth hook has the type of the principal's model: no cast, no `unknown`.
const hooks = {
  facts: loaders,
  auth: {
    bearer: async (token: string): Promise<srv.User | null> => users.get(token) ?? null,
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
  title: string;
  status: number;
  data: Record<string, unknown>;
  violations?: { rule: string; path: string; fields: string[]; message: string }[];
}

async function call(
  url: string,
  token: string,
  method: 'GET' | 'POST',
  path: string,
  body?: unknown,
): Promise<{ status: number; type: string | null; doc: Doc }> {
  const res = await fetch(`${url}${path}`, {
    method,
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const text = await res.text();
  return { status: res.status, type: res.headers.get('content-type'), doc: (text === '' ? {} : JSON.parse(text)) as Doc };
}

function stock(): void {
  bookings.clear();
  callers.length = 0;
  loaded.length = 0;
  served = 0;
  bookings.set(ID, { id: ID, guestId: GUEST, total: '10.00' });
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  const server = await listen(srv.createBookingsHandler(impl, { ...hooks, onRuleViolation: (e) => events.push(e) }));
  const pay = (token: string, amount = '10.00') => call(server.url, token, 'POST', `/bookings/${ID}/pay`, { amount });
  const get = (token: string) => call(server.url, token, 'GET', `/bookings/${ID}`);

  // A permitted caller, and the handler sees the principal the hook returned.
  stock();
  let r = await pay('guest');
  assert.equal(r.status, 201);
  assert.deepEqual(callers[0], users.get('guest'));

  // A denied caller: 403 Forbidden, the message of the answer and of the one violation.
  stock();
  events.length = 0;
  r = await pay('other');
  assert.equal(r.status, 403);
  assert.match(r.type ?? '', /^application\/problem\+json/);
  assert.equal(r.doc.title, 'Forbidden');
  assert.deepEqual(r.doc.data, { message: 'not permitted' });
  assert.equal(r.doc.violations?.length, 1);
  const v = r.doc.violations?.[0];
  assert.match(v?.rule ?? '', /^veld:bookings\/ManageBooking@[0-9a-f]{16}$/);
  assert.equal(v?.path, 'request');
  assert.deepEqual(v?.fields, []);
  assert.equal(v?.message, 'not permitted');
  assert.equal(served, 0);
  assert.equal(events[0]?.position, 'input');
  assert.equal(events[0]?.outcome, 'rejected');

  // `forbid` beats `permit` and gives its message.
  r = await pay('bad');
  assert.equal(r.status, 403);
  assert.deepEqual(r.doc.data, { message: 'A suspended account cannot manage bookings' });
  assert.equal(r.doc.violations?.[0]?.message, 'A suspended account cannot manage bookings');
  r = await pay('support');
  assert.equal(r.status, 201);

  // The order: a missing fact wins over a denial, a denial wins over require.
  bookings.clear();
  r = await pay('other');
  assert.equal(r.status, 404);
  stock();
  r = await pay('other', '1.00');
  assert.equal(r.status, 403, 'a denial wins over require');
  r = await pay('guest', '1.00');
  assert.equal(r.status, 422);

  // Several `allow` lines run in source order and the first denial ends the request.
  stock();
  r = await get('member');
  assert.equal(r.status, 200);
  r = await get('guest');
  assert.equal(r.status, 403);
  assert.deepEqual(r.doc.violations?.map((x) => x.rule.split('/')[1]?.split('@')[0]), ['AnyRole']);
  r = await get('badmember');
  assert.equal(r.status, 403);
  assert.deepEqual(r.doc.data, { message: 'A suspended account cannot manage bookings' });
  assert.deepEqual(r.doc.violations?.map((x) => x.rule.split('/')[1]?.split('@')[0]), ['ManageBooking']);

  // No credentials: authentication comes first.
  r = await call(server.url, 'nobody', 'GET', `/bookings/${ID}`);
  assert.equal(r.status, 401);

  // `can<Endpoint>` agrees with the server for a permitted and a denied caller.
  const booking = bookings.get(ID) as srv.Booking;
  for (const [token, user] of users) {
    stock();
    const payNow = await pay(token);
    assert.equal(cli.BookingsClient.canPayBooking(booking, user), payNow.status === 201, `canPayBooking ${token}`);
    const getNow = await get(token);
    assert.equal(cli.BookingsClient.canGetBooking(booking, user), getNow.status === 200, `canGetBooking ${token}`);
  }
  // The same function is in the server package, and the policies are plain functions of both.
  assert.equal(srv.manageBooking(users.get('guest') as srv.User, booking), true);
  assert.equal(srv.manageBooking(users.get('other') as srv.User, booking), false);
  assert.equal(cli.anyRole(users.get('guest') as cli.User), false);
  assert.deepEqual(srv.manageBookingVerdict(users.get('bad') as srv.User, booking, 0n), {
    permitted: false,
    message: 'A suspended account cannot manage bookings',
  });

  server.close();
  console.log('policies over HTTP: ok');
}

void main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
