# Rules and events: implementation status

Branch `feat/rules-and-events`. The plan is `rules-and-events.md`; the normative spec is `spec/rules.md`. This file is the checklist, so that nothing is forgotten across sessions.

**Statuses** are kept apart on purpose:
- `[ ]` not started
- `[~]` in progress
- `[i]` implemented: code and unit tests
- `[t]` tested: shared vectors and toolchain round trips green on every target
- production-supported is a release decision (the gates in the plan's §16) and is never claimed here

**Gates for the whole branch**
- A contract with no rules or events generates the same bytes as before.
- A target that cannot enforce a rule kind reports `E0801`.
- Every item ships with: its spec section, valid and invalid conformance cases, a `fmt` round trip, LSP still working on the new syntax, regenerated editor grammars, `spec/diagnostics.md`, `spec/ir.d.ts` and the IR schema, and a rebuilt playground WASM.

## Existing snapshots that changed (owner: please look)

- Commit `be05daf` (`ir_version` 1.0 to 1.1): 20 conformance snapshots, 5 OpenAPI snapshots (`x-veld-ir-version`) and the 4 example `veld.lock.ir.json` files each changed by exactly their version line. No generated source file changed. The new prelude error `InvalidTransition` changed no stored file, because a declaration appears in the IR only when a contract reaches it.

## Decisions waiting for the owner

- **Spring and a trailing slash.** The JDK router treats `/bookings` and `/bookings/` as the same path; the generated Spring controller maps only the first, so `POST /bookings/` is a 404 on Spring. For contracts with rules the controller now maps both. Fixing it for every contract changes Spring snapshots from before this branch, so it was left alone.
- **A Rust generator bug from before this branch.** A generated crate that uses SSE, in which one service has no SSE endpoint, does not compile (`call_sse` is missing for that service). Found while testing; not fixed here.
- **Should `veld generate` stop on a changed rule?** `veld diff --strict` now fails on a "possibly breaking" rule change. The breaking-change gate of `veld generate` (`crates/veld-tools/src/gate.rs`) still looks at breaking changes only, so editing a rule's expression does not prompt or block a generate. Making it count is two lines (`Severity::fails_check`); it would mean a prompt every time a rule is edited until the lock is updated. Recommended: yes, for consistency with other breaking changes.
- **Two Java helper names.** See the Phase 1 list: finishing the rename changes three lines of a snapshot.

## Found while writing the bookings example (to fix or decide)

- [ ] The `unused-model` lint (`L0001`) says a model is never used when only a `fact` uses it. A fact is a use.
- [ ] `testing = true` written as a direct target key in `veld.toml` is rejected with a message that lists only the direct keys; it belongs under the target's options. The message should say so.
- [ ] A `require` violation lists the parameter (`body`) in `fields`, not the field inside it (`body.guests`), so a form cannot place the message next to an input. Consider listing the paths.
- [ ] `veld.FetchLike` does not accept the global `fetch` type without a cast.
- [ ] Money arithmetic on `decimal` strings has to be written by hand in a handler; derived fields (Phase 5) will cover the common case.

## Phase 0: spec
- [i] `spec/rules.md`: expressions and invariants complete; endpoint rules, state machines, policies and examples have syntax and decisions
- [i] §9 and §12 (endpoint rules, examples, `veld test`) complete, with the IR in §13.1
- [i] §10 (state machines) complete
- [i] §11 (policies, `@version`, `@idempotent`) complete
- [ ] `spec/events.md` before E1
- [ ] `spec/platform.md` before P1
- [ ] `spec/config.md` rewrite before the config series

## Plumbing (one commit series, before any fan-out)
- [t] IR types for rules (`Rule`, `Expr`, ...), omitted from JSON when empty
- [t] `Feature`: `rules_invariants`, `rules_endpoint`, `rules_transitions`, `rules_policies`
- [t] No emitter claims a feature by default: `Capabilities::all_except` replaced or the new features listed as unsupported in all four emitters, the plugin host and the `new-emitter` template
- [t] `ir_version` 1.1 and prelude `InvalidTransition`, the single churn commit
- [t] Empty `crates/veld-rules` crate in the workspace

## Phase 1: invariants (the thin slice; nothing else starts before it is `[t]`)
- [t] Lexer: new operators, the minus rule
- [t] Parser and AST: expressions, `rule` in model and error bodies, contextual words
- [t] Typing pass: types, widening, guards for optional values, `E0901` to `E0908`, `W0901` to `W0903`
- [t] Lowering to IR, `reads`, inherited rules
- [t] Rule ids in `veld-compiler::signature`
- [t] Conformance cases: 19 new (every new code, every contextual word used as a name, every guard form)
- [t] Follow-ups: expression depth limit 32 with an IR read-back test; a unary minus on a literal is part of the literal
- [t] `fmt` for rules and expressions; fmt properties still hold (long rule lines are not wrapped yet)
- [t] `crates/veld-rules`: reference evaluator and value helpers; all 104 vectors pass
- [t] `tests/rules/`: 104 starter vectors in `tests/rules/core` covering the list in `spec/rules.md` §15
- [t] `veld-codegen`: `ExprPrinter`, plus `gate_regions` for runtime files
- [t] TypeScript client: check functions, helpers, local check before send, `violations` on `ValidationError`, `.partial(draft)`; 104 of 104 vectors
- [t] Node server: pipeline steps 5 and 8, output mode, `onRuleViolation`; HTTP round trip
- [t] Java: check functions, helpers, client and server (JDK and Spring); 103 of 103 vectors that apply; HTTP round trip
- [t] Rust: check functions (the reference evaluator's own helper files, embedded), client and server; 101 of 101 vectors that apply; HTTP round trip
- [t] Vector runner per target, and the reference cross-check that compiles every vector contract
- [t] One golden 422 answer (`tests/wire/rules`): the Node server, the JDK server, the Spring adapter and the Rust server all answer with the same canonical bytes
- [t] Invariants on SSE and WebSocket items, in every target
- [t] Invariants of `error` declarations on encode, in every target
- [t] `validate = false`: the one-time report and `W0904`, in every target
- [t] `in` over lists of dates and times: shared vectors in `tests/rules/core/calendar.*`, passing in every target and the reference
- [~] Java helper names follow the TypeScript reference list, except two (`instantMs`, `len`) that the printer still uses: finishing them changes three lines of a snapshot, and that edit was blocked in the agent's session, so it waits for the owner
- [i] React and Vue form helpers (`useFormRules`) on top of `.partial(draft)`
- [t] Full verification of the merged branch: draft PR #8, CI green on Linux, macOS and Windows (the local run was stopped for lack of memory, so CI is the check)
- [t] OpenAPI export does not break on rules (full `x-veld-rules` is Phase 5)
- [i] Target specs updated (`spec/targets/*.md`); `spec/ir.md`, `spec/language.md` cross-references still to do

## Phase 2: endpoint rules, examples, `veld test`
- [t] `fact`, `require ... else`, `ensure`, `old()`, `now`: IR, compiler, reference evaluator and every target (CI green on three systems)
- [t] Fact-loader interfaces and the pipeline in Node, Java and Rust
- [ ] Output mode and sampling in the runtime config (the mode is a handler option today; sampling and the typed runtime config come with the config series)
- [ ] WebSocket endpoints with endpoint rules: every server target refuses them with `E0801` for now (Java refuses SSE endpoints with rules too)
- [x] Stale-client safety: Java, Rust, TypeScript and Node have it for every service with a digest, with or without endpoint rules
- [i] `example` blocks: IR, compiler, and filling in and running examples in the reference evaluator
- [t] Endpoint rule vectors: `tests/rules/endpoints/bookings.veld`, 20 examples, all passing through the compiler and the reference evaluator
- [t] `veld test` offline, the shared runner (`crates/veld-testing`), `E0953` in `veld check`, and `docs/guides/testing.md`
- [t] `veld test --live` and `--dry-run`: all 20 examples pass against the generated Node server, the JDK server, the Spring adapter and the Rust server
- [ ] Boundary cases generated from rules
- [ ] `veld test --live --fuzz`

## Phase 3: state machines
- [~] `flow`, `final`, `transition`: IR, compiler (12 conformance cases) and reference evaluator implemented (`[i]`); all 14 hand-written examples pass in the evaluator (the file stays `transitions.veld.pending` until the generators support transitions); generators to do, together with policies
- [ ] `InvalidTransition`; the transition passed to the handler for a conditional write (a guard, not an atomic guarantee)
- [ ] Client `nextStates()`
- [ ] `emits` (with E3)
- [ ] State diagrams in docs and the playground

## Phase 4: policies
- [~] `principal`, `policy`, `permit`, `forbid`, `allow`; implicit `Forbidden`: spec section 11 complete, 10 examples written (`policies.veld.pending`); IR, compiler and reference evaluator in progress; generators to do
- [ ] Client `can<Operation>()` helpers
- [ ] `@version` optimistic concurrency (`ETag`, `If-Match`)
- [ ] `@idempotent` and the key store interface
- [ ] Lint: every endpoint has auth or `@auth(none)`

## Phase 5: proof and lifecycle
- [ ] `check --prove`: SMT-LIB encoding, external Z3 or cvc5, four results (proved, counterexample, unknown, unsupported), assumptions and solver version reported
- [i] Rule-aware `veld diff`: invariants, `require`, `ensure`, facts, flows and transitions are classified; with no solver yet, a changed expression is "possibly breaking" and shows the old and the new text; `--strict` fails on it
- [ ] `@rollout(audit)`
- [ ] Derived fields
- [i] OpenAPI `x-veld-rules`, `x-veld-facts`, `x-veld-flows`, `x-veld-finals`, `x-veld-transition`; a contract without rules exports the same bytes as before
- [ ] LSP: hover and completion inside expressions
- [ ] Playground: live rule evaluation (WASM)
- [ ] `veld test --from-violation`
- [ ] Rule benchmark gate in CI

## Phase 6: extras
- [ ] `veld test --coverage`
- [ ] `veld export rules`
- [ ] `@pii` and `@secret` redaction, lint for `@pii` without auth

## Events
- [ ] E1: `event`, `channel`, `publishes`, `subscribes`; CloudEvents envelope; typed publisher and subscriber in Node, Java and Rust; memory transport; event examples in `veld test`
- [ ] E2: Kafka and RabbitMQ adapters per language; retry and dead letter; inbox; broker capability matrix; Docker toolchain tests
- [ ] E3: outbox and relay for `emits` (at-least-once, stated as such); event-aware `diff`; AsyncAPI export; crash-injection tests

## Config
- [ ] Workspace root plus one `veld.toml` per member; the old single-file format removed
- [ ] `-p`, `--affected --since`, `owners`
- [ ] Typed runtime config per target; `VELD_<SERVICE>_URL` fallback removed; `fromEnv()` opt-in
- [ ] `veld export runtime-schema`
- [ ] `veld init`, every example, every test fixture and the docs moved to the new layout
- [ ] CLI and WASM share one target-resolution function (parity guardrail)

## Platform
- [ ] P1: `@visible`, `@tenant`, `@audited`, `@limit`
- [ ] P2: timed transitions and the scheduler hook; `flag()`; `@timeout`, `@slo`, metrics hook, OpenSLO export
- [ ] P3: sagas (experimental until crash and compensation-failure tests exist), with a needs-manual-intervention state
- [ ] P4: message catalogs (Arabic first), `@retain` and the retention export

## Docs, examples, website
- [i] New example `examples/bookings`: a Node server with fact loaders and a separate test server, a React form with live rule messages (`useFormRules`), eight examples, covered by the example tests and an end-to-end test (the web form is type-checked but was not opened in a browser); a Java server for it is still to do
- [ ] `examples/microservices` gains events
- [~] Guides: `docs/guides/rules.md` and `docs/guides/testing.md` written; events and configuration to do
- [ ] Website: docs pages, landing examples, playground presets, rebuilt WASM
- [ ] Editor grammars (VS Code, JetBrains) regenerated; README and `docs/README.md` index

## Not on this branch (teammates)
AI coding agents (§14 of the plan, including `veld export mcp`) and beyond web (§15: Kotlin and Swift clients, the desktop guide).
