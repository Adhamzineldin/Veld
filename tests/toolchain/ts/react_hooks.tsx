// Runtime test of the generated React hooks: components rendered with react-test-renderer call the
// REAL generated Express-served server through the generated client.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import express from 'express';
import { act, create } from 'react-test-renderer';
import type { ReactTestRenderer } from 'react-test-renderer';
import * as srv from './server/index';
import * as cli from './copy_a/index';
import { OrdersClient } from './copy_a/orders/client';
import { VeldProvider, useCreateOrder, useGetOrder, useHealth, useOrdersClient } from './copy_a/orders/react';
import type { FetchInit } from './copy_a/runtime/veld';
import { hooks, impl, runtime } from './roundtrip';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const USER_ID = '11111111-1111-4111-8111-111111111111';

/** A fetch that records every call, can delay it, and remembers the abort signals it was given. */
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

async function until(label: string, cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300; i += 1) {
    if (cond()) return;
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  assert.fail(`timed out waiting for: ${label}`);
}

type Seen<T> = { current: T | undefined };
// Reads through a function so the compiler does not narrow what the assertions already checked.
const peek = <T,>(s: Seen<T>): T | undefined => s.current;

async function main(): Promise<void> {
  const app = express();
  app.use(srv.toExpress(srv.createOrdersHandler(impl, hooks, runtime)));
  const server = http.createServer(app);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;

  const plain = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  const first = await plain.createOrder({
    body: { customerId: USER_ID, items: [{ sku: 'ABC-1', quantity: 1 }] },
  });
  const second = await plain.createOrder({
    body: { customerId: USER_ID, items: [{ sku: 'ABC-2', quantity: 2 }] },
  });

  const rec = recordingFetch();
  const client = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: rec.fetchLike });

  // ---- a query: loading, then data; then the request changes; then refetch ----------------------
  const seen: Seen<ReturnType<typeof useGetOrder>> = { current: undefined };
  const now = () => peek(seen);
  function Order(props: { id: string }) {
    seen.current = useGetOrder({ id: props.id });
    return <span>{seen.current.loading ? 'loading' : (seen.current.data?.id ?? 'none')}</span>;
  }
  let root!: ReactTestRenderer;
  await act(async () => {
    root = create(
      <VeldProvider clients={{ orders: client }}>
        <Order id={first.id} />
      </VeldProvider>,
    );
  });
  assert.equal(now()?.loading, true, 'loading first');
  assert.equal(now()?.data, undefined);
  assert.equal(JSON.stringify(root.toJSON()), JSON.stringify({ type: 'span', props: {}, children: ['loading'] }));
  await until('data', () => now()?.loading === false);
  assert.equal(now()?.data?.id, first.id);
  assert.equal(now()?.error, undefined);
  assert.equal(JSON.stringify(root.toJSON()), JSON.stringify({ type: 'span', props: {}, children: [first.id] }));

  // The same request as a NEW object has the same canonical bytes: no second call.
  const callsBefore = rec.calls.length;
  await act(async () => {
    root.update(
      <VeldProvider clients={{ orders: client }}>
        <Order id={first.id} />
      </VeldProvider>,
    );
  });
  await act(async () => {
    await new Promise((r) => setTimeout(r, 40));
  });
  assert.equal(rec.calls.length, callsBefore, 'a stable key does not refetch');

  // A different request: loading again with no stale data, the old call is aborted, then new data.
  const firstSignal = rec.signals[rec.signals.length - 1];
  await act(async () => {
    root.update(
      <VeldProvider clients={{ orders: client }}>
        <Order id={second.id} />
      </VeldProvider>,
    );
  });
  assert.equal(now()?.loading, true, 'loading after the request changed');
  assert.equal(now()?.data, undefined, 'no stale data for the new request');
  assert.equal(firstSignal?.aborted, true, 'the previous call is aborted on a request change');
  await until('second order', () => now()?.data?.id === second.id);

  // refetch re-runs the call and keeps the data while it loads.
  await plain.payOrder({ id: second.id, body: { kind: 'bank', iban: 'DE00' } });
  assert.equal(now()?.data?.status, 'pending');
  await act(async () => {
    now()?.refetch();
  });
  assert.equal(now()?.loading, true);
  assert.equal(now()?.data?.id, second.id, 'data is kept while refetching');
  await until('refetched', () => now()?.data?.status === 'paid');
  assert.equal(now()?.loading, false);

  // A typed error stays the generated class: `matches()` works.
  await act(async () => {
    root.update(
      <VeldProvider clients={{ orders: client }}>
        <Order id={randomUUID()} />
      </VeldProvider>,
    );
  });
  await until('error', () => now()?.error !== undefined);
  const err = now()?.error;
  assert.ok(cli.NotFound.matches(err), 'NotFound.matches(error)');
  assert.ok((err as object) instanceof cli.NotFound);
  assert.equal(now()?.data, undefined);
  assert.equal(now()?.loading, false);

  // Unmounting aborts the call that is still in flight.
  const slow = recordingFetch(80);
  const slowClient = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: slow.fetchLike });
  let slowRoot!: ReactTestRenderer;
  function Slow() {
    useHealth({}, { client: slowClient });
    return null;
  }
  await act(async () => {
    slowRoot = create(<Slow />);
  });
  assert.equal(slow.signals.length, 1);
  assert.equal(slow.signals[0]?.aborted, false);
  await act(async () => {
    slowRoot.unmount();
  });
  assert.equal(slow.signals[0]?.aborted, true, 'aborted on unmount');
  await act(async () => {
    await new Promise((r) => setTimeout(r, 120));
  });

  // `enabled: false` and a `null` request hold the call back.
  const idle = recordingFetch();
  const idleClient = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: idle.fetchLike });
  const idleSeen: Seen<ReturnType<typeof useGetOrder>> = { current: undefined };
  function Idle() {
    idleSeen.current = useGetOrder(null, { client: idleClient });
    return null;
  }
  await act(async () => {
    create(<Idle />);
  });
  assert.equal(idleSeen.current?.loading, false);
  assert.equal(idle.calls.length, 0);

  // ---- a mutation -------------------------------------------------------------------------------
  const mutation: Seen<ReturnType<typeof useCreateOrder>> = { current: undefined };
  const nowM = () => peek(mutation);
  function Create() {
    mutation.current = useCreateOrder();
    return null;
  }
  await act(async () => {
    create(
      <VeldProvider clients={{ orders: client }}>
        <Create />
      </VeldProvider>,
    );
  });
  assert.equal(nowM()?.loading, false);
  let created: srv.Order | undefined;
  await act(async () => {
    created = await nowM()?.mutate({
      body: { customerId: USER_ID, items: [{ sku: 'NEW-1', quantity: 3 }] },
    });
  });
  assert.equal(created?.items[0]?.sku, 'NEW-1');
  assert.equal(nowM()?.data?.id, created?.id);
  assert.equal(nowM()?.loading, false);

  // A declared error is stored as the typed class; `mutate` itself never rejects.
  let failed: unknown = 'unset';
  await act(async () => {
    failed = await nowM()?.mutate({
      body: { customerId: USER_ID, items: [{ sku: 'OUT-1', quantity: 1 }] },
    });
  });
  assert.equal(failed, undefined);
  assert.ok(cli.OutOfStock.matches(nowM()?.error));
  assert.deepEqual((nowM()?.error as cli.OutOfStock).data, { sku: 'OUT-1' });
  assert.equal(nowM()?.data, undefined);
  await act(async () => {
    nowM()?.reset();
  });
  assert.equal(nowM()?.error, undefined);
  assert.equal(nowM()?.loading, false);

  // Without a provider (and no `client` option) the hook says what to do.
  function Orphan() {
    useOrdersClient();
    return null;
  }
  const logged = console.error;
  console.error = () => undefined; // React reports the render error; the assertion below is the check
  try {
    assert.throws(() => create(<Orphan />), /VeldProvider/);
  } finally {
    console.error = logged;
  }

  server.close();
  process.stdout.write('react hooks ok\n');
  process.exit(0);
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
