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

# Runtime config (compared 26 Nov; `spec/runtime-config.md` sections 3 to 5 now say the rule)

## Java
- **The test handler takes no runtime config** (section 4). Today `<Service>TestHandler.create` takes it like the handler and requires it when a URL is required. Remove the runtime argument from the test handler's entry; its output mode is a member of its own hooks.
- **No production server keeps `rulesOutput` in its hooks** (section 5). Today a server that has rules only through consumed services keeps it. Remove it there; such a server has no output mode.
- `spec/targets/java.md` 14.3 says a server with only policies or only `@rollout` has no `rules.output`. That is wrong: `contract_has_rules` counts an `allow` (a statement) and a rule with `@rollout`. Only a server whose rules are all in consumed services differs. Correct the text.
- `<Target>Clients` is derived by trimming `Runtime` from the type name repeatedly, so the target `runtime` gives `Clients`. Use the shared `pascal_target`.

## Rust
- **The test handler takes no runtime config** (section 4): today its `create` and `plain` take it. Remove it; the output mode is a member of the test handler's own hooks.
- **No production server keeps `rules_output` in its hooks** (section 5): remove it for a server that has rules only through consumed services.
- `spec/targets/rust.md` 15 lists policies and `@rollout` among what makes the emitter's flag broader than the shared predicate. Only consumed services do. Correct the text.

## Node
- Nothing: the Node test handler already takes no runtime config.

## Low priority, all targets
- Two consumed services whose client names collide in `createClients`: TypeScript qualifies with the package, Java appends `_`, Rust reports `E0801`. Pick one (qualifying is the useful one) when someone meets it.
