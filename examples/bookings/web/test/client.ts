// Calls the running bookings server through the generated client.
// Usage: VELD_BOOKINGS_URL=http://127.0.0.1:3000 npm test
import assert from 'node:assert/strict';
import { BookingsClient, NotFound, RoomClosed, RoomFull, ValidationError, veld } from '@veld/web';

const DOUBLE = '00000000-0000-4000-8000-0000000000a1';
const CLOSED = '00000000-0000-4000-8000-0000000000a3';
const stay = { start: '2030-06-10T00:00:00Z', end: '2030-06-12T00:00:00Z' };
const backwards = { start: '2030-06-12T00:00:00Z', end: '2030-06-10T00:00:00Z' };

const baseUrl = process.env.VELD_BOOKINGS_URL;
assert.ok(baseUrl, 'set VELD_BOOKINGS_URL to the running server');

async function failure(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (e) {
    return e;
  }
  throw new Error('the call was expected to fail');
}

async function main(): Promise<void> {
  // A client that counts what it sends.
  let sent = 0;
  const counting: veld.FetchLike = (url, init) => {
    sent += 1;
    return fetch(url, init as RequestInit);
  };
  const client = new BookingsClient({ baseUrl, fetch: counting });

  // A valid booking: a created booking, priced by the server (2 nights at 89.50).
  const booking = await client.createBooking({ body: { roomId: DOUBLE, guests: 2, ...stay } });
  assert.equal(booking.total, '179.00');
  assert.deepEqual(await client.getBooking({ id: booking.id }), booking);

  // A declared error with a payload, from a `require ... else`.
  const full = await failure(client.createBooking({ body: { roomId: DOUBLE, guests: 3, ...stay } }));
  assert.ok(RoomFull.matches(full));
  assert.equal(full.data.capacity, 2);
  assert.ok(RoomClosed.matches(await failure(client.createBooking({ body: { roomId: CLOSED, guests: 1, ...stay } }))));
  assert.ok(NotFound.matches(await failure(client.getBooking({ id: '00000000-0000-4000-8000-00000000ffff' }))));

  // An invariant broken: the client refuses locally and sends nothing.
  const before = sent;
  const local = await failure(client.createBooking({ body: { roomId: DOUBLE, guests: 2, ...backwards } }));
  assert.ok(ValidationError.matches(local));
  assert.equal(local.local, true);
  assert.equal(sent, before, 'a request that breaks a rule is not sent');
  assert.deepEqual(local.violations[0]?.fields, ['end', 'start']);

  // With the local check off the server answers the same: a 422 with `violations`.
  const trusting = new BookingsClient({ baseUrl, rules: false });
  const remote = await failure(trusting.createBooking({ body: { roomId: DOUBLE, guests: 2, ...backwards } }));
  assert.ok(ValidationError.matches(remote));
  assert.equal(remote.local, false);
  assert.equal(remote.violations.length, 1);
  assert.equal(remote.violations[0]?.message, 'The stay must end after it starts');

  // A rule that needs the clock (`start >= now`) is the server's to decide.
  const past = await failure(
    trusting.createBooking({ body: { roomId: DOUBLE, guests: 2, start: '2020-01-01T00:00:00Z', end: '2020-01-02T00:00:00Z' } }),
  );
  assert.ok(ValidationError.matches(past));
  assert.equal(past.violations[0]?.message, 'A booking cannot start in the past');

  console.log('client ok');
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
