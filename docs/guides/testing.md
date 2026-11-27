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

An example writes down only part of an answer: `-> RoomFull { capacity: 2 }` says nothing about the violations. So when the answer agrees with the example, `veld test` also asks the reference evaluator what the whole error must be, and the server has to give exactly that: the same type, title, status, operation and fields, sent as `application/problem+json`, and the same violations in the same order, each with the same rule id, path, fields, message and error. (When a parameter contains a map, the violations are compared without their order, because the order of map entries is not defined.) Every answer must also carry the service's rules digest in `Veld-Rules`. A server that answers the right error with the wrong violation fails the example, and the report says what the evaluator expected.

An example without `returns` runs the application's real handler, so `-> ok { ... }` checks what it answered. With `--dry-run` such an example stops before the handler: the test handler answers `204` with `Veld-Test-Passed: 1` when every check passed. That makes a live run check exactly what an offline run checks.

Before the first example, `veld test` asks `GET /__veld/test`. A test handler answers that it is one and names the services it serves; a server that does not is refused, so a production server is never sent test headers. Examples and fuzz requests of a service the handler does not name are reported as skipped, not sent: in a project with several services, run `veld test --live` once per test handler.

**The test handler must never be mounted in production.** It lets any caller choose the facts, the principal and the clock of a request, which switches authorization off. It is generated only for a target with `testing = true`, in a file of its own, and says so at its top. Run it in CI or on a developer machine, never on a server that real users can reach.

## `veld test --live --fuzz`

```bash
veld test --live http://localhost:3000 --fuzz 200
veld test --live http://localhost:3000 --fuzz 200 --seed 1234567
```

Examples cover the cases you thought of. `--fuzz N` covers the ones you did not: after the examples, it sends `N` generated requests to **every endpoint**, valid and invalid, to the test handler in dry-run mode (whatever `--dry-run` says). For each request the reference evaluator works out what the answer must be; any difference in the status, the error type, the error's fields or the **violations** (in order: rule id, path, fields, message and error) fails the run with exit code 1. It stops at the first difference per endpoint.

The first requests of a run are the **boundaries**. Every comparison in a rule of the endpoint (its `require` lines and the rules of the models its parameters contain) with something the generator can write on one side (a field, `len(field)`, or the difference of two datetimes) is tried with the field exactly at the other side's value and one smallest step on each side: `guests` at `room.capacity` and one more, a stay of exactly `days(30)` and one millisecond longer, a coupon at `50` and `51`. The other side can be a literal, `now` or another field. Then come purely random requests. A request is not a rule violation every time: the generator runs the reference evaluator and, for each rule it reports as violated, sets the field of the comparison to what the rule asks for (and the clock sometimes lands next to a datetime of the request), so most rules are also reached with a request that passes them.

What the run prints:

```text
fuzz: 200 requests per endpoint, seed 1234567 (repeat with --seed 1234567)
fuzz Bookings.createBooking: 200 sent (24 boundary): NotFound 12, RoomClosed 9, RoomFull 18, ValidationError 61, ok 100
skip Bookings.upload (a string with @pattern or @format and no @example)
```

The count per outcome tells you whether the fuzz reached your rules. An endpoint is **skipped**, with the reason, when it takes a string with `@pattern` or `@format` and no `@example`, in a parameter, a fact or the principal's model (the generator cannot make a value for it; give the field an `@example` and the endpoint is fuzzed), when it is a stream, and when its service has no endpoint rules, because a server target generates a test handler only for a service that has some. An optional field with `@pattern` and no `@example` is simply left out.

Fuzzing covers every endpoint of the services the test handler serves: `--filter` and the file arguments choose the examples that run first, not the endpoints that are fuzzed. Requests are made from the parameters and the facts. `N` is from 1 to 100000.

**The principal.** When the package declares a `principal`, every endpoint with auth gets a generated caller of the principal's model, sent in `Veld-Test-Principal` and printed as `as { ... }` in the example block of a failing request. A random caller is denied by almost every `allow`, so the generator repairs a request the reference evaluator denies: it makes one `permit` clause of the policy true, chosen at random so that every `permit` is reached (for a comparison of two paths it copies one side into the other, for `x in list` it puts `x` into the list), and it makes a `forbid` that holds false. Some requests are left unrepaired on purpose, so the run sees permits, denials by "not permitted" and denials by a `forbid` with its own message. A clause the generator cannot write (a call such as `matches(...)`, or an argument of the `allow` that is not a plain path) is left to chance.

**State machines.** For an endpoint with a `transition`, the first requests include one boundary request for every state of the machine: each source state, which must get past the guard, and each state that is not a source, which must answer `InvalidTransition`. The rest of the run repairs an illegal state by writing one of the sources.

**The order of the checks.** A request often fails more than one check (a denied caller on a booking in an illegal state, an illegal state with a wrong amount). The generator leaves a failing request unrepaired one time in eight at each check, so such requests are common, and the server must answer with the check that comes first in `spec/rules.md` section 9.5: invalid parameters, then a missing fact, then a denied `allow`, then the transition guard, then a failed `require`.

Two things a green run does not hide:

- If every request of an endpoint ended in the same error, the run says so (`warn Bookings.x: every request ended in ValidationError …`). The generator could not get past that check, so the rules after it were not exercised. The usual cause is a rule the generator cannot satisfy by itself, such as `100 / n > 5`; an `@example` on the field helps.
- If the server stops answering in the middle, the run fails, and whatever it found before is still printed.

A disagreement prints the seed, the endpoint, what the reference evaluator expects, what the server answered, and the request as an `example` block you can paste into the contract (it passes offline, and fails live until the server is fixed):

```text
FAIL Bookings.createBooking (seed 1234567, boundary request 1)
     expected: RoomFull 409 {"capacity":372} violations [require: precondition failed]
     actual:   HTTP 204 (passed)
     the request as an example (paste it into a contract; offline it passes):
       example "fuzz seed 1234567 Bookings.createBooking request 1" { ... }
```

Without `--seed`, the seed comes from the clock and is printed first, so any run can be repeated: the same seed and the same contract give the same requests. `--fuzz` needs `--live`, and `--seed` needs `--fuzz`.

## `veld test --coverage`

```bash
veld test --coverage
veld test --coverage --fail-under 80
```

A rule that no example has seen fail is not tested. After the examples, `--coverage` counts every rule of the workspace: each invariant of a declaration a service reaches, each `require`, `ensure`, `allow` line and `transition` line. A rule is **covered** when one example made it hold and another made it fail (for an `allow`, was permitted and was denied; for a `transition`, was a legal move and an illegal one). An audited rule counts like an enforced one. The report says how many are covered and lists the others, with what is missing:

```text
coverage: 5 of 12 rules held and violated (41%)
  demo/NewStay.atMost30Days                             never violated
  demo/Closure.reasonGiven                              never evaluated
  demo/Bookings.payBooking allow ManageBooking          never denied
  demo/Bookings.payBooking transition pending -> paid   never illegal
  demo/Bookings.payBooking ensure paidTotal             never evaluated
```

`--fail-under N` (0 to 100) makes the run exit 1 when the percentage is below `N`. Three things to know:

- An `ensure` is evaluated only by an example with `returns`; an endpoint whose examples never say what the handler returns leaves its `ensure` lines "never evaluated".
- `-p` narrows the rules to the selected packages; `--filter` and file arguments narrow only the examples, so the percentage drops.
- The numbers come from the reference evaluator, also with `--live`. The fuzzer does not count: coverage is about what your own examples show.

## `veld test --from-violation`

A server can be told to put a **capture** (the request it was handling: parameters, facts, principal, clock) in the event it reports to `onRuleViolation`. `veld test --from-violation event.json` turns such an event into an `example` block, the outcome included, to paste into your contract:

```bash
veld test --from-violation event.json >> contracts/bookings.veld
```

Standard output holds the block only. Notes go to standard error: the values the server left out of the capture because they are `@pii` or `@secret` are filled in as examples are, each one named, and when the example does not end in the violation of the event (for instance because a filled-in value mattered, or because the rule is one of the server's own output, which needs a `returns` line) the tool says so. An event without a capture, an endpoint the contract does not have and a capture that does not fit the contract's types each exit with code 1. The JSON of the event is in [`spec/rules.md`](../../spec/rules.md) section 17.3. A capture contains whatever the caller sent: handle the file like a request log.
