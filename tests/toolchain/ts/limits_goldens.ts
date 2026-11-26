// Every case of tests/wire/limits/cases.json through the generated runtime (spec/platform.md
// section 5, tests/wire/limits/README.md).
//
//  - `windows`: the generated memory limiter, with a clock the test controls, is called with the
//    keys and times of the case and must answer the decisions of the reference.
//  - `keys`: the production handler of the service is called, with the caller, the principal and the
//    address the case gives, and the limiter it is built with records the key it is asked about.
//
// Expects `./server` generated from tests/wire/limits/contract.veld (compiled as the file `t.veld`
// of the package `bookings`) and the cases copied next to this file as `cases.json`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

interface Window {
  name: string;
  limit: number;
  windowSeconds: number;
  calls: { key: string; t: number }[];
  decisions: { allowed: boolean; retryAfterSeconds: number }[];
}

interface KeyCase {
  endpoint: string;
  caller?: string;
  address?: string;
  principal?: { id: string; tenantId?: string };
  key: string;
}

const cases = JSON.parse(readFileSync('cases.json', 'utf8')) as { windows: Window[]; keys: KeyCase[] };

for (const w of cases.windows) {
  let t = 0;
  const limiter = new srv.veld.MemoryLimiter({ now: () => t });
  const got = w.calls.map((call) => {
    t = call.t;
    return limiter.take(call.key, w.limit, w.windowSeconds);
  });
  assert.deepEqual(got, w.decisions, `${w.name}: the decisions`);
}

const ID = '00000000-0000-4000-8000-000000000001';
let current: KeyCase | undefined;
const asked: string[] = [];
const here = (): KeyCase => {
  assert.ok(current !== undefined);
  return current;
};
const limiter: srv.veld.Limiter = {
  take(key) {
    asked.push(key);
    return { allowed: true, retryAfterSeconds: 0 };
  },
};
const room = { id: ID };

const rooms = srv.createRoomsHandler(
  {
    byUser: async () => room,
    byTenant: async () => [room],
    byIp: async () => [room],
    archive: async () => undefined,
  },
  {
    auth: { bearer: async () => here().principal ?? { id: ID } },
    caller: async () => here().caller ?? '',
    address: async () => here().address ?? '',
    limiter,
  },
);
const open = srv.createPublicHandler(
  { listing: async () => [room], hot: async () => [room] },
  { address: async () => here().address ?? '', limiter },
);

// The request that reaches each endpoint of the contract.
const routes: Record<string, { handler: srv.veld.VeldHandler; method: string; path: string }> = {
  byUser: { handler: rooms, method: 'GET', path: `/rooms/${ID}` },
  byTenant: { handler: rooms, method: 'GET', path: '/rooms/tenant' },
  byIp: { handler: rooms, method: 'GET', path: '/rooms/ip' },
  archive: { handler: rooms, method: 'POST', path: `/rooms/${ID}/archive` },
  listing: { handler: open, method: 'GET', path: '/public' },
  hot: { handler: open, method: 'GET', path: '/public/hot' },
};

async function keys(): Promise<void> {
  for (const c of cases.keys) {
    current = c;
    asked.length = 0;
    const route = routes[c.endpoint];
    assert.ok(route !== undefined, `the contract has no endpoint ${c.endpoint}`);
    const res = await route.handler({ method: route.method, path: route.path, headers: { authorization: 'Bearer t' } });
    assert.ok(res !== null && res.status < 300, `${c.endpoint}: the call is allowed (${String(res?.status)})`);
    assert.deepEqual(asked, [c.key], `${c.endpoint}: the key`);
  }
}

keys()
  .then(() => {
    console.log(`limit goldens: ${cases.windows.length} windows, ${cases.keys.length} keys`);
  })
  .catch((err: unknown) => {
    console.error(err);
    process.exit(1);
  });
