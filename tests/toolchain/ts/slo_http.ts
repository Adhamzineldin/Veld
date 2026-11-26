// Service levels over the production handler (spec/platform.md section 13, spec/targets/typescript.md
// section 5j): the metrics hook. Expects `./server` generated from
// tests/toolchain/contract/platform_p2/service_levels.veld.
import assert from 'node:assert/strict';
import * as srv from './server/index';
import type { Metric, VeldRequest } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000001';
let mode: 'ok' | 'missing' | 'crash' | 'slow' = 'ok';

const impl: srv.RoomsService = {
  async getRoom(req) {
    if (mode === 'missing') throw new srv.NotFound({ resource: 'room' });
    if (mode === 'crash') throw new Error('the handler broke');
    if (mode === 'slow') await new Promise((resolve) => setTimeout(resolve, 400));
    return { id: req.id, name: 'a room' };
  },
  async listRooms() {
    return [];
  },
  async build(req) {
    return { id: ID, name: req.body.name };
  },
};

const get = (path: string): VeldRequest => ({ method: 'GET', path, headers: {} });

async function main(): Promise<void> {
  const seen: Metric[] = [];
  const handler = srv.createRoomsHandler(impl, {
    idempotency: { store: new srv.veld.MemoryIdempotencyStore() },
    metrics: (m) => {
      seen.push(m);
    },
    onError: () => undefined,
  });

  // A 200: one metric, the names of the OpenTelemetry conventions, no `error.type`.
  assert.equal((await handler(get(`/rooms/${ID}`)))?.status, 200);
  assert.equal(seen.length, 1);
  const ok = seen[0] as Metric;
  assert.equal(ok.name, 'http.server.request.duration');
  assert.equal(ok.unit, 's');
  assert.ok(ok.value >= 0 && ok.value < 1);
  assert.deepEqual(Object.keys(ok.attributes).sort(), [
    'http.request.method',
    'http.response.status_code',
    'http.route',
    'veld.endpoint',
    'veld.operation',
    'veld.service',
  ]);
  assert.equal(ok.attributes['http.request.method'], 'GET');
  assert.equal(ok.attributes['http.route'], '/rooms/{id}');
  assert.equal(ok.attributes['http.response.status_code'], 200);
  assert.equal(ok.attributes['veld.service'], 'bookings/Rooms');
  assert.equal(ok.attributes['veld.endpoint'], 'getRoom');

  // A declared error: the status of the error, no `error.type` below 500; `veld.operation` is the
  // operation signature the problem document names.
  mode = 'missing';
  const missing = await handler(get(`/rooms/${ID}`));
  assert.equal(missing?.status, 404);
  const operation = (JSON.parse(missing?.body ?? '{}') as { operation: string }).operation;
  const notFound = seen[1] as Metric;
  assert.equal(notFound.attributes['http.response.status_code'], 404);
  assert.equal(notFound.attributes['error.type'], undefined);
  assert.equal(notFound.attributes['veld.operation'], operation);
  assert.match(operation, /^veld:/);

  // A 500: `error.type`.
  mode = 'crash';
  assert.equal((await handler(get(`/rooms/${ID}`)))?.status, 500);
  const crashed = seen[2] as Metric;
  assert.equal(crashed.attributes['http.response.status_code'], 500);
  assert.equal(crashed.attributes['error.type'], '500');

  // A request that hit its deadline is a 504 with `error.type`.
  mode = 'slow';
  assert.equal((await handler(get(`/rooms/${ID}`)))?.status, 504);
  const slow = seen[3] as Metric;
  assert.equal(slow.attributes['http.response.status_code'], 504);
  assert.equal(slow.attributes['error.type'], '504');
  assert.ok(slow.value >= 0.19);

  // Every endpoint of the server is measured, with or without its own objective; a route that
  // matches nothing is not.
  mode = 'ok';
  assert.equal((await handler(get('/rooms')))?.status, 200);
  assert.equal(seen[4]?.attributes['veld.endpoint'], 'listRooms');
  assert.equal(await handler(get('/nowhere')), null);
  assert.equal(seen.length, 5);

  // A failing hook is ignored: a throwing one and one that rejects.
  for (const metrics of [
    (): void => {
      throw new Error('the hook broke');
    },
    (): Promise<void> => Promise.reject(new Error('the hook rejected')),
  ]) {
    const broken = srv.createRoomsHandler(impl, { idempotency: { store: new srv.veld.MemoryIdempotencyStore() }, metrics });
    const res = await broken(get(`/rooms/${ID}`));
    assert.equal(res?.status, 200);
  }
  // A server without the hook works as before.
  const bare = srv.createRoomsHandler(impl, { idempotency: { store: new srv.veld.MemoryIdempotencyStore() } });
  assert.equal((await bare(get(`/rooms/${ID}`)))?.status, 200);
  console.log('slo_http ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
