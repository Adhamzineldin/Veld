# 09. Built-in emitters and generated-code quality

Decision record: ADR-005 (tiers), ADR-002 (IR), ADR-007 (output writer). Evidence: Node/TS and multi-emitter audit tracks; chapter 05 F-NODE and F-MULTI items. The lead reviewer reproduced the Python and Go compile failures on the `veld init` scaffold (chapter 02).

## 9.1 Inventory at 6a47f81 (verified)

| Target | Registry name | Framework adapters (`strategy/`) | Runtime deps in output | Compiles? | Runs round trip? | Validation | Notes |
|---|---|---|---|---|---|---|---|
| Node backend | `node-ts` (alias `node`), `node-js` | plain, express | none by default; `express` adapter adds `import('express')` type references | `tsc --strict` fails with 4 errors (5 with `--validate`) (F-NODE-06); runtime works under `tsx` | GET/POST 201/PUT/PATCH/DELETE 204/headers work; static routes shadowed by `/:id` (F-NODE-05); 500 leaks error text (F-NODE-10) | off by default; `--validate` emits `_validators.ts` with `typeof` checks only (F-NODE-08, 09); no Zod anywhere | models are classes with getters/setters and branded scalars (F-NODE-07) |
| TypeScript client | `typescript` | n/a | none | fails strict `tsc` (shared errors) | path params not encoded (F-NODE-11); arrays serialized as `x,y` (F-NODE-12); no timeout/abort/auth hook (F-NODE-13) | n/a | `VeldApiError` with status, body, code |
| React / Vue / Svelte | `react`, `vue`, `svelte` | n/a | `@tanstack/react-query`, `vue`, `svelte/store` as expected | compile once F-NODE-06 is fixed | wrap the TS SDK | n/a | client directory duplicated per framework |
| Angular | `angular` | n/a | `@angular/core`, HttpClient | invalid TypeScript in `models/index.ts`; headers dropped (F-NODE-15) | no | n/a | re-implements the SDK instead of wrapping it |
| JavaScript frontend / backend | `javascript`, `node-js` | plain | none | frontend still emits `.ts` files; backend fails `node --check` at `types/items.js:141` | no | no | |
| Types only | `typesonly` | n/a | none | shares the TS type emitter | n/a | n/a | |
| Python backend | `python` | plain, flask | `flask` only with the adapter | `SyntaxError` in routes (F-MULTI-03); with `--validate` a second syntax error from hyphenated header names (F-MULTI-01); `Decimal` import missing (F-MULTI-04) | after two manual patches: 201, 400, 204 correct; headers arrive empty; query untyped; unknown enum accepted; malformed JSON gives 500 (F-MULTI-05) | `--validate` emits `_validators.py`; no Pydantic anywhere despite docs | ABC returns model classes while routes require dicts |
| Go backend | `go` | plain (net/http), chi | `chi` only with the adapter; ships `go.mod` | `undefined: mux` (F-MULTI-06); header-name syntax error | after patches: 201, 204, 400 on malformed JSON; `{}` accepted (no required-field validation); query and headers passed as `nil`; `date` and `datetime` both map to `time.Time`, so `"2020-01-01"` fails to decode | none | |
| Rust backend | `rust` | axum (docs say Actix) | `axum` not even declared in Cargo.toml | cannot compile by reading (F-MULTI-07) | no | none | stub: `router()` returns `()`, empty `main` |
| Java backend | `java` | spring (per docs) | none emitted | `javac` 8 errors (headers) then 2 more (`Role role = "user"`, missing return) (F-MULTI-08) | no | none | plain classes, hardcoded package `maayn.veld.generated` |
| C# backend | `csharp` | aspnet (per docs) | none emitted | cannot compile by reading (`Role Role = "user"`; no `ControllerBase`) | no | none | |
| PHP backend | `php` | laravel (docs say PSR-15) | none | cannot compile by reading (`->validate()` on an array) | no | none | `{id}` registered before `/old` and `/tree` (shadowing) |
| Dart / Kotlin / Swift clients | `dart`, `kotlin`, `swift` | n/a | none | defects by reading: Swift path `"${id}"` literal, Kotlin `BigDecimal` in `@Serializable` without serializer and query params "omitted for brevity", Dart `fromJson` assigns strings to enums (F-MULTI-09) | no | n/a | not compiled (no toolchains in this environment) |
| Service SDKs (`consumes`) | all 8 backends | n/a | none | Go SDK builds and vets; Python SDK compiles but rejects its own model types; TS SDK 24 strict errors (F-MULTI-10); Rust/Java/C#/PHP SDKs read only | env-var precedence verified for TS, Python, Go | n/a | WebSocket actions (`method: WS`, `stream:`, `emit:`) exist in the parser and are skipped by SDKs; undocumented |
| OpenAPI | `openapi` | n/a | n/a | valid 3.0.3 per `redocly lint` (style warnings only) | n/a | n/a | no servers/security; enum defaults dropped; error shape differs from runtime (F-MULTI-13) |
| GraphQL SDL | `graphql` | n/a | n/a | `buildSchema` throws on hyphenated header field (F-MULTI-12) | n/a | n/a | |
| SQL / Prisma | `schema` | n/a | n/a | every DTO becomes a table; `DEFAULT user` unquoted (Postgres reads `current_user`); `limit` unquoted; Prisma models have no `@id` (F-MULTI-12) | n/a | n/a | |
| Docs | `docs` | n/a | n/a | untested package (1,864 lines, 0 tests) | n/a | n/a | |
| Dockerfile / CI / envconfig | `dockerfile`, `cicd`, `envconfig` | n/a | n/a | Dockerfile and CI download `releases/latest/download/veld-linux-amd64`, which returns 404; compose bundles a registry Postgres with password `changeme` on a published port (F-MULTI-11) | n/a | n/a | |
| Mock | (none) | n/a | n/a | `internal/mock` is not imported by any command (F-MULTI-14) | n/a | n/a | dead code |

Tests: 523 test functions in the repository; 210 under `internal/emitter`; every emitter test is a substring assertion; no test compiles or runs generated output (F-MULTI-02, F-NODE-19). This is the single systemic cause of the table above.

## 9.2 Sustainable supported subset

See ADR-005 for the tier table and rules. Rationale in numbers:

| Scope | Monthly maintenance (audit estimate) | Share of team capacity (about 150 h/month committed technical, chapter 15) |
|---|---|---|
| Full matrix (8 backends, 10 frontends, 6 tools) | 75 to 110 h | 50 to 73 percent |
| Tier 1 only (3 backends, 2 frontends, 3 service SDKs, OpenAPI) | 25 to 35 h | 17 to 23 percent |
| Tier 1 plus tier-2 compile-only | 40 to 55 h | 27 to 37 percent |

Assumptions: one owner per emitter, one upstream breaking change per quarter (TypeScript, Flask, chi, React Query releases), CI toolchains installed by setup actions. The tier-1 share is the maximum a team that also has to deliver a plugin system, a registry and a report can sustain.

## 9.3 Boundaries and the meaning of "zero runtime dependencies"

| Layer | What Veld generates | Dependency rule | Status today |
|---|---|---|---|
| Types | interfaces, enums, scalar aliases | none, ever | true for TS; Python/Go types have the header-name and date bugs |
| Transport (clients) | request building, path and query encoding, error type | standard library only (`fetch`, `urllib`, `net/http`) | true, with encoding bugs |
| Validation | generated validator functions per model | generated code, no library; default ON after M1 | off by default; shallow |
| Routes (language level) | handler functions that bind path, query, headers, body and call the service interface | none beyond the language's HTTP primitives | Express-shaped assumptions in Node; broken in Python and Go |
| Framework adapters | `strategy/` packages that register handlers on a specific router | may import only the framework the user already installed; never a Veld runtime | express, flask, chi present; others documented but not implemented |
| Optional integrations | React Query hooks, Vue composables, Svelte stores | import the named library; experimental tier | wrap the TS SDK |

Precise claim to publish: "Generated types, clients and validators import nothing outside the language's standard library. Route adapters import only the framework you chose."

Unsupported semantics must fail loudly: the capability declaration from the plugin manifest (chapter 10) is also used for built-ins. Each built-in declares what it supports (headers, maps, recursive models, WebSocket, middleware, status codes); the compiler reports an error, not a silent omission, when a contract uses something the chosen emitter declares unsupported. Example: `WS` actions with the Python emitter produce `E_UNSUPPORTED_FEATURE` naming the action and the emitter.

## 9.4 Required properties for every tier-1 emitter

1. Capability declaration (above).
2. Stable type mapping table (chapter 08 section 8.4) applied through `internal/ir` type references, not per-emitter string matching. `date` and `datetime` are distinct (Go: `civil.Date`-style string wrapper or `string` for date, `time.Time` for datetime); `decimal` is a string on the wire in every language; `int` is 64-bit where the language allows and documented as 53-bit safe for JavaScript.
3. Naming: identifiers derived through one function per language (`lang.Ident(name, kind)`) that escapes reserved words and non-identifier characters (`Idempotency-Key` to `idempotencyKey` plus the original wire name kept as metadata). Collisions with language globals (`Date`, `Error`, `Map`) are validator errors with a suggestion.
4. Imports computed from the IR, never hardcoded.
5. Deterministic formatting: files assembled from sorted collections; a determinism test runs each emitter twice on every fixture.
6. Route ordering: static segments before parametric ones per method, and a validator error for overlapping routes (fixes F-NODE-05 and the PHP shadowing at once because ordering lives in the IR, not in emitters).
7. Error responses: one documented shape `{ "error": { "code", "message", "details"? } }`; `500` never includes exception text.
8. Runtime version policy: Node 20+, Python 3.10+, Go 1.26+ (matches the compiler's own Go policy); TypeScript 5.x; declared in the README matrix and tested in CI.
9. Validation default ON with `--no-validate` to opt out; validators check enums, element types, integers, `uuid`/`date`/`datetime` formats, optional versus `null`, and coerce query strings.

## 9.5 Testing beyond snapshots

| Level | What | Tier | Mechanism |
|---|---|---|---|
| Conformance fixtures | 30 to 40 `.veld` inputs with expected IR JSON and expected diagnostics | all | `go test ./internal/spec` |
| Golden output | per emitter, per fixture, the generated files | 1 and 2 | golden files with `-update`; reviewed in PRs |
| Compile | `tsc --strict`, `python -m compileall` plus `mypy --strict` on types, `go vet`, `javac`, `dotnet build`, `php -l` | 1 fast, 2 nightly | one Go test per emitter that shells out, skipped when the toolchain is absent (`testing.Short`), required in CI |
| Semantic assertions | parse the generated file and assert structure (for example, route registration count equals operation count; validator rejects an invalid enum) | 1 | language-native test files emitted alongside fixtures |
| Round trip | start the generated server with a reference service implementation, call it with the generated client, assert status codes, bodies, error shape, encoding of `a/b?x` path params, array query params, headers | 1 | `test/e2e/<lang>/` harness, run in CI on three OSes |
| Cross-language interoperability | Node server called by the Python and Go clients and the reverse, using one fixture | 1 | same harness, matrix job |
| Error cases | invalid body, unknown enum, missing required, oversized body, malformed JSON | 1 | round trip |
| Reproducibility | generate twice, byte-compare; generate on Windows and Linux, byte-compare | all registered | CI job |

Estimated cost to build the harness: 40 to 60 h (T4 with T2), then 2 to 4 h per new fixture. This is the second item on the critical path after the output writer, because every later emitter fix is verified through it.

## 9.6 Placement of auxiliary generators

| Generator | Decision | Reason |
|---|---|---|
| OpenAPI | core, tier 1 | interoperability escape hatch (chapter 03); already valid 3.0.3; add servers/security/defaults, align the error schema; 3.1 output as a small follow-up |
| GraphQL SDL | optional, tier 2 | fix via the identifier function; no further investment |
| SQL / Prisma | optional, tier 2 after quoting fixes (2 to 4 h) and a `@entity`-style opt-in so DTOs are not tables | tables from DTOs is wrong by construction; keep only as an opt-in sketch |
| Docs | optional, tier 2 | add tests; useful for the report and website |
| Mock server | remove | dead code; TypeSpec, Prism and others already do this |
| Dockerfile, CI, envconfig | remove from the release binary | they encode release URLs that do not exist and bundle insecure defaults; a project template in `examples/` serves the same purpose |

## 9.7 Sequence (see chapter 15 for dates)

1. M1: output writer (ADR-007); fix F-MULTI-01 header identifiers at the parser/IR level; fix Python syntax error and `Decimal` import; fix Go `mux`; fix Node route ordering, 500 leak, path encoding, strict-mode errors; compile tests for the three tier-1 backends in CI. Target: the vertical slice compiles and round-trips in CI by 2026-11-29.
2. M2: IR migration of tier-1 emitters; validation default ON and deepened; tier-3 code removed; tier-2 nightly compile job.
3. M4: React/Vue/Svelte on the fixed TS SDK; Java/C#/PHP compile-only or demoted; OpenAPI completeness.
4. Beta gate: tier-1 round trip green on three OSes for two sprints.

## 9.8 Candidate for extraction to an external plugin (chapter 10 acceptance test)

The `docs` generator (Markdown API documentation) is the best candidate: self-contained, no toolchain, useful, and currently untested. Extracting it to `plugins/veld-emitter-docs` proves the protocol on a real emitter without touching the tier-1 path. Second candidate, post-graduation: Dart client, as a community-maintained plugin.
