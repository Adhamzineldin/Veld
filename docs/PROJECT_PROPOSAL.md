# Veld — Project Proposal & Execution Plan

## 1. Vision

Veld is a contract-first, multi-stack API code generator: a developer writes a single `.veld` contract and Veld emits typed backend service interfaces, route wiring, and frontend SDKs across ~14 languages/frameworks, with a self-hosted package registry (`veld serve`) for sharing contracts across teams like npm packages.

This project has two goals in parallel, not two competing tracks:

1. **A strong graduation/capstone deliverable** — a working, demoable, well-architected system.
2. **The foundation of a real product** — something companies could adopt to standardize API contracts across polyglot teams, with a registry as the natural expansion into a hosted/commercial offering.

The two goals share the same requirement: **production-quality architecture, security, scalability, documentation, and testing** — not academic-only polish. Grading rewards depth and correctness; adoption requires trust. The execution plan below is written to satisfy both at once.

## 2. System Overview

Pipeline (strictly linear — only AST JSON passes between stages):

```
.veld files → Lexer → Parser → AST → import resolver → Validator → Emitter(s) → generated/
```

- **Lexer / Parser / AST** — turn `.veld` contract text into a typed AST (models, enums, modules, actions).
- **Validator** — semantic checks (circular inheritance, unknown types, workspace `consumes` graph) with `file:line` error context.
- **Emitters** — one self-contained, self-registering package per language/framework target (`internal/emitter/backend/*`, `internal/emitter/frontend/*`). Adding a new target never touches the pipeline — it's a new package + one blank import.
- **Service SDKs** — for microservice workspaces, a `consumes` declaration generates a typed, zero-dependency client SDK for one service inside another's output.
- **Registry** — a separate, PostgreSQL-backed subsystem (`internal/server`) so teams can `veld push`/`veld pull` contract packages. This is the component with real users, auth, and a database — the part most directly tied to a commercial future, and currently the least mature part of the system.

## 3. Current State (verified against the repo, not assumed)

**Already solid:**
- CI (`.github/workflows/ci.yml`) runs build + `go vet` + `go test -race` on every push/PR, plus a 5-platform cross-compile matrix.
- Core pipeline and most emitters (Go, Java, C#, Python, JS, Node, Rust, PHP backends; React, Vue, Svelte, Angular, TS, Dart, Kotlin, Swift frontends) have test coverage.
- Onboarding docs exist and are substantial: `README.md`, `docs/DEVELOPER_GUIDE.md`, `docs/architecture/overview.md`, `docs/guides/getting-started.md`, `docs/roadmap.md`.

**Gaps, ranked by relevance to production trust:**

| # | Gap | Why it matters |
|---|-----|-----------------|
| 1 | `internal/server/` (the registry) has **zero tests** — auth token issuance, TOTP, middleware, publish/download handlers all untested | This is the subsystem holding user accounts, tokens, and uploaded packages. It's the top risk the moment anyone outside the team touches it. |
| 2 | **No schema migration tooling** — `db.migrate()` is one hand-written `CREATE TABLE IF NOT EXISTS` block, no versioning/rollback | Breaks down the first time two people change the schema in parallel, or a production DB needs a safe rollback. |
| 3 | **No `CONTRIBUTING.md`** | Team is growing past one contributor; conventions need to be written down, not tribal. |
| 4 | Repo root/`examples/` cluttered with churn artifacts (`COMPLETION-REPORT.md`, `FINAL-SUMMARY.md`, `START-HERE.md`, etc.) | Cosmetic, but undermines the "production-quality" impression for graders and prospective adopters browsing the repo. |
| 5 | `internal/language/imports.go:79` hardcodes `Line: 1` instead of real import line tracking | Minor correctness gap; good first issue for a new teammate. |
| 6 | `internal/registry` (client), `graphqlgen`, `openapigen`, `docsgen`, most `strategy/` subpackages lack tests | Lower priority than the server (no user data/auth at stake), but should be backfilled incrementally. |

Note: most `TODO`s found in the codebase are **intentional** — stubs Veld emits into *generated user code* (e.g. generated WebSocket handlers, generated test scaffolding), not gaps in Veld itself. The team should not spend time chasing those.

## 4. Execution Plan

### Phase A — Trust the Registry (security-critical, do first)
The registry is the one subsystem with real attack surface (auth, tokens, file uploads, a database). It should not accept external traffic — even from teammates — until it has test coverage and a real migration story.

- Write tests for `internal/server/auth/*`: token issuance/verification (`token.go`), TOTP 2FA (`totp.go`), auth middleware (`middleware.go`).
- Write tests for `internal/server/handlers/*`: publish (multipart upload), download, deprecate, org membership flows.
- Replace the hand-rolled `CREATE TABLE IF NOT EXISTS` block in `internal/server/db/db.go` with a versioned migration tool (`golang-migrate` or `goose`) — one migration file per schema change, with down-migrations.

**Exit criteria:** `go test ./internal/server/...` passes with meaningful coverage; schema changes go through migration files, not hand-edited `ALTER TABLE` strings.

### Phase B — Team Scalability
Cheap, high-leverage changes that reduce friction as the team grows.

- Add `CONTRIBUTING.md`: branch/PR conventions, how to add a new emitter, how to run the test suite locally.
- Clean up `examples/` root clutter into a single curated index; remove or archive redundant meta-docs (`COMPLETION-REPORT.md`, `FINAL-SUMMARY.md`, `START-HERE.md`, `QUICK-REFERENCE.md`, `README-NEW.md`).

**Exit criteria:** a new contributor can go from clone → first PR without asking a teammate a process question.

### Phase C — Breadth & Robustness (parallelizable across team members)
Once the highest-risk subsystem (registry) is covered, backfill test coverage elsewhere — this phase splits cleanly across multiple people since each item is independent.

- Tests for `internal/registry` (client side), `internal/graphqlgen`, `internal/openapigen`, `internal/docsgen`.
- Tests for the remaining `strategy/` subpackages (only C#'s currently has any).
- Fix `internal/language/imports.go:79` real line-number tracking.

**Exit criteria:** no package in `internal/` has zero test coverage without a documented reason.

### Phase D — Commercial Framing
Once A–C are underway, revisit the product angle explicitly as a team decision, not an afterthought.

- Reconcile `docs/roadmap.md` against this audit.
- Decide the hosting story for `veld serve` (who runs it, multi-tenant or self-hosted-only).
- Decide licensing/pricing model if pursuing commercialization.
- Run a security review pass before accepting any external registry traffic (this depends directly on Phase A being complete).

## 5. Suggested Sequencing & Ownership

Phase A should not be parallelized with Phase C — the registry is the highest-risk surface and deserves focused attention before spreading the team thin. Suggested split:

- **1–2 people:** Phase A (registry tests + migrations) — security-critical, sequential.
- **1 person, in parallel:** Phase B (docs/cleanup) — independent, no shared-file conflicts with Phase A.
- **Remaining team, after Phase A lands:** Phase C, split by subsystem (one person per untested package group).
- **Whole team:** Phase D, as a planning/discussion session once A–C give the team real data about system maturity.

## 6. Verification

- `go test ./... -race -count=1` should pass after each phase with no regressions.
- After Phase A: `go test ./internal/server/...` should no longer report "no test files."
- After Phase B: a `CONTRIBUTING.md` exists at repo root and `examples/` has no redundant root-level meta-docs.
- After Phase C: `go test ./... -cover` shows no `internal/` package at 0% coverage.
