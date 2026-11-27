# Brief: `@version` and `@idempotent` in the generators

Three agents get this brief, one per target. The message that started you says which target is yours:

- **Node**: `crates/emitters/node` (the Node server, every adapter it has: express, fastify, hono, node-http, nest) and `crates/emitters/typescript` (the TypeScript client and the shared runtime).
- **Java**: `crates/emitters/java` (client, JDK server, Spring adapter).
- **Rust**: `crates/emitters/rust` (client and axum server).

Work only in your own emitter, its tests, its toolchain test files under `tests/toolchain/<your target>/`, and your target's spec file. The other two agents work in the other emitters at the same time; never touch their crates. Read `common.md` first: it is the law of this machine.

## Goal

Optimistic concurrency (`@version`: `If-Match`, `ETag`, `412 PreconditionFailed`) and idempotent endpoints (`@idempotent`: the key header, the store, replay, `409 Conflict`) are implemented in the compiler, the IR, the reference evaluator and the tools. No generator supports them: each refuses a contract that uses either with `E0801`, because none declares the capabilities `rules_version` and `idempotency`. Build both in your target, server and client.

## Read first

- `spec/rules.md` sections **11.7 to 11.12**. 11.9 and 11.10 are the exact behaviour, 11.12 the names of the generated code that all three targets share. Also 5.3 (the server pipeline), 12.4 (the test handler) and 13 (the IR).
- `spec/ir.md` section 3.5: `Field.version`, `EndpointRules.version`, `Endpoint.idempotency`, `Ir::etag_field`, the implicit errors.
- The reference: `crates/veld-rules/src/version.rs` (`parse_if_match`, `etag`), `crates/veld-rules/src/idempotency.rs` (`fingerprint`, `Scope`, `is_valid_key`, the two conflict messages), `crates/veld-rules/src/pipeline.rs` (where each step runs), and the goldens in `crates/veld-rules/tests/version_idempotency.rs`.
- What a live run sends and checks: `crates/veld-testing/src/conditional.rs`.
- The vectors: `tests/rules/endpoints/version.veld.pending` and `tests/rules/endpoints/idempotency.veld.pending`.
- `crates/veld-emit-conformance/src/version.rs`: `check_version_is_refused`, `check_idempotency_is_refused` and the contracts they compile.
- **How your emitter already does the nearest thing**, before you write anything: follow a `transition` from the IR to the generated server (the guard in step 6, the typed value the handler receives, the `409`, the hook call), to the client, to the test handler and to its tests. A version guard is built the same way and sits right before it. Follow a `@header` parameter to see where the client puts a header.

## What an implementation does

Everything in `spec/rules.md` 11.9, 11.10 and 11.12. The parts that are easy to get wrong:

**`@version`, server**

- `If-Match` is parsed in step 5 on an endpoint with a guard: `*`, or one strong tag holding a version (`0`, or `1-9` then digits, within `long`). Anything else is a `422` issue at `header.If-Match` with the exact code and message of 11.10, header issues first, no `violations`. An endpoint without a guard ignores the header.
- The guard runs in step 6 after the policies and before the transition guard. A mismatch is `412 PreconditionFailed` with `expected` and `actual` as decimal strings and one violation (the id is `EndpointRules.version.id`; `path` `request`, no fields, message `violates rule version`). An absent optional fact passes.
- The handler gets `version` only when a tag was sent and compared.
- `ETag: "<version>"` on a success whose type is the model itself (`Ir::etag_field`), whatever the 2xx status, and on a replayed success.
- `validate = false` switches none of this off.

**`@idempotent`, server**

- The key is checked in step 5 (1 to 255 visible ASCII; a required key that is missing), with the exact issues of 11.10.
- The lookup is after step 5 and before any fact is loaded. The fingerprint is the first 16 hex characters of the SHA-256 of the RFC 8785 JSON of the bound parameters in wire form (defaults applied, absent optionals left out; the key's own header is not part of it). Your target must compute the same fingerprints as the goldens of the reference. Use what the platform has (`node:crypto` or WebCrypto, `java.security.MessageDigest`, a crate your generated server already depends on or a small SHA-256 in the runtime file): **no new runtime dependency**.
- `started`: run, then `complete` below 500, `release` otherwise. `replay`: the stored status, body bytes, content type, `ETag` and `Location`, plus `Veld-Idempotent-Replay: true`, with no fact loaded, no rule evaluated and the handler not called. `running` and `other`: `409 Conflict` with the exact message for each.
- The scope text and the caller hook are as 11.12 says. A server that lacks the store or the caller hook it needs fails when it is created.
- A store that fails: as 11.12 says.
- The test handler seeds a fresh memory store per request from `Veld-Test-Idempotency`.

**Client**

- `ifMatch` and `idempotencyKey` as 11.12 says.

**Gates**

- A contract without the annotations generates the same bytes as before. No existing snapshot may change.
- Zero runtime dependencies beyond what your target already has. Deterministic output.

## How the capability is declared

Add `Capabilities::with(Feature::RulesVersion)` and `Capabilities::with(Feature::Idempotency)` for each role (server, client) once that role is done. In your emitter's conformance test, replace the calls to `check_version_is_refused` and `check_idempotency_is_refused` by checks that the same contracts generate. The helpers for that (`check_version_is_generated`, `check_idempotency_is_generated`) do not exist: if another agent has not added them when you get there, add them to `crates/veld-emit-conformance/src/version.rs` next to the "refused" ones, small and additive, and say so in your report. All three agents may add the same two functions; keep them to the obvious shape (same contracts, expects no diagnostics and at least one generated file) so that the copies merge.

## The vectors stay `.pending`

Do **not** rename the two vector files. The other targets' runners read every file of that folder, and they are not all done at the same moment. Your tests read `version.veld.pending` and `idempotency.veld.pending` by name. The main session renames them when all targets are in.

## Tests that must exist when you hand back (written, compiled, not run)

1. **The vector examples through your generated test handler**, by `veld_testing::Live`, as your target does for `transitions.veld` and `policies.veld`: every example of the two vector files, no expected outcome edited.
2. **Over HTTP against the production handler**, a test program per feature, as `state_machines_http` does for transitions:
   - version: a tag that matches, a stale tag (`412` and its body), `*`, no header, each malformed form of 11.10 (`422`), the `version` the handler receives in each case, the `ETag` of a success, the order of the checks (policy, then version, then transition), an endpoint without a guard that ignores `If-Match`;
   - idempotency: a new key runs and stores; the same key and parameters replays (same status, body, `ETag`; the replay header; the handler ran once; no fact loaded); other parameters `409`; a second request while the first is running `409`; a declared error is stored and replayed; a `500` is not stored and the next request runs; a required key that is missing and a malformed key (`422`); two callers with the same key do not replay each other; a server created without the store, and one without the caller hook where it is needed, fail at creation; a store whose `begin` fails gives `500` and the handler does not run.
3. **The fingerprint goldens** of the reference, computed by your generated runtime.
4. **The client**: the headers a call sends for `ifMatch` (a version and `*`) and `idempotencyKey`; a required key is required by the type.
5. **Snapshots** of the generated files of a small contract with both annotations (server, client, test handler), and a test that a contract without them generates the bytes it generated before (the existing snapshots are that test: none may change).
6. The conformance checks above.

## Write down what you establish

Your target's spec file (`spec/targets/typescript.md`, `java.md` or `rust.md`) gets a section "Version and idempotency" (files, the handler's types, the hooks, the client, the test handler, the tests), in a commit of its own. Where 11.9 to 11.12 are unclear for your target, choose, write it there, and list it in your report. Do not change `spec/rules.md`: if you think it is wrong, say so in your report.

## Not part of this task

The other targets. Derived fields, events, the runtime config, the platform annotations. The website and the guides.

## Your report

As `common.md` section 4 says, plus: the generated API of your target in at most ten lines, and whether you added the two conformance helpers.
