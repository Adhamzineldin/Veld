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

## Phase 0: spec
- [i] `spec/rules.md`: expressions and invariants complete; endpoint rules, state machines, policies and examples have syntax and decisions
- [i] §9 and §12 (endpoint rules, examples, `veld test`) complete, with the IR in §13.1
- [ ] Complete §10 (state machines) before Phase 3
- [ ] Complete §11 (policies) before Phase 4
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
- [~] Toolchain round trip: each target passes its own; a golden 422 answer is in `tests/wire/rules` and the Node server is held to it; Java and Rust to be held to the same file
- [~] Invariants on SSE and WebSocket items: TypeScript and Node done (`[i]`); Java and Rust to do
- [~] Invariants of `error` declarations on encode: TypeScript and Java done; Rust to do
- [~] `validate = false`: the one-time report and `W0904`: TypeScript and Node done, with a warning channel for generators; Java and Rust to do
- [t] `in` over lists of dates and times: shared vectors in `tests/rules/core/calendar.*`, passing in every target and the reference
- [ ] Java helper names aligned with the TypeScript reference list
- [i] React and Vue form helpers (`useFormRules`) on top of `.partial(draft)`
- [t] Full verification of the merged branch: draft PR #8, CI green on Linux, macOS and Windows (the local run was stopped for lack of memory, so CI is the check)
- [t] OpenAPI export does not break on rules (full `x-veld-rules` is Phase 5)
- [i] Target specs updated (`spec/targets/*.md`); `spec/ir.md`, `spec/language.md` cross-references still to do

## Phase 2: endpoint rules, examples, `veld test`
- [~] `fact`, `require ... else`, `ensure`, `old()`, `now`: IR, compiler (19 conformance cases) and the reference evaluator's pipeline are in (`[i]`); TypeScript and Node generators in progress; Java and Rust to do
- [ ] Fact-loader interfaces and the pipeline in Node, Java and Rust
- [ ] Output mode and sampling in the runtime config
- [~] Stale-client safety: the digest is computed by the compiler; TypeScript and Node in progress
- [i] `example` blocks: IR, compiler, and filling in and running examples in the reference evaluator
- [t] Endpoint rule vectors: `tests/rules/endpoints/bookings.veld`, 20 examples, all passing through the compiler and the reference evaluator
- [~] `veld test` offline, and the shared runner
- [~] `veld test --live` with the test handler (TypeScript and Node first), and `--dry-run`
- [ ] Boundary cases generated from rules
- [ ] `veld test --live --fuzz`

## Phase 3: state machines
- [ ] `flow`, `final`, `transition`; reachability and stuck-state checks
- [ ] `InvalidTransition`; the transition passed to the handler for a conditional write (a guard, not an atomic guarantee)
- [ ] Client `nextStates()`
- [ ] `emits` (with E3)
- [ ] State diagrams in docs and the playground

## Phase 4: policies
- [ ] `principal`, `policy`, `permit`, `forbid`, `allow`; implicit `Forbidden`
- [ ] Client `can<Operation>()` helpers
- [ ] `@version` optimistic concurrency (`ETag`, `If-Match`)
- [ ] `@idempotent` and the key store interface
- [ ] Lint: every endpoint has auth or `@auth(none)`

## Phase 5: proof and lifecycle
- [ ] `check --prove`: SMT-LIB encoding, external Z3 or cvc5, four results (proved, counterexample, unknown, unsupported), assumptions and solver version reported
- [ ] Rule-aware `veld diff`
- [ ] `@rollout(audit)`
- [ ] Derived fields
- [ ] OpenAPI `x-veld-rules`
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
- [ ] New example: bookings with rules, a Node server, a Java server and a React client
- [ ] `examples/microservices` gains events
- [ ] Guides: rules, testing, events, configuration
- [ ] Website: docs pages, landing examples, playground presets, rebuilt WASM
- [ ] Editor grammars (VS Code, JetBrains) regenerated; README and `docs/README.md` index

## Not on this branch (teammates)
AI coding agents (§14 of the plan, including `veld export mcp`) and beyond web (§15: Kotlin and Swift clients, the desktop guide).
