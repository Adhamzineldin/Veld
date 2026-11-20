# 15. Dated roadmap to June 2027, dependencies, capacity and contingency

Planning baseline 2026-10-21 (Wednesday). Conservative readiness target 2027-05-31 (Monday). June 2027 is reserved for submission, defense and corrections and is budgeted separately (chapter 16). Calendar computed by `scratchpad/plan/calendar.py` (reproduced in the appendix of chapter 16): 222 days, 31.7 weeks, one 12-day Sprint 0 plus fifteen 14-day sprints ending 2027-05-30. If work starts later than the baseline, drop sprints from the front of the plan and apply the cut list in 15.6; do not move the readiness target.

## 15.1 Capacity assumptions (to validate in Sprint 0)

| Parameter | Baseline | Low | High |
|---|---|---|---|
| Hours per week, technical member | 12 | 8 | 16 |
| Hours per week, D1 | 8 | 6 | 10 |
| Exam and holiday reductions (assumed; confirm with the academic calendar) | winter break Dec 21 to Jan 3 at 50%; fall finals Jan 9 to Jan 30 at 30%; spring midterms Apr 3 to 9 at 50%; spring finals from May 22 at 30% | same | same |
| Coordination, reviews, meetings | 15% of exam-adjusted hours | same | same |
| Contingency reserve (held, not allocated) | 20% of productive hours | same | same |
| Committed hours per technical member to readiness | 221 | 148 | 295 |
| Committed hours, five technical members | 1,106 | 738 | 1,475 |
| Reserve, five technical members | 277 | 184 | 369 |
| Committed hours, D1 | 148 | 111 | 184 |

Committed hours per technical member per sprint (baseline): S0 14, S1 16, S2 16, S3 16, S4 12, S5 11, S6 5, S7 11, S8 16, S9 16, S10 16, S11 15, S12 13, S13 16, S14 16, S15 9.

## 15.2 Budget check: planned work against committed capacity

| Workstream | Chapter | Estimate (h) | Midpoint |
|---|---|---|---|
| Output writer, manifest, path safety, dry-run, clean, init, watch | 08 §8.8, ADR-007 | 20 to 30 | 25 |
| Compiler frontend fixes, diagnostics, TypeRef, IR v1, conformance suite, validator, loader, language tables, diff, config, CLI, formatter | 08 §8.8 | 160 to 267 | 213 |
| Tier-1 emitter fixes (Node, TS, Python, Go), harness, tier-2 nightly job, removals | 09 | 135 to 215 | 175 |
| External emitter system (IR schema share, host, SDK, hello, extracted docs emitter) | 10 §10.6 | 114 to 192 | 153 |
| Language server and VS Code client | 11 §11.3 | 75 to 135 | 105 |
| Registry hardening, separate binary, package manifest/lockfile/resolver, compose, backup/restore | 12, 13 | 110 to 180 | 145 |
| Distribution, release automation, CI matrix, governance files | 12, 14 | 40 to 60 | 50 |
| Evaluation harness and user-study tooling | 03 §3.9, 17 | 40 to 60 | 50 |
| Total technical | | 694 to 1,139 | 916 |

The midpoint (916 h) fits the committed capacity (1,106 h) with the reserve (277 h) untouched; the high estimate (1,139 h) consumes 33 h of reserve. Under the low-capacity scenario (738 h committed) the cut list in 15.6 must be applied in Sprint 0. JetBrains LSP4IJ work (20 to 40 h) and every stretch item are outside this budget and are funded only from unused reserve after the Beta gate.

## 15.3 Milestones

| Milestone | Sprints / dates | Deliverables | Owners (primary) | Depends on | Exit criteria (measurable) | Effort (h, technical) | Cut if it slips |
|---|---|---|---|---|---|---|---|
| M0 Discovery and decisions | S0: Oct 21 to Nov 1 | ADR-001..010 accepted or amended; skills survey done; repository hygiene PR (binary, scratch dirs, `.idea`, docx, stale `out/`, `test.md` removed; `go 1.26`; LICENSE, CODEOWNERS, CHANGELOG); CI on Go 1.26; backlog imported into the tracker | T1 (ADRs), T5 (hygiene, CI), D1 (survey, ADR editing) | none | ADR status "accepted" with deciders' sign-off; `git ls-files | wc -c` under 5 MB; CI green on 1.26 | 50 to 70 | nothing; this milestone is the minimum |
| M1 Stabilize and vertical slice | S1 to S2: Nov 2 to Nov 29 | staged writer and manifest; gate ordering fixed; path safety; wildcard import, unterminated strings, header identifiers fixed; Python syntax and `Decimal`, Go `mux`, Node route ordering, 500 leak, path encoding, strict TS; tarball hardening and checksum-required pull; generated-code compile jobs for node-ts, python, go; e2e harness v0 | T2 (writer), T1 (parser fixes), T4 (emitters, harness), T3 (tarball, pull), T5 (CI jobs) | M0 | Journey A demo (chapter 03) runs from a clean clone in CI on ubuntu and windows; `veld generate --strict` fails on a removed action; `tsc --strict`, `compileall`, `go build` green for the three tier-1 backends on every fixture; malicious archive tests pass | 150 to 200 | tier-2 compile job; Go round-trip (keep compile only) |
| M2 Core: IR, diagnostics, LSP engine | S3 to S5: Nov 30 to Jan 10 | structured diagnostics and spans; `TypeRef`; IR v1 with JSON Schema and `veld ir`; conformance suite (30+ fixtures); tier-1 emitters on IR; validation default ON; tier-3 code removed; LSP on compiler API with imports and UTF-16; registry split binary, atomic publish, limits, authz tests; CI OS matrix; GoReleaser with checksums | T1 (IR, diagnostics), T2 (emitters on IR), T5 (LSP, release), T3 (registry), T4 (fixtures, validation) | M1 | `veld ir` output validates against the schema for every fixture; byte-identical generation on two OSes; LSP scripted session passes incl. multi-file; registry e2e test in CI with a PostgreSQL service; `diff` classifies enum removal and status change as breaking | 220 to 300 | validation depth (keep enum and format checks, defer coercion); registry authz test breadth |
| Migration decision deadline | Nov 29 | written confirmation that no rewrite spike is pursued (ADR-001) | T1 | M1 | ADR-001 accepted | 0 | n/a |
| M3 Exams: protocol freeze | S6 to S7: Jan 11 to Feb 7 (30 to 70% capacity) | VEP v1 specification frozen (ADR-003 accepted with schema); host runner skeleton with a stub plugin; package manifest and lockfile design finalized (ADR-008); docs: spec, tutorial drafts; report chapters 1 to 3 drafted | T2 (VEP), T3 (package design), D1 (docs, report) | M2 | protocol schema in repo with tests; stub plugin passes handshake, path-escape refusal, timeout kill | 60 to 90 | host runner moves to S8 |
| M4 Extend and harden | S8 to S10: Feb 8 to Mar 21 | TypeScript SDK, hello emitter published to npm (scoped test package), docs emitter extracted; VS Code thin client with binary discovery; completion, symbols, formatting in LSP; lockfile, resolver, cache, scoped registries; npm/pip/brew wrappers fixed; CLI `--json`, exit codes, doctor; OpenAPI completeness | T4 (SDK, plugins), T5 (VS Code, CLI, wrappers), T3 (pkg), T2 (host, OpenAPI), T1 (diff, formatter start) | M3 | **Alpha gate Mar 21**: external plugin acceptance test (chapter 10 §10.7) passes on two OSes; `veld install --frozen` reproducible; VS Code extension integration test green | 230 to 310 | docs-emitter extraction (keep hello only); formatter rewrite; OpenAPI 3.1 |
| M5 Alpha to Beta | S11 to S12: Mar 22 to Apr 18 | Docker Compose deployment, backup/restore runbook executed and timed; evaluation harness runs (Q1 to Q3); user study (6 to 10 participants); website and docs site updated to the real support matrix; JetBrains LSP4IJ only if reserve allows | T3 (ops), T4 (evaluation), D1 (study coordination, docs), T5 (website CI) | M4 | **Beta gate = feature freeze Apr 18**: tier-1 round trip green on three OSes for two consecutive sprints; restore drill meets RPO/RTO targets (chapter 13); evaluation data collected | 120 to 170 | JetBrains; expressiveness corpus reduced to 8 documents |
| M6 Hardening and evaluation | S13 to S14: Apr 19 to May 16 | bug fixes only; release candidate; security self-review (chapter 14 checklist); report chapters 4 to 9 complete with evidence; demo rehearsal 1; signed release artifacts | all T (fixes), D1 (report), T3 (security review) | M5 | **RC gate May 16**: zero open P0/P1; `veld doctor` matrix matches README; report draft reviewed by the supervisor | 100 to 140 | website polish; Open VSX |
| M7 Final | S15: May 17 to May 30 (55% capacity) | release 1.0.0 tagged and published (GitHub, npm, brew tap); final report; slides; demo video (offline fallback); defense rehearsal 2 | T5 (release), D1 (report, slides), T1 (demo) | M6 | **Readiness gate May 31**: all artifacts in the submission folder; demo runs offline from the recorded video and from a clean VM | 40 to 60 | nothing; only bug fixes |
| June 2027 | Jun 1 to Jun 30 | submission, defense, corrections | all | M7 | defense delivered | budgeted separately (chapter 16) | n/a |

Sum of milestone effort midpoints: about 1,000 h, consistent with 15.2 once overlaps (CI jobs counted in both M1 and M2) are removed.

## 15.4 Week-by-week plan for the first eight weeks (Oct 21 to Dec 15)

| Week | Dates | T1 | T2 | T3 | T4 | T5 | D1 | Checkpoint (Friday) |
|---|---|---|---|---|---|---|---|---|
| 1 | Oct 21 to 27 | read review; draft ADR acceptance comments; set up conformance fixture skeleton | reproduce F-NODE-01..04 locally; design staged writer API | reproduce tarball cases (F-REG); design hardened unpack | reproduce Python/Go compile failures; design e2e harness layout | hygiene PR (binary, scratch dirs, docx, `.idea`, `out/`); `go 1.26` in go.mod and CI | skills survey sent; ADR template; meeting notes system | Oct 23: kickoff; survey returned |
| 2 | Oct 28 to Nov 3 | ADR workshop lead; accept ADR-001, 002, 009 | ADR-003, 007 accepted; writer implementation starts | ADR-004, 008 accepted; unpack limits coded | ADR-005 accepted; harness v0 for node-ts | ADR-006, 010 accepted; LICENSE, CODEOWNERS, CHANGELOG, templates | ADRs edited and published; backlog imported | Oct 30: M0 exit review; Nov 1 Sprint 0 ends |
| 3 | Nov 4 to 10 | wildcard import token; unterminated string/comment errors; BOM | staged writer + manifest in `veld generate`; gate ordering fix | checksum-required pull; `DefaultRegistry` fix; `Pack` filtering | Python syntax error, `Decimal` import; Go `mux`; compile jobs | CI: generate every example + compile tier-1; windows runner | tutorial draft for Journey A; diagram D1 current-state | Nov 6: writer merged; CI shows the real compile status |
| 4 | Nov 11 to 17 | header identifier derivation (parser + lang helpers) | path safety; dry-run; clean semantics; init O_EXCL | malicious-archive tests in `internal/pkg`; server authz matrix tests start | Node route ordering, 500 leak, path encoding, strict TS; React on fixed SDK | watch on compile result; SIGTERM; config precedence | docs: safe-generation page; CLAUDE.md corrections list | Nov 13: `--strict` gate demo; Sprint 1 review Nov 15 |
| 5 | Nov 18 to 24 | loader strictness; aliases in validate/ast | e2e harness: Express server + TS client round trip in CI | registry publish atomicity design; PostgreSQL service in CI | Python Flask round trip; Go round trip (compile + run) | GoReleaser skeleton with checksums; version from build info | README support matrix rewritten to tiers (draft) | Nov 20: Journey A runs in CI |
| 6 | Nov 25 to Dec 1 | diagnostics design (`diag`), span plumbing starts | tier-3 removals PR (rust, angular, dart, kotlin, swift, mock, dockerfile, cicd, envconfig) | registry split binary PR | determinism test (two OSes); validation default ON | CI OS matrix; `veld doctor` registered | report chapter 1 draft; evaluation protocol draft | Nov 27: M1 exit review; **Nov 29 migration decision deadline** |
| 7 | Dec 2 to 8 | error recovery; all-errors reporting | emitters consume `diag`; `OutputFile` API finalized | atomic publish + failed-upload cleanup; size limits | validator extensions (path params, defaults, unions) | LSP: structured diagnostics into server; `recover`, MethodNotFound | spec draft: syntax and semantics pages | Dec 4: all-errors output demo |
| 8 | Dec 9 to 15 | `TypeRef` tree; nested generics | IR lowering v0 (models, enums) | authz tests complete; versioned migrations | fixtures: 15 of 30 written with expected diagnostics | LSP imports via loader with overlay; UTF-16 helper | diagram D2 proposed pipeline; glossary | Dec 11: Sprint 3 review; IR v0 JSON for one fixture |

## 15.5 Critical path and major risks

Critical path: M1 writer and parser fixes (T2, T1) -> M2 diagnostics and IR (T1) -> M3 VEP freeze and host skeleton (T2) -> M4 SDK, hello and extracted emitter (T4) -> M5 evaluation and user study (T4, D1) -> M6 RC -> M7 release. The LSP (T5) joins the critical path at M2 (needs diagnostics) and M4 (VS Code client). The registry stream (T3) is parallel and joins at M5 (self-hosting drill).

| Risk | Likelihood | Impact | Mitigation | Trigger to act |
|---|---|---|---|---|
| IR migration takes longer than S5 | medium | plugin SDK and diff slip | AST-to-IR adapter lets plugins start on the adapter output; built-ins stay on AST until M4 | IR not validating all fixtures by Jan 10 |
| Exam period (S6) removes more capacity than assumed | high | M3 slips | M3 contains only freeze-and-document work; host runner has slack until S8 | survey shows exams differ from assumptions |
| One technical member unavailable | medium | their workstream stalls | backup owners (chapter 16) pre-assigned with pairing from S1; scope cut per 15.6 | two consecutive sprints without commits from one member |
| Registry security work expands (audit pending) | medium | T3 overloaded | scope the registry to the graduation list in chapter 13; defer search and audit UI | F-REG items with security severity high exceed 40 h |
| Plugin extraction of the docs emitter too costly | low to medium | acceptance test weaker | hello emitter alone satisfies the thesis; docs emitter becomes stretch | not started by Mar 7 |
| User study recruitment fails | medium | evaluation chapter weak | fallback: within-faculty participants; minimum 4; report the limitation | fewer than 4 confirmed by Mar 21 |
| CI toolchains for tier-2 flaky | medium | noise | tier-2 is nightly and non-blocking | n/a |

## 15.6 Fallback plans

- Substantially lower capacity (8 h/week): drop JetBrains entirely; drop the docs-emitter extraction (hello only); drop the formatter rewrite (keep `fmt --check` disabled); limit the package resolver to exact versions (no ranges, no transitive resolution); user study with 4 participants; tier-2 emitters compile-checked but not fixed beyond the header bug. Saves roughly 200 to 260 h.
- A technical member becomes unavailable: the backup owner takes the stream at reduced scope: T1 lost -> IR limited to what plugins need, validator extensions deferred; T2 lost -> plugin host minimal (no `--describe` capabilities warnings), emitters stay on AST adapter; T3 lost -> registry frozen at hardened tarball, checksum-required pull, compose deployment, no lockfile resolver; T4 lost -> harness limited to Node and Python, evaluation Q3 dropped; T5 lost -> VS Code client minimal (no download), release manual with checksums only.
- Failed rewrite spike: not applicable, no rewrite is planned (ADR-001). If the team nevertheless starts one, it must prove viability (lexer, parser, validator passing the conformance suite) by Nov 29 or be abandoned.
- Plugin approach proves too expensive: ship the protocol specification, the host runner and the hello emitter (minimum for the thesis); defer the SDK scaffolding (`npm create`) and the extracted emitter. If even the host runner cannot be completed by the Alpha gate, present the protocol as a validated design with the stub plugin tests and state the limitation; this is the only scenario in which chapter 10 is partially delivered.

## 15.6a Amendment 2026-10-21 (ADR-012): extended scope

Chapter 20 adds an ordered queue of fourteen items (Q1 to Q14) behind the core, starting in S8, with an extended-feature freeze on 2027-05-02 and an automatic cut order. Under the AI-assisted multiplier of 1.3 the planned work becomes 1,251 to 2,169 hours against 1,438 equivalent committed hours plus 360 reserve; the midpoint leaves 88 hours of reserve and the high estimate is 371 hours short, which is why the cut is automatic. The milestone table above remains valid for the core; the queue's sprint placement is in chapter 20 §20.3 and in `csv/sprint-allocation.csv`. The productivity multiplier is measured at the M1 exit (B-077) and the queue is truncated per chapter 20 §20.6 if it is below 1.15.

## 15.7 Gates summary

| Gate | Date | Pass criteria | Decider |
|---|---|---|---|
| M0 exit | Nov 1 | ADRs accepted; hygiene merged | team |
| Migration decision | Nov 29 | ADR-001 accepted; no rewrite | T1 with supervisor |
| M1 exit | Nov 29 | vertical slice in CI; gate works | T2, T4 |
| M2 exit | Jan 10 | IR validates; LSP multi-file; registry e2e | T1, T5, T3 |
| Alpha | Mar 21 | plugin acceptance test; reproducible install | T2, T4 |
| Beta = core feature freeze | Apr 18 | tier-1 round trip two sprints green; restore drill | T4, T3 |
| Extended-feature freeze (ADR-012) | May 2 | queue items accepted or cut per chapter 20 §20.6 | T1 |
| RC | May 16 | zero P0/P1; report draft reviewed | all |
| Readiness | May 31 | submission folder complete; offline demo | all |
