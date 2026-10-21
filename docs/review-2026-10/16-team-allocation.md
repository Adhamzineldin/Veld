# 16. Team allocation: T1 to T5 and D1

No team member's skills are known to this review. Every assignment below is a planning scenario to be adjusted after the skills survey in 16.1, which must be completed in Sprint 0 (by 2026-10-30). Hours are the baseline model from chapter 15 §15.1 (12 h/week technical, 8 h/week D1, exam-adjusted, 15% coordination, 20% reserve held).

## 16.1 Skills survey (10 minutes per person)

Rate 0 (none) to 3 (can teach it): Go; TypeScript/Node; Python/Flask; React; PostgreSQL and SQL; Docker and Linux operations; HTTP API design; compilers or parsers (any coursework); VS Code extension or JetBrains plugin development; GitHub Actions; security basics (auth, hashing, input validation); technical writing; diagramming; statistics for evaluation. Also: preferred working days, exam dates for both semesters, any planned absence, and the one subsystem each person most wants to own. The lead (T1) re-balances the matrix in 16.3 using the answers; D1 records the result as a decision.

## 16.2 Capacity (baseline, to readiness target)

| Member | Role | Committed h | Reserve h | Notes |
|---|---|---|---|---|
| T1 | compiler core, lead architect | 221 | 55 | also chairs ADR decisions and the migration decision |
| T2 | emission host, output safety, Node/TS emitters, plugin host | 221 | 55 | |
| T3 | registry, package resolution, security, operations | 221 | 55 | |
| T4 | tier-1 Python/Go emitters, test harness, plugin SDK and reference plugins, evaluation | 221 | 55 | |
| T5 | LSP, VS Code, CLI/UX, distribution, release, CI | 221 | 55 | |
| D1 | documentation, diagrams, ADR editing, report, slides, study coordination | 148 | 37 | no production code; no sole technical validation |

June 2027 (submission and defense) is budgeted separately: assume 20 h per technical member and 40 h for D1 for corrections, rehearsal and defense; these are not available for implementation.

## 16.3 Component ownership matrix

| Component | Primary | Backup | Reviewer (never the author) | Validates D1's docs on this component |
|---|---|---|---|---|
| Language spec and conformance fixtures | T1 | T4 | T2 | T1 |
| Syntax (lexer, parser, AST) and diagnostics | T1 | T5 | T2 | T1 |
| Semantic analysis, IR, `veld ir` | T1 | T2 | T4 | T1 |
| Compiler API and config | T1 | T5 | T3 | T5 |
| Emission host, manifest, staged writer | T2 | T4 | T1 | T2 |
| Node/TS/React emitters and TS client | T2 | T4 | T5 | T2 |
| Python and Go emitters and clients | T4 | T2 | T1 | T4 |
| Generated-code test harness and CI compile matrix | T4 | T5 | T2 | T4 |
| VEP protocol and plugin host | T2 | T1 | T3 (security) | T2 |
| TypeScript emitter SDK, hello emitter, docs emitter extraction | T4 | T2 | T5 | T4 |
| Language server | T5 | T1 | T4 | T5 |
| VS Code extension | T5 | T4 | T2 | T5 |
| JetBrains (post-Beta, if funded) | T5 | T1 | T4 | T5 |
| CLI UX, exit codes, `--json`, doctor | T5 | T2 | T1 | T5 |
| Registry client, tarball, manifest, lockfile, resolver, cache | T3 | T1 | T2 | T3 |
| Registry server, DB, storage, authz, migrations | T3 | T5 | T1 (security review) | T3 |
| Self-hosting: compose, backup/restore, runbook | T3 | T5 | T4 | T3 |
| Distribution wrappers, GoReleaser, signing, provenance | T5 | T3 | T1 | T5 |
| CI pipelines and repository hygiene | T5 | T4 | T3 | T5 |
| OpenAPI and tier-2 tools | T2 | T4 | T1 | T2 |
| Evaluation harness and study tooling | T4 | T1 | T3 | T4 |
| Website and docs site build | T5 | D1 (content only) | T2 | T5 |
| Documentation, diagrams, ADR editing, report, slides, demo script | D1 | T1 (content owner) | per-component validator above | n/a |

Uneven allocation explained: T1 and T2 carry the critical path through M2 and M3; T3 runs a parallel stream with few dependencies; T4 and T5 are loaded later (M4, M5) when SDK, VS Code and evaluation work opens up. Integration ownership rotates every sprint (T1, T2, T3, T4, T5, repeat) so that no one person is the permanent integrator or release manager.

## 16.4 Per-member plan

### T1: compiler core (primary), diagnostics and LSP backend (secondary)

- Skills to learn: compiler engineering patterns (error recovery, spans), JSON Schema, writing conformance fixtures.
- Monthly deliverables: Oct/Nov ADRs accepted, lexer fixes, header identifiers, loader strictness; Dec `diag`, `TypeRef`, IR v0; Jan IR v1 validated on all fixtures (exam month: only review and fixture writing in S6); Feb diff on IR and gate; Mar validator extensions, formatter start; Apr formatter, evaluation Q1 support; May demo lead, report chapters 4 and 5 technical review.
- Expected effort: 221 h; heaviest in S3 to S5 (16 h per sprint committed).
- Dependencies: none upstream; everyone depends on IR by S5.
- Review obligations: emission host (T2), harness (T4), registry security review (T3).

### T2: emission host and Node/TS emitters (primary), plugin host and OpenAPI (secondary)

- Skills to learn: TypeScript strict mode and `tsc` tooling, Express/Fastify request models, process management in Go.
- Monthly deliverables: Nov staged writer and manifest, gate ordering, path safety, Node fixes, e2e round trip; Dec emitters on `diag` and IR, tier-3 removals; Jan VEP schema frozen, host runner skeleton with stub plugin; Feb/Mar host runner complete, `--describe`, docs emitter extraction with T4, OpenAPI completeness; Apr bug fixes, determinism checks; May release hardening.
- Expected effort: 221 h; heaviest in S1 to S2 and S8 to S10.
- Dependencies: IR (T1) from S5; SDK (T4) from S8.
- Review obligations: syntax and IR (T1), VS Code (T5), lockfile (T3).

### T3: registry, package resolution, security and operations (primary), server-side tests (secondary)

- Skills to learn: PostgreSQL migrations and transactions, Docker Compose, threat modelling, Go `httptest`.
- Monthly deliverables: Nov tarball hardening, checksum-required pull, `Pack` filtering, `DefaultRegistry` fix, authz matrix tests; Dec split binary, atomic publish, size limits, versioned migrations, CI PostgreSQL job; Jan ADR-008 design frozen (exam month); Feb/Mar manifest, lockfile, resolver, cache, scoped registries; Apr compose deployment, backup/restore drill, runbook, security self-review; May support for the demo registry.
- Expected effort: 221 h.
- Dependencies: few; IR not needed. Depends on T5 for release packaging of `veld-registry`.
- Review obligations: compiler API (T1), VEP security (T2), CI hygiene (T5).

### T4: Python/Go emitters, test harness, plugin SDK and reference plugins, evaluation (primary)

- Skills to learn: Flask, Go net/http, GitHub Actions matrices, npm package publishing, basic experiment design.
- Monthly deliverables: Nov Python and Go compile fixes, harness v0, compile jobs, Flask and Go round trips; Dec validation default ON, determinism test, 30 fixtures; Jan fixtures and expected IR (exam month); Feb/Mar TypeScript SDK, hello emitter on npm, docs emitter extraction, `npm create` scaffold; Apr evaluation runs Q1 to Q3, user study execution with D1; May evaluation write-up support.
- Expected effort: 221 h; heaviest S8 to S12.
- Dependencies: IR schema (T1) for SDK types; host (T2) for plugin runs.
- Review obligations: IR (T1), emission host (T2), ops runbook (T3), LSP (T5).

### T5: language server, VS Code, CLI/UX, distribution, release, CI (primary), config (secondary)

- Skills to learn: LSP 3.18 details, `vscode-languageclient`, GoReleaser, cosign and GitHub attestations.
- Monthly deliverables: Nov hygiene PR, `go 1.26`, CI compile jobs and OS matrix, config precedence, `doctor`; Dec LSP on compiler API, protocol fixes, UTF-16, imports; Jan GoReleaser with checksums (exam month); Feb/Mar VS Code thin client with discovery and download, completion/symbols/formatting, `--json` and exit codes, npm/pip/brew wrappers; Apr website CI, JetBrains LSP4IJ only if reserve allows; May release 1.0.0, signed artifacts.
- Expected effort: 221 h.
- Dependencies: `diag` (T1) by S4; compiler API by S5.
- Review obligations: Node/TS emitters (T2), SDK (T4), registry server (T3, non-security aspects).

### D1: documentation, diagrams, research organization, report, presentation

- Recurring work every sprint: update current/target diagrams (D1 to D8, with the technical owner named on each); requirements traceability matrix (requirement -> ADR -> backlog item -> test); ADR editing after decisions; installation and tutorial documentation (validated by the component owner); API and plugin documentation (validated by T2/T4); meeting decisions log; evaluation evidence folder; report chapters; slides; demo script.
- Monthly deliverables: Oct/Nov skills survey, ADR set published, tutorial for Journey A, report chapter 1 draft; Dec spec pages, safe-generation and support-matrix pages, chapter 2 and 3 drafts; Jan plugin author guide draft (from the frozen spec), chapter 4 draft; Feb/Mar user-study protocol and consent, recruitment with T4, chapters 5 and 6; Apr study sessions, operator runbook editing, chapters 7 to 9; May final report, slides, demo video script, defense rehearsal coordination.
- Validation rule: every technical statement in the docs and the report cites a test, a CI run, a command output or a finding ID, and is signed off by the validator named in 16.3. D1 may coordinate usability feedback but T4 runs the technical part of the study.

## 16.5 Pairing and knowledge-transfer checkpoints

| When | Pair | Purpose |
|---|---|---|
| S1 | T1 with T2 | writer API and where the gate sits in the pipeline |
| S2 | T3 with T5 | CI PostgreSQL job and release packaging of the registry |
| S4 | T1 with T5 | `diag` consumption in the LSP |
| S5 | T1 with T4 | IR schema to SDK type generation |
| S8 | T2 with T4 | plugin host and SDK end to end |
| S9 | T3 with T1 | lockfile and resolver review |
| S11 | T4 with D1 | user-study protocol dry run |
| S12 | T5 with T3 | compose deployment on a clean VM |
| every sprint review | integration owner demonstrates the vertical slice from a clean clone | shared understanding; catches silent breakage |

Each pairing produces a one-page note in `docs/notes/` written by the learner and checked by the owner, so that ownership is transferable.

## 16.6 Sprint responsibility table

| Sprint / dates | Shared milestone | T1 | T2 | T3 | T4 | T5 | D1 | Integration / acceptance checkpoint |
|---|---|---|---|---|---|---|---|---|
| S0 Oct 21 to Nov 1 | M0 decisions | ADR workshop; fixture skeleton | writer design; reproduce F-NODE-01..04 | tarball reproduction; unpack design | reproduce Python/Go failures; harness layout | hygiene PR; Go 1.26; governance files | survey; ADR editing; notes system | Oct 30: ADRs accepted; hygiene merged (owner T1) |
| S1 Nov 2 to 15 | M1 slice | wildcard import, unterminated strings, BOM, header identifiers | staged writer, manifest, gate ordering, path safety, dry-run, clean | checksum-required pull, `Pack` filtering, `DefaultRegistry`, archive tests | Python/Go compile fixes, compile jobs, harness v0 | CI compile jobs, windows runner, config precedence | tutorial A, diagram D1 | Nov 13: `--strict` gate demo; compile status visible in CI (owner T2) |
| S2 Nov 16 to 29 | M1 slice | loader strictness, aliases | e2e Express round trip; tier-3 removal PR | authz matrix tests; PostgreSQL CI job | Flask and Go round trips; determinism test | GoReleaser skeleton; version from build info; doctor | README tiers draft; chapter 1 draft | Nov 27: Journey A in CI on two OSes; **Nov 29 migration decision** (owner T3) |
| S3 Nov 30 to Dec 13 | M2 core | `diag`, spans, error recovery | emitters on `diag`; `OutputFile` API | registry split binary; atomic publish | validator extensions; 15 fixtures | LSP structured diagnostics; recover; MethodNotFound | spec draft; diagram D2 | Dec 11: all-errors output; IR v0 for one fixture (owner T4) |
| S4 Dec 14 to 27 (75%) | M2 core | `TypeRef`, nested generics | IR lowering v0 | size limits; versioned migrations | fixtures to 30 | LSP imports with overlay; UTF-16 | safe-generation page; chapter 2 draft | Dec 23: LSP multi-file session passes (owner T5) |
| S5 Dec 28 to Jan 10 (65%) | M2 exit | IR v1 + schema + `veld ir`; diff on IR | tier-1 emitters on IR | registry e2e in CI | validation default ON | compiler API in LSP; CI OS matrix | chapter 3 draft | Jan 8: M2 exit review; IR validates all fixtures (owner T1) |
| S6 Jan 11 to 24 (30%) | M3 freeze | review only | VEP schema draft | ADR-008 design | fixture review | GoReleaser checksums | plugin guide outline | Jan 22: VEP schema reviewed (owner T2) |
| S7 Jan 25 to Feb 7 (70%) | M3 freeze | gate tests | host runner skeleton; stub plugin tests | lockfile format; resolver design doc | SDK type generation from schema | VS Code client skeleton | chapter 4 draft | Feb 5: stub plugin handshake, escape refusal, timeout (owner T3) |
| S8 Feb 8 to 21 | M4 extend | validator extensions cont. | host runner complete; `--describe`; capabilities | manifest, lockfile, resolver | TypeScript SDK; test harness | VS Code client; binary discovery | plugin author guide | Feb 19: SDK runs hello against host locally (owner T4) |
| S9 Feb 22 to Mar 7 | M4 extend | formatter design | docs emitter extraction (with T4) | cache, offline, scoped registries | hello emitter on npm; `npm create` | completion, symbols, formatting in LSP | chapter 5 draft | Mar 5: `veld install --frozen` reproducible (owner T5) |
| S10 Mar 8 to 21 | M4 Alpha | diff classifier complete | OpenAPI completeness; determinism check command | registry authz hardening leftovers | extracted docs emitter acceptance | `--json`, exit codes, wrappers fixed | chapter 6 draft; study protocol | **Mar 19 Alpha gate**: plugin acceptance on two OSes (owner T1) |
| S11 Mar 22 to Apr 4 (93%) | M5 Beta prep | formatter rewrite | bug fixes from alpha | compose deployment; backup/restore drill | evaluation Q1, Q2 runs | website CI; docs site | study recruitment; runbook editing | Apr 2: restore drill timed (owner T2) |
| S12 Apr 5 to 18 (82%) | M5 Beta | bug fixes | bug fixes | runbook final; security self-review | user study sessions; Q3 run | JetBrains LSP4IJ if reserve allows | study sessions; chapters 7 and 8 | **Apr 16 Beta gate = feature freeze** (owner T3) |
| S13 Apr 19 to May 2 | M6 harden | P0/P1 fixes | P0/P1 fixes | P0/P1 fixes | evaluation analysis | signed release pipeline | chapter 9; evaluation chapter | Apr 30: demo rehearsal 1 (owner T4) |
| S14 May 3 to 16 | M6 RC | report technical review | RC fixes | RC fixes | RC fixes | RC tagged | report draft complete | **May 14 RC gate** (owner T5) |
| S15 May 17 to 30 (55%) | M7 final | demo lead | fixes only | demo registry | fixes only | release 1.0.0 | final report, slides, video | **May 28 readiness review; May 31 gate** (owner T1) |

Amendment 2026-10-21 (ADR-012): the queue items Q1 to Q14 are added to S8 to S13 in `csv/sprint-allocation.csv` and chapter 20 §20.3; per-member additions and the resulting over-allocation (T1 +190 h, T2 +225 h, T3 +180 h, T4 +204 h, T5 +122 h at the midpoint) are in chapter 20 §20.4. T5 is the first backup for T2 and T4 queue items. All AI agents and contributors work under `AGENTS.md`.

Capacity check: each cell is sized to the committed hours of that sprint (chapter 15 §15.1); exam sprints S6 and S15 contain review, freeze and release tasks only. Parallel tasks rest on agreed interfaces: the writer API (S1), `diag` (S3), IR schema (S5), VEP schema (S6), lockfile format (S7). Where a downstream task starts before its interface is final, a provisional contract is named in the backlog (chapter 18).
