// The generated TypeScript client against a server in another process (the generated Java server):
// every declared error arrives as the right class with the right operation signature.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import * as cli from './copy_a/index';
import { OrdersClient } from './copy_a/orders/client';

const sigs = (JSON.parse(readFileSync('sigs.json', 'utf8')) as { a: Record<string, string> }).a;
const baseUrl = process.argv[2];
assert.ok(baseUrl, 'usage: crossclient.js <base url>');
const USER_ID = '11111111-1111-4111-8111-111111111111';

async function main(base: string): Promise<void> {
  const client = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  const lax = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, validate: false });

  assert.equal(await client.health(), undefined);

  const created = await client.createOrder({
    body: { customerId: USER_ID, items: [{ sku: 'ABC-1', quantity: 2 }], payment: { kind: 'card', last4: '4242' } },
    idempotencyKey: 'key-1',
  });
  assert.equal(created.status, 'pending', 'default status');
  assert.equal(created.note, 'key-1', 'header param reaches the Java implementation');
  assert.equal(created.customer.displayName, 'Ada');
  assert.deepEqual(created.payment, { kind: 'card', last4: '4242' });
  assert.equal(created.seq, '9007199254740993', 'long keeps all its digits');
  assert.deepEqual(await client.getOrder({ id: created.id }), created);

  await assert.rejects(client.getOrder({ id: randomUUID() }), (e: unknown) => {
    assert.ok(e instanceof cli.NotFound, 'NotFound class');
    assert.equal(e.op, sigs['Orders.getOrder']);
    assert.equal(e.status, 404);
    assert.deepEqual(e.data, { resource: 'order' });
    assert.ok(cli.NotFound.matches(e));
    return true;
  });
  await assert.rejects(
    client.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'OUT-1', quantity: 1 }] } }),
    (e: unknown) => {
      assert.ok(e instanceof cli.OutOfStock);
      assert.equal(e.op, sigs['Orders.createOrder']);
      assert.equal(e.status, 409);
      assert.equal(e.sig, cli.OutOfStock.sig);
      assert.deepEqual(e.data, { sku: 'OUT-1' });
      return true;
    },
  );
  await assert.rejects(
    client.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'ABC-1', quantity: 1 }] }, idempotencyKey: 'quota' }),
    (e: unknown) => {
      assert.ok(e instanceof cli.QuotaExceeded);
      assert.equal(e.status, 429, 'use-site status');
      assert.equal(e.op, sigs['Orders.createOrder']);
      assert.deepEqual(e.data, { limit: 5 });
      return true;
    },
  );

  const page = await client.listOrders({ query: { status: 'pending', tag: ['a', 'b c'], limit: 10 } });
  assert.equal(page.total, '1');
  assert.equal(page.items.length, 1);
  assert.equal((await client.searchOrders({ q: 'key-1' })).length, 1);

  const paid = await client.payOrder({ id: created.id, body: { kind: 'bank', iban: 'DE00' } });
  assert.equal(paid.status, 'paid');
  assert.deepEqual(paid.payment, { kind: 'bank', iban: 'DE00' });
  assert.equal(await client.cancelOrder({ id: created.id }), undefined);
  await assert.rejects(client.cancelOrder({ id: created.id }), (e: unknown) => {
    assert.ok(e instanceof cli.NotFound);
    assert.equal(e.op, sigs['Orders.cancelOrder']);
    return true;
  });

  await assert.rejects(client.boom(), (e: unknown) => {
    assert.ok(e instanceof cli.Internal, 'undeclared failure is the declared Internal');
    assert.equal(e.status, 500);
    assert.equal(e.op, sigs['Orders.boom']);
    assert.ok(!JSON.stringify(e.data).includes('secret'), 'no leak');
    return true;
  });

  await assert.rejects(lax.createOrder({ body: { customerId: 'nope', items: [] } }), (e: unknown) => {
    assert.ok(e instanceof cli.ValidationError, 'remote ValidationError');
    assert.equal(e.status, 422);
    assert.equal(e.op, sigs['Orders.createOrder']);
    const paths = e.data.issues.map((i) => `${i.path}:${i.code}`);
    assert.ok(paths.includes('body.items:min_length'), paths.join());
    return true;
  });

  const anon = new OrdersClient({ baseUrl: base });
  await assert.rejects(anon.getOrder({ id: randomUUID() }), (e: unknown) => {
    assert.ok(e instanceof cli.Unauthorized);
    assert.equal(e.status, 401);
    assert.equal(e.op, sigs['Orders.getOrder']);
    return true;
  });
  assert.equal(await anon.health(), undefined);
  process.stdout.write('ts client against java server ok\n');
}

main(baseUrl).catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
