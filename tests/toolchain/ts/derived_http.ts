// Derived fields over real HTTP (spec/rules.md section 17.2): the production handler of a generated
// Node server and the generated client. Expects `./server` and `./client` generated from
// tests/toolchain/contract/derived/shop.veld.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000001';
const WALLET = '00000000-0000-4000-8000-000000000002';

// The application: it records what it was given, and answers with a derived value of its own
// that the server must replace.
let cartGiven: srv.Cart | undefined;
// The principal the handler of `createCart` found in its context: that endpoint's rules never read it.
let cartCaller: unknown;
let payGiven: (srv.PayRequest & srv.PayFacts) | undefined;
let draftGiven: unknown;
let calls = 0;
let fee = 30;
const wallets = new Map<string, srv.WalletOutput>();

const impl: srv.ShopService = {
  async createCart(req, ctx) {
    calls += 1;
    cartGiven = req.body;
    cartCaller = ctx.principal;
    // The handler's own derived values are replaced on the way out.
    return { items: req.body.items, coupon: req.body.coupon, subtotal: 111, payable: 222 };
  },
  async pay(req) {
    calls += 1;
    payGiven = req;
    return { id: req.id, amount: 100, fee, net: 999 };
  },
  async saveDraft(req) {
    calls += 1;
    draftGiven = req.body;
    return { serial: 1, qty: req.body.qty };
  },
  async *receipts(req) {
    for (let i = 0; i < req.count; i += 1) yield { id: ID, amount: 10 * (i + 1), fee: 3, net: 999 };
  },
  async refuse() {
    throw new srv.NotEnough({ needed: 10, have: 4, short: 999 });
  },
};

const loaders: srv.ShopFactLoaders = {
  async wallet(key) {
    return wallets.get(key);
  },
};

// The auth hook returns what the application knows of the caller: no derived `tag`.
const hooks = {
  facts: loaders,
  auth: {
    bearer: async (token: string): Promise<srv.UserOutput | null> =>
      token === 'alice'
        ? { id: ID, name: 'alice', level: 4 }
        : token === 'nobody'
          ? { id: ID, name: '', level: 4 }
          : token === 'broken'
            ? { id: ID, name: 'broken', level: 0 }
            : null,
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
  title?: string;
  status?: number;
  data?: Record<string, unknown> & { issues?: { path: string; code: string }[] };
  violations?: { rule: string; path: string; fields: string[]; message: string; error?: string }[];
  [key: string]: unknown;
}

/** The raw bodies the server received, as the client sent them. */
const sent: string[] = [];

async function call(url: string, token: string, method: 'GET' | 'POST', path: string, body?: unknown) {
  const res = await fetch(`${url}${path}`, {
    method,
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const text = await res.text();
  const isJson = (res.headers.get('content-type') ?? '').includes('json');
  return { status: res.status, text, doc: (isJson && text !== '' ? JSON.parse(text) : {}) as Doc };
}

function recording(handler: VeldHandler): VeldHandler {
  const wrapped = async (req: Parameters<VeldHandler>[0]): ReturnType<VeldHandler> => {
    const read = req.text;
    if (read === undefined) return handler(req);
    return handler({
      ...req,
      text: async () => {
        const text = await read();
        sent.push(text);
        return text;
      },
    });
  };
  return Object.assign(wrapped, { ws: handler.ws });
}

function reset(): void {
  cartGiven = undefined;
  cartCaller = undefined;
  draftGiven = undefined;
  payGiven = undefined;
  calls = 0;
  fee = 30;
  wallets.clear();
  wallets.set(WALLET, { id: WALLET, balance: 100, held: 20 });
  sent.length = 0;
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  const server = await listen(recording(srv.createShopHandler(impl, { ...hooks, onRuleViolation: (e) => events.push(e) })));
  const post = (path: string, body: unknown, token = 'alice') => call(server.url, token, 'POST', path, body);

  // --- a request: what the caller sent for a derived field is dropped, the rest is computed -------
  reset();
  let r = await post('/shop/carts', {
    items: [{ price: 3, qty: 2, total: 999 }],
    subtotal: 1,
    payable: 'not a number',
  });
  assert.equal(r.status, 201, r.text);
  assert.equal(cartGiven?.items[0]?.total, 6, 'the line is computed, the value sent is replaced');
  assert.equal(cartGiven?.subtotal, 6, 'a sum over the computed totals');
  assert.equal(cartGiven?.payable, 6, 'the default of the coupon is applied before the chain');
  // The handler's own derived values are replaced on the way out.
  const answered = JSON.parse(r.text) as { subtotal: number; payable: number; items: { total: number }[] };
  assert.equal(answered.subtotal, 6);
  assert.equal(answered.payable, 6);
  assert.equal(answered.items[0]?.total, 6);

  // An invariant reads the computed value.
  reset();
  events.length = 0;
  r = await post('/shop/carts', { items: [{ price: 3, qty: 2 }], coupon: 100 });
  assert.equal(r.status, 422);
  assert.equal(r.doc.violations?.length, 1);
  assert.match(r.doc.violations?.[0]?.rule ?? '', /^veld:bookings\/Cart\.payableFits@[0-9a-f]{16}$/);
  assert.deepEqual(r.doc.violations?.[0]?.fields, ['payable']);
  assert.equal(calls, 0);

  // A derived field that fails: one violation whose rule is the id of the field, the rules that
  // read it are skipped, and the answer is 422.
  reset();
  events.length = 0;
  r = await post('/shop/carts', { items: [{ price: 2000000000, qty: 2 }] });
  assert.equal(r.status, 422);
  assert.equal(r.doc.violations?.length, 1, JSON.stringify(r.doc.violations));
  const failed = r.doc.violations?.[0];
  assert.match(failed?.rule ?? '', /^veld:bookings\/Line\.total@[0-9a-f]{16}$/);
  assert.equal(failed?.error, 'overflow');
  assert.equal(failed?.path, 'body.items[0]');
  assert.equal(failed?.message, 'field total could not be computed');
  assert.deepEqual(failed?.fields, ['price', 'qty']);
  assert.equal(calls, 0);
  assert.equal(events[0]?.outcome, 'rejected');

  // A value of the wrong type for a derived field is not a shape error: it is dropped.
  reset();
  r = await post('/shop/carts', { items: [{ price: 1, qty: 1, total: { not: 'a number' } }] });
  assert.equal(r.status, 201, r.text);
  assert.equal(cartGiven?.items[0]?.total, 1);

  // --- a fact and a require: the derived value of the request and of the fact -------------------
  reset();
  r = await post(`/shop/pay/${WALLET}`, { items: [{ price: 30, qty: 2 }] });
  assert.equal(r.status, 200, r.text);
  assert.equal(payGiven?.wallet.available, 80, 'the fact is given its derived field');
  assert.equal(payGiven?.wallet.share, 5);
  assert.equal(payGiven?.body.payable, 60);
  // The result: the handler's `net: 999` is replaced.
  assert.equal((JSON.parse(r.text) as { net: number }).net, 70);

  // The require reads the derived values, and the declared error is given its own.
  reset();
  r = await post(`/shop/pay/${WALLET}`, { items: [{ price: 50, qty: 2 }] });
  assert.equal(r.status, 402, r.text);
  assert.deepEqual(r.doc.data, { needed: 100, have: 80, short: 20 });
  assert.equal(calls, 0);

  // An ensure reads the derived field of the result.
  reset();
  fee = 130;
  r = await post(`/shop/pay/${WALLET}`, { items: [{ price: 30, qty: 2 }] });
  assert.equal(r.status, 500, 'net is -30: the ensure fails, in strict mode the caller sees nothing');

  // A fact whose derived field cannot be computed is the application's mistake: 500, reported.
  reset();
  events.length = 0;
  wallets.set(WALLET, { id: WALLET, balance: 100, held: 0 });
  r = await post(`/shop/pay/${WALLET}`, { items: [{ price: 1, qty: 1 }] });
  assert.equal(r.status, 500);
  assert.equal(calls, 0);
  assert.equal(events[0]?.position, 'output');
  assert.equal(events[0]?.outcome, 'rejected');
  assert.match(events[0]?.violation.rule ?? '', /^veld:bookings\/Wallet\.share@[0-9a-f]{16}$/);
  assert.equal(events[0]?.violation.error, 'division_by_zero');

  // --- the principal: its derived field is computed before the policy reads it ------------------
  reset();
  r = await post(`/shop/pay/${WALLET}`, { items: [{ price: 30, qty: 2 }] }, 'alice');
  assert.equal(r.status, 200, 'tag is the name: the policy permits');
  r = await post(`/shop/pay/${WALLET}`, { items: [{ price: 30, qty: 2 }] }, 'nobody');
  assert.equal(r.status, 403, 'the empty name makes an empty tag: the policy does not permit');

  // The principal is completed once per request, after authentication, for every endpoint: the
  // handler of `createCart` (no rule reads the principal there) gets the completed value.
  reset();
  r = await post('/shop/carts', { items: [{ price: 3, qty: 2 }] }, 'alice');
  assert.equal(r.status, 201, r.text);
  assert.deepEqual(cartCaller, { id: ID, name: 'alice', level: 4, tag: 'alice', share: 25 }, 'ctx.principal is the completed one');
  // The one the rules read is the same value, and the handler of `pay` sees it too.
  // A principal whose derived field cannot be computed is the application's mistake: 500, reported,
  // on an endpoint whose rules do not read the principal as well.
  reset();
  events.length = 0;
  r = await post('/shop/carts', { items: [{ price: 3, qty: 2 }] }, 'broken');
  assert.equal(r.status, 500, r.text);
  assert.equal(calls, 0, 'the handler is not called');
  assert.equal(events[0]?.position, 'output');
  assert.equal(events[0]?.outcome, 'rejected');
  assert.match(events[0]?.violation.rule ?? '', /^veld:bookings\/User\.share@[0-9a-f]{16}$/);
  assert.equal(events[0]?.violation.error, 'division_by_zero');

  // --- a declared error the handler raised, and a stream ----------------------------------------
  reset();
  r = await call(server.url, 'alice', 'GET', '/shop/refuse');
  assert.equal(r.status, 402);
  assert.deepEqual(r.doc.data, { needed: 10, have: 4, short: 6 }, 'the handler said 999: replaced');

  reset();
  const stream = await call(server.url, 'alice', 'GET', '/shop/receipts?count=2');
  assert.equal(stream.status, 200);
  const items = stream.text
    .split('\n')
    .filter((l) => l.startsWith('data: '))
    .map((l) => JSON.parse(l.slice(6)) as { amount: number; fee: number; net: number });
  assert.deepEqual(items.map((i) => i.net), [7, 17], 'every item is given its derived field');
  server.close();

  // --- output mode `off` still computes ---------------------------------------------------------
  reset();
  const lax = await listen(srv.createShopHandler(impl, hooks, { rules: { output: 'off' } }));
  r = await call(lax.url, 'alice', 'POST', '/shop/carts', { items: [{ price: 3, qty: 2 }] });
  assert.equal(r.status, 201);
  assert.equal((JSON.parse(r.text) as { payable: number }).payable, 6, 'derived fields are values, not rules');
  lax.close();

  // --- the client ---------------------------------------------------------------------------------
  reset();
  const live = await listen(recording(srv.createShopHandler(impl, hooks)));
  const client = new cli.ShopClient({ baseUrl: live.url, auth: { bearer: () => 'alice' } });
  sent.length = 0;
  // A request type has no derived field to fill in, and none is sent.
  const body: cli.CartInput = { items: [{ price: 3, qty: 2 }], coupon: 0 };
  const made = await client.createCart({ body });
  assert.equal(made.payable, 6, 'a response is read as the server computed it');
  assert.equal(made.items[0]?.total, 6);
  assert.equal(sent.length, 1);
  assert.deepEqual(JSON.parse(sent[0] ?? '{}'), { items: [{ price: 3, qty: 2 }], coupon: 0 }, 'no derived field on the wire');

  // The local check computes before it checks: payable is -94, and nothing is sent.
  sent.length = 0;
  const refused = await client.createCart({ body: { items: [{ price: 3, qty: 2 }], coupon: 100 } }).then(
    () => null,
    (e: unknown) => e as cli.ValidationError,
  );
  assert.ok(refused instanceof cli.ValidationError);
  assert.equal(refused.local, true);
  assert.match(refused.violations[0]?.rule ?? '', /Cart\.payableFits@/);
  assert.equal(sent.length, 0, 'nothing was sent');

  // A `@readonly` member is absent in a request: the server drops what was sent for `serial`, so
  // `next` (which reads it) is absent and its rule is skipped. The local check reads the request the
  // same way, so the client does not refuse what the server accepts.
  reset();
  sent.length = 0;
  const unsaved = { serial: -5, qty: 2 } as unknown as cli.DraftInput;
  const saved = await client.saveDraft({ body: unsaved });
  assert.equal(saved.qty, 2);
  assert.equal(sent.length, 1, 'the draft was sent, not refused locally');
  const gotDraft = draftGiven as { qty?: number; double?: number; next?: number; serial?: number };
  assert.equal(gotDraft.double, 4, 'the server computed `double`');
  assert.equal(gotDraft.serial, undefined, 'a `@readonly` member sent in a request is dropped');
  assert.equal(gotDraft.next, undefined, '`next` reads it, so it stays absent');

  // A derived value of a response is kept as it came: the client never recomputes.
  const echo = new cli.ShopClient({
    baseUrl: 'http://shop.invalid',
    fetch: async () => ({
      status: 201,
      text: async () => JSON.stringify({ items: [{ price: 3, qty: 2, total: 5 }], coupon: 0, subtotal: 777, payable: 888 }),
    }),
  });
  const kept = await echo.createCart({ body });
  assert.equal(kept.subtotal, 777);
  assert.equal(kept.payable, 888);
  assert.equal(kept.items[0]?.total, 5);
  live.close();

  // --- the public function: a form shows `payable` while the user types ---------------------------
  const full = cli.Cart.derive({ items: [{ price: 3, qty: 2 }] });
  assert.equal(full.payable, 6);
  assert.equal(full.items?.[0]?.total, 6);
  assert.equal(full.coupon, 0, 'defaults are applied');
  // A derived field whose inputs are missing stays absent.
  const draft = cli.Cart.derive({ items: [{ price: 3 }] });
  assert.equal(draft.items?.[0]?.total, undefined);
  assert.equal(draft.subtotal, undefined);
  assert.equal(draft.payable, undefined);
  // What a draft holds for a derived field is replaced; the draft itself is not changed.
  const original = { price: 1, qty: 2, total: 99 };
  assert.equal(cli.Line.derive(original).total, 2);
  assert.equal(original.total, 99);
  // `partial` computes before it checks, so a form sees the violation of a rule on a derived field.
  const form = cli.Cart.partial({ items: [{ price: 3, qty: 2 }], coupon: 100 });
  assert.equal(form.length, 1);
  assert.match(form[0]?.rule ?? '', /Cart\.payableFits@/);
  assert.deepEqual(cli.Cart.partial({ items: [{ price: 3 }] }), [], 'a rule that needs an absent field is skipped');
  const out: cli.veld.Violation[] = [];
  cli.Line.derive({ price: 2000000000, qty: 2 }, out);
  assert.equal(out.length, 1);
  assert.equal(out[0]?.error, 'overflow');
  console.log('derived over http ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
