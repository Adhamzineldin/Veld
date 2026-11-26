# Veld platform features

Status: **DRAFT. Not implemented.** Proposal: `docs/proposals/rules-and-events.md` section 12. Sections 1 to 9 are phase **P1** (field visibility, tenancy, the audit log, rate limits). Phase **P2** (timed moves, flags, deadlines and service levels) is sections 10 to 16, **P3** (sagas) sections 17 to 21, and **P4** (message catalogs, retention) sections 22 to 24. The MCP export of the proposal's P4 belongs to the teammates.

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

The owner confirmed points 1 to 5 and 7 on 25 November 2026 (asked for 4 and 5: when the audit hook or the limiter itself fails, the service stays up). Point 6 is built as drafted unless the owner says otherwise:

1. **`@limit(10, per: minute, by: user)`** uses the argument form every other annotation has. The proposal wrote `@limit(10 per minute by user)`, which needs its own grammar.
2. **A `@tenant` field behaves as `@readonly`** (section 3): the caller never sends it. The alternative is to accept it in a request and reject a value that is not the caller's.
3. **No "all tenants" caller** (section 3): support staff who must see every tenant are not covered. Adding it later needs a way to say who they are.
4. **A failing audit hook does not fail the request** (section 4).
5. **A failing limiter lets the call through** (section 5).
6. **The loaders keep their signature** (section 3): the caller is in the context they already get, and the generated check after loading is what the contract guarantees.
7. **`@visible` is refused in events** (section 2), not silently dropped or silently kept.

---

# Phase P2: time, flags and service levels

Status: **DRAFT, written 26 November 2026. Not implemented.** Section 16 lists what this text chose and where it differs from the proposal. Sections 1 and 6 above apply: each feature is optional, has its own capability, and changes no byte of a contract that does not use it.

## 10. Timed moves: `flow … after`

```veld
enum BookingStatus {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled after hours(24)     // an unpaid booking expires
  final cancelled
}

model Booking {
  id: uuid @readonly
  status: BookingStatus = .pending
  pendingSince: datetime
}

service Bookings at "/bookings" {
  post "/{id}/expire" expireBooking(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending -> cancelled since booking.pendingSince
  }
}
```

```ebnf
flow        = "flow" IDENT "->" IDENT { "->" IDENT } [ "after" expr ] ;
transition  = "transition" path ":" IDENT { "," IDENT } "->" IDENT [ "since" path ] ;
```

- `after d` makes the one edge of its `flow` a **timed edge**: a value that has been in the source state for `d` may move to the target. `d` is a constant expression of type `duration` (`spec/rules.md` §3.5), greater than zero. `after` on a `flow` with more than one edge, a second `after` for the same edge, or a `d` that is not a positive constant is `E0935`. `after` and `since` are contextual words.
- A timed edge is an edge like any other in `nextStates` and in `final` checks. What differs is who may take it.
- **Only a `transition` with `since` takes a timed edge.** `since` names a required `datetime` field of the same fact, reached through no optional step: the time the value entered its state. The transition MUST have exactly one source, and that pair MUST be a timed edge. A `transition` over a timed edge without `since`, `since` on an edge that is not timed, or a `since` path that is not such a field is `E0936`.
- **The guard** (`spec/rules.md` §10.2) of such a transition also requires `now - since >= d`. A move that is not due answers `409 InvalidTransition` with `from` and `to` as usual; the violation's `message` is `violates rule transition: not due`. A fact whose `since` is in the future is not due.
- The id of a timed transition adds `"a": "<milliseconds of d>"` and `"n": [<wire names of the since path>]` to the hashed object of §10.2. The id of every other transition is what it was.
- `W0933`: a timed edge that no `transition` of the workspace takes.
- An endpoint with a timed transition is an ordinary endpoint: it has auth, policies and rules, and a caller who is permitted may call it. What is new is that the server can call it by itself.

### 10.1 The timer

The contract does not keep timers. The row that holds the state is the only source of truth, and a timer kept beside it could disagree with it. So the generated server **asks the application which moves are due** and runs each through the ordinary pipeline:

- `<Service>DueLoaders` has one method per endpoint with a timed transition: `expireBooking(before, limit, ctx)` answers the parameters of up to `limit` calls of that endpoint whose fact entered the source state at or before `before`. The application answers it from its own store (`where status = 'pending' and pending_since <= :before`).
- `run<Service>DueMoves(impl, hooks, options)` computes `before = now - d` for each such endpoint, asks its loader, and runs every answer through steps 4 to 10 of the pipeline, in process, as a call of that endpoint. Step 2 is replaced: the caller is what `hooks.timer.principal()` answers, the principal the application gives its own timer, and the policies judge it like any caller. A server whose timed endpoint has auth and that has no such hook does not start the run.
- Options: `now` (default: the clock of the hooks), `limit` per endpoint (default 100). It answers a report: per endpoint, how many calls ran, moved, were refused with `InvalidTransition` (another request was first, which is expected), and failed. One failure does not stop the others; failures go to `onError`.
- The application decides when to run it: an interval, a cron job, a queue worker. Running it twice, or on two machines at once, is safe exactly as far as the handler's conditional write is (`spec/rules.md` §10.2): the guard is not an atomic guarantee, and the documentation says so again here.
- Every target, clients included, gets `timedMoves()` for a state machine with timed edges, next to `nextStates` (§10.3 of `spec/rules.md`): the source, the target and the milliseconds of each, so an app can show "expires in 3 hours".
- **Examples** need nothing new: `given booking = { status: .pending, pendingSince: "2026-06-10T12:00:00Z" }`, `at "2026-06-11T11:00:00Z"`, and the call with its outcome.
- **IR.** A flow gains `"after_ms": "86400000"` and a transition `"since": [ step ]`, each omitted when absent. The feature is `rules_timed`, opt-in.

### 10.2 What the implementation chose where the text was silent

- **Constant durations.** The constant fragment of `after` is: whole numbers, `days`, `hours`, `minutes`, `seconds` and `ms` of a constant whole number, and `+`, `-` (two durations) and `*` (a duration and a whole number), with checked 64-bit arithmetic. Anything else, including a name, is `E0935`. The same fragment, without operators, is what `@timeout` and `@slo` take (sections 12 and 13).
- **A second declaration of an edge.** `flow a -> b after d` for an edge that is already declared is `E0935`, whichever of the two has the `after`. A plain `flow a -> b` repeated after a timed one stays `W0930`, and the first declaration decides.
- **`W0933` is a workspace check.** It is not reported while the workspace has errors: a transition that failed its checks would make every edge look untaken.
- **The due check** is part of the guard of §10.2 of `spec/rules.md`, after the state check, and uses the clock of the request: `now - since >= d`, in milliseconds, so a value that has waited exactly `d` is due and one that has waited one millisecond less is not. A `since` that is not a readable datetime is not due. The reference evaluator reads the clock only once, as for every other rule.
- **Reference functions.** `veld_rules::timed_endpoints(ir)` lists the endpoints that take a timed edge, in service and endpoint order, with the fact, the wire path of `since`, the source and target wire values and the milliseconds; `veld_rules::timed_moves(machine)` lists `(from, to, ms)` for a machine, which is what `timedMoves()` returns. A runner asks the loader for facts that entered the source state at or before `now - after_ms`.
- **`veld diff`.** What a caller built against the old contract sees is whether its call to the endpoint that takes the edge is still accepted.

  | Change | Judged |
  |---|---|
  | an edge becomes timed, or its duration grows | breaking: a call that was accepted is refused |
  | an edge stops being timed, or its duration shrinks | compatible |
  | the `since` field of a timed transition changes | possibly breaking: the clock is another |
  | a timed edge is removed or added | as any edge: removed is breaking, added is compatible |

## 11. Flags: `when flag("name")`

```veld
model Booking {
  rule maxStay: end - start <= days(14) when flag("shortStays")
}

policy ManageBooking(user: User, booking: Booking) {
  permit when "beta" in user.roles when flag("betaDesk")
}
```

```ebnf
flagged = "when" "flag" "(" STRING ")" ;      // after a rule, a require, an ensure, a permit, a forbid, a flow
```

- `when flag("f")` is a suffix of one statement: an invariant, a `require` (after its `else`), an `ensure`, a `permit` or `forbid` clause (after its own `when`), and a `flow` with one edge. It says the statement exists only while the flag `f` is on. It is not an expression: `flag` cannot be called inside one. A flag name is 1 to 64 characters of `A-Z a-z 0-9 _ . -`, starting with a letter (`E0937`, also for the suffix on anything else and for a `flow` with more than one edge).
- **The provider.** The hooks object of a server whose contract uses flags requires `flags`, an interface with one operation: `isOn(name, context)`, where the context has the principal (absent without one), the caller's tenant text (absent without one) and the operation signature. A memory provider is generated for tests.
- **One reading per request.** The server asks for each flag at most once per request, the first time a statement with it is reached, and uses that answer until the response is sent.
- **Off** means the statement is not there: not evaluated, not reported, and for a `flow`, not an edge. **On** means it is an ordinary statement.
- **When the provider fails**, the statement that restricts applies and the one that grants does not: an invariant, a `require`, an `ensure` and a `forbid` count as on; a `permit` and a `flow` count as off. The failure goes to `onError`.
- **Clients.** A client has no provider. It skips flagged invariants in its local check, as it skips audited ones, and the rules digest leaves them out. `can<Endpoint>` and the policy functions take an optional last argument, a function from a flag name to a boolean; without it they decide as a failing provider does. `nextStates` lists flagged edges too: the server still decides.
- **Events.** A flagged invariant of an event is evaluated by publisher and consumer with a context that has no principal.
- **Examples.** `given flag "shortStays"` turns a flag on for the example; `given flag "shortStays" off` says off, which is also what a flag the example does not name is. A flag name that no statement of the workspace uses is `E0938`.
- **Tools.** `veld check` ends its summary with the flags the workspace uses and how many statements each gates; `--graph=json` has them under `flags`. `veld diff`: a new statement that restricts and is flagged is **possibly breaking** with the text "breaking once the flag `f` is on"; adding a flag to a statement that restricts is compatible and removing it is judged as adding the statement; for a statement that grants it is the other way round. `--prove` asks its questions with every flag on.
- The flag is not part of a rule id. **IR:** `"flag": "shortStays"` on a `Rule`, an `EndpointRequire`, an `EndpointEnsure`, a `PolicyClause` and a flow, omitted when absent. The feature is `rules_flags`, opt-in.

## 12. Deadlines: `@timeout`

```veld
@timeout(seconds(2))
get "/{id}" getBooking(id: uuid) -> Booking
```

- `@timeout(d)` stands on an endpoint, or on a service as the default of its endpoints. `d` is a positive constant duration of at most one hour. On an `sse` or `ws` endpoint, or with another argument, it is `E0602`.
- The prelude gains `error DeadlineExceeded 504 { timeoutMs: long }`, which such an endpoint declares implicitly.
- **Server.** The request context carries `deadline`, the instant the request arrived plus `d`, and the target's usual way to notice it (an `AbortSignal` in TypeScript, a cancellation the handler future gets in Rust, `ctx.deadline()` and `ctx.expired()` in Java). When the pipeline has not produced its answer at the deadline, the server answers `504 DeadlineExceeded`. Where the language cannot stop a running handler, the handler runs on and its answer is dropped: the documentation says that a handler which writes must notice the deadline itself. The answer of a request that hit its deadline is not stored for an idempotency key (`spec/rules.md` §11.9: a `5xx` removes the entry).
- **Client.** A generated client method of such an endpoint uses `d` as its default timeout when the caller sets none, and fails with the client's existing timeout error.
- **IR.** `Endpoint` gains `"timeout_ms": "2000"`, omitted when absent. The feature is `timeouts`, opt-in.

## 13. Service levels: `@slo`

```veld
@slo(p99: ms(300), errors: 0.1)
get "/{id}" getBooking(id: uuid) -> Booking
```

- `@slo(...)` stands on an endpoint, or on a service as the default of its endpoints. Its arguments, at least one: `p50`, `p90`, `p95`, `p99` (a positive constant duration: that share of requests answers within it) and `errors` (a decimal above 0 and below 100: at most that many **percent** of requests end in a `5xx`). Anything else is `E0602`.
- **The contract only states the objective.** Generated code measures; it does not alert, shed load or refuse a request.
- **The metrics hook.** The hooks object of a server with an `@slo` takes `metrics(m)`, optional, called once for every request that matched an endpoint of that server, after the response is written:

  ```jsonc
  { "name": "http.server.request.duration", "unit": "s", "value": 0.0421,
    "attributes": { "http.request.method": "GET", "http.route": "/bookings/{id}",
                    "http.response.status_code": 200, "error.type": "500",       // only for a 5xx
                    "veld.service": "bookings/Bookings", "veld.endpoint": "getBooking",
                    "veld.operation": "veld:…" } }
  ```

  The names are those of the OpenTelemetry HTTP semantic conventions, so an exporter needs no mapping, and generated code depends on none. A failing hook is ignored.
- **`veld export slo [-o FILE]`** writes one YAML file of OpenSLO `v1` documents, one `SLO` per stated objective, in package, service and endpoint order: a ratio objective over a 28-day rolling window whose `good` and `total` queries are written for Prometheus over the metric above (`http_server_request_duration_seconds_bucket` and `_count`, selected by `veld_operation`). A latency objective whose threshold is not a bucket boundary the hook's consumer has is the user's to fix: the file is theirs, as for `veld export openapi`. A workspace without `@slo` is `E0803`.
- **IR.** `Endpoint` gains `"slo": { "p99_ms": "300", "errors": "0.1" }`, omitted when absent. The feature is `slo`, opt-in: a generator declares it when it calls the hook.

## 14. The pipeline with P2

Added to section 6: the deadline starts at step 1 and can end any later step; a flag is read where its statement stands; `metrics` is called after step 10. `run<Service>DueMoves` enters at step 3.

## 15. Diagnostics of P2

| Code | Meaning |
|---|---|
| `E0935` | `after` is wrong: on a `flow` with several edges, twice for one edge, or not a positive constant duration |
| `E0936` | `since` is wrong: missing on a timed edge, present on another, several sources, or not a required `datetime` field of the fact |
| `W0933` | A timed edge that no `transition` takes |
| `E0937` | `when flag(…)` is wrong: a bad name, or on a statement that cannot have it |
| `E0938` | `given flag "x"` in an example names a flag that no statement of the workspace uses |

## 16. What P2 chose

Built as drafted unless the owner says otherwise.

1. **No `Scheduler` interface.** The proposal schedules a timer when a value enters a state. This text asks the application's store which moves are due (section 10.1), because the row is then the only place that knows, and a lost timer cannot strand a booking. It is the proposal's `runDueTransitions()`, made the only way.
2. **A timed move is taken by an endpoint with `since`**, and the timer calls that endpoint as a caller the application names. Nothing skips auth or the policies.
3. **`flag` is a suffix, not a function**, so an expression stays a pure function of the value and a client can still check what is not flagged.
4. **A failing flag provider restricts** (section 11).
5. **`errors` is written in percent without a sign** (`errors: 0.1`): the proposal's `0.1%` needs a new literal.
6. **`@slo` generates a hook and an export, nothing that acts.**
7. **A deadline cannot stop a handler in every language**, and the text says which answer wins instead of promising a cancellation.

---

# Phase P3: sagas

Status: **DRAFT, written 26 November 2026. Not implemented. Experimental** when built: a server gets a saga only with the target option `sagas = "experimental"`, until the crash tests of section 21 exist for that target.

## 17. A saga

```veld
service Checkout at "/checkout" {
  saga post "/" checkout(body: CheckoutRequest) -> Booking | RoomFull | PaymentDeclined {
    step hold    = Rooms.hold(body: { roomId: body.roomId })       compensate Rooms.release(id: hold.id)
    step charge  = Payments.charge(body: body.payment)             compensate Payments.refund(id: charge.id)
    step confirm = Bookings.confirm(id: hold.bookingId)
    timeout minutes(5)
  }
}
```

```ebnf
saga     = { annotation } "saga" "post" STRING IDENT "(" params ")" "->" responses "{" step { step } "timeout" expr "}" ;
step     = "step" IDENT "=" call [ "compensate" call ] ;
call     = IDENT "." IDENT "(" [ arg { "," arg } ] ")" ;        // Service.endpoint(name: expr, …)
```

- A saga is an endpoint of its service whose implementation is generated: a list of calls to other endpoints, each with an optional call that undoes it. `saga`, `step`, `compensate` and `timeout` are contextual words.
- **A step** calls an endpoint of a service that this service consumes (`consumes` in `veld.toml`), or of its own service. Its arguments are named, as in an example, and are rule expressions over the saga's parameters and the results of earlier steps, each under its step's name. The value of a step is the success value of its call.
- **`compensate`** names the call that undoes the step. It may read the saga's parameters, earlier steps and its own step.
- **The result** of the saga is the value of its last step, whose type MUST be the saga's success type.
- **Every call is retried, so every call must be safe to repeat.** The endpoint of every step and of every `compensate` MUST be `@idempotent` (`E1202`). The orchestrator sends the key `<saga id>.<step>` for a step and `<saga id>.<step>.undo` for its compensation.
- **A saga endpoint is itself `@idempotent(required: true)`**, implicitly: the caller's key, in its scope (`spec/rules.md` §11.9), is the saga's id. A caller that lost its connection sends the same request again and gets the stored answer, or `409 Conflict` while the saga still runs.
- `timeout d` is required: a positive constant duration, the time from the start within which the last step must have answered.
- `E1201`: a step names a service that is not consumed, an endpoint that does not exist, a stream or `ws` endpoint, or arguments that do not fit. `E1203`: two steps with one name, a step called like a parameter, an expression that reads a later step, a last step whose type is not the success type, a missing or wrong `timeout`, a saga without a step. `W1201`: a step that is not the last has no `compensate`: once it has run, the saga cannot be undone past it.
- The prelude gains `error SagaFailed 502 { step: string, compensated: bool }`, which a saga declares implicitly.

## 18. What the orchestrator does

One saga instance runs its steps in order and saves its state after every answer.

1. **Start.** The request passes steps 1 to 5 of the pipeline (auth, middleware, validation, invariants) and the idempotency lookup. The state is saved as `running`, with the parameters and the deadline.
2. **A step.** The call is made through the generated client of that service.
   - A success: the value is saved and the next step starts.
   - **A declared error** of the step's endpoint (any answer below 500 that is not a success): the step failed for good. It is not retried.
   - **No answer, or a `5xx`:** the call is made again, after 200 ms, doubling up to 5 s, until it answers or the deadline passes.
3. **The deadline passes** before the last step answered: the saga fails at the step that was running.
4. **Failing.** The state is saved as `compensating`. The `compensate` calls of the steps that succeeded run in reverse order; a step without one is skipped.
   - A step whose call got no answer may have happened. The orchestrator first learns what it did: it repeats that call, with the same key, for as long as the saga's `timeout` again. A success is then undone like any other, a declared error means there is nothing to undo, and no answer ends the saga `stuck`.
   - A compensation that gets no answer or a `5xx` is retried in the same way, for as long as the saga's `timeout` again.
   - A compensation that answers a declared error, or runs out of time, ends the saga as **`stuck`**: it needs a person. The hook `onSagaStuck(state)` is called, and the state stays in the store.
5. **The answer.**
   - Every step succeeded: the last value, with the saga's success status. State `completed`.
   - Compensated after a declared error that is one of the saga's own declared errors (the same declaration): that error, as the step answered it. Otherwise `502 SagaFailed` with the step and `compensated: true`. State `compensated`.
   - Stuck: `502 SagaFailed` with `compensated: false`. State `stuck`.
   - `SagaFailed` is stored for the caller's key like every answer of the saga, although it is a `5xx`: the saga ran, and sending the request again must not run it twice.
6. **After a crash.** `resume<Service>Sagas(hooks, options)` asks the store for the sagas that are `running` or `compensating`, claims each, and goes on where the saved state ends: a `running` saga before its deadline repeats the step that has no saved answer, which is safe because its key is the same; one past its deadline fails. The application runs it when the server starts and on an interval.

**`SagaStore`** is the interface the application implements; a memory store is generated for tests:

```
create(state) -> created | exists        save(state)          load(id) -> state | none
unfinished(limit) -> [state]             claim(id, owner, untilMs) -> bool
```

`state` is plain data with the same members in every target: `id`, `operation`, `scope`, `status`, `parameters` (wire form), `deadline`, `steps` (per step: `name`, `status` of `pending`, `done`, `failed`, `unknown`, `undone`, and the wire value or error), `failedStep`, `answer`. `claim` gives one orchestrator the saga for a time, so two processes that resume do not both run it; a store that cannot claim atomically may run a call twice, which the keys make harmless.

- The service interface the application implements has no method for a saga endpoint. The hooks object requires `sagas: { store, onSagaStuck? }` and the clients of the consumed services.
- **A client** of the service sees an ordinary `post` endpoint with a required idempotency key.
- **Examples.** A saga example states what each call answers and what must have happened:

  ```veld
  example "a declined payment releases the room" {
    given step hold = { id: "h-1", bookingId: "0b7c…" }
    given step charge = PaymentDeclined { reason: "funds" }
    key "k-1"
    checkout(body: { … }) -> PaymentDeclined
    expect compensated hold
  }
  ```

  `given step s = <value | Error { … } | unavailable>` and `given undo s = <ok | Error { … } | unavailable>`; a step the example does not state answers a filled-in success, an undo answers `ok`. `unavailable` never answers. `expect compensated a, b` names the steps undone, in order; `expect stuck` says the saga ended stuck. The reference evaluator runs the plan without a network and is what the generated orchestrators are compared with: the calls in order with their arguments and keys, the final state and the answer.
- **IR.** `Endpoint` gains `"saga": { "steps": [ { "name", "call": { "service", "endpoint", "args": [ { "name", "value": Expr } ] }, "compensate": call | absent } ], "timeout_ms": "300000" }`, omitted when absent. The feature is `sagas`, opt-in. The OpenAPI export marks the operation `x-veld-saga: true`.

## 19. What a saga is not

It is not a transaction. Between a step and its compensation other requests see the step's effect. A compensation is a new action that can fail, which is why `stuck` exists and is reported instead of hidden. Steps over channels (§11 of the proposal) are not in this version: every step is an HTTP call.

## 20. Diagnostics of P3

| Code | Meaning |
|---|---|
| `E1201` | A step or a `compensate` names a call that does not exist or does not fit |
| `E1202` | The endpoint of a step or of a `compensate` is not `@idempotent` |
| `E1203` | The saga is not well formed (section 17) |
| `W1201` | A step that is not the last has no `compensate` |

## 21. Gate for P3

For each server target, with the memory store and real HTTP between the services: the orchestrator is stopped after every save and resumed, for a saga that completes and for one that fails at each step, and every run ends `completed` or `compensated` with each call's effect present once or undone once; a compensation that keeps failing ends `stuck` and calls the hook; two resumers on one store do not double an effect. Until these tests exist for a target, its sagas stay behind `sagas = "experimental"`.

---

# Phase P4: messages in other languages, and retention

Status: **DRAFT, written 26 November 2026. Not implemented.** The MCP export of the proposal's P4 belongs to the teammates.

## 22. Message catalogs

```toml
# services/bookings/messages/ar.toml
"bookings/NewBooking.atMost30Days" = "لا يمكن أن تتجاوز الإقامة ثلاثين يوما"
"veld/not-permitted" = "غير مسموح"
```

- A member MAY have a directory `messages` next to its contracts (`[service] messages = "messages"` names another), with one file per language: `<tag>.toml`, where `<tag>` is a BCP 47 language tag in lower case (`ar`, `fr`, `pt-br`). A file is a flat table from a **message key** to a text.
- **The message key** of a rule is its rule id without the `veld:` in front and without the `@` and the hash behind (`bookings/NewBooking.atMost30Days`, `bookings/Bookings.payBooking.notCheckedIn`). It stays the same when the expression changes, which the id does not. The fixed texts of the runtime have keys of their own: `veld/transition`, `veld/transition-not-due`, `veld/version`, `veld/not-permitted`.
- What a catalog translates: the message of an invariant, of a `require`, of an `ensure` and of a `forbid`, and those fixed texts. The texts of validation issues are not in a catalog: an issue has a `code`, and a client translates by it.
- The text in the contract is the default language. A catalog need not be complete.
- **Server.** A server target of a member with catalogs embeds them. For each request it picks the first language of `Accept-Language`, in the header's order of preference, for which a catalog exists (a tag matches a catalog of the same tag or of its primary language; `*` matches none), and writes every message of the answer in it: the `message` of each violation and the `message` of a `Forbidden`. A key the catalog lacks keeps the contract's text. When a catalog was used the answer has `Content-Language`. Every violation of such a server also has `messageKey`.
- **Without catalogs nothing changes**: no `messageKey`, no header, the same bytes.
- **Client.** A client target of a member with catalogs gets them as data (`messages.ar`), and the violations of its local check have `messageKey`, so an app can show its own language without asking the server.
- **`veld check`.** `E1301`: a catalog that is not a flat table of strings, or whose name is not a language tag. `W1301`: a key of a catalog that no rule of the member has. `W1302`: a rule with a message that a catalog of the member lacks, reported once per catalog with the count and the first keys.
- **`veld fmt`** does not touch catalogs. **`veld diff`** ignores them: a text is not a contract.
- **IR.** A package gains `"messages": { "ar": { "<key>": "<text>" } }`, sorted, omitted when it has no catalog. The feature is `messages`, opt-in; a generator that does not declare it reports `E0801` at the first catalog file.

## 23. Retention: `@retain`

```veld
@retain(days(30), since: checkedOutAt, then: anonymize)
model Guest {
  id: uuid @readonly
  name: string @pii
  phone?: string @pii
  checkedOutAt?: datetime
}
```

- `@retain(d, since: f, then: delete | anonymize)` stands on a model. `d` is a positive constant duration of whole days. `f` is a `datetime` or `date` field of the model, required or optional: the moment from which `d` counts; a value without it is kept. `anonymize` needs at least one `@pii` field (`spec/rules.md` §19.3): those are the fields it is about. Anything else is `E1310`.
- **Veld does not delete anything.** It knows no database. It says what is due and the application does it:
  - a server target with retained models gets `retentionPlan`, data that lists for each: the model, `afterMs`, the `since` field, `then`, and for `anonymize` the wire paths of the `@pii` fields;
  - `RetentionJobs` is the interface the application implements: `delete(model, before)` and `anonymize(model, before, fields)`, each answering how many values it changed;
  - `run<Target>Retention(jobs, options)` calls it once per entry with `before = now - d` and answers the counts. The application runs it from a job. A failure of one entry does not stop the others and is in the report.
- **`veld export retention [-o FILE]`** writes the data inventory of the workspace as Markdown: per package, every model with a `@pii` or `@secret` field, those fields, how long the model is kept and what happens then, or **"kept without limit"**, and where it travels (the endpoints that return it, the events that carry it). It is deterministic and the file is the user's.
- **Lint `L0017` `pii-without-retention`** (a warning by default): a model with a `@pii` field and no `@retain`.
- `@retain` changes nothing on the wire and no signature. **IR:** a model gains `"retain": { "after_ms": "2592000000", "since": "checkedOutAt", "then": "anonymize" }`, omitted when absent. The feature is `retention`, opt-in for a server; a client generates nothing for it.

## 24. Diagnostics of P4

| Code | Meaning |
|---|---|
| `E1301` | A message catalog is not a flat table of strings, or its name is not a language tag |
| `W1301` | A catalog has a key that no rule has |
| `W1302` | A rule's message is missing from a catalog |
| `E1310` | `@retain` is wrong: the duration, the `since` field, `then`, or `anonymize` without a `@pii` field |
