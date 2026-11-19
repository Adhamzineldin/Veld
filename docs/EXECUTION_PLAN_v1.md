# Veld — Execution Plan (Version 1)

**From audited prototype to production-ready v1.0**

**Duration:** 13 weeks core delivery + 4 weeks buffer (17 weeks total)
**Team:** 5 members
**Baseline:** the full-repository audit recorded in `docs/GRADUATION_PROPOSAL.md` §12

> **Version note.** This is **Version 1** of the execution plan, in which emitters are divided by *role*: one member owns all backend emitters, another owns all frontend emitters. Version 2 supersedes it with a division by *type-system family*, so that all three general members own both backends and frontends. Both versions are retained for comparison.

---

## Contents

1. [Plan at a glance](#1-plan-at-a-glance)
2. [Team structure & ownership](#2-team-structure--ownership)
3. [Master timeline](#3-master-timeline)
4. [Phase definitions & exit criteria](#4-phase-definitions--exit-criteria)
5. [Member 1 — Compiler core, CLI & tooling](#5-member-1--compiler-core-cli--developer-tooling)
6. [Member 2 — Backend emitters](#6-member-2--backend-emitters)
7. [Member 3 — Frontends, registry & production](#7-member-3--frontend-emitters-registry--production)
8. [Member 4 — Rust](#8-member-4--rust-dedicated)
9. [Member 5 — Documentation & diagrams](#9-member-5--documentation--diagrams)
10. [New emitter scope](#10-new-emitter-scope)
11. [Critical path & dependencies](#11-critical-path--dependencies)
12. [Definition of done](#12-definition-of-done)
13. [Process & ceremonies](#13-process--ceremonies)
14. [Risk register](#14-risk-register)
15. [Buffer month policy](#15-buffer-month-policy)

---

## 1. Plan at a glance

The project already has a sound architecture. What it lacks is *correctness of output*, *security in the registry*, and *coverage breadth*. This plan therefore does not restructure anything — it closes defects, enforces parity mechanically, adds the missing language targets, and hardens the system for release.

The single most important decision in this plan: **Phase 1 builds a compile-verification harness before fixing any output bug.** Four of the eleven critical defects are cases where the generator ran successfully and produced source code that does not compile. Fixing them individually is worth three days; making that entire class impossible is worth the whole quarter.

| Phase | Weeks | Theme | Gate |
|---|---|---|---|
| **P0** | 1 | Foundation & safety net | CI compiles generated output for every target |
| **P1** | 2–4 | Critical defects (C1–C11) | Zero non-compiling output; registry secured |
| **P2** | 5–8 | Parity, deduplication, high-severity | Conformance matrix green; duplicates collapsed |
| **P3** | 9–12 | New emitters & production hardening | New targets shipped; release-ready infrastructure |
| **P4** | 13 | Integration & release candidate | v1.0-rc tagged, demo rehearsed |
| **Buffer** | 14–17 | Overflow, hardening, v1.0 | v1.0 released |

---

## 2. Team structure & ownership

Ownership is by **subsystem**, not by task type. Each member owns their subsystem end to end — its bugs, its tests, its documentation inputs, and its release readiness. This avoids the coordination overhead of splitting a single file across people.

| Member | Subsystem | Scope |
|---|---|---|
| **M1** | Compiler core, CLI, developer tooling | `internal/{lexer,parser,ast,loader,validator,config,format,diff,lint,cache,lsp,mock,errors}`, `cmd/veld/` |
| **M2** | Backend emitters (7 non-Rust) | `internal/emitter/backend/{node,javascript,python,go,java,csharp,php}`, `internal/generators/`, plus **2 new backend targets** |
| **M3** | Frontend emitters, registry, production | `internal/emitter/frontend/*`, `internal/server/`, `internal/registry/`, `packages/`, CI/CD, plus **3 new frontend targets** |
| **M4** | **Rust — exclusively** | `internal/emitter/backend/rust/` (all of it), Rust client emitter, Rust compile verification |
| **M5** | **Documentation & diagrams — exclusively** | All diagrams, all written documentation, final report, presentation. Runs the full 17 weeks with no delivery deadline pressure. |

### Load balance rationale

M1, M2, and M3 carry comparable load. M2 has the most files but the most repetitive work (the same fix applied across seven backends). M3 has the widest variety — three unrelated domains — and carries the security-critical work, so its Phase 1 is deliberately front-loaded and its new-emitter scope is the cheapest of the three (SolidJS and promoted SDKs reuse existing code).

M4 is deliberately isolated: Rust carries the single worst defect (C1 — output does not compile at all), has the weakest test coverage, and is the only backend where *every* audited feature is missing or broken. It needs one person's sustained attention, not a slice of three people's.

M5 has no delivery deadline by design. Diagrams and documentation are the most commonly compressed deliverable when engineering slips, and they are also the most heavily weighted in academic assessment. Decoupling them from the engineering schedule protects both.

---

## 3. Master timeline

```
        │ W1 │ W2 │ W3 │ W4 │ W5 │ W6 │ W7 │ W8 │ W9 │W10 │W11 │W12 │W13 ║W14 │W15 │W16 │W17 │
        │ P0 │      P1      │        P2         │        P3         │ P4 ║      BUFFER       │
════════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╪════╬════╪════╪════╪════╡
 M1     │████│▓▓▓▓ fmt/lexer│████ CLI dedup ████│███ tooling ███│████│▓▓▓▓║ overflow │ v1.0  │
 core   │setup│  C4  │ ─strict gate │ dedup×4 │ mock+errors │ LSP │ RC ║                   │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M2     │████│████ C2/C3/C9/C11 ████│██ parity matrix ██│██ Ruby+Kotlin ██│████│▓▓▓▓║      │
 backend│harness│ compile fixes │ @serverSet ×4 │ WS bodies │ new backends │ RC ║           │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M3     │████│████ C5–C8 registry ██│██ C10 + frontends ██│██ Solid+clients ██│████│▓▓▓▓║   │
 fe/reg │audit│ security sprint │ swift/kotlin/WS │ new frontends │ prod infra │ RC ║       │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M4     │████│████ C1 axum ████│███ query/serverSet/WS ███│██ Actix + client ██│████│▓▓▓▓║  │
 RUST   │cargo CI│ make it compile │ feature parity │ new strategies │ RC ║               │
────────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────┼────╫────┼────┼────┼────┤
 M5     │████████████████████████████████████████████████████████████████████████████████████│
 docs   │ architecture diagrams │ subsystem docs │ feature docs │ report │ deck │ video    │
```

**Legend:** `████` active delivery · `▓▓▓▓` stabilisation · `║` buffer boundary

---

## 4. Phase definitions & exit criteria

### Phase 0 — Foundation & safety net (Week 1)

Nothing is fixed this week. The week exists to make every subsequent fix verifiable.

| Task | Owner |
|---|---|
| Install and pin the Go toolchain across all machines; run and record a baseline `go test ./...` | M1 |
| **Build the compile-verification harness** — generate into a temp dir, then invoke the target compiler | M2 |
| Wire the harness into CI as a required check | M2 |
| Rust arm of the harness (`cargo build` on generated output) | M4 |
| Baseline security review of `internal/server`; write the threat model | M3 |
| Branch protection, PR template, conventional-commit enforcement, code-owner map | M1 |
| Diagram inventory + architecture diagrams v1 (architecture is already stable) | M5 |

**Exit gate:** CI fails on any generated output that does not compile, for every target. Baseline test results recorded. Every member can build and test locally.

> **Why this comes first.** C2 (Go's default strategy emits an undefined identifier) went undetected because every Go test forces `chi`, so the default path was never exercised. C1, C10, and C11 are the same shape. Harness first means the Phase 1 fixes are *proven*, not asserted.

---

### Phase 1 — Critical defects (Weeks 2–4)

All eleven critical findings, plus the registry security programme.

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

**Exit gate:** All eleven closed with a regression test each. `go test ./...` green. Compile harness green for all eight backends. An external penetration pass over the registry finds no auth bypass. **No known path produces non-compiling output.**

---

### Phase 2 — Parity, deduplication & high-severity (Weeks 5–8)

The systemic work. Phase 1 fixed instances; Phase 2 fixes the causes.

**The parity conformance matrix (M2, weeks 5–6)** is the centrepiece: a table-driven suite asserting that every backend handles every AST feature — query params, headers, `@serverSet`, WebSocket actions, typed errors, inheritance, maps, unions, defaults. Every cell is a test. A new AST feature added later without cross-backend handling becomes a build failure rather than a silent gap discovered by audit.

**Deduplication (M1, weeks 6–8)** collapses the four diverged duplicate implementations: two workspace-generation paths, two OpenAPI generators, two route-conflict detectors, and ~10 hand-written scalar-mapping switches.

Plus the fourteen high-severity findings, distributed by subsystem ownership.

**Exit gate:** Conformance matrix passes for all 8 backends × all AST features, or every failing cell is an explicitly documented and accepted limitation. Zero duplicated subsystems remain. `--strict` works in workspace mode.

---

### Phase 3 — New emitters & production hardening (Weeks 9–12)

Breadth and release-readiness in parallel. See §10 for the full new-target scope.

**Exit gate:** All new emitters pass the same conformance matrix as existing ones — no new target ships below the parity bar. Registry has versioned migrations, rate limiting, and test coverage. Distribution packages install successfully on a clean machine.

---

### Phase 4 — Integration & release candidate (Week 13)

Full-system integration across all 10 examples, end-to-end demo rehearsal, performance profiling on the six-service workspace, `v1.0.0-rc1` tagged, release notes written.

**Exit gate:** `v1.0.0-rc1` tagged. Demo rehearsed end to end twice with no manual intervention. All documentation merged.

---

## 5. Member 1 — Compiler core, CLI & developer tooling

**Owns:** `internal/{lexer,parser,ast,loader,validator,config,format,diff,lint,cache,lsp,mock,errors}`, `cmd/veld/`

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | Toolchain setup and baseline test run. Branch protection, PR template, CODEOWNERS, conventional-commit CI check. Repository hygiene: remove `registry.exe`, `test-*/`, and churn artefacts from the tree. |
| **2–3** | **C4** — the `prefix:` data-loss bug. Two options: emit a real `TPrefix` token from the lexer, or delete the dead branch and handle `prefix` as the contextual `TIdent` it actually is. Prefer the latter (smaller, matches the parser). Add formatter round-trip property tests: *format(format(x)) == format(x)* and *parse(format(x)) == parse(x)* — these would have caught this class immediately. |
| **4** | Config precedence bug (nested `validate:true` overriding explicit flat `validate:false`). Diff coverage for **enums and constants** — currently invisible to breaking-change detection. Success-status-change classification corrected to breaking. |
| **5–6** | **CLI deduplication.** Collapse the two divergent workspace-generation implementations into one shared function used by both `generate` and `watch`. **Wire the `--strict` breaking-change gate into workspace mode** — currently CI safety silently does nothing for microservices projects, the exact case where it matters most. |
| **7–8** | Deduplicate route-conflict detection (validator vs. linter). Extract the ~10 hand-written scalar-mapping switches into one shared table consumed by `schema`, `database`, `graphqlgen`, `openapigen`, and the language adapters. Fix lint false-positives on `@relation` / map-value / union-member references. |
| **9–10** | **Wire in the dead subsystems.** Add the `veld mock` command (the server is built and tested — only registration is missing). Adopt `internal/errors` across the pipeline, replacing `fmt.Errorf` so the LSP and CI can inspect failures programmatically. Read `config.ToolsConfig` or remove it. Implement or remove `--quiet`. |
| **11–12** | **LSP import resolution** — call the loader so cross-file types stop showing as undefined in editors. Spec-compliant JSON-RPC error responses. Make `internal/cache` goroutine-safe. Refactor the ~800-line `runInit` wizard out of `main.go` and switch its config writing from string concatenation to `encoding/json`. Replace the hardcoded `sh -c` post-generate hook with a cross-platform implementation. |
| **13** | Integration support, RC stabilisation, release-notes input. |

### Deliverables
Formatter that never loses source. One workspace implementation. `--strict` working everywhere. Four deduplicated subsystems. `veld mock` shipped. Structured errors adopted. LSP resolving imports.

---

## 6. Member 2 — Backend emitters

**Owns:** `internal/emitter/backend/{node,javascript,python,go,java,csharp,php}`, `internal/generators/`

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | **Build the compile-verification harness** — the highest-leverage deliverable in the plan. Generate into a temp dir and invoke `tsc --noEmit`, `go build`, `python -m py_compile`, `javac`, `dotnet build`, `php -l`. Wire into CI as a required check. Coordinate the interface with M4, who implements the `cargo build` arm. |
| **2** | **C2** — Go `plain` strategy: reconcile the router identifier (`mux` vs `r`). **C11** — C# scaffold brace escaping (`fmt.Sprintf` has no `{{` semantics). |
| **3** | **C3** — Python: write the missing `schemas/schemas.py` emitter, or change the default path to not import it. Decide deliberately whether `--validate` should become the default; document the choice. |
| **4** | **C9** — PHP `HEAD` actions: map `HEAD` to `Route::get` (Laravel handles HEAD implicitly) or reject it at validation. Same fix pattern for C# ASP.NET, which silently routes `HEAD`/`OPTIONS` as `GET` behind a `// TODO`. |
| **5–6** | **Build the parity conformance matrix.** Table-driven: every backend × every AST feature. This is the deliverable that prevents the next audit from finding the same class of drift. |
| **7** | **`@serverSet` enforcement** in java, csharp, php (M4 covers rust). Security-relevant — without it, clients can set their own `role` or `id`. Also fix JS and Python validators, which ignore it. |
| **8** | Go query/header params (currently passes literal `"nil"`, discarding request data). C# typed-exception handling (currently collapses everything into a flat 500). Go SDK base-URL error on unresolved (currently proceeds silently with an empty base). C#/PHP SDK return types — make them actually typed, as their own header comments claim. `extends` in C#/PHP SDK paths. |
| **9–10** | **New backend emitter #1 — Ruby** (Rails + Sinatra strategies). Full parity matrix compliance required before merge. |
| **11–12** | **New backend emitter #2 — Kotlin backend** (Ktor + Spring Boot strategies). Note: Kotlin currently exists only as a *client*; this makes it a first-class backend. |
| **13** | Integration, RC stabilisation. |

### Continuous
Wire in the dormant `validation.go` generators (java, csharp, php). Emit Python `requirements.txt` (implemented, never called). Fix or remove the Gin stub. Route C#/PHP through their existing unit-tested language adapters instead of duplicating logic inline. WebSocket handler bodies for go, csharp, php, javascript.

### Deliverables
Compile harness in CI. All 7 non-Rust backends compiling and passing the parity matrix. Two new backend languages. Zero dead code in `backend/`.

---

## 7. Member 3 — Frontend emitters, registry & production

**Owns:** `internal/emitter/frontend/*`, `internal/server/`, `internal/registry/`, `packages/`, CI/CD

### Week-by-week

| Weeks | Work |
|---|---|
| **1** | Registry threat model and security baseline. Audit every handler for authentication, authorisation, and input validation. Set up a staging registry instance. |
| **2–3** | **Registry security sprint — the highest-risk work in the project.** **C5:** add a `partial` claim to `JWTClaims`; reject partial tokens in `resolveAuth`. **C6:** filter `ListPackages` by actual org membership. **C7:** validate `PkgName`/`Version` against a strict character class, and reject `..` segments in `storage.path()` (the tarball extractor already does this — reuse its guard). **C8:** compare `expires_at` in `GetTokenByHash`. |
| **4** | **C10** — Swift path-parameter interpolation (`${id}` → `\(id)`). Add a Swift-specific interpolation helper rather than reusing the JS-shaped shared one. Extend Swift/Kotlin tests to assert on the **request URL**, not just the function signature. |
| **5** | Kotlin and Swift query parameters — currently accepted in the signature and silently discarded. |
| **6–7** | **WebSocket support in React, Vue, Svelte, Angular.** None branch on `act.Method == "WS"`; each generates a call to a method that does not exist. Angular additionally calls `this.http.ws()`, which is not part of Angular's `HttpClient`. |
| **8** | **Angular refactor** — currently reimplements model emission from scratch instead of composing the TypeScript emitter like React/Vue/Svelte do. Bring it onto the shared path: shared `VeldApiError`, shared base-URL configuration. |
| **9–10** | **New frontend emitters** — SolidJS (thin wrapper over the TS emitter, same pattern as React/Vue/Svelte), plus standalone **Python** and **Go** client emitters promoted from the existing inter-service SDK code. See §10. |
| **11–12** | **Production infrastructure.** Versioned schema migrations for the registry (currently one hand-written `CREATE TABLE IF NOT EXISTS` with no rollback). Rate limiting on login/TOTP/registration. Test coverage for `internal/server` — currently zero on every auth-critical path. Fix Homebrew/Chocolatey placeholder checksums; implement the pip download mechanism; unify version bumping across the five distribution packages. Remove the hardcoded Postgres password from the Docker Compose template. |
| **13** | Integration, RC stabilisation, release engineering. |

### Continuous
Low-severity registry items: login timing side-channel, unescaped HTML in emails, unrecovered email goroutine, stale duplicate SPA file. Enable private-package publishing (schema supports it; the handler hardcodes `public`). Wire up the VS Code Generate/Validate commands, which are declared but never registered in `activate()`.

### Deliverables
Registry with no known auth bypass and real test coverage. All 10 frontends handling WS, query params, and path params correctly. Three new frontend targets. Working distribution packages.

---

## 8. Member 4 — Rust (dedicated)

**Owns:** `internal/emitter/backend/rust/` entirely, plus the Rust client emitter and the Rust compile-verification arm.

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
| **4** | Rust `plain` strategy verification — confirm the non-Axum path compiles too (Go's equivalent default path was silently broken; assume nothing). |
| **5–6** | **Query and header parameters.** Axum extractors (`Query<T>`, `TypedHeader`) plus the plain-strategy equivalent. Full parity-matrix compliance for both. |
| **7** | `@serverSet` enforcement (input-type field omission). `@deprecated` emission via `#[deprecated]`. |
| **8** | **Wire in `validation.go`** — fully implemented and never called. Make `--validate` produce real Rust validation. |
| **9** | **WebSocket handler bodies** — replace the non-compiling `TODO` stub with a working `axum::extract::ws` implementation covering both `stream` and `emit`. |
| **10** | **Rust SDK correctness** — `extends` flattening (currently dropped), typed deserialisation, base-URL resolution error handling consistent with Node/Python. |
| **11** | **New: Actix-Web strategy.** Second production framework alongside Axum, exercising the strategy abstraction on the hardest language. |
| **12** | **New: standalone Rust client emitter** — register Rust as a *frontend* target, promoting the inter-service SDK into a first-class client. Raise Rust test coverage to match Java's. |
| **13** | Integration, RC stabilisation. |

### Deliverables
Rust output that compiles, in both strategies. Full parity-matrix compliance. Working WebSocket support. Actix-Web strategy. Rust as a client target. Test coverage comparable to the best-tested backend.

> **Buffer note.** Rocket as a third strategy is explicitly a buffer-month stretch goal, not a Week 1–13 commitment.

---

## 9. Member 5 — Documentation & diagrams

**Owns:** every diagram and every written document. **No delivery deadline** — the full 17 weeks are available.

The sequencing below is ordered by *when the underlying subject stabilises*, not by delivery pressure. Architecture is already stable and can be documented immediately; feature-level documentation must wait for Phase 2–3 to settle, or it will be rewritten.

### Workstream A — Architecture diagrams (weeks 1–5, subject already stable)

- Compilation pipeline (lexer → parser → AST → loader → validator → emitters)
- Emitter plugin registry — the three interfaces, three registries, and `init()` self-registration
- Strategy pattern — the language × framework matrix
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

- **"Adding a new emitter"** — the highest-value contributor document, written by observing M2 and M4 actually doing it
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

> **Standing risk.** Documentation written during Phase 1–2 will partially invalidate as Phase 3 lands new emitters. Mitigation: Workstreams C and D deliberately trail engineering by ~3 weeks, and the buffer month exists partly to absorb late documentation churn.

---

## 10. New emitter scope

New targets ship **only** if they pass the same parity conformance matrix as existing ones. Adding a target below the parity bar recreates exactly the drift this plan exists to eliminate.

### Backend (M2)

| Target | Frameworks | Weeks | Rationale |
|---|---|---|---|
| **Ruby** | Rails, Sinatra | 9–10 | Largest ecosystem with no Veld support. Rails is a dominant API-backend framework entirely unserved today. |
| **Kotlin** | Ktor, Spring Boot | 11–12 | Kotlin exists only as a client. Promoting it to a backend also serves Android teams wanting one language across the stack. Java infrastructure is reusable. |

### Rust (M4)

| Target | Weeks | Rationale |
|---|---|---|
| **Actix-Web strategy** | 11 | Second production Rust framework; validates the strategy abstraction on the hardest language. |
| **Rust client emitter** | 12 | Registers Rust as a frontend target, promoting the existing inter-service SDK. |

### Frontend (M3)

| Target | Weeks | Rationale |
|---|---|---|
| **SolidJS** | 9 | Cheapest new target available — same wrapper pattern as React/Vue/Svelte over the shared TypeScript emitter. High value per unit effort. |
| **Python client** | 9–10 | **Promotion, not new construction.** A working Python HTTP client already exists as an inter-service SDK; registering it as a frontend emitter makes it available to any consumer. |
| **Go client** | 10 | Same promotion pattern. |

> **The promotion insight.** Veld already generates Python, Go, Java, C#, PHP, and Rust HTTP clients — but only for *inter-service* use inside a workspace. Registering these as frontend emitters exposes existing, tested code to a much larger set of users at a fraction of the cost of writing new emitters. This is the highest return-per-hour work in Phase 3, and Java/C#/PHP clients follow the same pattern in the buffer month if capacity allows.

### Total new targets

**8 in the core 13 weeks:** 2 backends (Ruby, Kotlin), 1 Rust framework strategy (Actix), 5 client targets (Rust, SolidJS, Python, Go — plus Kotlin backend counting once). Taking the system from 8 backends / 10 frontends to **10 backends / 14 frontends**.

---

## 11. Critical path & dependencies

```
  M2: compile harness (W1)
        │
        ├──────────────► M2 backend fixes (W2–4)  ─────► parity matrix (W5–6) ─────► new backends (W9–12)
        │                                                       │
        └──────────────► M4 cargo arm (W1) ─► C1 Rust (W2–3) ───┤
                                                                │
                                          all backends must pass ┘
                                                                │
  M1: CLI dedup (W5–6) ──► --strict in workspace (W6) ──────────┤
                                                                │
  M3: registry security (W2–3) ──► prod infra (W11–12) ─────────┴──► P4 integration (W13) ──► v1.0-rc
```

### Hard dependencies

| Dependency | Consequence if late |
|---|---|
| **Compile harness (M2, W1) → everything** | Every output fix becomes unverifiable. **This is the single highest-priority deliverable in the plan.** If it slips, the whole schedule slips. |
| **Parity matrix (M2, W5–6) → all new emitters** | New targets have no acceptance bar and will ship with the same drift the plan exists to remove. |
| **C1 Rust compiles (M4, W2–3) → Rust parity work** | Nothing downstream in Rust is testable until the output compiles. |
| **CLI dedup (M1, W5–6) → `--strict` in workspace** | CI safety remains silently absent for microservices projects. |
| **Registry security (M3, W2–3) → staging deployment** | No instance can be exposed, blocking realistic integration testing. |

### Cross-member coordination points

- **W1:** M2 and M4 agree the compile-harness interface before either implements their arm.
- **W5:** M2 publishes the parity matrix schema; M3 and M4 review before it becomes a merge gate.
- **W6:** M1's workspace refactor touches code M2 and M3 depend on — schedule as a single reviewed PR, not incremental commits.
- **W9:** all owners brief M5 for the "Adding a new emitter" document while doing it.
- **W13:** full-team integration; no solo work.

---

## 12. Definition of done

### Per pull request
Compiles · tests pass · **compile harness green for affected targets** · parity matrix green · reviewed by one non-author · conventional commit message · documentation impact flagged to M5.

### Per new emitter
Registered via `init()` · implements the full role interface · passes the complete parity matrix · generated output compiles in CI · unit tests at parity with the best-covered existing emitter · one worked example · target guide drafted by M5.

### Per phase
All phase items closed or explicitly deferred with a written rationale · exit gate met · demo of new capability at phase review · M5 briefed.

### v1.0 release
- Zero known non-compiling output paths across all targets
- Zero known auth bypasses in the registry; `internal/server` test coverage no longer zero
- Parity matrix green across all 10 backends and 14 frontends
- All 10 examples generate and build end to end
- Distribution packages install on clean Linux, macOS, and Windows machines
- CI green including compile verification on all five platforms
- Complete documentation set and diagram set merged
- No dead packages: `internal/mock` and `internal/errors` are wired in or deleted

---

## 13. Process & ceremonies

| Cadence | Event | Duration | Participants |
|---|---|---|---|
| Daily | Async written standup | — | All 5 |
| Weekly | Sync — blockers, cross-member dependencies | 45 min | All 5 |
| Phase boundary | Review + demo + M5 briefing | 2 h | All 5 |
| Weekly | Documentation review | 30 min | M5 + one rotating owner |
| Buffer entry (W13) | Go/no-go on scope for weeks 14–17 | 1 h | All 5 |

**Branching:** trunk-based off `master`, short-lived feature branches, PR required, one non-author approval, CI green including the compile harness.

**Escalation:** any member blocked more than one working day raises it in standup. Any Phase 1 critical not closed by end of Week 4 is escalated at the phase review and consumes buffer.

---

## 14. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | **Compile harness (W1) proves harder than expected** — six toolchains in CI | Medium | **Critical** — blocks everything | Start Day 1. Ship incrementally: TypeScript and Go first (fastest), others follow. A partial harness still gates the backends being fixed in W2. |
| R2 | **Compile harness reveals defects beyond the 11 known** | **High** | High | Expected, not feared — this is the harness working. Weeks 14–17 exist substantially for this. Triage new findings against the same critical/high bar. |
| R3 | **Rust proves deeper than one person's capacity** | Medium | High | M4 is dedicated for all 17 weeks. Actix and Rocket are explicitly stretch goals, cut first. If C1 is not closed by W4, escalate and reassign the Actix work to buffer. |
| R4 | **Registry security work uncovers systemic design problems** | Medium | High | W1 threat model exists to surface this early. Fallback: ship v1.0 with the registry documented as beta and not recommended for public exposure — the compiler is independently valuable. |
| R5 | **New emitters cannot meet the parity bar in time** | Medium | Medium | Parity is non-negotiable; *scope* is. Cut targets rather than lowering the bar: Kotlin backend first, then Ruby. SolidJS and the promoted clients are cheap and survive any cut. |
| R6 | **M5's documentation invalidated by late feature changes** | **High** | Low | Workstreams C and D deliberately trail engineering by ~3 weeks. Architecture documentation (stable) is front-loaded; feature documentation is back-loaded. |
| R7 | **M1's workspace deduplication regresses microservice generation** | Medium | High | The six-service example is the regression test. Land as one reviewed PR with M2 and M3 as reviewers, never as incremental commits. |
| R8 | **Single-owner subsystems create bus-factor risk** | Medium | Medium | Every PR needs a non-author reviewer, which forces at least one other person through each subsystem. M5's contributor documentation is itself a mitigation. |
| R9 | **Buffer consumed early, leaving no margin** | Medium | Medium | W13 go/no-go gate. If more than two weeks of buffer are consumed before W13, cut new emitters — never cut security or compile verification. |

---

## 15. Buffer month policy

Weeks 14–17 are **not** planned work. They are margin, allocated in strict priority order.

| Priority | Claim on buffer |
|---|---|
| **1** | Overflow from Phases 1–2 — critical and high-severity defects. Non-negotiable. |
| **2** | Defects newly surfaced by the compile harness and parity matrix (see R2 — expect these). |
| **3** | Release hardening: RC feedback, cross-platform install verification, performance. |
| **4** | M5 documentation completion and academic deliverables. |
| **5** | Stretch targets — Rocket strategy; Java, C#, PHP client promotions; Elixir/Phoenix backend. |

**Rule:** priority 5 work begins only when priorities 1–4 are fully closed. New emitters are the *first* thing cut and the *last* thing added.

**Rule:** the buffer is never used to lower the parity bar or skip compile verification. A target that cannot meet the bar is deferred to post-v1.0, not shipped below it.

---

## Appendix — Ownership quick reference

| Area | Owner |
|---|---|
| Lexer, parser, AST, loader, validator | M1 |
| Config, cache, diff, lint, format | M1 |
| LSP, mock, structured errors | M1 |
| `cmd/veld/` — all commands | M1 |
| node-ts, node-js, python, go backends | M2 |
| java, csharp, php backends | M2 |
| `internal/generators/` | M2 |
| Ruby, Kotlin backends (new) | M2 |
| Parity conformance matrix | M2 |
| Compile-verification harness | M2 (+ M4 for Rust) |
| All 10 existing frontend emitters | M3 |
| SolidJS, Python, Go clients (new) | M3 |
| `internal/server/`, `internal/registry/` | M3 |
| CI/CD, `packages/`, release engineering | M3 |
| **Rust — backend, SDK, strategies, client** | **M4** |
| **All diagrams and documentation** | **M5** |
