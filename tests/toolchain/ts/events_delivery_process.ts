// Phase E2 in one process over the memory transport (spec/events.md section 10, the contract
// tests/events/delivery.veld): `Orders` publishes `OrderEvents`, which has `@retry(max: 3,
// backoff: exponential)`, `@deadLetter` and `@ordered`; `Fulfilment` and `Billing` subscribe.
// Retries with the waits a fake `sleep` records, the copy on the dead-letter topic and a
// subscriber that reads it, a copy that cannot be published, the inbox, the options the
// generated publisher and consumer hand the transport. Prints `events delivery process ok`.
import { strict as assert } from 'assert';

import { createOrdersPublisher } from './server/server/orders-events';
import { subscribeBillingEvents } from './server/server/billing-events';
import { subscribeFulfilmentEvents } from './server/server/fulfilment-events';
import type { ChannelOptions, HandlerFailure, Outcome, Transport, TransportMessage } from './server/runtime/events';
import { MemoryInbox, MemoryTransport, subscribeEvents } from './server/runtime/events';
import type { EventRejected } from './server/runtime/events';
import type { Schema } from './server/runtime/veld';

const TOPIC = 'shop.orders.v1';
const ORDER = '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21';
const decode = (bytes: Uint8Array): Record<string, unknown> => JSON.parse(new TextDecoder().decode(bytes)) as Record<string, unknown>;

function publisher(transport: Transport, id = 'same') {
  return createOrdersPublisher(transport, { newId: () => id, clock: () => new Date('2026-06-10T12:00:00Z') });
}

/** The memory transport, noting what the generated code asks of it, and failing the copies when told to. */
class Spy extends MemoryTransport {
  publishOptions: (ChannelOptions | undefined)[] = [];
  subscribeOptions: (ChannelOptions | undefined)[] = [];
  failCopies = false;

  publish(topic: string, key: string | null, bytes: Uint8Array, options?: ChannelOptions): Promise<void> {
    if (topic.endsWith('.dlq') && this.failCopies) return Promise.reject(new Error('the copy is refused'));
    this.publishOptions.push(options);
    return super.publish(topic, key, bytes);
  }

  subscribe(topic: string, group: string, deliver: (m: TransportMessage) => Promise<Outcome>, options?: ChannelOptions) {
    this.subscribeOptions.push(options);
    return super.subscribe(topic, group, deliver);
  }
}

async function retriesWaitAndTheLastRunIsCopied(): Promise<void> {
  const transport = new Spy();
  const waits: number[] = [];
  const failures: HandlerFailure[] = [];
  const attempts: number[] = [];
  let runs = 0;
  await subscribeFulfilmentEvents(
    transport,
    {
      reserve: (_e, meta) => {
        runs += 1;
        attempts.push(meta.attempt);
        throw new Error('no stock');
      },
      notifyCustomer: () => undefined,
    },
    {
      sleep: (ms) => {
        waits.push(ms);
        return Promise.resolve();
      },
      onHandlerError: (e) => failures.push(e),
    },
  );
  // A reader of the dead-letter topic: it is a topic like any other.
  const copies: { key: string | null; bytes: Uint8Array }[] = [];
  await transport.subscribe(`${TOPIC}.dlq`, 'audit', (m) => {
    copies.push({ key: m.key, bytes: m.bytes });
    return Promise.resolve('done');
  });
  const sent = await publisher(transport, 'o-1').orderEvents.orderPlaced({ orderId: ORDER, quantity: 2 });
  await transport.idle();
  // n + 1 runs, the waits of the exponential backoff, attempts counted by the consumer.
  assert.equal(runs, 4);
  assert.deepEqual(waits, [1000, 2000, 4000]);
  assert.deepEqual(attempts, [1, 2, 3, 4]);
  assert.deepEqual(
    failures.map((f) => [f.attempt, f.final]),
    [[1, false], [2, false], [3, false], [4, true]],
  );
  // The message was acknowledged: the memory transport did not give up on it.
  assert.equal(transport.deadLetters().length, 0);
  assert.equal(copies.length, 1);
  assert.equal(copies[0]?.key, ORDER);
  const copy = decode(copies[0]?.bytes ?? new Uint8Array());
  assert.equal(copy['veldreason'], 'handler');
  assert.equal(copy['veldgroup'], 'fulfilment');
  assert.equal(copy['veldattempts'], 4);
  assert.equal(copy['id'], sent.id);
  assert.equal(copy['veldissues'], undefined);
  assert.equal(JSON.stringify(copy).includes('no stock'), false, 'nothing of the error is copied');
  // The options of the channel reach the transport on both sides.
  assert.deepEqual(transport.subscribeOptions[0], { ordered: true, deadLetter: true });
  assert.deepEqual(transport.publishOptions[0], { ordered: true, deadLetter: true });
}

async function aCopyThatIsPublishedAgainIsAValidMessage(): Promise<void> {
  const transport = new Spy();
  let failing = true;
  const handled: number[] = [];
  await subscribeFulfilmentEvents(
    transport,
    {
      reserve: (e) => {
        if (failing) throw new Error('down');
        handled.push(e.quantity);
      },
      notifyCustomer: () => undefined,
    },
    { sleep: () => Promise.resolve() },
  );
  const copies: Uint8Array[] = [];
  await transport.subscribe(`${TOPIC}.dlq`, 'audit', (m) => {
    copies.push(m.bytes);
    return Promise.resolve('done');
  });
  await publisher(transport, 'o-2').orderEvents.orderPlaced({ orderId: ORDER, quantity: 5 });
  await transport.idle();
  assert.equal(copies.length, 1);
  failing = false;
  await transport.publish(TOPIC, ORDER, copies[0] ?? new Uint8Array());
  await transport.idle();
  assert.deepEqual(handled, [5], 'a consumer ignores the members the copy added');
}

async function aCopyThatCannotBePublishedLeavesTheMessageUnacknowledged(): Promise<void> {
  const transport = new Spy();
  transport.failCopies = true;
  let runs = 0;
  await subscribeFulfilmentEvents(
    transport,
    {
      reserve: () => {
        runs += 1;
        throw new Error('down');
      },
      notifyCustomer: () => undefined,
    },
    { sleep: () => Promise.resolve() },
  );
  await publisher(transport, 'o-3').orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await transport.idle();
  // Every delivery runs the handler 4 times and fails to copy, so it answers `again`; the memory
  // transport gives up after its three attempts and keeps the message.
  assert.equal(runs, 12);
  const dead = transport.deadLetters();
  assert.equal(dead.length, 1);
  assert.equal(dead[0]?.attempts, 3);
  assert.equal(dead[0]?.group, 'fulfilment');
}

async function aRejectedMessageIsCopiedBeforeItIsAcknowledged(): Promise<void> {
  const transport = new Spy();
  const rejected: string[] = [];
  const charged: string[] = [];
  await subscribeBillingEvents(
    transport,
    { charge: (_e, m) => void charged.push(m.id) },
    { onEventRejected: (e) => rejected.push(e.reason) },
  );
  const copies: string[] = [];
  await transport.subscribe(`${TOPIC}.dlq`, 'audit', (m) => {
    copies.push(new TextDecoder().decode(m.bytes));
    return Promise.resolve('done');
  });
  // A valid message first, to learn the rules digest the consumer expects.
  const seen: Record<string, unknown>[] = [];
  await transport.subscribe(TOPIC, 'probe', (m) => {
    seen.push(decode(m.bytes));
    return Promise.resolve('done');
  });
  await publisher(transport, 'o-4').orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await transport.idle();
  const good = seen[0] as Record<string, unknown>;
  const bad = { ...good, id: 'o-5', data: { orderId: ORDER, quantity: 0 } };
  await transport.publish(TOPIC, ORDER, new TextEncoder().encode(JSON.stringify(bad)));
  await transport.publish(TOPIC, null, new TextEncoder().encode('this is not json'));
  await transport.idle();
  assert.deepEqual(rejected, ['rule', 'malformed']);
  assert.deepEqual(charged, ['o-4'], 'a rejected message reaches no handler');
  assert.equal(transport.deadLetters().length, 0, 'both were acknowledged');
  assert.equal(copies.length, 2);
  const rule = JSON.parse(copies[0] ?? '{}') as Record<string, unknown>;
  assert.deepEqual([rule['veldreason'], rule['veldattempts'], rule['veldgroup']], ['rule', 0, 'billing']);
  assert.equal(typeof rule['veldviolations'], 'string');
  assert.equal(rule['id'], 'o-5');
  assert.equal(copies[1], 'this is not json', 'what is not an object is copied as it is');
}

async function theInboxSkipsASecondDelivery(): Promise<void> {
  const transport = new Spy();
  const inbox = new MemoryInbox();
  const metas: string[] = [];
  const hooks: string[] = [];
  await subscribeBillingEvents(
    transport,
    { charge: (_e, m) => void metas.push(m.id) },
    { inbox, onEventRejected: () => hooks.push('rejected'), onHandlerError: () => hooks.push('error') },
  );
  const pub = publisher(transport, 'o-6');
  await pub.orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await pub.orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await transport.idle();
  assert.deepEqual(metas, ['o-6'], 'called once');
  assert.deepEqual(hooks, [], 'a skipped message reaches no hook');
  assert.equal(inbox.size, 1);
  assert.equal(inbox.has('veld:bookings/Orders', 'o-6'), true);
  assert.equal(transport.deadLetters().length, 0, 'a skipped message is acknowledged');

  // Without an inbox the same two messages run the handler twice.
  const bare = new Spy();
  const ran: string[] = [];
  await subscribeBillingEvents(bare, { charge: (_e, m) => void ran.push(m.id) });
  const again = publisher(bare, 'o-7');
  await again.orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await again.orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await bare.idle();
  assert.deepEqual(ran, ['o-7', 'o-7']);
}

async function everyRunGoesThroughTheInboxAgain(): Promise<void> {
  const transport = new Spy();
  const calls: string[] = [];
  const inbox = {
    handle: async (source: string, id: string, run: () => Promise<void> | void): Promise<boolean> => {
      calls.push(`${source} ${id}`);
      await run();
      return true;
    },
  };
  let failures = 2;
  await subscribeFulfilmentEvents(
    transport,
    {
      reserve: () => {
        if (failures > 0) {
          failures -= 1;
          throw new Error('twice');
        }
      },
      notifyCustomer: () => undefined,
    },
    { inbox, sleep: () => Promise.resolve() },
  );
  await publisher(transport, 'o-8').orderEvents.orderPlaced({ orderId: ORDER, quantity: 1 });
  await transport.idle();
  assert.equal(calls.length, 3, 'the inbox is asked for every run');
  assert.equal(new Set(calls).size, 1);
}

async function aFailedRunIsNotRecorded(): Promise<void> {
  const transport = new Spy();
  const inbox = new MemoryInbox();
  let fail = true;
  const ran: number[] = [];
  await subscribeBillingEvents(
    transport,
    {
      charge: (e) => {
        if (fail) throw new Error('declined');
        ran.push(e.quantity);
      },
    },
    { inbox, sleep: () => Promise.resolve() },
  );
  await publisher(transport, 'o-9').orderEvents.orderPlaced({ orderId: ORDER, quantity: 3 });
  await transport.idle();
  assert.equal(inbox.size, 0, 'the pair of a failed run is not recorded');
  fail = false;
  await publisher(transport, 'o-9').orderEvents.orderPlaced({ orderId: ORDER, quantity: 3 });
  await transport.idle();
  assert.deepEqual(ran, [3]);
  assert.equal(inbox.size, 1);
}

/**
 * A value that passes the shape check and then cannot be decoded (`spec/events.md` section 8, step
 * 3) is rejected as `shape`, once, and the handler never runs: it is not a handler failure that
 * would be retried. The schema is hand-made: its `ref` can be resolved by `validate` but not by
 * `fromWire`.
 */
async function aValueThatCannotBeDecodedIsRejectedAsShape(): Promise<void> {
  const transport = new Spy();
  const schema: Schema = {
    k: 'object',
    fields: [
      {
        n: 'a',
        t: {
          k: 'ref',
          to: (): Schema => {
            if ((new Error().stack ?? '').includes('fromWire')) throw new Error('cannot decode a');
            return { k: 'int' } as Schema;
          },
        },
      },
    ],
  } as Schema;
  const def = { name: 'Odd', sig: 'veld:bookings/Odd@0000000000000000', schema, digest: null };
  const rejected: EventRejected[] = [];
  const failures: HandlerFailure[] = [];
  const waits: number[] = [];
  let runs = 0;
  await subscribeEvents(
    transport,
    [
      {
        topic: 'odd.v1',
        group: 'g',
        on: [{ def, run: () => void (runs += 1) }],
        retry: { max: 3, backoff: 'fixed' },
      },
    ],
    {
      onEventRejected: (e) => rejected.push(e),
      onHandlerError: (e) => failures.push(e),
      sleep: (ms) => {
        waits.push(ms);
        return Promise.resolve();
      },
    },
  );
  const envelope = {
    specversion: '1.0',
    id: 'odd-1',
    source: 'veld:bookings/Odd',
    type: def.sig,
    data: { a: 1 },
  };
  await transport.publish('odd.v1', null, new TextEncoder().encode(JSON.stringify(envelope)));
  await transport.idle();
  assert.equal(runs, 0, 'the handler never ran');
  assert.equal(rejected.length, 1);
  assert.equal(rejected[0]?.reason, 'shape');
  assert.equal(rejected[0]?.event, 'Odd');
  assert.match(rejected[0]?.message ?? '', /cannot decode a/);
  assert.equal(failures.length, 0, 'it is not a handler failure');
  assert.deepEqual(waits, [], 'and it is not retried');
}

async function main(): Promise<void> {
  await retriesWaitAndTheLastRunIsCopied();
  await aCopyThatIsPublishedAgainIsAValidMessage();
  await aCopyThatCannotBePublishedLeavesTheMessageUnacknowledged();
  await aRejectedMessageIsCopiedBeforeItIsAcknowledged();
  await theInboxSkipsASecondDelivery();
  await everyRunGoesThroughTheInboxAgain();
  await aFailedRunIsNotRecorded();
  await aValueThatCannotBeDecodedIsRejectedAsShape();
  console.log('events delivery process ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
