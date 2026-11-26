// Cross-package equality: the same contract generated into separate directories, plus variants.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as A from './copy_a/index';
import * as B from './copy_b/index';
import * as C from './copy_c/index';
import * as D from './copy_d/index';
import { IamClient } from './copy_a/iam/client';
import { OrdersClient } from './copy_a/orders/client';

interface Sigs {
  a: Record<string, string>;
  c: Record<string, string>;
  d: Record<string, string>;
}
const sigs = JSON.parse(readFileSync('sigs.json', 'utf8')) as Sigs;

// Distinct generated copies really are distinct module instances.
assert.notEqual(A.VeldError, B.VeldError);

const outOfStock = (m: { OutOfStock: typeof A.OutOfStock }, sku: string, op: string | null) =>
  new m.OutOfStock({ sku }, { op });

// Equal: same signature, operation and payload, across two copies.
const a = outOfStock(A, 'X', sigs.a['Orders.createOrder'] ?? null);
const b = outOfStock(B, 'X', sigs.a['Orders.createOrder'] ?? null);
assert.ok(A.same(a, b), 'equal errors across copies');
assert.ok(B.same(b, a));
assert.ok(a instanceof A.OutOfStock && !(b instanceof A.OutOfStock), 'identity is by signature, not by class');

// Different payload.
assert.ok(!A.same(a, outOfStock(B, 'Y', sigs.a['Orders.createOrder'] ?? null)), 'different data');

// Different operation.
assert.ok(!A.same(a, outOfStock(B, 'X', sigs.a['Orders.payOrder'] ?? null)), 'different op');
assert.ok(!A.same(outOfStock(A, 'X', null), a), 'op set vs unset');

// Same prelude error name raised by two different services.
const nfOrders = new A.NotFound({ resource: 'x' }, { op: sigs.a['Orders.getOrder'] ?? null });
const nfIam = new B.NotFound({ resource: 'x' }, { op: sigs.a['Iam.getUser'] ?? null });
assert.ok(!A.same(nfOrders, nfIam), 'same error name, different service');
assert.ok(A.same(nfOrders, new B.NotFound({ resource: 'x' }, { op: sigs.a['Orders.getOrder'] ?? null })));

// Same error name declared in a different package (and so a different signature).
const dErr = new D.OutOfStock({ sku: 'X' }, { op: sigs.d['Orders.createOrder'] ?? null });
assert.ok(!A.same(a, dErr), 'different declaring package');

// A changed contract shape has a different signature.
const cErr = new C.OutOfStock({ sku: 'X', warehouse: 'w' }, { op: sigs.c['Orders.createOrder'] ?? null });
assert.notEqual(A.OutOfStock.sig, C.OutOfStock.sig);
assert.ok(!A.same(a, cErr), 'changed contract shape');

// matches: the declaration raised by its own service, from any operation.
assert.ok(A.OutOfStock.matches(b));
assert.ok(A.OutOfStock.matches(outOfStock(B, 'Z', sigs.a['Orders.payOrder'] ?? null)));
assert.ok(!A.OutOfStock.matches(dErr), 'different declaration');
assert.ok(!A.OutOfStock.matches(nfOrders));
assert.ok(A.NotFound.matches(nfIam), 'prelude errors match from any service');
assert.ok(!A.OutOfStock.matches(outOfStock(B, 'Z', sigs.a['Iam.getUser'] ?? null)), 'wrong declaring service');

// Models: canonical bytes agree across copies, and differ when the value differs.
const money = { amount: '1.50', currency: 'USD' };
assert.equal(A.Money.canonical(money), B.Money.canonical(money));
assert.ok(A.Money.same(money, { currency: 'USD', amount: '1.50' }));
assert.ok(!A.Money.same(money, { amount: '1.5', currency: 'USD' }));
assert.equal(A.Money.sig, B.Money.sig);

// Error data: schema defaults are applied before comparing (spec/ir.md 5.3).
const thr = (m: { Throttled: typeof A.Throttled }, data: { retryAfter?: number }) =>
  new m.Throttled(data as { retryAfter: number }, { op: sigs.a['Orders.createOrder'] ?? null });
assert.ok(A.same(thr(A, {}), thr(B, { retryAfter: 30 })), 'absent defaulted field equals the default value');
assert.ok(B.same(thr(B, { retryAfter: 30 }), thr(A, {})));
assert.ok(!A.same(thr(A, {}), thr(B, { retryAfter: 31 })), 'a different value is different');
assert.equal(A.canonicalError(thr(A, {})), A.canonicalError(thr(B, { retryAfter: 30 })));

// Models carry their declaration: `same` needs it on both sides and compares signature AND bytes.
const wire = { amount: '1.50', currency: 'USD' };
const decoded = A.Money.fromWire(wire);
const built = A.Money.of({ amount: '1.50', currency: 'USD' });
assert.ok(A.same(decoded, built), 'decoded vs constructed');
assert.ok(A.same(B.Money.of({ amount: '1.50', currency: 'USD' }), decoded), 'across two generated copies');
assert.ok(!A.same(decoded, A.Money.of({ amount: '2.00', currency: 'USD' })), 'different data');
assert.ok(C.same(C.Money.of(money), decoded), 'copy C changed only an error: Money keeps its signature');
// Same fields, different declaring package: a different signature, so never the same.
assert.notEqual(A.Money.sig, D.Money.sig);
assert.ok(!A.same(decoded, D.Money.of(money)) && !D.same(D.Money.of(money), decoded), 'same fields, other package');
assert.ok(!A.Money.same(decoded, D.Money.of(money)), 'the companion refuses a foreign declaration');
// A model against an error whose data is identical.
const errData = new A.OutOfStock({ sku: 'X' }, { op: null });
assert.ok(!A.same(decoded, errData) && !A.same(errData, decoded));
// Another model of the same package with the same shape is another declaration.
const orderItem = A.OrderItem.of({ sku: 'ABC-1', quantity: 1 });
const card = A.Card.of({ last4: '1234' });
const bank = A.BankTransfer.of({ iban: '1234' });
assert.equal(A.Card.canonical(card).replace('last4', 'x'), A.BankTransfer.canonical(bank).replace('iban', 'x'));
assert.ok(!A.same(card, bank) && !A.same(bank, card), 'Card vs BankTransfer: identical shape, different declaration');
assert.ok(!A.same(orderItem, card));
// Without identity `same` refuses to guess.
assert.throws(() => A.same(money, money), TypeError);
assert.throws(() => A.same(decoded, { ...decoded }), /Model.same/);
assert.throws(() => A.same('paid', 'paid'), TypeError, 'enum values are primitives: use Status.same or ===');
assert.ok(A.Money.same(money, decoded), 'the companion accepts plain objects');
assert.equal(JSON.stringify(decoded), JSON.stringify(wire), 'identity is not serialised');
assert.deepEqual(Object.keys(decoded), ['amount', 'currency']);

// Companion signatures equal the IR's.
assert.equal(A.Money.sig, sigs.a['decl:orders:catalog.Money']);
assert.equal(A.OutOfStock.sig, sigs.a['decl:orders:orders.OutOfStock']);

// createClients: one client per service, each with its URL from the runtime config, all with the same options.
void (async () => {
  const urls: string[] = [];
  const spy = (async (url: string) => {
    urls.push(url);
    throw new Error('offline');
  }) as unknown as A.veld.FetchLike;
  const runtime: A.WebRuntime = { services: { iam: { url: 'http://iam.test' }, orders: { url: 'http://orders.test/' } } };
  const api = A.createClients(runtime, { fetch: spy });
  assert.deepEqual(Object.keys(api).sort(), ['iam', 'orders']);
  assert.ok(api.orders instanceof OrdersClient && api.iam instanceof IamClient);
  await assert.rejects(api.orders.health({}), A.VeldTransportError);
  await assert.rejects(api.iam.getUser({ id: '11111111-1111-4111-8111-111111111111' }), A.VeldTransportError);
  assert.ok(urls[0]?.startsWith('http://orders.test/') && !urls[0].startsWith('http://orders.test//'), String(urls[0]));
  assert.ok(urls[1]?.startsWith('http://iam.test/'), String(urls[1]));
  process.stdout.write('equality ok\n');
})();
