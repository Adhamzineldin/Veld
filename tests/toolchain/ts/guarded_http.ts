// A contract that uses every guard at once (an inline contract of crates/emitters/node/tests/policies.rs):
// two `allow` lines, one of which reads a request parameter, a transition, and a `require` and an
// `ensure` that read the caller. Expects `./server` and `./client` generated from it.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldHandler } from './server/runtime/veld';

const ME = '00000000-0000-4000-8000-0000000000aa';
const YOU = '00000000-0000-4000-8000-0000000000bb';
const ID = '00000000-0000-4000-8000-000000000002';

const users = new Map<string, srv.User>([
  ['senior', { id: ME, level: 5 }],
  ['junior', { id: ME, level: 1 }],
  ['zero', { id: ME, level: 0 }],
  ['stranger', { id: YOU, level: 9 }],
]);
const items = new Map<string, srv.Item>();
const seen: unknown[] = [];
let owner = ME;

const impl: srv.BookingsService = {
  async publish(req) {
    seen.push(req.transition);
    return { ...req.item, status: 'live', owner };
  },
};

function listen(handler: VeldHandler): Promise<{ url: string; close(): void }> {
  const server = http.createServer(srv.toNodeHttp(handler));
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const port = (server.address() as AddressInfo).port;
      resolve({ url: `http://127.0.0.1:${String(port)}`, close: () => server.close() });
    });
  });
}

interface Doc {
  title: string;
  data: Record<string, unknown>;
  violations?: { rule: string; path: string; fields: string[]; message: string }[];
}

async function publish(url: string, token: string, need: number): Promise<{ status: number; doc: Doc }> {
  const res = await fetch(`${url}/items/${ID}/publish/${String(need)}`, {
    method: 'POST',
    headers: { authorization: `Bearer ${token}` },
  });
  const text = await res.text();
  return { status: res.status, doc: (text === '' ? {} : JSON.parse(text)) as Doc };
}

function stock(status: srv.Stage): void {
  items.clear();
  seen.length = 0;
  owner = ME;
  items.set(ID, { id: ID, owner: ME, status, note: 'n' });
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  const server = await listen(
    srv.createBookingsHandler(impl, {
      facts: { item: async (key) => items.get(key) },
      auth: { bearer: async (token): Promise<srv.User | null> => users.get(token) ?? null },
      onRuleViolation: (e) => events.push(e),
    }),
  );

  // The first allow reads the parameter `need`: it is in `fields`, and a denial wins over the guard.
  stock('live');
  let r = await publish(server.url, 'junior', 3);
  assert.equal(r.status, 403, 'a denial wins over the transition guard');
  assert.deepEqual(r.doc.violations?.[0]?.fields, ['need']);
  assert.deepEqual(r.doc.data, { message: 'not permitted' });

  // The second allow denies a stranger; its arguments read no parameter.
  stock('draft');
  r = await publish(server.url, 'stranger', 3);
  assert.equal(r.status, 403);
  assert.deepEqual(r.doc.violations?.[0]?.fields, []);

  // Permitted: the transition guard answers next, then require, which reads the caller.
  stock('live');
  r = await publish(server.url, 'senior', 3);
  assert.equal(r.status, 409);
  stock('draft');
  r = await publish(server.url, 'zero', 0);
  assert.equal(r.status, 422, 'require reads the caller');
  stock('draft');
  r = await publish(server.url, 'senior', 3);
  assert.equal(r.status, 201);
  assert.deepEqual(seen[0], { field: 'status', from: 'draft', to: 'live' });

  // An ensure that reads the caller: the result must belong to them.
  stock('draft');
  owner = YOU;
  events.length = 0;
  r = await publish(server.url, 'senior', 3);
  assert.equal(r.status, 500);
  assert.equal(events[0]?.position, 'output');

  // can<Endpoint> takes the fact, the caller and the parameter `need`, and agrees with the server.
  for (const status of ['draft', 'live'] as const) {
    for (const [token, user] of users) {
      for (const need of [0, 3, 7]) {
        stock(status);
        const answer = await publish(server.url, token, need);
        const item = items.get(ID) as srv.Item;
        const allowed = cli.BookingsClient.canPublish(item, user, need);
        // 403 and 409 are the two answers `can` is about; require and ensure are not.
        assert.equal(allowed, answer.status !== 403 && answer.status !== 409, `${status} ${token} ${String(need)}`);
      }
    }
  }
  server.close();
  console.log('guarded over HTTP: ok');
}

void main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
