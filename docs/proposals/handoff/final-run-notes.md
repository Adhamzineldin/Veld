# Notes for the one full test run at the end

Things known to need attention when the whole suite runs, collected from the agents' reports.

- `apps/veld/tests/conditional_cli.rs` expects `veld generate` to refuse `@version`/`@idempotent` with `E0801`; the generators now support them. Update the test to what `veld init` targets do.
- `website/src/pages/docs/cliHelp.ts` is generated from the binary (`website/scripts/gen-cli-help.mjs`); regenerate after all CLI flags are in (`export runtime-schema`, `export slo`, `export retention`, ...), and add each new subcommand to the script's command list.
- `crates/veld-lsp/tests/rules_prefixes.rs` analyses every prefix of the bookings contract: slow in a debug build.
- Vector files still named `.pending`: rename when every target supports the feature, and wire them into every runner (`version`, `idempotency`, later `derived`, `timed`, `sagas`, ...).
- Snapshots to store are listed in each agent's report; run the suite once with `INSTA_UPDATE=unseen`, read every new snapshot, then run again with `INSTA_UPDATE=no`.
- Rust `@idempotent`: a dropped request future or a handler panic leaves the key "running" (spec/targets/rust.md 14); `spec/rules.md` 11.12 says `release` also when the server fails after `begin`. Decide whether a drop guard is needed.
- Runtime config: Java and Rust must switch to the shared `contract_has_rules` predicate (it is the Node one).
- Linux `npm ci` in `tests/toolchain/ts` and `website` before any toolchain test; JDK from `~/.jdks/temurin-25.0.3`; Maven from `~/.m2/wrapper/dists`.
- Z3 here is 4.16.0; the solver tests were written against 4.8.12.
- Idempotency fingerprint: the Java runtime does not normalise scalars (a path uuid in upper case gives another fingerprint than in lower case); the Rust agent did not check normalisation against the reference either. Compare all three targets with the reference on a uuid in upper case and a datetime with an offset, and make them agree with `veld_rules::fingerprint` on the bound wire form.
- Java: no test handler for a service with `@idempotent` or an ETag and no endpoint rules (java.md 9.5), while Rust generates one. `veld test --live` then has nothing to talk to for such a Java service: decide whether Java should follow Rust.
- Java drivers do not cover `validate = false` for the guard and the header checks.
- E2 goldens `tests/wire/events/dead-letter.json` and `retry.json` were produced by independent Python, never compared with the Rust reference: on a mismatch check the Rust against spec/events.md 10.4 and 10.5 first.
- LSP tests with exact completion lists may move: the annotation table grew (`@retry`, `@deadLetter`, `@ordered`).
- CI on PR #8 (26 Nov): `conformance_snapshots` stops at `tests/conformance/e0601_unknown_annotation/snapshot.snap`, a snapshot from before the branch that changes because the list of known annotations grew. Read it, store it, list it in the status file under "Existing snapshots that changed".
- Sagas: the expected `at_ms` and `save_statuses` of `tests/rules/sagas/orchestrator.json` were simulated by hand (a Python script), never run against the Rust reference orchestrator. On a mismatch check the reference against spec/platform.md 18 first.
- Sagas: `veld check` fills saga examples but does not run them offline; an optional parameter cannot be passed as a step argument (`E0903`).
- Events E3 removed the conformance case `tests/conformance/e0003_flow_emits` (created on this branch; its premise, that `emits` is a syntax error, is gone) and changed assertions in `crates/veld-tools/src/diff/event_tests.rs` and `apps/veld/tests/tools_cli.rs` to what the table of spec/events.md 16 decides (an optional field added is compatible; a topic, key or removed-event change is breaking). List both in the status file under "Existing snapshots that changed".
- `veld export asyncapi`: the agent could not fetch the AsyncAPI 3.0.0 text. Its two starred points in spec/events.md 16.1 are right as far as the main session knows the standard (`schemaFormat` `application/schema+json;version=draft-07`; an operation's `messages` are `$ref`s into its channel's `messages`), but validate one exported document with an AsyncAPI 3 parser before calling it done.
- Runtime config, to compare across the three targets once Node reports: which servers keep `rulesOutput` in their hooks (Java and Rust keep it for a server whose model has no `rules.output`); what `contract_has_rules` counts (the Java and the Rust agent read it differently for policies and `@rollout`); where the generated types live (`runtimeconfig` package, `runtime_config.rs`, `runtime.ts`).
- Platform P2 changed the reference guard: a transition's edge must exist (a flagged edge that is off is not an edge). Check that no existing transition vector relied on a guard without an edge.
