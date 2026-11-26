// The fingerprint of a request (spec/rules.md section 11.9): the first 16 hex characters of the
// SHA-256 of the RFC 8785 JSON of the bound parameters in wire form. The goldens are the ones of
// the reference evaluator (crates/veld-rules/src/idempotency.rs and tests/version_idempotency.rs),
// so the generated runtime computes the same fingerprints as the Rust reference. Expects `./server`
// generated from the contract in crates/emitters/node/tests/version_idempotency.rs (service `F`).
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import * as srv from './server/index';

const ID = '00000000-0000-4000-8000-000000000001';
const MIXED = '0000000a-0000-4000-8000-00000000000b';

const seen: { key: string; fingerprint: string }[] = [];
const memory = new srv.veld.MemoryIdempotencyStore();
const store: srv.veld.IdempotencyStore = {
  async begin(scope, key, fingerprint) {
    seen.push({ key, fingerprint });
    return memory.begin(scope, key, fingerprint);
  },
  complete: (scope, key, answer) => memory.complete(scope, key, answer),
  release: (scope, key) => memory.release(scope, key),
};

const handler = srv.createFHandler(
  {
    async make(req) {
      return req.body;
    },
    async ping() {
      return { n: 1, note: 'none' };
    },
    async stamp() {
      return { n: 1, note: 'none' };
    },
  },
  { idempotency: { store } },
);

async function call(method: string, path: string, key: string, body?: unknown): Promise<string> {
  const res = await handler({ method, path, headers: { 'idempotency-key': key }, ...(body === undefined ? {} : { body }) });
  assert.ok(res !== null && res.status < 300, `${path}: ${String(res?.status)} ${String(res?.body)}`);
  const last = seen[seen.length - 1];
  assert.equal(last?.key, key);
  return last?.fingerprint ?? '';
}

async function main(): Promise<void> {
  // `note` has a default and is added; `tag` and `dry` were not sent and are not there.
  assert.equal(await call('PUT', `/f/${ID}`, 'a', { n: 1 }), 'bc508ee59e0a79b7');
  // The same request with the default written out has the same fingerprint.
  assert.equal(await call('PUT', `/f/${ID}`, 'b', { n: 1, note: 'none' }), 'bc508ee59e0a79b7');
  // An optional parameter that was sent counts, and so does an optional member.
  const query = (await handler({
    method: 'PUT',
    path: `/f/${ID}`,
    query: 'dry=true',
    headers: { 'idempotency-key': 'c' },
    body: { n: 1, tag: 'a' },
  }))?.status;
  assert.equal(query, 200);
  assert.equal(seen[seen.length - 1]?.fingerprint, '62c08a8b20a7323e');
  // No parameters at all: the fingerprint of `{}`.
  assert.equal(await call('POST', '/f/ping', 'd'), '44136fa355b3678a');
  // A uuid in capitals is the same parameter as in lower case.
  const lower = await call('PUT', `/f/${MIXED}`, 'e', { n: 1 });
  assert.equal(await call('PUT', `/f/${MIXED.toUpperCase()}`, 'f', { n: 1 }), lower);
  assert.notEqual(lower, 'bc508ee59e0a79b7');
  // A datetime is in the form of the reference evaluator: UTC, `.mmm` only when it is not zero, so
  // every way to write the same instant gives one fingerprint, and `.25` is `.250`.
  const instant = createHash('sha256').update('{"instant":"2026-01-01T10:00:00.250Z"}').digest('hex').slice(0, 16);
  const stamp = async (value: string, key: string): Promise<string | undefined> => {
    const res = await handler({ method: 'POST', path: '/f/stamp', query: `instant=${value}`, headers: { 'idempotency-key': key } });
    assert.equal(res?.status, 201, `${value}: ${String(res?.body)}`);
    return seen[seen.length - 1]?.fingerprint;
  };
  assert.equal(await stamp('2026-01-01T10:00:00.25Z', 'h'), instant);
  assert.equal(await stamp('2026-01-01T10:00:00.250Z', 'i'), instant);
  assert.equal(await stamp('2026-01-01T11:00:00.250%2B01:00', 'j'), instant);
  const whole = createHash('sha256').update('{"instant":"2026-01-01T10:00:00Z"}').digest('hex').slice(0, 16);
  assert.equal(await stamp('2026-01-01T10:00:00.000Z', 'k'), whole);
  // Another value is another fingerprint.
  assert.notEqual(await call('PUT', `/f/${ID}`, 'g', { n: 2 }), 'bc508ee59e0a79b7');
  console.log('idempotency fingerprint ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
