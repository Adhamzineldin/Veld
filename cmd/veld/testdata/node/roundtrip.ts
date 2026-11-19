// Express server + typed client round trip for the node-ts backend and the
// TypeScript client (B-015, M1). cmd/veld/node_toolchain_test.go generates
// app.veld next to this file into ./generated, compiles both with
// `tsc --strict` and runs the result. It exits non-zero if any check fails.
//
// Checks named after a finding fail on the output Veld generated before the
// fix (docs/review-2026-10/csv/defect-register.csv).
import express from 'express';
import type { AddressInfo } from 'node:net';
import { strict as assert } from 'node:assert';

import { itemsRouter } from './generated/routes/items.routes';
import type { IItemsService } from './generated/interfaces/IItemsService';
import type { IMiddleware } from './generated/middleware/IMiddleware';
import { Item, Ok, SearchQuery, CreateItemInput, SecureHeaders } from './generated/types';
import { ItemsClient } from './generated/client/itemsApi';
import { VeldApiError } from './generated/client/errors';

const SECRET = 'db password=hunter2 at /srv/x';
const calls: string[] = [];

function item(id: string, name: string): Item {
  return new Item({
    id: id as Item['id'],
    name,
    price: '9.99' as Item['price'],
    count: 1,
    createdAt: '2026-11-18T00:00:00Z' as Item['createdAt'],
    tags: [],
    meta: {},
  });
}

const service: IItemsService = {
  async getItem(id: string) { calls.push('getItem'); return item(id, 'by-id'); },
  async getMe() { calls.push('getMe'); return item('me', 'me'); },
  async getGuarded() { calls.push('getGuarded'); return new Ok({ ok: true }); },
  async search(query: SearchQuery) { calls.push('search'); return [item(String(query.q), 'search')]; },
  async createItem(input: CreateItemInput) { calls.push('createItem'); return item('new', input.name); },
  async deleteItem(_id: string) { calls.push('deleteItem'); },
  async lookup(input: SearchQuery) {
    calls.push('lookup');
    assert.ok(input instanceof SearchQuery, 'GET input should arrive as a SearchQuery instance');
    return [item(String(input.q), 'lookup')];
  },
  async boom() { calls.push('boom'); throw new Error(SECRET); },
  async secure(headers: SecureHeaders) {
    calls.push('secure');
    if (headers['X-Trace-Id'] !== 'trace-1') {
      throw Object.assign(new Error('missing trace id'), { status: 400, code: 'NO_TRACE' });
    }
    return new Ok({ ok: true });
  },
};

const middleware: IMiddleware = {
  authGuard(req, res, next) {
    calls.push('authGuard');
    if (req.headers['authorization'] !== 'Bearer ok') {
      res.status(401).json({ error: 'unauthorized' });
      return;
    }
    next();
  },
};

async function expectApiError(p: Promise<unknown>, status: number): Promise<VeldApiError> {
  try {
    await p;
  } catch (err) {
    assert.ok(err instanceof VeldApiError, `expected VeldApiError, got ${String(err)}`);
    assert.equal(err.status, status);
    return err;
  }
  throw new assert.AssertionError({ message: `expected HTTP ${status}, request succeeded` });
}

async function main(): Promise<void> {
  const app = express();
  app.use(express.json());
  const router = express.Router();
  itemsRouter(router, service, middleware);
  app.use(router);
  const server = app.listen(0, '127.0.0.1');
  await new Promise<void>((resolve) => server.once('listening', () => resolve()));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  const client = new ItemsClient(base);

  const checks: Array<[string, () => Promise<void>]> = [
    ['path param round trip', async () => {
      assert.equal((await client.getItem('abc')).id, 'abc');
    }],
    ['F-NODE-11 reserved characters in a path param', async () => {
      assert.equal((await client.getItem('a/b?x=1#h %')).id, 'a/b?x=1#h %');
    }],
    ['F-NODE-05 static route declared after /items/:id', async () => {
      calls.length = 0;
      assert.equal((await client.getMe()).name, 'me');
      assert.deepEqual(calls, ['getMe']);
    }],
    ['F-NODE-05 guarded static route runs its middleware', async () => {
      calls.length = 0;
      await expectApiError(client.getGuarded(), 401);
      assert.deepEqual(calls, ['authGuard']);
      const authed = new ItemsClient({ baseUrl: base, headers: { Authorization: 'Bearer ok' } });
      assert.equal((await authed.getGuarded()).ok, true);
    }],
    ['query model', async () => {
      const found = await client.search(new SearchQuery({ q: 'a b&c=d' }));
      assert.equal(found[0].id, 'a b&c=d');
    }],
    ['GET input travels as query parameters', async () => {
      const found = await client.lookup(new SearchQuery({ q: 'x?y' }));
      assert.equal(found[0].id, 'x?y');
    }],
    ['POST returns 201 and the created item', async () => {
      const created = await client.createItem(new CreateItemInput({ name: 'widget', count: 2, tags: ['x'] }));
      assert.equal(created.name, 'widget');
    }],
    ['DELETE without output resolves to undefined (204)', async () => {
      assert.equal(await client.deleteItem('abc'), undefined);
    }],
    ['headers block reaches the service', async () => {
      assert.equal((await client.secure(new SecureHeaders({ 'X-Trace-Id': 'trace-1' }))).ok, true);
    }],
    ['4xx errors keep their message and code', async () => {
      const err = await expectApiError(client.secure(new SecureHeaders({ 'X-Trace-Id': 'wrong' })), 400);
      assert.equal(err.body, 'missing trace id');
      assert.equal(err.code, 'NO_TRACE');
    }],
    ['F-NODE-10 500 does not leak the internal message', async () => {
      const err = await expectApiError(client.boom(), 500);
      assert.ok(!err.body.includes('hunter2'), `500 body leaked: ${err.body}`);
      assert.equal(err.body, 'Internal Server Error');
    }],
  ];

  let failed = 0;
  for (const [name, run] of checks) {
    try {
      await run();
      console.log(`ok   ${name}`);
    } catch (err) {
      failed++;
      console.log(`FAIL ${name}: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  server.closeAllConnections();
  await new Promise<void>((resolve) => server.close(() => resolve()));
  console.log(failed > 0 ? `${failed} of ${checks.length} checks failed` : `all ${checks.length} checks passed`);
  if (failed > 0) process.exitCode = 1;
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
