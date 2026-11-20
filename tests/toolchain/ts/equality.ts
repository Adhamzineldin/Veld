// Cross-package equality: the same contract generated into separate directories, plus variants.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as A from './copy_a/index';
import * as B from './copy_b/index';
import * as C from './copy_c/index';
import * as D from './copy_d/index';

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

// Companion signatures equal the IR's.
assert.equal(A.Money.sig, sigs.a['decl:orders:catalog.Money']);
assert.equal(A.OutOfStock.sig, sigs.a['decl:orders:orders.OutOfStock']);

process.stdout.write('equality ok\n');
