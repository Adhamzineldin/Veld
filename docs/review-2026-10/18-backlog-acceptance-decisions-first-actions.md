# 18. Prioritized backlog, acceptance criteria, unresolved decisions and the first ten actions

## 18.1 Backlog

The implementation-ready backlog is `csv/backlog.csv` (58 items, stable IDs B-001 to B-058). Every item has an outcome, priority, acceptance criteria, effort range, owner, reviewer, dependencies, target milestone and the findings or ADRs it closes. Items for M0 to M2 are specified to the level of a pull request; items from M4 onward are coarser and will be split at sprint planning. The sprint responsibility table is `csv/sprint-allocation.csv`.

M0 and M1 items (implementation-ready, in dependency order): B-001 hygiene, B-002 Go 1.26, B-004 ADRs and survey, B-005 fixture skeleton, B-003 governance files, B-010 lexer fixes, B-006 staged writer, B-007 gate ordering, B-008 path safety, B-009 dry-run/clean/init, B-011 header identifiers, B-012 determinism, B-013 Python, B-014 Go, B-015 Node, B-016 compile jobs, B-018 client integrity, B-019 hardened unpack, B-020 registry auth, B-017 harness v0.

Priority definitions are those of chapter 05 §5.1. Per-sprint load is checked in chapter 16 §16.6.

## 18.2 Acceptance criteria for the three release levels

| Level | Date | Criteria (all must hold) |
|---|---|---|
| Alpha | 2027-03-21 | B-001 to B-040 accepted; vertical slice (Journeys A and B) in CI on two OSes; external plugin acceptance test; `veld install --frozen` reproducible; LSP multi-file session; registry e2e in CI |
| Beta (feature freeze) | 2027-04-18 | B-041 to B-054 accepted or formally cut; tier-1 round trip green on three OSes for two sprints; restore drill recorded; evaluation data collected; docs generated from CI evidence |
| Release 1.0.0 (readiness) | 2027-05-31 | chapter 14 §14.5 table all green; zero open P0/P1; signed artifacts; report, slides, demo video complete |

## 18.3 Unresolved decisions (only those that change the plan)

| # | Decision | Options | Default if not decided by | Owner |
|---|---|---|---|---|
| U1 | Exact submission date and exam windows | from the faculty calendar | the assumed windows in chapter 15 §15.1 stand; readiness stays 2027-05-31 | T1 with D1, by Oct 30 |
| U2 | University IP and open-source policy (ADR-011) | assignment required / notice only / none | proceed as MIT with the author list from git; confirm in writing | T1, by Nov 15 |
| U3 | Team skills and ownership (survey) | re-balance chapter 16 matrix | matrix as written | T1, by Oct 30 |
| U4 | Keep TOTP in the 1.0 registry | fix and test / disable endpoints | disable for 1.0 if B-020 tests are not green by M2 exit | T3, by Jan 8 |
| U5 | Python backend adapter for tier 1 | Flask only / Flask and FastAPI | Flask only | T4, by Nov 15 |
| U6 | Go router for tier 1 | net/http only / net/http and chi | both (chi is one strategy file) | T4, by Nov 15 |
| U7 | Second reference plugin | docs emitter / none | docs emitter if B-040 lands by Mar 7, else none | T4, by Mar 7 |
| U8 | Website LiveDemo | compile the Go engine to WebAssembly / remove the demo | remove the TS re-implementation; WebAssembly only if reserve exists after Beta | T5, by Apr 18 |
| U9 | Public hosting of a demo registry during the defense | self-hosted VM / local compose only | local compose on the demo machine | T3, by May 1 |
| U10 | Validation default ON is a breaking change for existing users | ship ON with `--no-validate` / keep OFF | ON (the product promise is validation) | T2, by Dec 13 |
| U11 | AI-assisted productivity multiplier (ADR-012 assumes 1.3 blended) | measured ratio at M1 exit | if below 1.15, apply chapter 20 §20.6 cut order | integration owner, Nov 29 |
| U12 | Go WebSocket library for Q7 | `golang.org/x/net/websocket` / `nhooyr.io/websocket` | x/net (Go-team maintained) | T4, by Mar 21 |
| U13 | Signing mechanism for Q10 | Sigstore keyless / minisign keys | Sigstore keyless with minisign fallback for offline registries | T3, by Mar 21 |

Amendment 2026-10-21 (ADR-012): backlog items B-059 to B-077 add the extended queue (chapter 20), the IR reserved fields (B-074, M2), AGENTS.md enforcement in CI (B-075, M1), the May 2 extended-freeze review (B-076) and the productivity measurement (B-077). The first ten actions gain one: on Oct 24, T5 adds the PR template and backlog-ID check from `AGENTS.md` to CI.

## 18.4 First ten actions (this week and next, dated)

1. Oct 22 (T5): open the hygiene pull request: `git rm` the 20 MB binary, `test-gen*/`, `test-headers/`, `test-constants/`, `test.md`, `.idea/`, `docs/*.docx`, `editors/vscode/out/`, `internal/server/web/index.html` duplicate; set `go 1.26` in `go.mod` and CI; add `LICENSE` (MIT). Acceptance: B-001, B-002.
2. Oct 22 (D1): send the skills survey (chapter 16 §16.1) and collect exam dates; publish the ADR set in `docs/adr/` for comment. Acceptance: answers by Oct 30.
3. Oct 23 (T2): reproduce F-NODE-01 to 04 locally from the register and write the failing output-safety tests first (they become the acceptance tests for B-006 to B-009).
4. Oct 23 (T3): reproduce F-REG-09 and F-REG-10 with the review's archives (`appendix-track-E-registry.md` has the commands) and write the malicious-archive test suite skeleton (B-019).
5. Oct 24 (T4): reproduce the Python and Go compile failures on `veld init` output (chapter 02 commands) and add the first two compile tests, red, to CI (B-016).
6. Oct 24 (T1): create `internal/spec/testdata` with five fixtures from the audit's case files (`appendix-track-B-compiler-frontend.md`), including the wildcard-import and header cases (B-005).
7. Oct 27 (T1, D1): run the ADR workshop; record acceptance or amendments; close U3 (ownership) and U5, U6.
8. Oct 28 (T5): CI matrix job for ubuntu, windows, macos with `go test -race` and the drift job fixed to `git status --porcelain` (part of B-016, B-029).
9. Oct 29 (T2, T1 pairing): agree the `OutputFile` and staged-writer API and the gate ordering (interface contract for S1); write it as a one-page note in `docs/notes/`.
10. Oct 30 (all): M0 exit review: ADRs accepted, hygiene merged, survey results, backlog imported into the tracker with owners; confirm or recompute capacity; publish the Sprint 1 plan.

## 18.5 Consistency check performed on this plan

- Every mandatory component (writer, IR, plugin host and SDK, LSP, VS Code client, registry hardening, lockfile, compose, harness, evaluation) has an implementation owner, a reviewer and a milestone in `csv/backlog.csv` and a capacity line in chapter 16.
- No downstream item starts before its interface exists: the writer API (S1) precedes emitter migration; `diag` (S3) precedes LSP work; IR schema (S5) precedes the SDK; VEP schema (S6) precedes the host runner completion; the lockfile format (S7) precedes the resolver.
- Budget: planned technical work 694 to 1,139 h against 1,106 h committed plus 277 h reserve (chapter 15 §15.2); the exam sprints carry only review, freeze and documentation tasks.
- Release criteria (chapter 14 §14.5) map one-to-one to backlog items B-016, B-006, B-012, B-040, B-042, B-033, B-051, B-056, B-054, B-057, B-003.
