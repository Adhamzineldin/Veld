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
