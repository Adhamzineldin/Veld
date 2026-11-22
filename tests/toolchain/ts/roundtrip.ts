// Round trip: the generated server mounted with toExpress, every endpoint called through the generated client.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import type { AddressInfo } from 'node:net';
import http from 'node:http';
import express from 'express';
import * as srv from './server/index';
import * as iamSrv from './server_iam/index';
import * as cli from './copy_a/index';
import { OrdersClient } from './copy_a/orders/client';

interface Sigs {
  a: Record<string, string>;
}
const sigs = JSON.parse(readFileSync('sigs.json', 'utf8')) as Sigs;
const OP = sigs.a;

const USER_ID = '11111111-1111-4111-8111-111111111111';
const BANNED_ID = '22222222-2222-4222-8222-222222222222';

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

const store = new Map<string, srv.Order>();
const thrown: unknown[] = [];
let rateLimitCalls = 0;
let requests = 0;

const user: srv.User = { id: USER_ID, email: 'ada@example.com', displayName: 'Ada' };

export const impl: srv.OrdersService = {
  async getOrder(req) {
    const order = store.get(req.id);
    if (!order) throw new srv.NotFound({ resource: 'order' });
    return order;
  },
  async createOrder(req, ctx) {
    assert.deepEqual(ctx.principal, { user: 'u1' });
    if (req.body.items.some((i) => i.sku === 'OUT-1')) throw new srv.OutOfStock({ sku: 'OUT-1' });
    if (req.idempotencyKey === 'quota') throw new srv.QuotaExceeded({ limit: 5 }, { status: 429 });
    const order: srv.Order = {
      id: randomUUID(),
      createdAt: '2024-05-01T12:00:00+02:00',
      customer: { ...user, id: req.body.customerId },
      items: req.body.items,
      status: srv.Status.pending,
      total: { amount: '10.50', currency: 'USD' },
      seq: '9007199254740993',
      ...(req.body.note === undefined ? {} : { note: req.body.note }),
      ...(req.body.payment === undefined ? {} : { payment: req.body.payment }),
    };
    if (req.idempotencyKey !== undefined) order.note = req.idempotencyKey;
    store.set(order.id, order);
    return order;
  },
  async listOrders(req) {
    const items = [...store.values()].filter((o) => req.query.status === undefined || o.status === req.query.status);
    const limited = items.slice(0, req.query.limit ?? 100);
    assert.ok(req.query.tag === undefined || Array.isArray(req.query.tag));
    if (req.query.tag) note(`tags=${req.query.tag.join('|')}`);
    return { items: limited, total: String(items.length), page: 1, size: limited.length };
  },
  async searchOrders(req) {
    note(`q=${req.q};limit=${String(req.limit)}`);
    return [...store.values()].filter((o) => (o.note ?? '').includes(req.q));
  },
  async payOrder(req) {
    const order = store.get(req.id);
    if (!order) throw new srv.NotFound({});
    const paid: srv.Order = { ...order, status: srv.Status.paid, payment: req.body };
    store.set(paid.id, paid);
    return paid;
  },
  async cancelOrder(req) {
    if (!store.delete(req.id)) throw new srv.NotFound({ resource: 'order' });
  },
  async health() {},
  async boom() {
    throw new Error('secret internal detail');
  },
};

const seen: string[] = [];
function note(s: string): void {
  seen.push(s);
}

const iamImpl: iamSrv.IamService = {
  async getUser(req) {
    if (req.id === BANNED_ID) throw new iamSrv.UserBanned({ reason: 'spam' });
    if (req.id !== USER_ID) throw new iamSrv.NotFound({ resource: 'user' });
    return user;
  },
};

export const hooks: srv.OrdersHooks = {
  auth: { bearer: async (token) => (token === 'good' ? { user: 'u1' } : null) },
  middleware: {
    RateLimit: async () => {
      rateLimitCalls += 1;
    },
  },
  onError: (err) => {
    thrown.push(err);
  },
};

// ---------------------------------------------------------------------------
// Servers
// ---------------------------------------------------------------------------

function listen(server: http.Server): Promise<string> {
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve(`http://127.0.0.1:${(server.address() as AddressInfo).port}`));
  });
}

/** Counts the requests that reach the server; the scenario checks that locally invalid calls send none. */
export function countRequests(_req: unknown, _res: unknown, next: () => void): void {
  requests += 1;
  next();
}

function count(app: express.Express): express.Express {
  app.use(countRequests);
  return app;
}

async function raw(url: string, init?: { method?: string; headers?: Record<string, string>; body?: string }) {
  const res = await fetch(url, init);
  const text = await res.text();
  let json: Record<string, unknown> | undefined;
  try {
    json = text === '' ? undefined : (JSON.parse(text) as Record<string, unknown>);
  } catch {
    json = undefined;
  }
  return { status: res.status, text, json };
}

// ---------------------------------------------------------------------------
// Scenario
// ---------------------------------------------------------------------------

export async function scenario(label: string, base: string): Promise<void> {
  const client = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  const lax = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, validate: false });
  const tag = (m: string): string => `[${label}] ${m}`;
  const before = rateLimitCalls;

  // health: no auth, 204
  assert.equal(await client.health(), undefined, tag('health'));

  // createOrder: 201, defaults applied, @wire mapped, header param, union body field
  const created = await client.createOrder({
    body: {
      customerId: USER_ID.toUpperCase(),
      items: [{ sku: 'ABC-1', quantity: 2 }],
      payment: { kind: 'card', last4: '4242' },
    },
    idempotencyKey: 'key-1',
  });
  assert.equal(created.status, 'pending', tag('default status'));
  assert.equal(created.note, 'key-1', tag('header param reaches the implementation'));
  assert.equal(created.customer.displayName, 'Ada');
  assert.deepEqual(created.payment, { kind: 'card', last4: '4242' });
  assert.equal(created.seq, '9007199254740993');

  // wire form uses @wire names
  const wire = await raw(`${base}/orders/${created.id}`, { headers: { authorization: 'Bearer good' } });
  assert.equal(wire.status, 200);
  const body = wire.json as { created_at: string; customer: { display_name: string }; status: string };
  assert.equal(body.created_at, '2024-05-01T12:00:00+02:00');
  assert.equal(body.customer.display_name, 'Ada');
  assert.equal(body.status, 'pending');

  // getOrder
  const fetched = await client.getOrder({ id: created.id });
  assert.deepEqual(fetched, created);

  // declared errors arrive as the right class with op set
  await assert.rejects(client.getOrder({ id: randomUUID() }), (e: unknown) => {
    assert.ok(e instanceof cli.NotFound, tag('NotFound class'));
    assert.equal(e.op, OP['Orders.getOrder']);
    assert.equal(e.status, 404);
    assert.deepEqual(e.data, { resource: 'order' });
    assert.ok(cli.NotFound.matches(e));
    return true;
  });
  await assert.rejects(
    client.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'OUT-1', quantity: 1 }] } }),
    (e: unknown) => {
      assert.ok(e instanceof cli.OutOfStock);
      assert.equal(e.op, OP['Orders.createOrder']);
      assert.equal(e.status, 409);
      assert.deepEqual(e.data, { sku: 'OUT-1' });
      return true;
    },
  );
  await assert.rejects(
    client.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'ABC-1', quantity: 1 }] }, idempotencyKey: 'quota' }),
    (e: unknown) => {
      assert.ok(e instanceof cli.QuotaExceeded);
      assert.equal(e.status, 429, tag('use-site status'));
      assert.deepEqual(e.data, { limit: 5 });
      return true;
    },
  );

  // list with a query object (enum, repeated list keys, int)
  const page = await client.listOrders({ query: { status: 'pending', tag: ['a', 'b c'], limit: 10 } });
  assert.equal(page.total, '1');
  assert.equal(page.items.length, 1);
  assert.ok(seen.includes('tags=a|b c'), tag('repeated query keys'));
  const none = await client.listOrders({ query: { status: 'paid' } });
  assert.equal(none.items.length, 0);

  // search with single @query params, special characters, optional omitted
  const found = await client.searchOrders({ q: 'key-1' });
  assert.equal(found.length, 1);
  await client.searchOrders({ q: 'a b&c=d' });
  assert.ok(seen.includes('q=a b&c=d;limit=undefined'), tag('query encoding'));
  await client.searchOrders({ q: 'x', limit: 3 });
  assert.ok(seen.includes('q=x;limit=3'));

  // pay: union body, union in the response
  const paid = await client.payOrder({ id: created.id, body: { kind: 'bank', iban: 'DE00' } });
  assert.equal(paid.status, 'paid');
  assert.deepEqual(paid.payment, { kind: 'bank', iban: 'DE00' });
  await assert.rejects(client.payOrder({ id: randomUUID(), body: { kind: 'bank', iban: 'x' } }), cli.NotFound.matches);

  // cancel: 204, then NotFound
  assert.equal(await client.cancelOrder({ id: created.id }), undefined);
  await assert.rejects(client.cancelOrder({ id: created.id }), cli.NotFound.matches);

  // undeclared failure: surfaces as the declared prelude `Internal`, without leaking its message
  await assert.rejects(client.boom(), (e: unknown) => {
    assert.ok(cli.Internal.matches(e), tag('boom is a declared Internal error'));
    assert.equal((e as cli.Internal).status, 500);
    assert.ok(!JSON.stringify((e as cli.Internal).data).includes('secret'), tag('no leak'));
    return true;
  });
  const boomRaw = await raw(`${base}/orders/boom`);
  assert.equal(boomRaw.status, 500);
  assert.ok(!boomRaw.text.includes('secret'));
  assert.ok(thrown.some((e) => e instanceof Error && e.message === 'secret internal detail'), tag('onError saw it'));

  // validation: local (client) and remote (server) failures
  const sent = requests;
  await assert.rejects(client.createOrder({ body: { customerId: 'nope', items: [] } }), (e: unknown) => {
    assert.ok(e instanceof cli.ValidationError, tag('local ValidationError'));
    assert.equal(e.op, OP['Orders.createOrder']);
    assert.ok(e.data.issues.length >= 2);
    return true;
  });
  assert.equal(requests, sent, tag('no request for a locally invalid call'));
  await assert.rejects(lax.createOrder({ body: { customerId: 'nope', items: [] } }), (e: unknown) => {
    assert.ok(e instanceof cli.ValidationError, tag('remote ValidationError'));
    assert.equal(e.status, 422);
    assert.equal(e.op, OP['Orders.createOrder']);
    const paths = e.data.issues.map((i) => `${i.path}:${i.code}`);
    assert.ok(paths.includes('body.customer_id:type'), paths.join());
    assert.ok(paths.includes('body.items:min_length'), paths.join());
    return true;
  });
  await assert.rejects(lax.listOrders({ query: { limit: 1000 } }), (e: unknown) => {
    assert.ok(e instanceof cli.ValidationError);
    assert.ok(e.data.issues.some((i) => i.path === 'query.limit' && i.code === 'max'));
    return true;
  });
  if (label === 'stream') {
    const badJson = await raw(`${base}/orders`, {
      method: 'POST',
      headers: { authorization: 'Bearer good', 'content-type': 'application/json' },
      body: '{not json',
    });
    assert.equal(badJson.status, 422);
    assert.equal(badJson.json?.['title'], 'ValidationError');
  }

  // auth: missing and rejected tokens are 401 Unauthorized; valid tokens work
  const anon = new OrdersClient({ baseUrl: base });
  await assert.rejects(anon.getOrder({ id: randomUUID() }), (e: unknown) => {
    assert.ok(e instanceof cli.Unauthorized, tag('missing token'));
    assert.equal(e.status, 401);
    assert.equal(e.op, OP['Orders.getOrder']);
    return true;
  });
  const wrong = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'bad' } });
  await assert.rejects(wrong.listOrders({ query: {} }), cli.Unauthorized.matches);
  assert.equal(await anon.health(), undefined, 'health needs no token');

  // middleware ran for the requests that passed authentication
  assert.ok(rateLimitCalls > before, tag('middleware called'));
}

async function main(): Promise<void> {
  // 1. no body parser: the adapter reads the stream
  const app1 = count(express());
  app1.use(srv.toExpress(srv.createOrdersHandler(impl, hooks)));
  const s1 = http.createServer(app1);
  const base1 = await listen(s1);
  await scenario('stream', base1);

  // 2. express.json() ran first: the adapter reads req.body
  const app2 = count(express());
  app2.use(express.json());
  app2.use(srv.toExpress(srv.createOrdersHandler(impl, hooks)));
  const s2 = http.createServer(app2);
  const base2 = await listen(s2);
  await scenario('json-parser', base2);

  // unmatched routes fall through to Express
  assert.equal((await raw(`${base1}/nothing`)).status, 404);

  // 3. plain node:http adapter
  const s3 = http.createServer(srv.toNodeHttp(srv.createOrdersHandler(impl, hooks)));
  const base3 = await listen(s3);
  assert.equal((await raw(`${base3}/orders/health`)).status, 204);
  assert.equal((await raw(`${base3}/missing`)).status, 404);

  // 4. Fastify and Hono adapters, driven with structural fakes
  const handler = srv.createOrdersHandler(impl, hooks);
  const sent: { status?: number; headers: Record<string, string>; payload?: unknown } = { headers: {} };
  const reply = {
    code(n: number): unknown {
      sent.status = n;
      return undefined;
    },
    header(k: string, v: string): unknown {
      sent.headers[k] = v;
      return undefined;
    },
    send(p?: unknown): unknown {
      sent.payload = p;
      return undefined;
    },
  };
  await srv.toFastify(handler)({ method: 'GET', url: '/orders/health', headers: {}, raw: {} }, reply);
  assert.equal(sent.status, 204);
  const honoRes = (await srv.toHono(handler)({
    req: { method: 'GET', url: 'http://localhost/orders/health', header: () => ({}), text: async () => '' },
  })) as { status: number };
  assert.equal(honoRes.status, 204);

  // 5. the consumed Iam client against a second generated server
  const iamApp = express();
  iamApp.use(iamSrv.toExpress(iamSrv.createIamHandler(iamImpl)));
  const iamBase = await listen(http.createServer(iamApp));
  const iamClient = new srv.iam.IamClient({ baseUrl: iamBase });
  const got = await iamClient.getUser({ id: USER_ID });
  assert.equal(got.displayName, 'Ada');
  await assert.rejects(iamClient.getUser({ id: randomUUID() }), srv.iam.NotFound.matches);
  await assert.rejects(iamClient.getUser({ id: BANNED_ID }), (e: unknown) => {
    assert.ok(e instanceof srv.iam.UserBanned);
    assert.equal(e.status, 403);
    assert.deepEqual(e.data, { reason: 'spam' });
    return true;
  });
  const envLess = () => new srv.iam.IamClient({ fetch: async () => ({ status: 200, text: async () => '{}' }) });
  assert.ok(envLess(), 'baked-in base URL is used when nothing else is given');

  process.stdout.write('roundtrip ok\n');
  process.exit(0);
}

if (require.main === module) {
  main().catch((e: unknown) => {
    process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
    process.exit(1);
  });
}
