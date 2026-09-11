# Veld — Execution Plan (Version 2)

**From audited prototype to production-ready v1.0**

**Duration:** 13 weeks core delivery + 4 weeks buffer (17 weeks total)
**Team:** 5 members
**Baseline:** the full-repository audit recorded in `docs/GRADUATION_PROPOSAL.md` §12

> **Version note.** This is **Version 2** of the execution plan. It supersedes Version 1, in which emitters were divided by *role* (one member owned all backend emitters, another owned all frontend emitters). In this version emitters are divided by *type-system family*, so all three general members own both backends and frontends. The rationale for the change is set out in §2. Both versions are retained for comparison.

---

## Contents

1. [Plan at a glance](#1-plan-at-a-glance)
2. [How the emitters are divided](#2-how-the-emitters-are-divided)
3. [Team structure & ownership](#3-team-structure--ownership)
4. [Master timeline](#4-master-timeline)
5. [Phase definitions & exit criteria](#5-phase-definitions--exit-criteria)
6. [Member 1 — TypeScript/web family + compiler core](#6-member-1--typescriptweb-emitter-family--compiler-core)
7. [Member 2 — Dynamic & systems family + tool generators](#7-member-2--dynamic--systems-family--tool-generators)
8. [Member 3 — JVM/.NET/mobile family + registry](#8-member-3--jvmnetmobile-family--registry)
9. [Member 4 — Rust](#9-member-4--rust-dedicated)
10. [Member 5 — Documentation & diagrams](#10-member-5--documentation--diagrams)
11. [New emitter scope](#11-new-emitter-scope)
12. [Critical path & dependencies](#12-critical-path--dependencies)
13. [Definition of done](#13-definition-of-done)
14. [Process & ceremonies](#14-process--ceremonies)
15. [Risk register](#15-risk-register)
16. [Buffer month policy](#16-buffer-month-policy)

---

## 1. Plan at a glance

The project already has a sound architecture. What it lacks is *correctness of output*, *security in the registry*, and *coverage breadth*. This plan does not restructure anything — it closes defects, enforces parity mechanically, adds missing language targets, and hardens the system for release.

The single most important decision: **Phase 1 builds a compile-verification harness before fixing any output bug.** Four of the eleven critical defects are cases where the generator ran successfully and produced source that does not compile. Fixing them individually is worth three days; making that class impossible is worth the quarter.

| Phase | Weeks | Theme | Gate |
|---|---|---|---|
| **P0** | 1 | Foundation & safety net | CI compiles generated output for every target |
| **P1** | 2–4 | Critical defects (C1–C11) | Zero non-compiling output; registry secured |
| **P2** | 5–8 | Parity, deduplication, high-severity | Conformance matrix green; duplicates collapsed |
| **P3** | 9–12 | New emitters & production hardening | New targets shipped; release-ready infrastructure |
| **P4** | 13 | Integration & release candidate | v1.0-rc tagged, demo rehearsed |
| **Buffer** | 14–17 | Overflow, hardening, v1.0 | v1.0 released |

---

## 2. How the emitters are divided

All 18 existing emitters (8 backend + 10 frontend) plus the 6 tool generators are divided across M1, M2, and M3. Rust is carved out entirely for M4.

**The division is by type-system family, not by backend-versus-frontend.** This matters for three reasons:

1. **Shared code stays inside one owner.** The six TypeScript-family emitters all consume `tsshared`/`tshelpers`. Splitting them across members would turn every shared-layer change into a cross-member negotiation. Angular's refactor onto the shared path becomes internal work for one person instead of a coordination problem.
2. **Language expertise compounds.** A member fixing `@serverSet` in Java applies near-identical reasoning in C# and Kotlin. Fixing it in Python teaches nothing about Java.
3. **A backend and its matching client belong together.** The Python client emitter is a promotion of Python inter-service SDK code; the same person should own both.

| Family | Member | Backends | Frontends / clients | Tool generators |
|---|---|---|---|---|
| **TypeScript / web** | **M1** | node-ts, node-js | typescript, javascript, react, vue, svelte, angular, types-only, **+SolidJS** | docsgen, graphqlgen, schema |
| **Dynamic & systems** | **M2** | python, go, php, **+Ruby** | **+Python client, +Go client** | cicd, database, dockerfile, env, openapi, scaffold-tests, openapigen |
| **JVM / .NET / mobile** | **M3** | java, csharp, **+Kotlin backend** | kotlin, swift, dart | — |
| **Rust** | **M4** | rust | **+Rust client** | — |

**Totals per member (existing + new):** M1 = 11 emitters · M2 = 9 emitters + 7 generators · M3 = 6 emitters · M4 = 2 emitters.

### Why the counts are uneven

Emitter *count* is a poor proxy for effort. The load is balanced by weighting three factors:

- **Critical defects carried.** M2 holds four (C2 Go, C3 Python, C9 PHP, C11 scaffold); M3 holds five (C5–C8 registry, C10 Swift); M1 holds one (C4 formatter); M4 holds one, but it is the worst in the system.
- **Code sharing.** M1's eleven emitters share a common TypeScript codegen layer — React, Vue, and Svelte are ~170 lines each and simply wrap the TypeScript emitter. Eleven TS-family emitters cost far less than eleven independent ones.
- **Non-emitter load.** M1 also owns the compiler core and CLI; M3 also owns the registry and production infrastructure. M2 carries the largest emitter share precisely because its only other duties are the compile harness and the parity matrix.

---

## 3. Team structure & ownership

| Member | Emitter family | Other subsystems |
|---|---|---|
| **M1** | TypeScript / web (11) | Compiler core (`lexer`, `parser`, `ast`, `loader`, `validator`), CLI, developer tooling (`config`, `cache`, `diff`, `lint`, `format`, `lsp`, `mock`, `errors`) |
| **M2** | Dynamic & systems (9) + tool generators (7) | Compile-verification harness, parity conformance matrix |
| **M3** | JVM / .NET / mobile (6) | Registry (`internal/server`, `internal/registry`), CI/CD, packaging, release engineering |
| **M4** | **Rust — exclusively** (2) | Rust compile-verification arm |
| **M5** | **Documentation & diagrams — exclusively** | Runs the full 17 weeks with no delivery deadline |

M4 is deliberately isolated: Rust carries the single worst defect (output does not compile at all), has the weakest test coverage, and is the only backend where *every* audited feature is missing or broken. It needs one person's sustained attention, not a slice of three people's.

M5 has no delivery deadline by design. Diagrams and documentation are the first deliverable compressed when engineering slips, and the most heavily weighted in academic assessment. Decoupling them protects both.

---

## 4. Master timeline

```
        │ W1 │ W2 │ W3 │ W4 │ W5 │ W6 │ W7 │ W8 │ W9 │W10 │W11 │W12 │W13 ║W14 │W15 │W16 │W17 │
        │ P0 │      P1      │        P2         │        P3         │ P4 ║      BUFFER       │
════════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╬════╪════╪════╪════╡
 M1     │████│ C4 │ CLI dedup │ WS in 4 wrappers │ Angular │ mock │ LSP │████│▓▓▓▓║           │
 TS/web │setup│fmt │ ─strict   │ react/vue/svelte │ refactor│Solid │     │ RC ║  errors, init │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M2     │████│ C2 │ C3 │ C9 │ parity matrix │ go q/h │ Ruby backend │ clients │████│▓▓▓▓║     │
 dyn/sys│harness│ go │ py │php │ + C11 scaffold│ ­@serverSet │  (new)   │ py+go  │ RC ║        │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M3     │████│ C5–C8 registry │ C10 │ kotlin/swift q │ C# exc │ Kotlin backend │ prod │████│▓▓║
 jvm/mob│audit│ security sprint│swift│  params + dart │ + java │    (new)       │ infra│ RC ║  │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M4     │████│ C1 axum compile │ plain │ query/headers │serverSet│ WS │ SDK │Actix│client│████║
 RUST   │cargo CI│ make it build │ verify│   parity      │ +valid │body│ fix │     │      │ RC ║
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M5     │████████████████████████████████████████████████████████████████████████████████████│
 docs   │ architecture diagrams │ ER + sequence │ user docs │ contributor │ report │ deck    │
```

**Legend:** `████` active delivery · `▓▓▓▓` stabilisation · `║` buffer boundary

---

## 5. Phase definitions & exit criteria

### Phase 0 — Foundation & safety net (Week 1)

Nothing is fixed this week. The week exists to make every subsequent fix verifiable.

| Task | Owner |
|---|---|
| Install and pin the Go toolchain; run and record a baseline `go test ./...` | M1 |
| **Build the compile-verification harness** — generate into a temp dir, then invoke the target compiler | M2 |
| Wire the harness into CI as a required check | M2 |
| Rust arm of the harness (`cargo build` on generated output) | M4 |
| Baseline security review of `internal/server`; write the threat model | M3 |
| Branch protection, PR template, conventional commits, CODEOWNERS map | M1 |
| Diagram inventory + architecture diagrams v1 (architecture is already stable) | M5 |

**Exit gate:** CI fails on any generated output that does not compile, for every target. Baseline test results recorded. Every member can build and test locally.

> **Why this comes first.** C2 (Go's default strategy emits an undefined identifier) went undetected because every Go test forces `chi`, so the default path was never exercised. C1, C10, and C11 are the same shape. Harness first means Phase 1 fixes are *proven*, not asserted.

### Phase 1 — Critical defects (Weeks 2–4)

| ID | Defect | Owner |
|---|---|---|
| C1 | Rust + Axum output does not compile | M4 |
| C2 | Go `plain` strategy emits undefined `mux` | M2 |
| C3 | Python default output imports a schema module nobody writes | M2 |
| C4 | `veld fmt --write` silently deletes top-level `prefix:` lines | M1 |
| C5 | Registry 5-minute 2FA bypass via partial JWT | M3 |
| C6 | Any authenticated user can enumerate all private packages | M3 |
| C7 | Path traversal → arbitrary file write on publish | M3 |
| C8 | API token expiry stored but never enforced | M3 |
| C9 | PHP/Laravel `HEAD` actions crash the app at boot | M2 |
| C10 | Swift SDK sends the literal string `${id}` | M3 |
| C11 | C# test scaffolding emits invalid C# | M2 |

**Exit gate:** All eleven closed with a regression test each. `go test ./...` green. Compile harness green for all eight backends. External penetration pass over the registry finds no auth bypass. **No known path produces non-compiling output.**

### Phase 2 — Parity, deduplication & high-severity (Weeks 5–8)

Phase 1 fixed instances; Phase 2 fixes the causes.

**The parity conformance matrix (M2, weeks 5–6)** is the centrepiece: a table-driven suite asserting every backend handles every AST feature — query params, headers, `@serverSet`, WebSocket actions, typed errors, inheritance, maps, unions, defaults. Every cell is a test. A new AST feature added later without cross-backend handling becomes a build failure rather than a silent gap found by audit.

**Deduplication** collapses the four diverged duplicates: two workspace-generation paths (M1), two OpenAPI generators (M2 — owns both), two route-conflict detectors (M1 — owns both), and ~10 scalar-mapping switches (M2 leads, M1 co-owns three instances).

**Exit gate:** Conformance matrix passes for all backends × all AST features, or every failing cell is an explicitly documented and accepted limitation. Zero duplicated subsystems remain. `--strict` works in workspace mode.

### Phase 3 — New emitters & production hardening (Weeks 9–12)

Breadth and release-readiness in parallel. See §11.

**Exit gate:** All new emitters pass the same conformance matrix as existing ones — no new target ships below the parity bar. Registry has versioned migrations, rate limiting, and test coverage. Distribution packages install on a clean machine.

### Phase 4 — Integration & release candidate (Week 13)

Full-system integration across all 10 examples, end-to-end demo rehearsal, performance profiling on the six-service workspace, `v1.0.0-rc1` tagged, release notes written.

**Exit gate:** `v1.0.0-rc1` tagged. Demo rehearsed end to end twice with no manual intervention. All documentation merged.

---

## 6. Member 1 — TypeScript/web emitter family + compiler core

### Emitters owned (11)

| Type | Targets |
|---|---|
| Backends | `node-ts`, `node-js` |
| Frontends | `typescript`, `javascript`, `react`, `vue`, `svelte`, `angular`, `types-only` |
| New | **SolidJS** |
| Export generators | `docsgen`, `graphqlgen`, `schema` |

**Also owns:** compiler core (`lexer`, `parser`, `ast`, `loader`, `validator`), `cmd/veld/`, and developer tooling (`config`, `cache`, `diff`, `lint`, `format`, `lsp`, `mock`, `errors`).

**Family rationale.** These eleven emitters share one codegen layer (`tsshared`/`tshelpers`). `node-ts` is the reference backend and `typescript` the reference frontend; React, Vue, and Svelte literally call the TypeScript emitter and wrap it. Angular is the outlier that reimplements everything — bringing it onto the shared path is internal work here, not a cross-member negotiation. This is also the healthiest family in the audit, which is why it pairs with the heaviest non-emitter load.

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | Toolchain setup and baseline test run. Branch protection, PR template, CODEOWNERS, conventional-commit CI check. Repository hygiene: remove `registry.exe`, `test-*/`, churn artefacts. |
| **2** | **C4** — the `prefix:` data-loss bug. Either emit a real `TPrefix` token or delete the dead branch and handle `prefix` as the contextual `TIdent` it actually is (prefer the latter — smaller, matches the parser). Add formatter round-trip property tests: *format(format(x)) == format(x)* and *parse(format(x)) == parse(x)*. |
| **3–4** | **CLI deduplication.** Collapse the two divergent workspace-generation implementations into one function used by both `generate` and `watch`. **Wire the `--strict` breaking-change gate into workspace mode** — CI safety currently does nothing for microservices projects, the exact case where it matters most. Config precedence bug; diff coverage for enums and constants. |
| **5–6** | **WebSocket support in React, Vue, Svelte, Angular.** None branch on `act.Method == "WS"`; each generates a call to a method that does not exist (the client exports `subscribeTo<Name>`). Angular additionally calls `this.http.ws()`, which is not part of Angular's `HttpClient`. |
| **7** | **Angular refactor** — bring it onto the shared TypeScript path like React/Vue/Svelte: shared `VeldApiError`, shared base-URL configuration, no reimplemented model emission. |
| **8** | `node-js` and `javascript` frontend: `@serverSet` enforcement, WebSocket handler bodies (currently stubs), `@deprecated` emission. Deduplicate route-conflict detection (validator vs. linter — both M1). Lint false-positives on `@relation` / map-value / union-member references. |
| **9** | **New: SolidJS emitter** — same wrapper pattern as React/Vue/Svelte over the shared TypeScript emitter. Cheapest new target in the plan. |
| **10** | **Wire in `veld mock`** — the mock server is built and tested; only command registration is missing. `docsgen` / `graphqlgen` / `schema` maintenance; scalar-mapping dedup for the three instances M1 owns (coordinate with M2, who leads). |
| **11–12** | **LSP import resolution** — call the loader so cross-file types stop showing as undefined in editors. Spec-compliant JSON-RPC errors. Make `internal/cache` goroutine-safe. Refactor the ~800-line `runInit` wizard out of `main.go`; switch its config writing to `encoding/json`. Replace the hardcoded `sh -c` post-generate hook with a cross-platform implementation. |
| **13** | Integration, RC stabilisation, release-notes input. |

**Buffer-eligible:** `internal/errors` adoption across the pipeline; `runInit` refactor if weeks 11–12 run tight.

**Deliverables:** Formatter that never loses source. One workspace implementation. `--strict` working everywhere. All web frontends handling WebSocket actions. Angular on the shared path. SolidJS shipped. `veld mock` shipped. LSP resolving imports.

---

## 7. Member 2 — Dynamic & systems family + tool generators

### Emitters owned (9 + 7 generators)

| Type | Targets |
|---|---|
| Backends | `python`, `go`, `php` |
| New backend | **Ruby** (Rails + Sinatra) |
| New clients | **Python client**, **Go client** (promoted from inter-service SDKs) |
| Tool generators | `cicd`, `database`, `dockerfile`, `env`, `openapi`, `scaffold-tests`, `openapigen` |

**Also owns:** the compile-verification harness and the parity conformance matrix.

**Family rationale.** Python, PHP, and Ruby are dynamically typed with structurally similar validation strategies; Go pairs with them here because its client promotion belongs beside its backend. This family carries four of the eleven criticals, which is why M2's only other duties are the two cross-cutting test systems. Owning both OpenAPI generators makes their deduplication internal work.

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | **Build the compile-verification harness** — the highest-leverage deliverable in the plan. Generate into a temp dir, then invoke `tsc --noEmit`, `go build`, `python -m py_compile`, `javac`, `dotnet build`, `php -l`. Wire into CI as a required check. Agree the interface with M4, who implements the `cargo build` arm. |
| **2** | **C2** — Go `plain` strategy: reconcile the router identifier (`mux` vs `r`). This is the documented zero-dependency default path, so it must be verified by the harness, not by inspection. |
| **3** | **C3** — Python: write the missing `schemas/schemas.py` emitter, or change the default path to not import it. Decide deliberately whether `--validate` should become the default; document the choice. |
| **4** | **C9** — PHP `HEAD` actions: map to `Route::get` (Laravel handles HEAD implicitly) or reject at validation. **C11** — C# scaffold brace escaping (`fmt.Sprintf` has no `{{` semantics). |
| **5–6** | **Build the parity conformance matrix.** Table-driven: every backend × every AST feature. The deliverable that prevents the next audit finding the same class of drift. Publish the schema at W5 for M1/M3/M4 review before it becomes a merge gate. |
| **7** | Go query and header parameters — `buildCallArgs` currently passes the literal `"nil"`, discarding request data on every request. Go SDK base-URL error on unresolved (currently proceeds silently with an empty base). |
| **8** | `@serverSet` enforcement in python and php. Wire in PHP's dormant Laravel FormRequest validation. Emit Python `requirements.txt` (implemented, never called). WebSocket handler bodies for go and php. Fix or remove the Gin stub. |
| **9–10** | **New backend: Ruby** (Rails + Sinatra strategies). Full parity-matrix compliance required before merge. |
| **11** | **OpenAPI deduplication** — collapse `internal/openapigen` and `internal/generators/openapi`, which have no import relationship and produce different specs for the same contract. Lead the scalar-mapping dedup across the ~10 hand-written switches. |
| **12** | **New clients: Python and Go** — promoted from existing inter-service SDK code. Register as frontend emitters. Remove the Docker Compose hardcoded Postgres password. |
| **13** | Integration, RC stabilisation. |

**Deliverables:** Compile harness in CI. Parity matrix as a merge gate. Python, Go, PHP backends compiling and parity-compliant. Ruby backend shipped. Python and Go client emitters shipped. One OpenAPI generator, not two.

---

## 8. Member 3 — JVM/.NET/mobile family + registry

### Emitters owned (6)

| Type | Targets |
|---|---|
| Backends | `java`, `csharp` |
| New backend | **Kotlin** (Ktor + Spring Boot) |
| Frontends | `kotlin`, `swift`, `dart` |

**Also owns:** the registry (`internal/server`, `internal/registry`), CI/CD, packaging, release engineering.

**Family rationale.** Java, C#, and Kotlin are nominally typed with class inheritance and annotation-driven frameworks — one member fixing `@serverSet` or typed-exception handling in Java applies near-identical reasoning in the other two. The mobile clients (Kotlin, Swift, Dart) pair naturally: Kotlin client and Kotlin backend share type mapping, and Swift and Dart share the same missing-query-param defect. The smaller emitter count reflects the registry security work, which is the highest-risk workstream in the project.

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | Registry threat model and security baseline. Audit every handler for authentication, authorisation, and input validation. Set up a staging registry instance. |
| **2–3** | **Registry security sprint — highest-risk work in the project.** **C5:** add a `partial` claim to `JWTClaims`; reject partial tokens in `resolveAuth`. **C6:** filter `ListPackages` by actual org membership. **C7:** validate `PkgName`/`Version` against a strict character class and reject `..` in `storage.path()` (the tarball extractor already guards this — reuse it). **C8:** compare `expires_at` in `GetTokenByHash`. |
| **4** | **C10** — Swift path-parameter interpolation (`${id}` → `\(id)`). Add a Swift-specific interpolation helper rather than reusing the JS-shaped shared one. Extend Swift and Kotlin tests to assert on the **request URL**, not just the function signature. |
| **5–6** | Kotlin and Swift query parameters — currently accepted in the signature and silently discarded. Dart client parity review and base-URL handling. |
| **7** | **C# typed-exception handling** — the generated controller only has `catch (Exception e)`, collapsing every `NotFoundException`/`BadRequestException` into a flat 500 and discarding the whole typed-error system. Fix C# `HEAD`/`OPTIONS`, currently routed as `GET` behind a `// TODO`. |
| **8** | `@serverSet` enforcement in java and csharp — security-relevant, since without it clients can set their own `role` or `id`. `extends` in the C# and PHP SDK paths (currently dropped). C# and PHP SDK return types — make them genuinely typed, as their own header comments claim. Route C#/PHP through their existing unit-tested language adapters instead of duplicating logic inline. |
| **9** | Java header binding correctness; WebSocket handler bodies for csharp. Wire in the dormant `validation.go` generators for java and csharp. |
| **10–11** | **New backend: Kotlin** (Ktor + Spring Boot strategies). Kotlin exists only as a client today; this makes it first-class. Java infrastructure and the existing Kotlin client type mapping are both reusable. |
| **12** | **Production infrastructure.** Versioned schema migrations (currently one hand-written `CREATE TABLE IF NOT EXISTS` with no rollback). Rate limiting on login/TOTP/registration. Test coverage for `internal/server` — currently zero on every auth-critical path. Fix Homebrew/Chocolatey placeholder checksums; implement the pip download mechanism; unify version bumping across the five distribution packages. |
| **13** | Integration, RC stabilisation, release engineering. |

**Continuous:** Low-severity registry items — login timing side-channel, unescaped HTML in emails, unrecovered email goroutine, stale duplicate SPA file. Enable private-package publishing (schema supports it; the handler hardcodes `public`). Wire up the VS Code Generate/Validate commands, declared but never registered in `activate()`.

**Deliverables:** Registry with no known auth bypass and real test coverage. Java, C#, Kotlin, Swift, Dart all parity-compliant. Kotlin backend shipped. Working distribution packages.

---

## 9. Member 4 — Rust (dedicated)

### Emitters owned (2)

| Type | Targets |
|---|---|
| Backend | `rust` (Axum + plain, **+Actix-Web**) |
| New client | **Rust client** |

Rust is the weakest target in the system and the only one where *every* audited feature is missing or broken. It gets one person for the full duration.

### Starting position

| Feature | State |
|---|---|
| Axum output | **Does not compile** — `build_router()` calls handler identifiers that are never imported |
| Query parameters | Entirely absent — zero references in `routes.go` |
| Header parameters | Degraded — emits a default value |
| `@serverSet` | Ignored — every field client-settable |
| `@deprecated` | Not emitted |
| WebSocket handlers | `TODO` stub that does not compile |
| `--validate` | `validation.go` fully implemented, never called |
| `extends` in SDK | Silently dropped |
| Test coverage | One file |

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | Rust arm of the compile harness: generate → `cargo build` → fail CI on error. Establish a fixture crate. Read and map the existing Rust emitter end to end. |
| **2–3** | **C1 — make the output compile.** Fix handler imports in `build_router()`. Replace the `impl std::any::Any` placeholder return types with real Axum `Router` types. Verify against a non-trivial multi-module contract. |
| **4** | Rust `plain` strategy verification — confirm the non-Axum path compiles too. Go's equivalent default path was silently broken; assume nothing. |
| **5–6** | **Query and header parameters.** Axum extractors (`Query<T>`, `TypedHeader`) plus the plain-strategy equivalent. Full parity-matrix compliance for both. |
| **7** | `@serverSet` enforcement (input-type field omission). `@deprecated` via `#[deprecated]`. **Wire in `validation.go`** — fully implemented and never called; make `--validate` produce real Rust validation. |
| **8–9** | **WebSocket handler bodies** — replace the non-compiling `TODO` stub with a working `axum::extract::ws` implementation covering both `stream` and `emit`. |
| **10** | **Rust SDK correctness** — `extends` flattening (currently dropped), typed deserialisation, base-URL resolution error handling consistent with Node and Python. |
| **11** | **New: Actix-Web strategy.** Second production framework alongside Axum, exercising the strategy abstraction on the hardest language. |
| **12** | **New: standalone Rust client emitter** — register Rust as a frontend target, promoting the inter-service SDK to a first-class client. Raise Rust test coverage to match Java's. |
| **13** | Integration, RC stabilisation. |

**Buffer-eligible:** Rocket as a third strategy — explicitly a stretch goal, not a Week 1–13 commitment.

**Deliverables:** Rust output that compiles, in both strategies. Full parity-matrix compliance. Working WebSocket support. Actix-Web strategy. Rust as a client target. Test coverage comparable to the best-tested backend.

---

## 10. Member 5 — Documentation & diagrams

**Owns:** every diagram and every written document. **No delivery deadline** — the full 17 weeks are available.

Sequencing is ordered by *when the underlying subject stabilises*, not by delivery pressure. Architecture is already stable and can be documented immediately; feature-level documentation must wait for Phases 2–3 to settle or it will be rewritten.

### Workstream A — Architecture diagrams (weeks 1–5, subject already stable)

- Compilation pipeline (lexer → parser → AST → loader → validator → emitters)
- Emitter plugin registry — three interfaces, three registries, `init()` self-registration
- Strategy pattern — the language × framework matrix
- **Emitter ownership map** — the family division in §2, as a diagram
- Two-pass workspace resolution with cross-service alias injection
- Frontend AST merging vs. backend per-service SDK generation
- Package/module dependency graph of `internal/`
- Registry deployment topology

### Workstream B — Data & behaviour diagrams (weeks 4–9)

- ER diagram of the registry schema (users, orgs, members, packages, versions, tokens)
- Sequence: `veld generate` end to end
- Sequence: workspace generation with `consumes` resolution
- Sequence: registry `push` / `pull` including digest verification
- Sequence: 2FA login flow — **the corrected one, post-C5**
- State: breaking-change gate decision flow (`--strict` / `--force` / interactive)
- AST class diagram
- Request lifecycle through generated route handlers

### Workstream C — User documentation (weeks 6–12, after features settle)

- Getting started (rewritten against the fixed CLI)
- `.veld` language reference — complete, every annotation and construct
- CLI reference — every command and flag
- Configuration reference — nested and flat formats
- Per-target guides for all backends and frontends, **including the new ones**
- Microservices / workspace guide
- Registry operator guide — self-hosting, migrations, security configuration

### Workstream D — Contributor documentation (weeks 8–13)

- **"Adding a new emitter"** — the highest-value contributor document, written by observing M2, M3, and M4 actually doing it three times
- Architecture decision records for the major choices
- `CONTRIBUTING.md`, testing guide, parity-matrix explainer
- Release process and version-bump procedure

### Workstream E — Academic deliverables (weeks 10–17)

- Final report / thesis chapters
- Presentation deck
- Demo script and recorded video
- Poster
- Updated proposal reflecting delivered state

### Working method

Attend all reviews; interview each owner at phase boundaries. Diagrams live in-tree as source (Mermaid or draw.io XML), never as opaque images, so they are diffable and reviewable. Every diagram is reviewed by the owner of the subsystem it depicts — a diagram nobody verified is worse than no diagram.

> **Standing risk.** Documentation written during Phases 1–2 will partially invalidate as Phase 3 lands new emitters. Mitigation: Workstreams C and D deliberately trail engineering by ~3 weeks, and the buffer month absorbs late churn.

---

## 11. New emitter scope

New targets ship **only** if they pass the same parity conformance matrix as existing ones. Adding a target below the parity bar recreates exactly the drift this plan exists to eliminate.

| Target | Owner | Weeks | Type | Rationale |
|---|---|---|---|---|
| **SolidJS** | M1 | 9 | Frontend | Cheapest target available — same wrapper pattern as React/Vue/Svelte over the shared TypeScript emitter. |
| **Ruby** (Rails, Sinatra) | M2 | 9–10 | Backend | Largest ecosystem with no Veld support. Rails is a dominant API backend entirely unserved today. |
| **Python client** | M2 | 12 | Frontend | Promotion of existing inter-service SDK code. |
| **Go client** | M2 | 12 | Frontend | Promotion of existing inter-service SDK code. |
| **Kotlin backend** (Ktor, Spring) | M3 | 10–11 | Backend | Kotlin exists only as a client. Serves Android teams wanting one language across the stack; reuses Java infrastructure. |
| **Actix-Web strategy** | M4 | 11 | Framework | Second production Rust framework; validates the strategy abstraction on the hardest language. |
| **Rust client** | M4 | 12 | Frontend | Promotion of existing inter-service SDK code. |

### The promotion insight

Veld already generates working Python, Go, Java, C#, PHP, and Rust HTTP clients — but only for *inter-service* use inside a workspace. Registering these as frontend emitters exposes existing, tested code to a much larger set of users at a fraction of the cost of writing new emitters. This is the highest return-per-hour work in Phase 3. Java, C#, and PHP clients follow the identical pattern in the buffer month if capacity allows.

### Net result

**8 backends / 10 frontends → 10 backends / 14 frontends**, plus one new Rust framework strategy.

---

## 12. Critical path & dependencies

```
  M2: compile harness (W1)
        │
        ├────► M2 backend fixes (W2–4) ──► parity matrix (W5–6) ──► Ruby (W9–10) ──► clients (W12)
        │                                        │
        ├────► M1 C4 + CLI dedup (W2–4) ─────────┤
        │                                        │
        ├────► M3 registry security (W2–3) ──────┤──► Kotlin backend (W10–11) ──► prod infra (W12)
        │                                        │
        └────► M4 cargo arm (W1) ─► C1 (W2–3) ───┘
                                                 │
                          all emitters must pass ┴──► P4 integration (W13) ──► v1.0-rc
```

### Hard dependencies

| Dependency | Consequence if late |
|---|---|
| **Compile harness (M2, W1) → everything** | Every output fix becomes unverifiable. **The single highest-priority deliverable in the plan.** If it slips, the schedule slips. |
| **Parity matrix (M2, W5–6) → all new emitters** | New targets have no acceptance bar and ship with the same drift the plan exists to remove. |
| **C1 Rust compiles (M4, W2–3) → all Rust parity work** | Nothing downstream in Rust is testable until the output compiles. |
| **CLI dedup (M1, W3–4) → `--strict` in workspace** | CI safety remains silently absent for microservices projects. |
| **Registry security (M3, W2–3) → staging deployment** | No instance can be exposed, blocking realistic integration testing. |

### Cross-member coordination points

- **W1:** M2 and M4 agree the compile-harness interface before either implements their arm.
- **W3–4:** M1's workspace refactor touches code M2 and M3 depend on — land as a single reviewed PR, not incremental commits.
- **W5:** M2 publishes the parity-matrix schema; M1, M3, and M4 review before it becomes a merge gate.
- **W8:** `@serverSet` lands in four backends across three members (M2 python/php, M3 java/csharp, M4 rust) — agree the semantics once, in writing, before any of them implements it.
- **W11:** M2's scalar-mapping dedup touches three instances M1 owns — joint PR.
- **W9–12:** M2, M3, M4 each build a new emitter; all three brief M5 for the "Adding a new emitter" guide while doing it.
- **W13:** full-team integration; no solo work.

---

## 13. Definition of done

### Per pull request
Compiles · tests pass · **compile harness green for affected targets** · parity matrix green · reviewed by one non-author · conventional commit message · documentation impact flagged to M5.

### Per new emitter
Registered via `init()` · implements the full role interface · passes the complete parity matrix · generated output compiles in CI · unit tests at parity with the best-covered existing emitter · one worked example · target guide drafted by M5.

### Per phase
All phase items closed or explicitly deferred with written rationale · exit gate met · demo of new capability at phase review · M5 briefed.

### v1.0 release
- Zero known non-compiling output paths across all targets
- Zero known auth bypasses in the registry; `internal/server` test coverage no longer zero
- Parity matrix green across all 10 backends and 14 frontends
- All 10 examples generate and build end to end
- Distribution packages install on clean Linux, macOS, and Windows machines
- CI green including compile verification on all five platforms
- Complete documentation and diagram set merged
- No dead packages: `internal/mock` and `internal/errors` wired in or deleted

---

## 14. Process & ceremonies

| Cadence | Event | Duration | Participants |
|---|---|---|---|
| Daily | Async written standup | — | All 5 |
| Weekly | Sync — blockers, cross-member dependencies | 45 min | All 5 |
| Phase boundary | Review + demo + M5 briefing | 2 h | All 5 |
| Weekly | Documentation review | 30 min | M5 + one rotating owner |
| Buffer entry (W13) | Go/no-go on scope for weeks 14–17 | 1 h | All 5 |

**Branching:** trunk-based off `master`, short-lived feature branches, PR required, one non-author approval, CI green including the compile harness.

**Cross-family review rule:** every emitter PR is reviewed by an owner from a *different* family. This is the main defence against the drift that caused the audit findings — a Java reviewer looking at a Python `@serverSet` fix will ask whether the semantics match theirs.

**Escalation:** any member blocked more than one working day raises it in standup. Any Phase 1 critical not closed by end of Week 4 is escalated at the phase review and consumes buffer.

---

## 15. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | **Compile harness (W1) proves harder than expected** — six toolchains in CI | Medium | **Critical** — blocks everything | Start Day 1. Ship incrementally: TypeScript and Go first (fastest), others follow. A partial harness still gates the backends fixed in W2. |
| R2 | **Harness reveals defects beyond the 11 known** | **High** | High | Expected, not feared — this is the harness working. Weeks 14–17 exist substantially for this. Triage new findings against the same critical/high bar. |
| R3 | **Rust proves deeper than one person's capacity** | Medium | High | M4 dedicated for all 17 weeks. Actix and Rocket are explicit stretch goals, cut first. If C1 is not closed by W4, escalate and move Actix to buffer. |
| R4 | **Registry security uncovers systemic design problems** | Medium | High | W1 threat model exists to surface this early. Fallback: ship v1.0 with the registry documented as beta and not recommended for public exposure — the compiler is independently valuable. |
| R5 | **New emitters cannot meet the parity bar in time** | Medium | Medium | Parity is non-negotiable; *scope* is. Cut targets rather than lowering the bar. Cut order: Ruby, then Kotlin backend. SolidJS and the promoted clients are cheap and survive any cut. |
| R6 | **`@serverSet` semantics diverge across the three members implementing it** | **High** | Medium | W8 coordination point: agree semantics in writing before implementation, and make it a parity-matrix cell so divergence fails CI. |
| R7 | **M1 overloaded** — 11 emitters plus compiler core and CLI | Medium | Medium | The TS family is the healthiest in the audit (one critical, heavy code sharing). `internal/errors` adoption and the `runInit` refactor are explicitly buffer-eligible. If W5–7 slips, move SolidJS to buffer. |
| R8 | **M5's documentation invalidated by late feature changes** | **High** | Low | Workstreams C and D deliberately trail engineering by ~3 weeks. Architecture docs front-loaded, feature docs back-loaded. |
| R9 | **M1's workspace deduplication regresses microservice generation** | Medium | High | The six-service example is the regression test. Land as one reviewed PR with M2 and M3 as reviewers. |
| R10 | **Single-owner families create bus-factor risk** | Medium | Medium | The cross-family review rule forces every member through other families' code. M5's contributor documentation is itself a mitigation. |
| R11 | **Buffer consumed early, leaving no margin** | Medium | Medium | W13 go/no-go gate. If more than two weeks of buffer are consumed before W13, cut new emitters — never cut security or compile verification. |

---

## 16. Buffer month policy

Weeks 14–17 are **not** planned work. They are margin, allocated in strict priority order.

| Priority | Claim on buffer |
|---|---|
| **1** | Overflow from Phases 1–2 — critical and high-severity defects. Non-negotiable. |
| **2** | Defects newly surfaced by the compile harness and parity matrix (see R2 — expect these). |
| **3** | Release hardening: RC feedback, cross-platform install verification, performance. |
| **4** | M5 documentation completion and academic deliverables. |
| **5** | Stretch targets — Rocket strategy (M4); Java, C#, PHP client promotions (M3, M2); Elixir/Phoenix backend (M2). |

**Rule:** priority 5 work begins only when priorities 1–4 are fully closed. New emitters are the *first* thing cut and the *last* thing added.

**Rule:** the buffer is never used to lower the parity bar or skip compile verification. A target that cannot meet the bar is deferred to post-v1.0, not shipped below it.

---

## Appendix — Emitter ownership quick reference

| Emitter | Type | Owner | Family |
|---|---|---|---|
| `node-ts` | Backend | M1 | TypeScript / web |
| `node-js` | Backend | M1 | TypeScript / web |
| `typescript` | Frontend | M1 | TypeScript / web |
| `javascript` | Frontend | M1 | TypeScript / web |
| `react` | Frontend | M1 | TypeScript / web |
| `vue` | Frontend | M1 | TypeScript / web |
| `svelte` | Frontend | M1 | TypeScript / web |
| `angular` | Frontend | M1 | TypeScript / web |
| `types-only` | Frontend | M1 | TypeScript / web |
| **SolidJS** *(new)* | Frontend | M1 | TypeScript / web |
| `docsgen`, `graphqlgen`, `schema` | Export | M1 | — |
| `python` | Backend | M2 | Dynamic & systems |
| `go` | Backend | M2 | Dynamic & systems |
| `php` | Backend | M2 | Dynamic & systems |
| **Ruby** *(new)* | Backend | M2 | Dynamic & systems |
| **Python client** *(new)* | Frontend | M2 | Dynamic & systems |
| **Go client** *(new)* | Frontend | M2 | Dynamic & systems |
| `cicd`, `database`, `dockerfile`, `env`, `openapi`, `scaffold-tests`, `openapigen` | Tool | M2 | — |
| `java` | Backend | M3 | JVM / .NET / mobile |
| `csharp` | Backend | M3 | JVM / .NET / mobile |
| **Kotlin backend** *(new)* | Backend | M3 | JVM / .NET / mobile |
| `kotlin` | Frontend | M3 | JVM / .NET / mobile |
| `swift` | Frontend | M3 | JVM / .NET / mobile |
| `dart` | Frontend | M3 | JVM / .NET / mobile |
| `rust` | Backend | **M4** | Rust |
| **Rust client** *(new)* | Frontend | **M4** | Rust |

### Non-emitter ownership

| Area | Owner |
|---|---|
| Lexer, parser, AST, loader, validator | M1 |
| Config, cache, diff, lint, format, LSP, mock, errors | M1 |
| `cmd/veld/` — all commands | M1 |
| Compile-verification harness | M2 (+ M4 for the Rust arm) |
| Parity conformance matrix | M2 |
| `internal/server/`, `internal/registry/` | M3 |
| CI/CD, `packages/`, release engineering | M3 |
| **All diagrams and documentation** | **M5** |
