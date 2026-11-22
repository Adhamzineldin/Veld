// Runtime test of the generated query options (`query-options = true`). The generated `queries.ts`
// imports nothing from any package; THIS test imports a pinned @tanstack/react-query to drive the
// plain objects through useQuery, useMutation and invalidateQueries against the REAL generated
// Express-served server, and shows the same objects working with a hand-written hook.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import express from 'express';
import { QueryClient, QueryClientProvider, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { act, create } from 'react-test-renderer';
import type { ReactTestRenderer } from 'react-test-renderer';
import * as srv from './server/index';
import * as cli from './copy_a/index';
import { OrdersClient } from './copy_a/orders/client';
import { createOrdersQueries, ordersMutations, ordersQueries } from './copy_a/orders/queries';
import type { OrdersGetOrderError } from './copy_a/orders/queries';
import type { FetchInit } from './copy_a/runtime/veld';
import { hooks, impl } from './roundtrip';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const USER_ID = '11111111-1111-4111-8111-111111111111';

/** A fetch that records every call, can delay it, and remembers the abort signals it saw. */
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
  for (let i = 0; i < 400; i += 1) {
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

function newQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: Infinity, staleTime: Infinity } },
  });
}

/** A tiny library-agnostic hook: SWR-style, built only from the plain query options. */
function useFetched<T>(options: { queryKey: readonly unknown[]; queryFn: (ctx: { signal?: undefined }) => Promise<T> }) {
  const [state, set] = useState<{ data?: T; error?: unknown; done: boolean }>({ done: false });
  const key = JSON.stringify(options.queryKey);
  useEffect(() => {
    let live = true;
    options.queryFn({}).then(
      (data) => live && set({ data, done: true }),
      (error: unknown) => live && set({ error, done: true }),
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return state;
}

async function main(): Promise<void> {
  const app = express();
  app.use(srv.toExpress(srv.createOrdersHandler(impl, hooks)));
  const server = http.createServer(app);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;

  const plain = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  const first = await plain.createOrder({ body: { customerId: USER_ID, items: [{ sku: 'ABC-1', quantity: 1 }] } });

  const rec = recordingFetch();
  const client = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: rec.fetchLike });
  const queries = newQueryClient();
  const wrap = (child: JSX.Element, qc = queries): JSX.Element => (
    <QueryClientProvider client={qc}>{child}</QueryClientProvider>
  );

  // ---- keys: stable, built from the canonical bytes of the request ---------------------------------
  assert.deepEqual(ordersQueries.keys.all, ['veld', 'orders']);
  assert.equal(
    JSON.stringify(ordersQueries.keys.getOrder({ id: 'x' })),
    JSON.stringify(['veld', 'orders', 'getOrder', '{"id":"x"}']),
  );
  assert.deepEqual(ordersMutations.createOrder(client).mutationKey, ['veld', 'orders', 'createOrder']);

  // ---- useQuery with the plain object ---------------------------------------------------------------
  const seen: Seen<ReturnType<typeof useQuery<cli.Order, OrdersGetOrderError>>> = { current: undefined };
  function Order(props: { id: string }) {
    seen.current = useQuery<cli.Order, OrdersGetOrderError>(ordersQueries.getOrder({ id: props.id }, client));
    return null;
  }
  let root!: ReactTestRenderer;
  await act(async () => {
    root = create(wrap(<Order id={first.id} />));
  });
  await until('data', () => peek(seen)?.isSuccess === true);
  assert.equal(peek(seen)?.data?.id, first.id);
  assert.ok(queries.getQueryData(ordersQueries.keys.getOrder({ id: first.id })), 'cached under the generated key');

  // A typed error keeps its class: `NotFound.matches(e)` works on what the query hands back.
  await act(async () => {
    root.update(wrap(<Order id={randomUUID()} />));
  });
  await until('error', () => peek(seen)?.isError === true);
  const err = peek(seen)?.error;
  assert.ok(cli.NotFound.matches(err), 'NotFound.matches(error)');
  assert.ok((err as object) instanceof cli.NotFound);

  // ---- the signal reaches the client: unmounting the last observer aborts the call ------------------
  const slow = recordingFetch(120);
  const slowClient = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: slow.fetchLike });
  let slowRoot!: ReactTestRenderer;
  function Slow() {
    useQuery(ordersQueries.health({}, slowClient));
    return null;
  }
  await act(async () => {
    slowRoot = create(wrap(<Slow />, newQueryClient()));
  });
  assert.equal(slow.signals.length, 1);
  await act(async () => {
    slowRoot.unmount();
  });
  assert.equal(slow.signals[0]?.aborted, true, 'the query function aborts the call');
  await act(async () => {
    await new Promise((r) => setTimeout(r, 160));
  });

  // ---- useMutation + invalidateQueries over the service key -----------------------------------------
  const list: Seen<ReturnType<typeof useQuery>> = { current: undefined };
  const mutation: Seen<ReturnType<typeof useMutation<cli.Order, Error, cli.CreateOrderRequest>>> = { current: undefined };
  function Everything() {
    const qc = useQueryClient();
    list.current = useQuery(ordersQueries.listOrders({ query: {} }, client));
    mutation.current = useMutation<cli.Order, Error, cli.CreateOrderRequest>({
      ...ordersMutations.createOrder(client),
      onSuccess: () => qc.invalidateQueries({ queryKey: ordersQueries.keys.all }),
    });
    return null;
  }
  await act(async () => {
    create(wrap(<Everything />, newQueryClient()));
  });
  await until('the list', () => peek(list)?.isSuccess === true);
  const total = (peek(list)?.data as { items: unknown[] }).items.length;
  let created: cli.Order | undefined;
  await act(async () => {
    created = await mutation.current?.mutateAsync({
      body: { customerId: USER_ID, items: [{ sku: 'NEW-1', quantity: 3 }] },
    });
  });
  assert.equal(created?.items[0]?.sku, 'NEW-1');
  await until('the list refetched', () => ((peek(list)?.data as { items: unknown[] }).items.length) === total + 1);
  // A failed mutation is typed, and `onSuccess` (the invalidation) does not run.
  const callsBeforeFail = rec.calls.length;
  let failure: unknown;
  await act(async () => {
    failure = await mutation.current
      ?.mutateAsync({ body: { customerId: USER_ID, items: [{ sku: 'OUT-1', quantity: 1 }] } })
      .catch((e: unknown) => e);
  });
  assert.ok(cli.OutOfStock.matches(failure), 'a typed error from a mutation');
  assert.equal(rec.calls.length, callsBeforeFail + 1, 'no refetch after a failed mutation');

  // ---- a custom default client: no global, the factory decides --------------------------------------
  let made = 0;
  const custom = createOrdersQueries(() => {
    made += 1;
    return client;
  });
  const viaDefault = await custom.getOrder({ id: first.id }).queryFn({});
  assert.equal(viaDefault.id, first.id);
  assert.equal(made, 1, 'the default client comes from the factory, once per call');

  // ---- the same objects with a hand-written hook (no TanStack involved) -----------------------------
  const own: Seen<ReturnType<typeof useFetched<cli.Order>>> = { current: undefined };
  function Own() {
    own.current = useFetched(ordersQueries.getOrder({ id: first.id }, client));
    return null;
  }
  await act(async () => {
    create(<Own />);
  });
  await until('the custom hook', () => peek(own)?.done === true);
  assert.equal(peek(own)?.data?.id, first.id, 'plain options work without any query library');

  server.closeAllConnections();
  server.close();
  process.stdout.write('query-options ok\n');
  process.exit(0);
}

main().catch((e: unknown) => {
  process.stderr.write(`${String(e instanceof Error ? (e.stack ?? e.message) : e)}\n`);
  process.exit(1);
});
