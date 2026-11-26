# Cross-target alignment: fixes owed per emitter

Found by comparing the three targets' reports for `@version` and `@idempotent` (26 Nov). `spec/rules.md` 11.12 now settles each. Fold the items of a target into the next agent that works in that emitter, as a first series of commits of its own.

## Rust (`crates/emitters/rust`)
- The handler's arguments are in the order facts, `version`, `transition` (11.12). Today `version` comes after the transition. Update the generator, the harness programs, the snapshots' expectations and `spec/targets/rust.md` 14.
- A dropped or cancelled request, or a handler panic, must release its idempotency key (11.12): a guard that hands `release` to a task of its own when the request future is dropped after `begin`. Today the key stays "running". A harness test for it.
- A failing `idempotency_caller` hook is `500 Internal` and the handler is not called (today the hook cannot fail: keep it so if the signature returns a `String`; say so in the target spec).
- **The fingerprint** (`spec/rules.md` 11.10, rewritten 26 Nov; goldens `tests/wire/idempotency/fingerprints.json`, 115 cases, values printed by the reference): the request is hashed in its canonical form. As read, not run, the Rust runtime (`runtime/server.rs` `idem_start`, `schema.rs` `to_wire(typed, true, false)`) fails every `*_uuid_upper`, `*_datetime_offset`, `*_datetime_millis_zero` and `body_datetime_millis_25` (uuid and datetime are not canonicalised: `normalize` is false there), and `body_any_null` (`map_model` filters `null` even for a `json` member, and null map values: a bug against `spec/language.md` 5.3 too). A harness program that runs every case of the file.

## Java (`crates/emitters/java`)
- A service with an `@idempotent` endpoint, a guard or an `ETag` and no other endpoint rule gets a test handler (11.12). Today it gets none (`spec/targets/java.md` 9.5).
- **The fingerprint** (`spec/rules.md` 11.10, rewritten 26 Nov; goldens `tests/wire/idempotency/fingerprints.json`): as read, not run, the Java runtime (`Router.java` `bind`, `bindSimple`, `Wire.prepare(REQUEST)`) fails every `*_uuid_upper`, `*_datetime_offset`, `*_datetime_millis_zero`, `body_datetime_millis_25` (a `CANONICAL` mode exists but the fingerprint uses `REQUEST`), and the `long` cases `path_long_007`, `query_long_007`, `body_size_leading_zeros` (a long must be hashed as the canonical decimal string of its value). A driver that runs every case of the file.
- A failing caller hook: `500 Internal`, the handler is not called.
- The `validate = false` driver for the guard and the header checks is missing.

## Node (`crates/emitters/node`, `crates/emitters/typescript`)
- **The fingerprint** (`spec/rules.md` 11.10, rewritten 26 Nov; goldens `tests/wire/idempotency/fingerprints.json`): the TypeScript runtime (`veld.ts` `fingerprintOf`, `normalForm`) is close. As read, not run, it fails `path_long_007`, `query_long_007`, `body_size_leading_zeros`, `body_size_minus_zero` (`coerce` keeps the text of a long; it must be the canonical decimal string of its value). Whether `holder_id_sent` and the derived-field cases (`holder_*`) pass is unknown. A driver that runs every case of the file.

## Every target, every later three-target feature
- After the three agents of a feature report, compare their target-spec sections side by side (argument order, option names, what gets a test handler, what happens on failure) before the next feature starts, and settle each difference in the shared spec.
