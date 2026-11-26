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

# Platform P2 (Node is in; Java and Rust mirror `spec/targets/typescript.md` 5j)

Settled in `spec/platform.md` after the Node work: every flag an endpoint can reach is read once, after authentication (not at the first statement that has it); `metrics` is called when the answer is ready to be written; the due-moves runner enters after step 3 and runs no middleware.

## Node (owed)
- The operands of `old(...)` that only a flagged `ensure` uses are still evaluated when the flag is off; the reference removes the statement. It shows only when the operand raises an evaluation error.

# Still open after the Rust alignment pass (26 Nov)

## Rust
- `require`, `allow`, `ensure` and `old(...)` of an endpoint block that read a request parameter's derived field without a value still read the stand-in (step 6 receives the arguments after the stand-ins are filled). To fix: step 5 carries the absent patterns of each parameter to step 6, and the reach analysis handles parameter roots in the guards of `block.rs`.
- The skip of a read of an absent derived field is by pattern, not by item: one absent item skips the whole read, where the reference can still fail an earlier item (written in `spec/targets/rust.md` 16).
- A derived field inside the items of a `Page` is not skipped when absent.
- A derived field that is absent in a request is a stand-in in the handler's struct (kept on purpose: an `Option` would have to be threaded through every site that reads a field by its IR path).

## The compiler, for all three targets
- An IR key that marks a derived field which can be absent in a request (it reaches a `@readonly` field), so that the three generators can type it and guard it without each working it out.

# An `allow` that reads a derived field without a value denies (26 Nov; `spec/rules.md` 17.2)

A skipped `allow` would permit, so this one case fails closed: `403 Forbidden`, as for any policy argument that cannot be evaluated (11.3).
- **Java**: today `Derive.orSkip` skips the `allow` (permitted). Make it a denial; a case in `DerivedAlign.java`.
- **Rust**: today the stand-in is read (open item above). When that is fixed, an `allow` denies.
- **Node**: check what `veld.permits` does with an absent derived argument, and add the case to `derived_http.ts`.

# Still open after the Java pass (26 Nov)
- Flagged invariants on incoming WebSocket messages are not evaluated in Java (no flag view on the socket thread).
- The Java drivers for timed moves, flags and derived fields run through the JDK handler only, not Spring.
- `RuleWalk.java` in the Java runtime has an unused import (`java.util.LinkedHashMap`) from before this work; the import-hygiene test of `spec/targets/java.md` section 7 probably fails on it.
- A request with a deadline runs on a thread of an unbounded cached pool in Java.

# Two shared test contracts grew after Java used them (26 Nov)

The Node agent added to `tests/toolchain/contract/platform_p2/flags.veld` an endpoint `resizeRoom` (a flagged `ensure` over an `old(...)` operand that divides by the capacity) and to `tests/toolchain/contract/derived/shop.veld` a field `level` and a derived `share` on `User`, a model `Draft` (a `@readonly` member and a derived field that reads it) and an endpoint `saveDraft`.
- **Java**: the stubs and drivers that implement these two contracts (`tests/toolchain/java/platform_p2/Flags*`, `tests/toolchain/java/derived/DerivedShop*`) no longer implement every endpoint and will not compile. Add `resizeRoom` and `saveDraft`, and the cases the Node drivers have for them (`flags_http.ts`, `derived_http.ts`).
- **Rust**: the same for its harness crates, where they use these contracts.
- From now on a target that needs another endpoint in a shared contract adds it for all three, or uses a contract of its own.

# Platform P2 in Rust: gaps (26 Nov)

The Rust target declares `rules_flags` but:
- **Flags on events are not built**: the publisher and the subscriber take no provider, so a flagged invariant of an event is never evaluated (Node and Java evaluate it with a context without a principal). Either build it or withdraw the capability for a service whose events have a flagged invariant (`E0801`).
- A flagged invariant on a WebSocket message or an SSE item is not evaluated (no flag view there). Java has the same gap for incoming WebSocket messages.
- A service whose only rules are flagged invariants gets no test handler, so `Veld-Test-Flags` cannot reach it.
- No test for the metric of a panicking handler.

# Platform P1 in Java: what differs from Node and 9.2 (26 Nov)

- ~~A `Page`'s `total` in the audit event~~ **This item was wrong, and the main session's mistake.** `Page.total` is a `long`: a string of digits on the wire and therefore in the audit event. The golden had it as a number and is corrected (`tests/wire/audit/cases.json`, the case "a page names its items under items"). See the section below for what Java now owes.
- **The client's retry on `429`** does not sit inside the call's timeout (each of the two requests has its own): 9.2 says inside.
- The test handler takes the caller from `Veld-Test-Principal` also for an `@auth(none)` endpoint.
- Not tested: the limiter's start-up notice; a `@tenant` member of a query object; the examples on Spring.

# Platform P4 part 1, "no message repeats a value" (Node is in, 26 Nov)

The Node agent found one message that repeats a value outside its own crates:
- **The reference and Rust**: the consumer's step-1 message for a wrong `datacontenttype` quotes the value (`crates/veld-rules/src/envelope/consume.rs` near line 125; `crates/emitters/rust/runtime/events.rs` near line 1824). Node now says `is not application/json`. Make the reference and Rust say the same fixed text (and Java, if it quotes it), and update any golden that holds the old text.
- Capture option: Node compiles the capture in only with the generator option `capture-violations` (so existing output keeps its bytes) and has a hooks member to turn it off. `spec/rules.md` 17.3 calls it "a server option, off by default": Java and Rust use the same option name and the same two levels.

# A page total in the audit event: Java must go back (26 Nov)

On a wrong instruction of the main session, the Java target was changed to write a `Page`'s `total` as a JSON number in the audit event (commit "fix(java): the total of a page is a JSON number in the audit event", `Platform.numberTotals`). The wire form is a string of digits, the audit event carries the result as it is sent, and the golden now says `"total":"1"`.
- **Java**: remove `Platform.numberTotals` and its call, so the event has the string again; keep the driver `AuditGoldens.java` without any conversion; correct the sentence in `spec/targets/java.md` 17.
- **Node, Rust**: write what is sent (a string); no conversion anywhere.
