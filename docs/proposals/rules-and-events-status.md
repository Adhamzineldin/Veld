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

Nothing yet. The one planned change is the plumbing commit: `ir_version` 1.0 to 1.1 and the new prelude error `InvalidTransition`, which shift stored IR only.

## Phase 0: spec
- [i] `spec/rules.md`: expressions and invariants complete; endpoint rules, state machines, policies and examples have syntax and decisions
- [ ] Complete §9 and §12 (endpoint rules, examples) before Phase 2
- [ ] Complete §10 (state machines) before Phase 3
- [ ] Complete §11 (policies) before Phase 4
- [ ] `spec/events.md` before E1
- [ ] `spec/platform.md` before P1
- [ ] `spec/config.md` rewrite before the config series

## Plumbing (one commit series, before any fan-out)
- [ ] IR types for rules (`Rule`, `Expr`, ...), omitted from JSON when empty
- [ ] `Feature`: `rules_invariants`, `rules_endpoint`, `rules_transitions`, `rules_policies`
- [ ] No emitter claims a feature by default: `Capabilities::all_except` replaced or the new features listed as unsupported in all four emitters, the plugin host and the `new-emitter` template
- [ ] `ir_version` 1.1 and prelude `InvalidTransition`, the single churn commit
- [ ] Empty `crates/veld-rules` crate in the workspace

## Phase 1: invariants (the thin slice; nothing else starts before it is `[t]`)
- [ ] Lexer: new operators, the minus rule
- [ ] Parser and AST: expressions, `rule` in model and error bodies, contextual words
- [ ] Typing pass: types, widening, guards for optional values, `E0901` to `E0908`, `W0901` to `W0903`
- [ ] Lowering to IR, `reads`, inherited rules
- [ ] Rule ids in `veld-compiler::signature`
- [ ] Conformance cases: every new code, every contextual word used as a field, variant and parameter name
- [ ] `fmt` for rules and expressions; fmt properties still hold
- [ ] `crates/veld-rules`: reference evaluator and value helpers (decimal, instant, code points, checked arithmetic)
- [ ] `tests/rules/`: starter vectors (40 or more) covering the list in `spec/rules.md` §15
- [ ] `veld-codegen`: `ExprPrinter`
- [ ] TypeScript client: check functions, helpers, local check before send, `violations` on `ValidationError`, `.partial(draft)`
- [ ] Node server: pipeline steps 5 and 8, output mode, `onRuleViolation`
- [ ] Java: check functions, helpers, client and server (JDK and Spring)
- [ ] Rust: check functions, helpers, client and server
- [ ] Vector runner per target: all vectors identical in TypeScript, Java, Rust and the reference
- [ ] Toolchain round trip: a rule violation over real HTTP, same `violations` from the Node, Java and Rust servers
- [ ] OpenAPI export does not break on rules (full `x-veld-rules` is Phase 5)
- [ ] Target specs updated (`spec/targets/*.md`), `spec/ir.md`, `spec/language.md` cross-references

## Phase 2: endpoint rules, examples, `veld test`
- [ ] `fact`, `require ... else`, `ensure`, `old()`, `now`, the injected clock
- [ ] Fact-loader interfaces and the pipeline in Node, Java and Rust
- [ ] Output mode and sampling in the runtime config
- [ ] Stale-client safety: `Veld-Rules` digest, `onContractMismatch`
- [ ] `example` blocks: grammar, IR, value synthesis for partial literals
- [ ] `veld test` offline (reference evaluator, given facts)
- [ ] `veld test --live` with server test mode
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
