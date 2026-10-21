# 04. Current architecture and verified capability inventory (6a47f81)

Every row states how it was verified. "Tests" is Go statement coverage from `go test -cover` plus the count of test functions where measured; coverage says nothing about generated-code correctness.

## 4.1 Pipeline and dependencies

The pipeline is `.veld -> lexer -> parser -> AST -> loader (imports) -> validator -> emitters -> files`, with the AST struct passed by value; there is no IR and no serialization between stages except `veld ast` (chapter 07 §7.2). External Go dependencies: `github.com/spf13/cobra v1.8.0`, `github.com/lib/pq v1.10.9`, `golang.org/x/crypto v0.24.0` (plus `pflag`, `mousetrap` indirect). `go.mod` declares `go 1.22` (out of support since Go 1.27 shipped on 2026-08-19); the code builds on 1.26 and uses only 1.22-era APIs (grep for `slices`, `maps`, `cmp`, `iter`, `min`/`max`, range-over-int found nothing; `http.ServeMux` method patterns from 1.22 are used).

Coupling observations: `cmd/veld` holds 4,729 lines across ten files (no longer monolithic: `main.go` is 109 lines since the 2026-08-27 split); emitters import `internal/emitter` helpers and `internal/ast` only (the isolation rule holds); the registry server is compiled into the CLI; the LSP re-reads keyword lists rather than using `internal/language`.

## 4.2 Capability inventory

| Area | Capability | State | Verification | Tests |
|---|---|---|---|---|
| Language | models, enums, modules, actions, `extends`, `Map`, arrays, optional, `@default`, `@deprecated`, headers block, middleware, status suffix, constants, unions (undocumented), `WS` actions with `stream`/`emit` (undocumented), HEAD/OPTIONS (undocumented) | implemented | parser read; 72 adversarial cases | parser 53.7%, lexer 60.9%, validator 53.7% |
| Imports | alias, relative, wildcard | alias and relative work; wildcard `@alias/*` is lexed as a comment (F-FE-01) | reproduced | loader 30.2% |
| Diagnostics | validator reports all errors with `file:line` and suggestions; lexer/parser first error only, line only | partial | reproduced | errors pkg 78.4% but unused by the pipeline |
| Robustness | no panic or hang in 1.67M fuzz executions; 5 MB file and 100k-field model validate under 1 s | strong | fuzzing | |
| Formatter | `veld fmt` | broken: drops comments and quotes, panics on `model }` | reproduced | 63.6% |
| Lint | 8 rules | works; error-level rules unreachable (validator rejects first); false positive on nested models | reproduced | 84.0% |
| Breaking-change detection | `diff`, `.veld.lock.json`, `--strict`, `--force`, prompt | implemented but dead in `generate` (lock deleted first); classifier shallow | reproduced | diff 55.2% |
| Incremental cache | `.veld-cache.json` | wiped on every run; keyed on `.veld` content only | source-supported | 92.1% |
| Config | nested and flat forms, aliases, workspace, hooks, `tools` block | flat overrides nested (docs say the reverse); `tools` ignored; unknown keys ignored; `--out` ignored when config sets backend out | reproduced | 74.5% |
| Generation safety | "never touches user files", "never writes outside out" | false: `RemoveAll` of output dirs before parsing; `--out` at the source dir deletes sources | reproduced | cmd/veld 0% (tests run a built binary) |
| Determinism | documented guarantee | false (export order varies) | reproduced | |
| Node backend | types, interfaces, routes, validators (opt-in), middleware interface, express/plain strategies | runs under tsx; fails strict tsc; route shadowing; 500 leaks; no Zod | reproduced | node 19.6% |
| TS client | fetch SDK, `VeldApiError`, path params | works except encoding, query arrays, strict tsc | reproduced | typescript 59.3% |
| React / Vue / Svelte | hooks, composables, stores wrapping the SDK | compile once SDK fixed | reproduced | 83.4 / 79.6 / 79.8% |
| Angular | HttpClient services | invalid output | reproduced | 72.4% |
| JavaScript targets | plain JS | frontend emits `.ts`; backend fails `node --check` | reproduced | 62.0 / 29.5% |
| Python backend | models, ABC interfaces, routes, validators, flask/plain | syntax error in routes; routes never registered; `Decimal` import missing | reproduced | 28.5% |
| Go backend | models, interfaces, routes, server, go.mod, plain/chi | `undefined: mux`; no validation; date/datetime conflated | reproduced | 46.8% |
| Rust / Java / C# / PHP backends | structs/classes, routes | none compile; Rust is a stub | reproduced (Java via javac) and source-supported | 48.5 / 58.2 / 59.1 / 47.9% |
| Dart / Kotlin / Swift clients | HTTP clients | defects by reading; not compiled | source-supported | 63.6 / 66.3 / 64.9% |
| Service SDKs (`consumes`) | 8 languages; `veld deps`, `--validate` | TS, Python, Go verified for env-var precedence; TS 24 strict errors; Python rejects its own types; others unverified | reproduced | sdkhelpers 100% |
| OpenAPI | 3.0.3 export | valid per redocly (style warnings); incomplete | reproduced | 65.6% |
| GraphQL SDL | export | invalid on header fields | reproduced | 0% |
| SQL / Prisma | export | invalid defaults, DTOs as tables | reproduced | schema 70.0%, database 79.9% |
| Docs generator | Markdown docs | untested | not exercised | 0% (1,864 lines) |
| Mock server | `internal/mock` | dead code, no command | source | 61.2% |
| Dockerfile / CI / envconfig | generators | download 404 asset; insecure compose | reproduced | 76.6 / 56.4 / 66.7% |
| LSP | stdio server: sync, diagnostics, completion, hover, definition | single-file only; byte offsets; first error; no MethodNotFound | reproduced | 33.6% |
| VS Code | extension | independent TS implementation; commands not registered; builds; no tests | reproduced | none |
| JetBrains | PSI plugin, Kotlin 4,090 lines | not built in review; hand-written duplication | source | none |
| Language metadata | `internal/language`, `cmd/generate-language` | generator works, no drift; compiler does not consume it | reproduced | 0% |
| Registry client | login, push, pull, versions, tokens | works in the happy path; integrity optional; registry chosen from map order | reproduced | 0% |
| Registry server | auth, orgs, packages, tokens, TOTP, email, SPA | works in the happy path; 2FA bypass, path traversal, scopes unenforced, XSS sinks, no limits, no health endpoint, no tests | reproduced | 0% |
| Distribution | npm, pip, homebrew, go, nuget, chocolatey, apt, composer | go install works; npm/pip need work; others placeholders | reproduced / source | none |
| CI | ubuntu-only build, vet, race tests, drift, cross-compile; release to 7 channels without signing | works for what it covers; no generated-code compile; no OS matrix | source | |
| Website | Vite 7, React 19; marketing, docs, LiveDemo with its own TS parser | builds (no lockfile, lint errors); not in CI | reproduced | |
| Examples | 10 | 9 validate; `microservices` fails (wildcard import); only `node-typescript` output compiles | reproduced | |
| Hygiene | tracked 20 MB binary, scratch dirs, `.idea`, `.docx`, stale `out/`, `test.md` | 24.8 MB tracked, 82% is the binary | reproduced | |
| Governance | LICENSE, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, CODEOWNERS, templates, CHANGELOG | absent (changelog for v0.1 only) | reproduced | |

## 4.3 Operational maturity

- No health endpoint, no migrations versioning, no backup tooling, no TLS option, no rate limiting in the registry.
- No signed releases, SBOM or provenance; wrappers do not verify checksums.
- No OS matrix in CI; Windows path handling has visible defects (config path re-basing, `sh -c` hooks).
- No telemetry (good); no crash reporting.

## 4.4 Public interfaces today

- CLI (cobra): `init`, `validate`, `ast`, `generate`, `watch`, `clean`, `openapi`, `lint`, `fmt`, `docs`, `graphql`, `schema`, `diff`, `setup`, `deps`, `ci`, `lsp`, `registry {login, logout, push, pull, versions, token, serve, ...}`; `doctor` defined but not registered; several hidden commands.
- Go interfaces: `emitter.Emitter`, `BackendEmitter`, `FrontendEmitter`, `ToolEmitter` (`internal/emitter/emitter.go:43-86`); no external protocol.
- Registry HTTP API `/api/v1` (auth, orgs, packages, tokens).
- LSP over stdio.
- Config schema `editors/veld-config.schema.json` (synchronized copies verified byte-identical).

## 4.5 Summary judgement

The repository contains a working compiler frontend of good quality, one backend and client pair (Node and TypeScript) that is close to working, a wide but unverified emitter matrix, a registry whose happy path works but whose security does not hold, and tooling claims that outrun the code. The defects cluster in four systemic causes: output written destructively without a manifest; no test compiles generated code; three independent implementations of the language; and a registry written without tests or a threat model. Chapters 06 to 16 address exactly those four causes.
