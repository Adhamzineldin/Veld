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
- Policies (commits `f6acf79` to `9924791`): the conformance case `e0003_endpoint_later_phases` and its snapshot were removed, because it asserted that `allow` is "not supported yet" and `allow` now is. No other snapshot from before the branch changed or was renamed. `e0952_example_value_mismatch` keeps a help line that mentions "principals arrive with policies", now out of date, so that its snapshot stays byte-identical; correcting the wording changes that one snapshot.

- The `auth-required` decision (see "Decided by the owner"): files from before the branch that gained one `@auth(none)` line are `examples/hello/contracts/hello.veld`, `examples/rust-catalog/contracts/books.veld`, four contract templates in `apps/veld/src/commands/init/render.rs`, two website presets and three website snippets. No generated source file changed; the two examples' `veld.lock.ir.json` moved by one line number each. No insta snapshot changed.

## Decisions waiting for the owner

- **Spring and a trailing slash.** The JDK router treats `/bookings` and `/bookings/` as the same path; the generated Spring controller maps only the first, so `POST /bookings/` is a 404 on Spring. For contracts with rules the controller now maps both. Fixing it for every contract changes Spring snapshots from before this branch, so it was left alone.
- **A Rust generator bug from before this branch.** A generated crate that uses SSE, in which one service has no SSE endpoint, does not compile (`call_sse` is missing for that service). Found while testing; not fixed here.
- **Should `veld generate` stop on a changed rule?** `veld diff --strict` now fails on a "possibly breaking" rule change. The breaking-change gate of `veld generate` (`crates/veld-tools/src/gate.rs`) still looks at breaking changes only, so editing a rule's expression does not prompt or block a generate. Making it count is two lines (`Severity::fails_check`); it would mean a prompt every time a rule is edited until the lock is updated. Recommended: yes, for consistency with other breaking changes.
- **Two Java helper names.** See the Phase 1 list: finishing the rename changes three lines of a snapshot.
- **The policies feature covers more than `allow`.** A contract whose `require` or `ensure` reads the principal is marked with the `rules_policies` feature even without an `allow`, so a generator that cannot supply the principal refuses it (`E0801`) and never emits code that fails at run time. `spec/rules.md` 11.4 said only an `allow` uses it; the spec is to be widened to match unless you disagree.
- **`Auth.declared` is not in the IR JSON.** The IR cannot tell an explicit `@auth(none)` from no `@auth`; adding a key would have changed every stored IR snapshot. The flag exists in memory only, for the lint, and a plugin or a tool that reads `ir.json` does not see it. Making it part of the IR is an `ir_version` change.
- **Events: two choices in `spec/events.md` section 20 that were not asked, drafted as recommended.**
  1. *A rejected message is acknowledged*, so one bad message cannot block a Kafka partition. Without `@deadLetter` and without a hook that stores it, the message is gone.
  2. *`@retry` on the channel* (one policy for every consumer, as in the proposal), not per subscription.
- **`412` or `409` for a failed `If-Match`.** `spec/rules.md` 11.7 says `412 PreconditionFailed` (the HTTP meaning). Not built yet; say so if you want `409`.
- **Idempotency needs to know who the caller is.** A key must never replay another caller's answer, so the store's scope includes the caller's identity. `spec/rules.md` 11.9 drafts it as a text the application returns from a server hook; a server whose endpoint needs it and has no such hook does not start. The alternative is to name a field of the principal in the contract (`@idempotent(caller: user.id)`).
- **Workspace layout: six choices in `spec/workspace.md` section 16**, drafted as recommended. The ones you would notice: defaults are written per role (`[workspace.defaults.server.java]`), one level deeper than the proposal's sketch, because a Java server takes a framework and a Java client takes none; a project that is one file must write the service's `name`, because the folder name changes with where the repository is cloned; there is no converter from today's layout; and `veld.lock.ir.json` stays one file for the workspace, refreshed only by a run over every member.
- **Derived fields: two things you will see in contracts and in handler code** (`spec/rules.md` 17.2), not built until you confirm. (1) `total: decimal = 0` is a default and `total: decimal = subtotal - discount` is derived: the only difference is whether a literal follows the `=`. The alternative is a keyword. (2) In the type a handler returns, a derived field is optional and whatever the handler puts there is overwritten.

## Decided by the owner on 25 Nov 2026

- **Events, contract skew:** the tolerant consumer. It decodes with its own type, ignores fields it does not know, and checks rules only when its rules digest equals the publisher's (`spec/events.md` section 8).
- **Events, a service without `at`:** allowed for a service that has no endpoints (`spec/events.md` section 5).
- **The `auth-required` lint (`L0014`):** on by default, and what the repository ships says `@auth(none)` where it is public. Done for `examples/hello`, `examples/rust-catalog`, the contracts `veld init` writes, and the website's hello and streaming presets and snippets; a test now checks that every `veld init` scaffold passes `veld check` without a warning.
- **Emitters:** new features are built in the Rust, Java and Node generators; Node includes the TypeScript client. Every other emitter belongs to the teammates.

## Found while writing the bookings example (to fix or decide)

- [i] The `unused-model` lint (`L0001`) said a model is never used when only a `fact` uses it. Fixed: a fact's type and key type count as uses; the example no longer turns the lint off.
- [ ] `veld fmt` joins an example's call into one line however long it is (130 characters and more in the bookings contract), so the hand-wrapped examples of `examples/bookings` do not pass `veld fmt --check`. The formatter should wrap a long call or a long record.
- [ ] `testing = true` written as a direct target key in `veld.toml` is rejected with a message that lists only the direct keys; it belongs under the target's options. The message should say so.
- [ ] A `require` violation lists the parameter (`body`) in `fields`, not the field inside it (`body.guests`), so a form cannot place the message next to an input. Consider listing the paths.
- [ ] `veld.FetchLike` does not accept the global `fetch` type without a cast.
- [ ] Money arithmetic on `decimal` strings has to be written by hand in a handler; derived fields (Phase 5) will cover the common case.

Found when the example gained a state machine and policies (25 Nov), most important first:

- [ ] **A handler under `veld test --live` cannot find the example's facts in its own store.** The test handler takes the facts from a header and never calls the loaders, so a handler that writes conditionally (as a transition requires) finds no row. The example's test server works around it by putting the given booking into its store before the handler runs. The test handler needs a hook for this, or `spec/rules.md` 12.4 must say how a real handler sees a given fact.
- [ ] **The caller is typed `unknown` inside a handler.** The auth hook returns the typed principal, but `ctx.principal` is not typed, so a handler that needs the caller (the owner of a new booking) casts. The context should carry the principal's type.
- [ ] **A `post` that moves a state answers `201`** unless the contract writes `-> 200 Booking`, because `201` is the default of every `post`. Nothing warns. A lint could: a `post` with a `transition` that returns the fact's type.
- [ ] **`can<Endpoint>` answers only yes or no.** An app can grey a button but cannot say why until the server refuses. A form that also gives the policy's message would help.
- [ ] **A client has no way to learn who the caller is** except an endpoint the developer adds (`whoAmI` in the example), and the `can<Endpoint>` helpers need that value.
- [ ] **Examples repeat themselves.** Every example of a signed-in call writes the whole principal again; there is no way to name a caller, or a given, once.
- [ ] **`veld fmt` and a hand-formatted contract disagree in more places:** besides joining example calls into one long line, it puts every enum variant on its own line and expands a one-line `error X 409 { f: t }`. The example's contract does not pass `veld fmt --check`.
- [ ] **`veld generate` stops on "breaking change" for the first edit of a contract under development** (adding auth adds `401` to every endpoint). True, but a project with no released client has nobody to break. `--accept-breaking` gets typed without reading.
- [ ] **`npm ci` inside the example end-to-end test has no time limit.** It hung once for about 50 minutes while the machine was busy.
- [i] The example test asserted `contains("8 passed")`, which "38 passed" also satisfies. It now counts the `example` blocks of the contract and requires exactly that many.

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
- [i] `veld test --live <url> --fuzz N [--seed S]`: seeded generated requests, boundary requests first (numbers, lengths, datetimes, dates, decimals), the reference evaluator's verdict compared with the server's answer including the violations; a mismatch prints the seed and a paste-ready `example` block. The generated Node server agrees on 800 requests in the committed test (8,000 in a one-off run). Not yet: boundaries for `time` values
- [i] An independent checker tried to break the fuzzer and the live comparison (one-token mutations of a generated Node server were all caught within three seeds; 2,000 printed example blocks compiled and gave their requests back). It found seven problems, all fixed with tests in `136061c`: a minimum length above 64 was cut short (a false failure), a huge `--fuzz N` overflowed, a server dying mid-run lost the report, the problem document's `title`, `operation`, media type and `Veld-Rules` header were not compared, violation order was compared where the spec leaves it open (maps), any JSON object passed the probe, and a run that only ever hit one error looked green. Not checked by it, and still open: values the generator never makes (non-BMP strings, `-0`, `long` above 2^53, datetimes with offsets), and the Java and Rust test handlers under fuzz
- [i] `veld test --live` sends a service's examples and fuzz requests only to a test handler that names that service in its probe; the rest are skipped. A workspace with several services runs it once per handler

## Phase 3: state machines
- [i] `flow`, `final`, `transition`: IR, compiler, reference evaluator and **all three generators** (Java client, JDK server and Spring; TypeScript client and Node server; Rust client and axum server). The 14 hand-written examples pass through the generated test handler of each, under the whole-error comparison. For Node, the behaviour over HTTP and the examples run on the `node-http` adapter only; the other adapters wrap the same handler, the typed auth hooks are asserted in the output of all five, and the Nest server is compiled, but none of the four was started for these two features
- [i] `InvalidTransition`; the transition passed to the handler for a conditional write (a guard, not an atomic guarantee): a typed value after the facts in every target (`Transition<Status>` in Java and Rust, `veld.Transition<Status>` in TypeScript), with the "guard, not guarantee" wording in its doc comment
- [i] Next-state and final-state helpers, client and server: `nextStates()` and `isFinal()` on the enum in Java, `Status.nextStates(state)` and `Status.isFinal(state)` in TypeScript, `next_states()` and `is_final()` in Rust
- [ ] `emits` (with E3)
- [ ] State diagrams in docs and the playground

## Phase 4: policies
- [i] `principal`, `policy`, `permit`, `forbid`, `allow`; implicit `Forbidden`: IR, compiler (13 conformance cases), reference evaluator and **all three generators**: the auth hook returns the generated type of the principal's model, `allow` runs after the facts and before the transition guard, a denial is `403` with the forbid's message. The 10 hand-written examples pass through the generated test handler of each
- [i] Client `can<Endpoint>()` helpers and policy functions in every target: `Policies.manageBooking(user, booking)` and `BookingsClient.canPayBooking(booking, user)` in Java, `manageBooking(user, booking)` and `BookingsClient.canPayBooking(booking, user)` in TypeScript, `manage_booking(&user, &booking)` and `can_pay_booking(&booking, &user)` in Rust. The argument order is fixed in `spec/rules.md` 11.4
- [i] The vector files `tests/rules/endpoints/transitions.veld` and `policies.veld` are no longer held back as `.pending`: every generator supports them
- [i] The fuzzer generates the caller, makes policies permit and forbid on purpose, puts a fact in every state of its machine, and leaves some requests failing two checks at once so the order of the checks is exercised. It runs against the generated Node, Java (JDK) and Rust servers for all three rule contracts: 300 requests per endpoint, the same tallies in all three, no disagreement with the reference evaluator. Both denial messages are compared. Not covered: the Spring adapter under fuzz; an endpoint with both an `allow` and a `transition` against a real server (no served contract has both; the order is checked against the reference only); `matches(...)` and `!=` on floats or strings in a policy are left to chance
- [i] `examples/bookings` uses both: a `Status` machine moved by four endpoints, a `principal`, three policies, 38 examples, an Express server that writes conditionally, a React app that shows only the moves the server would accept, and an end-to-end test (HTTP checks, `veld test --live`, a fuzz run). The React app is type-checked and driven through the generated client; it was never opened in a browser
- [ ] Not yet, and required by "every item ships with" above: the language server has no hover or completion for `policy`, `principal` and `allow` (reported by the agent that built them; whether it handles `flow` and `transition` has not been checked), and there is no record of the playground WASM being rebuilt since rules were added
- [i] With `validate = false`, policies and the transition guard still run (`spec/rules.md` 5.3 corrected: it used to say everything in step 6 but the facts is switched off, which would have let a validation option switch authorization off)
- [~] `@version` optimistic concurrency (`ETag`, `If-Match`): the exact behaviour, the example syntax (`ifMatch 3`) and the test handler are drafted in `spec/rules.md` 11.9; no code
- [~] `@idempotent` and the key store interface: the store, the fingerprint, the scope, the replay, the example syntax (`key "k-1" stored ...`) and the test handler are drafted in `spec/rules.md` 11.9; no code
- [i] Lint `auth-required` (`L0014`; `L0005` was taken): every endpoint has auth or `@auth(none)`

## Phase 5: proof and lifecycle
- [~] Spec: `spec/rules.md` sections 17 (`@rollout(audit)`, derived fields, `veld test --from-violation`) and 18 (`check --prove`: the questions, the four results, the modelled fragment, the solver) drafted. No code yet
- [ ] `check --prove`: SMT-LIB encoding, external Z3 or cvc5, four results (proved, counterexample, unknown, unsupported), assumptions and solver version reported
- [i] Rule-aware `veld diff`: invariants, `require`, `ensure`, facts, flows and transitions are classified; with no solver yet, a changed expression is "possibly breaking" and shows the old and the new text; `--strict` fails on it
- [ ] `@rollout(audit)`
- [ ] Derived fields: specified in 17.2, held until the owner confirms two visible choices (see "Decisions waiting for the owner"). The proposal names them as the first Phase 5 cut
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
- [~] Spec: `spec/events.md` drafted (grammar, envelope, publish and consume pipelines, transport, IR, diagnostics `E1001` to `E1010`, phases); 11 hand-written example vectors in `tests/events/bookings.veld.pending`. No code yet: E1 starts after the Phase 3 and 4 generator passes are merged, and after the four choices in "Decisions waiting for the owner" are answered
- [ ] E1: `event`, `channel`, `publishes`, `subscribes`; CloudEvents envelope; typed publisher and subscriber in Node, Java and Rust; memory transport; event examples in `veld test`
- [ ] E2: Kafka and RabbitMQ adapters per language; retry and dead letter; inbox; broker capability matrix; Docker toolchain tests
- [ ] E3: outbox and relay for `emits` (at-least-once, stated as such); event-aware `diff`; AsyncAPI export; crash-injection tests

## Config
The work is two series, because only the second changes generated code. The layout series must leave every generated file, manifest and lock file of every example and fixture the same bytes; the runtime config series changes client code for every contract, and those snapshots go on the owner's list.
- [~] Spec for the layout: `spec/workspace.md` drafted (root and member files, the single file form, defaults, target names and default `out`, `-p`, `--affected --since`, `owners`, the error for the earlier layout, codes `E0709` to `E0712`, and the list of every reader of the layout). No code. Six choices in its section 16 wait for the owner
- [~] Spec for the typed runtime config: `spec/runtime-config.md` drafted (what a setting is, one `<Target>Runtime` type per target, the check that lists every problem at startup, `fromEnv` with names the application chooses, what is removed, `veld export runtime-schema`, and which generated files change). No code. Six choices in its section 9 wait for the owner; read it before it is built, because it changes the constructor of every generated client
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
AI coding agents (§14 of the plan, including `veld export mcp`), beyond web (§15: Kotlin and Swift clients, the desktop guide), and every emitter other than Rust, Java and Node with its TypeScript client (owner, 25 Nov 2026). A new emitter gets rule support by implementing the `ExprPrinter` hooks and passing the vectors under `tests/rules/`; until it does, it refuses a contract with rules with `E0801`.
