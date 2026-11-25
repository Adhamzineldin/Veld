# Veld platform features

Status: **DRAFT. Not implemented.** Proposal: `docs/proposals/rules-and-events.md` section 12. This file covers phase **P1** (field visibility, tenancy, the audit log, rate limits). Phases P2 (timed transitions, flags, timeouts and SLOs), P3 (sagas) and P4 (message catalogs, retention) are not drafted yet. The MCP export of the proposal's P4 belongs to the teammates.

This file extends `spec/language.md` and `spec/rules.md`. Where it is silent, those files apply. Each feature is optional, has its own capability, and changes no byte of a contract that does not use it.

## 1. What the four features share

- Each is an annotation. Its expressions are rule expressions (`spec/rules.md` §3) and are checked like them.
- Each needs to know the caller, so each needs the **principal** (`spec/rules.md` §11.1): the value the application's auth hook returned.
- Each is enforced by the generated server, in a fixed place of the pipeline (`spec/rules.md` §5.3). Section 6 gives the whole order.
- **No setting turns them off.** They are not output checks in the sense of the output mode: `audit` and `off` do not apply to visibility, to the tenant check or to a limit.
- A generator that cannot enforce one refuses the contract (`E0801`). Clients have nothing to enforce for visibility, tenancy and audit, and declare those capabilities by generating nothing.

## 2. Field visibility: `@visible`

```veld
principal User { id: uuid  roles: string[] }

model Booking {
  guestId: uuid
  guestPhone?: string @visible(when: user.id == guestId || "support" in user.roles)
}
```

- `@visible(when: e)` stands on a field of a model or an error. The field MUST be optional (`E1101`): a client already handles its absence, so hiding it breaks no caller.
- `e` is a boolean expression. Its scope is that of an invariant (the fields of the declaration, `spec/rules.md` §5.1) plus the caller, under the name the package's principal has (`user`, or the name after `as`), and `now`. The package of the declaration MUST declare a principal (`E1101`).
- **Where it applies.** The server drops the field from every value it sends to a caller: a response, a declared error, a stream item, at any depth (in a nested model, a list, a map, a page, the arm of a union). It applies after the output rules, so an `ensure` and the invariants of the result see the whole value.
- **It fails closed.** The field is dropped when `e` is false, when `e` cannot be evaluated (`spec/rules.md` §3.7), and when the request has no caller of that principal type: an endpoint with `@auth(none)`, or a service whose principal is another declaration. `veld check` warns about such an endpoint (`W1101`), because the field can never be seen there.
- It does not apply to what a caller sends: a request may contain the field.
- An event MUST NOT reach a field with `@visible` (`E1101`): a published event has no caller to decide for. Use another model for the event.
- **Clients** get nothing new: the field is optional in their types already.
- **Examples.** With `returns`, the reference evaluator applies visibility to the result before the pattern after `ok` is matched. A pattern MAY say `field: absent` to state that a field is not there.
- **`veld diff`.** Adding `@visible` to a field, or changing its expression, is possibly breaking; removing it is compatible.
- **IR.** `Field` gains `"visible": Expr`, omitted when absent. The expression is not part of the declaration's signature. The feature is `visibility`. The OpenAPI export marks the property `x-veld-visible: true`.

## 3. Tenancy: `@tenant`

```veld
principal User { id: uuid  tenantId: uuid @tenant }

model Booking {
  id: uuid @readonly
  tenantId: uuid @tenant
}
```

- `@tenant` stands on a field of a model, with no argument. A model has at most one, its own or inherited; the field MUST be required and of type `uuid`, `string`, `int` or `long` (`E1110`). A model with one is **tenant-scoped**.
- **The caller's tenant** is the value of the `@tenant` field of the principal. A service that loads or returns a tenant-scoped model MUST have a principal whose model has a `@tenant` field of the same type, and every such endpoint MUST have auth (`E1110`).
- **A `@tenant` field is set by the server, never by the caller.** It behaves as `@readonly` does: a request does not carry it, and the application sets it from the caller.
- **Facts.** After a fact of a tenant-scoped model is loaded, its tenant is compared with the caller's. A fact of another tenant is treated as **not found**: `404 NotFound` with the fact's name for a required fact, absent for an optional one. It is never `403`, so a caller cannot learn that another tenant's record exists. The loader gets the caller in its context as today and should query by tenant; the check is there for the loader that does not.
- **Responses.** Before a value is sent, every tenant-scoped value in it, at any depth, is compared with the caller's tenant. A value of another tenant is never sent: the answer is `500 Internal` with no detail, and `onRuleViolation` gets one event (position `output`, outcome `rejected`) whose rule is the model's tenant id, `veld:<package>/<Model>.tenant@<hex16>`, over the RFC 8785 JSON of `{ "d": "<qualified model name>", "k": "tenant", "n": "<wire name>" }`. The same holds for a declared error and a stream item.
- A caller whose principal has no tenant value (an optional `@tenant` field that is absent) has no tenant: every tenant-scoped fact is not found for it and every tenant-scoped value in a response is refused. There is no "all tenants" caller in this version.
- **Events.** An event with a `@tenant` field carries its value text in the CloudEvents extension attribute `veldtenant` (`spec/events.md` §6.2), and the handler's metadata has it. A consumer does not check it: it has no caller.
- **Examples.** A `@tenant` field that an example leaves out is filled in with the tenant of the example's principal, written or filled in, so an example is in one tenant unless it says otherwise.
- **IR.** `Field` gains `"tenant": true`, omitted when false. The feature is `tenancy`.

## 4. The audit log: `@audited`

```veld
@auth(bearer)
service Bookings at "/bookings" {
  @audited
  post "/{id}/pay" payBooking(id: uuid, body: Payment) -> Booking | AmountMismatch { … }
}
```

- `@audited` stands on an endpoint, or on a service for all its endpoints. It takes no argument. On a `ws` endpoint it is `E0801` in this version.
- The hooks object of a server with an audited endpoint requires `audit(event)`. The generated server calls it **once for every request that matched the endpoint**, whatever the outcome, after the outcome is known and before the response is written.

```jsonc
AuditEvent = {
  "operation": "veld:bookings/Bookings.payBooking@…",   // the operation signature
  "time": "2026-06-10T12:00:00.000Z",                    // the request's clock reading
  "principal": { … } | absent,                           // the caller; absent when authentication failed
  "outcome": { "status": 200 } | { "status": 409, "error": "AmountMismatch" },
  "parameters": { … },                                   // wire form, as received after validation
  "facts": { … },                                        // wire form, as loaded: the values before the handler
  "result": { … } | absent,                              // wire form, as sent: the value after
  "hidden": [ "guestPhone" ],                            // wire paths that `@visible` dropped from the result
  "redacted": [ "parameters.body.card" ] }               // wire paths left out because of `@pii` or `@secret`
```

- The value of every `@pii` and `@secret` field is left out of `principal`, `parameters`, `facts` and `result`, and its path is listed in `redacted` (`spec/rules.md` §19.3).
- A request that failed validation has no `parameters` beyond what was bound, and no `facts`.
- **When the hook fails**, the response is sent unchanged and the failure goes to `onError` with the event. The handler has already done its work, and a `500` would make the caller repeat it. An application that needs the audit record and the state change in one transaction writes the record in its handler; this hook cannot give that, and the guide says so.
- The event is a plain value with the same members in every target. Where it goes is the application's choice: a table, a log stream, a channel.
- **IR.** `Endpoint` gains `"audited": true`, omitted when false. The feature is `audit`.

## 5. Rate limits: `@limit`

```veld
@limit(10, per: minute, by: user)
post "/{id}/pay" payBooking(id: uuid, body: Payment) -> Booking
```

- `@limit(n, per: w, by: k)` stands on an endpoint, or on a service as the default of its endpoints: each endpoint is counted on its own, and an endpoint's own `@limit` replaces the service's. An endpoint has at most one (`E1120`).
- `n` is an integer from 1. `w` is `second`, `minute`, `hour` or `day`. `k` is `user`, `tenant` or `ip` (`E1120` for anything else).
  - `user` counts per caller. It needs auth on the endpoint (`E1120`) and the **caller identity** hook of the server, the one idempotency uses (`spec/rules.md` §11.9): a text the application returns for the caller. A server that needs it and has none does not start.
  - `tenant` counts per caller's tenant (section 3). It needs a principal with a `@tenant` field (`E1120`).
  - `ip` counts per client address. The address is a text the application returns from a hook, because only the application knows which proxy header to trust. A server that needs it and has none does not start.
- The hooks object takes a **limiter**, an interface with one operation:

```
take(key, limit, windowSeconds) -> { allowed, retryAfterSeconds }
```

  `key` is the operation signature, `k` and the identity, joined with `|`. An implementation MUST allow at most `limit` calls of one key in any window it defines of `windowSeconds`. A memory limiter is generated: a fixed window per key that starts with the key's first call. Without a limiter in the hooks the server uses the memory limiter and says so once when it starts, because a memory limiter counts per process.
- **Where it runs:** after authentication and before the middleware, so a refused call costs nothing more. A refused call answers the prelude's `429 TooManyRequests` with `retryAfter` in seconds and the `Retry-After` header.
- When the limiter itself fails, the call is **allowed** and the failure goes to `onError`: a limiter that is down must not take the service down with it.
- **Clients.** A generated client that gets `429` with `retryAfter` waits that long and sends the request again, once, when the endpoint is a `get` or is `@idempotent`; otherwise it raises the typed error at once. The option `retryOnLimit: false` turns the wait off.
- **IR.** `Endpoint` gains `"limit": { "count": 10, "per": "minute", "by": "user" }`, omitted when absent. The feature is `limits`.

## 6. The pipeline with the four features

The steps keep the numbers they have in `spec/rules.md` §5.3. What this file adds is in bold and has no number of its own:

- 1 match, 2 auth
- **the limit** (section 5)
- 3 middleware, 4 bind and validate, 5 invariants
- 6 the clock; the facts, **each compared with the caller's tenant** (section 3); the policies; the transition guard; `require`; `old`
- 7 the handler, 8 invariants on the result, 9 `ensure`
- **the tenant check of the result** (section 3)
- **visibility** (section 2)
- **the audit event** (section 4)
- 10 serialise

An answer that ends the pipeline early (a `401`, a `429`, a `422`, a `404`) skips to the audit event: an audited endpoint records refused calls too.

## 7. Diagnostics

| Code | Meaning |
|---|---|
| `E1101` | `@visible` on a field that is not optional, in a package without a principal, or reached by an event |
| `W1101` | An endpoint returns a field with `@visible` and has no caller of that principal type: the field is never sent there |
| `E1110` | `@tenant` is wrong: two in one model, an optional field or a type that cannot be a tenant, a principal without one, or a tenant-scoped endpoint without auth |
| `E1120` | `@limit` is wrong: two on one endpoint, a bad count, window or subject, or a subject the endpoint cannot supply |

`@audited` and the others in a wrong place, or with wrong arguments, are `E0602`, as for every annotation.

## 8. Gate for P1

The shared vectors pass on the Node, Java and Rust servers: a fact of another tenant is `404`; a value of another tenant in a response is `500` and never sent; a hidden field is absent for one caller and present for another, also inside a list and a page; the audit event of one call is equal in the three targets after canonicalisation, with its `hidden` and `redacted` lists; a limit refuses the call after the last allowed one and lets it through in the next window.

## 9. Open points

Decided in this draft, for the owner to confirm:

1. **`@limit(10, per: minute, by: user)`** uses the argument form every other annotation has. The proposal wrote `@limit(10 per minute by user)`, which needs its own grammar.
2. **A `@tenant` field behaves as `@readonly`** (section 3): the caller never sends it. The alternative is to accept it in a request and reject a value that is not the caller's.
3. **No "all tenants" caller** (section 3): support staff who must see every tenant are not covered. Adding it later needs a way to say who they are.
4. **A failing audit hook does not fail the request** (section 4).
5. **A failing limiter lets the call through** (section 5).
6. **The loaders keep their signature** (section 3): the caller is in the context they already get, and the generated check after loading is what the contract guarantees.
7. **`@visible` is refused in events** (section 2), not silently dropped or silently kept.
