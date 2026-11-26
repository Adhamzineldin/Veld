// Derived fields with request validation turned off (`validate = false`, spec/rules.md section 17.2):
// no rule is checked, but a derived field that cannot be computed is still the caller's fault: `422`
// with that field's violation, and the handler is not called (its type promises the value). Expects
// `./server` and `./client` generated from tests/toolchain/contract/derived/shop.veld with
// `validate = false`.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';
import type { VeldHandler } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000001';

let given: srv.Cart | undefined;
let calls = 0;

const impl: srv.ShopService = {
  async createCart(req) {
    calls += 1;
    given = req.body;
    // The result must satisfy the rules too: they still run on output (`strict`), the request is what is not checked.
    return { items: req.body.items, coupon: 0, subtotal: 0, payable: 0 };
  },
  async pay(req) {
    calls += 1;
    return { id: req.id, amount: 1, fee: 0 };
  },
  async saveDraft(req) {
    calls += 1;
    return { serial: 1, qty: req.body.qty };
  },
  async *receipts() {
    yield { id: ID, amount: 1, fee: 0 };
  },
  async refuse() {
    throw new srv.NotEnough({ needed: 1, have: 0 });
  },
};

const hooks = {
  facts: { wallet: async () => undefined },
  auth: { bearer: async (): Promise<srv.UserOutput | null> => ({ id: ID, name: 'alice', level: 4 }) },
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

async function post(url: string, body: unknown): Promise<{ status: number; doc: Record<string, unknown> }> {
  const res = await fetch(`${url}/shop/carts`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: 'Bearer alice' },
    body: JSON.stringify(body),
  });
  return { status: res.status, doc: (await res.json()) as Record<string, unknown> };
}

async function main(): Promise<void> {
  const warn = console.warn;
  console.warn = () => undefined;
  let off: Awaited<ReturnType<typeof listen>>;
  try {
    off = await listen(srv.createShopHandler(impl, hooks));
  } finally {
    console.warn = warn;
  }

  // A request whose derived field cannot be computed: 422 with the violation of that field.
  let r = await post(off.url, { items: [{ price: 2000000000, qty: 2 }] });
  assert.equal(r.status, 422, JSON.stringify(r.doc));
  const violations = r.doc['violations'] as { rule: string; path: string; error?: string }[];
  assert.equal(violations.length, 1);
  assert.match(violations[0]?.rule ?? '', /^veld:bookings\/Line\.total@[0-9a-f]{16}$/);
  assert.equal(violations[0]?.error, 'overflow');
  assert.equal(calls, 0, 'the handler was not called');

  // No rule is checked: a coupon larger than the cart is accepted, and the derived value is there.
  r = await post(off.url, { items: [{ price: 3, qty: 2 }], coupon: 100 });
  assert.equal(r.status, 201, JSON.stringify(r.doc));
  assert.equal(given?.payable, -94, 'the derived value is computed, the invariant is not checked');
  assert.equal(calls, 1);
  off.close();
  console.log('derived no validate ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
