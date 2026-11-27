// @version over real HTTP (spec/rules.md sections 11.7, 11.9, 11.10 and 11.12): the production
// handlers of a generated Node server, and the generated client. Expects `./server` and `./client`
// generated from tests/rules/endpoints/version.veld.pending (both services).
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000002';
const STAFF: srv.User = { id: '00000000-0000-4000-8000-0000000000aa', roles: ['staff'] };
const GUEST: srv.User = { id: '00000000-0000-4000-8000-0000000000bb', roles: [] };
const MALFORMED = 'must be * or one strong entity tag holding a version, such as "3"';

const rooms = new Map<string, srv.Room>();
const counters = new Map<string, srv.Counter>();
const orders = new Map<string, srv.Order>();
const seen: { name: string; version: unknown; keys: string[]; transition?: unknown }[] = [];
const sent: { method: string; path: string; ifMatch: string | undefined }[] = [];

function next(version: string): string {
  return String(BigInt(version) + 1n);
}

const roomsImpl: srv.RoomsService = {
  async rename(req) {
    seen.push({ name: 'rename', version: req.version, keys: Object.keys(req).sort() });
    return { ...req.room, name: req.body.name, revision: next(req.room.revision) };
  },
  async touch(req) {
    seen.push({ name: 'touch', version: req.version, keys: Object.keys(req).sort() });
    return req.maybe === undefined ? { id: req.id, name: 'new', revision: '1' } : { ...req.maybe, revision: next(req.maybe.revision) };
  },
  async tick(req) {
    seen.push({ name: 'tick', version: req.version, keys: Object.keys(req).sort() });
    return { ...req.counter, total: req.counter.total + 1, version: req.counter.version + 1 };
  },
  async getRoom(req) {
    seen.push({ name: 'getRoom', version: undefined, keys: Object.keys(req).sort() });
    return rooms.get(req.id) ?? { id: req.id, name: 'none', revision: '0' };
  },
};

const ordersImpl: srv.OrdersService = {
  async payOrder(req) {
    seen.push({ name: 'payOrder', version: req.version, keys: Object.keys(req).sort(), transition: req.transition });
    return { ...req.order, status: 'paid', revision: next(req.order.revision) };
  },
};

const roomLoaders: srv.RoomsFactLoaders = {
  async room(key) {
    return rooms.get(key);
  },
  async maybe(key) {
    return rooms.get(key);
  },
  async counter(key) {
    return counters.get(key);
  },
};
const orderLoaders: srv.OrdersFactLoaders = {
  async order(key) {
    return orders.get(key);
  },
};

/** Notes the `If-Match` of every request, then hands it to the handler. */
function recording(handler: VeldHandler): VeldHandler {
  const wrapped = async (req: Parameters<VeldHandler>[0]): ReturnType<VeldHandler> => {
    const tag = req.headers['if-match'];
    sent.push({ method: req.method, path: req.path, ifMatch: Array.isArray(tag) ? tag[0] : tag });
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
  etag: string | null;
  doc: Doc;
}

async function send(url: string, method: string, path: string, init: { headers?: Record<string, string>; body?: unknown } = {}): Promise<Answer> {
  const res = await fetch(`${url}${path}`, {
    method,
    headers: { ...(init.body === undefined ? {} : { 'content-type': 'application/json' }), ...init.headers },
    ...(init.body === undefined ? {} : { body: JSON.stringify(init.body) }),
  });
  const text = await res.text();
  return { status: res.status, etag: res.headers.get('etag'), doc: (text === '' ? {} : JSON.parse(text)) as Doc };
}

function reset(): void {
  rooms.clear();
  counters.clear();
  orders.clear();
  seen.length = 0;
  sent.length = 0;
}

function put(revision: string): void {
  reset();
  rooms.set(ID, { id: ID, name: 'Red', revision });
}

function rename(url: string, ifMatch?: string): Promise<Answer> {
  const headers: Record<string, string> = ifMatch === undefined ? {} : { 'if-match': ifMatch };
  return send(url, 'PUT', `/rooms/${ID}`, { headers, body: { name: 'Blue' } });
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  const roomsServer = await listen(
    recording(srv.createRoomsHandler(roomsImpl, { facts: roomLoaders, onRuleViolation: (e) => events.push(e) })),
  );
  const ordersServer = await listen(
    srv.createOrdersHandler(ordersImpl, {
      facts: orderLoaders,
      auth: { bearer: async (token) => (token === 'staff' ? STAFF : token === 'guest' ? GUEST : null) },
      onRuleViolation: (e) => events.push(e),
    }),
  );
  const url = roomsServer.url;

  // A tag that is the current version: the handler runs and receives the version it was compared with.
  put('4');
  let r = await rename(url, '"4"');
  assert.equal(r.status, 200);
  assert.deepEqual(seen[0]?.version, '4');
  assert.deepEqual(seen[0]?.keys, ['body', 'id', 'room', 'version']);
  // The ETag is the version of the value the handler returned.
  assert.equal(r.etag, '"5"');

  // A stale tag: 412 with expected and actual as decimal strings, one violation, the handler never runs.
  put('4');
  events.length = 0;
  r = await rename(url, '"3"');
  assert.equal(r.status, 412);
  assert.equal(r.doc.title, 'PreconditionFailed');
  assert.equal(r.doc.type, srv.PreconditionFailed.sig);
  assert.deepEqual(r.doc.data, { expected: '3', actual: '4' });
  assert.equal(r.doc.violations?.length, 1);
  const v = r.doc.violations?.[0];
  assert.match(v?.rule ?? '', /^veld:bookings\/Rooms\.rename\.version@[0-9a-f]{16}$/);
  assert.equal(v?.path, 'request');
  assert.deepEqual(v?.fields, []);
  assert.equal(v?.message, 'violates rule version');
  assert.equal(seen.length, 0);
  assert.equal(r.etag, null);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.position, 'input');
  assert.equal(events[0]?.outcome, 'rejected');
  assert.equal(events[0]?.violation.rule, v?.rule);

  // A tag ahead of the row is stale too; the largest long is a version.
  r = await rename(url, '"9"');
  assert.equal(r.status, 412);
  assert.deepEqual(r.doc.data, { expected: '9', actual: '4' });
  put('9223372036854775807');
  r = await rename(url, '"9223372036854775807"');
  assert.equal(r.status, 200, JSON.stringify(r.doc));
  assert.equal(seen[0]?.version, '9223372036854775807');

  // `*` and no header pass, and hand the handler no version.
  for (const tag of ['*', undefined]) {
    put('4');
    r = await rename(url, tag);
    assert.equal(r.status, 200);
    assert.equal(seen[0]?.version, undefined);
    assert.deepEqual(seen[0]?.keys, ['body', 'id', 'room']);
    assert.equal(r.etag, '"5"');
  }

  // Version zero is a version.
  put('0');
  assert.equal((await rename(url, '"0"')).status, 200);
  assert.equal(seen[0]?.version, '0');

  // Every malformed form is `422` with one issue at header.If-Match, no violations, and the handler never runs.
  put('4');
  for (const bad of ['3', 'W/"3"', '"3", "4"', '"03"', '"-1"', '"+1"', '""', '"a"', '"1.5"', '**', '"9223372036854775808"', '"3', '3"', "'3'"]) {
    seen.length = 0;
    r = await rename(url, bad);
    assert.equal(r.status, 422, bad);
    assert.equal(r.doc.title, 'ValidationError', bad);
    assert.deepEqual(r.doc.data, { issues: [{ path: 'header.If-Match', code: 'type', message: MALFORMED }] }, bad);
    assert.equal(r.doc.violations, undefined, bad);
    assert.equal(seen.length, 0, bad);
  }

  // The forms an HTTP client will not carry (spaces around the value, non-ASCII digits, a line feed): call the handler.
  const direct = srv.createRoomsHandler(roomsImpl, { facts: roomLoaders });
  for (const bad of [' "3"', '"3" ', '"٣"', '"3"\n', '', 'W/"3"']) {
    seen.length = 0;
    const res = await direct({ method: 'PUT', path: `/rooms/${ID}`, headers: { 'if-match': bad }, body: { name: 'Blue' } });
    assert.equal(res?.status, 422, JSON.stringify(bad));
    const doc = JSON.parse(res?.body ?? '{}') as Doc;
    assert.deepEqual(doc.data, { issues: [{ path: 'header.If-Match', code: 'type', message: MALFORMED }] });
    assert.equal(doc.violations, undefined);
    assert.equal(seen.length, 0);
  }

  // An endpoint without a guard ignores If-Match, even a malformed one, and still sends the ETag.
  put('4');
  r = await send(url, 'GET', `/rooms/${ID}`, { headers: { 'if-match': 'garbage' } });
  assert.equal(r.status, 200);
  assert.equal(r.etag, '"4"');
  assert.equal(seen[0]?.version, undefined);
  r = await send(url, 'GET', `/rooms/${ID}`, { headers: { 'if-match': '"1"' } });
  assert.equal(r.status, 200);

  // An int version: the handler gets a number, `expected` and `actual` are still strings.
  reset();
  counters.set(ID, { id: ID, total: 7, version: 2 });
  r = await send(url, 'POST', `/rooms/${ID}/tick`, { headers: { 'if-match': '"2"' } });
  assert.equal(r.status, 200);
  assert.strictEqual(seen[0]?.version, 2);
  assert.equal(r.etag, '"3"');
  seen.length = 0;
  counters.set(ID, { id: ID, total: 7, version: 2 });
  r = await send(url, 'POST', `/rooms/${ID}/tick`, { headers: { 'if-match': '"1"' } });
  assert.equal(r.status, 412);
  assert.deepEqual(r.doc.data, { expected: '1', actual: '2' });
  assert.equal(seen.length, 0);

  // An optional fact: absent, there is nothing to compare and the handler gets no version; present, it is compared.
  reset();
  r = await send(url, 'PATCH', `/rooms/${ID}/touch`, { headers: { 'if-match': '"3"' } });
  assert.equal(r.status, 200);
  assert.equal(seen[0]?.version, undefined);
  put('4');
  r = await send(url, 'PATCH', `/rooms/${ID}/touch`, { headers: { 'if-match': '"3"' } });
  assert.equal(r.status, 412);
  r = await send(url, 'PATCH', `/rooms/${ID}/touch`, { headers: { 'if-match': '"4"' } });
  assert.equal(r.status, 200);
  assert.equal(seen[0]?.version, '4');

  // A missing fact is `404` before any tag is compared.
  reset();
  r = await rename(url, '"3"');
  assert.equal(r.status, 404);

  // The order of the checks: the policy, then the version, then the transition, then `require`.
  const asOrder = (token: string, tag: string, text = 'ok'): Promise<Answer> =>
    send(ordersServer.url, 'POST', `/orders/${ID}/pay`, { headers: { authorization: `Bearer ${token}`, 'if-match': tag }, body: { text } });
  const putOrder = (status: srv.Status, revision: string): void => {
    reset();
    orders.set(ID, { id: ID, status, revision });
  };
  putOrder('pending', '4');
  r = await asOrder('guest', '"3"');
  assert.equal(r.status, 403, 'the policy comes before the version');
  assert.equal(r.doc.title, 'Forbidden');
  putOrder('paid', '4');
  r = await asOrder('staff', '"3"');
  assert.equal(r.status, 412, 'the version comes before the transition');
  assert.deepEqual(r.doc.data, { expected: '3', actual: '4' });
  assert.match(r.doc.violations?.[0]?.rule ?? '', /^veld:bookings\/Orders\.payOrder\.version@[0-9a-f]{16}$/);
  r = await asOrder('staff', '"4"');
  assert.equal(r.status, 409, 'a current tag leaves the transition to answer');
  assert.equal(r.doc.title, 'InvalidTransition');
  putOrder('pending', '4');
  r = await asOrder('staff', '"3"', 'far too long');
  assert.equal(r.status, 412, 'the version comes before require');
  r = await asOrder('staff', '"4"', 'far too long');
  assert.equal(r.status, 422);
  assert.equal(r.doc.title, 'TooLong');
  assert.equal(seen.length, 0);
  // Every step passes: the handler gets the version (the wire name `rev` is not its name) and the transition.
  r = await asOrder('staff', '"4"');
  assert.equal(r.status, 200);
  assert.equal(seen[0]?.version, '4');
  assert.deepEqual(seen[0]?.transition, { field: 'status', from: 'pending', to: 'paid' });
  assert.deepEqual(seen[0]?.keys, ['body', 'id', 'order', 'transition', 'version']);
  assert.equal(r.etag, '"5"');
  assert.equal(r.doc.status as unknown, 'paid');

  // The generated client: the header it writes, and the typed error it raises.
  put('4');
  const client = new cli.RoomsClient({ baseUrl: url });
  const renamed = await client.rename({ id: ID, body: { name: 'Blue' }, ifMatch: '4' });
  assert.equal(renamed.revision, '5');
  assert.equal(sent[sent.length - 1]?.ifMatch, '"4"');
  put('4');
  await client.rename({ id: ID, body: { name: 'Blue' }, ifMatch: '*' });
  assert.equal(sent[sent.length - 1]?.ifMatch, '*');
  put('4');
  await client.rename({ id: ID, body: { name: 'Blue' } });
  assert.equal(sent[sent.length - 1]?.ifMatch, undefined);
  put('4');
  await assert.rejects(client.rename({ id: ID, body: { name: 'Blue' }, ifMatch: '3' }), (e: unknown) => {
    assert.ok(e instanceof cli.PreconditionFailed);
    assert.deepEqual(e.data, { expected: '3', actual: '4' });
    return true;
  });
  // An int version is a number in the client, and the tag is its decimal text.
  reset();
  counters.set(ID, { id: ID, total: 1, version: 2 });
  await client.tick({ id: ID, ifMatch: 2 });
  assert.equal(sent[sent.length - 1]?.ifMatch, '"2"');
  // The client checks the syntax of nothing: the server answers 422 for a tag that is not one.
  put('4');
  await assert.rejects(client.rename({ id: ID, body: { name: 'Blue' }, ifMatch: '03' }), (e: unknown) => {
    assert.ok(e instanceof cli.ValidationError);
    return true;
  });
  assert.equal(sent[sent.length - 1]?.ifMatch, '"03"');

  roomsServer.close();
  ordersServer.close();
  console.log('version over http ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
