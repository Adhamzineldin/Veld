# Veld — Execution Plan (Version 3)

**Project:** Veld — a contract-first, multi-stack API code generator
**Module:** `github.com/Adhamzineldin/Veld`
**Plan version:** 3
**Supersedes:** Version 2 (`docs/EXECUTION_PLAN_v2.md`), Version 1 (`docs/EXECUTION_PLAN_v1.md`)
**Duration:** 12 weeks core delivery + 4 weeks buffer = 16 weeks
**Team:** 5 members

---

## 1. Plan at a Glance

### 1.1 The four principles of this version

Version 3 is a deliberate narrowing of Version 2. Where Version 2 answered the
question *"what else can Veld generate?"*, Version 3 answers *"what can Veld
generate correctly, provably, and repeatably?"* Four principles govern every
decision below.

| # | Principle | What it means in practice |
|---|-----------|---------------------------|
| **P1** | **No new emitters. No new features.** | Not one new backend, frontend, tool generator, framework strategy, or contract keyword is written during the 16 weeks. Every commitment Version 2 made to *add* capability is withdrawn (§2.1). |
| **P2** | **Four backends carry the v1.0 guarantee.** | `node-ts`, `python`, `go`, `java` form the Tier 1 supported set for the first three members. `rust` is Tier 1 as well, owned exclusively by Member 4. Everything else is frozen, not deleted (§3). |
| **P3** | **The registry becomes a professional, organised service.** | Promoted from a Version 2 side-task to a headline objective with its own programme, its own sprint, and its own definition of done (§7). |
| **P4** | **The editor plugins become a maintained product surface.** | The VS Code extension and the JetBrains plugin are promoted from unowned side-artefacts to a named programme with a named owner per plugin and a shared correctness contract (§8). |

A fifth rule governs sequencing rather than scope:

> **P5 — Every task belonging to Members 1, 2 and 3 is exactly two weeks long.**
> No task is half a sprint, no task spills into the next one silently, and no
> task is "mostly done". A task either meets its written exit criteria at the
> sprint boundary or it is formally re-planned in front of the whole team (§15).

### 1.2 The sprint grid

Six two-week sprints of core delivery, then two two-week buffer sprints.

| Sprint | Weeks | Theme | Member 1 | Member 2 | Member 3 |
|--------|-------|-------|----------|----------|----------|
| **S1** | 1–2 | Ownership & baseline | TS/web emitters | Dynamic & systems emitters | JVM/.NET & mobile emitters |
| **S2** | 3–4 | Verification infrastructure | Reference semantics spec | Compile-verification harness | Parity conformance runner |
| **S3** | 5–6 | Tier 1 parity, pass 1 | `node-ts` to green | `python` + `go` to green | `java` to green |
| **S4** | 7–8 | Tier 1 parity, pass 2 + Tier 2 freeze | 7 web frontends to green | Tool generators + `php`/`node-js` freeze | Mobile clients + `csharp` freeze |
| **S5** | 9–10 | **Registry programme** | Registry API contract | Registry data & storage | Registry UI, ops & lead |
| **S6** | 11–12 | **Plugin programme + release** | VS Code extension | Shared grammar & schema CI | JetBrains plugin |
| **B1** | 13–14 | Buffer — carry-over & hardening | as re-planned | as re-planned | as re-planned |
| **B2** | 15–16 | Buffer — release candidate & defence | as re-planned | as re-planned | as re-planned |

**Member 4** runs a single continuous Rust track across weeks 1–12 with no
two-week subdivision (§11). **Member 5** runs the documentation and diagrams
workstream across the full 16 weeks with no fixed deadlines (§12).

---

## 2. What Changed From Version 2, and Why

### 2.1 Every "new emitter" commitment is withdrawn

Version 2 committed to seven pieces of new generation capability. All seven are
cancelled under **P1**.

| Version 2 commitment | Version 3 disposition | Reason |
|----------------------|-----------------------|--------|
| SolidJS frontend emitter | **Cancelled** | Eleventh frontend before any of the ten existing ones is verified. |
| Ruby backend emitter | **Cancelled** | Ninth backend; adds a toolchain nobody on the team maintains. |
| Kotlin **backend** emitter | **Cancelled** | Kotlin already exists as a *frontend client* (`internal/emitter/frontend/kotlin/`). A second Kotlin surface doubles the maintenance without doubling the value. |
| Actix-Web strategy for Rust | **Cancelled** | Axum already works. A second strategy is variance, not capability. |
| Python service-client promotion | **Cancelled as new work** | The Python service SDK already exists at `internal/emitter/backend/python/sdk.go`. It gets *verified*, not extended. |
| Go service-client promotion | **Cancelled as new work** | Same — `internal/emitter/backend/go/sdk.go` exists. |
| Rust service-client promotion | **Cancelled as new work** | Same — `internal/emitter/backend/rust/sdk.go` exists. |

The engineering time this frees is not returned to the schedule as slack. It is
redirected into the two verification systems (§9.2, §9.3), the registry
programme (§7) and the plugin programme (§8).

### 2.2 Eight registered backends, four carrying the guarantee

The registry calls in `internal/emitter/` show what is registered today:

- **8 backends:** `csharp`, `go`, `java`, `node-js`, `node-ts`, `php`, `python`, `rust`
- **10 frontends:** `angular`, `dart`, `javascript`, `kotlin`, `react`, `svelte`, `swift`, `typescript`, `types-only`, `vue`
- **6 tool generators:** `cicd`, `database`, `dockerfile`, `env`, `openapi`, `scaffold-tests`

The instruction driving this version is **four backends**. Version 3 implements
that as a *support tier*, not as a deletion.

> **Stated assumption.** "Four backends" is read as *four backends carried by
> Members 1–3* — `node-ts`, `python`, `go`, `java` — **plus `rust`, which is
> Member 4's sole responsibility and is therefore counted separately**. This
> reading is what makes the "Rust belongs only to Member 4" instruction
> coherent: Rust is not one of the three members' four.
>
> **If the intended reading is instead "four backends in total, Rust
> included"**, the change is a one-line swap and no other part of this plan
> moves: demote `java` from Tier 1 to Tier 2 in §3, and Member 3's S3 task
> (§10.3) becomes an early start on the mobile-client work currently in S4.
> Every sprint boundary, every dependency and every buffer week stays exactly
> where it is.

### 2.3 Frozen, not deleted

`node-js`, `csharp` and `php` keep their source, keep their `init()`
registration, and keep working. What they lose is the *promise*. Tier 2 means:
"this compiles, and we will not knowingly break it, but we do not claim
feature parity and we do not ship a worked example for it."

This is a documentation and testing change, not a code deletion. Nothing a user
can run today stops working (risk **R6**, §16).

### 2.4 Why two-week tasks

Two weeks is long enough to finish a real vertical slice — a whole emitter
brought to parity, a whole subsystem of the registry — and short enough that a
task which is going wrong is visible before a third of the project is gone. It
also produces exactly six tasks per member across the twelve core weeks, which
makes the workload directly comparable between members and directly auditable
at the defence.

---

## 3. The v1.0 Support Matrix

This table is the single public statement of what Veld v1.0 supports. It is
published by Member 5 and is the contract the CI gates enforce.

### 3.1 Backends

| Emitter | Tier | Owner | v1.0 guarantee |
|---------|------|-------|----------------|
| `node-ts` | **Tier 1** | Member 1 | Compiles · full parity matrix · worked example · target guide |
| `python` | **Tier 1** | Member 2 | Compiles · full parity matrix · worked example · target guide |
| `go` | **Tier 1** | Member 2 | Compiles · full parity matrix · worked example · target guide |
| `java` | **Tier 1** | Member 3 | Compiles · full parity matrix · worked example · target guide |
| `rust` | **Tier 1** | **Member 4** | Compiles · full parity matrix · worked example · target guide |
| `node-js` | Tier 2 | Member 1 | Compiles only — no parity claim |
| `csharp` | Tier 2 | Member 3 | Compiles only — no parity claim |
| `php` | Tier 2 | Member 2 | Lints (`php -l`) only — no parity claim |

### 3.2 Frontends

All ten registered frontends are Tier 1 for **type correctness** — every one of
them must produce output that its own toolchain accepts. None of them is
extended, and none is removed.

| Emitter | Owner | Verification |
|---------|-------|--------------|
| `typescript`, `javascript`, `react`, `vue`, `svelte`, `angular`, `types-only` | Member 1 | `tsc --noEmit` / `node --check` on generated output |
| `kotlin`, `swift`, `dart` | Member 3 | `kotlinc` / `swiftc -parse` / `dart analyze` on generated output |

### 3.3 Tool generators

All six (`cicd`, `database`, `dockerfile`, `env`, `openapi`, `scaffold-tests`)
are owned by Member 2 and verified by output-shape assertions rather than by
compilation, since their output is configuration rather than code (§9.5).

---

## 4. Division of the Emitters

Eighteen emitters and six tool generators are divided so that each member owns a
coherent *type-system family*. This is not an arbitrary split: it means a member
learns one mental model — structural typing, or duck typing, or nominal typing
with generics — and applies it across everything they own.

### 4.1 Member 1 — TypeScript and the web

| Kind | Owned |
|------|-------|
| Backends | `node-ts` (Tier 1), `node-js` (Tier 2) |
| Frontends | `typescript`, `javascript`, `react`, `vue`, `svelte`, `angular`, `types-only` |
| Subsystems | `internal/docsgen/`, `internal/graphqlgen/`, `internal/schema/` |
| Shared code | `internal/emitter/tshelpers/` — the type-mapping layer every TS surface depends on |
| Registry share | API contract & authentication surface (S5) |
| Plugin share | **VS Code extension** (S6) |

**Rationale.** `tshelpers` is imported by nine of Member 1's eleven emitters. A
change to `VeldFieldToTS` moves nine outputs at once. Concentrating that blast
radius in one owner is the single highest-value ownership decision in the plan.

### 4.2 Member 2 — Dynamic and systems languages

| Kind | Owned |
|------|-------|
| Backends | `python` (Tier 1), `go` (Tier 1), `php` (Tier 2) |
| Tool generators | `cicd`, `database`, `dockerfile`, `env`, `openapi`, `scaffold-tests` |
| Subsystems | `internal/openapigen/`, `internal/generators/` |
| Verification systems | **Compile-verification harness** (§9.2) |
| Registry share | Data layer, migrations & package storage (S5) |
| Plugin share | **Shared grammar & schema CI** (S6) |

**Rationale.** Member 2 owns the two languages the project's own tooling is
written in or closest to — Go is the host language, Python drives the document
pipeline — which makes them the natural owner of the harness that shells out to
every toolchain.

### 4.3 Member 3 — JVM, .NET and mobile

| Kind | Owned |
|------|-------|
| Backends | `java` (Tier 1), `csharp` (Tier 2) |
| Frontends | `kotlin`, `swift`, `dart` |
| Verification systems | **Parity conformance matrix** (§9.3) |
| Registry | **Programme lead** across all 16 weeks, plus UI & operations (S5) |
| Plugin share | **JetBrains plugin** (S6) |

**Rationale.** Nominal type systems with generics behave alike across Java, C#,
Kotlin, Swift and Dart — one mental model covers all five. The JetBrains plugin
is written in Kotlin (`editors/jetbrains/src/main/kotlin/`, 5,436 lines across
34 source files), which puts it in exactly the same family, so Member 3's plugin
work needs no context switch.

### 4.4 Member 4 — Rust, exclusively

| Kind | Owned |
|------|-------|
| Backend | `rust` (Tier 1) — Axum strategy, Serde structs, services trait |
| Service SDK | `internal/emitter/backend/rust/sdk.go` — reqwest client |

Member 4 owns Rust and nothing else. No shared subsystem, no registry share, no
plugin share, no tool generator. This is a deliberate isolation: Rust's borrow
checker and lifetime rules make generated-code debugging slower than any other
target in the project, and the plan protects the other four members from that
tail risk (§11, risk **R7**).

### 4.5 Member 5 — Documentation and diagrams

Owns no emitter. Owns every document, every diagram, and the defence materials
(§12).

---

## 5. Team Structure

| Member | Role | Time model |
|--------|------|------------|
| **Member 1** | TypeScript & web emitters · reference semantics author · VS Code extension | 6 × two-week tasks |
| **Member 2** | Dynamic & systems emitters · tool generators · compile harness · grammar & schema CI | 6 × two-week tasks |
| **Member 3** | JVM/.NET/mobile emitters · parity runner · **registry lead** · JetBrains plugin | 6 × two-week tasks |
| **Member 4** | Rust backend and Rust service SDK, exclusively | Continuous track, weeks 1–12 |
| **Member 5** | Documentation, diagrams, defence materials | Continuous, full 16 weeks, no fixed deadlines |

**Cross-cutting leads.** Member 3 is registry lead for the whole 16 weeks even
though the registry's concentrated sprint is S5 — the lead role is about keeping
the design coherent, not about doing all the work. Member 1 is reference-semantics
author, which means their S2 output is a hard dependency for Members 2, 3 and 4.

---

## 6. Calendar

```
        WEEK  1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16
              |---S1--|---S2--|---S3--|---S4--|---S5--|---S6--|---B1--|---B2--|

MEMBER 1   [ own TS ][ ref-sem ][node-ts ][frontends][ reg API ][ VS Code ][ buffer ][ buffer ]
              base      spec      green     green      contract   extension

MEMBER 2   [ own dyn ][ compile ][ py+go  ][ tools +  ][ reg data ][ grammar ][ buffer ][ buffer ]
              base      harness    green     freeze     storage     + schema

MEMBER 3   [ own JVM ][ parity  ][  java  ][ mobile + ][ reg UI   ][JetBrains][ buffer ][ buffer ]
              base      runner     green     freeze     + ops       plugin

MEMBER 4   [========== RUST: continuous track, weeks 1-12, no sprint split ==========][ buffer ]

MEMBER 5   [============= DOCUMENTATION & DIAGRAMS: weeks 1-16, no fixed deadlines =============]

           ^                     ^                    ^          ^         ^
           |                     |                    |          |         |
        kickoff            HARD DEPENDENCY:        registry   plugin    release
                        ref-sem spec must land    programme  programme  candidate
                        before S3 implementation
```

**The one hard dependency in the plan.** Member 1's reference semantics
specification (M1-S2) must be complete at the end of week 4, because Members 2,
3 and 4 implement against it from week 5 onward. If it slips, three people are
blocked. It is the only single point of schedule failure in the plan (risk
**R5**, §16).

---

## 7. Programme A — Registry Professionalisation

### 7.1 Why this is a headline objective

The registry (`internal/server/`, 1,699 lines across six files) works — it
registers users, issues `vtk_` tokens, publishes and downloads packages, manages
organisations, and serves an embedded SPA. What it is not yet is *operable*. The
gap is not features; it is the engineering discipline around the features.

### 7.2 Current state — verified, not asserted

Every row below can be checked by opening the named file.

| Finding | Evidence in the repository |
|---------|----------------------------|
| **Zero automated tests** | No `_test.go` file exists anywhere under `internal/server/`. Every one of the 1,699 lines is unverified by any test. |
| **Migrations are not versioned** | `internal/server/db/db.go:43` — `migrate()` issues a flat sequence of `CREATE TABLE IF NOT EXISTS`, followed at lines 124–127 by `ALTER TABLE users ADD COLUMN IF NOT EXISTS`. There is no schema-version table, no ordering guarantee and no rollback path. |
| **No rate limiting** | `internal/server/server.go` — the entire middleware chain is `Handler: cors(logger(s.mux))`. Registration, login and publish are all unthrottled. |
| **Permissive CORS** | The `cors` middleware sets `Access-Control-Allow-Origin: *` as a fixed value with no allowlist and no configuration hook. |
| **No health or readiness endpoint** | `registerRoutes()` enumerates every route the server exposes; all are `/api/v1/...` application routes. No orchestrator can determine whether the process is ready. |
| **Unstructured logging** | The `logger` middleware formats `"%s %s %s"`. No request ID, no latency, no status code, no structured fields — a failed publish cannot be traced. |
| **Upload size is bounded in memory only** | `internal/server/handlers/packages.go:111` — `r.ParseMultipartForm(32 << 20)` bounds *memory*, not upload size; the remainder spills to disk. There is no effective cap on what a client can push. |
| **Oversized handler file** | `internal/server/handlers/auth.go` is 482 lines, mixing registration, login, logout, TOTP and email verification in one file. |

### 7.3 Target state at v1.0

| Dimension | Target |
|-----------|--------|
| Tests | Every handler has table-driven tests against an ephemeral PostgreSQL; auth and publish paths have negative tests — bad token, expired token, wrong org, oversized tarball, duplicate version. |
| Migrations | Numbered, ordered, forward-only migration files with a `schema_migrations` version table. Startup refuses to serve on an unknown or newer schema version. |
| Security | Per-IP and per-token rate limits on registration, login and publish. CORS origin driven by config, defaulting to the configured `base_url`. Hard upload cap enforced before the body is read. |
| Operability | `/healthz` (process alive) and `/readyz` (database reachable, storage writable). Structured request logging with a request ID propagated into every log line. |
| Structure | `auth.go` decomposed by concern; no handler file over roughly 250 lines. |
| Documentation | A complete API reference for every `/api/v1` route, and an operator runbook covering deploy, backup and restore. |

### 7.4 How the work is distributed

The registry gets six person-weeks concentrated in S5, split three ways so that
each member works on the part closest to what they already own, plus Member 3's
continuous lead across all 16 weeks.

| Task | Owner | Scope |
|------|-------|-------|
| M1-S5 | Member 1 | API contract & authentication surface — route and response documentation, auth handler decomposition, rate limiting, CORS configuration |
| M2-S5 | Member 2 | Data & storage — versioned migrations, storage-layer tests, upload cap, tarball verification tests |
| M3-S5 | Member 3 | UI, operations & integration — SPA review, health and readiness endpoints, structured logging, runbook, end-to-end publish→pull test |

---

## 8. Programme B — The Veld Editor Plugins

### 8.1 Why this is a headline objective

Veld ships two editor plugins today: a VS Code extension (`editors/vscode/`,
1,676 lines of TypeScript in `src/extension.ts`) and a JetBrains plugin
(`editors/jetbrains/`, 5,436 lines of Kotlin across 34 source files). Together
they are the first thing a new user of Veld touches — before the CLI, before the
registry, before any generated code. They are also, at present, the least
maintained surface in the repository.

They are promoted here to a full programme with a named owner per plugin,
because a contract-first tool whose contract editor misleads the author has
failed at the first step.

### 8.2 Current state — verified, not asserted

| Finding | Evidence in the repository |
|---------|----------------------------|
| **Declared commands are never registered** | `editors/vscode/package.json` contributes three commands — `veld.validate`, `veld.generate`, `veld.generateDryRun` — with palette titles. `activate()` at `editors/vscode/src/extension.ts:1534` registers completion, hover, definition, reference, semantic-token and document-symbol providers, and **not a single `registerCommand` call**. Invoking any of the three from the command palette fails. |
| **Four grammars describe one language** | The authoritative grammar is `internal/lexer/lexer.go` + `internal/parser/parser.go`. It is re-implemented a second time in TypeScript inside `extension.ts`, a third time in Kotlin (`VeldLexer.kt`, `VeldParser.kt`, plus seven PSI declaration classes), and a fourth time as a TextMate grammar in `editors/vscode/syntaxes/veld.tmLanguage.json`. Any syntax change must land in four places or the editors lie to the user. |
| **The LSP server exists but neither plugin uses it** | `internal/lsp/` is 1,151 lines with a working handler, completion, hover, definition and protocol layer — and `lsp_test.go`, the only test in the whole editor-adjacent surface. The VS Code extension re-implements all of it client-side instead; the JetBrains plugin ignores it entirely. |
| **JetBrains diagnostics shell out to a PATH binary** | `VeldExternalAnnotator.kt:25` runs `ProcessBuilder("veld", "validate")` and scrapes stderr with a regex. There is no configurable binary path and no fallback if `veld` is not on `PATH`. |
| **Zero tests in either plugin** | No test source exists anywhere under `editors/`. |
| **The config schema is triplicated by hand** | `editors/veld-config.schema.json` is canonical; `scripts/sync-schema.sh` copies it into `editors/vscode/` and `editors/jetbrains/src/main/resources/schemas/`. Nothing verifies the copies are current — a stale copy silently ships. |
| **The language spec is a hand-maintained parallel pair** | `editors/vscode/src/veld-language-spec.ts` (50 lines) and `VeldLanguageSpec.kt` (53 lines) hold the same keyword and type lists in two languages, kept in step by discipline alone. |
| **A stale build artefact is committed** | `editors/vscode/veld-vscode-0.1.0.vsix` is checked in while `package.json` declares version `0.2.0` — the packaged extension in the repository is not the extension in the repository. |

### 8.3 Target state at v1.0

| Dimension | Target |
|-----------|--------|
| Commands | All three contributed commands registered and working in VS Code; equivalent actions verified in the JetBrains plugin (`VeldActions.kt`). |
| Single source of truth | Keyword lists, type names and annotation names are generated from one source into both plugins rather than hand-copied, so the parallel spec pair cannot drift. |
| Schema integrity | A CI check fails the build if any synced copy of `veld-config.schema.json` differs from the canonical file. `scripts/sync-schema.sh` stops being a discipline and becomes a gate. |
| Diagnostics | The JetBrains annotator resolves the `veld` binary from a configurable setting with a clear error when it is missing, instead of assuming `PATH`. |
| Tests | Both plugins have a test suite covering syntax highlighting, completion and diagnostics on a shared fixture contract. |
| Packaging | Reproducible packaging documented in both `PUBLISHING.md` files; the stale committed `.vsix` removed and build artefacts ignored. |
| Documentation | An editor-setup guide covering install, configuration and the alias/import behaviour for both plugins. |

**Explicitly out of scope under P1.** No new plugin, no new editor, and no
migration of either plugin onto `internal/lsp/`. Replacing the VS Code
extension's client-side language implementation with the LSP server the project
already owns is the single largest simplification available to this codebase —
and it is a *feature change*, so it is recorded in §17.2 as the first item of
post-v1.0 work, not attempted here.

### 8.4 How the work is distributed

Like the registry programme, the plugin programme gets six person-weeks
concentrated in one sprint, split so each member stays inside the language
family they already own.

| Task | Owner | Scope |
|------|-------|-------|
| M1-S6 | Member 1 | VS Code extension — TypeScript, matching Member 1's existing family |
| M2-S6 | Member 2 | Shared grammar source-of-truth and schema-drift CI — Go-side tooling, matching Member 2's harness ownership |
| M3-S6 | Member 3 | JetBrains plugin — Kotlin, matching Member 3's existing family |

---

## 9. Shared Engineering Systems

### 9.1 Reference contract

One `.veld` contract, checked into the repository, that exercises every feature
the language has: models, `extends` inheritance, enums, optional fields, arrays,
`Map<K,V>`, defaults, `@deprecated` on both fields and actions, all five HTTP
methods, path parameters, query models, custom headers, middleware, explicit
status codes, and multi-file imports in both alias and relative style. Every
verification system below runs against this one contract, which is what makes
results comparable across eighteen emitters. It also serves as the shared
fixture for both plugin test suites (§8.3). **Authored by Member 1 in S1.**

### 9.2 Compile-verification harness — Member 2, S2

Generates the reference contract to a temporary directory for a given emitter,
then invokes the real toolchain against the output and asserts a zero exit code:

| Target | Command |
|--------|---------|
| `node-ts` + TypeScript frontends | `tsc --noEmit` |
| `node-js` + JavaScript frontend | `node --check` |
| `python` | `python -m py_compile` |
| `go` | `go build ./...` |
| `java` | `javac` |
| `csharp` | `dotnet build` |
| `php` | `php -l` |
| `rust` | `cargo build` |
| `kotlin` / `swift` / `dart` | `kotlinc` / `swiftc -parse` / `dart analyze` |

Toolchains that are unavailable on a given machine **skip with a recorded
reason**; they never silently pass. The harness runs as a required CI check for
every Tier 1 and Tier 2 emitter.

### 9.3 Parity conformance matrix — Member 3, S2

A table-driven suite of *(emitter × language feature)* cells. Each cell asserts
that the generated output contains the construct the reference semantics
specification says it must. A cell has exactly three states: **pass**,
**fail**, or **documented-gap** — with the gap written into the support matrix
and visible to users. There is no fourth state, and in particular there is no
"unknown". Merging a change that turns a passing cell red is blocked.

### 9.4 Reference semantics specification — Member 1, S2

Prose, not code. For every language feature, it states what the generated output
must mean, independent of target language. For example: *an optional field must
be omittable, not null-valued, in every target that can distinguish the two*.
Without this document, eighteen emitters make eighteen independent judgement
calls and "parity" becomes unfalsifiable. It is the hard dependency of §6.

### 9.5 Tool-generator assertions — Member 2, S4

Tool generators emit configuration, not code, so they are verified by shape: the
GitHub Actions workflow parses as valid YAML and names a job per detected
language; the Dockerfile's stages are ordered and its base images pinned; the
OpenAPI document validates against the 3.0 schema; the SQL schema parses; the
`.env` template lists exactly the variables the contract implies.

---

## 10. The Eighteen Two-Week Tasks

Every task below is exactly two weeks and carries written exit criteria. A task
is done when its exit criteria are demonstrably met — not when the member
believes it is close.

### 10.1 Member 1 — TypeScript and the web

---

#### **M1-S1 · Weeks 1–2 · Own the TypeScript surface and establish the baseline**

**Scope.** Take ownership of `node-ts`, `node-js` and the seven web frontends
(`typescript`, `javascript`, `react`, `vue`, `svelte`, `angular`, `types-only`).
Author the reference contract of §9.1. Generate every one of the nine emitters
against it and record, per emitter, exactly what is produced and what fails.

**Exit criteria.**
1. The reference contract is committed and exercises every documented language feature.
2. All nine emitters have been run against it; output is committed as a reviewable baseline.
3. A written defect list exists, one entry per emitter, ranked by severity.
4. `internal/emitter/tshelpers/` is documented: what each function does and which emitters call it.

---

#### **M1-S2 · Weeks 3–4 · Reference semantics specification** *(hard dependency)*

**Scope.** Write the specification of §9.4 — the target-independent meaning of
every language feature, covering optionality, arrays, maps, inheritance, enums,
defaults, deprecation, status codes, path/query/header binding and error shape.
Review it with Members 2, 3 and 4, because they implement against it.

**Exit criteria.**
1. Every feature in the reference contract has a written semantic definition.
2. Members 2, 3 and 4 have each signed off that the specification is implementable in their targets.
3. Every cell of the parity matrix has a defined pass condition traceable to a specification clause.
4. Committed under `docs/` by the end of week 4 — no extension is available for this task.

---

#### **M1-S3 · Weeks 5–6 · `node-ts` to full parity**

**Scope.** Bring the flagship backend to green: every parity cell passes, output
compiles under `tsc --noEmit`, Zod schemas match the semantics specification,
route handlers produce the documented status codes, and the service SDK
(`internal/emitter/backend/node/sdk.go`) is verified.

**Exit criteria.**
1. Every `node-ts` parity cell is pass or documented-gap; none is fail.
2. Generated output compiles clean under the compile harness.
3. A worked example — contract in, running Express service out — is committed and reproducible.
4. Zero runtime dependencies in generated type-only output, verified by inspection (**NON-NEGOTIABLE RULE 1**).

---

#### **M1-S4 · Weeks 7–8 · Seven web frontends to full parity**

**Scope.** Bring `typescript`, `javascript`, `react`, `vue`, `svelte`, `angular`
and `types-only` to green. Verify `VeldApiError`, path-parameter interpolation,
all five HTTP methods, and base-URL resolution behave identically across all
seven.

**Exit criteria.**
1. All seven compile or parse clean under the harness.
2. Error handling and base-URL resolution are demonstrably identical across the seven.
3. Each framework wrapper (`react`, `vue`, `svelte`, `angular`) is shown correctly delegating to the TypeScript SDK rather than re-implementing it.
4. `node-js` passes its Tier 2 compile-only gate.

---

#### **M1-S5 · Weeks 9–10 · Registry API contract and authentication surface**

**Scope.** Member 1's share of the registry programme (§7.4). Document every
`/api/v1` route with method, auth requirement, request and response shape.
Decompose `handlers/auth.go` (482 lines) by concern. Add per-IP and per-token
rate limiting to registration, login and publish. Replace fixed
`Access-Control-Allow-Origin: *` with a configured origin.

**Exit criteria.**
1. Complete API reference committed; every route in `registerRoutes()` appears in it.
2. `auth.go` decomposed; no resulting handler file exceeds roughly 250 lines.
3. Rate limiting active on the three named endpoints, with tests proving the limit triggers.
4. CORS origin read from configuration, defaulting to `base_url`; the wildcard is gone.

---

#### **M1-S6 · Weeks 11–12 · VS Code extension**

**Scope.** Member 1's share of the plugin programme (§8.4). Register the three
contributed commands that `activate()` currently omits — `veld.validate`,
`veld.generate`, `veld.generateDryRun` — so they work from the palette. Add a
test suite over the shared fixture contract covering highlighting, completion
and diagnostics. Remove the stale committed `veld-vscode-0.1.0.vsix` and ignore
build artefacts. Verify `PUBLISHING.md` describes a reproducible package step.

**Exit criteria.**
1. All three palette commands execute successfully against a real project; failure modes — no config found, binary missing — produce a clear message rather than an exception.
2. An automated test suite runs in CI and covers highlighting, completion and diagnostics.
3. The stale `.vsix` is removed and packaging output is git-ignored.
4. `PUBLISHING.md` has been followed end to end by someone other than its author, and the packaged extension version matches `package.json`.

---

### 10.2 Member 2 — Dynamic and systems languages

---

#### **M2-S1 · Weeks 1–2 · Own the dynamic and systems surface and establish the baseline**

**Scope.** Take ownership of `python`, `go`, `php` and all six tool generators.
Generate each against the reference contract and record what is produced and
what fails. Audit the Flask and FastAPI strategies for `python` and the chi and
plain strategies for `go`.

**Exit criteria.**
1. All three backends and all six tool generators have been run against the reference contract; output committed.
2. A per-emitter defect list exists, ranked by severity.
3. Every framework strategy is documented: which frameworks exist, which is default, and what differs between them.
4. Toolchain availability for Python, Go and PHP is confirmed and recorded (see risks **R1** and **R2**).

---

#### **M2-S2 · Weeks 3–4 · Compile-verification harness**

**Scope.** Build the system of §9.2 and wire it into CI as a required check.

**Exit criteria.**
1. The harness runs every Tier 1 and Tier 2 emitter through its real toolchain.
2. Missing toolchains skip with a recorded reason and are never reported as passes.
3. The harness runs in CI and blocks merge on failure.
4. A one-command local invocation is documented so any member can reproduce a CI failure.

---

#### **M2-S3 · Weeks 5–6 · `python` and `go` to full parity**

**Scope.** Bring both Tier 1 backends to green against the reference semantics
specification. For Python: Pydantic schema fidelity, ABC service interfaces,
Flask route generation, `try/except` behaviour. For Go: chi routing, typed
interfaces, generated `go.mod` and `server.go`, and the flattening of model
inheritance that Go's lack of struct inheritance forces.

**Exit criteria.**
1. Every `python` and `go` parity cell is pass or documented-gap.
2. Both compile clean under the harness.
3. Two worked examples — one Flask service, one chi service — are committed and reproducible.
4. The inheritance-flattening rule for Go is written into the support matrix as a documented, deliberate divergence.

---

#### **M2-S4 · Weeks 7–8 · Tool generators verified; `php` and `node-js` frozen**

**Scope.** Implement the tool-generator assertions of §9.5 for all six
generators. Establish and enforce the Tier 2 compile-only gate for `php`, and
confirm the `node-js` gate Member 1 established in S4.

**Exit criteria.**
1. All six tool generators have shape assertions running in CI.
2. `php` passes `php -l` on generated output in CI.
3. The Tier 2 policy — compiles, no parity claim — is written into the support matrix for `php`, `node-js` and `csharp`.
4. Language auto-detection in the CI/CD and Dockerfile generators is verified against all five Tier 1 backends.

---

#### **M2-S5 · Weeks 9–10 · Registry data layer and storage**

**Scope.** Member 2's share of the registry programme (§7.4). Replace the flat
`migrate()` in `db.go` with numbered, ordered, forward-only migrations backed by
a `schema_migrations` table. Add a hard upload cap enforced before the body is
read, replacing the memory-only bound at `packages.go:111`. Add tests for the
storage layer and for tarball pack, unpack and verify.

**Exit criteria.**
1. Migrations are numbered and ordered; startup refuses to serve on an unknown or newer schema version.
2. A documented upgrade path exists from the current flat schema to the versioned one.
3. Uploads above the configured cap are rejected before the body is consumed, with a test proving it.
4. Storage and tarball layers have table-driven tests, including the corrupt-tarball and duplicate-version paths.

---

#### **M2-S6 · Weeks 11–12 · Shared grammar source-of-truth and schema-drift CI**

**Scope.** Member 2's share of the plugin programme (§8.4). Make the keyword,
type and annotation lists that today live twice — in
`editors/vscode/src/veld-language-spec.ts` and `VeldLanguageSpec.kt` — derive
from one source, so the two plugins cannot drift from each other or from the Go
lexer. Turn `scripts/sync-schema.sh` from a manual discipline into a CI gate
that fails when any synced copy of `veld-config.schema.json` differs from the
canonical file.

**Exit criteria.**
1. Both plugins' language spec files are generated from one source; regenerating on a clean tree produces no diff.
2. A CI check fails the build when a synced schema copy is stale, demonstrated on a deliberately stale copy.
3. The set of keywords the plugins recognise is verified equal to the set `internal/lexer/lexer.go` accepts.
4. The four-grammar duplication is documented in the developer guide with an explicit statement of which grammar is authoritative.

---

### 10.3 Member 3 — JVM, .NET and mobile

---

#### **M3-S1 · Weeks 1–2 · Own the JVM/.NET/mobile surface and establish the baseline**

**Scope.** Take ownership of `java`, `csharp`, `kotlin`, `swift`, `dart` and —
as programme lead — the registry. Generate all five emitters against the
reference contract. Read `internal/server/` end to end and produce the
registry's current-state assessment.

**Exit criteria.**
1. All five emitters have been run against the reference contract; output committed.
2. A per-emitter defect list exists, ranked by severity.
3. A registry current-state assessment is written, confirming or correcting every finding in §7.2.
4. The registry work plan for S5 is agreed with Members 1 and 2.

---

#### **M3-S2 · Weeks 3–4 · Parity conformance matrix**

**Scope.** Build the system of §9.3 and wire it into CI as a merge gate.

**Exit criteria.**
1. The matrix covers every Tier 1 emitter × every feature in the reference semantics specification.
2. Each cell reports pass, fail or documented-gap — never "unknown".
3. The matrix renders as a committed table that Member 5 can publish directly.
4. A change that turns a passing cell red is blocked from merging, demonstrated on a deliberate regression.

---

#### **M3-S3 · Weeks 5–6 · `java` to full parity**

**Scope.** Bring the Java backend to green: Spring Boot controllers, service
interfaces, and `BigDecimal` handling for `decimal`. Verify that the
`ObjectMapper` configuration — `JavaTimeModule` and ISO-8601 date serialization,
added in commit `8703a62` — behaves correctly for `date` and `datetime` under
the reference semantics specification. Verify the Java service SDK.

**Exit criteria.**
1. Every `java` parity cell is pass or documented-gap.
2. Generated output compiles clean under `javac` in the harness.
3. A worked example — contract in, running Spring Boot service out — is committed and reproducible.
4. `date` and `datetime` round-trip is tested end to end, not merely asserted from the `ObjectMapper` configuration.

---

#### **M3-S4 · Weeks 7–8 · Mobile clients to parity; `csharp` frozen**

**Scope.** Bring `kotlin`, `swift` and `dart` to green. Establish the Tier 2
compile-only gate for `csharp`.

**Exit criteria.**
1. All three mobile clients parse or compile clean under their toolchains in the harness.
2. Error handling and base-URL resolution match the TypeScript SDK's documented behaviour, or the divergence is recorded as a documented gap.
3. `csharp` passes `dotnet build` on generated output in CI.
4. The mobile clients' `decimal` handling is verified against the extended type-mapping table.

---

#### **M3-S5 · Weeks 9–10 · Registry UI, operations and integration**

**Scope.** Member 3's share of the registry programme (§7.4), plus integration
of Members 1 and 2's registry work. Add `/healthz` and `/readyz`. Replace the
`"%s %s %s"` logger with structured logging carrying a request ID. Review the
embedded SPA. Write the operator runbook. Build the end-to-end publish→pull
test.

**Exit criteria.**
1. `/healthz` and `/readyz` exist; `/readyz` fails when the database is unreachable or storage is unwritable, demonstrated.
2. Structured logging emits a request ID, path, status and latency, and the ID is traceable across a single request's log lines.
3. An end-to-end test performs `login → push → pull` against a live server and asserts a byte-identical round-trip.
4. The operator runbook covers deploy, configuration precedence, backup and restore, and has been followed by a second person.

---

#### **M3-S6 · Weeks 11–12 · JetBrains plugin**

**Scope.** Member 3's share of the plugin programme (§8.4). Make the `veld`
binary path configurable in `VeldExternalAnnotator.kt` — replacing the bare
`ProcessBuilder("veld", "validate")` PATH assumption — with a clear diagnostic
when the binary is missing. Verify every action in `VeldActions.kt` works. Add a
test suite over the same fixture contract Member 1 uses in VS Code. Confirm the
JetBrains schema copy is covered by Member 2's CI gate.

**Exit criteria.**
1. The `veld` binary path is configurable in plugin settings; a missing binary produces an actionable message, not a silent absence of diagnostics.
2. Every action in `VeldActions.kt` is verified working in a real IDE session.
3. An automated test suite covers highlighting, completion and diagnostics on the shared fixture contract.
4. `PUBLISHING.md` has been followed end to end to produce a plugin build, by someone other than its author.

---

## 11. Member 4 — The Rust Track

Member 4 works one continuous track across weeks 1–12 with no two-week
subdivision, because Rust's compile-and-fix cycle on generated code does not
decompose cleanly into fixed-length units.

| Phase | Weeks | Work |
|-------|-------|------|
| Baseline | 1–2 | Own `rust`. Generate against the reference contract; record what compiles and what does not. |
| Specification review | 3–4 | Review Member 1's reference semantics specification and flag every clause Rust's ownership model makes expensive or impossible. **This feedback must reach Member 1 before week 4 ends.** |
| Parity, pass 1 | 5–8 | Serde structs, Axum handlers, the services trait. Drive parity cells green. |
| Parity, pass 2 + SDK | 9–12 | Close remaining cells. Verify the reqwest service SDK. Commit a worked example and the Rust target guide. |

**Exit criteria for the track.**
1. Every `rust` parity cell is pass or documented-gap.
2. Generated output compiles clean under `cargo build` in the harness.
3. A worked example — contract in, running Axum service out — is committed and reproducible.
4. The reqwest service SDK is verified against a live Tier 1 service.

**Escalation rule.** If at the end of **week 4** the Rust baseline does not
compile against the reference contract, Member 3 joins the Rust track for two
weeks and Member 3's S4 mobile-client task moves into buffer sprint B1. This is
decided at the week-4 review, not later, and it is the only pre-authorised
cross-member reassignment in the plan (risk **R7**, §16).

---

## 12. Member 5 — Documentation and Diagrams

Member 5 has the full 16 weeks and no fixed deadlines. The constraint is that
each workstream must be *complete* before the defence, not that it lands in a
particular week.

| Workstream | Content |
|------------|---------|
| **W1 — Architecture** | Pipeline diagram (`.veld → lexer → parser → AST → loader → validator → emitters → generated/`); the emitter registry and `init()` activation model; the marker-method interface hierarchy; the strategy pattern for framework variance; the workspace and `consumes` service-SDK model. |
| **W2 — Support matrix & parity** | Publish the Tier 1 / Tier 2 matrix (§3) and the parity conformance matrix as user-facing documentation. Every documented gap gets a plain-language explanation of what it means for a user. |
| **W3 — Target guides** | One guide per Tier 1 backend — `node-ts`, `python`, `go`, `java`, `rust` — contract in, running service out, built on the worked example each owner committed. |
| **W4 — Registry & operations** | The API reference and operator runbook produced in S5, edited into publishable form. Deployment diagram, authentication flow diagram, publish and pull sequence diagrams. |
| **W5 — Editor plugins** | Install and setup guide for both plugins; feature comparison table; the authoritative-grammar statement from M2-S6; screenshots of highlighting, completion and diagnostics in both editors. |
| **W6 — Defence materials** | Slide deck, live demonstration script, and the "what we deliberately did not build" section — which is where the Version 2 cancellations of §2.1 are explained as a scoping decision rather than a shortfall. |

**Standing constraint.** No document Member 5 produces may describe a capability
that does not exist in the repository, and no component may be presented as more
mature than it is. Every technical claim must be verifiable by a reader opening
the code. Where a component is partial, the documentation says so.

---

## 13. Critical Path

```
M1-S1 reference contract (wk 1-2)
        │
        ▼
M1-S2 reference semantics spec (wk 3-4)  ◄── M4 specification review feedback
        │
        ├──────────────┬──────────────┬──────────────┐
        ▼              ▼              ▼              ▼
   M1-S3 node-ts   M2-S3 py+go    M3-S3 java     M4 rust parity
   (wk 5-6)        (wk 5-6)       (wk 5-6)       (wk 5-12)
        │              │              │              │
        ▼              ▼              ▼              │
   M1-S4 frontends M2-S4 tools    M3-S4 mobile       │
   (wk 7-8)        (wk 7-8)       (wk 7-8)           │
        │              │              │              │
        └──────────────┴──────────────┴──────────────┘
                       │
                       ▼
        S5 REGISTRY PROGRAMME (wk 9-10) — M1 + M2 + M3
                       │
                       ▼
        S6 PLUGIN PROGRAMME + RELEASE (wk 11-12) — M1 + M2 + M3
                       │
                       ▼
                 B1 / B2 buffer (wk 13-16)
```

Two enabling systems sit off the critical path but gate everything after week 4:
Member 2's compile harness and Member 3's parity runner, both built in S2 in
parallel with Member 1's specification.

---

## 14. Definition of Done for v1.0

Veld v1.0 ships when **all** of the following hold. Each is objectively
checkable; none depends on judgement.

1. **Five Tier 1 backends** — `node-ts`, `python`, `go`, `java`, `rust` — each
   compile clean under the harness, pass every parity cell or carry a documented
   gap, ship a worked example, and ship a target guide.
2. **Three Tier 2 backends** — `node-js`, `csharp`, `php` — compile or lint
   clean in CI, and the support matrix states plainly that no parity is claimed.
3. **All ten frontends** compile or parse clean under their own toolchains.
4. **All six tool generators** pass their shape assertions.
5. **The registry meets §7.3** — tested handlers, versioned migrations, rate
   limiting, configured CORS, enforced upload cap, health and readiness
   endpoints, structured logging, API reference, operator runbook, and a passing
   end-to-end publish→pull test.
6. **Both editor plugins meet §8.3** — registered commands, single-source
   grammar and schema with CI drift detection, configurable binary resolution,
   test suites, reproducible packaging, no stale artefacts, and a setup guide.
7. **CI is green and gating** — compile harness and parity matrix both block
   merge; no emitter is exempt.
8. **The non-negotiable rules hold**, verified by inspection of generated
   output: zero runtime dependencies by default, framework-agnostic route
   handlers, native `fetch` in the TypeScript SDK.
9. **Member 5's six workstreams are complete** and every claim in them is
   verifiable in the repository.

---

## 15. Working Process

| Ritual | Cadence | Purpose |
|--------|---------|---------|
| Sprint planning | First Monday of each sprint | Confirm the two-week task and its exit criteria in writing before work starts. |
| Mid-sprint check | Wednesday of week 1 | Early warning only. A task in trouble is surfaced here, not at the boundary. |
| Sprint review | Last Friday of each sprint | Each member demonstrates their exit criteria against the reference contract. |
| Integration merge | Last Friday of each sprint | All work merges to `master` with CI green. No sprint ends with unmerged work. |
| Registry sync | Weekly, all 16 weeks | Member 3 as lead keeps the registry design coherent between concentrated bursts. |

**The sprint-boundary rule.** A task that does not meet its exit criteria on the
last Friday is **not silently extended**. It is re-planned in front of the whole
team, and the shortfall moves into a buffer sprint with a named owner and a new
written exit condition. Two-week tasks only have value if the two-week boundary
means something.

**Branch policy.** One branch per task, named for the task index — for example
`m1-s3-node-ts-parity`. Merged only with CI green. `master` is always
releasable.

---

## 16. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|-----------|--------|------------|
| **R1** | A toolchain is unavailable on a member's machine, so an emitter cannot be compile-verified. | High | High | Toolchain availability is confirmed in S1, before it can block S3. CI provides the authoritative environment. Missing toolchains skip with a recorded reason and never report a false pass. |
| **R2** | Go is not installed on the current development machine, so `go test ./...` has never been run locally. | Confirmed | Medium | Treated as an S1 blocker for every member. Until it is resolved, CI is the only source of truth for test results, and no document may claim tests pass without a CI run attached. |
| **R3** | The registry programme (§7) is larger than six person-weeks. | Medium | High | §7.3 is ordered by priority. Tests, migrations and the upload cap are mandatory; SPA polish is the first thing to move into buffer B1. |
| **R4** | The plugin programme (§8) is larger than six person-weeks. | Medium | Medium | Command registration, schema-drift CI and configurable binary resolution are mandatory. Test-suite breadth is the first thing to move into buffer B1. |
| **R5** | The reference semantics specification slips past week 4, blocking three members. | Medium | **Critical** | It is the plan's only single point of schedule failure. M1-S2 has no other deliverable competing with it. Member 4's review feedback is required before the week-4 boundary. If it slips, S3 becomes specification support for all members and one buffer sprint is consumed immediately. |
| **R6** | Freezing three backends to Tier 2 reads as a loss of capability. | Medium | Low | Nothing is deleted; all three still generate and still compile in CI. The support matrix states what is guaranteed rather than what exists, which is a more honest claim than Version 2 made. |
| **R7** | Rust proves harder than one member can carry. | Medium | Medium | Pre-authorised escalation at the week-4 review (§11): Member 3 joins for two weeks and M3-S4 moves to buffer B1. |
| **R8** | Parity turns out to be genuinely impossible for a target — Go's lack of struct inheritance is the known case. | High | Low | "Documented-gap" is a first-class cell state. A gap that is written down and explained is a feature of the support matrix, not a failure. |
| **R9** | Scope creep — a member adds "just one small emitter". | Medium | High | **P1** is absolute and applies to buffer weeks too (§17.2). Any new emitter proposal goes to the post-v1.0 list. |
| **R10** | Registry work concentrated in S5 collides across three members. | Medium | Medium | The three shares (§7.4) touch different files: Member 1 in `handlers/auth.go` and middleware, Member 2 in `db/` and `storage/`, Member 3 in `server.go`, `handlers/web.go` and the runbook. Member 3 as lead resolves any overlap. |
| **R11** | Plugin work concentrated in S6 collides across three members. | Low | Medium | The three shares touch different trees: Member 1 in `editors/vscode/`, Member 3 in `editors/jetbrains/`, Member 2 in `scripts/` and the shared spec generator. The one shared artefact is the fixture contract, agreed in S1. |
| **R12** | The buffer is consumed before week 13 by carry-over. | Medium | High | The sprint-boundary rule (§15) makes carry-over visible the moment it happens. If more than one task carries over before week 9, the Tier 1 set is reduced immediately by demoting `java` (see §2.2) rather than allowing the whole plan to slip. |

---

## 17. Buffer Policy

### 17.1 What the buffer is for

Weeks 13–16 are two two-week buffer sprints, allocated in priority order:

1. **Carry-over** from any task that missed its exit criteria (§15).
2. **Documented gaps** promoted to fixes where the fix is small and the gap is
   user-visible.
3. **Release candidate** — full clean-machine verification of every Tier 1
   target from a fresh clone.
4. **Defence rehearsal** — Member 5's demonstration script executed end to end
   by someone who did not write it.

### 17.2 What the buffer is not for

> **No new emitter, no new frontend, no new tool generator, no new framework
> strategy and no new contract keyword is written during weeks 13–16.** The
> buffer exists to make what is already promised true. If the team finishes
> early, the surplus goes into tests and documentation.

Ideas that arise during the project are recorded as post-v1.0 work rather than
absorbed. The list starts with, in priority order:

1. **Migrate the VS Code extension onto `internal/lsp/`**, deleting the
   1,676-line client-side re-implementation of a language server the project
   already owns. This is the largest single simplification available to the
   codebase and the clearest reason the four-grammar duplication of §8.2 exists.
2. The seven cancelled Version 2 emitter commitments (§2.1).
3. Promotion of `node-js`, `csharp` or `php` from Tier 2 to Tier 1.

---

## Appendix A — Sprint Task Index

| Index | Weeks | Owner | Task |
|-------|-------|-------|------|
| M1-S1 | 1–2 | Member 1 | Own the TypeScript surface; author the reference contract; baseline nine emitters |
| M1-S2 | 3–4 | Member 1 | **Reference semantics specification** *(hard dependency)* |
| M1-S3 | 5–6 | Member 1 | `node-ts` to full parity |
| M1-S4 | 7–8 | Member 1 | Seven web frontends to full parity; `node-js` Tier 2 gate |
| M1-S5 | 9–10 | Member 1 | Registry API contract and authentication surface |
| M1-S6 | 11–12 | Member 1 | VS Code extension |
| M2-S1 | 1–2 | Member 2 | Own dynamic/systems surface; baseline three backends and six tool generators |
| M2-S2 | 3–4 | Member 2 | Compile-verification harness |
| M2-S3 | 5–6 | Member 2 | `python` and `go` to full parity |
| M2-S4 | 7–8 | Member 2 | Tool-generator assertions; `php` Tier 2 gate |
| M2-S5 | 9–10 | Member 2 | Registry data layer, migrations and storage |
| M2-S6 | 11–12 | Member 2 | Shared grammar source-of-truth and schema-drift CI |
| M3-S1 | 1–2 | Member 3 | Own JVM/.NET/mobile surface; registry current-state assessment |
| M3-S2 | 3–4 | Member 3 | Parity conformance matrix |
| M3-S3 | 5–6 | Member 3 | `java` to full parity |
| M3-S4 | 7–8 | Member 3 | Mobile clients to parity; `csharp` Tier 2 gate |
| M3-S5 | 9–10 | Member 3 | Registry UI, operations and integration |
| M3-S6 | 11–12 | Member 3 | JetBrains plugin |
| M4 | 1–12 | Member 4 | Rust backend and Rust service SDK — continuous track |
| M5 | 1–16 | Member 5 | Six documentation and diagram workstreams — continuous |

---

## Appendix B — Ownership Quick Reference

| Component | Owner |
|-----------|-------|
| `internal/emitter/backend/node/` (`node-ts`) | Member 1 |
| `internal/emitter/backend/javascript/` (`node-js`) | Member 1 |
| `internal/emitter/frontend/` — `typescript`, `javascript`, `react`, `vue`, `svelte`, `angular`, `typesonly` | Member 1 |
| `internal/emitter/tshelpers/` | Member 1 |
| `internal/docsgen/`, `internal/graphqlgen/`, `internal/schema/` | Member 1 |
| `editors/vscode/` | Member 1 |
| `internal/emitter/backend/python/` | Member 2 |
| `internal/emitter/backend/go/` | Member 2 |
| `internal/emitter/backend/php/` | Member 2 |
| `internal/generators/` — all six tool generators | Member 2 |
| `internal/openapigen/` | Member 2 |
| `scripts/sync-schema.sh` and the shared language-spec generator | Member 2 |
| Compile-verification harness | Member 2 |
| `internal/emitter/backend/java/` | Member 3 |
| `internal/emitter/backend/csharp/` | Member 3 |
| `internal/emitter/frontend/` — `kotlin`, `swift`, `dart` | Member 3 |
| `internal/server/` (registry) — **programme lead** | Member 3 |
| `editors/jetbrains/` | Member 3 |
| Parity conformance matrix | Member 3 |
| `internal/emitter/backend/rust/` | Member 4 |
| `docs/` — all documentation and diagrams | Member 5 |

---

## Appendix C — Version History

| Version | Approach | Backends promised | Status |
|---------|----------|-------------------|--------|
| **v1** | Role-based division — parser, emitters, registry, docs | 8 registered, no tier system | Superseded — `docs/EXECUTION_PLAN_v1.md` |
| **v2** | Type-system-family division, expansion-oriented | 8 existing + Ruby + Kotlin backend | Superseded — `docs/EXECUTION_PLAN_v2.md` |
| **v3** | Type-system-family division, consolidation-oriented | 4 Tier 1 for Members 1–3 + `rust` for Member 4; 3 frozen at Tier 2 | **Current** |

---

*End of Execution Plan, Version 3.*
