# Veld IR — v1

Status: **DRAFT — frozen together with `language.md`.**

The IR is the only input that generators, tools and plugins receive. It is produced by `veld-compiler` and only after every check has passed, so a consumer never has to re-check anything. It is immutable and fully resolved:

- No imports remain.
- Inheritance is resolved.
- Every implicit item is made explicit: default statuses, the implicit `ValidationError`, parameter bindings and wire names.

The Rust types live in `crates/veld-ir`. Their JSON form, `veld debug ir` and the plugin protocol, is described below. Every compatible change bumps `ir_version` minor, and every breaking change bumps it major.

## 1. Top level

```jsonc
{
  "ir_version": "1.1",
  "packages": [ { "id": 0, "name": "orders", "kind": "service" | "library" | "registry" | "prelude" } ],
  "decls":    [ Decl, ... ],          // arena; DeclId = index
  "services": [ Service, ... ],       // arena; ServiceId = index
  "consumes": { "<service>": ["<service>", ...] }   // may contain cycles
}
```
Arrays are sorted by canonical name, so the IR JSON for the same workspace is byte-identical on every run.

## 2. Declarations

```jsonc
Decl = {
  "id": 3, "package": 0, "module": "billing.invoices", "name": "Invoice",
  "qualified": "orders:billing.invoices.Invoice",
  "sig": "veld:orders/Invoice@9f2c41d07ab3e855",   // §5
  "docs": "markdown or null",
  "deprecated": "msg or null",
  "kind": Model | Error | Enum | Union | Const
}
Model  = { "kind": "model", "parent": DeclId|null, "fields": [Field] }  // fields = own + inherited, parent first
Error  = { "kind": "error", "default_status": 404|null, "fields": [Field] }
Enum   = { "kind": "enum", "variants": [ { "name": "paid", "wire": "PAID", "docs": .., "deprecated": .. } ] }
                                                          // state machines (spec/rules.md §10.4); both keys omitted when empty:
Enum  += { "flows": [ { "from": "pending", "to": "paid" } ],   // variant names, first declaration order, no duplicates
           "finals": [ "cancelled" ] }                          // variant names, declaration order
Union  = { "kind": "union", "discriminator": "kind", "arms": [ { "label": "card", "decl": DeclId } ] }
Const  = { "kind": "const", "type": Type, "value": Literal }
Field  = { "name": "firstName", "wire": "first_name", "type": Type, "optional": false,
           "default": Literal|null, "readonly": false, "inherited_from": DeclId|null,
           "constraints": Constraints, "example": Literal|null,
           "docs": .., "deprecated": .. }
Type   = { "scalar": "string|int|long|float|decimal|bool|date|datetime|time|uuid|bytes|json" }
       | { "list": Type } | { "map": Type } | { "page": Type } | { "decl": DeclId }
Constraints = { "min": n|null, "max": n|null, "min_len": n|null, "max_len": n|null,
                "pattern": "re"|null, "format": "email|uri|hostname|ipv4|ipv6"|null }
```
In Rust, `Type`, `DeclKind`, `Feature` and the other IR enums are deliberately **not** `#[non_exhaustive]`, and inside the workspace nothing matches them with a wildcard `_ =>` arm (clippy enforces this with `wildcard_enum_match_arm`). Adding a variant therefore breaks the build at every site that has not handled it, so "nothing forgotten" is checked by the compiler. External plugins are protected by other means: a plugin only sees the JSON IR, which is versioned by `ir_version`, and a new `Feature` is only ever sent to a plugin whose declared capabilities cover it (§4 and `spec/plugin-protocol.md`).

## 3. Services

```jsonc
Service = {
  "id": 0, "package": 0, "name": "Orders", "prefix": "/orders",
  "base_url": "http://orders:3002" | null,
  "docs": .., "auth": Auth, "middleware": ["RateLimit"],
  "endpoints": [Endpoint]                 // source order
}
Endpoint = {
  "name": "getOrder", "method": "GET", "path": "/orders/{id}",
  "sig": "veld:orders/Orders.getOrder@51ab0e9c33d2f7a0",   // §5
  "docs": .., "deprecated": .., "auth": Auth, "middleware": [..],
  "params": [ { "name": "id", "binding": "path|query|query_object|header|body",
                "header": "Idempotency-Key"|null, "type": Type, "optional": false,
                "stream": false, "constraints": Constraints } ],
  "success": { "status": 200, "type": Type|null, "stream": false },
  "errors":  [ { "status": 404, "decl": DeclId } ]      // includes implicit ValidationError; sorted by status, then sig
}
Auth = { "kind": "none|bearer|basic|api_key", "header": "X-API-Key"|null }
```

### 3.1 Endpoint rules, service facts and examples

Three additions carry endpoint rules (`spec/rules.md` §9) and examples (§12). Each key is **omitted from the JSON when empty or absent**, so the IR of a contract without them is unchanged:

```jsonc
Endpoint += { "rules": EndpointRules }                    // facts, requires and ensures of the endpoint block
EndpointRules += { "transition": EndpointTransition }         // omitted when absent; spec/rules.md §10.4
Service  += { "facts": [ ServiceFact ],                   // one loader method per fact name, sorted by name
              "rules_digest": "3fa9c2e1b7d04a55" }        // spec/rules.md §9.7
Ir       += { "examples": [ Example ] }                   // spec/rules.md §12
```

`EndpointRules`, `ServiceFact`, `Example` and `ExampleValue` (`json`, `ref`, `object`, `list`) are defined in `spec/rules.md` §13.1 and described by `spec/ir.d.ts`. An endpoint with `rules` uses the feature `rules_endpoint`; examples need no feature. An enum with `flows`, or an endpoint with a `transition`, uses the feature `rules_transitions`; a generator that cannot represent it reports `E0801` at the first flow or transition.

### 3.2 Policies, the principal and `allow`

Three more keys carry policies (`spec/rules.md` §11), each **omitted from the JSON when empty or absent**:

```jsonc
Ir            += { "policies": [ Policy ] }                          // sorted by qualified name
Service       += { "principal": { "decl": DeclId, "name": "user" } } // the principal of the service's package
EndpointRules += { "allows": [ { "policy": 0, "args": [Expr], "reads": ["id"], "origin": Origin } ] }
                                                                     // in source order; "policy" indexes Ir.policies
Policy         = { "name", "package", "module", "qualified", "id": "veld:<package>/<Policy>@<hex16>",
                   "params": [ { "name", "type": Type } ],
                   "clauses": [ { "effect": "permit" | "forbid", "expr": Expr, "message": "…" | null, "origin": Origin } ],
                   "docs", "origin" }
```

In a policy's expressions a parameter is the root `{ "param": name }`; in an endpoint block the caller is the root `"principal"`. An endpoint with an `allow` uses the feature `rules_policies`; a generator that cannot represent it reports `E0801` at the first `allow`. A policy that no endpoint uses needs no feature. An endpoint with an `allow` also declares `Forbidden` among its `errors`.

### 3.3 Events

Events, channels, `publishes` and `subscribes` are described in `spec/events.md` §13. Every key is **omitted from the JSON when empty or absent**: `Decl.kind` `"event"` (with `fields`, `rules`, `rules_digest`), `Ir.channels`, `Service.publishes`, `Service.subscriptions`, and in an `Example` the keys `event` and `expect`. An example of the event form has no `service`, `endpoint` or `outcome`. `Service.prefix` is `""` for a service without `at`. A `Channel` has `"retry": { "max": 5, "backoff": "fixed" | "exponential" }`, `"dead_letter": true` and `"ordered": true` for `@retry`, `@deadLetter` and `@ordered` (`spec/events.md` §10.2), each omitted when absent, and an event example's `expect` entry has `"once": true` for `called once`. A service that publishes or subscribes to a channel with one of the three annotations uses the opt-in feature `events_delivery` (`spec/events.md` §10.8); a generator that does not declare it reports `E0801` at the name of that channel. A service with `publishes` or `subscriptions` uses the feature `events`; a generator that cannot represent it reports `E0801` at the first such line. `Service.publishes` is a list of `{ "channel": <index>, "origin": Origin }`, one entry per channel in source order; the origin is the position of the channel name in its `publishes` line, so that the first one is where `E0801` points.

### 3.4 Rolling a rule out

A `Rule` (an invariant of a model, an error or an event), an `EndpointRequire` and an `EndpointEnsure` carry `"rollout": "audit"` when the source has `@rollout(audit)` (`spec/rules.md` §17.1). The key is **omitted from the JSON when the rule is enforced**, so the IR of a contract without the annotation is unchanged. The rule id and the signature of the declaration do not depend on it. A service that reaches an audited invariant (of a model or an error its endpoints reach) or has an audited `require` or `ensure` uses the feature `rules_rollout`; so does a service that publishes or subscribes to a channel whose event has an audited invariant, on the event or on a declaration the event reaches (`spec/rules.md` §17.1; a client generates nothing for events and is not concerned); a generator that does not declare it reports `E0801` at the first audited rule. The `rules_digest` of a service and of an event covers only the invariants that are enforced, and a service whose reached invariants are all audited has none.

### 3.5 Versions and idempotent endpoints

`@version` and `@idempotent` (`spec/rules.md` §11.7 to §11.9) add four keys, each **omitted from the JSON when absent**, so the IR of a contract without the annotations is unchanged:

```jsonc
Field         += { "version": true }                       // the @version field of a model (never false: the key is left out)
EndpointRules += { "version": { "id": "veld:<pkg>/<Service>.<endpoint>.version@<hex16>",
                                "fact": "room", "field": "revision", "wire": "rev", "origin": Origin } }
Endpoint      += { "idempotency": { "header": "Idempotency-Key", "required": false, "caller": true, "origin": Origin } }
Example       += { "if_match": "any" | { "version": "3" },
                   "key": { "value": "k-1", "store": "other" | "running" | { "stored": ExampleOutcome } } }
```

`EndpointRules.version` is the guard: the first fact, in declaration order, whose type is a model with a `version` field (an optional fact too: when it is absent there is nothing to compare). `id` is the rule id of §11.9; `field` and `wire` name the version field of the fact's model. `Idempotency.caller` is true when the endpoint's effective auth is not `none`: the scope of a key then includes the caller. `ExampleKey.store` is left out when the line is the key alone (the store holds nothing for it). The errors of the two features are in `Endpoint.errors` like any other: `PreconditionFailed` (412) with a guard, `Conflict` (409) with `@idempotent`, and `ValidationError` (422) with either, because a malformed header is refused with it. They are not declared by the author and they appear in the IR only when a contract reaches them, like `InvalidTransition`. No declaration signature and no operation signature depends on either annotation.

A service uses the feature `rules_version` when an endpoint has a guard **or** answers with a model that has a `version` field (the `ETag`), and `idempotency` when an endpoint has `idempotency`. Both are opt-in, like `rules_rollout`: `Capabilities::all_except` does not hand them out and a generator adds them with `Capabilities::with` once it implements them; until then it reports `E0801` at the first use (the fact of a guard, the endpoint that only owes an `ETag`, the `@idempotent` line). A guard is not a statement: an endpoint that has only a guard and no other statement does not use `rules_endpoint` on that account (a guard needs a fact, so it always has one).

## 4. Capabilities

The compiler computes the set of features that a target needs (`Feature` enum: `unions`, `constraints`, `auth`, `headers`, `streaming_sse`, `streaming_ws`, `page`, `consts`, `bytes`, `rules_invariants`, `rules_endpoint`, `rules_transitions`, `rules_policies`, `rules_rollout`, `rules_version`, `idempotency`, `events`, `events_delivery`, …). A feature that changes what a rule means or what a request does (`rules_rollout`, `rules_version`, `idempotency`, `events_delivery`) is opt-in: `Capabilities::all_except` does not hand it out, and a generator adds it with `Capabilities::with` once it implements it. Before it invokes a generator, the core compares that set with the generator's declared `Capabilities`. For every feature that is used but not supported, it emits `E0801` with the span of the first use.

## 5. Identity signatures (normative)

Signatures make generated types comparable across packages, repos and languages without any shared runtime. Every generator MUST embed both signatures exactly as the IR gives them. CI compares them byte for byte (`tests/wire/`).

### 5.1 Declaration signature

```
decl_sig  = "veld:" package "/" Name "@" hex16( sha256( shape_bytes ) )
```
- `package` is the package name. Registry packages keep their `@org/name`. Prelude declarations use `veld`.
- `hex16` is the first 16 lowercase hex characters of the digest.
- `shape_bytes` is RFC 8785 canonical JSON of the object below:
  ```jsonc
  { "root": "<qualified>", "decls": { "<qualified>": LocalShape, ... } }
  ```
  `decls` contains the root declaration and every declaration it reaches transitively, keyed by qualified name. Cycles are allowed, because references are by name.
- `LocalShape` contains only what affects the meaning of the data on the wire:
  - model or error: `{ "k": "model"|"error", "f": [ { "w": wire, "t": TShape, "o": optional } ...sorted by w ] }`, with inherited fields included
  - enum: `{ "k": "enum", "v": [wire values sorted] }`
  - union: `{ "k": "union", "d": discriminator, "a": { label: "<qualified>" } }`
  - `TShape`: `"string"`, …, `{ "list": TShape }`, `{ "map": TShape }`, `{ "page": TShape }`, `{ "ref": "<qualified>" }`
- The signature does not include docs, deprecations, constraints, defaults, examples or source spans. Renaming a field in source while keeping `@wire` the same keeps the signature.
- Each `Decl.sig` is computed once by `veld-compiler::signature`. No other code computes signatures.

### 5.2 Operation signature

```
op_sig    = "veld:" package "/" Service "." endpoint "@" hex16( sha256( op_bytes ) )
op_bytes  = RFC 8785 JSON { "m": METHOD, "p": path with every {param} replaced by {} }
```

### 5.3 Canonical value bytes

The canonical bytes of a value are the RFC 8785 JSON of its wire form after decoding:
- defaults are applied
- absent optional fields are omitted
- scalar formats follow `language.md` §5.1: UTC datetimes with `Z`, lowercase uuids, padded base64

Every generator's runtime has `canonical(value) -> bytes`.

### 5.4 Equality (the same in every target)

```
same(a, b) :=
     a.decl_sig == b.decl_sig
  && (a is error  =>  a.op_sig == b.op_sig)
  && canonical(a) == canonical(b)

X.matches(err) := err.decl_sig == X.decl_sig
                  && service_of(err.op_sig) == service_of(X's declaring context)
```

- Canonical bytes alone never decide: two different declarations with identical fields have identical bytes, so every target needs the declaration signature of **both** operands. A value that carries none cannot be compared with `same(a, b)`; a target either refuses loudly (TypeScript throws a `TypeError`) or is false (Java), and offers a declaration-qualified form (`Model.same(a, b)`) for plain data.
- The comparison is by signature, never by class or prototype identity, so two copies of `Orders.NotFound` generated into different packages are `same` when the service, operation and payload all match.
- A `NotFound` from `payments`, or from a different operation, is never `same`.
- `matches` is the looser check: it means "this declaration, raised by this service, from any operation".

### 5.5 Errors on the wire

Every error response is RFC 9457 `application/problem+json`:
```json
{ "type": "veld:orders/OutOfStock@1c0d…", "title": "OutOfStock", "status": 409,
  "operation": "veld:orders/Orders.createOrder@77e1…", "data": { "sku": "A-1" } }
```
A client recovers a typed error from `type`, falling back to `status` if the type is unknown. Errors that are not described this way, for example HTTP 502 from a proxy, surface as `VeldTransportError`.
