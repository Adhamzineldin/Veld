# Brief: platform phase P2 in the generators

Three agents get this brief, one per target (Node with TypeScript, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

Four features of `spec/platform.md` sections 10 to 16 are implemented outside the generators, each behind its own opt-in feature that your generator refuses today (`E0801`): **timed moves** (`rules_timed`), **flags** (`rules_flags`), **deadlines** (`timeouts`) and **service levels** (`slo`). Build them in your target, in that order, each as its own series of commits with its own capability, so that a stop in the middle leaves whole features.

## Read first

- `spec/platform.md` sections 10 to 16 (and 1 and 6), with what the compiler work added to them (10.x on the constant fragment and the guard, 11.1 the `diff` table, the `Veld-Test-Flags` bullet in `spec/rules.md` 12.4, 13.1 the OpenSLO shape).
- The IR: `Flow.after_ms`, `Flow.flag`, `EndpointTransition.since`; `flag` on `Rule`, `EndpointRequire`, `EndpointEnsure`, `PolicyClause`; `Example.flags`; `Endpoint.timeout_ms`; `Endpoint.slo`; `Ir::flag_usage()`.
- The reference: `veld_rules::timed_endpoints(&ir)`, `timed_moves(&Enum)`, `FlagState::{On, ProviderFailed}` and `.resolve(&ir)`, the guard in `crates/veld-rules/src/machine.rs`; its tests in `crates/veld-rules/tests/platform_p2.rs`.
- The vectors (all `.pending`): `tests/rules/endpoints/timed.veld.pending` (8 examples), `tests/rules/core/flags.veld.pending` with `flags.json.pending` (case keys `flags` and `provider: "failed"`), `tests/rules/endpoints/flags.veld.pending` (14 examples).
- `crates/veld-emit-conformance/src/platform_p2.rs`: the "refused" checks.
- **The nearest things your emitter already does:** a `transition` (its guard, the `409`, the typed value the handler gets, `nextStates`) for timed moves; an audited rule (`@rollout(audit)`: a rule that is evaluated or skipped by a property of the rule, on server and client) for flags; the idempotency hooks (a required interface in the hooks with a generated memory implementation, a failure at creation when it is missing) for the flag provider and the due loaders; `onRuleViolation` (a hook called from the pipeline with a plain event) for the metrics hook.

## 1. Timed moves (`rules_timed`)

- **The guard.** A transition with `since` also requires `now - since >= after_ms`, where `now` is the request's clock reading. Not due answers `409 InvalidTransition` with the usual `field`, `from`, `to`; the violation's message is `violates rule transition: not due`. A `since` in the future or one that cannot be read is not due. Exactly `d` is due; one millisecond less is not. The transition id comes from the IR.
- **The runner** (10.1): `<Service>DueLoaders` (one method per endpoint with a timed transition: `before`, `limit`, the context; it answers the parameters of the calls that are due), `run<Service>DueMoves(impl, hooks, options)` (per endpoint: `before = now - after_ms`, ask the loader, run every answer through steps 4 to 10 of the pipeline in process as a call of that endpoint, with the caller `hooks.timer.principal()` in place of step 2; options `now`, `limit` default 100; a report per endpoint of calls run, moved, refused with `InvalidTransition`, failed; one failure does not stop the others and goes to `onError`). A server whose timed endpoint has auth and that has no `timer.principal` hook does not start the run. Name the pieces as 10.1 does, in your target's casing.
- **`timedMoves()`** on a state machine with timed edges, next to `nextStates`, in server and client: the source, the target and the milliseconds of each.

## 2. Flags (`rules_flags`)

- **The provider:** `flags` in the hooks, an interface with `isOn(name, context)` (context: the principal or none, the caller's tenant text or none, the operation signature); required for a server whose contract uses a flag; a generated memory provider for tests.
- **One reading per request per flag**, at the first statement that has it, used until the response is sent.
- **Off:** the statement is not there (not evaluated, not reported; a flagged flow edge is not an edge for the guard: the guard requires the edge to exist). **On:** an ordinary statement.
- **A failing provider:** an invariant, a `require`, an `ensure` and a `forbid` count as on; a `permit` and a `flow` count as off; the failure goes to `onError`.
- **Clients:** skip flagged invariants in the local check and in `.partial` (the digest in the IR already leaves them out); `can<Endpoint>` and the policy functions take an optional last argument, a function from a flag name to a boolean, and without it decide as a failing provider does; `nextStates` lists flagged edges too.
- **Events:** a flagged invariant of an event is evaluated by publisher and consumer with a context without a principal (the publisher and subscriber options take the provider).
- **The test handler** reads `Veld-Test-Flags` (unpadded base64url of a JSON array of the flags that are on); without the header every flag is off.

## 3. Deadlines (`timeouts`)

- **Server:** the request context carries `deadline` (arrival plus `timeout_ms`) and your target's usual way to notice it (an `AbortSignal` in TypeScript; `ctx.deadline()` and `ctx.expired()` in Java; a cancelled handler future in Rust). When the pipeline has no answer at the deadline the server answers `504 DeadlineExceeded` with `timeoutMs`. Where the language cannot stop a running handler it runs on and its answer is dropped: say so in the handler's documentation. A request that hit its deadline releases its idempotency key (a `5xx` is not stored). `DeadlineExceeded` is in the endpoint's errors in the IR.
- **Client:** a method of such an endpoint uses the deadline as its default timeout when the caller sets none, and fails with the client's existing timeout error.

## 4. Service levels (`slo`)

- **The metrics hook** `metrics(m)` in the hooks of a server with an `@slo`, optional, called once for every request that matched an endpoint of that server, after the response is written, with exactly the value of section 13 (the names of the OpenTelemetry HTTP conventions; `veld.operation` is the endpoint's full operation signature; `error.type` only for a `5xx`). A failing hook is ignored. Declaring the capability means: the hook is called.

## Gates, as they apply here

Four capabilities, each declared when its part is done. A contract that uses none of the four generates the bytes it generated before. The vectors keep their `.pending` names.

## Tests that must exist when you hand back (written and compiled, not run)

- Timed: the 8 examples through your generated test handler; over the real entry point the boundary (exactly due, one millisecond early, `since` in the future); the runner with a memory loader: a due move runs through the pipeline and reaches the handler with the timer's principal, a move that another request made first is counted as refused and not as failed, a failing handler does not stop the others, the report; a run without the timer principal on an endpoint with auth.
- Flags: the core cases (with `flags` and `provider: "failed"`) through your vector runner and the 14 examples through the test handler with `Veld-Test-Flags`; over the real entry point: on, off, the provider failing for a statement that restricts and for one that grants, one reading per request (a provider that counts its calls), a flagged flow edge off; the client's local check and `can<Endpoint>` with and without the function; a server created without the provider.
- Deadlines: a handler that answers in time; one that does not (the `504`, its body, the signal or cancellation observed by the handler, the late answer dropped, the idempotency key released); the client's default timeout.
- Service levels: the metric of a `200`, of a declared error, of a `500`; a failing hook is ignored; a server without `@slo` has no hook in its type.
- Snapshots of a small contract with all four; the conformance checks.

## Your report

As `brief-in-the-generators.md` says, one part per feature, plus the generated names of each part in your target.
