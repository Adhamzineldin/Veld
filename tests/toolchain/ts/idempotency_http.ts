// @idempotent over real HTTP (spec/rules.md sections 11.8, 11.9, 11.10 and 11.12): the production
// handler of a generated Node server, the store it is given, and the generated client. Expects
// `./server` and `./client` generated from tests/rules/endpoints/idempotency.veld.pending.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { VeldHandler } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000001';
const MIXED = '0000000a-0000-4000-8000-00000000000b';
const SIG = /^veld:bookings\/Bookings\.[a-zA-Z]+@[0-9a-f]{16}$/;
const OTHER = 'this idempotency key was used with other parameters';
const RUNNING = 'a request with this idempotency key is still running';
const KEY_ISSUE = { path: 'header.Idempotency-Key', code: 'type', message: 'must be 1 to 255 visible ASCII characters' };

/** What the store was asked, and failures on demand. */
class SpyStore implements srv.veld.IdempotencyStore {
  readonly inner = new srv.veld.MemoryIdempotencyStore();
  readonly log: { op: 'begin' | 'complete' | 'release'; scope: string; key: string; fingerprint?: string; status?: number }[] = [];
  failBegin = false;
  failComplete = false;
  failRelease = false;

  async begin(scope: string, key: string, fingerprint: string): Promise<srv.veld.IdempotencyBegin> {
    this.log.push({ op: 'begin', scope, key, fingerprint });
    if (this.failBegin) throw new Error('begin failed');
    return this.inner.begin(scope, key, fingerprint);
  }

  async complete(scope: string, key: string, answer: srv.veld.StoredAnswer): Promise<void> {
    this.log.push({ op: 'complete', scope, key, status: answer.status });
    if (this.failComplete) throw new Error('complete failed');
    return this.inner.complete(scope, key, answer);
  }

  async release(scope: string, key: string): Promise<void> {
    this.log.push({ op: 'release', scope, key });
    if (this.failRelease) throw new Error('release failed');
    return this.inner.release(scope, key);
  }
}

// The application: it counts what runs, and can be held or made to fail.
let calls: string[] = [];
let loads: string[] = [];
let versions: (string | undefined)[] = [];
let errors: unknown[] = [];
let capacity = 9;
let failures = 0;
let gate: Promise<void> | null = null;
let entered: () => void = () => undefined;
const shelves = new Map<string, srv.Shelf>();

const impl: srv.BookingsService = {
  async book(req) {
    calls.push('book');
    entered();
    if (gate !== null && req.body.guests === 5) await gate;
    if (failures > 0 && req.body.guests === 7) {
      failures -= 1;
      throw new Error('the handler failed');
    }
    return { id: ID, guests: req.body.guests };
  },
  async pay(req) {
    calls.push('pay');
    return { id: ID, guests: req.body.guests };
  },
  async replace(req) {
    calls.push('replace');
    return { id: req.id, guests: req.body.guests };
  },
  async cancel() {
    calls.push('cancel');
  },
  async bookMine(req) {
    calls.push('bookMine');
    return { id: ID, guests: req.body.guests };
  },
  async saveRoom(req) {
    calls.push('saveRoom');
    versions.push(req.version);
    return { ...req.shelf, name: req.body.name, revision: String(BigInt(req.shelf.revision) + 1n) };
  },
};

const loaders: srv.BookingsFactLoaders = {
  async room() {
    loads.push('room');
    return { id: ID, capacity };
  },
  async shelf(key) {
    loads.push('shelf');
    return shelves.get(key);
  },
};

function hooks(store: srv.veld.IdempotencyStore): srv.BookingsHooks {
  return {
    facts: loaders,
    idempotency: { store, caller: (principal) => String((principal as { name: string }).name) },
    auth: { bearer: async (token) => (token === 'alice' || token === 'bob' ? { name: token } : null) },
    onError: (err) => errors.push(err),
  };
}

function reset(): void {
  calls = [];
  loads = [];
  versions = [];
  errors = [];
  capacity = 9;
  failures = 0;
  gate = null;
  entered = () => undefined;
  shelves.clear();
}

/** The idempotency header of every request the server saw. */
const keysSeen: (string | undefined)[] = [];
const requestsSeen: Record<string, string | string[] | undefined>[] = [];

function recording(handler: VeldHandler): VeldHandler {
  const wrapped = async (req: Parameters<VeldHandler>[0]): ReturnType<VeldHandler> => {
    const key = req.headers['idempotency-key'];
    keysSeen.push(Array.isArray(key) ? key[0] : key);
    requestsSeen.push(req.headers);
    return handler(req);
  };
  return Object.assign(wrapped, { ws: handler.ws });
}

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

interface Answer {
  status: number;
  text: string;
  replay: string | null;
  etag: string | null;
  contentType: string | null;
  doc: Doc;
}

async function send(
  url: string,
  method: string,
  path: string,
  init: { headers?: Record<string, string>; body?: unknown } = {},
): Promise<Answer> {
  const res = await fetch(`${url}${path}`, {
    method,
    headers: { ...(init.body === undefined ? {} : { 'content-type': 'application/json' }), ...init.headers },
    ...(init.body === undefined ? {} : { body: JSON.stringify(init.body) }),
  });
  const text = await res.text();
  return {
    status: res.status,
    text,
    replay: res.headers.get('veld-idempotent-replay'),
    etag: res.headers.get('etag'),
    contentType: res.headers.get('content-type'),
    doc: (text === '' ? {} : JSON.parse(text)) as Doc,
  };
}

function book(url: string, key: string | undefined, guests: number): Promise<Answer> {
  const headers: Record<string, string> = key === undefined ? {} : { 'idempotency-key': key };
  return send(url, 'POST', '/bookings', { headers, body: { guests } });
}

async function main(): Promise<void> {
  const spy = new SpyStore();
  const server = await listen(recording(srv.createBookingsHandler(impl, hooks(spy))));
  const url = server.url;
  const begins = (): SpyStore['log'] => spy.log.filter((e) => e.op === 'begin');

  // A new key runs the request and stores the answer; the fingerprint is the one of the reference.
  reset();
  const first = await book(url, 'k-1', 2);
  assert.equal(first.status, 201);
  assert.equal(first.replay, null);
  assert.deepEqual(calls, ['book']);
  assert.deepEqual(loads, ['room']);
  assert.match(begins()[0]?.scope ?? '', SIG);
  assert.ok(!(begins()[0]?.scope ?? '').includes('\n'), 'an endpoint without auth has no caller in its scope');
  assert.equal(begins()[0]?.fingerprint, '74ff6d8fbb076cfc');
  assert.deepEqual(spy.log.map((e) => e.op), ['begin', 'complete']);
  assert.equal(spy.log[1]?.status, 201);

  // The same key and the same parameters: the stored answer again, nothing loaded, the handler not called.
  const again = await book(url, 'k-1', 2);
  assert.equal(again.status, 201);
  assert.equal(again.text, first.text);
  assert.equal(again.contentType, first.contentType);
  assert.equal(again.replay, 'true');
  assert.deepEqual(calls, ['book']);
  assert.deepEqual(loads, ['room']);

  // The same key with other parameters: 409 Conflict, with the message that says why. Nothing runs.
  const other = await book(url, 'k-1', 3);
  assert.equal(other.status, 409);
  assert.equal(other.doc.title, 'Conflict');
  assert.equal(other.doc.type, srv.Conflict.sig);
  assert.deepEqual(other.doc.data, { message: OTHER });
  assert.equal(other.doc.violations, undefined);
  assert.equal(other.replay, null);
  assert.equal(begins()[2]?.fingerprint, '19b1b33ecd669a6f');
  assert.deepEqual(calls, ['book']);
  assert.deepEqual(loads, ['room']);

  // A request without a key is not idempotent: it runs every time and the store is not asked.
  const asked = spy.log.length;
  assert.equal((await book(url, undefined, 2)).status, 201);
  assert.equal((await book(url, undefined, 2)).status, 201);
  assert.equal(spy.log.length, asked);
  assert.deepEqual(calls, ['book', 'book', 'book']);

  // A second request while the first is running: 409, and the first one still finishes and is stored.
  reset();
  let release: () => void = () => undefined;
  gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const started = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const slow = book(url, 'k-2', 5);
  await started;
  const busy = await book(url, 'k-2', 5);
  assert.equal(busy.status, 409);
  assert.deepEqual(busy.doc.data, { message: RUNNING });
  release();
  const done = await slow;
  assert.equal(done.status, 201);
  const stored = await book(url, 'k-2', 5);
  assert.equal(stored.replay, 'true');
  assert.equal(stored.text, done.text);
  assert.deepEqual(calls, ['book']);

  // A declared error is stored and replayed too, byte for byte, with its violation.
  reset();
  capacity = 1;
  const full = await book(url, 'k-3', 2);
  assert.equal(full.status, 409);
  assert.equal(full.doc.title, 'RoomFull');
  assert.deepEqual(full.doc.data, { capacity: 1 });
  assert.equal(full.doc.violations?.length, 1);
  const fullAgain = await book(url, 'k-3', 2);
  assert.equal(fullAgain.status, 409);
  assert.equal(fullAgain.text, full.text);
  assert.equal(fullAgain.replay, 'true');
  assert.deepEqual(loads, ['room']);
  assert.deepEqual(calls, []);

  // An error of step 6 that is not declared by the author is stored as well: `NotFound`, and `412`.
  reset();
  const put = (key: string, ifMatch?: string): Promise<Answer> =>
    send(url, 'PUT', `/bookings/rooms/${ID}`, {
      headers: { 'idempotency-key': key, ...(ifMatch === undefined ? {} : { 'if-match': ifMatch }) },
      body: { name: 'Blue' },
    });
  const missing = await put('s-404');
  assert.equal(missing.status, 404);
  shelves.set(ID, { id: ID, name: 'Red', revision: '4' });
  const missingAgain = await put('s-404');
  assert.equal(missingAgain.status, 404);
  assert.equal(missingAgain.replay, 'true');
  assert.equal(missingAgain.text, missing.text);
  assert.deepEqual(calls, []);

  // A 412 is an answer below 500: stored, and replayed even when the tag the retry sends is the current one.
  const stale = await put('s-412', '"3"');
  assert.equal(stale.status, 412);
  assert.deepEqual(stale.doc.data, { expected: '3', actual: '4' });
  const staleAgain = await put('s-412', '"4"');
  assert.equal(staleAgain.status, 412);
  assert.equal(staleAgain.replay, 'true');
  assert.equal(staleAgain.text, stale.text);
  assert.deepEqual(calls, []);

  // A success carries the ETag, and so does its replay; the lookup is before the version guard.
  const saved = await put('s-ok', '"4"');
  assert.equal(saved.status, 200);
  assert.equal(saved.etag, '"5"');
  assert.deepEqual(versions, ['4']);
  const loaded = loads.length;
  const savedAgain = await put('s-ok', '"4"');
  assert.equal(savedAgain.replay, 'true');
  assert.equal(savedAgain.etag, '"5"');
  assert.equal(savedAgain.text, saved.text);
  const savedStale = await put('s-ok', '"9"');
  assert.equal(savedStale.status, 200, 'a replay does not check If-Match');
  assert.equal(savedStale.replay, 'true');
  assert.equal(loads.length, loaded, 'a replay loads no fact');
  assert.deepEqual(calls, ['saveRoom']);
  // A new key with a stale tag is refused by the guard, as without a key.
  assert.equal((await put('s-new', '"9"')).status, 412);

  // A 500 is not stored: the entry is removed, and the next request with the key runs again.
  reset();
  spy.log.length = 0;
  failures = 1;
  const broken = await book(url, 'k-4', 7);
  assert.equal(broken.status, 500);
  assert.equal(errors.length, 1);
  assert.deepEqual(spy.log.map((e) => e.op), ['begin', 'release']);
  const retried = await book(url, 'k-4', 7);
  assert.equal(retried.status, 201);
  assert.equal(retried.replay, null);
  assert.deepEqual(calls, ['book', 'book']);

  // A required key that is missing, and a key that is malformed: 422, no violations, the store is not asked.
  reset();
  spy.log.length = 0;
  const unkeyed = await send(url, 'POST', '/bookings/paid', { body: { guests: 2 } });
  assert.equal(unkeyed.status, 422);
  assert.deepEqual(unkeyed.doc.data, { issues: [{ path: 'header.Idempotency-Key', code: 'required', message: 'is required' }] });
  assert.equal(unkeyed.doc.violations, undefined);
  // The issue of the header comes before the issues of the rules the body breaks.
  const both = await send(url, 'POST', '/bookings/paid', { body: { guests: 0 } });
  assert.equal(both.status, 422);
  const issues = both.doc.data['issues'] as { path: string; code: string }[];
  assert.equal(issues.length, 2);
  assert.deepEqual(issues[0], { path: 'header.Idempotency-Key', code: 'required', message: 'is required' });
  assert.equal(issues[1]?.code, 'rule');
  assert.equal(both.doc.violations?.length, 1);
  for (const bad of ['a b', 'x'.repeat(256)]) {
    const r = await book(url, bad, 2);
    assert.equal(r.status, 422, bad);
    assert.deepEqual(r.doc.data, { issues: [KEY_ISSUE] }, bad);
    assert.equal(r.doc.violations, undefined);
  }
  const maxed = await book(url, 'x'.repeat(255), 2);
  assert.equal(maxed.status, 201, 'a key of 255 characters is a key');
  const named = await send(url, 'PUT', `/bookings/${ID}`, { headers: { 'x-request-id': 'a b' }, body: { guests: 2 } });
  assert.deepEqual(named.doc.data, { issues: [{ ...KEY_ISSUE, path: 'header.X-Request-Id' }] });
  const mineMissing = await send(url, 'POST', '/bookings/mine', { headers: { authorization: 'Bearer alice' }, body: { guests: 2 } });
  assert.deepEqual(mineMissing.doc.data, { issues: [{ path: 'header.Idempotency-Token', code: 'required', message: 'is required' }] });
  assert.deepEqual(calls, ['book']);
  assert.deepEqual(spy.log.map((e) => e.op), ['begin', 'complete']);

  // The keys a client cannot send (an empty one, controls, non-ASCII): the handler is called directly.
  const direct = srv.createBookingsHandler(impl, hooks(spy));
  for (const bad of ['', 'a\tb', 'café', 'a\u007fb', '\u0001']) {
    const res = await direct({ method: 'POST', path: '/bookings', headers: { 'idempotency-key': bad }, body: { guests: 2 } });
    assert.equal(res?.status, 422, JSON.stringify(bad));
    assert.deepEqual((JSON.parse(res?.body ?? '{}') as Doc).data, { issues: [KEY_ISSUE] });
  }

  // Authentication comes first: no caller, no look at the key.
  reset();
  spy.log.length = 0;
  const anon = await send(url, 'POST', '/bookings/mine', { headers: { 'idempotency-token': 't-0' }, body: { guests: 2 } });
  assert.equal(anon.status, 401);
  assert.equal(spy.log.length, 0);

  // Two callers with the same key do not replay each other: the caller is part of the scope.
  const mine = (who: string, key: string): Promise<Answer> =>
    send(url, 'POST', '/bookings/mine', { headers: { authorization: `Bearer ${who}`, 'idempotency-token': key }, body: { guests: 2 } });
  const alice = await mine('alice', 't-1');
  const bob = await mine('bob', 't-1');
  assert.equal(alice.status, 201);
  assert.equal(bob.status, 201);
  assert.equal(bob.replay, null);
  assert.deepEqual(calls, ['bookMine', 'bookMine']);
  const aliceAgain = await mine('alice', 't-1');
  assert.equal(aliceAgain.replay, 'true');
  assert.deepEqual(calls, ['bookMine', 'bookMine']);
  const scopes = begins().map((e) => e.scope);
  assert.match(scopes[0] ?? '', /^veld:bookings\/Bookings\.bookMine@[0-9a-f]{16}\nalice$/);
  assert.match(scopes[1] ?? '', /\nbob$/);
  assert.equal(scopes[0], scopes[2]);

  // An answer without a body (204) is stored and replayed too.
  reset();
  const del = (key: string): Promise<Answer> => send(url, 'DELETE', `/bookings/${ID}`, { headers: { 'idempotency-key': key } });
  const cancelled = await del('c-1');
  assert.equal(cancelled.status, 204);
  const cancelledAgain = await del('c-1');
  assert.equal(cancelledAgain.status, 204);
  assert.equal(cancelledAgain.text, '');
  assert.equal(cancelledAgain.replay, 'true');
  assert.deepEqual(calls, ['cancel']);

  // The fingerprint is taken on the normalised value: a uuid in capitals is the same parameter.
  reset();
  spy.log.length = 0;
  const lower = await send(url, 'PUT', `/bookings/${MIXED}`, { headers: { 'x-request-id': 'u-1' }, body: { guests: 2 } });
  assert.equal(lower.status, 200);
  const upper = await send(url, 'PUT', `/bookings/${MIXED.toUpperCase()}`, { headers: { 'x-request-id': 'u-1' }, body: { guests: 2 } });
  assert.equal(upper.replay, 'true');
  assert.equal(begins()[0]?.fingerprint, begins()[1]?.fingerprint);

  // A store that fails: `begin` is 500 and the handler does not run; `complete` and `release` go to onError.
  reset();
  spy.failBegin = true;
  const unavailable = await book(url, 'f-1', 2);
  assert.equal(unavailable.status, 500);
  assert.deepEqual(calls, []);
  assert.deepEqual(loads, []);
  assert.equal(errors.length, 1);
  spy.failBegin = false;
  spy.failComplete = true;
  const unsaved = await book(url, 'f-2', 2);
  assert.equal(unsaved.status, 201, 'the answer is sent as it is');
  assert.equal(errors.length, 2);
  spy.failComplete = false;
  spy.failRelease = true;
  failures = 1;
  const unreleased = await book(url, 'f-3', 7);
  assert.equal(unreleased.status, 500);
  assert.equal(errors.length, 4);
  spy.failRelease = false;

  // A server without what it needs does not start, and names the endpoint that needs it.
  const bare = { facts: loaders, auth: { bearer: async () => null } };
  assert.throws(() => srv.createBookingsHandler(impl, bare as unknown as srv.BookingsHooks), /book: hooks\.idempotency\.store is required/);
  const noCaller = { ...bare, idempotency: { store: new srv.veld.MemoryIdempotencyStore() } };
  assert.throws(() => srv.createBookingsHandler(impl, noCaller as unknown as srv.BookingsHooks), /bookMine: hooks\.idempotency\.caller is required/);
  // The type says so too.
  // @ts-expect-error the hooks of a service with an @idempotent endpoint need `idempotency`
  const untyped: srv.BookingsHooks = { facts: loaders, auth: { bearer: async () => null } };
  void untyped;

  // The generated client sends the key in the header the annotation names, and never invents one.
  reset();
  keysSeen.length = 0;
  requestsSeen.length = 0;
  const client = new cli.BookingsClient({ baseUrl: url });
  const made = await client.book({ body: { guests: 2 }, idempotencyKey: 'cl-1' });
  assert.equal(made.guests, 2);
  assert.equal(keysSeen[0], 'cl-1');
  await client.book({ body: { guests: 2 } });
  assert.equal(keysSeen[1], undefined);
  await client.pay({ body: { guests: 2 }, idempotencyKey: 'cl-2' });
  assert.equal(keysSeen[2], 'cl-2');
  await client.replace({ id: ID, body: { guests: 2 }, idempotencyKey: 'cl-3' });
  assert.equal(requestsSeen[3]?.['x-request-id'], 'cl-3');
  assert.equal(keysSeen[3], undefined);
  await client.cancel({ id: ID });
  assert.equal(requestsSeen[4]?.['x-request-id'], undefined);
  // A retry with the same key is answered from the store: the application ran once.
  await client.book({ body: { guests: 4 }, idempotencyKey: 'cl-4' });
  await client.book({ body: { guests: 4 }, idempotencyKey: 'cl-4' });
  assert.deepEqual(calls.filter((c) => c === 'book'), ['book', 'book', 'book']);
  // A required key is required by the type, and a key the server does not like is a typed 422.
  // @ts-expect-error `pay` is `@idempotent(required: true)`: the key is part of its request type
  void (() => client.pay({ body: { guests: 2 } }));
  await assert.rejects(client.book({ body: { guests: 2 }, idempotencyKey: 'a b' }), (e: unknown) => e instanceof cli.ValidationError);
  await assert.rejects(client.book({ body: { guests: 9 }, idempotencyKey: 'cl-4' }), (e: unknown) => e instanceof cli.Conflict);

  server.close();
  console.log('idempotency over http ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
