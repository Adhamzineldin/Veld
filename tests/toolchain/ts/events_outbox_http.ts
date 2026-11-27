// The outbox over the production handler (spec/events.md section 11, spec/targets/typescript.md
// section 5f): the moves of tests/events/outbox.veld.pending (compiled as the file `t.veld` of the
// package `bookings`) through `createBookingsHandler`, then the relay with the memory transport and
// the subscriber of `Notifications`, all in one process. Prints `events outbox http ok`.
//
// What it checks: a handler that writes its event (the entry is in the outbox, nothing was
// published), one that forgets (500 and `onError`), one that writes twice, one that writes an event
// of another edge, a handler that answers a declared error and writes nothing, an invalid event
// (`emit` throws the typed error and writes nothing), a move that emits nothing (no `emit`), a
// server created without an outbox, and the relay: the subscriber gets the event, a second run does
// not publish twice, an entry whose publish fails stays pending and stops the run.
import assert from 'node:assert/strict';
import * as srv from './server/index';
import type { VeldRequest } from './server/runtime/veld';

const BOOKING = '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21';
const decode = (bytes: Uint8Array): Record<string, unknown> => JSON.parse(new TextDecoder().decode(bytes)) as Record<string, unknown>;

let state: srv.Status = srv.Status.pending;
type Mode = 'right' | 'forget' | 'twice' | 'both' | 'declared' | 'invalid';
let mode: Mode = 'right';
let invalidError: unknown;
let emitSeen: unknown = 'not called';

const paid = { bookingId: BOOKING, amount: '120.00' };

function booking(status: srv.Status): srv.Booking {
  return { id: BOOKING, status, total: '120.00' };
}

const impl: srv.BookingsService = {
  async pay(req) {
    switch (mode) {
      case 'right':
        await req.emit.bookingPaid(paid);
        break;
      case 'twice':
        await req.emit.bookingPaid(paid);
        await req.emit.bookingPaid(paid);
        break;
      case 'declared':
        throw new srv.InvalidTransition({ field: req.transition.field, from: req.transition.from, to: req.transition.to });
      case 'invalid':
        invalidError = await req.emit.bookingPaid({ bookingId: BOOKING, amount: '-1.00' }).then(
          () => undefined,
          (e: unknown) => e,
        );
        break;
      case 'forget':
      case 'both':
        break;
    }
    return booking(srv.Status.paid);
  },
  async cancel(req) {
    if (mode === 'both') {
      // The edge from `paid` emits RefundDue; BookingCancelled belongs to the other edge.
      await req.emit.refundDue({ bookingId: BOOKING });
      await req.emit.bookingCancelled({ bookingId: BOOKING });
    } else if (req.transition.from === 'paid') {
      await req.emit.refundDue({ bookingId: BOOKING });
    } else {
      await req.emit.bookingCancelled({ bookingId: BOOKING });
    }
    return booking(srv.Status.cancelled);
  },
  async refund(req) {
    emitSeen = (req as { emit?: unknown }).emit;
    return booking(srv.Status.refunded);
  },
};

const errors: unknown[] = [];

function handlerWith(outbox: srv.veldEvents.Outbox): srv.veld.VeldHandler {
  return srv.createBookingsHandler(impl, {
    facts: { booking: async () => booking(state) },
    outbox,
    events: { newId: () => 'fixed-id', clock: () => new Date('2026-06-10T12:00:00Z') },
    onError: (err) => errors.push(err),
  });
}

function post(action: string): VeldRequest {
  return { method: 'POST', path: `/bookings/${BOOKING}/${action}`, headers: {} };
}

async function status(handler: srv.veld.VeldHandler, action: string): Promise<number> {
  return (await handler(post(action)))?.status ?? 0;
}

function reset(): void {
  errors.length = 0;
  invalidError = undefined;
  emitSeen = 'not called';
}

async function main(): Promise<void> {
  // A handler that writes its event: the entry is in the outbox, with the topic, the key and the
  // envelope; nothing was published (there is no transport to publish to).
  reset();
  state = srv.Status.pending;
  mode = 'right';
  let outbox = new srv.veldEvents.MemoryOutbox();
  let handler = handlerWith(outbox);
  assert.equal(await status(handler, 'pay'), 201);
  const pending = outbox.pending(100);
  assert.equal(pending.length, 1);
  assert.equal(pending[0]?.topic, 'shop.bookings.v1');
  assert.equal(pending[0]?.key, BOOKING);
  const envelope = decode(pending[0]?.bytes ?? new Uint8Array());
  assert.equal(envelope['id'], 'fixed-id');
  assert.equal(envelope['time'], '2026-06-10T12:00:00Z');
  assert.equal(envelope['source'], 'veld:bookings/Bookings');
  assert.match(String(envelope['type']), /^veld:bookings\/BookingPaid@/);
  assert.equal(errors.length, 0);

  // A handler that forgets: the state changed without its event, 500, reported to `onError`.
  reset();
  mode = 'forget';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  assert.equal(await status(handler, 'pay'), 500);
  assert.equal(errors.length, 1);
  assert.match(String((errors[0] as Error).message), /missing BookingPaid/);
  assert.equal(outbox.size, 0);

  // The event twice.
  reset();
  mode = 'twice';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  assert.equal(await status(handler, 'pay'), 500);
  assert.match(String((errors[0] as Error).message), /twice BookingPaid/);

  // An event of another edge: from `paid` the move to `cancelled` emits RefundDue, not BookingCancelled.
  reset();
  state = srv.Status.paid;
  mode = 'both';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  assert.equal(await status(handler, 'cancel'), 500);
  assert.match(String((errors[0] as Error).message), /unexpected BookingCancelled/);

  // The same move written right: the edge taken decides (RefundDue from `paid`, BookingCancelled from `pending`).
  reset();
  mode = 'right';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  assert.equal(await status(handler, 'cancel'), 201);
  assert.match(String(decode(outbox.pending(1)[0]?.bytes ?? new Uint8Array())['type']), /RefundDue@/);
  assert.equal(outbox.pending(1)[0]?.topic, 'shop.refunds.v1');
  state = srv.Status.pending;
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  assert.equal(await status(handler, 'cancel'), 201);
  assert.match(String(decode(outbox.pending(1)[0]?.bytes ?? new Uint8Array())['type']), /BookingCancelled@/);
  assert.equal(errors.length, 0);

  // A handler that answers a declared error need not have written.
  reset();
  state = srv.Status.pending;
  mode = 'declared';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  assert.equal(await status(handler, 'pay'), 409);
  assert.equal(errors.length, 0);
  assert.equal(outbox.size, 0);

  // A move refused by the guard never reaches the handler.
  state = srv.Status.paid;
  mode = 'right';
  assert.equal(await status(handler, 'pay'), 409);

  // An invalid event: `emit` throws the typed error, nothing is written.
  reset();
  state = srv.Status.pending;
  mode = 'invalid';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  await status(handler, 'pay');
  assert.ok(invalidError instanceof srv.ValidationError, String(invalidError));
  assert.equal(outbox.size, 0);

  // A move that emits nothing has no `emit` and is not checked.
  reset();
  state = srv.Status.paid;
  mode = 'right';
  handler = handlerWith(new srv.veldEvents.MemoryOutbox());
  assert.equal(await status(handler, 'refund'), 201);
  assert.equal(emitSeen, undefined);

  // A server created without an outbox fails when it is created.
  assert.throws(
    () => srv.createBookingsHandler(impl, { facts: { booking: async () => booking(state) } } as unknown as srv.BookingsHooks),
    /hooks\.outbox is required/,
  );

  // --- end to end: the relay publishes, a subscriber's handler gets the event -----------------------
  reset();
  state = srv.Status.pending;
  mode = 'right';
  outbox = new srv.veldEvents.MemoryOutbox();
  handler = handlerWith(outbox);
  const transport = new srv.veldEvents.MemoryTransport();
  const received: { amount: string; id: string }[] = [];
  await srv.subscribeNotificationsEvents(transport, {
    sendReceipt: (event, meta) => void received.push({ amount: event.amount, id: meta.id }),
    sendApology: () => undefined,
  });
  assert.equal(await status(handler, 'pay'), 201);
  assert.equal(received.length, 0, 'nothing is published until the relay runs');
  assert.equal(await srv.runBookingsOutboxRelay(outbox, transport), 1);
  await transport.idle();
  assert.deepEqual(received, [{ amount: '120.00', id: 'fixed-id' }]);
  assert.equal(outbox.size, 0);
  // Run twice: nothing is published twice.
  assert.equal(await srv.runBookingsOutboxRelay(outbox, transport), 0);
  await transport.idle();
  assert.equal(received.length, 1);

  // An entry whose publish fails stays pending, and stops the run (the next entry is not tried).
  mode = 'right';
  state = srv.Status.pending;
  await status(handler, 'pay');
  await status(handler, 'pay');
  assert.equal(outbox.size, 2);
  const failures: unknown[] = [];
  let attempts = 0;
  const refusing: srv.veldEvents.Transport = {
    publish: () => {
      attempts += 1;
      return Promise.reject(new Error('broker down'));
    },
    subscribe: (topic, group, deliver) => transport.subscribe(topic, group, deliver),
  };
  assert.equal(await srv.runBookingsOutboxRelay(outbox, refusing, { onError: (e, entry) => failures.push([String(e), entry.id]) }), 0);
  assert.equal(attempts, 1, 'the run stops at the first failure');
  assert.equal(outbox.size, 2);
  assert.equal(failures.length, 1);
  assert.equal(await srv.runBookingsOutboxRelay(outbox, transport, { limit: 1 }), 1, 'the limit bounds the run');
  assert.equal(outbox.size, 1);
  assert.equal(await srv.runBookingsOutboxRelay(outbox, transport), 1);
  await transport.idle();
  assert.equal(received.length, 3);

  console.log('events outbox http ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
