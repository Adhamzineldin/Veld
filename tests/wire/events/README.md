# Event envelope goldens

The bytes of an event message (`spec/events.md` sections 6 to 8). The Node, Java and Rust generators must each reproduce them, and each must decode the others'. The reference is `veld_rules::envelope` (`crates/veld-rules/src/envelope`); its tests `crates/veld-rules/tests/envelope.rs` (the first three files) and `crates/veld-rules/tests/dead_letter.rs` (the last two) run every file here.

| File | What it is |
|---|---|
| `envelopes.json` | The publish goldens: an array of cases. |
| `consume.json` | The consume goldens: what a consumer does with a message. |
| `dead-letter.json` | The dead-letter copy goldens (phase E2, section 10.5): the bytes of the copy. |
| `retry.json` | The retry goldens (phase E2, section 10.4): how often the handler runs, what is copied, what the transport is told. |
| `outbox.json` | The outbox goldens (phase E3, section 11.1): the entry of an emitted event, the relay and what the server checks after a handler. |
| `edges.veld` | The contract of the edge cases. The other contract is `tests/events/bookings.veld`. |

Every contract is compiled as the package `bookings`. The package is part of `source` (`veld:bookings/Bookings`) and of `type`, which is the declaration signature.

## `envelopes.json`

One case per message:

```json
{ "name": "…", "contract": "tests/events/bookings.veld",
  "service": "Bookings", "channel": "BookingEvents", "event": "BookingPaid",
  "id": "0b5c9c3e-…", "time": "2026-06-10T12:00:00Z",
  "value": { "bookingId": "…", "amount": "120.00", "paidAt": "…" },
  "canonical": "{\"data\":{…},\"datacontenttype\":…}" }
```

A generator's test compiles `contract`, builds a publisher for `service`, fixes the id source to `id` and the clock to `time`, publishes `value` as `event` on `channel` through a transport that keeps the bytes, and requires the bytes to equal `canonical` (UTF-8, compared as bytes, not as parsed JSON). `value` is the event in wire form, decimals and longs as strings; a typed publisher builds the same value from its typed event. A `uuid` or `datetime` in `value` may be written non-canonically (uppercase, an offset): the message has it canonical (lowercase, UTC with `Z`, `spec/events.md` section 6.3). Fields with a default may be left out of `value`: the message has them.

Then the opposite direction: the consumer of that event, given `canonical`, hands its handler the same event and the metadata `id`, `source`, `type`, `subject` and `time`.

What the cases cover: a default applied (`nights`), a default inside a nested value (`qty`), the key as `subject` for a `uuid`, a `string`, an `int` and a `long`, scalars written non-canonically on input, a channel without a key (no `subject`), an optional field absent and present, a `@wire` name, a decimal, a datetime with and without milliseconds (the fraction is written only when it is not zero), a non-ASCII string, an event without rules (no `veldrules`), and an event whose rules are all audited (no `veldrules`, and the audited rule, broken by the value, does not stop the message).

## `consume.json`

```json
{ "name": "…", "contract": "tests/events/bookings.veld",
  "handled": ["BookingPaid"], "message": "{…}", "outcome": "rule" }
```

`handled` lists the events the subscription has an `on` line for. A generator's test feeds `message` to the consumer and requires `outcome`:

| `outcome` | Meaning (section 8) |
|---|---|
| `malformed` | Step 1: rejected, reason `malformed` |
| `ignored` | Step 2: the type is not handled here; acknowledged, no handler, no hook |
| `shape` | Step 3: rejected, reason `shape` |
| `rule` | Step 4: rejected, reason `rule`, with the violations |
| `delivered` | The handler is called |
| `delivered, rules not checked` | The handler is called; `veldrules` differs from the consumer's own digest, so step 4 was skipped and `onContractMismatch` is called |
| `delivered, other type hash` | The handler is called; the hash of `type` differs from the consumer's own signature, so `onContractMismatch` is called |

Messages are written as ordinary JSON text, not in canonical form: a consumer must accept any JSON.

## `dead-letter.json`

```json
{ "name": "…", "original": "{…}", "reason": "shape", "group": "notifications", "attempts": 0,
  "issues": [ { "path": "$.amount", "code": "type", "message": "…" } ],
  "violations": [ { "rule": "veld:bookings/…@…", "path": "$", "fields": ["amount"], "message": "…", "error": "division_by_zero" } ],
  "copy": "{…}" }
```

A generator's test gives the dead-letter function of its consumer the bytes `original`, the `reason`, the `group` of the consumer, the number of `attempts` (runs of the handler, 0 for a rejected message) and the `issues` and `violations`, and requires the bytes `copy` (UTF-8, compared as bytes). `issues` are the `ValidationIssue` of the prelude (`path`, `code`, `message`); a violation is the object of `spec/rules.md` section 7 (`rule`, `path`, `fields`, `message`, and `error` only for an evaluation error). `veldissues` and `veldviolations` are written only when the list is not empty, each as a string that holds the canonical JSON of the list.

A case with `originalHex` and `copyHex` has bytes that are not UTF-8. What is not a JSON object is copied as it is, whatever it is. A member of the original named `veldreason`, `veldgroup`, `veldattempts`, `veldissues` or `veldviolations` is replaced, or removed when the copy has no list for it, so that a copy which fails again describes its own failure. Every other member is kept, and the object is written as canonical JSON.

The key of the copy is the key of the original message and is not part of the bytes.

What the cases cover: text that is not JSON, an array, a string, a number, `null`, bytes that are not UTF-8 and an empty message (all copied as they are); an object that is not an envelope; each reason; issues, violations, an evaluation error and both lists together; members the consumer does not know; whitespace and member order; text that is not ASCII and escaped text; quotes, backslashes and line breaks inside the list strings; a copy that fails again; numbers written as canonical JSON numbers; an empty object.

## `retry.json`

```json
{ "name": "…", "retry": { "max": 3, "backoff": "exponential" } | null, "deadLetter": true,
  "delayMs": 1000, "maxDelayMs": 60000, "transportAttempt": 1,
  "results": ["failed", "returned"],
  "expect": { "runs": 2, "failures": [ { "attempt": 1, "last": false } ], "waitsMs": [1000],
              "copy": null, "copyAttempts": 0, "answer": "done", "answerIfCopyFails": "done" } }
```

Section 10.4 as a table. `results[i]` is what run `i + 1` of the handler comes to (`returned` or `failed`); a run without an entry takes the last one, and an empty list is a handler that returns. A generator's test builds a consumer with the channel's `@retry` and `@deadLetter`, a `sleep` that records the wait it was asked for, the options `retryDelayMs` and `retryMaxDelayMs` and a handler that does `results`, delivers the message with the attempt `transportAttempt`, and requires: the number of handler `runs`; one entry of `failures` per failed run, with the `meta.attempt` of the run and `last` (the `final` of `onHandlerError`); the waits in `waitsMs`; the `copy` published to the dead-letter topic (its reason, or `null`) with the `copyAttempts` it carries; and the `answer` to the transport (`done` or `again`). `answerIfCopyFails` is the answer when publishing the copy fails (section 10.5).

A case with `rejected` (`malformed`, `shape` or `rule`) is a message that step 1, 3 or 4 rejects: no run, acknowledged, copied with that reason when the channel has `@deadLetter`. A case with `ignored` is a message of step 2: acknowledged and nothing else.

## `outbox.json`

Three arrays. The reference is `veld_rules::outbox` (`crates/veld-rules/src/outbox`), its test `crates/veld-rules/tests/outbox.rs` runs the file.

**`entries`**: what `emit.<event>(value)` hands to `outbox.save(topic, key | none, bytes)` (steps 1 to 5 of publishing, section 7). It is the cases of `envelopes.json`, with the same `service`, `channel`, `event`, `id`, `time` and `value`, and three more members: `topic` (the topic of the channel), `key` (the key text, or `null`: the `subject` of the message) and `canonical` (the bytes, the same as in `envelopes.json`). A generator's test fixes the hooks' id source to `id` and their clock to `time`, calls `emit` with `value`, and requires that its outbox received exactly `topic`, `key` and the bytes `canonical`, and that nothing reached a transport.

```json
{ "name": "…", "contract": "tests/events/bookings.veld", "service": "Bookings", "channel": "BookingEvents",
  "event": "BookingPaid", "id": "0b5c9c3e-…", "time": "2026-06-10T12:00:00Z", "value": { … },
  "topic": "shop.bookings.v1", "key": "5d2b3a0e-…", "canonical": "{…}" }
```

**`relay`**: the relay as a table. A case saves `saves` entries into a fresh memory outbox (named by the order of their save, `"1"`, `"2"`, …; a generated `MemoryOutbox` has ids of its own and the test maps them) and then makes the `runs` in order against that outbox. A run is `limit`, a `script` and what it must `expect`. `script[i]` is what happens to the `i`-th entry the run looks at; an entry past the end of the script is accepted:

| Step | The transport | `done` | The run |
|---|---|---|---|
| `"accepted"` | accepts the message | succeeds | goes on |
| `{ "publishFails": "text" }` | fails with `text` | not called | stops; the failure goes to `onError` |
| `{ "doneFails": "text" }` | accepts the message | fails with `text` | stops; the failure goes to `onError`; the entry is pending again and will be sent again |
| `"crashAfterPublish"` | accepts the message | not called: the process ends | stops; nothing is reported; the entry will be sent again |

`expect` has `published` (the entries the transport accepted, in order), `done` (the entries `done` succeeded for), `sent` (what the relay answers: how many entries it sent, the length of `published`) and `failure` (`{ "entry", "error" }` or `null`). `pendingAfter` is what `pending(…)` lists after the run. The relay looks at the first `limit` entries `pending` gives it, in that order.

**`written`**: what the server checks after the handler of an endpoint whose move emits (section 11.1). `expected` are the events the edge taken emits, `written` the events the handler wrote in order, both by name, and `result` is `ok`, or `missing E`, `twice E` or `unexpected E`: a success is then answered `500 Internal` and goes to `onError`. A handler that answered an error is not checked.

The contract `tests/events/outbox.veld.pending` has the moves of the cases in the `written` array (a state machine whose edges emit, one endpoint per move) and the call examples with `expect emitted`; the examples are run live against a generated test handler (section 11.1, `Veld-Test-Emitted`) and offline they are reported as live only.

## Rules

- A golden changes only with the contract's signatures or with the spec. Never edit the `canonical` text by hand to make a generator pass; compute it again with the reference encoder and read the diff.
- A new edge case goes into the file with its contract; the reference test and every generator's test pick it up without code changes.
