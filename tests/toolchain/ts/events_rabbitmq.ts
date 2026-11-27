// The RabbitMQ transport against a real broker (spec/events.md sections 10.1 and 10.7). It runs
// when `VELD_RABBITMQ` holds an AMQP URL (`amqp://guest:guest@localhost:5672`) and prints
// `events rabbitmq ok`; the Rust test starts it. Every topic and group is new for each run.
//
// The rows of the matrix: publish and consume, the key, the order per key, a handler that fails
// is delivered again, `@ordered`, the dead-letter exchange and queue, a message published before
// the group subscribed, two consumers of one group, `close()`, and the arguments of a queue.
import { strict as assert } from 'assert';
import { randomUUID } from 'crypto';

import * as amqp from 'amqplib';

import { RabbitMqTransport } from './server/server/rabbitmq';
import type { AmqpConnection } from './server/server/rabbitmq';
import { OrderPlaced } from './server/events/t';
import { createOrdersPublisher } from './server/server/orders-events';
import type { Outcome, Transport, TransportMessage } from './server/runtime/events';
import { subscribeEvents } from './server/runtime/events';

const url = process.env['VELD_RABBITMQ'];
if (url === undefined || url === '') throw new Error('VELD_RABBITMQ holds no AMQP URL');

const run = randomUUID().slice(0, 8);
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const FAST = { redelivery: { initialMs: 50, maxMs: 200 } };

let counter = 0;
const fresh = (what: string): string => `veld-${run}-${what}-${(counter += 1)}`;
const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/** Waits until `done()` holds, for at most `ms`. */
async function until(what: string, done: () => boolean, ms = 20000): Promise<void> {
  const end = Date.now() + ms;
  while (!done()) {
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await sleep(25);
  }
}

/** The next message of a queue, read with `basic.get`, for at most 20 seconds. */
async function nextOf(check: amqp.Channel, queue: string): Promise<amqp.GetMessage> {
  const end = Date.now() + 20000;
  for (;;) {
    const message = await check.get(queue, { noAck: false });
    if (message !== false) return message;
    if (Date.now() > end) throw new Error(`timed out waiting for a message in ${queue}`);
    await sleep(50);
  }
}

interface Got {
  key: string | null;
  text: string;
  attempt: number;
}

/** A consumer that records what it gets and answers `answer(got)`. */
function recorder(answer: (got: Got) => Outcome = () => 'done'): { got: Got[]; deliver: (m: TransportMessage) => Promise<Outcome> } {
  const got: Got[] = [];
  return {
    got,
    deliver: (m) => {
      const item = { key: m.key, text: decoder.decode(m.bytes), attempt: m.attempt };
      got.push(item);
      return Promise.resolve(answer(item));
    },
  };
}

async function main(): Promise<void> {
  const connection = await amqp.connect(url as string);
  // The adapter is built from the connection the application made, as amqplib returns it.
  const open: AmqpConnection = connection;
  const check = await connection.createChannel();
  try {
    await publishAndConsume(open, check);
    await orderPerKey(open);
    await aFailedHandlerIsDeliveredAgain(open);
    await ordered(open);
    await theDeadLetterQueue(open, check);
    await aMessageBeforeTheGroupSubscribed(open);
    await twoConsumersOfOneGroup(open);
    await close(open);
    await aQueueWithOtherArguments(open);
  } finally {
    await check.close().catch(() => undefined);
    await connection.close();
  }
  console.log('events rabbitmq ok');
}

async function publishAndConsume(open: AmqpConnection, check: amqp.Channel): Promise<void> {
  const t = fresh('basic');
  const group = fresh('g');
  const transport = new RabbitMqTransport(open, FAST);
  const r = recorder();
  const sub = await transport.subscribe(t, group, r.deliver);
  // A queue of the test, bound to the same exchange, to see what the publisher wrote.
  const spy = fresh('spy');
  await check.assertQueue(spy, { exclusive: true });
  await check.bindQueue(spy, t, '');
  await transport.publish(t, 'k1', encoder.encode('hello'));
  await transport.publish(t, null, encoder.encode('no key'));
  await until('two messages', () => r.got.length >= 2);
  assert.deepEqual(r.got.map((g) => g.text), ['hello', 'no key']);
  assert.equal(r.got[0]?.key, 'k1', 'the key is the routing key');
  assert.equal(r.got[1]?.key, null, 'the empty routing key is no key');
  assert.ok(r.got.every((g) => g.attempt === 1));
  // `publish` completed when the broker confirmed, so the message is already in the queue.
  const stored = await nextOf(check, spy);
  assert.equal(stored.fields.routingKey, 'k1');
  assert.equal(stored.properties.contentType, 'application/cloudevents+json; charset=UTF-8');
  assert.equal(stored.properties.deliveryMode, 2, 'the message is persistent');
  check.ack(stored);
  // The exchange is a durable fanout exchange and the queue a durable queue: they exist.
  await check.checkExchange(t);
  const queue = await check.checkQueue(`${t}.${group}`);
  assert.equal(queue.queue, `${t}.${group}`);
  await sub.close();
  await transport.close();
}

async function orderPerKey(open: AmqpConnection): Promise<void> {
  const t = fresh('order');
  const transport = new RabbitMqTransport(open, FAST);
  const r = recorder();
  await transport.subscribe(t, fresh('g'), r.deliver);
  for (const key of ['a', 'b']) {
    for (let i = 0; i < 10; i += 1) await transport.publish(t, key, encoder.encode(`${key}${i}`));
  }
  await until('twenty messages', () => r.got.length >= 20);
  for (const key of ['a', 'b']) {
    const mine = r.got.filter((g) => g.key === key).map((g) => g.text);
    assert.deepEqual(mine, Array.from({ length: 10 }, (_, i) => `${key}${i}`), `order of key ${key}`);
  }
  await transport.close();
}

async function aFailedHandlerIsDeliveredAgain(open: AmqpConnection): Promise<void> {
  const t = fresh('again');
  const transport = new RabbitMqTransport(open, FAST);
  const r = recorder((g) => (g.text === 'first' && g.attempt < 3 ? 'again' : 'done'));
  await transport.subscribe(t, fresh('g'), r.deliver);
  await transport.publish(t, 'same', encoder.encode('first'));
  await transport.publish(t, 'same', encoder.encode('second'));
  await until('three deliveries of the first and one of the second', () => r.got.length >= 4);
  assert.deepEqual(
    r.got.map((g) => `${g.text}#${g.attempt}`),
    ['first#1', 'first#2', 'first#3', 'second#1'],
    'the same consumer delivers it again, and takes nothing else meanwhile',
  );
  await transport.close();
}

async function ordered(open: AmqpConnection): Promise<void> {
  const t = fresh('ordered');
  const group = fresh('g');
  const a = new RabbitMqTransport(open, FAST);
  const b = new RabbitMqTransport(open, FAST);
  const ra = recorder();
  const rb = recorder();
  await a.subscribe(t, group, ra.deliver, { ordered: true });
  await b.subscribe(t, group, rb.deliver, { ordered: true });
  for (let i = 0; i < 10; i += 1) await a.publish(t, `key-${i}`, encoder.encode(`m${i}`), { ordered: true });
  await until('ten messages', () => ra.got.length + rb.got.length >= 10);
  await sleep(300);
  const [first, second] = ra.got.length > 0 ? [ra, rb] : [rb, ra];
  assert.deepEqual(first.got.map((g) => g.text), Array.from({ length: 10 }, (_, i) => `m${i}`), 'one active consumer, one total order');
  assert.equal(second.got.length, 0, 'the other consumer of the queue waits');
  await Promise.all([a.close(), b.close()]);
}

async function theDeadLetterQueue(open: AmqpConnection, check: amqp.Channel): Promise<void> {
  const t = fresh('dead');
  const transport = new RabbitMqTransport(open, FAST);
  let valid: Uint8Array = new Uint8Array();
  const keep: Transport = {
    publish: (_t, _k, bytes) => {
      valid = bytes;
      return Promise.resolve();
    },
    subscribe: () => Promise.resolve({ close: () => Promise.resolve() }),
  };
  await createOrdersPublisher(keep, { newId: () => 'o-1', clock: () => new Date('2026-06-10T12:00:00Z') }).orderEvents.orderPlaced({
    orderId: '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21',
    quantity: 2,
  });
  let runs = 0;
  const handle = await subscribeEvents(
    transport,
    [
      {
        topic: t,
        group: fresh('g'),
        on: [
          {
            def: { name: 'OrderPlaced', sig: OrderPlaced.sig, schema: OrderPlaced.schema, digest: null },
            run: () => {
              runs += 1;
              throw new Error('down');
            },
          },
        ],
        retry: { max: 1, backoff: 'fixed' },
        deadLetter: true,
      },
    ],
    { retryDelayMs: 10 },
  );
  // Subscribing with `@deadLetter` declared the exchange and the queue of the copies.
  await check.checkExchange(`${t}.dlq`);
  await check.checkQueue(`${t}.dlq`);
  await transport.publish(t, '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21', valid);
  await until('both runs', () => runs >= 2);
  const copy = await nextOf(check, `${t}.dlq`);
  const body = JSON.parse(decoder.decode(copy.content)) as Record<string, unknown>;
  assert.equal(body['veldreason'], 'handler');
  assert.equal(body['veldattempts'], 2);
  assert.equal(body['id'], 'o-1');
  assert.equal(copy.fields.routingKey, '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21', 'the copy has the key of the original');
  check.ack(copy);
  await handle.close();
  await transport.close();
}

async function aMessageBeforeTheGroupSubscribed(open: AmqpConnection): Promise<void> {
  const t = fresh('early');
  const transport = new RabbitMqTransport(open, FAST);
  await transport.publish(t, 'k', encoder.encode('early'));
  const r = recorder();
  await transport.subscribe(t, fresh('g'), r.deliver);
  await sleep(500);
  assert.equal(r.got.length, 0, 'no queue was bound when it was published: it is dropped');
  await transport.publish(t, 'k', encoder.encode('late'));
  await until('the late message', () => r.got.length >= 1);
  assert.deepEqual(r.got.map((g) => g.text), ['late']);
  await transport.close();
}

async function twoConsumersOfOneGroup(open: AmqpConnection): Promise<void> {
  const t = fresh('share');
  const group = fresh('g');
  const a = new RabbitMqTransport(open, FAST);
  const b = new RabbitMqTransport(open, FAST);
  const ra = recorder();
  const rb = recorder();
  const slowly = (r: ReturnType<typeof recorder>) => async (m: TransportMessage): Promise<Outcome> => {
    await sleep(20);
    return r.deliver(m);
  };
  await a.subscribe(t, group, slowly(ra));
  await b.subscribe(t, group, slowly(rb));
  for (let i = 0; i < 20; i += 1) await a.publish(t, `key-${i}`, encoder.encode(`m${i}`));
  await until('all messages', () => ra.got.length + rb.got.length >= 20);
  const all = [...ra.got, ...rb.got].map((g) => g.text);
  assert.equal(new Set(all).size, 20, 'every message reached one consumer of the group');
  assert.equal(all.length, 20, 'and only one');
  assert.ok(ra.got.length > 0 && rb.got.length > 0, 'the two consumers share the load');
  await Promise.all([a.close(), b.close()]);
}

async function close(open: AmqpConnection): Promise<void> {
  const t = fresh('close');
  const group = fresh('g');
  const transport = new RabbitMqTransport(open, FAST);
  let started = false;
  let finished = false;
  const sub = await transport.subscribe(t, group, async () => {
    started = true;
    await sleep(500);
    finished = true;
    return 'done';
  });
  await transport.publish(t, 'k', encoder.encode('slow'));
  await until('the delivery to start', () => started);
  await sub.close();
  assert.equal(finished, true, 'close waits for the delivery that runs');
  // A message that is delivered again when the consumer closes is kept for the next consumer.
  const stubborn = await transport.subscribe(t, group, () => Promise.resolve('again'));
  await transport.publish(t, 'k', encoder.encode('kept'));
  await sleep(300);
  await stubborn.close();
  const next = recorder();
  await transport.subscribe(t, group, next.deliver);
  await until('the kept message', () => next.got.length >= 1);
  assert.deepEqual(next.got.map((g) => g.text), ['kept'], 'unacknowledged, so the broker gives it to the next consumer');
  await transport.close();
  await assert.rejects(transport.publish(t, null, encoder.encode('x')));
}

async function aQueueWithOtherArguments(open: AmqpConnection): Promise<void> {
  const t = fresh('args');
  const group = fresh('g');
  const transport = new RabbitMqTransport(open, FAST);
  await transport.subscribe(t, group, recorder().deliver);
  // The same queue, asked for as `@ordered`: the broker refuses it, and the adapter passes that on.
  await assert.rejects(transport.subscribe(t, group, recorder().deliver, { ordered: true }), /PRECONDITION_FAILED|inequivalent/);
  await transport.close();
}

main().then(
  () => process.exit(0),
  (e: unknown) => {
    console.error(e);
    process.exit(1);
  },
);
