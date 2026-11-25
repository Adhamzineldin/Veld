// The business logic of the bookings server, and the data it works on (in memory).
//
// Nothing here checks what the contract already checks. The generated handler has already
// verified that the stay ends after it starts and lasts at most 30 days, that the room exists
// and is open, that the party fits and that the stay does not start in the past, before any
// function below runs. It will also check that the booking this code returns keeps the room and
// the guests of the request.
import { randomUUID } from 'node:crypto';
import { NotFound } from '@veld/api';
import type { Booking, BookingsFactLoaders, BookingsService, Room } from '@veld/api';

/** The rooms of the hotel. The web app lists the same three. */
export const ROOMS: readonly Room[] = [
  { id: '00000000-0000-4000-8000-0000000000a1', name: 'Double room', capacity: 2, closed: false, nightly: '89.50' },
  { id: '00000000-0000-4000-8000-0000000000a2', name: 'Family suite', capacity: 4, closed: false, nightly: '160.00' },
  { id: '00000000-0000-4000-8000-0000000000a3', name: 'Attic room (closed)', capacity: 2, closed: true, nightly: '70.00' },
];

export interface Store {
  rooms: Map<string, Room>;
  bookings: Map<string, Booking>;
}

export function createStore(): Store {
  return {
    rooms: new Map(ROOMS.map((room) => [room.id, room])),
    bookings: new Map(),
  };
}

/** One loader per `fact` of the contract. A room that is not there is `undefined`: the server answers 404. */
export function createLoaders(store: Store): BookingsFactLoaders {
  return {
    async room(key) {
      return store.rooms.get(key);
    },
  };
}

const DAY_MS = 24 * 60 * 60 * 1000;

/** The price of a stay: whole nights, started nights count. Money is handled in cents. */
export function price(nightly: string, start: string, end: string): string {
  const nights = Math.ceil((Date.parse(end) - Date.parse(start)) / DAY_MS);
  const [units = '0', cents = ''] = nightly.split('.');
  const nightlyCents = Number(units) * 100 + Number(cents.padEnd(2, '0').slice(0, 2));
  const total = nightlyCents * nights;
  return `${Math.floor(total / 100)}.${String(total % 100).padStart(2, '0')}`;
}

export function createService(store: Store): BookingsService {
  return {
    async createBooking({ body, room }) {
      const booking: Booking = {
        id: randomUUID(),
        roomId: body.roomId,
        guests: body.guests,
        start: body.start,
        end: body.end,
        total: price(room.nightly, body.start, body.end),
      };
      store.bookings.set(booking.id, booking);
      return booking;
    },

    async getBooking({ id }) {
      const booking = store.bookings.get(id);
      if (booking === undefined) throw new NotFound({ resource: id });
      return booking;
    },
  };
}
