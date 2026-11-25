// The business logic of the bookings server, and the data it works on (in memory).
//
// Nothing here checks what the contract already checks. Before any function below runs, the
// generated handler has verified who the caller is (the auth hook), that the booking exists, that
// the caller may manage it (the policies), that the move is legal (the state machine), and that the
// stay, the party size and the payment are right. It also checks what this code returns.
import { randomUUID } from 'node:crypto';
import { InvalidTransition, NotFound } from '@veld/api';
import type { Booking, BookingsFactLoaders, BookingsService, Room, Status, User, veld } from '@veld/api';

/** The rooms of the hotel. The web app reads them from `GET /bookings/rooms`. */
export const ROOMS: readonly Room[] = [
  { id: '00000000-0000-4000-8000-0000000000a1', name: 'Double room', capacity: 2, closed: false, nightly: '89.50' },
  { id: '00000000-0000-4000-8000-0000000000a2', name: 'Family suite', capacity: 4, closed: false, nightly: '160.00' },
  { id: '00000000-0000-4000-8000-0000000000a3', name: 'Attic room (closed)', capacity: 2, closed: true, nightly: '70.00' },
];

/** The demo accounts. A real server would look a session up; here the token is the key. */
export const USERS: Readonly<Record<string, User>> = {
  'token-ann': { id: '00000000-0000-4000-8000-0000000000aa', name: 'Ann (guest)', roles: [], suspended: false },
  'token-bob': { id: '00000000-0000-4000-8000-0000000000bb', name: 'Bob (guest)', roles: [], suspended: false },
  'token-sam': { id: '00000000-0000-4000-8000-0000000000cc', name: 'Sam (staff)', roles: ['staff'], suspended: false },
  'token-eve': { id: '00000000-0000-4000-8000-0000000000ee', name: 'Eve (suspended guest)', roles: [], suspended: true },
};

/** The auth hook: a bearer token to the typed `User` of the contract, or `null` for a 401. */
export async function authenticate(token: string): Promise<User | null> {
  return USERS[token] ?? null;
}

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

/** One loader per `fact` of the contract. A record that is not there is `undefined`: the server answers 404. */
export function createLoaders(store: Store): BookingsFactLoaders {
  return {
    async room(key) {
      return store.rooms.get(key);
    },
    async booking(key) {
      return store.bookings.get(key);
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

/**
 * The caller. The auth hook has already run and `allow` has already used it; the handler only needs
 * it to say whose booking this is. The generated `RequestContext` types it as `unknown`, so it is
 * narrowed here, once.
 */
function caller(ctx: veld.RequestContext): User {
  return ctx.principal as User;
}

/**
 * Moves a booking from `transition.from` to `transition.to`, but only if it is still in
 * `transition.from`. The contract checked the state a moment ago on the booking it loaded; another
 * request may have moved it since. This is the conditional write the guide asks for. With a database
 * it is `UPDATE bookings SET status = $to WHERE id = $id AND status = $from`, and "no row changed"
 * means the same as the `throw` below. Here the store is one `Map` and this runs without a pause,
 * so the compare and the write cannot be separated.
 */
function move(store: Store, id: string, transition: veld.Transition<Status>): Booking {
  const current = store.bookings.get(id);
  if (current === undefined) throw new NotFound({ resource: 'booking' });
  if (current.status !== transition.from) {
    throw new InvalidTransition({ field: transition.field, from: current.status, to: transition.to });
  }
  const moved: Booking = { ...current, status: transition.to };
  store.bookings.set(id, moved);
  return moved;
}

export function createService(store: Store): BookingsService {
  return {
    async listRooms() {
      return [...store.rooms.values()];
    },

    async whoAmI(_req, ctx) {
      return caller(ctx);
    },

    async createBooking({ body, room }, ctx) {
      const booking: Booking = {
        id: randomUUID(),
        guestId: caller(ctx).id,
        roomId: body.roomId,
        guests: body.guests,
        start: body.start,
        end: body.end,
        total: price(room.nightly, body.start, body.end),
        status: 'pending',
      };
      store.bookings.set(booking.id, booking);
      return booking;
    },

    async listBookings(_req, ctx) {
      const me = caller(ctx);
      const mine = (b: Booking) => me.roles.includes('staff') || b.guestId === me.id;
      return [...store.bookings.values()].filter(mine);
    },

    async getBooking({ booking }) {
      return booking;
    },

    async payBooking({ id, transition }) {
      return move(store, id, transition);
    },

    async cancelBooking({ id, transition }) {
      return move(store, id, transition);
    },

    async checkIn({ id, transition }) {
      return move(store, id, transition);
    },

    async refundBooking({ id, transition }) {
      return move(store, id, transition);
    },
  };
}
