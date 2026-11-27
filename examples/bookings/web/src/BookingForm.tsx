import { useState } from 'react';
import type { FormEvent } from 'react';
import { BookingsClient, NewBooking, RoomClosed, RoomFull } from '@veld/web';
import type { NewBooking as NewBookingData, User } from '@veld/web';
import { useCreateBooking, useFormRules, useListRooms } from '@veld/web/bookings/react';
import { describe } from './describe';

/** A date input gives `2026-06-10`; the contract wants a datetime. */
const midnight = (day: string): string | undefined => (day === '' ? undefined : `${day}T00:00:00Z`);

export function BookingForm({ me, onBooked }: { me: User; onBooked: () => void }) {
  // The rooms come from the public endpoint `listRooms` (`@auth(none)`).
  const rooms = useListRooms().data ?? [];
  const [chosen, setChosen] = useState('');
  const roomId = chosen === '' ? (rooms[0]?.id ?? '') : chosen;
  const [guests, setGuests] = useState('1');
  const [start, setStart] = useState('');
  const [end, setEnd] = useState('');

  // The draft is what the user has typed so far. Fields that are still empty are left out.
  const draft: Partial<NewBookingData> = { roomId, guests: guests === '' ? undefined : Number(guests) };
  const from = midnight(start);
  const to = midnight(end);
  if (from !== undefined) draft.start = from;
  if (to !== undefined) draft.end = to;

  // The invariants of the contract, checked on the draft as the user types. A rule that needs a
  // field the user has not filled in yet is skipped, not failed.
  const rules = useFormRules(NewBooking, draft);

  const create = useCreateBooking();
  const complete = from !== undefined && to !== undefined && guests !== '';

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (from === undefined || to === undefined) return;
    const booking = await create.mutate({ body: { roomId, guests: Number(guests), start: from, end: to } });
    if (booking !== undefined) onBooked();
  }

  // The policy `MayBook`, asked here with the generated helper. The server asks it again.
  const mayBook = BookingsClient.canCreateBooking(me);

  const problems = (field: string) =>
    (rules.messages[field] ?? []).map((m) => (
      <p key={m} role="alert" className="problem">
        {m}
      </p>
    ));

  return (
    <form onSubmit={submit}>
      <label>
        Room
        <select value={roomId} onChange={(e) => setChosen(e.target.value)}>
          {rooms.map((r) => (
            <option key={r.id} value={r.id}>
              {r.name} ({r.capacity} guests)
            </option>
          ))}
        </select>
      </label>
      <label>
        Guests
        <input type="number" value={guests} onChange={(e) => setGuests(e.target.value)} />
      </label>
      {problems('guests')}
      <label>
        Arrival
        <input type="date" value={start} onChange={(e) => setStart(e.target.value)} />
      </label>
      {problems('start')}
      <label>
        Departure
        <input type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
      </label>
      {problems('end')}

      <button type="submit" disabled={!mayBook || !complete || !rules.valid || create.loading}>
        Book
      </button>
      {!mayBook && <p>The server would refuse this account.</p>}

      {create.data && (
        <p role="status">
          Booked. Total {create.data.total}, booking {create.data.id}.
        </p>
      )}
      {RoomFull.matches(create.error) && (
        <p role="alert" className="problem">
          That room sleeps only {create.error.data.capacity} guests.
        </p>
      )}
      {RoomClosed.matches(create.error) && (
        <p role="alert" className="problem">
          That room is closed.
        </p>
      )}
      {/* Any other refusal (a denied account, a rule, a missing room) is shown in the server's own words. */}
      {create.error !== undefined && create.error !== null && !RoomFull.matches(create.error) && !RoomClosed.matches(create.error) && (
        <p role="alert" className="problem">
          {describe(create.error)}
        </p>
      )}
    </form>
  );
}
