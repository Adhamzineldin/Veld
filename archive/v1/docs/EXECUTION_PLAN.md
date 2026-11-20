# Veld — Execution Plan (Version 4)

**Project:** Veld — a contract-first, multi-stack API code generator
**Module:** `github.com/Adhamzineldin/Veld`
**Plan version:** 4
**Supersedes:** Version 3 (`docs/EXECUTION_PLAN_v3.md`), Version 2, Version 1
**Duration:** 16 weeks, divided into 6 phases
**Team:** 5 members — 4 implementing, 1 documentation
**Cadence:** tasks assigned weekly; every week ends with a checkpoint

---

## 1. What This Version Changes

Version 3 organised the project into six two-week sprints. Version 4 keeps the
same scope discipline but restructures delivery around **six phases with weekly
task assignment and a weekly checkpoint**, and adds three things Version 3 did
not cover:

| New in Version 4 | Where |
|------------------|-------|
| **A stabilisation phase that comes first** — audit, prioritise and resolve every existing error before any new work begins | §4 (Phase 1) |
| **Language and runtime version support** — version detection, compatibility testing, and a maintained compatibility matrix across all non-EOL versions | §5 (Phase 2) |
| **User-facing command standardisation** — the CLI surface made consistent and professional | §8 (Phase 5) |
| **Weekly assignment and weekly checkpoints** replacing two-week task blocks | §10, §11 |

Everything Version 3 established that you did not change is carried forward
unaltered: no new emitters, no new features, frozen-not-deleted tiers, and the
registry and plugin programmes.

---

## 2. Decisions and Assumptions

### 2.1 Confirmed by you

| Decision | Value |
|----------|-------|
| **Frontend adapters (5)** | `typescript`, `react`, `vue`, `angular`, `svelte` |
| **Language version support** | All actively supported (non-EOL) versions of every officially supported language and runtime, with version detection, compatibility testing, and a maintained compatibility matrix |
| **Documentation member** | Works the full 16 weeks; receives **no weekly implementation tasks** |

### 2.2 Assumed — stated openly, each with a one-line change path

These four were not settled. I have taken the default carried from Version 3 in
each case rather than block the plan. Say the word on any of them and the change
is local — no phase boundary moves.

| # | Assumption | If wrong |
|---|-----------|----------|
| **A1** | **Backend adapters (5)** are `node-ts`, `python`, `go`, `java`, `rust` | Swap `java` → `csharp` in §6.1 and reassign Member 3's Phase 3 weeks. Nothing else moves. |
| **A2** | **Rust remains exclusively Member 4's**, per your original instruction | If Rust may be shared, Member 4 absorbs a registry or plugin slice in Phases 4–5. |
| **A3** | **Duration is 16 weeks**, with Phase 6 acting as the buffer | A shorter run drops Phase 6 first, then compresses Phase 3. |
| **A4** | **Weekly assignment replaces the two-week task rule** from Version 3 | Revert to the v3 sprint grid; the phase structure survives either way. |

---

## 3. Phase Overview

| Phase | Weeks | Theme | Exit condition |
|-------|-------|-------|----------------|
| **P1** | 1–2 | **Stabilisation** — audit, prioritise, resolve all existing errors | Full build and test suite green on every installed toolchain; defect register closed to P0/P1 |
| **P2** | 3–4 | **Version & Compatibility** — detection, testing, matrix | Every generated project declares its supported runtime range; compatibility matrix published and CI-enforced |
| **P3** | 5–10 | **Adapters** — 5 backends (W5–W7), 5 frontends (W8–W10) | All ten adapters compile clean across their full version matrix |
| **P4** | 11–12 | **Registry** — correctness, performance, reliability | Registry meets §7.3; measured performance targets met |
| **P5** | 13–14 | **Plugins & Commands** — plugin system, CLI standardisation | Both plugins tested and packaged; CLI surface consistent |
| **P6** | 15–16 | **Hardening, buffer & release** | Definition of done (§12) satisfied from a clean clone |

Phases are project-level themes. Members work in parallel within each phase —
the week-by-week assignment in §10 is the authoritative statement of who does
what.

---

## 4. Phase 1 — Stabilisation (Weeks 1–2)

> *"Audit, prioritize, and resolve all current Veld errors and issues."*

### 4.1 Blocking finding: three toolchains are missing

Checked on the development machine on 2026-08-26:

| Toolchain | Status | Consequence |
|-----------|--------|-------------|
| Node / npm | ✅ installed | — |
| Python | ✅ installed | — |
| Java | ✅ JDK 25 (Adoptium) | — |
| PHP | ✅ 8.4 | — |
| **Go** | ❌ **not installed** | **`go build ./...` and `go test ./...` cannot run. Veld is written in Go — the project cannot be compiled or tested at all.** |
| **Rust / cargo** | ❌ not installed | The Rust adapter's output cannot be compile-verified. |
| **.NET** | ❌ not installed | The C# adapter's output cannot be compile-verified. |
| TypeScript | ⚠️ present only inside an unrelated project's `node_modules` | No global `tsc`; generated TypeScript cannot be checked reproducibly. |

**Installing Go is Week 1, Day 1, and it blocks everything else in the plan.**
Until it is done, no build error, test failure or `go vet` warning in this
repository has ever been observed — the defect register in §4.3 is therefore
seeded from static inspection and must be re-derived from a real build in W1.

### 4.2 What the codebase actually looks like

Correcting an impression Version 3 could have left: this project is **not**
untested. It has substantial coverage — with one glaring hole.

| Measure | Value |
|---------|-------|
| Test files | **52** `_test.go` files across the repository |
| Covered | lexer, parser, validator, loader, config, diff, format, lint, LSP, cache, errors, mock, schema, setup, every backend emitter, every frontend emitter, every tool generator |
| **Not covered** | **`internal/server/` — the registry. Zero test files, 1,699 lines.** |
| Open code markers | **23** `TODO` / `FIXME` / `HACK` / `XXX` across `internal/` and `cmd/` |
| Largest file | `cmd/veld/main.go` — **4,610 lines**, 33 command definitions |

### 4.3 Defect register — seeded categories

Week 1 produces the real register. These are the categories it must cover, each
already evidenced:

| ID | Category | Evidence | Priority |
|----|----------|----------|----------|
| **D1** | Build and test cannot run | Go not installed | **P0** |
| **D2** | Registry wholly untested | No `_test.go` under `internal/server/` | **P0** |
| **D3** | Open code markers | 23 `TODO`/`FIXME`/`HACK`/`XXX` — each triaged to fix, ticket, or delete | P1 |
| **D4** | Hardcoded runtime versions | 13 sites, §5.2 | P1 |
| **D5** | CLI inconsistency | §8.2 | P1 |
| **D6** | Registry operability gaps | §7.2 | P1 |
| **D7** | Plugin defects | §8.3 — including three commands declared but never registered | P1 |
| **D8** | `main.go` at 4,610 lines | Single file holding 33 commands | P2 |

**Prioritisation rule.** P0 = blocks other work or ships broken output. P1 =
user-visible defect or a correctness risk. P2 = maintainability. Phase 1 closes
all P0 and P1. P2 items are scheduled into later phases or the Phase 6 buffer —
never silently dropped.

---

## 5. Phase 2 — Language & Version Support (Weeks 3–4)

> *"Support all actively supported (non-EOL) versions of every officially
> supported language/runtime. Add proper version detection and compatibility
> testing. Maintain a clear compatibility matrix."*

### 5.1 The problem in one sentence

Veld currently hardcodes a single version of every runtime, in a single place,
with no detection, no configurability, and no test that any other version works.

### 5.2 Every hardcoded version — all 13 sites

| What | Where | Value |
|------|-------|-------|
| Go module directive | `internal/emitter/backend/go/middleware.go:157` | `go 1.22` |
| Rust edition | `internal/emitter/backend/rust/main.go:208` | `edition = "2021"` |
| Java (Gradle) | `internal/emitter/backend/java/strategy/plain.go:92` | `JavaVersion.VERSION_17` |
| Java (Maven) | `internal/emitter/backend/java/strategy/spring.go:280–282` | `<java.version>17</java.version>` + compiler source/target |
| .NET target | `internal/emitter/backend/csharp/strategy/aspnet.go:114` | `net8.0` |
| .NET target | `internal/emitter/backend/csharp/strategy/plain.go:58` | `net8.0` |
| CI — Go | `internal/generators/cicd/cicd.go:111` | `go-version: '1.22'` |
| CI — Java | `internal/generators/cicd/cicd.go:143` | `java-version: '17'` |
| CI — Python | `internal/generators/cicd/cicd.go:188` | `python-version: '3.12'` |
| CI — Node | `internal/generators/cicd/cicd.go:202` | `node-version: '20'` |
| Docker — Go | `internal/generators/dockerfile/dockerfile.go:86` | `golang:1.22-alpine` |
| Docker — Rust | `internal/generators/dockerfile/dockerfile.go:104` | `rust:1.76-slim` |
| Docker — Java | `internal/generators/dockerfile/dockerfile.go:122,130` | `maven:3.9-eclipse-temurin-17`, `eclipse-temurin:17-jre-jammy` |
| Docker — Python | `internal/generators/dockerfile/dockerfile.go:169` | `python:3.12-slim` |
| Docker — Node | `internal/generators/dockerfile/dockerfile.go:182,190` | `node:20-alpine` |

**Two silent gaps, worse than the hardcoding.** Generated `package.json` has
**no `engines` field** and generated Python has **no `requires-python`**. Node
and Python output therefore declares *nothing* about what runtime it needs — a
user on an unsupported version gets a syntax error, not a clear message.

**One live mismatch.** The development machine runs **JDK 25**; the emitters
target **Java 17**. Nobody has verified the generated Java on the JDK actually
installed here.

### 5.3 The three deliverables

**1 — Version detection.** A module that resolves, for each target, the version
range the generated code supports, in this precedence:

```
explicit config  →  detected local toolchain  →  project default (lowest non-EOL)
```

Generated output then *declares* that range: `engines` in `package.json`,
`requires-python` in Python packaging, the `go` directive, Java
source/target, and Cargo `edition` + `rust-version` (MSRV).

**2 — Compatibility testing.** The compile harness runs each adapter against
**every** non-EOL version in its column, not just one. A version that passes
enters the matrix as supported; one that fails enters as a documented exclusion
with the reason. No version is ever listed as supported without a passing job.

**3 — The compatibility matrix.** A generated, CI-updated document — never
hand-maintained, because a hand-maintained matrix is wrong within a month.

### 5.4 Indicative version columns

Derived at Week 3 from upstream support calendars — **these are indicative and
must be re-confirmed in W3**, because EOL dates move and this plan should not
be the authority on them.

| Runtime | Indicative non-EOL set | Policy |
|---------|------------------------|--------|
| Node.js | 22 LTS, 24 LTS (+ 20 if still in maintenance at W3) | Test every LTS in support |
| Python | 3.10 – 3.14 | Test every branch receiving security fixes |
| Go | last 3 minor releases | Go supports the last 2; we add one for margin |
| Java | 17, 21, 25 (LTS only) | LTS only — non-LTS releases are explicitly out of scope |
| Rust | current stable + pinned MSRV; editions 2021 and 2024 | MSRV is a declared, tested floor |

**Scope note.** "All officially supported languages" is read as the five Tier 1
adapters plus the frozen Tier 2 set. Tier 2 adapters (`node-js`, `csharp`,
`php`) get a **single** version column each — enough to keep the compile gate
honest, without paying for a full matrix on an adapter that carries no parity
promise.

---

## 6. Phase 3 — Adapters (Weeks 5–10)

### 6.1 The 5 backend adapters, in priority order

Priority is by deployment reach and by how much other work each unblocks.

| # | Adapter | Owner | Why this priority |
|---|---------|-------|-------------------|
| **1** | `node-ts` | Member 1 | The flagship. Its `tshelpers` type-mapping layer is shared with all five frontends — fixing it fixes six adapters at once. Highest leverage in the project. |
| **2** | `python` | Member 2 | Largest API-backend population after Node. Pydantic and Flask paths are the most-used non-Node output. |
| **3** | `go` | Member 2 | Veld's own host language. The team reads it fluently, the toolchain is already required to build the project, and its output is the cheapest to verify. |
| **4** | `java` | Member 3 | Enterprise reach, and the most recent work in the repository (commit `8703a62` fixed `ObjectMapper` date handling) — momentum is already there. |
| **5** | `rust` | **Member 4** | Highest per-fix cost — borrow checker and lifetimes make generated-code debugging slower than any other target. Isolated to one member by design so its tail risk cannot spread. |

Frozen at Tier 2, compile-gate only, no parity claim: `node-js`, `csharp`, `php`.

### 6.2 The 5 frontend adapters, in priority order

As you specified. Priority within the set is by dependency, not popularity —
`typescript` must be first because the other four wrap it.

| # | Adapter | Owner | Why this priority |
|---|---------|-------|-------------------|
| **1** | `typescript` | Member 1 | The base SDK. `react`, `vue`, `angular` and `svelte` all delegate to it — every defect here is four defects downstream. Must be green before the others start. |
| **2** | `react` | Member 1 | Largest user population; React Query hooks are the most complex wrapper. |
| **3** | `vue` | Member 3 | Composables — second-largest population. |
| **4** | `angular` | Member 3 | Enterprise; its DI-based service pattern is the most structurally different wrapper and most likely to hide defects. |
| **5** | `svelte` | Member 3 | Stores — smallest surface, lowest risk, correctly last. |

All five share one toolchain (`tsc`), which is what makes a five-adapter phase
fit in three weeks.

**Frozen at Tier 2:** `javascript`, `types-only`, `kotlin`, `swift`, `dart` —
kept, still generating, still compile-gated, but no parity claim.

### 6.3 Definition of green for an adapter

Identical for all ten, so results are comparable:

1. Compiles clean on **every** version in its compatibility column.
2. Passes every parity cell, or carries a written documented-gap.
3. Declares its supported runtime range in generated output.
4. Ships a worked example — contract in, running service or client out.
5. Zero runtime dependencies by default (**NON-NEGOTIABLE RULE 1**).

---

## 7. Phase 4 — Registry (Weeks 11–12)

> *"Fix all current Registry issues. Improve its performance, reliability, and
> efficiency."*

### 7.1 Correctness issues — verified

| Finding | Evidence |
|---------|----------|
| **Zero automated tests** | No `_test.go` anywhere under `internal/server/`; 1,699 lines unverified |
| **Migrations not versioned** | `db/db.go:43` — flat `CREATE TABLE IF NOT EXISTS`, then `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` at 124–127. No version table, no ordering, no rollback |
| **No rate limiting** | `server.go` — the entire chain is `Handler: cors(logger(s.mux))` |
| **Permissive CORS** | `Access-Control-Allow-Origin: *`, fixed, no allowlist |
| **No health or readiness endpoint** | `registerRoutes()` exposes only `/api/v1/...` routes |
| **Unstructured logging** | `logger` formats `"%s %s %s"` — no request ID, status or latency |
| **Upload bounded in memory only** | `handlers/packages.go:111` — `ParseMultipartForm(32 << 20)` bounds memory; the remainder spills to disk. No effective upload cap |
| **Oversized handler** | `handlers/auth.go` — 482 lines mixing registration, login, logout, TOTP, email verification |

### 7.2 Performance, reliability and efficiency

New in Version 4. Every target is **measured**, not asserted — Week 11 begins
by establishing a baseline, because "improve performance" is meaningless without
a before number.

| Dimension | Work | Target |
|-----------|------|--------|
| **Baseline** | Load-test publish, download and search; record p50/p95/p99 | A recorded baseline exists before any optimisation |
| **Database** | Index audit — every column used in a `WHERE`, `JOIN` or `ORDER BY`; eliminate N+1 in org-member and package-version listing | No unindexed query on a hot path |
| **Connection pooling** | `db.go` opens a pool with defaults; tune `MaxOpenConns`, `MaxIdleConns`, `ConnMaxLifetime` | Pool sized deliberately and documented |
| **Downloads** | Package tarballs are served through the process | `ETag` + `Cache-Control`; conditional `304` on repeat pulls |
| **Reliability** | Graceful shutdown exists; add DB retry with backoff on transient failure, and a storage-write failure path that does not corrupt package state | No partial publish survives a mid-write failure |
| **Efficiency** | Streaming upload handling to replace the memory-bounded multipart parse | Publish memory is bounded regardless of tarball size |

### 7.3 Registry definition of done

Tested handlers · versioned migrations · rate limiting · configured CORS ·
enforced upload cap · `/healthz` and `/readyz` · structured logging with request
IDs · API reference · operator runbook · passing end-to-end publish→pull test ·
**measured performance improvement against the Week 11 baseline**.

---

## 8. Phase 5 — Plugins & User Commands (Weeks 13–14)

### 8.1 Command standardisation — the CLI is the product's face

`cmd/veld/main.go` is **4,610 lines** defining **33 commands**. The surface has
drifted.

### 8.2 Verified inconsistencies

| Finding | Evidence | Fix |
|---------|----------|-----|
| **Two names for one concept** | `--out` used by 6 commands, `--output` by 5 | Standardise on `--out`; keep `--output` as a hidden deprecated alias |
| **Two near-identical flags** | `--service-sdk` and `--server-sdk` — one character apart, different meanings | Rename for unambiguity; alias the old names |
| **Inconsistent error handling** | 29 commands use `RunE:`, **10 use `Run:`** — those ten cannot return an error idiomatically, so exit-code behaviour differs across the CLI | All commands to `RunE:` |
| **Usage dumped on runtime errors** | Only **2** occurrences of `SilenceUsage`/`SilenceErrors` across 33 commands — most commands print full help text on an ordinary runtime failure | Set both at root |
| **No stdout/stderr discipline** | **273** `fmt.Println`/`Printf` calls in `main.go`, 4 `os.Exit` | Data → stdout, diagnostics → stderr, so output can be piped |
| **No consistent machine-readable output** | `--format` appears on only 2 commands | A uniform `--json` on every command that emits data |
| **Monolithic command file** | 4,610 lines, 33 commands, one file | Split by command group (P2 — may land in Phase 6) |

**Standard applied to every command:** consistent flag vocabulary · `RunE` with
proper exit codes · quiet on success, informative on failure · `--json` where
data is emitted · uniform help text with at least one example · no breaking
change without a deprecation alias.

### 8.3 Plugin system — verified defects

| Finding | Evidence |
|---------|----------|
| **Declared commands never registered** | `editors/vscode/package.json` contributes `veld.validate`, `veld.generate`, `veld.generateDryRun`; `activate()` at `src/extension.ts:1534` registers six providers and **zero** `registerCommand` calls. All three fail from the palette |
| **Four grammars, one language** | Go lexer/parser (authoritative) · 1,676-line TypeScript re-implementation · Kotlin PSI layer · `veld.tmLanguage.json`. A syntax change must land in four places |
| **The LSP server exists, unused** | `internal/lsp/` is 1,151 lines with a passing `lsp_test.go`; VS Code re-implements it client-side, JetBrains ignores it |
| **PATH assumption** | `VeldExternalAnnotator.kt:25` — `ProcessBuilder("veld", "validate")`, no configurable path, no fallback |
| **Zero plugin tests** | No test source anywhere under `editors/` |
| **Schema triplicated by hand** | `scripts/sync-schema.sh` copies the canonical schema to two plugins; nothing verifies the copies are current |
| **Stale artefact committed** | `editors/vscode/veld-vscode-0.1.0.vsix` checked in while `package.json` declares `0.2.0` |

**Out of scope, deliberately.** Migrating VS Code onto `internal/lsp/` is the
single largest simplification available to this codebase — and it is a feature
change, so it is post-v1.0 work (§13), not Phase 5.

---

## 9. Phase 6 — Hardening, Buffer & Release (Weeks 15–16)

Allocated in priority order: **carry-over** from any missed weekly checkpoint →
**P2 defects** deferred from Phase 1 → **release candidate** verified from a
fresh clone on a clean machine → **defence rehearsal** run by someone who did
not write the script.

> **No new adapter, no new feature.** The buffer exists to make what is already
> promised true. Surplus time goes to tests and documentation.

---

## 10. Weekly Task Assignment

The authoritative schedule. Member 5 does not appear — by your instruction they
carry no weekly implementation tasks (§11.2).

| Wk | Phase | Member 1 — Node/TS & web | Member 2 — Python/Go & infra | Member 3 — Java, web frameworks & registry | Member 4 — Rust only | Checkpoint |
|----|-------|--------------------------|------------------------------|--------------------------------------------|----------------------|------------|
| **1** | P1 | Audit `node-ts` + 5 frontends; log defects | **Install Go**; run full build, `go vet`, `go test ./...`; triage every failure | Audit `java`; read `internal/server/` end to end | **Install Rust**; audit `rust` adapter | **C1** — consolidated defect register, every item prioritised P0/P1/P2 |
| **2** | P1 | Fix P0/P1 in TS surface | Fix P0/P1 in Go/Python surface; triage all 23 `TODO`/`FIXME` markers | Fix P0/P1 in Java; write registry current-state assessment | Fix P0/P1 in Rust | **C2** — build green, `go test ./...` green, P0/P1 closed |
| **3** | P2 | Node/TS version policy; add `engines` to generated `package.json` | Build the **version-detection module**; stand up the CI version matrix | Java version policy; verify generated Java on **JDK 25** (currently targets 17) | Rust MSRV + edition policy (2021 / 2024) | **C3** — compatibility matrix v0 published; non-EOL columns confirmed against upstream calendars |
| **4** | P2 | Wire version declaration into TS/Node output | Add `requires-python`; make the 13 hardcoded sites configurable | Java source/target from detection, not constants | `rust-version` (MSRV) in generated `Cargo.toml` | **C4** — every generated project declares its runtime range; matrix CI-enforced |
| **5** | P3 | **`node-ts` to green** (priority 1 — unblocks 5 frontends) | **`python` to green** (priority 2) | **`java` to green** (priority 4) | **`rust`** — Serde structs, Axum handlers | **C5** — each backend demonstrated against the reference contract |
| **6** | P3 | `node-ts` across full version matrix; service SDK verified | `python` matrix; start **`go`** (priority 3) | `java` matrix; `ObjectMapper` date round-trip tested end to end | `rust` — services trait, parity cells | **C6** — backends green on every supported version, not just one |
| **7** | P3 | `node-ts` worked example + target guide | **`go` to green** + worked example | `java` worked example + target guide; `csharp` Tier 2 gate | `rust` — reqwest service SDK | **C7** — all 5 backends meet §6.3; Tier 2 freeze in place |
| **8** | P3 | **`typescript` to green** (priority 1 — the base all others wrap) | Tool-generator assertions; `php` Tier 2 gate | **`vue` to green** (priority 3) | `rust` — worked example | **C8** — base TS SDK green; no frontend proceeds until it is |
| **9** | P3 | **`react` to green** (priority 2) | Compile harness extended to the full frontend matrix | **`angular` to green** (priority 4 — most structurally different) | `rust` — target guide | **C9** — 4 of 5 frontends green |
| **10** | P3 | `react` matrix; frontend parity review across all 5 | CI matrix consolidation | **`svelte` to green** (priority 5) | `rust` — full version matrix (editions + MSRV) | **C10** — all 10 adapters meet §6.3 |
| **11** | P4 | Registry **API contract & auth** — document every route; decompose `auth.go` (482 lines); rate limiting; CORS from config | Registry **performance baseline** — load-test publish/download/search, record p50/p95/p99; index audit | Registry **operability** — `/healthz`, `/readyz`, structured logging with request IDs | `rust` — hardening against the finished parity matrix | **C11** — performance baseline recorded before any optimisation |
| **12** | P4 | Auth handler tests, including negative paths | Versioned migrations + `schema_migrations`; streaming upload cap; connection-pool tuning; ETag/`304` on downloads | SPA review; operator runbook; end-to-end publish→pull test | `rust` — release verification | **C12** — registry meets §7.3; measured gain vs C11 baseline |
| **13** | P5 | **VS Code extension** — register the 3 missing commands; test suite; remove stale `.vsix` | **CLI standardisation** — `--out`/`--output`; `--service-sdk`/`--server-sdk`; all `Run:` → `RunE:`; `SilenceUsage` | **JetBrains plugin** — configurable binary path; verify `VeldActions.kt`; test suite | `rust` — docs and examples review | **C13** — palette commands work in both editors |
| **14** | P5 | Plugin packaging verified via `PUBLISHING.md` by a second person | stdout/stderr discipline; uniform `--json`; help text with examples; schema-drift CI gate | Plugin packaging verified; JetBrains schema copy under the CI gate | `rust` — final matrix confirmation | **C14** — CLI consistent; both plugins tested and packaged |
| **15** | P6 | Carry-over + P2 defects | Carry-over; `main.go` split by command group if time allows | Carry-over + P2 defects | Carry-over | **C15** — release candidate cut from a fresh clone |
| **16** | P6 | Release verification | Release verification | Release verification | Release verification | **C16** — definition of done (§12) satisfied; defence rehearsed |

### 10.1 Reading the schedule

- **Dependencies are real.** `typescript` (W8) precedes `react`/`vue`/`angular`/`svelte`
  because all four wrap it. `node-ts` (W5) precedes everything because
  `tshelpers` is shared. The version-detection module (W3) precedes every
  adapter phase because adapters must declare ranges they cannot yet compute.
- **Member 4 is Rust for all 16 weeks**, per assumption **A2**. The Rust track
  is genuinely full: adapter parity, service SDK, worked example, target guide,
  and the edition-plus-MSRV version matrix, which is the deepest version work in
  the project.
- **Member 2 carries the shared infrastructure** — version detection, compile
  harness, CI matrix — which is why their adapter load is two backends rather
  than three.

---

## 11. Weekly Checkpoints

### 11.1 The protocol

Every week ends with a checkpoint (C1–C16). Thirty minutes, same four questions,
every member answers:

1. **Completed** — what was finished, *demonstrated*, not described.
2. **Issues** — what went wrong, and what is blocked.
3. **Carry-over** — what did not finish, and where it now goes.
4. **Next** — the coming week's assignment, confirmed in writing.

**The carry-over rule.** Work that does not finish is **never silently
extended**. It is re-planned at the checkpoint with a named owner and a new
target week. If it cannot land inside its phase, it moves to Phase 6 explicitly.

**Escalation.** Two consecutive checkpoints missing the same item triggers a
scope decision, not a third attempt — reassign, cut, or demote the adapter to
Tier 2.

**Evidence.** A checkpoint claim of "done" needs a build, a test run, or a
generated artefact. Nothing is accepted on assertion.

### 11.2 Member 5 — Documentation

Works the full 16 weeks and receives **no weekly implementation task**, by your
instruction. They attend every checkpoint as recorder, and own:

| Workstream | Content |
|------------|---------|
| Architecture | Pipeline, emitter registry and `init()` activation, interface hierarchy, strategy pattern, `consumes` service-SDK model |
| **Compatibility matrix** | The user-facing publication of §5 — which runtime versions are supported per adapter, and why any is excluded |
| Support matrix | Tier 1 / Tier 2 and the parity matrix, with every documented gap explained in plain language |
| Target guides | One per Tier 1 backend — `node-ts`, `python`, `go`, `java`, `rust` |
| CLI reference | Every command, flag and exit code — rewritten against the Phase 5 standard |
| Registry & operations | API reference, operator runbook, deployment and auth-flow diagrams |
| Editor plugins | Setup guide for both, feature comparison, authoritative-grammar statement |
| Defence materials | Slide deck, demonstration script, and the "what we deliberately did not build" section |

**Standing constraint.** No document may describe a capability that does not
exist in the repository, and no component may be presented as more mature than
it is. Every technical claim must be verifiable by a reader opening the code.

---

## 12. Definition of Done

1. **All P0 and P1 defects closed**; P2 items scheduled or explicitly deferred with a reason.
2. **5 backend adapters** — `node-ts`, `python`, `go`, `java`, `rust` — meet §6.3 on **every** version in their compatibility column.
3. **5 frontend adapters** — `typescript`, `react`, `vue`, `angular`, `svelte` — meet §6.3.
4. **Tier 2 adapters** — `node-js`, `csharp`, `php`, `javascript`, `types-only`, `kotlin`, `swift`, `dart` — compile or lint clean; the matrix states no parity is claimed.
5. **Version support complete** — detection implemented, every generated project declares its runtime range, compatibility testing runs on every supported version, and the matrix is CI-generated.
6. **Registry meets §7.3**, including a measured performance improvement against the Week 11 baseline.
7. **Both plugins meet §8.3** — commands registered, tests passing, packaging reproducible, schema drift CI-gated.
8. **CLI meets the §8.2 standard** — consistent flags, `RunE` throughout, stdout/stderr discipline, uniform `--json`.
9. **CI is green and gating** — compile harness, parity matrix and version matrix all block merge.
10. **Non-negotiable rules hold** — zero runtime dependencies by default, framework-agnostic handlers, native `fetch`.
11. **Member 5's workstreams complete**, every claim verifiable in the repository.

---

## 13. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|-----------|--------|------------|
| **R1** | Go is not installed, so nothing can be built or tested | **Confirmed** | **Critical** | Week 1 Day 1 task, ahead of all other work. Until resolved, no claim about build or test status is admissible. |
| **R2** | Rust and .NET missing; TypeScript only in an unrelated `node_modules` | **Confirmed** | High | Installed in W1. CI is the authoritative environment; missing toolchains skip with a recorded reason and never report a false pass. |
| **R3** | The version matrix multiplies CI cost beyond what is practical | High | Medium | Tier 2 adapters get one column, not a matrix. Full matrix runs nightly and on release branches; pull requests run the lowest and highest supported version only. |
| **R4** | EOL dates shift during the project, invalidating the matrix | Medium | Low | The matrix is generated, not hand-written. A version leaving support is a CI change, not a document rewrite. |
| **R5** | Supporting the oldest non-EOL version forces compromises in generated code | Medium | Medium | The floor is declared per adapter and tested. If a modern construct is worth more than an old version, the floor rises and the matrix records why. |
| **R6** | `typescript` slips at W8, blocking four frontends | Medium | High | It is scheduled alone in W8 with no competing task. If it slips, `svelte` (lowest priority) is cut to Tier 2 rather than delaying the phase. |
| **R7** | Rust proves harder than one member can carry | Medium | Medium | Decided at **C7** (W7), not later: Member 3 joins the Rust track for two weeks and `angular`/`svelte` move to Phase 6. |
| **R8** | Registry performance work finds no headroom, or regresses correctness | Low | Medium | W11 establishes the baseline *before* optimising. Any change failing the correctness suite is reverted regardless of its speed gain. |
| **R9** | CLI standardisation breaks existing users' scripts | Medium | High | Every renamed flag keeps a hidden deprecated alias. No flag is removed in v1.0 — only deprecated. |
| **R10** | Weekly checkpoints degrade into status reading | Medium | Medium | Every "done" claim requires a demonstration. The four questions are fixed and the recorder is Member 5, who is not competing for time. |
| **R11** | Phase 1 uncovers more defects than two weeks can absorb | Medium | High | P0/P1 close in Phase 1; P2 defers by design. If P0/P1 alone exceeds two weeks, Phase 3 loses `svelte` first, then `angular`. |
| **R12** | `main.go` at 4,610 lines makes CLI standardisation risky | Medium | Medium | Behavioural standardisation (flags, `RunE`, streams) comes first as P1; the file split is P2 and may land in Phase 6 or after v1.0. |

---

## 14. Post-v1.0 — Recorded, Not Absorbed

In priority order. Nothing here is attempted during the 16 weeks.

1. **Migrate the VS Code extension onto `internal/lsp/`**, deleting the 1,676-line client-side re-implementation of a language server the project already owns. Largest single simplification available.
2. **Split `cmd/veld/main.go`** by command group, if not reached in Phase 6.
3. **The seven cancelled Version 2 emitter commitments** — SolidJS, Ruby, Kotlin backend, Actix strategy, and three service-client promotions.
4. **Promote a Tier 2 adapter to Tier 1** — `csharp` is the strongest candidate.
5. **Generate the plugin grammars** from the Go lexer, retiring the four-grammar duplication.

---

## Appendix A — Ownership

| Component | Owner |
|-----------|-------|
| `internal/emitter/backend/node/` — `node-ts` | Member 1 |
| `internal/emitter/frontend/typescript/`, `react/` | Member 1 |
| `internal/emitter/tshelpers/` | Member 1 |
| `editors/vscode/` | Member 1 |
| `internal/emitter/backend/python/`, `go/` | Member 2 |
| `internal/generators/` — all six tool generators | Member 2 |
| Version detection · compile harness · CI matrix | Member 2 |
| `cmd/veld/main.go` — CLI standardisation | Member 2 |
| Registry data, migrations, performance | Member 2 |
| `internal/emitter/backend/java/` | Member 3 |
| `internal/emitter/frontend/vue/`, `angular/`, `svelte/` | Member 3 |
| `internal/server/` — registry operability, **programme lead** | Member 3 |
| `editors/jetbrains/` | Member 3 |
| `internal/emitter/backend/rust/` — **exclusive** | Member 4 |
| `docs/` — all documentation and diagrams | Member 5 |

---

## Appendix B — Checkpoint Index

| ID | Week | Phase | Gate |
|----|------|-------|------|
| C1 | 1 | P1 | Defect register complete and prioritised |
| C2 | 2 | P1 | Build green, tests green, P0/P1 closed |
| C3 | 3 | P2 | Compatibility matrix v0 published |
| C4 | 4 | P2 | Runtime ranges declared; matrix CI-enforced |
| C5 | 5 | P3 | Backends demonstrated against the reference contract |
| C6 | 6 | P3 | Backends green on every supported version |
| C7 | 7 | P3 | All 5 backends meet §6.3 · **Rust escalation decision point** |
| C8 | 8 | P3 | Base `typescript` SDK green |
| C9 | 9 | P3 | 4 of 5 frontends green |
| C10 | 10 | P3 | All 10 adapters meet §6.3 |
| C11 | 11 | P4 | Registry performance baseline recorded |
| C12 | 12 | P4 | Registry meets §7.3 with measured gain |
| C13 | 13 | P5 | Palette commands work in both editors |
| C14 | 14 | P5 | CLI consistent; plugins tested and packaged |
| C15 | 15 | P6 | Release candidate cut from a fresh clone |
| C16 | 16 | P6 | Definition of done satisfied |

---

## Appendix C — Version History

| Version | Structure | Status |
|---------|-----------|--------|
| **v1** | Role-based division | Superseded — `docs/EXECUTION_PLAN_v1.md` |
| **v2** | Type-system families, expansion-oriented | Superseded — `docs/EXECUTION_PLAN_v2.md` |
| **v3** | Type-system families, six two-week sprints | Superseded — `docs/EXECUTION_PLAN_v3.md` |
| **v4** | Six phases, weekly assignment, weekly checkpoints; adds stabilisation, version support and CLI standardisation | **Current** |

---

*End of Execution Plan, Version 4.*
