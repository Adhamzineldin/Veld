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

- Generated files from before the branch that each new piece of syntax regenerates (`cargo xtask codegen`): the three editor grammars (`editors/veld-language.json`, the VS Code and the JetBrains TextMate grammar) and `spec/ir.d.ts`. `@rollout(audit)` added the annotation name to the grammars and the `rollout` key to the IR types; events did the same for their words.
- The `auth-required` decision (see "Decided by the owner"): files from before the branch that gained one `@auth(none)` line are `examples/hello/contracts/hello.veld`, `examples/rust-catalog/contracts/books.veld`, four contract templates in `apps/veld/src/commands/init/render.rs`, two website presets and three website snippets. No generated source file changed; the two examples' `veld.lock.ir.json` moved by one line number each. No insta snapshot changed.

## Decisions waiting for the owner

- **Open the `plink` session to the zbook when convenient.** The broker tests of events phase E2 (Kafka and RabbitMQ in Docker) and the solver for `check --prove` (Z3) run there, and neither can start without it.
- **The CI workflow changed** (`c81c139`): the three jobs that build Spring now cache Maven and the wrapper's Maven distribution, because Maven Central answered `429` on a busy day. Say so if you want it reverted.
- **Five temp files of a checker run** are still in `%TEMP%` (`vt.log`, `vb.log`, `cli.log`, `node.log`, `alldone`): the checker's own delete was refused by a permission check, so they were left for you.
- **A subagent emptied this session's scratch folder** on 25 Nov while cleaning up after itself: about 200 scratch files of the session (helper scripts, logs, commit-message drafts, an old copy of the plan page). Nothing of the repository and nothing that was the only copy of a deliverable was in it: the plan page's source is kept elsewhere and is intact. Each agent now gets its own scratch subfolder and may delete only that.
- **`@rollout(audit)`: choices made where `spec/rules.md` 17.1 was silent**, written into that section. The ones you would notice: a generator supports the feature only when it says so (it is never part of "everything except"), so no emitter can enforce an audited rule by accident; a service whose invariants are all audited has no rules digest, so its clients check nothing locally; `@rollout` on a `fact`, an `allow`, a `transition`, a policy, a principal or an example is `E0602`; `veld test` lists the audited rules an example would violate, and `veld check` does not; in `veld diff`, a rule that is new and audited, or that changes while audited, is compatible.
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
- **Typed runtime config: six choices in `spec/runtime-config.md` section 9**, drafted as recommended. The one that matters most: a client keeps its `baseUrl` option, which becomes required, and a generated `createClients(runtime)` fills it; the alternative, a client constructor that takes the runtime config, changes every place that builds a client today. Either way every generated client changes, so the client snapshots from before the branch change with this work.
- **Platform P1: seven choices in `spec/platform.md` section 9**, drafted as recommended. The ones you would notice: `@limit(10, per: minute, by: user)` instead of the proposal's `@limit(10 per minute by user)`; a `@tenant` field is set by the server and never sent by the caller; there is no caller who sees every tenant; a failing audit hook does not fail the request, and a failing limiter lets the call through.
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
- [i] `spec/events.md` before E1 (see Events)
- [~] `spec/platform.md` before P1: drafted for P1, waiting for the owner's seven choices (see Platform)
- [~] The config specs before the config series: `spec/workspace.md` and `spec/runtime-config.md` drafted, waiting for the owner's choices (see Config); `spec/config.md` is rewritten with the layout series

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
- [~] Spec: `spec/rules.md` sections 17 (`@rollout(audit)`, derived fields, `veld test --from-violation`) and 18 (`check --prove`: the questions, the four results, the modelled fragment, the solver) drafted. Only 17.1 has code
- [ ] `check --prove`: SMT-LIB encoding, external Z3 or cvc5, four results (proved, counterexample, unknown, unsupported), assumptions and solver version reported
- [i] Rule-aware `veld diff`: invariants, `require`, `ensure`, facts, flows and transitions are classified; with no solver yet, a changed expression is "possibly breaking" and shows the old and the new text; `--strict` fails on it
- [i] `@rollout(audit)` outside the generators (`spec/rules.md` 17.1): the annotation on an invariant, a `require` and an `ensure`; `"rollout": "audit"` in the IR, omitted when the rule is enforced, so a contract without it keeps its IR; the rules digest of a service and of an event covers only enforced invariants; the reference evaluator evaluates an audited rule, reports it and never rejects; `veld test` lists under each example the audited rules it would violate, and an audited rule cannot fail an example or `veld check`; `fmt`, `diff` (adding the annotation is judged like removing the rule), the OpenAPI export, the fuzzer, completion and hover know it; seven conformance cases; the guide has a section "Rolling a rule out". Every generator refuses an audited rule (`E0801` at the first one) through the new feature `rules_rollout`
- [ ] `@rollout(audit)` in the Node server, the TypeScript client, Java and Rust. The vectors wait as `tests/rules/core/rollout.veld.pending`, `rollout.json.pending` (8 cases) and `tests/rules/endpoints/rollout.veld.pending` (11 examples). Gates for that work: (1) the vectors alone do not show that a server reports an audited rule, which is the only thing the feature does, so each target's vector test must record what reaches `onRuleViolation` and compare it with what the reference evaluator reports for the same example, and removing the report call in one target must fail a test; (2) vectors still missing: output mode `off`, and an audited invariant of an error the server encodes
- [ ] An audited invariant of an **event** is outside the `rules_rollout` check today, because no endpoint reaches an event: harmless while no server generates events, wrong the day one does (it would reject a message on a rule that must not reject). The helper that lists the events of a service (see Events) must make such a service use `rules_rollout`, before any generator declares `events`
- [ ] Derived fields: specified in 17.2, held until the owner confirms two visible choices (see "Decisions waiting for the owner"). The proposal names them as the first Phase 5 cut
- [i] OpenAPI `x-veld-rules`, `x-veld-facts`, `x-veld-flows`, `x-veld-finals`, `x-veld-transition`; a contract without rules exports the same bytes as before
- [ ] LSP: hover and completion inside expressions
- [ ] Playground: live rule evaluation (WASM)
- [ ] `veld test --from-violation`
- [ ] Rule benchmark gate in CI

## Phase 6: extras
- [~] Spec: `spec/rules.md` section 19 drafted (what `--coverage` counts and prints, `--fail-under`; what the rule book of `veld export rules` contains; `@pii` and `@secret`: no change on the wire, what generated code must leave out, the lints `L0015` `pii-without-auth` and `L0016` `secret-in-output`). No code
- [ ] `veld test --coverage`
- [ ] `veld export rules`
- [ ] `@pii` and `@secret` redaction, lint for `@pii` without auth

## Events
- [~] Spec: `spec/events.md` (grammar, envelope, publish and consume pipelines, transport, IR, diagnostics `E1001` to `E1010`, phases). The points the first implementation found unclear are written back into it (default topic of a registry package, when the graph checks run, how `expect S.handler` finds `S`, what the reference evaluator does not check, what an event example omits in the IR)
- [i] E1 in the IR, the compiler, the reference evaluator and the tools: `event`, `channel`, `publishes`, `subscribes` and event examples parse, check and lower; the checks `E1001` to `E1010`, `W1001` and `W1002` each have a conformance case; event signatures and the event rules digest; `veld test` and `veld check` run event examples offline; `fmt`, lint, OpenAPI and `diff` know events; the language server has outline, hover and tokens; the editor grammars, `spec/ir.d.ts` and `spec/diagnostics.md` are regenerated. All 11 hand-written vectors (`tests/events/bookings.veld`) pass with no expected outcome edited. A contract without events keeps its IR and its generated bytes
- [i] Until a generator emits events: every server generator refuses a service that publishes or subscribes (`E0801`, through the shared capability check, so a plugin or a new emitter refuses too), and a client generates nothing for events and keeps its bytes
- [ ] Before the generators: the IR records no origin for a `publishes` line (the `E0801` points at the `service` line), and generators need one helper that lists the events of a service and the declarations only events refer to
- [ ] Known limit: the reference evaluator checks no constraints (`@min`, `@length`, `@pattern`) on a written event value, as for call examples, so `expect rejected` for a constraint is not testable offline (`spec/events.md` section 12)
- [ ] Found by the checker, to fix: `veld diff` reports every event change as "possibly breaking", so an added event, channel, publisher or subscriber fails `veld diff --strict`, although `spec/events.md` section 16 calls an added event compatible; and the heading of that list says "a rule changed" when none did. Additions must be compatible now; the full table of section 16 stays with phase E3
- [ ] Found by the checker, small: a doc comment before `publishes` or `subscribes` is accepted by `veld check` with a warning and refused by `veld fmt` as a syntax error; `event F extends B` reports "expected a declaration" with a list that does not name `event` or `channel`
- [i] Checked independently with the real binary (about 40 odd inputs, every rule of sections 3 to 5, the graph checks, signatures and digests, event examples, `fmt` idempotence, the generators' refusal and the client's bytes): no panic, no other difference from the spec. Not covered by that check: the language server, the WASM build, the default topic of a registry package
- [ ] Language server: completion after `carries`, `on` and `given event`; hover with the topic and the key
- [ ] E1 in the generators: CloudEvents envelope and wire goldens; typed publisher and subscriber in Node, Java and Rust; memory transport; the vectors on every target
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
- [~] Spec for P1: `spec/platform.md` drafted (where each of the four runs in the pipeline, that no setting turns them off, what fails closed, the audit event, the limiter interface, codes `E1101`, `W1101`, `E1110`, `E1120`). No code. Seven choices in its section 9 wait for the owner. P2, P3 and P4 are not drafted
- [ ] P1: `@visible`, `@tenant`, `@audited`, `@limit`
- [ ] P2: timed transitions and the scheduler hook; `flag()`; `@timeout`, `@slo`, metrics hook, OpenSLO export
- [ ] P3: sagas (experimental until crash and compensation-failure tests exist), with a needs-manual-intervention state
- [ ] P4: message catalogs (Arabic first), `@retain` and the retention export

## Docs, examples, website
- [i] New example `examples/bookings`: a booking life cycle (a state machine), a signed-in caller and three policies, a Node server with an auth hook, fact loaders and conditional writes, a separate test server, a React app that shows only the moves the server accepts, 38 examples, covered by the example tests and an end-to-end test that also runs the live examples and the fuzzer (the web app is type-checked and driven through the generated client, but was not opened in a browser); a Java server for it is still to do
- [ ] `examples/microservices` gains events
- [~] Guides: `docs/guides/rules.md` and `docs/guides/testing.md` written; events and configuration to do
- [ ] Website: docs pages, landing examples, playground presets, rebuilt WASM
- [ ] Editor grammars (VS Code, JetBrains) regenerated; README and `docs/README.md` index

## Not on this branch (teammates)
AI coding agents (§14 of the plan, including `veld export mcp`), beyond web (§15: Kotlin and Swift clients, the desktop guide), and every emitter other than Rust, Java and Node with its TypeScript client (owner, 25 Nov 2026). A new emitter gets rule support by implementing the `ExprPrinter` hooks and passing the vectors under `tests/rules/`; until it does, it refuses a contract with rules with `E0801`. For events, a client emitter has nothing to generate: it skips event declarations and declares the `events` capability, and until it does it refuses a service that publishes or subscribes with `E0801`. `rules_rollout` is the one feature `Capabilities::all_except` never hands out: an emitter that implements `@rollout(audit)` (a server evaluates and reports the rule and never rejects; a client skips it) adds it with `Capabilities::with(Feature::RulesRollout)`, and a plugin lists `"rules_rollout"` in its handshake. The `new-emitter` template and the plugin scaffold start with no capability, so neither claims it by accident.
