// Calls the running bookings server through the generated client.
// Usage: VELD_BOOKINGS_URL=http://127.0.0.1:3000 npm test
import assert from 'node:assert/strict';
import {
  AmountMismatch,
  BookingsClient,
  Forbidden,
  InvalidTransition,
  NotFound,
  RoomClosed,
  RoomFull,
  Status,
  Unauthorized,
  ValidationError,
  veld,
} from '@veld/web';

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

/** A client that signs in with one of the demo tokens of the server. */
function as(token: string, extra: { rules?: boolean; fetch?: veld.FetchLike } = {}): BookingsClient {
  return new BookingsClient({ baseUrl, auth: { bearer: () => token }, ...extra });
}

async function main(): Promise<void> {
  // A client that counts what it sends.
  let sent = 0;
  const counting: veld.FetchLike = (url, init) => {
    sent += 1;
    return fetch(url, init as RequestInit);
  };
  const ann = as('token-ann', { fetch: counting });
  const bob = as('token-bob');
  const sam = as('token-sam');
  const eve = as('token-eve');

  // Who am I: the auth hook returned a typed User.
  const me = await ann.whoAmI();
  assert.equal(me.name, 'Ann (guest)');

  // A public endpoint needs no token. A protected one without a token is a 401.
  const anonymous = new BookingsClient({ baseUrl });
  assert.equal((await anonymous.listRooms()).length, 3);
  assert.ok(Unauthorized.matches(await failure(anonymous.listBookings())));
  assert.ok(Unauthorized.matches(await failure(as('token-nobody').listBookings())));

  // A valid booking: a created booking, priced by the server (2 nights at 89.50), owned by the caller.
  const booking = await ann.createBooking({ body: { roomId: DOUBLE, guests: 2, ...stay } });
  assert.equal(booking.total, '179.00');
  assert.equal(booking.guestId, me.id);
  assert.equal(booking.status, 'pending');
  assert.deepEqual(await ann.getBooking({ id: booking.id }), booking);

  // A declared error with a payload, from a `require ... else`.
  const full = await failure(ann.createBooking({ body: { roomId: DOUBLE, guests: 3, ...stay } }));
  assert.ok(RoomFull.matches(full));
  assert.equal(full.data.capacity, 2);
  assert.ok(RoomClosed.matches(await failure(ann.createBooking({ body: { roomId: CLOSED, guests: 1, ...stay } }))));

  // A missing record is a 404, even for a caller who may not see anything.
  const missing = await failure(bob.getBooking({ id: '00000000-0000-4000-8000-00000000ffff' }));
  assert.ok(NotFound.matches(missing));
  assert.equal(missing.data.resource, 'booking');

  // The policies. Another guest and a suspended account are refused with the server's own words.
  const stranger = await failure(bob.getBooking({ id: booking.id }));
  assert.ok(Forbidden.matches(stranger));
  assert.equal(stranger.data.message, 'not permitted');
  const suspended = await failure(eve.createBooking({ body: { roomId: DOUBLE, guests: 1, ...stay } }));
  assert.ok(Forbidden.matches(suspended));
  assert.equal(suspended.data.message, 'A suspended account cannot book');
  assert.ok(!(await bob.listBookings()).some((b) => b.id === booking.id), 'a guest lists only their own bookings');
  assert.ok((await sam.listBookings()).some((b) => b.id === booking.id), 'the staff lists every booking');

  // The generated helpers answer what the server will answer.
  const sams = await sam.whoAmI();
  const eves = await eve.whoAmI();
  const bobs = await bob.whoAmI();
  assert.equal(BookingsClient.canPayBooking(booking, me), true);
  assert.equal(BookingsClient.canPayBooking(booking, bobs), false);
  assert.equal(BookingsClient.canGetBooking(booking, sams), true);
  assert.equal(BookingsClient.canRefundBooking({ ...booking, status: 'paid' }, me), false);
  assert.equal(BookingsClient.canRefundBooking({ ...booking, status: 'paid' }, sams), true);
  assert.equal(BookingsClient.canCreateBooking(eves), false);
  assert.deepEqual(Status.nextStates('pending'), ['paid', 'cancelled']);
  assert.equal(Status.isFinal('refunded'), true);

  // The state machine. The payment must be the price, but only after the move is known to be legal.
  const wrong = await failure(ann.payBooking({ id: booking.id, body: { amount: '1.00' } }));
  assert.ok(AmountMismatch.matches(wrong));
  assert.equal(wrong.data.expected, '179.00');
  const paid = await ann.payBooking({ id: booking.id, body: { amount: booking.total } });
  assert.equal(paid.status, 'paid');
  assert.equal(BookingsClient.canPayBooking(paid, me), false, 'a paid booking cannot be paid again');

  // An illegal move: a 409 with the two states. The client sends it anyway when asked to.
  const again = await failure(ann.payBooking({ id: booking.id, body: { amount: booking.total } }));
  assert.ok(InvalidTransition.matches(again));
  assert.deepEqual(again.data, { field: 'status', from: 'paid', to: 'paid' });

  // The policy is asked before the transition: a stranger learns nothing about the state.
  assert.ok(Forbidden.matches(await failure(bob.cancelBooking({ id: booking.id }))));
  // A guest may not refund; the staff may. A refunded booking is final.
  assert.ok(Forbidden.matches(await failure(ann.refundBooking({ id: booking.id }))));
  const refunded = await sam.refundBooking({ id: booking.id });
  assert.equal(refunded.status, 'refunded');
  const stuck = await failure(sam.cancelBooking({ id: booking.id }));
  assert.ok(InvalidTransition.matches(stuck));
  assert.equal(stuck.data.from, 'refunded');

  // A second booking runs the other way: cancelled, then nothing.
  const second = await ann.createBooking({ body: { roomId: DOUBLE, guests: 1, ...stay } });
  assert.equal((await ann.cancelBooking({ id: second.id })).status, 'cancelled');
  assert.ok(InvalidTransition.matches(await failure(ann.checkIn({ id: second.id }))));

  // An invariant broken: the client refuses locally and sends nothing.
  const before = sent;
  const local = await failure(ann.createBooking({ body: { roomId: DOUBLE, guests: 2, ...backwards } }));
  assert.ok(ValidationError.matches(local));
  assert.equal(local.local, true);
  assert.equal(sent, before, 'a request that breaks a rule is not sent');
  assert.deepEqual(local.violations[0]?.fields, ['end', 'start']);

  // With the local check off the server answers the same: a 422 with `violations`.
  const trusting = as('token-ann', { rules: false });
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
