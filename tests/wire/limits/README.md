# Limit goldens

What a rate limiter is asked and what the generated memory limiter answers (`spec/platform.md` section 5). The Node, Java and Rust servers must each reproduce `decisions` and `key`. The reference is `veld_rules::limits` (`crates/veld-rules/src/limits.rs`), whose module documentation is the rule of the memory limiter; its test `crates/veld-rules/tests/limits.rs` runs every case. Every `decisions` and `key` in the file was printed by the reference itself, and read through.

| File | What it is |
|---|---|
| `contract.veld` | The contract of the keys, compiled as the file `t.veld` of the package `bookings`. |
| `cases.json` | `{ "package", "contract", "windows": [ … ], "keys": [ … ] }` |

## `windows`

```json
{ "name": "…", "note": "…", "limit": 3, "windowSeconds": 60,
  "calls": [ { "key": "k", "t": 0 }, { "key": "k", "t": 3000 } ],
  "decisions": [ { "allowed": true, "retryAfterSeconds": 0 }, { "allowed": false, "retryAfterSeconds": 57 } ] }
```

A generator's test builds the **generated memory limiter** with a clock it controls (every target's memory limiter takes its clock as an option, default the system clock), calls `take(key, limit, windowSeconds)` for each call with the clock set to `t` (milliseconds from an arbitrary zero), and requires the answer to equal `decisions`, in order. The rule is in `limits.rs`: a fixed window per key that starts with the key's first call after the previous window ended; a window is open up to, not including, its start plus its length; a refused call does not count; `retryAfterSeconds` is the time left rounded up, at least 1, and 0 for an allowed call.

What the cases cover: the last allowed call and the first refused, the exact end of a window (1 ms before, and at it), refused calls that do not count, a window that starts with the first call and not with the clock, the rounding of the time left, two keys, a window of a second and of a day, a clock that goes back, and the first call after a long quiet.

## `keys`

```json
{ "endpoint": "byTenant", "principal": { "id": "…", "tenantId": "…" }, "key": "veld:bookings/Rooms.byTenant@…|tenant|0b5c9c3e-…" }
```

The key a server asks its limiter about is `<operation signature>|<k>|<identity>`. For `by: user` the identity is the text the application's caller-identity hook returns (`caller`); for `by: ip` the text the application's address hook returns (`address`); for `by: tenant` the tenant text of the principal (`principal`): a `string` as it is, a `uuid` in lowercase, an `int` or a `long` in decimal digits. An identity that holds a `|` is written as it is.

## What they cannot state

A limiter of the application (Redis, a gateway): the contract only says it MUST allow at most `limit` calls of one key in any window it defines. A limiter that fails (the call is allowed and the failure goes to `onError`), and the `429` answer itself (the prelude's `TooManyRequests` with `retryAfter` in seconds and the `Retry-After` header): an example cannot state a limit (the reference pipeline runs after the limiter), so those are the generators' tests.
