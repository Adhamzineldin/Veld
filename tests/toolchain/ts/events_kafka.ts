// The Kafka transport against a real broker (spec/events.md sections 10.1 and 10.7). It runs when
// `VELD_KAFKA` names the bootstrap servers (`localhost:9092`) and prints `events kafka ok`; the
// Rust test starts it. Every topic and group is new for each run, and the topics are created here
// with the admin client, because the adapter creates nothing.
//
// The rows of the matrix: publish and consume, the key, the order per key, a handler that fails
// is delivered again, `@ordered`, the dead-letter topic, a message published before the group
// subscribed, two consumers of one group, `close()`, and a delivery that outlives the session.
import { strict as assert } from 'assert';
import { randomUUID } from 'crypto';

import { Kafka, logLevel } from 'kafkajs';

import { KafkaTransport } from './server/server/kafka';
import { OrderPlaced } from './server/events/t';
import { createOrdersPublisher } from './server/server/orders-events';
import type { Outcome, Transport, TransportMessage } from './server/runtime/events';
import { subscribeEvents } from './server/runtime/events';

const brokers = (process.env['VELD_KAFKA'] ?? '').split(',').filter((b) => b !== '');
if (brokers.length === 0) throw new Error('VELD_KAFKA names no broker');

const run = randomUUID().slice(0, 8);
const kafka = new Kafka({ clientId: `veld-${run}`, brokers, logLevel: logLevel.NOTHING });
const admin = kafka.admin();
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const FAST = { redelivery: { initialMs: 50, maxMs: 200 } };

let counter = 0;
const fresh = (what: string): string => `veld-${run}-${what}-${(counter += 1)}`;

async function topic(what: string, partitions = 3): Promise<string> {
  const name = fresh(what);
  await admin.createTopics({ topics: [{ topic: name, numPartitions: partitions }], waitForLeaders: true });
  return name;
}

const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/** Waits until `done()` holds, for at most `ms`. */
async function until(what: string, done: () => boolean, ms = 30000): Promise<void> {
  const end = Date.now() + ms;
  while (!done()) {
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
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

/** Reads a topic from the start with a raw kafkajs consumer, for what the adapter wrote. */
async function readRaw(name: string, count: number): Promise<{ partition: number; key: string | null; text: string; contentType: string }[]> {
  const consumer = kafka.consumer({ groupId: fresh('raw') });
  const seen: { partition: number; key: string | null; text: string; contentType: string }[] = [];
  await consumer.connect();
  await consumer.subscribe({ topics: [name], fromBeginning: true });
  await consumer.run({
    eachMessage: ({ partition, message }) => {
      seen.push({
        partition,
        key: message.key === null ? null : message.key.toString('utf8'),
        text: message.value === null ? '' : message.value.toString('utf8'),
        contentType: String(message.headers?.['content-type'] ?? ''),
      });
      return Promise.resolve();
    },
  });
  await until(`${count} raw records of ${name}`, () => seen.length >= count);
  await consumer.disconnect();
  return seen;
}

async function publishAndConsume(): Promise<void> {
  const t = await topic('basic');
  const transport = new KafkaTransport(kafka, FAST);
  const r = recorder();
  const sub = await transport.subscribe(t, fresh('g'), r.deliver);
  await transport.publish(t, 'k1', encoder.encode('hello'));
  await transport.publish(t, null, encoder.encode('no key'));
  await until('two messages', () => r.got.length >= 2);
  assert.deepEqual(r.got.map((g) => g.text).sort(), ['hello', 'no key']);
  assert.equal(r.got.find((g) => g.text === 'hello')?.key, 'k1', 'the key is the record key');
  assert.equal(r.got.find((g) => g.text === 'no key')?.key, null);
  assert.ok(r.got.every((g) => g.attempt === 1));
  await sub.close();
  // What is on the topic: the envelope bytes as the value and the media type as a header.
  const raw = await readRaw(t, 2);
  assert.ok(raw.every((m) => m.contentType === 'application/cloudevents+json; charset=UTF-8'));
  assert.equal(raw.find((m) => m.text === 'hello')?.key, 'k1');
  await transport.close();
}

async function orderPerKey(): Promise<void> {
  const t = await topic('order');
  const transport = new KafkaTransport(kafka, FAST);
  const r = recorder();
  await transport.subscribe(t, fresh('g'), r.deliver);
  for (const key of ['a', 'b', 'c']) {
    for (let i = 0; i < 10; i += 1) await transport.publish(t, key, encoder.encode(`${key}${i}`));
  }
  await until('thirty messages', () => r.got.length >= 30);
  for (const key of ['a', 'b', 'c']) {
    const mine = r.got.filter((g) => g.key === key).map((g) => g.text);
    assert.deepEqual(mine, Array.from({ length: 10 }, (_, i) => `${key}${i}`), `order of key ${key}`);
  }
  await transport.close();
}

async function aFailedHandlerIsDeliveredAgain(): Promise<void> {
  const t = await topic('again');
  const transport = new KafkaTransport(kafka, FAST);
  // The first message fails twice; the second, of the same partition, waits for it.
  const r = recorder((g) => (g.text === 'first' && g.attempt < 3 ? 'again' : 'done'));
  await transport.subscribe(t, fresh('g'), r.deliver);
  await transport.publish(t, 'same', encoder.encode('first'));
  await transport.publish(t, 'same', encoder.encode('second'));
  await until('three deliveries of the first and one of the second', () => r.got.length >= 4);
  assert.deepEqual(
    r.got.map((g) => `${g.text}#${g.attempt}`),
    ['first#1', 'first#2', 'first#3', 'second#1'],
    'the same consumer delivers it again, and takes nothing else of the partition meanwhile',
  );
  await transport.close();
}

async function ordered(): Promise<void> {
  const t = await topic('ordered', 3);
  const transport = new KafkaTransport(kafka, FAST);
  const r = recorder();
  await transport.subscribe(t, fresh('g'), r.deliver, { ordered: true });
  for (let i = 0; i < 12; i += 1) await transport.publish(t, `key-${i}`, encoder.encode(`m${i}`), { ordered: true });
  await until('twelve messages', () => r.got.length >= 12);
  assert.deepEqual(r.got.map((g) => g.text), Array.from({ length: 12 }, (_, i) => `m${i}`), 'one total order across keys');
  const raw = await readRaw(t, 12);
  assert.ok(raw.every((m) => m.partition === 0), 'every message of an ordered channel is in partition 0');
  await transport.close();
}

async function theDeadLetterTopic(): Promise<void> {
  const t = await topic('dead');
  // The adapter creates nothing: the topic of the copies is made here, under its real name.
  const dead = `${t}.dlq`;
  await admin.createTopics({ topics: [{ topic: dead, numPartitions: 1 }], waitForLeaders: true });
  const transport = new KafkaTransport(kafka, FAST);
  // A valid message, taken from the generated publisher over a transport that keeps it.
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
  const copies = recorder();
  await transport.subscribe(dead, fresh('g'), copies.deliver);
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
  await transport.publish(t, '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21', valid);
  await until('the copy', () => copies.got.length >= 1);
  assert.equal(runs, 2);
  const copy = JSON.parse(copies.got[0]?.text ?? '{}') as Record<string, unknown>;
  assert.equal(copy['veldreason'], 'handler');
  assert.equal(copy['veldattempts'], 2);
  assert.equal(copy['id'], 'o-1');
  assert.equal(copies.got[0]?.key, '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21', 'the copy has the key of the original');
  await handle.close();
  await transport.close();
}

async function aMessageBeforeTheGroupSubscribed(): Promise<void> {
  const t = await topic('early');
  const transport = new KafkaTransport(kafka, FAST);
  await transport.publish(t, 'k', encoder.encode('early'));
  const r = recorder();
  await transport.subscribe(t, fresh('g'), r.deliver);
  await until('the early message', () => r.got.length >= 1);
  assert.equal(r.got[0]?.text, 'early', 'a new group starts at the earliest offset');
  await transport.close();
}

async function twoConsumersOfOneGroup(): Promise<void> {
  const t = await topic('share', 2);
  const group = fresh('g');
  const a = new KafkaTransport(kafka, FAST);
  const b = new KafkaTransport(kafka, FAST);
  const ra = recorder();
  const rb = recorder();
  await a.subscribe(t, group, ra.deliver);
  await b.subscribe(t, group, rb.deliver);
  const sender = new KafkaTransport(kafka, FAST);
  for (let i = 0; i < 24; i += 1) await sender.publish(t, `key-${i}`, encoder.encode(`m${i}`));
  await until('all messages', () => ra.got.length + rb.got.length >= 24);
  await sleep(500);
  const all = [...ra.got, ...rb.got].map((g) => g.text);
  assert.equal(new Set(all).size, 24, 'every message reached one consumer of the group');
  assert.equal(all.length, 24, 'and only one');
  assert.ok(ra.got.length > 0 && rb.got.length > 0, 'the two consumers share the load');
  await Promise.all([a.close(), b.close(), sender.close()]);
}

async function close(): Promise<void> {
  const t = await topic('close', 1);
  const group = fresh('g');
  const transport = new KafkaTransport(kafka, FAST);
  let started = false;
  let finished = false;
  const sub = await transport.subscribe(t, group, async () => {
    started = true;
    await sleep(600);
    finished = true;
    return 'done';
  });
  const sender = new KafkaTransport(kafka, FAST);
  await sender.publish(t, 'k', encoder.encode('slow'));
  await until('the delivery to start', () => started);
  await sub.close();
  assert.equal(finished, true, 'close waits for the delivery that runs');
  // It was committed: the group, subscribed again, is not given the message.
  const again = recorder();
  await transport.subscribe(t, group, again.deliver);
  await sender.publish(t, 'k', encoder.encode('after'));
  await until('the next message', () => again.got.length >= 1);
  assert.deepEqual(again.got.map((g) => g.text), ['after']);
  await transport.close();
  await sender.close();
  // A closed transport takes no more.
  await assert.rejects(transport.publish(t, null, encoder.encode('x')));
}

async function aDeliveryThatOutlivesTheSession(): Promise<void> {
  const t = await topic('long', 1);
  const group = fresh('g');
  // The broker's smallest session timeout is 6 seconds: the handler runs for longer than that.
  const transport = new KafkaTransport(kafka, { ...FAST, consumer: { sessionTimeout: 6000, heartbeatInterval: 1000 } });
  const r = recorder((g) => (g.text === 'long' && g.attempt === 1 ? 'again' : 'done'));
  await transport.subscribe(t, group, async (m) => {
    if (m.attempt === 1 && decoder.decode(m.bytes) === 'long') await sleep(9000);
    return r.deliver(m);
  });
  const sender = new KafkaTransport(kafka, FAST);
  await sender.publish(t, 'k', encoder.encode('long'));
  await until('the message to be delivered a second time', () => r.got.length >= 2, 60000);
  assert.deepEqual(r.got.map((g) => g.attempt), [1, 2], 'one consumer, delivered again by itself, not taken away');
  await sender.publish(t, 'k', encoder.encode('next'));
  await until('the next message', () => r.got.length >= 3);
  assert.equal(r.got[2]?.attempt, 1);
  await transport.close();
  await sender.close();
}

async function main(): Promise<void> {
  await admin.connect();
  try {
    await publishAndConsume();
    await orderPerKey();
    await aFailedHandlerIsDeliveredAgain();
    await ordered();
    await theDeadLetterTopic();
    await aMessageBeforeTheGroupSubscribed();
    await twoConsumersOfOneGroup();
    await close();
    await aDeliveryThatOutlivesTheSession();
  } finally {
    await admin.disconnect();
  }
  console.log('events kafka ok');
}

main().then(
  () => process.exit(0),
  (e: unknown) => {
    console.error(e);
    process.exit(1);
  },
);
