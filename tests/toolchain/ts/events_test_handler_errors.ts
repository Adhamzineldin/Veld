// The event endpoint of a generated test handler (spec/events.md section 10.9): a request without
// `Veld-Test-Topic`, with an empty one, or with a `Veld-Test-Deliveries` that is neither `1` nor
// `2` is answered `400` with `{ "error": "<text>" }`; an absent `Veld-Test-Deliveries` means 1.
// Expects `./server` generated with `testing = true` from tests/events/delivery.veld. Prints
// `events test handler errors ok`.
import { strict as assert } from 'assert';

import { createFulfilmentTestHandler } from './server/server/fulfilment-test-handler';
import type { VeldRequest } from './server/runtime/veld';

const handler = createFulfilmentTestHandler();

async function send(headers: Record<string, string>): Promise<{ status: number; body: Record<string, unknown> }> {
  const req: VeldRequest = {
    method: 'POST',
    path: '/__veld/test/event',
    headers,
    text: () => Promise.resolve('not json'),
  };
  const res = await handler(req);
  if (res === null) throw new Error('the test handler did not answer');
  return { status: res.status, body: JSON.parse(res.body ?? '{}') as Record<string, unknown> };
}

async function main(): Promise<void> {
  for (const [what, headers] of [
    ['no topic', {}],
    ['an empty topic', { 'veld-test-topic': '' }],
    ['deliveries 0', { 'veld-test-topic': 'shop.orders.v1', 'veld-test-deliveries': '0' }],
    ['deliveries 3', { 'veld-test-topic': 'shop.orders.v1', 'veld-test-deliveries': '3' }],
    ['deliveries abc', { 'veld-test-topic': 'shop.orders.v1', 'veld-test-deliveries': 'abc' }],
    ['deliveries empty', { 'veld-test-topic': 'shop.orders.v1', 'veld-test-deliveries': '' }],
  ] as [string, Record<string, string>][]) {
    const got = await send(headers);
    assert.equal(got.status, 400, what);
    assert.deepEqual(Object.keys(got.body), ['error'], what);
    assert.equal(typeof got.body['error'], 'string', what);
    assert.notEqual(got.body['error'], '', what);
  }

  // Absent, `1` and `2` are accepted: the message is not JSON, so it is rejected as malformed.
  const accepted: Record<string, string>[] = [
    { 'veld-test-topic': 'shop.orders.v1' },
    { 'veld-test-topic': 'shop.orders.v1', 'veld-test-deliveries': '1' },
    { 'veld-test-topic': 'shop.orders.v1', 'veld-test-deliveries': '2' },
  ];
  for (const headers of accepted) {
    const got = await send(headers);
    assert.equal(got.status, 200, JSON.stringify(got.body));
    assert.equal(got.body['outcome'], 'rejected');
    assert.equal(got.body['reason'], 'malformed');
  }
  console.log('events test handler errors ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
