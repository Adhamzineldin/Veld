// `validate = false` switches off neither the version guard nor the syntax of `If-Match` and of the
// idempotency key (spec/rules.md section 11.10): they are part of what an endpoint does, not
// validation of its parameters. Expects `./server` generated from
// tests/rules/endpoints/idempotency.veld.pending with the target option `validate = false`.
import assert from 'node:assert/strict';
import * as srv from './server/index';

const ID = '00000000-0000-4000-8000-000000000001';
const calls: string[] = [];
const shelves = new Map<string, srv.Shelf>([[ID, { id: ID, name: 'Red', revision: '4' }]]);

const impl: srv.BookingsService = {
  async book(req) {
    calls.push('book');
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
    calls.push(`saveRoom ${String(req.version)}`);
    return { ...req.shelf, name: req.body.name };
  },
};

const handler = srv.createBookingsHandler(impl, {
  facts: {
    async room() {
      return { id: ID, capacity: 1 };
    },
    async shelf(key) {
      return shelves.get(key);
    },
  },
  idempotency: { store: new srv.veld.MemoryIdempotencyStore(), caller: () => 'caller' },
  auth: { bearer: async () => ({}) },
});

interface Doc {
  data: { issues?: { path: string; code: string }[] };
  violations?: unknown[];
}

async function call(method: string, path: string, headers: Record<string, string>, body: unknown) {
  const res = await handler({ method, path, headers, body });
  assert.ok(res !== null);
  return { status: res.status, headers: res.headers, doc: JSON.parse(res.body ?? '{}') as Doc };
}

async function main(): Promise<void> {
  // Validation really is off: a body that breaks its rule, and a `require` that would fail, are not checked.
  const unchecked = await call('POST', '/bookings', { 'idempotency-key': 'k-1' }, { guests: 0 });
  assert.equal(unchecked.status, 201);
  assert.deepEqual(calls, ['book']);

  // The key and its replay work as with validation.
  const replayed = await call('POST', '/bookings', { 'idempotency-key': 'k-1' }, { guests: 0 });
  assert.equal(replayed.status, 201);
  assert.equal(replayed.headers['veld-idempotent-replay'], 'true');
  assert.deepEqual(calls, ['book']);
  const other = await call('POST', '/bookings', { 'idempotency-key': 'k-1' }, { guests: 3 });
  assert.equal(other.status, 409);

  // A malformed key, and a required key that is missing, are still `422`.
  const bad = await call('POST', '/bookings', { 'idempotency-key': 'a b' }, { guests: 2 });
  assert.equal(bad.status, 422);
  assert.deepEqual(bad.doc.data.issues?.map((i) => i.path), ['header.Idempotency-Key']);
  assert.equal(bad.doc.violations, undefined);
  const missing = await call('POST', '/bookings/paid', {}, { guests: 2 });
  assert.equal(missing.status, 422);
  assert.deepEqual(missing.doc.data.issues?.map((i) => i.code), ['required']);

  // The version guard still runs, and so does the syntax of `If-Match`.
  calls.length = 0;
  const stale = await call('PUT', `/bookings/rooms/${ID}`, { 'if-match': '"3"' }, { name: 'Blue' });
  assert.equal(stale.status, 412);
  const malformed = await call('PUT', `/bookings/rooms/${ID}`, { 'if-match': '3' }, { name: 'Blue' });
  assert.equal(malformed.status, 422);
  assert.deepEqual(malformed.doc.data.issues?.map((i) => i.path), ['header.If-Match']);
  const current = await call('PUT', `/bookings/rooms/${ID}`, { 'if-match': '"4"' }, { name: 'Blue' });
  assert.equal(current.status, 200);
  assert.equal(current.headers['etag'], '"4"');
  assert.deepEqual(calls, ['saveRoom 4']);
  console.log('idempotency without validation ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
