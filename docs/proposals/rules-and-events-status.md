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
- Events groundwork (26 Nov, approved by the owner): five conformance snapshots that were created on this branch changed by one thing each, a service's `publishes` entry, which was a bare channel number and is now `{ "channel", "origin" }`: `ok_events_bookings`, `ok_events_two_packages`, `ok_rules_rollout`, `w1001_unsubscribed_channel`, `w1002_unhandled_event`. No snapshot from before the branch changed.
- Workspace layout (26 Nov): `.github/workflows/ci.yml`, a file from before the branch, changed in its determinism job only: the one `veld.toml` it wrote in the earlier format is now four files of the new layout (a root, two services, one app), with the contracts, `out/` and the lock file in the same places, so the job's two comparisons are unchanged. Every example's `veld.toml` and README changed with the layout; no generated file did.
- Workspace layout (26 Nov), two snapshots from before the branch, stored by the main session under the owner's standing permission of that day: `apps/veld/src/commands/init/snapshots/…summary_snapshot.snap` (`files 5 to create` is now `9`) and `…full_flow_transcript_snapshot.snap` (6 files became 10: the file list shows one `veld.toml` per member, `services/users`, `services/billing`, `clients/java`, `web`, and the contracts under `services/<service>/contracts/`; the next step reads `edit services/<service>/contracts/`). Nothing else in either file changed.
- `check --prove` (26 Nov): `.github/workflows/ci.yml` gained one step in the test job, Linux only (`sudo apt-get install -y z3`), and one line in the test step's environment (`VELD_REQUIRE_SOLVER` is `1` on Linux), so the solver tests must run there and are skipped with a message on macOS and Windows. `spec/diagnostics.md` gained the two lines of `E0970` and `W0970`. No snapshot changed.
- The `auth-required` decision (see "Decided by the owner"): files from before the branch that gained one `@auth(none)` line are `examples/hello/contracts/hello.veld`, `examples/rust-catalog/contracts/books.veld`, four contract templates in `apps/veld/src/commands/init/render.rs`, two website presets and three website snippets. No generated source file changed; the two examples' `veld.lock.ir.json` moved by one line number each. No insta snapshot changed.
- The three bug fixes (26 Nov, approved by the owner on 25 Nov), four snapshots from before the branch, stored by the main session after reading every changed line: `spring_server_gen__a__adapters__orders__OrdersController.java` (8 mapping lines now map the path and the path with `/`), `streaming_spring_gen__s__adapters__live__LiveController.java` (3 such lines), `rules__rules_order_java` (2 lines: `Rules.instantMs` is `Rules.toDatetime`, `Rules.len` is `Rules.lenString`) and `endpoint_rules__endpoint_rules_BookingsHandler` (1 line, `Rules.toDatetime`). Nothing else changed in them. The Rust SSE fix changed no stored file.
- The `@readonly` request-body fix (26 Nov): committed generated output of two examples changed, regenerated by the main session. `examples/microservices`, Java transaction service: `runtime/Router.java` (4 lines: a request body and a query object get placeholders for required read-only fields before decoding), `runtime/Wire.java` (121 lines added: the placeholder code, moved there) and the manifest's two hashes. `examples/rust-catalog`, client and server: `src/veld/schema.rs` (50 lines added in each: the same placeholder for a required read-only field in request mode) and each manifest's hash. No contract, no hand-written file and no other generated file changed.

## Decisions waiting for the owner

- **Keep the `plink` window open** (title "ZBOOK"; the owner logged in on 26 Nov at 00:32). The zbook is reachable through it: Kafka and RabbitMQ images are there for events phase E2, and a Z3 4.8.12 runs in the image `veld-z3:bookworm`, built there on 26 Nov from `debian:bookworm-slim`, for `check --prove`. If the window is closed, both stop until it is opened again.
- **Five temp files of a checker run** are still in `%TEMP%` (`vt.log`, `vb.log`, `cli.log`, `node.log`, `alldone`): the checker's own delete was refused by a permission check, so they were left for you.
- **A stray empty file, `C:\Program Files\Git\scratch_dummy`**, was created by an agent's mistaken command on 26 Nov; its own delete was refused by a safety check, so it is left for you to remove.
- **A subagent emptied this session's scratch folder** on 25 Nov while cleaning up after itself: about 200 scratch files of the session (helper scripts, logs, commit-message drafts, an old copy of the plan page). Nothing of the repository and nothing that was the only copy of a deliverable was in it: the plan page's source is kept elsewhere and is intact. Each agent now gets its own scratch subfolder and may delete only that.
- **Built as drafted, not asked one by one.** Say so if you want any of them changed:
  - `@rollout(audit)` (`spec/rules.md` 17.1): a generator supports the feature only when it says so, so no emitter can enforce an audited rule by accident; a service whose invariants are all audited has no rules digest, so its clients check nothing locally; `@rollout` on a `fact`, an `allow`, a `transition`, a policy, a principal or an example is `E0602`; `veld test` lists the audited rules an example would violate, and `veld check` does not; in `veld diff`, a rule that is new and audited, or that changes while audited, is compatible.
  - Workspace layout (`spec/workspace.md` section 16): defaults are written per role (`[workspace.defaults.server.java]`), one level deeper than the proposal's sketch; a project that is one file must write the service's `name`.
  - Typed runtime config (`spec/runtime-config.md` section 9): the five choices other than how a client gets its URL.
  - Platform P1 (`spec/platform.md` section 9): the loaders keep their signature, and the generated check after loading is what the contract guarantees.
  - Derived fields (`spec/rules.md` 17.2): in the type a handler returns, a derived field is optional and whatever the handler puts there is overwritten.
  - The policies feature covers more than `allow`: a contract whose `require` or `ensure` reads the principal uses `rules_policies` even without an `allow`, so a generator that cannot supply the principal refuses it. `spec/rules.md` 11.4 is widened to say so.
- **`Auth.declared` is not in the IR JSON.** The IR cannot tell an explicit `@auth(none)` from no `@auth`; adding a key would have changed every stored IR snapshot. The flag exists in memory only, for the lint, and a plugin or a tool that reads `ir.json` does not see it. Making it part of the IR is an `ir_version` change.

## Decided by the owner on 25 Nov 2026, in the evening

Asked as twelve questions; every answer was the recommended one.

- **Typed runtime config, how a client gets its URL:** the client keeps its `baseUrl` option, which becomes required, and a generated `createClients(runtime)` builds every client from the checked config. The `VELD_<SERVICE>_URL` fallback goes.
- **Today's single `veld.toml`:** an error with a pointer to what to write instead. No converter, and the two layouts are not accepted side by side.
- **Platform P1, when the limiter or the audit hook itself fails:** the service stays up. A failing limiter lets the call through, a failing audit hook does not fail the request, and both go to `onError`.
- **Derived fields:** an expression after `=` makes a field derived; a literal after `=` is a default. No keyword.
- **`@version`, a stale `If-Match`:** `412 PreconditionFailed`.
- **`@idempotent`, who a key belongs to:** a text the application returns from a server hook. A server that needs it and has no hook does not start.
- **Events, a message that fails validation or a rule:** acknowledged and reported (dead letter when configured, a hook otherwise), so it cannot block a partition.
- **`veld generate` and a changed rule:** it stops and asks, like for other breaking changes. Done (26 Nov): the gate uses `Severity::fails_check`, so a possibly breaking change (a changed rule, a non-additive change to an event) stops it and `--accept-breaking` covers it; its message says the tool cannot tell the direction. An added event, channel, publisher or subscriber does not stop it.
- **Three bugs from before the branch are to be fixed (done on 26 Nov; the changed snapshots are listed under "Existing snapshots that changed"):** the Spring controller and a trailing slash (`POST /bookings/` is a 404 on Spring and works on the JDK server); the Rust crate that uses SSE and does not compile when one service has no SSE endpoint; the two Java helper names (`instantMs`, `len`).
- **The rest, as drafted:** `@retry` is set on the channel; `@limit(10, per: minute, by: user)`; a `@tenant` field is set by the server and never sent by the caller; no caller sees every tenant; `@visible` is refused on events; the CI Maven cache (`c81c139`) stays; the workspace keeps one `veld.lock.ir.json`.
- **How the work runs:** up to three agents at once (two that compile, one for docs and the website), although that slows the other project's builds on this machine. After a merge only the crates that changed are run locally; the whole suite is the agent's run in its worktree and CI on three systems.

## Decided by the owner on 25 Nov 2026, earlier

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
- [t] Java helper names follow the TypeScript reference list: the printer writes `toDatetime` and `lenString`; `instantMs` and `len` stay in the runtime as aliases (the change touches 4 places on 3 lines of two snapshots, `rules_order_java` and `endpoint_rules_BookingsHandler`)
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
- [~] `@version` optimistic concurrency (`ETag`, `If-Match`, `412 PreconditionFailed`) outside the generators: language, checks, IR, reference evaluator, `veld test` (`ifMatch 3`, `ifMatch any`), `fmt`, `diff`, OpenAPI, language server, 7 conformance cases and the vectors `tests/rules/endpoints/version.veld.pending` (19 examples). `rules_version` is opt-in and every generator refuses it with `E0801`. Not yet: generated code (the guard, the `ETag`, the client's `ifMatch`) in any target
- [~] `@idempotent` outside the generators: language, checks, IR, reference evaluator (the lookup after step 5, replay, `409 Conflict`, what is stored), `veld test` (`key "k-1" stored ...`, `other`, `running`), `fmt`, `diff`, OpenAPI, language server and the vectors `tests/rules/endpoints/idempotency.veld.pending` (21 examples). `idempotency` is opt-in and every generator refuses it with `E0801`. Not yet: the `IdempotencyStore` interface, the caller hook and the `Veld-Test-Idempotency` header in a generated server. The choices made where the text was silent are in `spec/rules.md` 11.10
- [i] Lint `auth-required` (`L0014`; `L0005` was taken): every endpoint has auth or `@auth(none)`

## Phase 5: proof and lifecycle
- [~] Spec: `spec/rules.md` sections 17 (`@rollout(audit)`, derived fields, `veld test --from-violation`) and 18 (`check --prove`: the questions, the four results, the modelled fragment, the solver) drafted. Only 17.1 has code
- [i] `veld check --prove` and `veld diff --prove` (`spec/rules.md` 18; the choices made are in 18.4a): a new crate `veld-prove` encodes the questions as SMT-LIB 2 and runs an external solver (`--solver`, `VELD_SOLVER`, then `z3` and `cvc5` on `PATH`; `--prove-timeout`); `E0970` for a model no value can satisfy, a `require` that can never pass and a policy that permits nobody, each naming the rules that conflict; `veld diff --prove` says "tightened" with a value the old rule accepted and the new one rejects, or "not tightened" and proved; a value from the solver is shown only after the reference evaluator confirmed it; four results, `W0970` for unknown and unsupported; without a solver one line says so and the normal checks run. The encoder is checked against the reference evaluator on every rule of the vector contracts (250 checks, 186 exact and 64 one-sided where a part is left open). Tested here with Z3 4.8.12 on the zbook
- [ ] `--prove`, open: cvc5 was never run, only its options are written; the invariants of nested values and of list items are not part of the first question; `veld diff --prove` covers invariants and `require`, not policies, `ensure` or state machines; a list or map of more than 256 items, or a string that needs `@pattern` or `@format`, gives "unknown"
- [i] Rule-aware `veld diff`: invariants, `require`, `ensure`, facts, flows and transitions are classified; with no solver yet, a changed expression is "possibly breaking" and shows the old and the new text; `--strict` fails on it
- [i] `@rollout(audit)` outside the generators (`spec/rules.md` 17.1): the annotation on an invariant, a `require` and an `ensure`; `"rollout": "audit"` in the IR, omitted when the rule is enforced, so a contract without it keeps its IR; the rules digest of a service and of an event covers only enforced invariants; the reference evaluator evaluates an audited rule, reports it and never rejects; `veld test` lists under each example the audited rules it would violate, and an audited rule cannot fail an example or `veld check`; `fmt`, `diff` (adding the annotation is judged like removing the rule), the OpenAPI export, the fuzzer, completion and hover know it; seven conformance cases; the guide has a section "Rolling a rule out". A generator that does not declare the new feature `rules_rollout` refuses an audited rule (`E0801` at the first one); the four built-in generators (`node`, `typescript`, `java`, `rust`) declare it and implement the rule, in both roles, with the server vectors, a live comparison of `Veld-Test-Audited` and the generated test handlers
- [i] `@rollout(audit)` in the Node server, the TypeScript client, Java and Rust: an audited rule is evaluated where the enforced one would be, reported to `onRuleViolation` with the outcome `audited`, and never rejects; an audited `require ... else E` never builds its payload; clients skip audited rules. The vectors are live (`tests/rules/core/rollout.*`, `rollout_off.*` for output mode `off`, `tests/rules/endpoints/rollout.veld` and `rollout_error.veld` for an error the server encodes). Each server target runs the core vectors through its generated handler and compares what reaches the hook with the vector's `audited` list; `veld test --live` compares the test handler's `Veld-Test-Audited` header with the reference evaluator. Removing the report call was tried in each target and failed tests in each. Fifteen new snapshots (Node 6, Java 5, Rust 4), stored by the main session
- [ ] `@rollout(audit)`, not covered by a test that runs: WebSocket items and messages on the Java and Rust servers (Rust compiles and lints the endpoints; the JDK adapter cannot serve WebSocket); on Spring, anything but the live examples of `rollout.veld`. `veld test --live --fuzz` does not compare the audited header, as specified
- [i] Found by that work and fixed: a request body whose model has a required `@readonly` field was accepted by the Node server (as `spec/language.md` section 7 says), answered `500` by the Java server and `422` by the Rust server; `master` has the same code, so the bug is older than the branch. Java and Rust now decode such a body with or without the field and ignore a value the caller sent; a test per target sends both forms, and the two vector tests no longer relax their status check
- [i] An audited invariant of an **event** counts: a service that publishes or subscribes to a channel which carries such an event (the rule on the event or on a declaration the event reaches) uses `rules_rollout`, and the `E0801` points at the rule. A client, which generates nothing for events, is not concerned
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
- [i] Before the generators: every `publishes` line has an origin in the IR, `Ir::events_of` and `Ir::event_reachable_decls` list the events of a service and what they reach, and an audited invariant of an event makes the service use `rules_rollout`
- [ ] Known limit: the reference evaluator checks no constraints (`@min`, `@length`, `@pattern`) on a written event value, as for call examples, so `expect rejected` for a constraint is not testable offline (`spec/events.md` section 12)
- [i] Found by the checker and fixed: `veld diff` reported every event change as "possibly breaking", so growing a stream failed `veld diff --strict`. Additions are now compatible (a new event, a new channel, an event a channel newly carries, a new `publishes` line, a new subscription, a new `on` line); every other event change stays "possibly breaking" until the table of `spec/events.md` section 16 is built with phase E3. The heading of that list no longer says "a rule changed"
- [i] Found by the checker and fixed: `event F extends B` now says that an event has no `extends`; `event F` or `channel C` without its `{` is reported as that declaration; a `publishes` or `subscribes` with no channel name asks for one; `veld fmt` says why it leaves a file alone when the only reason is a doc comment attached to nothing (a doc comment before `publishes` or `subscribes` is one: those lines take none, by the grammar)
- [ ] Left as it is: after a response that is only a status, an unfinished `publishes` or `subscribes` on the next line is still read as the type of the response (`E0205 unknown type`). The spec switches to a new line only when an identifier follows, so that either word stays usable as a type name
- [i] Checked independently with the real binary (about 40 odd inputs, every rule of sections 3 to 5, the graph checks, signatures and digests, event examples, `fmt` idempotence, the generators' refusal and the client's bytes): no panic, no other difference from the spec. Not covered by that check: the language server, the WASM build, the default topic of a registry package
- [i] Language server: completion of events after `carries`, `on` and `given event`, of channels after `publishes` and `subscribes`; hover on a channel shows the topic, the key and the signatures of its events. Limits: channel hover needs a contract without errors and does not follow a `use` alias
- [i] The reference envelope (`veld_rules::envelope`: encode, decode, the consume outcomes) and its byte goldens in `tests/wire/events/` (14 publish cases, 17 consume cases, an edge-case contract, a README that says how a generator's test uses them). What the reference chose where the spec was silent is `spec/events.md` 6.3
- [i] E1 in the Node generator: for every service that publishes or subscribes, a typed publisher (`create<Service>Publisher`) and a typed subscriber (`subscribe<Service>Events` with a `<Service>EventHandlers` interface), the envelope, the publish and consume pipelines, the `Transport` interface and a memory transport; the same event files for express, fastify, hono, node-http and nest; a contract without events keeps its bytes. The generated code reproduces every byte golden of `tests/wire/events` and passes the event examples over the memory transport; removing the audited report fails a test. What it settled for the other targets is `spec/targets/typescript.md` 5f and `spec/events.md` 6.4, 8.1, 9 and 14. The features of event fields (unions, constraints and the rest) now count for a server that publishes or subscribes; a client still generates nothing for events. Seven new snapshots, stored by the main session
- [ ] E1 in Node, open: no counter for ignored messages (the spec says "counted"); the test handler (`testing = true`) does not cover events
- [ ] E1 in the Java and Rust generators, each reproducing the same byte goldens and mirroring the Node names
- [ ] E2: Kafka and RabbitMQ adapters per language; retry and dead letter; inbox; broker capability matrix; Docker toolchain tests
- [ ] E3: outbox and relay for `emits` (at-least-once, stated as such); event-aware `diff`; AsyncAPI export; crash-injection tests

## Config
The work is two series, because only the second changes generated code. The layout series must leave every generated file, manifest and lock file of every example and fixture the same bytes; the runtime config series changes client code for every contract, and those snapshots go on the owner's list.
- [i] Spec for the layout: `spec/workspace.md` is implemented; its section 17 lists the choices the implementation made where the text was unclear
- [~] Spec for the typed runtime config: `spec/runtime-config.md` drafted (what a setting is, one `<Target>Runtime` type per target, the check that lists every problem at startup, `fromEnv` with names the application chooses, what is removed, `veld export runtime-schema`, and which generated files change). No code. The owner confirmed how a client gets its URL; read it before it is built, because it changes the constructor of every generated client
- [i] Workspace root plus one `veld.toml` per member, and the single-file form; a file in the earlier format is the error `E0709`, which says what to write instead (errors `E0709` to `E0712`)
- [i] `-p`/`--package`, `--workspace`, the current member, `--affected --since <ref>` with its reasons, `owners` in `veld diff` and `veld check --graph=json`
- [ ] Typed runtime config per target; `VELD_<SERVICE>_URL` fallback removed; `fromEnv()` opt-in
- [ ] `veld export runtime-schema`
- [i] `veld init` (every scaffold), all five examples, every test fixture, the editor schemas and templates, and the docs moved to the new layout (new guide `docs/guides/workspaces.md`). **No generated byte changed**: for all five examples every generated file, manifest and `veld.lock.ir.json` is identical before and after, and the 145 parity cases (examples and the six presets) have the same hashes
- [i] CLI and WASM share one target-resolution function; the parity test resolves the same files both ways and compares targets, packages and bytes
- [ ] Layout, left open: the browser playground still passes the WASM build its earlier JSON shape and does not call the new `resolveWorkspace`; a plugin emitter's defaults are checked only when used, not at load; `veld doctor` does not print member counts; the READMEs that generators write still say "the project that holds veld.toml"; the JetBrains (Kotlin) tests were edited and not run locally

## Platform
- [~] Spec for P1: `spec/platform.md` drafted (where each of the four runs in the pipeline, that no setting turns them off, what fails closed, the audit event, the limiter interface, codes `E1101`, `W1101`, `E1110`, `E1120`). No code. Seven choices in its section 9 wait for the owner. P2, P3 and P4 are not drafted
- [ ] P1: `@visible`, `@tenant`, `@audited`, `@limit`
- [ ] P2: timed transitions and the scheduler hook; `flag()`; `@timeout`, `@slo`, metrics hook, OpenSLO export
- [ ] P3: sagas (experimental until crash and compensation-failure tests exist), with a needs-manual-intervention state
- [ ] P4: message catalogs (Arabic first), `@retain` and the retention export

## Docs, examples, website
- [i] New example `examples/bookings`: a booking life cycle (a state machine), a signed-in caller and three policies, a Node server with an auth hook, fact loaders and conditional writes, a separate test server, a React app that shows only the moves the server accepts, 38 examples, covered by the example tests and an end-to-end test that also runs the live examples and the fuzzer (the web app is type-checked and driven through the generated client, but was not opened in a browser); a Java server for it is still to do
- [ ] `examples/microservices` gains events
- [i] Guides: `docs/guides/rules.md`, `docs/guides/testing.md`, `docs/guides/workspaces.md` and `docs/guides/events.md` written. The events guide covers what is built (the language, `veld test`, the Node generator, the memory transport, the envelope, `veld diff`) and lists what is not; every snippet in it was run with the binary, and its program was compiled with `tsc --strict` and run. A guide for the typed runtime config comes with that work
- [i] Website, for the rules features that are built: a "Business rules" group of six docs pages (overview and invariants, endpoint rules and output mode, state machines, policies, testing, tools); the language, handler, configuration, diagnostics and CLI pages updated (the diagnostics table had been dropping every lint row, so `L0014` never showed); a rules section on the landing page; a `bookings` playground preset; code blocks colour the rule words. Every snippet comes from `examples/bookings`, the guides, or was checked with the binary. The playground's WebAssembly was rebuilt for the checks (143 targets byte-identical to the CLI, six presets match); it is a build product and is not committed
- [ ] Website, still to do: pages for what is not built yet (`@rollout(audit)` in generated code, `@version`, `@idempotent`, derived fields, events, the workspace layout, the runtime config, the platform features) as each lands; Rust rows in the pages that list targets
- [i] Editor grammars (VS Code, JetBrains) are regenerated with each new piece of syntax; the README has a rules paragraph and `docs/README.md` lists `spec/rules.md` and `spec/diagnostics.md`

## Not on this branch (teammates)
AI coding agents (§14 of the plan, including `veld export mcp`), beyond web (§15: Kotlin and Swift clients, the desktop guide), and every emitter other than Rust, Java and Node with its TypeScript client (owner, 25 Nov 2026). A new emitter gets rule support by implementing the `ExprPrinter` hooks and passing the vectors under `tests/rules/`; until it does, it refuses a contract with rules with `E0801`. For events, a client emitter has nothing to generate: it skips event declarations and declares the `events` capability, and until it does it refuses a service that publishes or subscribes with `E0801`. `rules_rollout` is the one feature `Capabilities::all_except` never hands out: an emitter that implements `@rollout(audit)` (a server evaluates and reports the rule and never rejects; a client skips it) adds it with `Capabilities::with(Feature::RulesRollout)`, and a plugin lists `"rules_rollout"` in its handshake. The `new-emitter` template and the plugin scaffold start with no capability, so neither claims it by accident.
