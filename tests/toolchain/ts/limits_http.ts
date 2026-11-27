// Rate limits (spec/platform.md section 5) over the production handler and the generated client of
// tests/toolchain/contract/platform_p1/limits.veld: the key a limiter is asked about, the last
// allowed call and the first refused one (the `429`, its body and its header), the next window,
// each of `user`, `tenant` and `ip`, a limiter that fails, a server created without the hook it
// needs, the memory limiter and its notice, the audit of a refused call, and the client: a single
// retry of a `get` and of an `@idempotent` call, no retry of another `post`, `retryOnLimit: false`.
// Expects `./server` and `./client` generated from that contract (compiled as the package
// `bookings`).
import assert from 'node:assert/strict';
import * as srv from './server/index';
import * as cli from './client/index';

const ID = '00000000-0000-4000-8000-000000000001';
const T1 = '00000000-0000-4000-8000-0000000000a1';
const principals: Record<string, { id: string; tenantId?: string }> = {
  ann: { id: '00000000-0000-4000-8000-0000000000c1', tenantId: T1.toUpperCase() },
  bob: { id: '00000000-0000-4000-8000-0000000000c2', tenantId: T1 },
  eve: { id: '00000000-0000-4000-8000-0000000000c3' },
};

let now = 1_000_000;
let address = '10.0.0.1';
const asked: { key: string; limit: number; windowSeconds: number }[] = [];
const failures: unknown[] = [];
const events: srv.veld.AuditEvent[] = [];
let broken = false;

/** The memory limiter with a clock the test controls, which also records what it is asked. */
const memory = new srv.veld.MemoryLimiter({ now: () => now });
const limiter: srv.veld.Limiter = {
  take(key, limit, windowSeconds) {
    asked.push({ key, limit, windowSeconds });
    if (broken) throw new Error('the limiter is down');
    return memory.take(key, limit, windowSeconds);
  },
};

const room = { id: ID };
const impl: srv.RoomsService = {
  byUser: async () => room,
  byTenant: async () => [room],
  archive: async () => undefined,
  create: async (req) => req.body,
  free: async () => [room],
};
const desksImpl: srv.DesksService = { mine: async () => [room] };

function rooms(extra: Record<string, unknown> = {}): srv.veld.VeldHandler {
  const hooks = {
    auth: { bearer: async (token: string) => principals[token] ?? null },
    idempotency: { store: new srv.veld.MemoryIdempotencyStore(), caller: async (p: { id: string }) => p.id },
    address: async () => address,
    limiter,
    audit: (e: srv.veld.AuditEvent) => {
      events.push(e);
    },
    onError: (err: unknown) => failures.push(err),
    ...extra,
  } as unknown as srv.RoomsHooks;
  return srv.createRoomsHandler(impl, hooks);
}

async function send(handler: srv.veld.VeldHandler, method: string, path: string, token?: string, body?: unknown, more: Record<string, string> = {}): Promise<srv.veld.VeldResponse> {
  const headers: Record<string, string> = { ...(token === undefined ? {} : { authorization: `Bearer ${token}` }), ...more };
  const res = await handler({ method, path, headers, ...(body === undefined ? {} : { body }) });
  assert.ok(res !== null, `${method} ${path} matched no route`);
  return res;
}

function keyOf(sig: string, by: string, identity: string): string {
  return `${sig}|${by}|${identity}`;
}

function sigOf(name: string): string {
  const found = asked.find((a) => a.key.includes(`.${name}@`));
  assert.ok(found !== undefined, `the limiter was asked about ${name}`);
  return found.key.split('|')[0] ?? '';
}

async function server(): Promise<void> {
  const handler = rooms();

  // By user: two calls an minute. The key is `operation|user|identity`.
  let res = await send(handler, 'GET', `/rooms/${ID}`, 'ann');
  assert.equal(res.status, 200);
  const sig = sigOf('byUser');
  assert.deepEqual(asked.at(-1), { key: keyOf(sig, 'user', principals['ann']?.id ?? ''), limit: 2, windowSeconds: 60 });
  res = await send(handler, 'GET', `/rooms/${ID}`, 'ann');
  assert.equal(res.status, 200, 'the last allowed call');

  // The first refused call: 429 TooManyRequests with `retryAfter` and the header.
  now += 10_000;
  events.length = 0;
  res = await send(handler, 'GET', `/rooms/${ID}`, 'ann');
  assert.equal(res.status, 429);
  const doc = JSON.parse(res.body ?? '{}') as { title: string; type: string; status: number; data: { retryAfter: number } };
  assert.equal(doc.title, 'TooManyRequests');
  assert.equal(doc.type, srv.TooManyRequests.sig);
  assert.equal(doc.data.retryAfter, 50, 'the time left in the window, rounded up');
  assert.equal(res.headers['retry-after'], '50');
  assert.equal(res.headers['content-type'], 'application/problem+json');

  // The audit log records the refused call, with the caller and nothing else.
  assert.equal(events.length, 1);
  assert.deepEqual(events[0]?.outcome, { status: 429, error: 'TooManyRequests' });
  assert.deepEqual(events[0]?.parameters, {});
  assert.deepEqual(events[0]?.facts, {});
  assert.deepEqual(events[0]?.principal, { id: principals['ann']?.id, tenantId: T1.toUpperCase() }, 'the principal as the auth hook returned it');
  assert.equal(events[0]?.result, undefined);

  // Another caller has a count of their own; a refused call did not count.
  res = await send(handler, 'GET', `/rooms/${ID}`, 'bob');
  assert.equal(res.status, 200, 'another caller');
  // The next window.
  now += 50_000;
  res = await send(handler, 'GET', `/rooms/${ID}`, 'ann');
  assert.equal(res.status, 200, 'the window is over');

  // An unauthenticated call is a 401 and costs nothing: the limit runs after authentication.
  asked.length = 0;
  res = await send(handler, 'GET', `/rooms/${ID}`, 'nobody');
  assert.equal(res.status, 401);
  assert.equal(asked.length, 0, 'the limiter was not asked');

  // By tenant: the tenant text of the principal (a uuid in lowercase), shared by two callers.
  asked.length = 0;
  for (const who of ['ann', 'bob', 'ann']) assert.equal((await send(handler, 'GET', '/rooms/tenant', who)).status, 200);
  assert.equal(asked.length, 3);
  assert.ok(asked.every((a) => a.key.endsWith(`|tenant|${T1}`)), 'the tenant text, in lowercase');
  assert.equal((await send(handler, 'GET', '/rooms/tenant', 'bob')).status, 429, 'the tenant is counted together');
  // A caller with no tenant has the empty identity: all of them are counted together.
  asked.length = 0;
  assert.equal((await send(handler, 'GET', '/rooms/tenant', 'eve')).status, 200);
  assert.ok(asked[0]?.key.endsWith('|tenant|'), 'the empty identity');

  // By address: a call from another address is another count.
  address = '10.0.0.1';
  assert.equal((await send(handler, 'POST', `/rooms/${ID}/archive`, 'ann')).status, 204);
  res = await send(handler, 'POST', `/rooms/${ID}/archive`, 'ann');
  assert.equal(res.status, 429);
  assert.equal(res.headers['retry-after'], '1');
  address = '10.0.0.2';
  assert.equal((await send(handler, 'POST', `/rooms/${ID}/archive`, 'ann')).status, 204, 'another address');
  assert.ok(asked.some((a) => a.key.endsWith('|ip|10.0.0.2')));

  // A limiter that fails lets the call through, and the failure goes to `onError`.
  broken = true;
  failures.length = 0;
  res = await send(handler, 'GET', '/rooms/free', 'ann');
  assert.equal(res.status, 200, 'an endpoint with no limit does not ask');
  res = await send(handler, 'GET', `/rooms/${ID}`, 'ann');
  assert.equal(res.status, 200, 'the call is allowed');
  assert.equal(failures.length, 1);
  assert.match(String((failures[0] as Error).message), /the limiter is down/);
  // So does a hook that cannot name the address.
  broken = false;
  failures.length = 0;
  const flaky = rooms({
    address: async () => {
      throw new Error('no proxy header');
    },
  });
  now += 120_000;
  assert.equal((await send(flaky, 'POST', `/rooms/${ID}/archive`, 'ann')).status, 204);
  assert.equal(failures.length, 1);

  // A server created without the hook it needs does not start.
  assert.throws(() => rooms({ address: undefined }), /hooks\.address is required/);
  assert.throws(
    () =>
      srv.createDesksHandler(desksImpl, { auth: { bearer: async () => principals['ann'] ?? null } } as unknown as srv.DesksHooks),
    /hooks\.caller is required/,
  );

  // The caller hook of a service with no `@idempotent` endpoint is a hook of its own.
  const desks = srv.createDesksHandler(desksImpl, {
    auth: { bearer: async (t: string) => principals[t] ?? null },
    caller: async (p) => (p as { id: string }).id,
    limiter,
  });
  assert.equal((await send(desks, 'GET', '/desks', 'ann')).status, 200);
  assert.equal((await send(desks, 'GET', '/desks', 'ann')).status, 429);
  assert.equal((await send(desks, 'GET', '/desks', 'bob')).status, 200);

  // The memory limiter and its notice: one at start-up, none when the application gives a limiter.
  const warned: string[] = [];
  const original = console.warn;
  console.warn = (message?: unknown) => {
    warned.push(String(message));
  };
  try {
    const open = srv.createPublicHandler({ listing: async () => [room] }, { address: async () => 'a' });
    assert.equal(warned.length, 1, 'one notice at start-up');
    assert.match(warned[0] ?? '', /memory/);
    // The limit a service gives its endpoints: 2 a second per address.
    assert.equal((await send(open, 'GET', '/public')).status, 200);
    assert.equal((await send(open, 'GET', '/public')).status, 200);
    assert.equal((await send(open, 'GET', '/public')).status, 429);
    warned.length = 0;
    srv.createPublicHandler({ listing: async () => [room] }, { address: async () => 'a', limiter });
    assert.equal(warned.length, 0, 'no notice when the application has a limiter');
  } finally {
    console.warn = original;
  }
}

/** A `fetch` that hands every request to the production handler, and counts what it sent. */
function bridge(handler: srv.veld.VeldHandler, sent: string[]): NonNullable<ConstructorParameters<typeof cli.RoomsClient>[0]['fetch']> {
  return async (url, init) => {
    const u = new URL(url);
    sent.push(`${init.method} ${u.pathname}`);
    const res = await handler({
      method: init.method,
      path: u.pathname,
      query: u.search.slice(1),
      headers: init.headers,
      ...(init.body === undefined ? {} : { body: JSON.parse(init.body) as unknown }),
    });
    assert.ok(res !== null);
    return { status: res.status, text: async () => res.body ?? '' };
  };
}

async function client(): Promise<void> {
  const handler = rooms();
  const sent: string[] = [];
  const slept: number[] = [];
  const options = {
    baseUrl: 'http://rooms.test',
    fetch: bridge(handler, sent),
    auth: { bearer: () => 'ann' },
    // A sleep that does not wait: it moves the clock of the limiter by what it was asked to wait.
    sleep: async (ms: number) => {
      slept.push(ms);
      now += ms;
    },
  };
  const rooms1 = new cli.RoomsClient(options);

  // A `get` that is refused waits `retryAfter` and sends again, once.
  now += 1_000_000;
  await rooms1.byUser({ id: ID });
  await rooms1.byUser({ id: ID });
  sent.length = 0;
  const got = await rooms1.byUser({ id: ID });
  assert.equal(got.id, ID);
  assert.equal(sent.length, 2, 'the call was sent twice');
  assert.deepEqual(slept, [60_000], 'it waited what the server said');

  // A `post` that is not idempotent raises the typed error at once.
  slept.length = 0;
  sent.length = 0;
  await rooms1.archive({ id: ID });
  await assert.rejects(rooms1.archive({ id: ID }), (e: unknown) => {
    assert.ok(e instanceof cli.TooManyRequests, String(e));
    assert.equal(e.data.retryAfter, 1);
    return true;
  });
  assert.equal(sent.length, 2, 'sent once each');
  assert.equal(slept.length, 0, 'it did not wait');

  // An `@idempotent` post waits and sends again, with the same key.
  slept.length = 0;
  now += 10_000;
  await rooms1.create({ body: room, idempotencyKey: 'k1' });
  const made = await rooms1.create({ body: room, idempotencyKey: 'k2' });
  assert.equal(made.id, ID);
  assert.deepEqual(slept, [1000]);

  // `retryOnLimit: false` turns the wait off.
  slept.length = 0;
  const impatient = new cli.RoomsClient({ ...options, retryOnLimit: false });
  now += 1_000_000;
  await impatient.byUser({ id: ID });
  await impatient.byUser({ id: ID });
  await assert.rejects(impatient.byUser({ id: ID }), (e: unknown) => e instanceof cli.TooManyRequests);
  assert.equal(slept.length, 0);

  // A single retry: when the second answer is a `429` too it is raised, not waited for again.
  slept.length = 0;
  sent.length = 0;
  const stubborn = new cli.RoomsClient({
    ...options,
    sleep: async (ms: number) => {
      slept.push(ms);
    },
  });
  now += 1_000_000;
  await stubborn.byUser({ id: ID });
  await stubborn.byUser({ id: ID });
  sent.length = 0;
  await assert.rejects(stubborn.byUser({ id: ID }), (e: unknown) => e instanceof cli.TooManyRequests);
  assert.equal(sent.length, 2, 'sent twice, no more');
  assert.equal(slept.length, 1);

  // A `429` without `retryAfter` is raised at once.
  const noHint = new cli.RoomsClient({
    baseUrl: 'http://rooms.test',
    fetch: async () => ({
      status: 429,
      text: async () => JSON.stringify({ type: cli.TooManyRequests.sig, title: 'TooManyRequests', status: 429, operation: 'x', data: {} }),
    }),
    sleep: async (ms: number) => {
      slept.push(ms);
    },
  });
  slept.length = 0;
  await assert.rejects(noHint.byUser({ id: ID }), (e: unknown) => e instanceof cli.TooManyRequests);
  assert.equal(slept.length, 0);
}

async function main(): Promise<void> {
  await server();
  await client();
  console.log('limits_http: ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
