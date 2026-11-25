# Testing with examples

An `example` states what one call does. It is documentation that runs: `veld test` checks it.

```veld
example "a full room is rejected" {
  given room = { capacity: 2, closed: false }
  createBooking(body: { roomId: room.id, guests: 3 }) -> RoomFull { capacity: 2 }
}
```

- `given name = value` sets a **fact** the endpoint loads (`given name = absent` makes the loader find nothing).
- The call names an endpoint and its arguments. `at "..."` sets the clock, `as value` the principal.
- `-> ok` or `-> ErrorName` is the outcome. A value after it is a **pattern**: every field it names must match, fields it does not name are ignored.
- `returns value` states what the handler returns, so the rules about the result (`ensure`) can run.

The full language is in [`spec/rules.md`](../../spec/rules.md) section 12.

## What an example may leave out

Everything you do not write is filled in, the same way every time: a string is `a` (or as long as `@length` needs), a number is `0` moved inside `@min` and `@max`, a `bool` is `false`, ids count up from `00000000-0000-4000-8000-000000000001`, datetimes start at the example's clock, an optional field stays absent, an enum is its first variant. A field with `@example` or a default takes that value. Section 12.2 of the spec has the whole table.

A filled-in value must be valid. If it breaks an invariant, or cannot be filled in at all (a `@pattern` field with no `@example`), the example is wrong, and `veld check` reports `E0953` at the example, naming the fields to write.

## `veld test`

```bash
veld test                      # every example, offline
veld test --filter "full room" # examples whose name or Service.endpoint contains the text
veld test contracts/rooms.veld # only the examples written in these files
```

Offline, each example runs against the reference evaluator: no server, no network. One line per example (`ok`, `skip` with the reason, or `FAIL` with the expected and the actual outcome), then a summary. The exit code is 1 when an example failed. An example that expects `ok { ... }` but has no `returns` cannot be checked offline and is skipped.

## `veld test --live`

```bash
veld test --live http://localhost:3000
veld test --live http://localhost:3000 --dry-run
```

`--live` sends each example as a real HTTP request to a running **test handler** and compares the status, the error type and the patterns. The request carries the example's facts, principal and clock in the headers `Veld-Test-Facts`, `Veld-Test-Principal` and `Veld-Test-Now` (and the handler's result in `Veld-Test-Result`), so the server needs no data of its own.

An example without `returns` runs the application's real handler, so `-> ok { ... }` checks what it answered. With `--dry-run` such an example stops before the handler: the test handler answers `204` with `Veld-Test-Passed: 1` when every check passed. That makes a live run check exactly what an offline run checks.

Before the first example, `veld test` asks `GET /__veld/test`. A server that does not answer as a test handler is refused.

**The test handler must never be mounted in production.** It lets any caller choose the facts, the principal and the clock of a request, which switches authorization off. It is generated only for a target with `testing = true`, in a file of its own, and says so at its top. Run it in CI or on a developer machine, never on a server that real users can reach.

## `veld test --live --fuzz`

```bash
veld test --live http://localhost:3000 --fuzz 200
veld test --live http://localhost:3000 --fuzz 200 --seed 1234567
```

Examples cover the cases you thought of. `--fuzz N` covers the ones you did not: after the examples, it sends `N` generated requests to **every endpoint**, valid and invalid, to the test handler in dry-run mode (whatever `--dry-run` says). For each request the reference evaluator works out what the answer must be; any difference in the status, the error type, the error's fields or the **set of violations** (rule ids, paths and fields) fails the run with exit code 1. It stops at the first difference per endpoint.

The first requests of a run are the **boundaries**. Every comparison in a rule of the endpoint (its `require` lines and the rules of the models its parameters contain) with something the generator can write on one side (a field, `len(field)`, or the difference of two datetimes) is tried with the field exactly at the other side's value and one smallest step on each side: `guests` at `room.capacity` and one more, a stay of exactly `days(30)` and one millisecond longer, a coupon at `50` and `51`. The other side can be a literal, `now` or another field. Then come purely random requests. A request is not a rule violation every time: the generator runs the reference evaluator and, for each rule it reports as violated, sets the field of the comparison to what the rule asks for (and the clock sometimes lands next to a datetime of the request), so most rules are also reached with a request that passes them.

What the run prints:

```text
fuzz: 200 requests per endpoint, seed 1234567 (repeat with --seed 1234567)
fuzz Bookings.createBooking: 200 sent (24 boundary): NotFound 12, RoomClosed 9, RoomFull 18, ValidationError 61, ok 100
skip Bookings.upload (a string with @pattern or @format and no @example)
```

The count per outcome tells you whether the fuzz reached your rules. An endpoint is **skipped**, with the reason, when it takes a string with `@pattern` or `@format` and no `@example`: the generator cannot make a value for it. Give the field an `@example`, and the endpoint is fuzzed.

A disagreement prints the seed, the endpoint, what the reference evaluator expects, what the server answered, and the request as an `example` block you can paste into the contract (it passes offline, and fails live until the server is fixed):

```text
FAIL Bookings.createBooking (seed 1234567, boundary request 1)
     expected: RoomFull 409 {"capacity":372} violations [require: precondition failed]
     actual:   HTTP 204 (passed)
     the request as an example (paste it into a contract; offline it passes):
       example "fuzz seed 1234567 Bookings.createBooking request 1" { ... }
```

Without `--seed`, the seed comes from the clock and is printed first, so any run can be repeated: the same seed and the same contract give the same requests. `--fuzz` needs `--live`, and `--seed` needs `--fuzz`.
