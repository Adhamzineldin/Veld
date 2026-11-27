import { BookingsClient, Status } from '@veld/web';
import type { Booking, User } from '@veld/web';
import { useCancelBooking, useCheckIn, useListBookings, usePayBooking, useRefundBooking } from '@veld/web/bookings/react';
import { describe } from './describe';

/**
 * The bookings the caller may manage, each with the moves that make sense.
 *
 * A button appears only when both questions are yes:
 *  - the state machine allows the move: `Status.nextStates(booking.status)`;
 *  - the server would let this caller make it: `BookingsClient.canPayBooking(booking, me)` and friends
 *    (the policies and the transition guard, evaluated here with the same rules).
 * The server decides anyway. If it refuses a call (the booking moved in another window, say), the
 * message it sent is shown.
 */
export function BookingList({ me }: { me: User }) {
  const list = useListBookings();
  const pay = usePayBooking();
  const cancel = useCancelBooking();
  const checkIn = useCheckIn();
  const refund = useRefundBooking();

  // Whatever went wrong last, in the server's own words.
  const refused = pay.error ?? cancel.error ?? checkIn.error ?? refund.error;
  const clear = () => [pay, cancel, checkIn, refund].forEach((m) => m.reset());

  /** The calls, by the state they move a booking to. `can` asks the generated helper. */
  const moves: Record<string, { label: string; can: (b: Booking) => boolean; run: (b: Booking) => Promise<unknown> }> = {
    paid: {
      label: 'Pay',
      can: (b) => BookingsClient.canPayBooking(b, me),
      run: (b) => pay.mutate({ id: b.id, body: { amount: b.total } }),
    },
    cancelled: {
      label: 'Cancel',
      can: (b) => BookingsClient.canCancelBooking(b, me),
      run: (b) => cancel.mutate({ id: b.id }),
    },
    checkedIn: {
      label: 'Check in',
      can: (b) => BookingsClient.canCheckIn(b, me),
      run: (b) => checkIn.mutate({ id: b.id }),
    },
    refunded: {
      label: 'Refund',
      can: (b) => BookingsClient.canRefundBooking(b, me),
      run: (b) => refund.mutate({ id: b.id }),
    },
  };

  async function act(booking: Booking, state: string) {
    clear();
    await moves[state]?.run(booking);
    list.refetch();
  }

  const bookings = list.data ?? [];
  return (
    <div>
      {bookings.length === 0 && <p>No bookings yet.</p>}
      <ul>
        {bookings.map((b) => (
          <li key={b.id} data-status={b.status}>
            {b.start.slice(0, 10)} to {b.end.slice(0, 10)}, {b.guests} guest(s), total {b.total}: <strong>{b.status}</strong>
            {Status.isFinal(b.status) && ' (final)'}{' '}
            {Status.nextStates(b.status).map((next) => {
              const move = moves[next];
              if (move === undefined || !move.can(b)) return null;
              return (
                <button key={next} type="button" onClick={() => void act(b, next)}>
                  {move.label}
                </button>
              );
            })}
          </li>
        ))}
      </ul>
      {refused !== undefined && refused !== null && (
        <p role="alert" className="problem">
          {describe(refused)}
        </p>
      )}
    </div>
  );
}
