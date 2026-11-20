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
  "ir_version": "1.0",
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
Union  = { "kind": "union", "discriminator": "kind", "arms": [ { "label": "card", "decl": DeclId } ] }
Const  = { "kind": "const", "type": Type, "value": Literal }
Field  = { "name": "firstName", "wire": "first_name", "type": Type, "optional": false,
           "default": Literal|null, "readonly": false, "inherited_from": DeclId|null,
           "constraints": Constraints, "meta": ["unique","index","primary"], "example": Literal|null,
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

## 4. Capabilities

The compiler computes the set of features that a target needs (`Feature` enum: `unions`, `constraints`, `auth`, `headers`, `streaming_sse`, `streaming_ws`, `page`, `consts`, `bytes`, …). Before it invokes a generator, the core compares that set with the generator's declared `Capabilities`. For every feature that is used but not supported, it emits `E0801` with the span of the first use.

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
- The signature does not include docs, deprecations, constraints, defaults, examples, `meta` or source spans. Renaming a field in source while keeping `@wire` the same keeps the signature.
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
