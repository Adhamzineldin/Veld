# 05. Prioritized defect register and documentation discrepancies

The full register is `csv/defect-register.csv` (153 rows, one per finding, all columns required by the brief: ID, category, evidence status, user-impact severity, security severity, exploitability, priority, file:line, trigger/reproducer, expected vs actual, impact, root cause, recommendation, regression check, effort estimate, owner, reviewer, dependencies, lead-reviewer verification, source track). The raw audit-track notes with command logs are in `appendix-track-*.md`. This chapter explains the scales, lists everything at P0 and P1, and states what to preserve.

## 5.1 Scales (kept separate)

| Scale | Values | Meaning |
|---|---|---|
| Evidence status | reproduced defect, source-supported defect, suspected risk, design limitation, documentation mismatch, unverified | chapter 02 §2.5 |
| User-impact severity | critical (data loss, or the headline workflow cannot complete), high (a documented feature does not work), medium (works with a visible defect or workaround), low (cosmetic, docs) | effect on a user of the current release |
| Security severity | critical, high, medium, low, none | CVSS-like judgement: high = exploitable by an unprivileged party with meaningful impact |
| Delivery priority | P0 (fix before any other feature work; blocks the vertical slice, destroys user data, or security high), P1 (fix before the Alpha gate; documented feature broken or security medium), P2 (fix before Beta or demote the feature), P3 (fix opportunistically or close as won't-fix with a docs note) | order of work |

Counts at 6a47f81: 153 findings; by evidence: 88 reproduced, 19 source-supported, 34 documentation mismatches, 8 design limitations, 4 suspected risks; by priority: 25 P0, 43 P1, 36 P2, 49 P3; with security severity high: 6 (F-NODE-05, F-REG-01, F-REG-02, F-REG-03, F-REG-09, F-REG-11). The lead reviewer independently reproduced or confirmed at source 20 of them (column `lead_verified`).

## 5.2 P0 (25)

| ID | Finding | Evidence | Security | Location | Effort | Owner |
|---|---|---|---|---|---|---|
| F-NODE-01 / F-CLI-01 / F-CLI-03 | `generate`, `watch` and `clean` run `os.RemoveAll` on every output directory before parsing; user files inside the output directory are deleted; `clean` with `out: ./src` removed `src/business.ts` | reproduced (lead confirmed at source) | low | `cmd/veld/generate.go:40-52`, `:758-760`; `watch.go:56` | 6 to 10 h (shared) | T2 |
| F-NODE-02 / F-CLI-02 | `--out=../proj` or `--out=../veld` accepted; the project's own `.veld` sources were deleted; validation only inspects the last path segment | reproduced | low | `internal/config/config.go:371-379` | 2 to 5 h | T2 |
| F-NODE-03 | a contract with an error wipes the previous good output; `watch` does so on every save | reproduced | none | `generate.go:758-760` | 4 to 8 h | T2 |
| F-NODE-04 / F-FE-02 | `.veld.lock.json` deleted inside `runClean` before the diff: `--strict`, `--force` and the prompt never fire; a removed action passes `--strict` with exit 0 | reproduced (lead confirmed) | none | `generate.go:51`, `:1019-1035` | 1 to 6 h | T1 |
| F-CLI-05 | `veld init` overwrites existing `README.md` and `veld/*.veld` when no config exists | reproduced | none | `cmd/veld/init.go:20-33, 651-658` | 1 to 2 h | T5 |
| F-LEAD-01 | generation is not deterministic: `types/index.ts` export order varies (1 of 6 runs by the lead reviewer; 4 distinct hashes in 6 runs by the docs track) | reproduced | none | `internal/emitter/tsshared/types.go:442` | 1 to 2 h | T2 |
| F-FE-01 | `import @models/*` and `@alias/**` are lexed as a block comment; the rest of the file disappears and `validate` reports success (lead: AST shows 0 models); the shipped `examples/microservices` hits it | reproduced | none | `internal/lexer/lexer.go:240-257` | 2 to 4 h | T1 |
| F-MULTI-01 | header names such as `Idempotency-Key` become identifiers in every language; output does not compile | reproduced | none | `internal/parser/parser.go:961` and every emitter | 6 to 12 h | T1 with T4 |
| F-MULTI-02 / F-NODE-19 | all 210 emitter tests are substring checks; no test compiles generated code; the suite is green while most outputs fail to compile | reproduced | none | `internal/emitter/**/*_test.go` | 24 to 40 h (harness) | T4 |
| F-MULTI-03 / F-CLI-20 | default Python output has a `SyntaxError` in every route file with a body and registers no routes (lead reproduced on the `init` scaffold) | reproduced | none | `internal/emitter/backend/python/routes.go:139` | 8 to 14 h | T4 |
| F-MULTI-06 / F-CLI-19 | Go routes: `undefined: mux`; no validation; query and headers `nil` (lead reproduced) | reproduced | low | `backend/go/strategy/plain.go:21,52` | 12 to 20 h | T4 |
| F-NODE-05 | static routes shadowed by an earlier `GET /:id`: a middleware-guarded action is unreachable and another action answers without the guard | reproduced (lead confirmed at source) | high | `backend/node/routes.go:75-87` | 3 to 5 h | T2 |
| F-NODE-06 | generated TypeScript fails `tsc --strict` (4 errors; 5 with `--validate`); React, Vue, Svelte inherit them | reproduced | none | `frontend/typescript/client.go`, `backend/node/routes.go` | 1 to 10 h | T2 |
| F-REG-01 | pre-TOTP "partial" JWT is a full session: 2FA bypass | reproduced (lead confirmed at source) | high | `internal/server/handlers/auth.go:105-117`, `auth/middleware.go:70-100` | 1 to 2 h | T3 |
| F-REG-02 | storage key from unvalidated `name`/`version`: write outside the storage root, overwrite and delete another org's tarball | reproduced (lead confirmed) | high | `handlers/packages.go:122,177`, `storage/storage.go:33-36` | 3 to 6 h | T3 |
| F-REG-03 | token scopes unenforced: read-only token mints admin tokens and deletes versions | reproduced (lead confirmed) | high | `handlers/auth.go:393-439`, `packages.go:276-338` | 4 to 8 h | T3 |
| F-REG-09 | download integrity optional and self-referential; MITM with stripped or replaced header pulled successfully; `http://` accepted | reproduced (lead confirmed) | high | `cmd/veld/registry.go:232-236`, `internal/registry/client.go:208-210` | 6 to 16 h | T3 |
| F-REG-10 | `Unpack`: no limits (1.24 MiB to 1 GiB in 2.9 s; 100,000 files), in-place writes, errors dropped; containment holds by accident | reproduced (lead confirmed) | medium | `internal/registry/tarball.go:98-143` | 4 to 8 h | T3 |
| F-REG-11 | stored XSS sinks across the registry UI; no CSP | source-supported (payloads stored and returned) | high | `internal/server/web/index.html:335-587` | 6 to 12 h | T3 |

(Rows that merge two IDs are counted as separate entries in the CSV; the 25 P0 IDs are: F-CLI-01, 02, 03, 05, 19, 20; F-FE-01, 02; F-LEAD-01; F-MULTI-01, 02, 03, 06; F-NODE-01, 02, 03, 04, 05, 06; F-REG-01, 02, 03, 09, 10, 11.)

## 5.3 P1 (43)

Compiler and tooling (T1): F-FE-03 formatter corrupts files and panics; F-FE-05 first-error-only, line-only, sometimes wrong line; F-FE-18 breaking-change classifier misses enums, nested models, middleware, unions, status changes; F-FE-22 `internal/language` is not the source of truth; dead regex parser.

Emitters (T2, T4): F-NODE-08 validation off by default and documented Zod does not exist; F-NODE-09 `--validate` checks only `typeof`; F-NODE-10 500 responses leak exception text; F-NODE-11 path params not URL-encoded; F-NODE-15 Angular output invalid; F-NODE-17 non-atomic writes, no manifest; F-MULTI-04 Python validators miss the `Decimal` import (every decimal request returns 500); F-MULTI-05 Python service contract inconsistent (ABC returns classes, routes pass dicts; headers empty; query untyped; enum unchecked; malformed JSON gives 500); F-MULTI-07 Rust backend is a stub; F-MULTI-08 Java, C#, PHP do not compile or route; F-MULTI-10 service SDKs (Python rejects its own types; TS 24 strict errors; Go no query); F-MULTI-11 Dockerfile and CI generators download a 404 asset and bundle a default-credential Postgres.

Language server and editors (T5): F-LSP-01 imports never resolved (false errors in multi-file projects); F-LSP-11 VS Code extension re-implements the language and never uses the server; F-LSP-16 language metadata not consumed by the compiler.

Registry (T3): F-REG-04 admin can remove the sole owner; F-REG-05 token expiry unenforced; F-REG-06 private visibility inconsistent and unreachable from the API; F-REG-07 concurrent publish corrupts; F-REG-08 versions mutable, "latest" by publish time; F-REG-12 no rate limiting or TOTP replay protection; F-REG-13 no body limits or archive validation; F-REG-14 sessions not revocable, 7-day JWT; F-REG-15 no TLS option, insecure cookie, CORS `*`, no security headers; F-REG-16 default registry chosen from a Go map; F-REG-20 pull can overwrite project files; F-REG-21 no health endpoint, SIGTERM, migrations table, backups or tests.

CLI, distribution, CI (T5): F-CLI-07 flat config overrides nested, contrary to docs; F-CLI-08 `--out` ignored when config sets `backendConfig.out`; F-CLI-16 npm and pip wrappers (no checksum, `latest` download, swallowed failure, Python 3.8/3.9 syntax); F-CLI-18 CI verifies Linux unit tests only; F-CLI-23 untested registry server and strategy packages.

Documentation (D1 with validators): D-01 Zod claim false everywhere; D-02 determinism claim false; D-03 `veld login/push/pull/serve/doctor` documented as top-level (only `veld registry ...` exists; `doctor` unregistered); D-10 MIT claimed, no LICENSE file.

## 5.4 P2 and P3

P2 (36) and P3 (49) are listed in the CSV. Representative P2: F-FE-07 scalar-type names cannot be field names; F-FE-08 no nested generics; F-FE-09 path parameters never checked against fields; F-FE-13 missing imports silently ignored; F-NODE-07 models as classes; F-NODE-12 lossy query serialization; F-NODE-13 no timeout, fetch injection or auth hook; F-NODE-14 module named `index` clobbers the barrel; F-LSP-03 byte-offset positions; F-LSP-08 single-threaded, no `recover`; F-CLI-09 unknown config keys ignored; F-MULTI-12 SQL/Prisma/GraphQL invalid output. Representative P3: the 26 documentation discrepancies D-11 to D-26 (install names, stale developer guide, roadmap marking done work as TODO, wrong team list), F-CLI-21 hygiene (20 MB binary and scratch directories tracked), F-FE-29 AST JSON `null` collections, F-REG-17 plaintext credentials.

## 5.5 Documentation discrepancies (summary of D-01 to D-26)

The documentation audit inventoried 62 claims across README, CLAUDE.md, docs, the scraped hosted docs page (`test.md`), the website and the editor READMEs, and found 26 mismatches. The most consequential: Zod and Pydantic validation do not exist (D-01, D-08); output is not deterministic (D-02); documented top-level registry commands and `veld doctor` do not exist (D-03); the `tools` config block is parsed and ignored (D-04); GitLab CI generation does not exist (D-05); Rust is Axum not Actix and PHP is Laravel not PSR-15 (D-06); the module path, dependency list and SQLite claims in the architecture overview and developer guide are wrong (D-07); the Python output layout differs from the docs (D-08); VS Code commands do not exist (D-09); no LICENSE file (D-10); five different install names (D-11); getting-started snippets use wrong function names (D-12). Verified true: 8 backends and 10 frontends registered, 6 tools, all 8 backends have service SDK code, 5-platform release builds exist.

Historical plans (for the record, not to inherit): roadmap of April 2026 ran to "~16 weeks remaining" with no team named; execution plans v1 to v4 assumed 16 to 17 weeks with 5 members, planned up to 10 backends and 14 frontends (v1, v2), then cancelled all new emitters (v3), then listed 5 backends and 5 frontends as "supported" (v4, which is byte-identical to `docs/EXECUTION_PLAN.md`). The graduation proposal claims a Zod demo and 188 commits to 19 April 2026; git shows 204 commits to 12 September 2026 by four authors, with 171 of them in March and April.

## 5.6 What to preserve, and debt that may remain

Preserve (evidence that it works): the lexer and parser's structure (linear, fuzz-clean, fast); the validator's rule set and error reporting; the loader's alias mechanism; the emitter registry plumbing (`init()` plus blank import is clean); the Node emitter's route and SDK architecture; the service-SDK design (env var precedence verified for TS, Python, Go); the registry's schema and storage abstraction; the language-metadata generator; the CI drift check idea; the website build.

Debt that can safely remain through graduation: string-building emitters (no template engine needed for a small language); the cubic `extends` cycle check (26 s only at 3,000 chained models); plaintext credential storage (industry-standard for CLIs); the embedded SPA as a plain HTML file once escaping is fixed; `internal/setup` (tsconfig auto-configuration) untouched; tier-2 emitters compiling but not round-trip tested.

Remove rather than fix: `internal/mock`, the Dockerfile/CI/envconfig generators, the Rust backend, the Angular/Dart/Kotlin/Swift emitters (ADR-005), the incremental cache, the dead regex parser in `internal/language`, the tracked binary and scratch directories, `test.md`, the four `.docx` plans, the stale `internal/server/web/index.html` duplicate.
