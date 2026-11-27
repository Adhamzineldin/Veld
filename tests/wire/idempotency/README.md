# Idempotency fingerprint goldens

What an `@idempotent` request hashes (`spec/rules.md` section 11.10): the first 16 hex characters of the SHA-256 of the RFC 8785 JSON of the bound parameters, keyed by parameter name. The Node, Java and Rust generators must each give the fingerprint the reference gives. The reference is `veld_rules` (`Reported.fingerprint`); its test `crates/veld-rules/tests/idempotency_fingerprints.rs` runs every case here. Every `hashed` and `fingerprint` in the file was printed by the reference itself, not calculated by hand.

| File | What it is |
|---|---|
| `fingerprints.veld` | The contract: one `@idempotent` endpoint for each position and kind of value. Compiled as the file `t.veld` of the package `bookings`, service `Fingerprints`. |
| `fingerprints.json` | The cases: an array. |

## A case

```json
{ "name": "path_int_007", "endpoint": "byInt",
  "request": { "method": "PUT", "path": "/fp/int/007", "query": {}, "headers": { "Idempotency-Key": "k" } },
  "same_as": "path_int_7",
  "hashed": "{\"n\":7}", "fingerprint": "…" }
```

- `request` is what a client sends. `path` and the values of `query` are **decoded** text (`/fp/text/a b`, `"s": "a b&c"`): a runner percent-encodes them. A `query` value is a string, or an array of strings for a repeated key (`ids=1&ids=2`). `headers` always carries the idempotency key (`Idempotency-Key: k`; it is not part of the fingerprint). `body` is a JSON value; `body_text` is the body as text, sent as it is, for a case whose point is the order or the spelling of the members (`1.50`, `15e-1`). A case has `body` or `body_text`, never both.
- `hashed` is the canonical JSON that is hashed; `fingerprint` is its hash. A generator's test sends `request` to a generated server (or calls its binding) and compares the fingerprint it takes, or the canonical JSON of its bound parameters, with these two. A test that cannot see the fingerprint can ask the store: the server calls `begin(scope, key, fingerprint)`, so a store that records the fingerprint it was given is enough.
- `same_as` names a case of the same endpoint that must have the same `hashed` and `fingerprint`; `differs_from` one that must have another. Both are checked by the reference test, so they hold for the reference; a target that fails one of them differs from the reference.
- `rejected` (`"422"`) marks a case that has no fingerprint: the request is not valid, step 5 answers it, and nothing is hashed. The reference is not run on it. A target must answer it as `rejected` says.
- `note` says why the case is there.

## What the cases cover

Per kind of value, in a path, a query, a header and a body wherever the language allows it: `uuid` in lower and upper case (one value); `datetime` as `Z`, with `.000`, `.250`, `.25` and with an offset (`.000` and the offset fold into the `Z` form, `.25` is `.250`); `decimal` `1.50` and `1.5`; `date`, `time`, `bytes`, enums; `int` (`7`, `007`, `-7`), `long` (`7`, `007`, a value beyond 2^53, leading zeros in a body), `float` (`7`, `7.0`, `7.50`, and `1.5`, `1.50`, `15e-1` in a body), `bool`; a list in a query in two orders; the `query` model with its default; `@header` parameters (part of the fingerprint, by parameter name, the header name without case); an optional parameter or member that was not sent, sent, and sent as `false`, `""` and `null` (a `json` member); a default written out and left out, also in a nested model, in a list and in a map; an empty list and an empty map; a map and a body whose members came in another order; a member the model does not have; a derived field (computed; a sent value dropped); a `@readonly` member that was sent (dropped), an unknown member at the top, nested and in a list (dropped); and the empty request, which hashes `{}`.

The goldens hold the request as the server understands it, in a canonical form (`spec/rules.md` section 11.10): a `uuid` in lower case and a `datetime` in UTC with `Z` (the fraction only when it is not zero) are one value however they were written; a `long` is the decimal string of its value (`007` is `"7"`); a member the model does not have, and a `@readonly` member, are dropped at any depth. A `decimal`, `date`, `time`, `bytes`, string and enum are the text as sent, so `1.50` and `1.5` stay two values, and a list keeps its order.
