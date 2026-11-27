# Brief: platform phase P1 in the generators

Three agents get this brief, one per target (Node with TypeScript, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

Four features of `spec/platform.md` sections 1 to 9 are implemented outside the generators, each behind an opt-in feature your generator refuses today (`E0801`): **field visibility** (`visibility`), **tenancy** (`tenancy`), **the audit log** (`audit`) and **rate limits** (`limits`). Build them in your target, in that order, each as its own series of commits with its own capability.

**No setting turns any of them off** (section 1): not the output mode, not `validate = false`.

## Read first

- `spec/platform.md` sections 1 to 9, and **9.1** (what the compiler work chose: the shape of `hidden` and `redacted` paths, what the audit event contains in each case, the limiter's rule, which role is asked for which feature, the names of the hooks).
- `spec/rules.md` 5.3 (the pipeline), 11 (the principal; 11.12 for the caller-identity hook that `@limit(by: user)` shares with idempotency), 19.3 (`@pii`, `@secret`).
- The IR: `Field.visible` (a boolean `Expr` whose roots are the declaration's fields, the caller and `now`) and `Field.visible_principal`; `Field.tenant`, `Field.tenant_id`, `Decl::tenant_field()`, `Field::server_set()` (readonly or tenant); `Endpoint.audited`; `Endpoint.limit` (`Limit`, `LimitWindow::seconds()`, `LimitSubject`).
- The reference and its goldens, which hold the three targets to the same bytes:
  - visibility: `veld_rules::apply_visibility_type` / `_decl` / `_type_at`, `Pipeline::show`; goldens `tests/wire/visibility/`; examples `tests/rules/endpoints/visibility.veld.pending` (14);
  - tenancy: `veld_rules::tenant::{caller_tenant, refused, text}`, `Pipeline::refused`; examples `tests/rules/endpoints/tenancy.veld.pending` (17); the envelope attribute in `tests/wire/events/tenant.{veld,json}`;
  - audit: `veld_rules::audit::{event, Ended}`, `Pipeline::audit_event`; goldens `tests/wire/audit/` with a README that says how a request is driven;
  - limits: `veld_rules::limits::{key, MemoryLimiter, replay, Decision}`; goldens `tests/wire/limits/`.
- `crates/veld-emit-conformance/src/` (visibility, tenancy, audit, limits): the "refused" checks. Which role is asked: visibility and audit only a server, for its own services; tenancy a server, and a client when an endpoint returns a tenant-scoped model; limits every role.
- **The nearest things your emitter already does:** a policy's expression over the caller and a value (for `@visible`); `@readonly` (a `@tenant` field behaves like it in a request: the P1 report lists the places in each emitter where `@readonly` is treated specially; find them by searching `readonly`); the `404` of a fact that is not found (a fact of another tenant is exactly that); `onRuleViolation` and the capture (for the audit event and its `redacted`, which must use your runtime's redaction if it exists, else the same rule as `veld_rules::redact_*`); the idempotency store and its caller hook (for the limiter).

## 1. Visibility (`visibility`)

The server drops a field with `@visible(when: e)` from every value it sends to a caller (a response, a declared error, a stream item, at any depth) unless `e` is true for that caller. It fails closed: dropped when `e` is false, cannot be evaluated, or the request has no caller of the principal model `visible_principal` names (an endpoint with `@auth(none)`, a service with another principal). A field whose value is `null` is dropped and listed too. It runs after the output rules and after the tenant check, so they see the whole value; the `ETag` is taken before it. The dropped wire paths (`hidden`, unrooted: `guestPhone`, `items[0].x`) go to the audit event. A request may contain the field. A client gets nothing new.

## 2. Tenancy (`tenancy`)

A `@tenant` field is set by the server, never by the caller: in a request it is treated as `@readonly` is (9.1 says a member a request carries is kept as received and the application sets the real one; follow 9.1 exactly). After a fact of a tenant-scoped model is loaded, its tenant is compared with the caller's (the `@tenant` field of the principal): another tenant is **not found** (`404` with the fact's name; absent for an optional fact), never `403`. Before a value is sent, every tenant-scoped value in it, at any depth, is compared: another tenant's value is never sent; the answer is `500 Internal` with no detail and one `onRuleViolation` event per value (position `output`, outcome `rejected`, the rule is `Field.tenant_id` from the IR). The same for a declared error and a stream item. A caller without a tenant owns nothing. An event with a `@tenant` field carries `veldtenant` in its envelope and the handler's metadata has it; a consumer does not check it. A client where the feature applies only needs its types right (the field is not something a request sends).

## 3. The audit log (`audit`)

The hooks of a server with an audited endpoint require `audit(event)`. The server calls it once for every request that matched the endpoint, whatever the outcome, after the outcome is known and before the response is written, with exactly the `AuditEvent` of section 4 and 9.1: `operation`, `time` (the clock read once, when the request is matched), `principal` (absent when authentication failed), `outcome`, `parameters` and `facts` (always objects), `result` (the success body, or the data of a declared error; a prelude error has none), `hidden` (from visibility), `redacted` (rooted paths; every `@pii` and `@secret` value left out). An answer that ends the pipeline early (`401`, `429`, `422`, `404`) is audited too. When the hook fails the response is sent unchanged and the failure goes to `onError`. An audited streaming or `ws` endpoint is `E0801`. Your generated events must equal the goldens of `tests/wire/audit/` after canonicalisation.

## 4. Rate limits (`limits`)

The hooks take a `limiter` (`take(key, limit, windowSeconds) -> { allowed, retryAfterSeconds }`); a generated memory limiter (the fixed-window rule of 9.1, with an injectable clock) is used, with one notice at start-up, when the application gives none. The key is `operation|k|identity` exactly as `veld_rules::limits::key` builds it. `by: user` needs the caller-identity hook (the one idempotency uses; a server that needs it and has none does not start); `by: tenant` the caller's tenant text; `by: ip` an `address` hook (a server that needs it and has none does not start). It runs after authentication and before the middleware. A refused call answers the prelude's `429 TooManyRequests` with `retryAfter` and the `Retry-After` header. When the limiter itself fails the call is allowed and the failure goes to `onError`. **Client:** a method that gets `429` with `retryAfter` waits that long and sends again, once, when the endpoint is a `get` or `@idempotent`; otherwise it raises the typed error at once; the option `retryOnLimit: false` turns the wait off; the wait uses an injectable sleep so a test does not wait.

## Gates

Four capabilities, each declared when its part is done, for the roles 9.1 names. A contract that uses none of the four generates the bytes it generated before. The vectors keep their `.pending` names.

## Tests that must exist when you hand back (written and compiled, not run)

- Visibility: the goldens of `tests/wire/visibility/` through your runtime's function; the 14 examples through the generated test handler; over the real entry point a field hidden for one caller and present for another, inside a list and a page, an expression that cannot be evaluated, an endpoint with `@auth(none)`, that an `ensure` still sees the field, that `validate = false` and output mode `off` change nothing.
- Tenancy: the 17 examples; a fact of another tenant (`404`), an optional one (absent), a value of another tenant in a result, in a list, in a declared error, in a stream item (`500`, nothing sent, the hook events); a caller without a tenant; the `veldtenant` goldens through the generated publisher.
- Audit: every case of `tests/wire/audit/` through the generated server with a recording hook, canonical bytes compared; a failing hook; the early answers.
- Limits: the goldens of `tests/wire/limits/` through the generated memory limiter with a fake clock; over the real entry point the last allowed call, the first refused one with its body and header, the next window, each of `user`, `tenant`, `ip`; a failing limiter lets the call through; a server created without the hook it needs; the client's single retry, no retry for a `post` that is not idempotent, `retryOnLimit: false`.
- Snapshots of a small contract with all four; the conformance checks.

## Your report

As `brief-in-the-generators.md` says, one part per feature, plus the generated names in your target, and every place where you treated `@tenant` like `@readonly`.
