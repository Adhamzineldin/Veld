# bookings

A small hotel-booking project that shows **business rules** in the contract: invariants, endpoint
rules (`fact`, `require`, `ensure`, `now`) and `example` blocks. The guide
[`docs/guides/rules.md`](../../docs/guides/rules.md) explains the ideas; this folder is the working code.

```
contracts/bookings.veld     the contract: models, rules, errors, the service, examples
veld.toml                   two targets: api (Node + Express, with the test handler) and web (React)
server/                     hand-written Express app
  src/service.ts            the business logic, the loaders and the in-memory data
  src/main.ts               the real server
  src/test-server.ts        the TEST server (never for production)
  src/generated/            generated, committed  ->  package @veld/api
web/                        Vite + React 18 app, hand-written
  src/BookingForm.tsx       the form: live rule messages and typed errors
  test/client.ts            calls the running server through the generated client
  src/generated/            generated, committed  ->  package @veld/web
```

## What the contract says

- A `NewBooking` has two invariants: the stay ends after it starts, and it lasts at most 30 days.
  `guests >= 1` is a `@min(1)` constraint.
- `createBooking` loads the room (`fact room`), then requires: the room is open (else `RoomClosed`),
  the party fits (else `RoomFull` with the capacity), and the stay does not start in the past
  (`start >= now`). It promises that the booking it returns keeps the room and the guests of the request (`ensure`).
- Eight `example` blocks show each outcome. They run with `veld test`.

## What you write

`server/src/service.ts` has only business logic. It never checks what the contract already checks:
the dates, the party size, the room being open. The generated handler does it before this code runs.

```ts
async createBooking({ body, room }) {   // `room` is the fact the loader found
  const booking: Booking = { id: randomUUID(), roomId: body.roomId, guests: body.guests,
    start: body.start, end: body.end, total: price(room.nightly, body.start, body.end) };
  store.bookings.set(booking.id, booking);
  return booking;
}
```

The loader is one function per fact:

```ts
async room(key) { return store.rooms.get(key); }   // undefined: the server answers 404
```

## 1. Generate and install

Build the CLI once (`cargo build -p veld --release`, from the repository root) and put `target/release` on your `PATH`. Then, in this folder:

```bash
veld check                 # the contract, its rules and its examples
veld generate              # writes server/src/generated and web/src/generated
veld setup                 # wires @veld/api and @veld/web into the package.json files (already done)
(cd server && npm install)
(cd web && npm install)
```

## 2. Run the examples of the contract

```bash
veld test
```

This needs no server. Each `example` runs against the reference evaluator.

## 3. Run the server and the web app

```bash
(cd server && npm start)         # http://127.0.0.1:3000   (PORT overrides)
(cd web && npm run dev)          # http://localhost:5173, proxies /bookings to the server
```

Open the page. Choose a departure before the arrival: the message appears under the fields as you type,
and the button stays disabled. Choose three guests in the double room: the server answers `RoomFull` and the page shows its capacity.

`cd web && VELD_BOOKINGS_URL=http://127.0.0.1:3000 npm test` runs a script that calls the running
server through the generated client (success, `RoomFull`, `RoomClosed`, the local check and the 422 with `violations`).

## 4. Run the examples against a server

The test server mounts the generated **test handler**. It lets a caller choose the facts, the clock
and the principal of a request, so it must run only on your machine or in CI.

```bash
(cd server && npm run test-server)                               # http://127.0.0.1:3001
veld test --live http://127.0.0.1:3001 --dry-run                 # every check before the handler
veld test --live http://127.0.0.1:3001                           # also runs service.ts
```

`--dry-run` stops before your handler, so it checks the contract and the generated code. Without it,
an example that expects `ok` runs the real handler in `src/service.ts`.

`cargo test -p veld --test examples_e2e bookings` does all of this automatically.
