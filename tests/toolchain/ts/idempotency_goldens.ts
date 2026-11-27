// Every case of tests/wire/idempotency/fingerprints.json through the generated server
// (spec/rules.md section 11.10, tests/wire/idempotency/README.md). Each request is sent to the
// production handler of the service `Fingerprints`; a store that records what `begin` is given
// shows the fingerprint it took. Expects `./server` generated from fingerprints.veld (compiled as
// the file `t.veld` of the package `bookings`) and the cases copied next to this file as
// `fingerprints.json`.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

interface Case {
  name: string;
  endpoint: string;
  request: {
    method: string;
    path: string;
    query: Record<string, string | string[]>;
    headers: Record<string, string>;
    body?: unknown;
    body_text?: string;
  };
  same_as?: string;
  differs_from?: string;
  hashed?: string;
  fingerprint?: string;
  rejected?: string;
}

const cases = JSON.parse(readFileSync('fingerprints.json', 'utf8')) as Case[];

let memory = new srv.veld.MemoryIdempotencyStore();
let seen: string[] = [];
const store: srv.veld.IdempotencyStore = {
  async begin(scope, key, fingerprint) {
    seen.push(fingerprint);
    return memory.begin(scope, key, fingerprint);
  },
  complete: (scope, key, answer) => memory.complete(scope, key, answer),
  release: (scope, key) => memory.release(scope, key),
};

// Every endpoint answers 204 and does nothing: the goldens look at the request, not the answer.
const impl = new Proxy({}, { get: () => async () => undefined }) as unknown as srv.FingerprintsService;
const handler = srv.createFingerprintsHandler(impl, { idempotency: { store } });

/** The path with every segment percent-encoded: the case holds the decoded text. */
function encodedPath(path: string): string {
  return path
    .split('/')
    .map((s) => encodeURIComponent(s))
    .join('/');
}

function queryText(query: Record<string, string | string[]>): string {
  const pairs: string[] = [];
  for (const k of Object.keys(query)) {
    const v = query[k];
    for (const one of Array.isArray(v) ? v : [v]) pairs.push(`${encodeURIComponent(k)}=${encodeURIComponent(String(one))}`);
  }
  return pairs.join('&');
}

async function send(c: Case): Promise<{ status: number; fingerprints: string[] }> {
  memory = new srv.veld.MemoryIdempotencyStore();
  seen = [];
  const headers: Record<string, string> = {};
  for (const k of Object.keys(c.request.headers)) headers[k.toLowerCase()] = String(c.request.headers[k]);
  const text = c.request.body_text;
  const res = await handler({
    method: c.request.method,
    path: encodedPath(c.request.path),
    query: queryText(c.request.query),
    headers,
    ...(c.request.body === undefined ? {} : { body: c.request.body }),
    ...(text === undefined ? {} : { text: async () => text }),
  });
  assert.ok(res !== null, `${c.name}: no route matched`);
  return { status: res.status, fingerprints: seen };
}

async function main(): Promise<void> {
  const taken = new Map<string, string>();
  for (const c of cases) {
    const { status, fingerprints } = await send(c);
    if (c.rejected !== undefined) {
      assert.equal(String(status), c.rejected, `${c.name}: the status`);
      assert.deepEqual(fingerprints, [], `${c.name}: a rejected request is not hashed`);
      continue;
    }
    assert.equal(status, 204, `${c.name}: the status`);
    assert.equal(fingerprints.length, 1, `${c.name}: begin is asked once`);
    const fingerprint = fingerprints[0] ?? '';
    // The golden is the hash of the canonical text it prints: a damaged file is not blamed on the server.
    assert.equal(createHash('sha256').update(c.hashed ?? '').digest('hex').slice(0, 16), c.fingerprint, `${c.name}: the golden`);
    assert.equal(fingerprint, c.fingerprint, `${c.name}: the fingerprint of ${c.hashed ?? ''}`);
    taken.set(c.name, fingerprint);
  }
  let checked = 0;
  for (const c of cases) {
    const mine = taken.get(c.name);
    if (mine === undefined) continue;
    if (c.same_as !== undefined) {
      assert.equal(mine, taken.get(c.same_as), `${c.name} is the same as ${c.same_as}`);
      checked += 1;
    }
    if (c.differs_from !== undefined) {
      assert.notEqual(mine, taken.get(c.differs_from), `${c.name} differs from ${c.differs_from}`);
      checked += 1;
    }
  }
  assert.ok(checked > 0);
  console.log(`idempotency goldens ok: ${cases.length} cases, ${checked} comparisons`);
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
