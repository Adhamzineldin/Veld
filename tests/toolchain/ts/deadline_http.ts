// Deadlines over the production handler and the generated client (spec/platform.md section 12,
// spec/targets/typescript.md section 5j). Expects `./server` and `./client` generated from
// tests/toolchain/contract/platform_p2/service_levels.veld.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RequestContext, VeldRequest } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000001';
const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

interface Seen {
  deadline?: Date | undefined;
  signal?: boolean;
  aborted: boolean;
  finished: boolean;
}
const seen = new Map<string, Seen>();
let roomDelay = 0;
let buildDelays: number[] = [];
let builds = 0;

const impl: srv.RoomsService = {
  async getRoom(req, ctx: RequestContext) {
    const entry: Seen = { deadline: ctx.deadline, signal: ctx.signal !== undefined, aborted: false, finished: false };
    seen.set('getRoom', entry);
    ctx.signal?.addEventListener?.('abort', () => {
      entry.aborted = true;
    });
    await sleep(roomDelay);
    entry.finished = true;
    return { id: req.id, name: 'late' };
  },
  async listRooms(_req, ctx) {
    seen.set('listRooms', { deadline: ctx.deadline, signal: ctx.signal !== undefined, aborted: false, finished: true });
    return [];
  },
  async build(req) {
    builds += 1;
    const entry: Seen = { aborted: false, finished: false };
    seen.set(`build${String(builds)}`, entry);
    await sleep(buildDelays.shift() ?? 0);
    entry.finished = true;
    return { id: ID, name: req.body.name };
  },
};

function get(path: string, headers: Record<string, string> = {}): VeldRequest {
  return { method: 'GET', path, headers };
}

async function main(): Promise<void> {
  const errors: unknown[] = [];
  const store = new srv.veld.MemoryIdempotencyStore();
  const handler = srv.createRoomsHandler(impl, { onError: (e) => errors.push(e), idempotency: { store } });

  // A handler that answers in time: the context has the deadline and a signal; the answer is as before.
  const before = Date.now();
  const fast = await handler(get(`/rooms/${ID}`));
  assert.equal(fast?.status, 200);
  const ctx = seen.get('getRoom');
  assert.ok(ctx?.deadline instanceof Date && ctx.deadline.getTime() - before >= 190 && ctx.deadline.getTime() - before < 400);
  assert.equal(ctx?.signal, true);
  assert.equal(ctx?.aborted, false);

  // An endpoint without a deadline has neither.
  assert.equal((await handler(get('/rooms')))?.status, 200);
  assert.equal(seen.get('listRooms')?.deadline, undefined);
  assert.equal(seen.get('listRooms')?.signal, false);

  // A handler that does not answer in time: `504 DeadlineExceeded` with `timeoutMs`, the signal fires,
  // and the answer the handler gives later is dropped.
  roomDelay = 450;
  const t0 = Date.now();
  const late = await handler(get(`/rooms/${ID}`));
  assert.ok(Date.now() - t0 < 400, 'the answer came at the deadline, not when the handler finished');
  assert.equal(late?.status, 504);
  assert.equal(late?.headers['content-type'], 'application/problem+json');
  const doc = JSON.parse(late?.body ?? '{}') as { type: string; status: number; data: unknown };
  assert.equal(doc.type, srv.DeadlineExceeded.sig);
  assert.equal(doc.status, 504);
  assert.deepEqual(doc.data, { timeoutMs: '200' });
  assert.equal(seen.get('getRoom')?.aborted, true);
  assert.equal(seen.get('getRoom')?.finished, false);
  await sleep(350);
  assert.equal(seen.get('getRoom')?.finished, true, 'the handler ran on');
  assert.deepEqual(errors, [], 'a deadline is a declared error, not a failure');
  roomDelay = 0;

  // The idempotency key of a request that hit its deadline is released: a retry with the key runs,
  // and the late answer of the first request does not overwrite it.
  const post = (): VeldRequest => ({
    method: 'POST',
    path: '/rooms',
    headers: { 'idempotency-key': 'k1', 'content-type': 'application/json' },
    body: { name: 'first' },
  });
  buildDelays = [450, 0];
  const first = await handler(post());
  assert.equal(first?.status, 504);
  const retry = await handler(post());
  assert.equal(retry?.status, 201, 'the key was released at the deadline');
  assert.equal(builds, 2);
  await sleep(350);
  assert.equal(seen.get('build1')?.finished, true);
  const again = await handler(post());
  assert.equal(again?.status, 201);
  assert.equal(again?.headers['veld-idempotent-replay'], 'true', 'the stored answer is the retry, not the late one');
  assert.equal(builds, 2);

  // The generated client: the deadline is the default timeout, a caller's timeout wins.
  const server = http.createServer((_req, res) => {
    setTimeout(() => {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ id: ID, name: 'slow' }));
    }, 500);
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;
  const client = new cli.RoomsClient({ baseUrl: url });
  const t1 = Date.now();
  await assert.rejects(client.getRoom({ id: ID }), (e: unknown) => {
    assert.ok(e instanceof cli.VeldTransportError, String(e));
    assert.match((e as Error).message, /timed out after 200 ms/);
    return true;
  });
  assert.ok(Date.now() - t1 < 450);
  // The caller's timeout replaces the default.
  const room = await client.getRoom({ id: ID }, { timeoutMs: 2000 });
  assert.equal(room.name, 'slow');
  await assert.rejects(client.getRoom({ id: ID }, { timeoutMs: 50 }), /timed out after 50 ms/);
  server.close();
  console.log('deadline_http ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
