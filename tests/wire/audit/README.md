# Audit goldens

The audit event of one request (`spec/platform.md` section 4): the bytes that the `audit(event)` hook of the Node, Java and Rust servers gets, after canonicalisation. The reference is `veld_rules::audit::event` and `Pipeline::audit_event` (`crates/veld-rules/src/audit.rs`); its test `crates/veld-rules/tests/audit.rs` runs every case, through `crates/veld-rules/tests/common/audit_runner.rs`, which says how a case becomes a request. Every `event` in the file was printed by the reference itself, not calculated by hand, and read through.

| File | What it is |
|---|---|
| `contract.veld` | The contract, compiled as the file `t.veld` of the package `bookings`. |
| `cases.json` | `{ "package", "contract", "cases": [ … ] }` |

One case:

```json
{ "name": "…", "note": "…", "service": "Bookings", "endpoint": "payBooking",
  "request": { "method": "POST", "path": "/bookings/…/pay", "query": {}, "headers": {}, "body": { … } },
  "clock": "2026-06-10T12:00:00Z",
  "principal": { … } | null,
  "facts": { "booking": { … } | null },
  "returns": { … } | null,
  "limiter": null | { "allowed": false, "retryAfterSeconds": 30 },
  "status": 201, "error": null | "AmountMismatch",
  "event": "{\"facts\":…}" | null }
```

A generator's test builds a server with these stubs and sends `request`:

- `clock` is the clock of the hooks: the one reading of the request.
- `principal` is what the application's auth hook returns; `null` is a hook that refuses (`401`), or no principal at all for an endpoint with no auth.
- `facts` is what the loaders return by fact name (`null` is not found); `returns` is what the handler returns (`null`: it returns nothing, or is never reached); `limiter`, when it is there, is what `take` answers.
- The server answers with `status` and `error` (the name of the error, `null` for a success), and calls the `audit` hook once with an event whose RFC 8785 canonical JSON is `event`. A case with `"event": null` is an endpoint that is not audited: the hook is not called.

The members of an event, in every case: `operation` (`Endpoint.sig`), `time` (written like every `datetime`: the fraction only when it is not zero), `outcome` (`status`, and `error` for an error), `parameters` (an object), `facts` (an object), `hidden` and `redacted` (arrays). `principal` is absent when there is no caller, and `result` when the answer has no body or is an error of the prelude. `spec/platform.md` section 9.1 says what each member holds when the request ended early.

What the cases cover: a success (`200` through a 201, `204`), a declared error with a `@secret` in its data, the errors of the prelude (`401`, `403`, `404`, `409` `InvalidTransition`, `412`, `422`, `429`, `500`), the principal's `@pii` and `@secret`, `@pii` and `@secret` in a parameter, a fact and a result, `@visible` fields dropped from a result (also inside a list and a page, where `hidden` and `redacted` name the items with an index), a field that is both hidden and `@pii` (hidden wins and is not listed in `redacted`), a clock with milliseconds, an endpoint with no auth, and an endpoint that is not audited.

What they cannot state: the failure of the hook (the response is sent unchanged and the failure goes to `onError`, section 4); `@audited` on a `ws` endpoint (`E0801` in a generator); what a server does when its own hook for `audit` is missing (the type requires it where the language can).
