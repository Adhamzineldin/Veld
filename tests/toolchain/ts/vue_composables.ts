// Runtime test of the generated Vue composables, in an effect scope, against the REAL generated
// Express-served server through the generated client.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import express from 'express';
import { createApp, effectScope, nextTick, ref } from 'vue';
import * as srv from './server/index';
import * as cli from './copy_a/index';
import { OrdersClient } from './copy_a/orders/client';
import { createVeldPlugin, useCreateOrder, useGetOrder, useHealth, useOrdersClient } from './copy_a/orders/vue';
import type { GetOrderRequest } from './copy_a/orders/client';
import type { FetchInit } from './copy_a/runtime/veld';
import { hooks, impl } from './roundtrip';

const USER_ID = '11111111-1111-4111-8111-111111111111';

function recordingFetch(delayMs = 0) {
  const calls: string[] = [];
  const signals: { aborted: boolean }[] = [];
  const fetchLike = async (url: string, init: FetchInit) => {
    calls.push(`${init.method} ${url}`);
    if (init.signal) signals.push(init.signal);
    if (delayMs > 0) await new Promise((r) => setTimeout(r, delayMs));
    return fetch(url, init as unknown as RequestInit);
  };
  return { fetchLike, calls, signals };
}

/** Reads a ref through a function so the compiler does not narrow what the assertions already checked. */
const valueOf = <T,>(r: { readonly value: T }): T => r.value;

async function until(label: string, cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300; i += 1) {
    if (cond()) return;
    await nextTick();
    await new Promise((r) => setTimeout(r, 10));
  }
  assert.fail(`timed out waiting for: ${label}`);
}

async function main(): Promise<void> {
  const app = express();
  app.use(srv.toExpress(srv.createOrdersHandler(impl, hooks)));
  const server = http.createServer(app);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;

  const plain = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  const first = await plain.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'ABC-1', quantity: 1 }] } });
  const second = await plain.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'ABC-2', quantity: 2 }] } });

  const rec = recordingFetch();
  const client = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: rec.fetchLike });

  // ---- a query that reacts to its request ref ---------------------------------------------------
  const scope = effectScope();
  const req = ref<GetOrderRequest | null>({ id: first.id });
  const query = scope.run(() => useGetOrder(req, { client }));
  assert.ok(query);
  assert.equal(query.loading.value, true, 'loading first');
  assert.equal(query.data.value, undefined);
  await until('first order', () => query.data.value?.id === first.id);
  assert.equal(query.loading.value, false);
  assert.equal(query.error.value, undefined);
  assert.equal(rec.calls.length, 1);

  // An equal request (a new object, same canonical bytes) does not refetch.
  req.value = { id: first.id };
  await nextTick();
  await new Promise((r) => setTimeout(r, 40));
  assert.equal(rec.calls.length, 1, 'a stable key does not refetch');

  // A changed request refetches: loading again, no stale data, the old call aborted.
  const firstSignal = rec.signals[0];
  req.value = { id: second.id };
  await nextTick();
  assert.equal(query.loading.value, true);
  assert.equal(query.data.value, undefined, 'no stale data for the new request');
  assert.equal(firstSignal?.aborted, true, 'the previous call is aborted on a request change');
  await until('second order', () => query.data.value?.id === second.id);
  assert.equal(rec.calls.length, 2);

  // refetch() re-runs the call.
  await plain.payOrder({ id: second.id, body: { kind: 'bank', iban: 'DE00' } });
  assert.equal(valueOf(query.data)?.status, 'pending');
  query.refetch();
  assert.equal(query.loading.value, true);
  await until('refetched', () => query.data.value?.status === 'paid');
  assert.equal(rec.calls.length, 3);

  // A typed error stays the generated class.
  req.value = { id: randomUUID() };
  await until('error', () => query.error.value !== undefined);
  assert.ok(cli.NotFound.matches(query.error.value), 'NotFound.matches(error)');
  assert.ok((query.error.value as object) instanceof cli.NotFound);
  assert.equal(query.data.value, undefined);
  assert.equal(query.loading.value, false);

  // null holds the call back.
  const before = rec.calls.length;
  req.value = null;
  await nextTick();
  assert.equal(query.loading.value, false);
  assert.equal(rec.calls.length, before);
  scope.stop();

  // ---- disposing the scope aborts the call in flight --------------------------------------------
  const slow = recordingFetch(80);
  const slowClient = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: slow.fetchLike });
  const slowScope = effectScope();
  slowScope.run(() => useHealth({}, { client: slowClient }));
  await until('the slow call starts', () => slow.signals.length === 1);
  assert.equal(slow.signals[0]?.aborted, false);
  slowScope.stop();
  assert.equal(slow.signals[0]?.aborted, true, 'aborted when the scope is disposed');
  await new Promise((r) => setTimeout(r, 120));

  // ---- a mutation -------------------------------------------------------------------------------
  const mutation = effectScope().run(() => useCreateOrder({ client }));
  assert.ok(mutation);
  const created = await mutation.mutate({ body: { customerId: USER_ID, items: [{ sku: 'NEW-1', quantity: 3 }] } });
  assert.equal(created?.items[0]?.sku, 'NEW-1');
  assert.equal(mutation.data.value?.id, created?.id);
  assert.equal(mutation.loading.value, false);
  const failed = await mutation.mutate({ body: { customerId: USER_ID, items: [{ sku: 'OUT-1', quantity: 1 }] } });
  assert.equal(failed, undefined);
  assert.ok(cli.OutOfStock.matches(mutation.error.value));
  assert.deepEqual((mutation.error.value as cli.OutOfStock).data, { sku: 'OUT-1' });
  mutation.reset();
  assert.equal(mutation.error.value, undefined);

  // ---- the plugin provides the client through provide/inject ------------------------------------
  const vue = createApp({});
  vue.use(createVeldPlugin({ orders: client }));
  assert.equal(
    vue.runWithContext(() => useOrdersClient()),
    client,
  );
  assert.throws(() => createApp({}).runWithContext(() => useOrdersClient()), /createVeldPlugin/);

  server.close();
  process.stdout.write('vue composables ok\n');
  process.exit(0);
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
