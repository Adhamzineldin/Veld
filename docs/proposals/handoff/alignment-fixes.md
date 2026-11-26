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

# Derived fields (compared 26 Nov; `spec/rules.md` 17.2, the paragraph "Where the three generators differed", is the rule)

## Node and TypeScript
- **`validate = false`**: a request whose derived field cannot be computed is `422` with that field's violation. Today `checkInputRules` returns early and the handler gets `undefined` where its type promises a value.
- **The principal**: complete it once per request after authentication, for every endpoint (today: only when the endpoint's rules read it), and hand the completed value to the handler as `ctx.principal` (today the handler sees the auth hook's own value).
- **Rename** the public function `compute` to `derive` (`<Decl>.derive(draft, out?)`, `derive<Decl>Derived` and the like), in the generator, the tests, the drivers and `spec/targets/typescript.md` 5i (which says the other targets mirror its names).
- The client's local check keeps `@readonly` members the server would drop before deriving (`checkRules` uses `withDefaults`), so it can compute a derived value the server would not: treat a `@readonly` input as absent there too.

## Java
- **`validate = false`**: `422` for a request whose derived field cannot be computed (today: silent, the handler gets `null`).
- **A nested `@readonly` read in a request** leaves the derived field absent (today a placeholder is read and the handler sees an invented value).
- **`require`, `allow`, `old`** that read a derived field without a value skip (today they can fail with a `NullPointerException`).
- **A consumed event** whose derived field cannot be computed is rejected as `rule` whether or not the digests match (today only when they match).
- **The principal**: completed for every endpoint, not only one with a rule block.

## Rust
- **A read of a derived field without a value through a nested path skips** (today the stand-in is read: a valid request can get a `422`).
- **A nested `@readonly` read in a request** leaves the derived field absent (today the stand-in is read).
- **A consumed event** whose derived field cannot be computed is rejected as `rule` (today the failure is dropped and the handler gets a stand-in).
- **The principal**: completed for every endpoint and handed to the handler (today: only when the endpoint's rules read it, and the handler sees the auth hook's value).
- **Types**: a derived field of type bytes, json, list, map, a model or `Page` is `E0801` today; only `duration` may be refused. `spec/targets/rust.md` 16 does not mention `duration`: add it.
- **A derived field that is absent in a request** (it reads a `@readonly` field) is a stand-in in the handler's struct, which hides that it is missing. Make it visible in the type (`Option`) for exactly those fields, or say in the report why not.

## All three, later
- A derived field that can be absent in a request (it reaches a `@readonly` field) should be marked so in the type a handler receives. The compiler could say which fields those are (one more key next to `derived`), so that three generators do not each work it out.

# Events phase E2 (compared 26 Nov; `spec/events.md` 10.1, 10.7 and 10.9 now say the rule)

The RabbitMQ topology, the dead-letter copy, retry and the inbox agree in all three targets. The differences:

## Rust
- **The Kafka partitioner** (wire, cross-target): the adapter sets none, so librdkafka's default (`consistent_random`, CRC32) applies, while the Java client and kafkajs use murmur2. A Rust producer and a Node or Java producer then put one key into different partitions and per-key order is lost across producers. Set `partitioner=murmur2_random` on the producer (check the property name against the rdkafka version in use), say so in `spec/targets/rust.md` 13, and add the row to the Kafka broker program (a key published by the Rust producer lands in the partition the Java rule gives: compute the expected partition with murmur2 in the test).
- **A revoked partition**: the adapter keeps delivering a record again and commits it after its partition was taken away. Abandon the delivery on revoke (a rebalance callback sets a flag that the delivery loop checks; no commit, no further redelivery).
- `allow.auto.create.topics=false` is set on the producer, where librdkafka does not read it (it is a consumer property). Remove the claim from the adapter's comment and from `spec/targets/rust.md` 13.
- **`publish` on RabbitMQ waits without limit** for the confirm: add `publish_timeout_ms` (30000 by default).
- The test handler's event endpoint: `400` with `{ "error": … }` for a missing or empty `Veld-Test-Topic` and for a `Veld-Test-Deliveries` that is neither `1` nor `2` (today: a plain-text body, and the number is clamped).

## Java
- The test handler's event endpoint: the `400` body is `{ "error": … }` (today `{ "message": … }`).

## Node
- **A message whose value cannot be decoded** (the step after the shape check, `fromWire` in `runOnce`) is treated as a handler failure and retried. It is a rejection with the reason `shape` (`spec/events.md` 8, step 3), as in Java and Rust: decode before the handler path.
- **`publish` on RabbitMQ waits without limit** for the confirm: add `publishTimeoutMs` (30000 by default).
- The test handler's event endpoint: `400` with `{ "error": … }` also for an empty topic and for a `Veld-Test-Deliveries` that is neither `1` nor `2` (today anything but `"2"` means 1).
- The `E0802` of `messaging` lacks the "invalid options for generator" prefix the other option errors have.

## All three
- The test programs that serve several services' test handlers at one address (`events_serve.ts`, `EventsServe.java`, the generated `delivery_serve`) each merge the answers in their own way: make each follow the one rule now in 10.9.
