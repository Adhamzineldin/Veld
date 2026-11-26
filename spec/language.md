# Veld Contract Language — v2

Status: **DRAFT — awaiting owner freeze (Day 1 gate).** After the freeze, any change needs owner sign-off and a matching conformance case.

This document is normative. The keywords MUST, MUST NOT and SHOULD carry their usual meaning. Every rule here is backed by at least one valid and one invalid case in `tests/conformance/`.

## 1. Overview

```veld
use iam:users.{User}

/// A customer order.
model Order {
  id: uuid @readonly
  customer: User
  items: OrderItem[] @length(min: 1)
  status: Status = .pending
  note?: string @length(max: 500)
}

enum Status { pending, paid, shipped }

error OutOfStock 409 {
  sku: string
}

/// Order management.
@auth(bearer)
service Orders at "/orders" {
  /// Fetch one order.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound

  post "/" createOrder(body: CreateOrder) -> 201 Order | OutOfStock

  get "/" listOrders(query: OrderFilter) -> Page<Order>

  @auth(none)
  get "/health" health() -> 204
}
```

## 2. Projects, packages and modules

- A **package** is a directory tree of `.veld` files with one root. The package kinds are:
  - a workspace service (a `[service]` member of the workspace)
  - a workspace library (a `[library]` member)
  - a registry package (`@org/name`)
- A **module** is one `.veld` file. Its module path is the file path relative to the package root, with `/` replaced by `.` and the `.veld` extension removed. For example, `contracts/orders/billing/invoices.veld` inside package `orders` is the module `billing.invoices`.
- Module path segments MUST be valid identifiers (§3.2). If a segment is not, the error is `E0101`.
- Every top-level declaration is visible to any module that imports it. There is no `export` keyword and no private scope.
- Declaration names MUST be unique within a package (`E0201`), even when they are in different modules. This gives every declaration one canonical name `<package>:<module>.<Name>`.

### 2.1 Imports

```
use users.{User, Role}                  // same package
use iam:users.{User as IamUser}          // another workspace service or library
use @acme/billing:invoices.{Invoice}     // registry package (must be in veld.lock)
```

- Every name that does not come from the current module or the prelude MUST be imported explicitly. Glob imports do not exist.
- `as` renames a name locally. An imported name that collides with a local name is an error (`E0202`).
- An import that is never used is a warning (`W0201`).
- Imports MAY form cycles, between modules and between packages. Resolution has two phases: first every declaration in every reachable module is collected, then references are resolved. A cycle is never an error.
- Unknown package: `E0203`. Unknown module: `E0204`. Unknown name: `E0205`.
- v1 forms such as `import @alias/x` and `import "./x.veld"` are syntax errors (`E0001`), and the hint says "this is v1 syntax; see spec/language.md".

## 3. Lexical structure

- Source files are UTF-8. A BOM is ignored. Line endings may be LF or CRLF.
- Whitespace is not significant, except that `///` doc comments attach to the next declaration, field or endpoint.
- Comments:
  - `// line`
  - `/* block */`, which does not nest
  - `/// doc`, which is Markdown and becomes `docs` in the IR

### 3.1 Tokens

```
IDENT     = [A-Za-z_][A-Za-z0-9_]*
INT       = "-"? [0-9]+
FLOAT     = "-"? [0-9]+ "." [0-9]+ ([eE] [+-]? [0-9]+)?
STRING    = '"' ( escape | [^"\\\n] )* '"'      escape = \\ \" \n \t \r \u{XXXX}
STATUS    = INT in 100..=599 (checked semantically)
PUNCT     = { } ( ) [ ] < > , : . ? = | @ -> *
```

### 3.2 Keywords

Reserved words, which cannot be used as identifiers:
`use as model enum error union service at extends const stream true false null`

Contextual words, which are keywords only where the grammar expects them and are ordinary identifiers everywhere else (so a field may be called `get` or `body`):
`get post put patch delete head options ws`

The words of rules (`spec/rules.md` section 2.2) and of events (`spec/events.md` section 2: `event channel carries key publishes subscribes on called rejected`) are contextual in the same way.

The single source of truth for both lists is `crates/veld-compiler/src/language.rs`. The editor grammars are generated from that file.

## 4. Grammar (EBNF)

```ebnf
file        = { use } { decl } EOF ;
use         = "use" [ pkg ":" ] modpath "." "{" import { "," import } [","] "}" ;
pkg         = IDENT | "@" IDENT "/" IDENT ;
modpath     = IDENT { "." IDENT } ;
import      = IDENT [ "as" IDENT ] ;

decl        = { doc } { annotation } ( model | enum | error | union | service | const ) ;
model       = "model" IDENT [ "extends" typeref ] "{" { field } "}" ;
error       = "error" IDENT [ STATUS ] "{" { field } "}" ;
field       = { doc } { annotation } IDENT [ "?" ] ":" type [ "=" literal ] { annotation } ;
enum        = "enum" IDENT "{" variant { "," variant } [","] "}" ;
variant     = { doc } { annotation } IDENT [ "=" STRING ] ;
union       = "union" IDENT "{" unionarm { "," unionarm } [","] "}" ;
unionarm    = IDENT ":" typeref ;
const       = "const" IDENT ":" type "=" literal ;

service     = "service" IDENT "at" STRING "{" { endpoint } "}" ;
endpoint    = { doc } { annotation } method STRING IDENT "(" [ params ] ")" [ "->" responses ] ;
method      = "get" | "post" | "put" | "patch" | "delete" | "head" | "options" | "ws" ;
params      = param { "," param } [","] ;
param       = { annotation } IDENT [ "?" ] ":" [ "stream" ] type ;
responses   = success { "|" errorresp } | errorresp { "|" errorresp } ;
success     = STATUS [ [ "stream" ] type ] | [ "stream" ] type ;
errorresp   = [ STATUS ] typeref ;

type        = typeref [ "[" "]" ] { "[" "]" } ;
typeref     = IDENT [ "<" type { "," type } ">" ] ;
literal     = STRING | INT | FLOAT | "true" | "false" | "null" | "." IDENT | "[" "]" ;
annotation  = "@" IDENT [ "(" [ arg { "," arg } [","] ] ")" ] ;
arg         = [ IDENT ":" ] ( literal | IDENT ) ;
doc         = DOC_COMMENT ;
```

Annotations may appear before a field (`@deprecated("…") name: T`) or after it (`name: T @deprecated("…")`). The two positions mean the same thing. The formatter always prints them after the field.

## 5. Types

### 5.1 Scalars

| Veld | Meaning | Canonical JSON |
|---|---|---|
| `string` | UTF-8 text | string |
| `int` | signed 32-bit | number |
| `long` | signed 64-bit | string (to stay exact in JS) |
| `float` | IEEE-754 double | number |
| `decimal` | arbitrary-precision decimal | string |
| `bool` | boolean | `true`/`false` |
| `date` | RFC 3339 full-date | string `YYYY-MM-DD` |
| `datetime` | RFC 3339 date-time, UTC on the wire | string `YYYY-MM-DDTHH:MM:SS[.fff]Z` |
| `time` | RFC 3339 partial-time | string |
| `uuid` | RFC 9562 UUID | lowercase string |
| `bytes` | binary | base64 string (RFC 4648, padded) |
| `json` | any JSON value | as is |

### 5.2 Composite types
- `T[]` is a list. Lists of lists are written `T[][]`.
- `Map<string, T>` is a map. Its key MUST be `string` (`E0301`).
- `Page<T>` is a built-in generic: `{ items: T[], total: long, page: int, size: int }`.
- No other generic types exist. `Foo<T>` on a user declaration is `E0302`.

### 5.3 Optionality
- `name?: T` means the field MAY be absent. `null` is not accepted for an optional field. It is accepted only where the type is `json`.
- A required field MUST be present.
- A default (`= literal`) is applied when the field is absent. A field with a default is required in the output and optional in the input.

### 5.4 Declarations
- **model**: a record with named fields.
  - `extends` takes exactly one parent model. Fields are inherited.
  - Redeclaring an inherited field is an error (`E0401`).
  - An inheritance cycle is an error (`E0402`).
- **enum**: string-valued.
  - The wire value is the variant name unless it is overridden with `= "WIRE_VALUE"`.
  - Wire values MUST be unique (`E0403`).
- **union**: a discriminated union whose arms are models.
  - The discriminator property defaults to `kind`. `@discriminator("type")` on the union overrides it.
  - The arm label (`card:` in `union Payment { card: Card, bank: BankTransfer }`) is the discriminator value.
  - An arm model that already has a field with the discriminator name is an error (`E0404`).
- **error**: a model with error identity.
  - It can be used only in error position of an endpoint (`E0405`).
  - The optional `STATUS` is its default status, and it MUST be 4xx or 5xx (`E0406`).
- **const**: a named literal that is emitted as a constant in every target.
- **Recursion:** a type may refer to itself only through `?`, `[]`, `Map` or a union arm. A required self-cycle such as `model A { a: A }` is `E0407`, because no value of that type can exist.

### 5.5 Prelude
These names are implicitly in scope in every module. A local declaration may shadow them, which produces a warning (`W0202`).

```veld
error BadRequest 400      { message: string }
error Unauthorized 401    { message?: string }
error Forbidden 403       { message?: string }
error NotFound 404        { resource?: string }
error Conflict 409        { message: string }
error ValidationError 422 { issues: ValidationIssue[] }
error TooManyRequests 429 { retryAfter?: int }
error Internal 500        { message?: string }
model ValidationIssue     { path: string, code: string, message: string }
error InvalidTransition 409 { field: string, from: string, to: string }
error PreconditionFailed 412 { expected: long, actual: long }
```

Prelude errors belong to the reserved package `veld`. Their identity at runtime still includes the service and operation that raised them (see `spec/ir.md` §5). A `NotFound` raised by `orders.getOrder` therefore never compares equal to a `NotFound` raised by `payments.getPayment`.

## 6. Services and endpoints

- `service Name at "/prefix"` groups endpoints. The prefix MUST start with `/` (`E0501`). A service with endpoints MUST have an `at` (`E0501`); a service that only publishes or subscribes (`spec/events.md` section 5) has none, and its prefix is the empty string.
- **Path templates** use `{name}` segments. The full path is the prefix joined with the endpoint path. A trailing `/` is normalised away, except on the root path.
- **Parameter binding**, in this order:
  1. The name appears in the path template: it is a **path** parameter. It MUST be a scalar or an enum, and MUST NOT be optional (`E0502`).
  2. The parameter is named `body`: it is the request **body**. At most one is allowed. It is not allowed on `get`, `head` or `options` (`E0503`).
  3. The parameter is named `query`: its type MUST be a model whose fields are scalars, enums or lists of them (`E0504`). Its fields become query parameters.
  4. The parameter is annotated `@query`: it is a single query parameter, and it may be optional.
  5. The parameter is annotated `@header("Header-Name")`: it is a header.
  6. Anything else is `E0505`, with a hint that lists the options above.
  
  A `{name}` in the path with no matching parameter is `E0506`.
- **Responses:**
  - **Success:** at most one, and its status MUST be 2xx (`E0507`).
    - If no status is written, it defaults to `201` for `post` and `200` otherwise.
    - With no `->` at all, or with a bare status such as `-> 204`, the response has no body. When nothing is written the status is `204`.
  - **Errors:** each MUST be an `error` declaration (`E0405`).
    - The status is the one written at the use site, otherwise the error's default status, otherwise `E0508`.
    - Two errors with the same status are allowed, and they are told apart on the wire by `type` (see `spec/ir.md` §5).
  - **Implicit `ValidationError`:** every endpoint that has a body, query, header or path parameters implicitly declares `| ValidationError` unless it already lists one.
  - **Implicit `Unauthorized` and `Internal`:** every endpoint whose effective auth is not `none` implicitly declares `| Unauthorized` (401), and every endpoint implicitly declares `| Internal` (500), unless already listed. Together with `ValidationError` this means every response the server runtime can produce has a declared signature.
- **Endpoint names** MUST be unique within a service, and so must its event handler names (`E0509`). The pair (method, full path) MUST be unique across all services of one package (`E0510`). Paths are compared with parameter names erased: `/a/{x}` and `/a/{y}` collide.
- **Streaming** (capability-gated; the wire format is in the target specs: `spec/targets/typescript.md` §4b, `spec/targets/java.md`):
  - On `get`, `-> stream T` is Server-Sent Events.
  - On `ws`, `-> stream T` sends server-to-client messages, and `body: stream T` receives client-to-server messages.
  - `stream` is not allowed anywhere else (`E0511`).

## 7. Annotations

An unknown annotation is `E0601`. Annotations with the wrong arguments or the wrong target are `E0602`.

| Annotation | Target | Meaning |
|---|---|---|
| `@auth(bearer \| basic \| none \| apiKey(header: "X"))` | service, endpoint | Auth requirement. The endpoint's value overrides the service's. |
| `@middleware(Name, …)` | service, endpoint | Named hooks that the generated server asks the user to implement. |
| `@deprecated("msg")` | any declaration, field, variant, endpoint | Deprecation notice. |
| `@min(n)` / `@max(n)` | numeric field/param | Inclusive bounds. |
| `@length(min: n, max: n)` | string, list, map | Length bounds. |
| `@pattern("regex")` | string | ECMAScript-compatible subset (§7.1). |
| `@format(email \| uri \| hostname \| ipv4 \| ipv6)` | string | Format check. |
| `@readonly` | field | Set by the server. Ignored if present in a request body, and always present in responses. |
| `@wire("name")` | field | Overrides the JSON property name. The source name is kept for identifiers. |
| `@example(literal)` | field, param | Documentation and mock data. |
| `@discriminator("prop")` | union | Discriminator property. |
| `@query` / `@header("Name")` | param | Binding (§6). |
| `@rollout(audit)` | invariant, `require`, `ensure` | The rule is evaluated and reported and never rejects (`spec/rules.md` §17.1). `E0602` anywhere else. |
| `@version` | one `int` or `long` field of a model | The version of the model: the `ETag` of a response and what `If-Match` is compared with (`spec/rules.md` §11.7). `E0602` on another type, an optional field, a field of an error or an event, or a second one. |
| `@idempotent` / `@idempotent(header: "Name", required: true)` | endpoint | The endpoint answers a request once per key (`spec/rules.md` §11.8). `E0602` on a stream or `ws` endpoint, or for a header that is not a token, that Veld or HTTP uses (`Authorization`, `Content-Type`, `Content-Length`, `Accept`, `Host`, `If-Match`, `ETag`, `Veld-*`) or that the endpoint's own parameters or `apiKey` already use. |

### 7.1 Regex subset
The allowed syntax is literal characters, `.`, classes `[...]`, `\d \w \s`, the quantifiers `* + ? {n,m}`, groups `( )`, alternation `|`, and the anchors `^ $`. Lookaround and backreferences are `E0603`, because every target must enforce the same pattern.

## 8. Names
- Identifiers in the contract are the **source names**.
- Wire names are the source names unless `@wire` overrides them.
- Each generator derives target identifiers through `veld-codegen::naming`, which handles casing and escapes reserved words. Wire names never change.
- Lint (not error): types and services SHOULD be `PascalCase`, fields and endpoints `camelCase`, and enum variants `camelCase` or `snake_case`.

## 9. Diagnostics
- Every diagnostic has a stable code (`E####` error, `W####` warning), a primary span and optional secondary labels.
- Codes are never reused. Ranges:
  - `E00xx` syntax
  - `E01xx` modules
  - `E02xx` names
  - `E03xx` types
  - `E04xx` declarations
  - `E05xx` services
  - `E06xx` annotations
  - `E07xx` workspace/config
  - `E08xx` generation (for example `E0801 E_UNSUPPORTED_FEATURE`)
- The full catalogue is `spec/diagnostics.md`, which is generated from the compiler.

### 9.1 Codes added during implementation (2026-11-20)

| Code | Meaning |
|---|---|
| `E0004` | Reserved word used as a name |
| `E0005` | Nesting too deep: types (`Map<string, ...>`, `T[][]`) and annotation call arguments are limited to 64 levels |
| `E0006` | `extends` chain longer than 64 models |
| `E0303` | Default/example literal does not match the field type (also: optional field with a default) |
| `E0304` | Enum default names an unknown variant |
| `E0305` | Float literal is not finite (overflows a 64-bit float, for example `1.0e999`); applies to defaults, examples, consts and `@min`/`@max` |
| `E0408` | `extends` target is not a model |
| `E0409` | Union arm is not a model, or union is empty |
| `E0410` | Duplicate field, wire name, variant or arm |
| `E0512` | Duplicate parameter |
| `W0001` | Doc comment not attached to anything |

Clarifications:
- Commas between fields, variants and arms are optional.
- A trailing field annotation must be on the same line as the field.
- Parameters take annotations only before the name.
- In a response list, the first item that resolves to an `error` declaration starts the error part. A bare status (`-> 204`) is allowed only as the first item.
- An endpoint's effective auth is its own if it has one, otherwise the service's. Its effective middleware is the service list followed by its own, with duplicates removed.
