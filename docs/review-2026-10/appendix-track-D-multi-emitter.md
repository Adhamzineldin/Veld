# Appendix: audit-track notes (D-multi-emitter)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Multi-emitter audit (non-Node emitters + auxiliary generators)
Repo D:\University\Veld @ 6a47f81. Audit date 2026-10-21. Scratch: <scratchpad>/multi-audit (contract/, out/, py/, ws/, aux/).
Environment limits (not defects): no cargo, dotnet, php, psql, Maven/Gradle, Dart/Kotlin/Swift compilers. Those emitters were judged by reading only, plus `javac` for Java (with stub Jackson annotations). The command-line `curl` is blocked in this sandbox; HTTP calls were made with a Python urllib script (multi-audit/py/hit.py).

## 1. Run log
Shared contract: multi-audit/contract/{app.veld, models/all.veld, modules/items.veld} (all scalars, optional, array, Map, enum @default, extends, recursive Node, GET path+query, POST 201 + headers + middleware, PUT, PATCH, DELETE no output, @deprecated action+field). `veld validate` OK. Note: imports must be repeated in each module file that uses a type (a model imported only from app.veld is rejected with "defined in all.veld but not imported").
Build: `go build -o multi-audit/veld.exe ./cmd/veld` OK. `go test ./internal/emitter/... ./internal/generators/...` all PASS (tests only assert substrings; see F-MULTI-02).
Generate: `veld.exe generate --backend=<t> --frontend=none --out=../out/<t>` for python, go, rust, java, csharp, php, node-js; `--backend=none` is rejected (frontends generated with `--backend=node-ts --frontend=X`).

### Part A Python (venv, flask + pydantic installed)
- `generate --backend=python` (default, no framework, no --validate): `python -m compileall` -> SyntaxError routes/items_routes.py:41 `input = CreateItemSchema(**body = data  # CreateItemSchema).model_dump()`. Also: routes file lacks `from flask import request`; imports `..schemas.schemas`, which is never emitted (no schemas/ dir, no Pydantic output despite CLAUDE.md); routes are never registered (only a comment `# register: ['GET'] /api/items/<id>`); error path does `raise RuntimeError(body)` rather than returning a response.
- `generate --backend=python --backend-framework=flask --validate`: compileall -> SyntaxError models/items.py:218 `def __init__(self, idempotency-_key: str ...` (header name "Idempotency-Key" used as identifier). Importing routes fails because routes -> interfaces -> models.
- After hand-patching only that identifier (in a copy, py/pkg) the app ran (port 5055). Results:
  - GET /api/items/abc?limit=5&q=x -> first run 500 `name 'Decimal' is not defined` (_validators.py uses Decimal, never imports it). After adding the import: 200; the service received `{'limit':'5','q':'x'}`, a plain dict of strings (not ItemQuery, limit not int). Decimal output serialised as string "1.10" by Flask >= 2.2 jsonify.
  - POST valid -> 201; service got headers `{}` (route has comment "Extracted from request headers" but extracts nothing; Idempotency-Key/Authorization never reach the service).
  - POST {"name":5} -> 400 {"code":"VALIDATION_ERROR",...} good. POST role:"bogus" -> 201 (enum not validated). POST non-JSON body -> HTTP 500 (Flask BadRequest caught by `except Exception`; body code field says 400).
  - PUT where service raises ValueError -> 500 {"error":"boom"} (all exceptions caught, message leaked to client). PATCH where service returns None -> 500 contract violation. DELETE -> 204. Static routes /old, /tree after /<id>: fine in Flask.
  - The ABC declares `-> Item` (class with get_/set_ methods) but routes require a `dict` (assert_item) and jsonify cannot serialise Item objects: a service honouring the interface type fails at runtime.
  - Model classes use snake_case attributes (created_at) while JSON/validators use camelCase (createdAt), so `from_dict(cls(**data))` breaks on wire input. Mutable default args (`tags: List[str] = []`).
  - date/datetime/uuid are plain `str`, no format validation. Recursive Node: assert_node recurses fine.

### Part B Go
- Output ships `go.mod` (module example.com/veld-generated, go 1.22, stdlib only for default; chi adds a require without go.sum).
- `go build ./...` / `go vet ./...` on raw output: FAIL, internal/models/items.go:184 `Idempotency-key string` (syntax). After renaming the field in a copy (out/go2): FAIL `internal/routes/items.go:14:2: undefined: mux` (function param is `r *http.ServeMux` but body calls `mux.HandleFunc`, from strategy/plain.go:21). After sed `mux`->`r`: builds and vets clean; server.go compiles.
- Ran a ~30-line main implementing ItemsService: GET path param OK (200); POST valid -> 201; DELETE -> 204; invalid JSON -> 400 {"error":"invalid request body"}; `{}` body -> 201 (NO validation of required fields, enums, formats; plain json.Decoder only); role:"bogus" accepted; service error -> 500 with raw err.Error() (no typed status mapping); query `*ItemQuery` and headers `*CreateItemHeaders` are always `nil` (`svc.GetItem(r.Context(), id, nil)`).
- Mappings: decimal -> string; date AND datetime -> time.Time (a date "2020-01-01" fails decode: `parsing time "2020-01-01" as "2006-01-02T15:04:05Z07:00"`); int -> int (not int64); optional -> *T `omitempty`; Map -> map[string]string; enum -> `type Role string` + consts; extends -> embedded struct (but `NewItem` constructor omits Base fields); nil slices serialise as JSON null; recursive Node -> []Node OK.
- `--backend-framework=chi`: go.mod requires chi v5.0.12, no go.sum; `go mod tidy` (network OK) succeeds and build passes (after header rename). Go 1.22 mux resolves /old vs /{id} correctly.

### Part C
Static inspection plus javac. See table in section 2 and F-MULTI-06..09. Java: `javac` (stub Jackson annotations) on raw output: 8 syntax errors (CreateItemHeaders hyphen); after renaming: 2 more (`Item.java:22 private Role role = "user"` String->Role; `ItemsController.java:94 missing return statement` in deleteItem). Node JS backend: `node --check types/items.js` SyntaxError line 141 `this.Idempotency-Key = ...`. Rust/C#/PHP/Dart/Kotlin/Swift read by eye; the same hyphen-identifier bug appears in all (root cause parser.go:961 builds synthetic `<Action>Headers` model with raw header names as field names).
- docs/architecture/overview.md:37 says "Actix handlers" for Rust; code is Axum-oriented (rust/errors.go:20 `use axum::`, middleware imports axum) but the emitted Cargo.toml does NOT list axum and the router is a stub. CLAUDE.md "Axum" is the closer description. docs/EXECUTION_PLAN_v1.md itself schedules "C1 axum / make it compile".

### Part D Service SDKs
Workspace ws/veld/veld.config.json: iam (node-ts, rich contract), txn (python/flask, consumes iam), cards (go, consumes iam,txn), gw (node-ts, consumes iam,txn).
- `veld deps` prints graph (txn->iam, cards->iam,txn); `deps --validate` "Workspace dependencies valid". `generate` -> sdk/ dirs for every consumer.
- WebSocket: syntax exists (`method: WS` with required `stream: T`, optional `emit: T`; validator.go:252). CLAUDE.md lists methods GET POST PUT DELETE PATCH only; language/constants.go also lists HEAD, OPTIONS, WS. The WS action `Live` is absent from all SDKs (skipped as claimed; sdk.go:243/364/204).
- Python SDK: compileall OK, imports OK. Env var: ctor arg > `VELD_IAM_URL` > baked "http://iam:3001" (verified). Error type `VeldApiError(status, body)` verified against a fake server (404 -> status 404). BUT typed model args do not work: `create_item(CreateItem(...))` -> TypeError "not JSON serializable"; `get_item("a", ItemQuery(limit=3))` -> AttributeError 'ItemQuery' has no attribute 'items'; only raw dicts work. Header params ignored. `urlopen` without timeout.
- Go SDK: `go build ./sdk/...` and `go vet` clean. Precedence arg > env > baked. `*VeldApiError`. No query-param support (GetItem has no query arg); http.DefaultClient without timeout; inheritance flattened as documented.
- TS SDK (gw, tsc 5 --strict): 24 errors: sdk/shared/index.ts uses UUID, ISODateTime, ISODate, Decimal without importing `_scalars` (22 errors), `role!: Role = "user"` TS1263, `query as Record<string,unknown>` TS2352 (iam/client.ts:44), TS2322 in models. Env var `process.env.VELD_IAM_URL` precedence correct.

### Part E Auxiliary generators
- OpenAPI: version 3.0.3. `redocly lint` 1.25.0: structurally valid; 10 errors are recommended-ruleset (no servers, trailing-slash path `/api/items/`, no `security`) plus 2 unused-component warnings. Representation: decimal = string/format decimal; date/date-time/uuid formats; optional omitted from required; enum $ref Role (default NOT emitted); extends = allOf [Base,{...}] with `required` beside allOf; path param + query model flattened into parameters; header model -> header params; 201 honoured; DELETE no output -> 204; every op gets 400/500 ErrorResponse ({code:int,message}, which does not match runtime shape {error}); middleware yields bare 401/403 but no securitySchemes; no servers although baseUrl exists; CreateItemHeaders/ItemQuery leak as unused schemas.
- GraphQL: `buildSchema` (graphql@16.8.1) FAILS: `Idempotency-Key: String!` invalid name. After renaming it parses. Map -> `meta: String` (lossy), uuid/date -> String (no ID/custom scalars), GET -> Query, POST/PUT/PATCH/DELETE -> Mutation, extends flattened, `Base` emitted as stand-alone type.
- Schema SQL (Postgres dialect; psql unavailable, judged by reading): every model including DTOs (ItemQuery, CreateItem, Msg, headers) becomes a table; `limit INTEGER` unquoted reserved word; `DEFAULT user` unquoted (in PostgreSQL this is current_user, silently wrong); arrays/maps as `VARCHAR(255)`; hyphen column `idempotency-_key`; pluralisation `item_querys`, `create_item_headerss`; extends flattened, no FKs. Prisma: models lacking `@id` (invalid), `@default(user)` unquoted (invalid), `Idempotency-Key String` invalid, Map as `String`.
- Docs: `veld docs` -> HTML by default, `--format=markdown` supported; markdown has method table, models, enums; `internal/docsgen` 1864 LOC, 0 tests; graphqlgen 0 tests.
- Mock: `internal/mock` (284 LOC, 12 tests) is imported by nothing outside itself; there is no `veld mock` command (`unknown command "mock"`).
- Dockerfile: Node template assumes user-owned package.json, `npm ci` (needs lockfile), `npm run build --if-present`, copies /app/dist (may not exist), `CMD ["node","index.js"]` (nonexistent entry unless user creates one), runs as root, downloads unpinned, unverified binary `.../releases/latest/download/veld-linux-amd64` which returns HTTP 404 (verified; the release publishes `.tar.gz` assets). docker-compose.yml bundles an unrelated postgres service named `registry` (password `changeme`, port 5432 published). Language detection is by marker files in the --out dir (go.mod, Cargo.toml, requirements.txt, package.json); default Node when none.
- CI workflow: same 404 install URL (fails at first step); npm-only unless marker files exist. Env: `.env.example` generic (PORT, DATABASE_URL, NODE_ENV) and not derived from the contract.
- `veld generate` post-generation hints print hardcoded names (`UserSchema`, `IUsersService`, `from pyflask.models import User`, `maayn.veld.generated.services.IUsersService`) unrelated to the contract.

## 2. Part C comparison table
| Emitter | Files emitted | Framework assumed | Runtime deps | Validation | Status codes | Path | Query | Headers | decimal/date/uuid | Optional | Enum | extends | Map | Recursive | Compile/syntax errors found | LOC (non-test) | Tests |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| node (ref) | types/interfaces/routes/schemas/client | any router | zod only if used | Zod / --validate | 201/204/overridable | yes | yes | req.headers | string/string/string | `?` | union | interface extends | Record | yes | not audited here | 1774 | 10 |
| python (flask) | models/items.py, interfaces/i_*.py, routes/*_routes.py, middleware, _validators.py (--validate), README | Flask (no registration without --validate) | flask; stdlib validators | only with --validate (shape check, enum NOT checked); default path references nonexistent Pydantic schemas | 201, 204; exceptions -> status attr or 500 | yes | untyped dict of strings | NOT extracted (empty dict) | Decimal / str / str | Optional | Literal (not validated) | class inheritance (snake/camel mismatch) | Dict | yes | models/items.py:218 (hyphen); routes:41 SyntaxError (default); Decimal NameError in validators | 2632 | 10 |
| go | go.mod, server.go, main.go, internal/{models,interfaces,routes,middleware} | stdlib net/http (chi opt-in) | none (chi opt-in) | none (JSON decode only; 400 on malformed) | 201/204/200; service errors always 500 | yes (PathValue) | passed nil | passed nil | string / time.Time / time.Time (date breaks) | *T omitempty | string type + consts | embedded struct | map[string]string | []Node | items.go:184 hyphen; routes/items.go:14 `mux` undefined | 2061 | 9 |
| rust | Cargo.toml, src/{lib,main,router,services,errors,items/mod,items_middleware,models/*}.rs | Axum (docs/architecture/overview.md says Actix: wrong) | axum used in code but NOT in Cargo.toml | none | handler return type mismatches trait return | Path<String> | no | `CreateItemHeaders::default()` (no Default derive) | String / String / String | Option | enum OK but `fn item_default_role() -> Role { String::from("user") }` mismatch | struct DROPS parent fields | HashMap | Vec<Node> OK | router() returns `()` stub, main empty, hyphen field, missing axum imports in items/mod.rs | 1991 | 8 (rust_test.go:160 asserts the stub) |
| java | build.gradle, models/**, services/, controllers/, errors/, middleware/ | none (plain class; docs say Spring Boot) | Jackson not declared in build.gradle; LocalDate needs jsr310 | none | comments only (`// 201 Created`) | method params | query model arg, not bound | headers model arg, not bound | BigDecimal / LocalDate / UUID | boxed types | enum with @JsonValue | `extends Base` | Map | List<Node> OK | CreateItemHeaders hyphen; `Role role = "user"`; deleteItem missing return; package hardcoded `maayn.veld.generated` | 2576 | 12 |
| csharp | csproj, Controllers, Models, Services, Errors, Middleware | ASP.NET (not referenced in csproj) | none declared | none | `Task<object>`, no attributes | param name `Id` | Dictionary | `[FromHeader]` model | decimal / DateTime / Guid | `T?` | enum + JsonStringEnumConverter (EnumMember ignored by System.Text.Json: wire "Admin" not "admin") | `: Base` | Dictionary | List<Node> OK | `Idempotency-Key` property; `Role Role = "user"`; controller lacks `using Microsoft.AspNetCore.Mvc`, ControllerBase, verb attributes | 1761 | 12 |
| php | composer.json, routes/api.php, app/{Http,Models,Services,Middleware,Exceptions} | Laravel (no dependency declared) | none declared | `$request->validate(...)` called on `array $request` | comments only | yes | `$request->query()` on array | `$request->headers->all()` on array | string / string / string | `?T = null` before required params | backed enum | `extends Base` (parent ctor not called) | array | array | `array $request` has no methods (fatal); `Role $role = 'user'` fatal; `idempotency-_key` param; wrong `use App\Models\CreateItem` (actual App\Models\Items\CreateItem); `/old`, `/tree` registered after `{id}` so shadowed | 1602 | 10 |
| javascript (backend) | package.json, index.js, routes/*.js, types/*.js, interfaces, middleware | any router | none | none | 201, 204 | yes | req.query | req.headers | string | undefined | JSDoc union | class extends | {} | OK | types/items.js:141 `this.Idempotency-Key =` SyntaxError; `:id` before `/old` shadowing | 1911 | 9 |
| dart | client/api_client.dart, pubspec.yaml | dart:io HttpClient | none | none | >=400 -> VeldApiError(status, body) | interpolated | query string built unencoded | param accepted but unused | String / String / String | `T?` | enum, but fromJson assigns String to enum (runtime TypeError) | flattened | Map assigned from dynamic (runtime error) | List<Node> from dynamic (runtime error) | `String Idempotency-Key` hyphen | 465 | 6 |
| kotlin | client/ApiClient.kt, build.gradle.kts | java.net.http + kotlinx.serialization | kotlinx-serialization (declared) | none | >=400 -> VeldApiError | `${id}` | "omitted for brevity" (client.go:107) | accepted, unused | `BigDecimal` in @Serializable without serializer (compile error) / String / String | `T? = null` | enum + @SerialName | flattened | Map | List<Node> OK | `val Idempotency-Key`; BigDecimal serializer | 420 | 6 |
| swift | client/APIClient.swift, Package.swift | URLSession | none | none | >=400 -> VeldApiError | BUG: `"/api/items/${id}"` (Swift needs `\(id)`; literal "${id}" sent) | accepted, unused | no | Decimal (JSONDecoder cannot decode "1.10" string into Decimal) / String / String | `T?` | String raw enum | flattened | [String:String] | [Node] OK | `var Idempotency-Key`; types are internal (no `public`) so unusable from a consuming module | 395 | 6 |

## 3. Divergence between emitters
- Validation: Node (Zod/--validate) and Python (--validate shape checks, no enum check) validate; Go/Java/C#/PHP/Rust/Kotlin/Swift/Dart do none usable (Java/C# emit validation helper files that are not wired in). CLAUDE.md "full input validation" holds only for Node/Python.
- Status codes: Node/Python/Go/JS set 201/204; Java/C# only add comments; Rust via return tuple. Errors: Go always 500 for any service error, Python/JS honour `e.status`, Java/C# rethrow generic exceptions.
- Error body shape: Node/JS/Python `{error, code?}`; Go `{"error": msg}`; OpenAPI declares `{code:int, message}`; generated Errors/ApiErrorResponse classes (Java/C#/PHP) use a third shape.
- Naming: JSON camelCase on the wire everywhere, but Python model attrs are snake_case (breaks from_dict), Rust snake_case with serde rename, Go PascalCase with `Id` not `ID`.
- extends: Node/Java/C#/PHP/Python use inheritance (PHP/Python constructors break); Go embeds; Dart/Kotlin/Swift flatten; Rust DROPS parent fields; OpenAPI allOf; SQL/GraphQL flatten.
- date/datetime: Go time.Time for both (date breaks), Java LocalDate, C# DateTime, others string.
- Query/headers binding: Node binds; Python passes untyped dict / empty dict; Go passes nil; Java/C#/PHP/Rust declare but do not bind; Kotlin/Swift/Dart clients ignore them.
- Route registration order: Node/JS/PHP register `/:id` before static `/old`, `/tree` (shadowing in order-based routers); Go 1.22 and Flask are safe.
- Header field names: every emitter reuses raw header keys as identifiers; hyphenated names break all 9 non-TS targets plus GraphQL, Prisma and SQL.

## 4. Findings
### F-MULTI-01: Hyphenated header names are emitted as identifiers in every language, producing uncompilable output
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/parser/parser.go:961 (synthetic `<Action>Headers` model); consumers: python models/items.py:218, go internal/models/items.go:184, java CreateItemHeaders.java:8, csharp CreateItemHeaders.cs, php CreateItemHeaders.php:12, rust models/items.rs:211, node-js types/items.js:141, dart/kotlin/swift clients, graphql, prisma, sql
- Trigger/reproducer: `headers: { "Idempotency-Key": string }` (the form shown in CLAUDE.md contract syntax). `veld generate --backend=python --backend-framework=flask --validate` then `python -m compileall out/pyflask` -> SyntaxError. Same via go build, javac, node --check, graphql buildSchema.
- Expected vs actual: valid identifier (`idempotency_key`) with wire name kept in a string literal vs `idempotency-_key` / `Idempotency-key`.
- Impact: any contract using the documented headers feature with a standard header name cannot compile in any non-TS language.
- Root cause: header keys flow into Field.Name unchanged; each emitter's case-converter passes '-' through.
- Recommendation: fix -- sanitise header field names once in the parser (keep original as wire name) and have emitters use the wire name only in string literals.
- Regression check: compile-of-generated-code CI job per language with a contract containing a hyphenated header.
- Effort estimate: 6h-12h

### F-MULTI-02: Emitter unit tests assert substrings; generated output is never compiled, so most non-Node backends ship uncompilable code with a green suite
- Category: emitter-rust
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/emitter/backend/*/*_test.go (e.g. rust/rust_test.go:160 asserts `pub fn router<S>() -> impl std::any::Any`)
- Trigger/reproducer: `go test ./internal/emitter/...` passes; building generated output fails (see run log).
- Expected vs actual: tests should fail when output does not compile; they enshrine broken stubs.
- Impact: regressions and pre-existing breakage are invisible; claims of working multi-stack generation are unsupported.
- Root cause: no golden-file or toolchain compile harness (docs/EXECUTION_PLAN_v1.md already lists "harness" as M2).
- Recommendation: fix -- add per-language compile smoke tests (go build, python compileall+import, javac, tsc, node --check; dockerised cargo/dotnet/php) driven from one rich contract.
- Regression check: CI matrix job fails on current main for rust/java/csharp/php/go/python.
- Effort estimate: 24h-40h

### F-MULTI-03: Default Python backend (no --validate) emits a syntax-error route file and no route registration
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/emitter/backend/python/routes.go:139 (and strategy), registration branch in routes.go
- Trigger/reproducer: `veld generate --backend=python --frontend=none`; compileall -> `input = CreateItemSchema(**body = data  # CreateItemSchema).model_dump()`.
- Expected vs actual: Pydantic-validated route calling the service and registered with Flask vs syntax error, missing `from flask import request`, missing schemas module (not emitted), `# register:` comment instead of add_url_rule, `raise RuntimeError(body)` instead of returning a response.
- Impact: the primary documented Python flow (CLAUDE.md "Pydantic validation") produces nothing runnable; only `--backend-framework=flask --validate` works (after F-MULTI-01/04 are fixed).
- Root cause: a strategy placeholder (ExtractBody) is interpolated as code; the Pydantic schema emitter named in python.go's header no longer exists.
- Recommendation: rewrite -- make Flask the default strategy and always emit the zero-dependency validators; drop Pydantic claims.
- Regression check: compileall + import + run Flask smoke test for default flags.
- Effort estimate: 8h-14h

### F-MULTI-04: Python validators miss the `Decimal` import; any contract with a decimal field returns 500 on every request
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/emitter/backend/python/validate.go:348 (emits isinstance(..., (str, Decimal)) with no import); output _validators.py:8
- Trigger/reproducer: GET /api/items/abc -> 500 {"error":"name 'Decimal' is not defined"}; fixed by adding `from decimal import Decimal`.
- Expected vs actual: 200 vs 500 (swallowed by blanket except).
- Impact: decimal is one of the eight core scalars.
- Root cause: validator header emits only `from typing import Any`.
- Recommendation: fix -- emit the import (or avoid Decimal in validators).
- Regression check: contract with a decimal field exercised in the Flask smoke test.
- Effort estimate: 1h-2h

### F-MULTI-05: Python service contract is internally inconsistent (ABC returns model classes, routes require dicts; headers not extracted; query untyped; malformed JSON -> 500; enum unchecked; exception text leaked)
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: python/routes.go:145-150 and the catch-all except block, python/types.go, validate.go
- Trigger/reproducer: see Part A run log.
- Expected vs actual: typed Item/ItemQuery/headers reach the service; bad JSON -> 400; unknown enum -> 400 vs dict-only returns, `{}` headers, `{'limit':'5'}`, 500 on bad JSON, `role:"bogus"` accepted, `str(e)` returned for all non-API errors.
- Impact: the generated interface cannot be implemented as typed; auth headers (the point of `headers:`) are dropped; information disclosure via str(e) for unexpected exceptions.
- Root cause: validators and models developed independently; header extraction left as a comment.
- Recommendation: refactor -- return dicts or convert via to_dict, bind headers/query, restrict error bodies to known exception types.
- Regression check: Flask smoke test asserting headers present, typed query, 400 for bad JSON/enum, generic 500 body.
- Effort estimate: 10h-18h

### F-MULTI-06: Go routes do not compile (`mux` undefined) and run without input validation or query/header binding
- Category: emitter-go
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: low
- Location: internal/emitter/backend/go/strategy/plain.go:21,52; go/routes.go handler generation
- Trigger/reproducer: `go build ./...` in out/go -> `internal/routes/items.go:14:2: undefined: mux` (after the F-MULTI-01 rename). After patching: `{}` body -> 201; role "bogus" accepted; GetItem receives nil query; CreateItem receives nil headers; date field "2020-01-01" fails to decode.
- Expected vs actual: compile + validate vs compile error and zero validation.
- Impact: Go backend unusable out of the box; once patched, required fields/enums are unenforced and auth headers unreachable.
- Root cause: strategy emits `mux.` while the function parameter is named `r`; no validation emitter for Go.
- Recommendation: fix -- name the param consistently, add generated Validate() methods, bind query/headers, map date to a date-aware type.
- Regression check: go build + vet of generated output; httptest-based smoke test.
- Effort estimate: 12h-20h

### F-MULTI-07: Rust backend cannot compile and has no working router or main
- Category: emitter-rust
- Evidence status: source-supported defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/emitter/backend/rust/routes.go:35,100; Cargo.toml template in rust/main.go; output src/items/mod.rs, src/models/items.rs
- Trigger/reproducer: read generated tree (cargo unavailable: environment limitation; verdict by inspection).
- Expected vs actual: Axum router vs `build_router() -> impl Any { items_router() }` returning `()`; Cargo.toml lacks axum; handlers lack `use axum::{extract::{State,Path},Json,http::StatusCode}; use std::sync::Arc`; handlers return the trait result whose type (`Result<T,Box<dyn Error>>`) differs from the declared handler return; `CreateItemHeaders::default()` without a Default derive; `item_default_role() -> Role { String::from("user") }`; hyphenated identifiers; extended structs drop parent fields (Item has no id/createdAt).
- Impact: not a usable backend; docs disagree (Actix vs Axum).
- Root cause: partially stubbed emitter whose tests lock in the stub.
- Recommendation: freeze -- mark experimental until a cargo CI job exists; fix docs/architecture/overview.md.
- Regression check: cargo check in CI.
- Effort estimate: 30h-50h to make real

### F-MULTI-08: Java, C#, PHP backends emit framework-less classes that do not compile and do not route
- Category: emitter-java
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: java/types.go (enum default; package const at line 26), java/routes.go (deleteItem), csharp/types.go, csharp/routes.go, php/routes.go, php/types.go
- Trigger/reproducer: javac (stub Jackson): `Item.java:22 incompatible types: String cannot be converted to Role`; `ItemsController.java:94 missing return statement`. C#: `Role Role { get; set; } = "user";`, controller without ControllerBase/Mvc using/verb attributes (csproj lacks AspNetCore). PHP: `Role $role = 'user'` fatal, `array $request` then `->validate()`, required params after optional. C#/PHP read only (no dotnet/php).
- Expected vs actual: working Spring/ASP.NET/Laravel integration vs plain POJOs; Java output package is the author's `maayn.veld.generated`, not configurable.
- Impact: three backends advertised as Spring Boot / ASP.NET Core / Laravel are neither compilable nor routable.
- Root cause: default-value emission treats enum default as a string literal; route emitters are framework-neutral but docs/tests claim otherwise.
- Recommendation: freeze -- keep model+interface output, drop controller/route claims until compile CI exists.
- Regression check: javac in CI now; dotnet/php in docker.
- Effort estimate: 20h-35h each to reach a working state

### F-MULTI-09: Dart/Kotlin/Swift clients have compile or runtime defects beyond the header bug
- Category: emitter-swift
- Evidence status: source-supported defect
- Severity (user impact): high
- Security severity: none
- Location: frontend/swift/client.go (`${id}` interpolation), frontend/kotlin/helpers.go:20 + client.go:107, frontend/dart/types.go (fromJson)
- Trigger/reproducer: generated APIClient.swift line 85 `"/api/items/${id}"`; Kotlin `val price: java.math.BigDecimal` inside @Serializable class (no serializer: compile error) and "query params omitted for brevity"; Dart `role: json['role']`, `tags: json['tags']`, `children: json['children']` (runtime cast errors); Swift types internal and Decimal cannot decode a JSON string "1.10". Toolchains unavailable: judged by reading.
- Expected vs actual: working SDKs vs path params literally sent as "${id}" (Swift), query/header params silently dropped (Kotlin/Swift/Dart), decimal broken.
- Impact: the three mobile SDKs are unusable for any non-trivial contract.
- Root cause: string-template emitters never compiled or run.
- Recommendation: freeze -- experimental label; fix after compile CI exists.
- Regression check: kotlinc / swift build / dart analyze jobs.
- Effort estimate: 10h-16h per SDK

### F-MULTI-10: Service SDKs: Python SDK rejects its own model types; TS SDK fails strict tsc; Go SDK lacks query support
- Category: service-sdk
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: python/sdk.go (body/query serialisation), node/sdk.go (shared index scalar imports), go/sdk.go
- Trigger/reproducer: Part D: `create_item(CreateItem(...))` TypeError; `get_item("a", ItemQuery(limit=3))` AttributeError; `tsc --strict` 24 errors in sdk/shared/index.ts (UUID/ISODateTime/ISODate/Decimal undeclared), TS1263, TS2352.
- Expected vs actual: typed clients usable with their own models vs only raw dicts work (Python); compile errors in TS.
- Impact: inter-service SDK, a headline feature, needs manual patching. Positives: env var precedence (arg > VELD_<NAME>_URL > baked) verified for Python, Go, TS; WS actions skipped; error type VeldApiError; deps and deps --validate work.
- Root cause: SDK templates reuse model emitters whose output is not exercised.
- Recommendation: fix -- add to_dict conversion, import scalars in shared module, add query param to Go.
- Regression check: SDK smoke tests against a local fake server (as in ws/t.py).
- Effort estimate: 8h-14h

### F-MULTI-11: Dockerfile and CI generators download a release asset that returns 404, and bundle an unrelated registry Postgres
- Category: gen-dockerfile
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: medium
- Location: internal/generators/dockerfile/dockerfile.go (download line, compose template), internal/generators/cicd/cicd.go
- Trigger/reproducer: HTTP HEAD https://github.com/Adhamzineldin/Veld/releases/latest/download/veld-linux-amd64 -> 404 (assets are .tar.gz). Generated docker-compose.yml starts `postgres:16-alpine` named "registry", POSTGRES_PASSWORD=changeme, 5432:5432 published.
- Expected vs actual: working build vs both the Dockerfile download and the CI "Install Veld" step fail; compose exposes a default-credential DB on the host.
- Impact: generated pipelines fail on first run; unpinned, unverified binary download into the build (supply chain); image runs as root; `CMD node index.js` and `/app/dist` need user-created files; Python/Go projects get Node templates unless marker files are present in --out.
- Root cause: copy-paste from registry compose; release naming not tested.
- Recommendation: fix -- pin version and checksum, correct asset name, remove DB service, add non-root USER.
- Regression check: `docker build` against examples/ in CI.
- Effort estimate: 4h-8h

### F-MULTI-12: SQL/Prisma/GraphQL generators emit invalid output for common contracts
- Category: gen-schema
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/schema/ (BuildSQL, BuildPrisma), internal/graphqlgen/
- Trigger/reproducer: `veld schema` / `veld graphql` on the shared contract; `buildSchema` throws on `Idempotency-Key: String!`.
- Expected vs actual: loadable DDL/Prisma/SDL vs DTO models (ItemQuery, CreateItem, headers) become tables; `DEFAULT user` unquoted (silently current_user in Postgres) and Prisma `@default(user)` invalid; `limit` unquoted; arrays/maps as VARCHAR(255) / String; Prisma models without `@id`; GraphQL Map -> String, ids as String not ID; no relations. graphqlgen and docsgen have 0 tests.
- Impact: schema output is a sketch, not usable DDL; the default-value bug changes semantics.
- Root cause: every Model treated as an entity; no quoting layer.
- Recommendation: refactor -- opt-in table marker, quote defaults and reserved words, skip DTO models.
- Regression check: parse generated SQL with a real parser, Prisma validate, graphql buildSchema in CI.
- Effort estimate: 10h-16h

### F-MULTI-13: OpenAPI output is valid 3.0.3 but incomplete (no servers/security, error shape mismatch, defaults dropped)
- Category: gen-openapi
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Location: internal/generators/openapi/
- Trigger/reproducer: `veld openapi` + `redocly lint` (10 recommended-ruleset errors, 0 structural).
- Expected vs actual: servers from baseUrl, securitySchemes for middleware, enum default, ErrorResponse matching runtime `{error}`, no trailing-slash paths vs all absent or mismatched.
- Impact: documentation fidelity; downstream client generators would mislead.
- Root cause: minimal generator (361 LOC).
- Recommendation: fix -- add servers, security, defaults, align error schema across emitters.
- Regression check: redocly lint with a project ruleset in CI.
- Effort estimate: 4h-8h

### F-MULTI-14: `internal/mock` is unwired dead code; docs list a feature with no command
- Category: gen-mock
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: internal/mock/ (284 LOC); CLAUDE.md architecture table
- Trigger/reproducer: `veld mock` -> unknown command; grep for imports of the package outside itself: none.
- Expected vs actual: runnable mock server vs unreachable package.
- Impact: maintenance burden, misleading docs.
- Root cause: command never wired or removed.
- Recommendation: remove -- or wire `veld mock` and add an integration test.
- Regression check: command present in `veld --help` test.
- Effort estimate: 1h-6h

### F-MULTI-15: Documentation mismatches (Rust framework, Python Pydantic, WS undocumented, hardcoded post-generate hints, HTTP method list)
- Category: emitter-rust
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: docs/architecture/overview.md:37 (Actix vs Axum in code), CLAUDE.md type-mapping table (Pydantic), CLAUDE.md HTTP methods (code also supports HEAD, OPTIONS, WS with required `stream:`/optional `emit:`), cmd/veld/generate.go summary hints (UserSchema, IUsersService, maayn package)
- Trigger/reproducer: compare docs to generated trees.
- Expected vs actual: docs match behaviour vs not.
- Impact: users follow instructions that cannot work.
- Root cause: docs not generated from code.
- Recommendation: fix -- generate hints from the AST and update docs.
- Regression check: doc-lint test comparing the language spec to docs.
- Effort estimate: 2h-4h

## 5. Maintenance-cost estimate
Assumptions: CI matrix per emitter compiling and running the shared rich contract (docker images for cargo, dotnet, php, kotlin; macOS runner for Swift), one maintainer familiar with the emitter, about one breaking upstream change per emitter per quarter, triage included, feature work excluded. One-off remediation to reach green is in the findings above.
| Emitter | Hours/month (steady state) | Notes |
|---|---|---|
| node-ts (ref) | 4-6 | primary; most real usage |
| typescript/react/vue/angular/svelte frontends | 2-3 each (8-12 total) | thin wrappers over the TS SDK |
| python | 4-6 | stdlib validators; Flask/Python versions stable |
| go | 4-6 | stdlib net/http only; chi pinned |
| javascript backend/frontend | 2-3 | shares Node logic |
| java | 6-10 | Spring/Jackson/Gradle churn if framework support is claimed; 3-4 for plain models only |
| csharp | 6-10 | ASP.NET controllers + System.Text.Json quirks |
| php | 5-8 | Laravel major release yearly |
| rust | 8-12 | axum 0.x breaking releases, trait/lifetime complexity |
| dart / kotlin / swift | 3-5 each | needs macOS runner for Swift |
| openapi / docs / graphql / schema | 2-3 each | validate with external linters |
| dockerfile / cicd / envconfig / scaffold | 2-4 total | pin versions, test docker build |
| service SDKs (8 languages) | 6-10 | each sdk.go duplicates model emitters |
Total realistic: roughly 75-110 h/month for the full matrix versus roughly 25-35 h/month for tier 1 (node, python, go, TS/React) with a compile CI.

## Recommended tiers
- Tier 1 (keep, fix first): node-ts, TypeScript/React frontends, Python (Flask + validators), Go (stdlib/chi), OpenAPI, service SDKs for TS/Python/Go.
- Experimental (label; ship models/interfaces only): javascript backend, Vue/Angular/Svelte, Java, C#, PHP, Dart, Kotlin, Swift, SQL/Prisma/GraphQL, dockerfile/cicd.
- Freeze/remove: Rust backend until cargo CI exists, internal/mock (remove or wire), Pydantic claims, Rust/Java/C#/PHP service SDKs (read only, not compile-verified).
