# Brief: sagas in the generators (platform phase P3, experimental)

Three agents get this brief, one per target (Node, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

A `saga post` endpoint is an endpoint whose implementation is generated: a list of calls to other endpoints, each with an optional call that undoes it. Outside the generators this is done, including a reference orchestrator. Your server refuses a saga (`E0801`, feature `sagas`). Build the orchestrator in your target's server. A client sees an ordinary `post` with a required idempotency key and needs nothing new.

**Experimental:** a server gets a saga only with the target option `sagas = "experimental"`. Without the option a contract with a saga is still `E0801` for your server, with a message that names the option.

## Read first

- `spec/platform.md` sections 17 to 21, all of it: 17 the syntax and the checks, **18 what the orchestrator does** (with 18.1, the choices the reference made: the saga id and the keys of step calls, the backoff, the windows of time, attempts, what a restart does), 19 what a saga is not, 21 the gate.
- `spec/rules.md` 11.8 to 11.12 (`@idempotent`: a saga endpoint is implicitly `@idempotent(required: true)`, but **it does not use the `IdempotencyStore`**: the saga store is the record of its key; section 18 says what a request whose key names an existing saga gets).
- The IR: `Endpoint.saga` (`steps[]` with `name`, `call { service, endpoint, args[] }`, `compensate`; `timeout_ms`; `passes`); an argument's `value` is an `Expr` or `{ object: [...] }`; a step's value is read as a fact root, a parameter as a param root. The saga endpoint has `idempotency` with `required: true`, and `SagaFailed` (502), `Conflict` (409) and `ValidationError` (422) among its errors.
- **The reference orchestrator**, `veld_rules::saga`: `run_saga(&SagaInput, &Script) -> SagaRun`, `saga_id`, `step_key`, `BACKOFF_START_MS`, `BACKOFF_MAX_MS`, `SagaState` (the record and its member names, the same in every target), the `Script` format. The script vectors: `tests/rules/sagas/checkout.veld` with `orchestrator.json` (27 cases: attempts, stop and restart after every save, deadlines, a late answer, an unknown step that turns out to have succeeded and is undone, the three replays). The examples: `tests/rules/endpoints/sagas.veld.pending` (14).
- `crates/veld-emit-conformance/src/sagas.rs`.
- **The nearest things your emitter already does:** a server that consumes services gets generated clients for them (`clients = true`; since the runtime config, built with `createClients`): the orchestrator calls its steps through exactly those clients; the idempotency runtime (key syntax, scope, fingerprint, the two conflict messages, the replay header); the expression printer (step arguments are rule expressions over the parameters and earlier steps).

## What to build

1. **The orchestrator**, one per saga, generic over a small generated plan (the steps, their calls and argument builders, the compensations, the timeout, which errors pass through): start, steps in order with a save after every answer, retries with the backoff of 18.1 until the window ends, failing and compensating in reverse order, learning what an unanswered step did, `stuck`, the answer. **Time and waiting are injectable** (a clock and a `sleep` in the options), so a test drives it without waiting and the script vectors can be replayed exactly.
2. **Step calls** go through the clients of the consumed services that the application gave (the hooks: `sagas: { store, clients, onSagaStuck? }` in your target's form), with the application's own credentials, never the caller's. Each call carries the key of 18.1 (`<saga id>.<step>` and `<saga id>.<step>.undo`; the hashed form above 255 characters).
3. **`SagaStore`** (`create`, `save`, `load`, `unfinished`, `claim`) with the state record of the reference, and a generated memory store.
4. **The saga endpoint**: steps 1 to 5 of the pipeline, then the saga store instead of the idempotency store: a new key creates the saga; an existing one answers `409 Conflict` (other fingerprint; still running) or replays the saved answer with `Veld-Idempotent-Replay: true`, `SagaFailed` included. The service interface the application implements has **no method** for a saga endpoint.
5. **`resume<Service>Sagas(hooks, options)`**: the unfinished sagas of the store, each claimed, continued where its saved state ends.
6. **The answer** as section 18 says: the last step's value; a declared error of the saga's own (`passes`) as the step answered it; otherwise `502 SagaFailed` with `step` and `compensated`.
7. The capability `sagas` (server role, with the option), the conformance check, and `E0801` naming the option without it.

## Tests that must exist when you hand back (written and compiled, not run)

- **Every case of `orchestrator.json` through the generated orchestrator** with scripted clients, a fake clock and `sleep`: the calls in order with their arguments and keys, the virtual time of each (`at_ms`), every saved status, the final state and the answer. This is the test that makes three orchestrators one.
- The 14 examples. The live runner skips saga examples in this version, so run them the way the script cases run: a test that builds the script from the example (the reference has the function that does it; say which) and compares the answer and `expect compensated` / `expect stuck`.
- Over HTTP with the memory store and two small generated step services in the same test (real requests between them): a saga that completes; one that fails at each step and is compensated; a compensation that keeps failing ends `stuck` and calls the hook; the same key again replays; another body with the same key is `409`; the step services see the keys of 18.1 and the orchestrator's credentials, not the caller's.
- **Stop and resume** (the gate of section 21, with the memory store): the orchestrator is stopped after every save, `resume` continues, and every run ends `completed` or `compensated` with each step's effect present once or undone once; two resumers on one store do not double an effect.
- Snapshots of the generated files; the conformance check.

## Your report

As `brief-in-the-generators.md` says, plus: the generated names in your target, how time is injected, which of the 27 script cases you expect trouble with and why, and whether the gate of section 21 is fully written as tests.
