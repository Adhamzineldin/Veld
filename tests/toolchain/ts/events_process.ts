// Two generated services in one process over the memory transport: `Bookings` publishes,
// `Notifications` and `Ledger` subscribe (tests/events/bookings.veld). Redelivery when a handler
// throws, two groups that each get the message, competing consumers of one group, ordering per
// key, and what an invalid event does. Prints `events process ok`.
import { strict as assert } from 'assert';

import { createBookingsPublisher } from './server/server/bookings-events';
import { subscribeLedgerEvents } from './server/server/ledger-events';
import { subscribeNotificationsEvents } from './server/server/notifications-events';
import type { EventMeta } from './server/runtime/events';
import { MemoryTransport } from './server/runtime/events';
import type { Transport } from './server/runtime/events';
import { BookingCancelled, BookingPaid } from './server/events/t';
import { Reason } from './server/models/t';

const A = '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21';
const B = '7e6f0a52-3d84-4a4b-9c27-0f8e2b1d6a90';

const paid = (bookingId: string, amount = '120.00', nights = 1): BookingPaid => ({ bookingId, amount, paidAt: '2026-06-10T11:59:58Z', nights });

function counter(): () => string {
  let n = 0;
  return () => `id-${(n += 1)}`;
}

/** A pass through a few ticks, so that a handler is slow enough for order to matter. */
async function slow(): Promise<void> {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
}

async function twoGroupsEachGetTheMessage(): Promise<void> {
  const transport = new MemoryTransport();
  const heard: string[] = [];
  const metas: EventMeta[] = [];
  await subscribeNotificationsEvents(transport, {
    sendReceipt: (e, m) => {
      heard.push(`receipt ${e.bookingId}`);
      metas.push(m);
    },
    sendApology: () => {
      heard.push('apology');
    },
  });
  await subscribeLedgerEvents(transport, {
    recordPayment: (e) => {
      heard.push(`ledger ${e.bookingId}`);
    },
    auditPayment: () => {
      heard.push('audit');
    },
  });
  const bookings = createBookingsPublisher(transport, { newId: counter(), clock: () => new Date('2026-06-10T12:00:00Z') });
  const sent = await bookings.bookingEvents.bookingPaid(paid(A));
  await transport.idle();
  assert.deepEqual(heard.sort(), [`ledger ${A}`, `receipt ${A}`]);
  assert.equal(sent.id, 'id-1');
  assert.equal(sent.subject, A);
  assert.equal(metas[0]?.id, 'id-1');
  assert.equal(metas[0]?.source, 'veld:bookings/Bookings');
  assert.equal(metas[0]?.type, BookingPaid.sig);
  assert.equal(metas[0]?.subject, A);
  assert.equal(metas[0]?.time, '2026-06-10T12:00:00Z');
  assert.equal(metas[0]?.attempt, 1);
  // The other channel is another topic and another group.
  await bookings.audit.bookingPaid(paid(B));
  await transport.idle();
  assert.deepEqual(heard.sort(), [`ledger ${A}`, 'audit', `receipt ${A}`].sort());
  assert.equal(transport.deadLetters().length, 0);
}

async function aHandlerThatThrowsGetsTheMessageAgain(): Promise<void> {
  const transport = new MemoryTransport();
  const attempts: number[] = [];
  const failures: unknown[] = [];
  const receipts: string[] = [];
  await subscribeNotificationsEvents(transport, {
    sendReceipt: (e) => {
      receipts.push(e.bookingId);
    },
    sendApology: () => undefined,
  });
  await subscribeLedgerEvents(
    transport,
    {
      recordPayment: (_e, m) => {
        attempts.push(m.attempt);
        if (m.attempt < 3) throw new Error(`not yet (${m.attempt})`);
      },
      auditPayment: () => undefined,
    },
    { onHandlerError: (f) => failures.push(f) },
  );
  await createBookingsPublisher(transport).bookingEvents.bookingPaid(paid(A));
  await transport.idle();
  assert.deepEqual(attempts, [1, 2, 3], 'at least once: delivered again at once, with the attempt counted');
  assert.equal(failures.length, 2);
  assert.deepEqual(receipts, [A], 'the other group is not delivered again');
  assert.equal(transport.deadLetters().length, 0);
}

async function aMessageThatNeverSucceedsIsDeadLettered(): Promise<void> {
  const transport = new MemoryTransport({ maxAttempts: 2 });
  let calls = 0;
  await subscribeLedgerEvents(transport, {
    recordPayment: () => {
      calls += 1;
      throw new Error('always');
    },
    auditPayment: () => undefined,
  });
  await createBookingsPublisher(transport).bookingEvents.bookingPaid(paid(A));
  await transport.idle();
  assert.equal(calls, 2);
  const dead = transport.deadLetters();
  assert.equal(dead.length, 1);
  assert.equal(dead[0]?.group, 'ledger');
  assert.equal(dead[0]?.topic, 'shop.bookings.v1');
  assert.equal(dead[0]?.attempts, 2);
  assert.equal(dead[0]?.key, A);
}

async function orderPerKeyHolds(): Promise<void> {
  const transport = new MemoryTransport();
  const seen: string[] = [];
  await subscribeLedgerEvents(transport, {
    recordPayment: async (e) => {
      await slow();
      seen.push(`${e.bookingId.slice(0, 4)}:${e.nights}`);
    },
    auditPayment: () => undefined,
  });
  const bookings = createBookingsPublisher(transport);
  for (let n = 1; n <= 6; n += 1) {
    await bookings.bookingEvents.bookingPaid(paid(n % 2 === 0 ? A : B, '1.00', n));
  }
  await transport.idle();
  const of = (id: string): string[] => seen.filter((s) => s.startsWith(id.slice(0, 4)));
  assert.deepEqual(of(A), ['5d2b:2', '5d2b:4', '5d2b:6']);
  assert.deepEqual(of(B), ['7e6f:1', '7e6f:3', '7e6f:5']);
  assert.equal(seen.length, 6);
}

async function competingConsumersOfOneGroupShareTheMessages(): Promise<void> {
  const transport = new MemoryTransport();
  const one: string[] = [];
  const two: string[] = [];
  const quiet = { sendApology: () => undefined };
  const first = await subscribeNotificationsEvents(transport, { ...quiet, sendReceipt: (e) => void one.push(e.bookingId) });
  await subscribeNotificationsEvents(transport, { ...quiet, sendReceipt: (e) => void two.push(e.bookingId) });
  const bookings = createBookingsPublisher(transport);
  for (let n = 0; n < 4; n += 1) await bookings.bookingEvents.bookingPaid(paid(A, '1.00', n + 1));
  await transport.idle();
  assert.equal(one.length + two.length, 4, 'each message is handled by one copy of the service');
  assert.ok(one.length > 0 && two.length > 0, 'both copies get some');
  await first.close();
  await bookings.bookingEvents.bookingPaid(paid(A));
  await transport.idle();
  assert.equal(one.length + two.length, 5);
  assert.equal(one.length, 2);
}

async function aTopicNobodySubscribedToDropsTheMessage(): Promise<void> {
  const transport = new MemoryTransport();
  const bookings = createBookingsPublisher(transport);
  await bookings.bookingEvents.bookingPaid(paid(A));
  await transport.idle();
  const seen: string[] = [];
  const handle = await subscribeLedgerEvents(transport, { recordPayment: (e) => void seen.push(e.bookingId), auditPayment: () => undefined });
  await transport.idle();
  assert.deepEqual(seen, [], 'no retention and no replay');
  await handle.close();
  await bookings.bookingEvents.bookingPaid(paid(A));
  await transport.idle();
  assert.deepEqual(seen, [], 'a closed subscription hears nothing');
}

async function anInvalidEventNeverLeavesThePublisher(): Promise<void> {
  const sent: Uint8Array[] = [];
  const recording: Transport = {
    publish: (_topic, _key, bytes) => {
      sent.push(bytes);
      return Promise.resolve();
    },
    subscribe: () => Promise.resolve({ close: () => Promise.resolve() }),
  };
  const heard: { rule: string; outcome: string; position: string; operation: string }[] = [];
  const bookings = createBookingsPublisher(recording, { onRuleViolation: (e) => heard.push(e) });
  // An invariant: amount > 0.
  const err = await bookings.bookingEvents.bookingPaid(paid(A, '0.00')).then(
    () => null,
    (e: unknown) => e as Error & { title: string; violations: { rule: string; path: string }[]; local: boolean },
  );
  assert.ok(err !== null);
  assert.equal(err.title, 'ValidationError');
  assert.equal(err.local, true);
  assert.equal(err.violations.length, 1);
  assert.equal(err.violations[0]?.rule, BookingPaid.ruleIds.positive);
  assert.equal(err.violations[0]?.path, '$');
  assert.deepEqual(heard.map((h) => [h.outcome, h.position, h.operation]), [['rejected', 'output', BookingPaid.sig]]);
  // The shape: a field of the wrong type.
  const shape = await bookings.bookingEvents.bookingPaid({ ...paid(A), paidAt: 'yesterday' }).then(
    () => null,
    (e: unknown) => e as Error & { title: string },
  );
  assert.equal(shape?.title, 'ValidationError');
  // The invariant of an optional field.
  const note = await bookings.bookingEvents
    .bookingCancelled({ bookingId: A, reason: Reason.guest, note: '' })
    .then(
      () => null,
      (e: unknown) => e as Error & { title: string },
    );
  assert.equal(note?.title, 'ValidationError');
  assert.equal(sent.length, 0, 'nothing was sent');
  await bookings.bookingEvents.bookingCancelled({ bookingId: A, reason: Reason.guest, note: 'x' });
  assert.equal(sent.length, 1);
}

async function aRejectedMessageIsAcknowledgedAndReported(): Promise<void> {
  const transport = new MemoryTransport();
  const rejected: { reason: string; event: string | null; message: string }[] = [];
  const called: string[] = [];
  await subscribeLedgerEvents(
    transport,
    { recordPayment: () => void called.push('record'), auditPayment: () => undefined },
    { onEventRejected: (e) => rejected.push({ reason: e.reason, event: e.event, message: e.message }) },
  );
  const text = (value: unknown): Uint8Array => new TextEncoder().encode(JSON.stringify(value));
  const message = (data: unknown, veldrules = '0018ccc5f17d6c76'): unknown => ({
    specversion: '1.0', id: 'x', source: 's', type: BookingPaid.sig, veldrules, data,
  });
  await transport.publish('shop.bookings.v1', null, text(message({ bookingId: A, amount: '0.00', paidAt: '2026-06-10T11:59:58Z' })));
  await transport.publish('shop.bookings.v1', null, text(message({ amount: '1.00' })));
  await transport.publish('shop.bookings.v1', null, new TextEncoder().encode('{'));
  await transport.idle();
  assert.deepEqual(rejected.map((r) => r.reason), ['rule', 'shape', 'malformed']);
  assert.equal(rejected[0]?.event, 'BookingPaid');
  assert.equal(rejected[2]?.event, null);
  assert.deepEqual(called, []);
  assert.equal(transport.deadLetters().length, 0, 'acknowledged, not dead-lettered');
}

async function main(): Promise<void> {
  await twoGroupsEachGetTheMessage();
  await aHandlerThatThrowsGetsTheMessageAgain();
  await aMessageThatNeverSucceedsIsDeadLettered();
  await orderPerKeyHolds();
  await competingConsumersOfOneGroupShareTheMessages();
  await aTopicNobodySubscribedToDropsTheMessage();
  await anInvalidEventNeverLeavesThePublisher();
  await aRejectedMessageIsAcknowledgedAndReported();
  assert.ok(BookingCancelled.sig.length > 0);
  process.stdout.write('events process ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
