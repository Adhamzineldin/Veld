# bookings

A small hotel-booking project that shows **business rules** in the contract: invariants, endpoint
rules (`fact`, `require`, `ensure`, `now`), a **state machine** for the life of a booking, **policies**
for who may do what, and `example` blocks that `veld test` checks. The guide
[`docs/guides/rules.md`](../../docs/guides/rules.md) explains the ideas; this folder is the working code.

```
contracts/bookings.veld     the contract: models, rules, status machine, policies, the service, examples
veld.toml                   two targets: api (Node + Express, with the test handler) and web (React)
server/                     hand-written Express app
  src/service.ts            the business logic, the auth hook, the loaders and the in-memory data
  src/main.ts               the real server
  src/test-server.ts        the TEST server (never for production)
  src/generated/            generated, committed  ->  package @veld/api
web/                        Vite + React 18 app, hand-written
  src/App.tsx               sign in as a demo account
  src/BookingForm.tsx       the form: live rule messages and typed errors
  src/BookingList.tsx       the bookings, with only the moves the server would accept
  test/client.ts            calls the running server through the generated client
  src/generated/            generated, committed  ->  package @veld/web
```

## What the contract says

**Rules about the data and the call**

- A `NewBooking` has two invariants: the stay ends after it starts, and it lasts at most 30 days.
  `guests >= 1` is a `@min(1)` constraint.
- `createBooking` loads the room (`fact room`), then requires: the room is open (else `RoomClosed`),
  the party fits (else `RoomFull` with the capacity), and the stay does not start in the past
  (`start >= now`). It promises that the booking it returns keeps the room and the guests of the request,
  belongs to the caller and starts `pending` (`ensure`).

**A life cycle: the `Status` state machine**

```
pending -> paid -> checkedIn
pending -> cancelled
paid    -> cancelled
paid    -> refunded          (checkedIn, cancelled and refunded are final)
```

- `payBooking`, `cancelBooking`, `checkIn` and `refundBooking` each say which move they make
  (`transition booking.status: pending -> paid`). The server refuses any other state with
  `409 InvalidTransition` and `{ field, from, to }`, before your code runs.
- The handler receives the move as data (`transition`) and writes only if the booking is still in the
  state the check saw. That is `move` in `server/src/service.ts`.

**Who may call: the principal and the policies**

- `principal User` says who a caller is. The auth hook turns a bearer token into a `User`.
- `ManageBooking` lets the guest who made a booking and the staff manage it; a suspended account never.
  `MayBook` refuses suspended accounts. `StaffOnly` guards the refund.
- The service is `@auth(bearer)`. Only `listRooms` says `@auth(none)`: anyone may look at the rooms.
- The order of the checks is fixed: the booking must exist (404), then the policies (403), then the
  transition (409), then each `require` (the declared error). The examples pin it down.

**Examples.** 38 `example` blocks show each outcome: a legal and an illegal move, a permitted and a denied
caller, the order of the checks, and the target state. They run with `veld test`.

## What you write

`server/src/service.ts` has only business logic. It never checks what the contract already checks:
the dates, the party size, who the caller is, whether they may touch the booking, whether the move is legal.
The generated handler does it before this code runs.

```ts
async payBooking({ id, transition }) {   // `transition` is { field: 'status', from: 'pending', to: 'paid' }
  return move(store, id, transition);
}
```

`move` is the conditional write. It compares the stored state with `transition.from` before it writes,
and answers `InvalidTransition` itself when another request moved the booking first.

The auth hook and the loaders are small functions that the server passes to the generated handler:

```ts
auth: { bearer: authenticate },          // a token to a typed User, or null for a 401
facts: createLoaders(store),             // one function per fact: room, booking
```

Demo tokens: `token-ann` and `token-bob` (guests), `token-sam` (staff), `token-eve` (a suspended guest).

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

Open the page and sign in as Ann. Book a room, then pay: the buttons next to a booking are only the
moves the server would accept (`Status.nextStates` and `BookingsClient.canPayBooking`). Ann sees no
Refund button, and after she pays there is no Pay button. Sign in as Sam (staff) to see every booking
and refund a paid one. Sign in as Eve: the Book button is off, because the server would refuse her.
Choose a departure before the arrival: the message appears under the fields as you type.
Choose three guests in the double room: the server answers `RoomFull` and the page shows its capacity.

If the server refuses a call anyway (the booking moved in another window, say), the page shows the
server's own message. Open two tabs as Ann. Cancel a booking in one tab, then press Pay on the stale
list in the other: the page shows `A booking cannot go from cancelled to paid.`

`cd web && VELD_BOOKINGS_URL=http://127.0.0.1:3000 npm test` runs a script that calls the running
server through the generated client (every rule above, the `can...` helpers and the local check).

## 4. Run the examples against a server

The test server mounts the generated **test handler**. It lets a caller choose the facts, the clock
and the principal of a request, so it must run only on your machine or in CI.

```bash
(cd server && npm run test-server)                               # http://127.0.0.1:3001
veld test --live http://127.0.0.1:3001 --dry-run                 # every check before the handler
veld test --live http://127.0.0.1:3001                           # also runs service.ts
veld test --live http://127.0.0.1:3001 --fuzz 100 --seed 1       # generated requests, compared with the evaluator
```

`--dry-run` stops before your handler, so it checks the contract and the generated code. Without it,
an example that expects `ok` runs the real handler in `src/service.ts`.

`src/test-server.ts` has one extra: the test handler takes its facts from headers and never calls your
loaders, so the booking an example describes is not in the in-memory store. It puts that booking into the
store before the four handlers that move a booking run.

`cargo test -p veld --test examples_e2e bookings` does all of this automatically.

## A note on `veld fmt`

`veld fmt` puts every long call of an example on one line and every enum variant on its own line.
The contract keeps the examples readable (a call with a body over three lines), so `veld fmt --check`
reports this file. The enum and the error added for the state machine follow the formatter.
