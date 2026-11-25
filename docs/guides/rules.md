# Business rules in the contract

A rule is a statement about your data that must always be true. In Veld the contract states it, and the generated code enforces it on the server and checks it early in the client. You write it once.

This guide uses the [`examples/bookings`](../../examples/bookings) project. It is a small hotel-booking service, and everything shown here runs. The full language is in [`spec/rules.md`](../../spec/rules.md).

## Why a rule lives in the contract

Without rules, the same check is written three times: in the server handler, in the web form, and in the mobile app. The three copies drift apart. A rule in the contract has one source. The server cannot forget it, and the client uses the same one.

Rules are plain boolean expressions. They have no side effects and always finish, so every generator can enforce them the same way.

## Invariants: rules about one value

An invariant is a rule inside a model. It must hold for every value of that model.

```veld
model NewBooking {
  roomId: uuid
  guests: int @min(1)
  start: datetime
  end: datetime

  rule endAfterStart: end > start
    message "The stay must end after it starts"
  rule atMost30Days: end - start <= days(30)
    message "A stay is at most 30 days"
}
```

`@min(1)` is a constraint on one field. A `rule` can compare several fields. The `message` is what a person sees; without it the message is `violates rule <name>`.

### What happens when one is violated

**On the server.** The generated handler checks the request body before your code runs. A body that breaks a rule gets `422 ValidationError`. The problem document lists the issues as before, and adds a `violations` list:

```json
{ "title": "ValidationError", "status": 422,
  "data": { "issues": [ { "path": "body", "code": "rule", "message": "The stay must end after it starts" } ] },
  "violations": [ { "rule": "veld:bookings/NewBooking.endAfterStart@ada6a4114a0970df",
                    "path": "body", "fields": ["end", "start"],
                    "message": "The stay must end after it starts" } ] }
```

`fields` names the fields the rule reads. A client can show the message next to them.

**In the client.** Before it sends a request, the generated client runs the same rules on the body. A request that breaks one is **not sent**. The call throws a `ValidationError` with `local = true` and the `violations`:

```ts
try {
  await client.createBooking({ body });
} catch (e) {
  if (ValidationError.matches(e)) {
    console.log(e.local);       // true: nothing was sent
    console.log(e.violations);  // [{ rule, path, fields, message }]
  }
}
```

A violation that comes back from the server has the same `violations` and `local = false`. Turn the local check off with `new BookingsClient({ rules: false })`.

You can also check a value yourself. Every model that has rules has `NewBooking.check(value)`, which returns the violations, and `NewBooking.partial(draft)`, which skips a rule that needs a field the draft does not have yet.

**In a form.** `useFormRules` runs `partial` on what the user has typed and groups the messages by field:

```tsx
import { NewBooking } from '@veld/web';
import { useFormRules } from '@veld/web/bookings/react';

const rules = useFormRules(NewBooking, draft);   // draft: what has been typed so far
rules.messages.end;     // ['The stay must end after it starts'] while the dates are wrong
rules.valid;            // true when the draft has no violations yet
```

An incomplete draft is not invalid: until the user picks a departure, `endAfterStart` has nothing to compare and is skipped. Keep the draft in state as a new object on every change. See `examples/bookings/web/src/BookingForm.tsx`.

## Endpoint rules: `fact`, `require`, `ensure`

Some rules need more than the request. Booking a room depends on the room. An endpoint block can load that data and state its rules:

```veld
post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
  fact room: Room = body.roomId
  allow MayBook(user)
  require not room.closed else RoomClosed
  require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
  require notInPast: body.start >= now
    message "A booking cannot start in the past"
  ensure keepsRoom: result.roomId == body.roomId
  ensure keepsGuests: result.guests == body.guests
  ensure belongsToCaller: result.guestId == user.id
  ensure startsPending: result.status == .pending
}
```

(`allow` says who may call. It comes [below](#policies-who-may-call).)

- `fact room: Room = body.roomId` says: load a `Room` using this key. If there is none, the answer is `404 NotFound`.
- `require ... else Error` is a precondition. If it fails, the server answers the declared error. `RoomFull { capacity: room.capacity }` fills the error's data from the fact. The first failing `require` wins, in the order written.
- `require` without `else` answers `422 ValidationError` with a violation, like an invariant.
- `now` is the time of the request, read once, so a rule that uses it is testable.
- `ensure` is a promise about what your handler returns. If your code breaks it, that is a bug on the server, so the caller gets `500 Internal` with no detail. In the example, your handler may not change the number of guests, and the booking it returns must belong to the caller and start `pending`. `old(x)` in an `ensure` is the value `x` had before the handler ran.

### What you write after generating

Two things, and no checks.

**A loader per fact.** The generated `BookingsFactLoaders` has one method per fact (`room` and `booking`, the second for the sections below):

```ts
export function createLoaders(store: Store): BookingsFactLoaders {
  return {
    async room(key) {
      return store.rooms.get(key);   // undefined or null: the server answers 404
    },
    async booking(key) {
      return store.bookings.get(key);
    },
  };
}
```

**The handler.** It receives the request and the loaded facts, already checked:

```ts
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
```

and the server mounts it with the loaders (and the auth hook, shown [below](#policies-who-may-call)):

```ts
createBookingsHandler(createService(store), {
  facts: createLoaders(store),
  auth: { bearer: authenticate },
})
```

This handler does not test the dates, the party size or whether the room is closed. The contract already did. If you find yourself writing such a check here, move it into the contract.

## State machines: which moves are legal

A cancelled booking must not become paid. Say which moves exist on the enum, and which move an endpoint makes:

```veld
enum Status {
  pending,
  paid,
  checkedIn,
  cancelled,
  refunded,

  flow pending -> paid -> checkedIn
  flow pending -> cancelled
  flow paid -> cancelled
  flow paid -> refunded
  final checkedIn, cancelled, refunded
}

/// Pays for a booking. The amount must be the price.
post "/{id}/pay" payBooking(id: uuid, body: Payment) -> 200 Booking | AmountMismatch {
  fact booking: Booking = id
  allow ManageBooking(user, booking)
  transition booking.status: pending -> paid
  require body.amount == booking.total else AmountMismatch { expected: booking.total }
}

/// Cancels a booking that has not been used.
post "/{id}/cancel" cancelBooking(id: uuid) -> 200 Booking {
  fact booking: Booking = id
  allow ManageBooking(user, booking)
  transition booking.status: pending, paid -> cancelled
}
```

- `flow` lists the legal moves. A `final` state has no way out. `veld check` warns about a state you can get stuck in.
- `transition booking.status: pending -> paid` says this endpoint moves the booking from `pending` to `paid`. A list of sources (`pending, paid -> cancelled`) means any of them. If the loaded booking is in any other state, the server answers `409 InvalidTransition` with `field`, `from` and `to`, before your handler runs.
- When the endpoint returns the booking, the server also checks that your handler really moved it to `paid`.
- The check comes after the policies and before every `require`. A cancelled booking that is paid with the wrong amount is `InvalidTransition`, not `AmountMismatch`.

The examples state this. These two are in `examples/bookings/contracts/bookings.veld`:

```veld
example "a paid booking cannot be paid again" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .paid, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 179.00 })
    -> InvalidTransition { field: "status", from: "paid", to: "paid" }
}

example "the transition is checked before the amount" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .cancelled, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 1.00 }) -> InvalidTransition { from: "cancelled" }
}
```

**This is a guard, not a lock.** Two requests can pass the check at the same moment. So the handler receives the move as data, and writes only if the booking is still where the check found it. This is `move` in `examples/bookings/server/src/service.ts`; the store is a `Map`, so the compare and the write cannot be separated here, but the shape is the one a database needs (`UPDATE ... WHERE id = $id AND status = $from`):

```ts
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

// the handler:
async payBooking({ id, transition }) {
  return move(store, id, transition);
},
```

`transition` is `{ field: 'status', from: 'pending', to: 'paid' }`, typed with the enum. The handler does not check the amount or who the caller is: the contract did.

Clients get the same machine, to show only the buttons the server would accept:

```ts
Status.nextStates('pending');   // ['paid', 'cancelled']
Status.isFinal('refunded');     // true
```

## Policies: who may call

A policy says who may do something. It is written once and used by every endpoint that needs it:

```veld
model User {
  id: uuid
  name: string
  roles: string[]
  suspended: bool
}

principal User

policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "staff" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}

@auth(bearer)
service Bookings at "/bookings" {
  @auth(none)
  get "/rooms" listRooms() -> Room[]

  get "/{id}" getBooking(id: uuid) -> Booking {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
  }
  // ...
}
```

- `principal User` says what a caller is. In an endpoint block the caller is `user`.
- A policy permits when at least one `permit` is true and no `forbid` is. A `forbid` always wins.
- `allow` runs after the request has been validated and the facts are loaded, and before the state check and every `require`. A denial is `403 Forbidden`, with the message of the `forbid` that was true, or `not permitted`.
- A record that does not exist is `404` before the policy is asked.
- The service says `@auth(bearer)`, so every endpoint needs a token. `listRooms` is public, and says so with `@auth(none)`.

The examples show the three kinds of answer, and the order:

```veld
example "another guest does not see it" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa" }
  as { id: "00000000-0000-4000-8000-0000000000bb", roles: [], suspended: false }
  getBooking(id: booking.id) -> Forbidden { message: "not permitted" }
}

example "forbid beats permit: suspended staff is refused too" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa" }
  as { id: "00000000-0000-4000-8000-0000000000cc", roles: ["staff"], suspended: true }
  getBooking(id: booking.id) -> Forbidden { message: "A suspended account cannot manage bookings" }
}

example "the policy is asked before the transition" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .cancelled, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000bb", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 179.00 }) -> Forbidden
}
```

Your auth hook returns the caller, with the generated type (`Promise<User | null>`; `null` is a `401`). In the example it is a lookup in a table of demo tokens:

```ts
export async function authenticate(token: string): Promise<User | null> {
  return USERS[token] ?? null;
}

// in main.ts
createBookingsHandler(createService(store), {
  facts: createLoaders(store),
  auth: { bearer: authenticate },
})
```

A handler that needs to know the caller reads `ctx.principal`. It is typed `unknown`, so the example narrows it once:

```ts
function caller(ctx: veld.RequestContext): User {
  return ctx.principal as User;
}
```

The client gets the policy too. `canPayBooking` answers whether the server would let this call past its policies and its state check, so the app can hide a button the server would refuse. It never sends anything, and the server still decides. The booking list of the web app (`examples/bookings/web/src/BookingList.tsx`) shows a button only when the state machine has the move and the helper agrees:

```tsx
{Status.nextStates(b.status).map((next) => {
  const move = moves[next];
  if (move === undefined || !move.can(b)) return null;
  return (
    <button key={next} type="button" onClick={() => void act(b, next)}>
      {move.label}
    </button>
  );
})}
```

with, for example, `can: (b) => BookingsClient.canPayBooking(b, me)`. When a call is refused anyway (the booking moved in another window), the page shows the server's own words: the `message` of a `Forbidden`, the two states of an `InvalidTransition` (see `web/src/describe.ts`).

Say who may call every endpoint. `veld check` warns about an endpoint with no `@auth` at all; write `@auth(none)` when it is meant to be public.

## Examples and `veld test`

An `example` states what one call does:

```veld
example "a room that sleeps two refuses three guests" {
  given room = { capacity: 2, closed: false }
  at "2026-06-01T00:00:00Z"
  createBooking(body: {
    roomId: room.id, guests: 3,
    start: "2026-06-10T00:00:00Z", end: "2026-06-12T00:00:00Z"
  }) -> RoomFull { capacity: 2 }
}
```

The example does not say who calls, so the principal is filled in too. The examples of the state machine and the policies above name the caller with `as { ... }`.

`veld test` runs every example without a server. It is also documentation that cannot go out of date. Run `veld test --live <url>` against the test server of your project to run them against your real handler. The test server, `--dry-run`, and what an example may leave out are in [testing with examples](testing.md). The test handler is generated only with `testing = true` on the server target, and it must never run in production.

## How strict the server is about its own answers

The server also checks what **it** sends: the invariants of the returned value and the `ensure` lines. The default, `strict`, answers `500 Internal` with no detail when your code returns something that breaks a rule. You can choose another mode when you create the handler:

```ts
createBookingsHandler(service, {
  facts: loaders,
  rules: { output: 'audit' },      // 'strict' (default), 'audit' or 'off'
  onRuleViolation: (event) => log.warn(event.rule, event.outcome, event.position),
});
```

| Mode | A broken rule in your own answer |
|---|---|
| `strict` | The caller gets `500 Internal`. The hook is called. |
| `audit` | The answer is sent unchanged. The hook is called. Use it to find violations in existing data before you enforce. |
| `off` | Not checked. |

The hook `onRuleViolation` also runs for rejected requests (`position: 'input'`). It is for logs and metrics. An exception inside it never changes the response.

## What belongs in the contract

Put a rule in the contract when it is about the data or the call and can be said with the fields you have: ranges, relations between fields, "this must exist and be open", a limit that depends on a loaded record, a promise about the result.

Keep it in code when it needs something a rule cannot do: a database query beyond loading one record by key, a call to another service, a random or time-dependent decision beyond `now`, a state that changes between requests (a nightly rate that depends on bookings made earlier). A rule never has side effects.

## When the client and the server disagree

Your server may change its rules after a client was built. The server sends the digest of its rules in a `Veld-Rules` header. When it differs from the one the client was generated with, the client stops its local checks (the server still checks) and calls `onContractMismatch` once:

```ts
new BookingsClient({
  baseUrl,
  onContractMismatch: ({ service }) => console.warn(`The ${service} rules changed. Rebuild the app.`),
});
```

A browser can read this header from another origin only if the server sends `Access-Control-Expose-Headers: Veld-Rules`. Without it the client still works, but it never rejects a request locally.

## What is not supported yet

- **Optimistic concurrency and idempotency** (`@version`, `@idempotent`), **rolling a rule out** (`@rollout(audit)`), **derived fields** and **`veld check --prove`**: they are designed in [`spec/rules.md`](../../spec/rules.md) (sections 11.7, 11.8, 17 and 18) and not implemented. Do not use them in a contract today.
- **Events** (messaging with Kafka and RabbitMQ): designed in [`spec/events.md`](../../spec/events.md), not implemented.
- **Generators**: everything in this guide is enforced by the TypeScript client and the Node server, by Java (client, JDK server and Spring) and by Rust (client and axum server). The code in this guide is TypeScript; the Java and Rust shapes are in their target pages below. A generator that cannot enforce a rule, a plugin for example, stops with error `E0801`. It never generates code that ignores a rule.

## Where to go next

- [`examples/bookings`](../../examples/bookings): the project from this guide, with its README.
- [Testing with examples](testing.md).
- [`spec/rules.md`](../../spec/rules.md): the language of rules and the wire format.
- [`spec/targets/typescript.md`](../../spec/targets/typescript.md): the generated check functions, the client and server steps.
- [`spec/targets/java.md`](../../spec/targets/java.md) and [`spec/targets/rust.md`](../../spec/targets/rust.md): the same for Java and Rust.
