// Runtime test of the target option `validate = false` (generated for the node server AND the
// typescript client): requests are not validated at run time, the types are unchanged, and
// `validate: true` brings the checks back.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import express from 'express';
import * as srv from './server/index';
import * as cli from './copy_a/index';
import { OrdersClient } from './copy_a/orders/client';
import type { FetchInit } from './copy_a/runtime/veld';
import { hooks, impl } from './roundtrip';

async function main(): Promise<void> {
  const app = express();
  app.use(srv.toExpress(srv.createOrdersHandler(impl, hooks)));
  const server = http.createServer(app);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;
  const bad = 'not-a-uuid';

  // The server: a path parameter that is not a uuid reaches the implementation (404 from it), not 422.
  const raw = await fetch(`${base}/orders/${bad}`, { headers: { authorization: 'Bearer good' } });
  assert.equal(raw.status, 404, 'the server did not validate the request');

  // The client: validation is off by default, so the call is made and the server's answer is typed.
  let calls = 0;
  const counting = (url: string, init: FetchInit) => {
    calls += 1;
    return fetch(url, init as unknown as RequestInit);
  };
  const client = new OrdersClient({ baseUrl: base, auth: { bearer: () => 'good' }, fetch: counting });
  await assert.rejects(client.getOrder({ id: bad }), (e: unknown) => cli.NotFound.matches(e));
  assert.equal(calls, 1, 'the request was sent without local validation');

  // Asking for validation again restores the local check: nothing is sent.
  const strict = new OrdersClient({ baseUrl: base, validate: true, fetch: counting });
  await assert.rejects(strict.getOrder({ id: bad }), (e: unknown) => cli.ValidationError.matches(e));
  assert.equal(calls, 1, 'validate: true rejects before sending');

  server.closeAllConnections();
  server.close();
  process.stdout.write('no-validate ok\n');
  process.exit(0);
}

main().catch((e: unknown) => {
  process.stderr.write(`${String(e instanceof Error ? (e.stack ?? e.message) : e)}\n`);
  process.exit(1);
});
