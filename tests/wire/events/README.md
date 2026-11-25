# Event envelope goldens

The bytes of an event message (`spec/events.md` sections 6 to 8). The Node, Java and Rust generators must each reproduce them, and each must decode the others'. The reference is `veld_rules::envelope` (`crates/veld-rules/src/envelope`); its test `crates/veld-rules/tests/envelope.rs` runs every file here.

| File | What it is |
|---|---|
| `envelopes.json` | The publish goldens: an array of cases. |
| `consume.json` | The consume goldens: what a consumer does with a message. |
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

A generator's test compiles `contract`, builds a publisher for `service`, fixes the id source to `id` and the clock to `time`, publishes `value` as `event` on `channel` through a transport that keeps the bytes, and requires the bytes to equal `canonical` (UTF-8, compared as bytes, not as parsed JSON). `value` is the event in wire form and already holds canonical scalars (lowercase uuid, `Z` datetimes, decimals and longs as strings); a typed publisher builds the same value from its typed event. Fields with a default may be left out of `value`: the message has them.

Then the opposite direction: the consumer of that event, given `canonical`, hands its handler the same event and the metadata `id`, `source`, `type`, `subject` and `time`.

What the cases cover: a default applied (`nights`), a default inside a nested value (`qty`), the key as `subject` for a `uuid`, an `int` and a `long`, a channel without a key (no `subject`), an optional field absent and present, a `@wire` name, a decimal, a datetime with and without milliseconds (the fraction is written only when it is not zero), a non-ASCII string, an event without rules (no `veldrules`), and an event whose rules are all audited (no `veldrules`, and the audited rule, broken by the value, does not stop the message).

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

## Rules

- A golden changes only with the contract's signatures or with the spec. Never edit the `canonical` text by hand to make a generator pass; compute it again with the reference encoder and read the diff.
- A new edge case goes into the file with its contract; the reference test and every generator's test pick it up without code changes.
