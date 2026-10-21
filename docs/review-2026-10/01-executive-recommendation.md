# 01. Executive recommendation

Reviewed: Veld at commit `6a47f81` (2026-09-12), audited 2026-10-21 on Go 1.26 with Node 22, Python 3.13, Java 25 and Docker. Build, vet and all 523 tests pass. The review ran seven delegated audit tracks plus lead-reviewer verification of the highest-severity items; 153 findings are registered (chapter 05, `csv/defect-register.csv`).

## Product direction

Keep the thesis and narrow the claim. Veld's defensible position is **a minimal, fully checked contract language whose compiler emits dependency-free server interfaces, route wiring, validation and clients for a few stacks, with build-tool safety properties (deterministic, non-destructive, breaking-change aware)**. The custom `.veld` DSL is justified because it is small enough to check completely and to support in an editor; OpenAPI export is the interoperability escape hatch, OpenAPI import is a stretch goal. Do not claim eighteen targets, Docker or CI generation, GraphQL, SQL, or a hosted registry as differentiators (chapter 03).

## Retain, refactor or rewrite

**Selective replacement, no rewrite** (chapter 06, weighted score 4.25 against 3.35 for pure stabilization and 2.05 for a rewrite). Evidence: the lexer, parser and validator are fast and survived 1.67 million fuzz executions with no panic; the Node/TypeScript pair runs end to end; the registry architecture is sound. What is broken is in boundaries and tests, not in the language choice:

1. Generation is destructive: `veld generate`, `watch` and `clean` run `os.RemoveAll` on the output directory before parsing; `--out=../veld` deleted the project's own sources; the breaking-change gate never fires because the lock file is deleted first (P0, reproduced, `cmd/veld/generate.go:40-52`).
2. No test compiles generated code: Python output has a syntax error in every route file, Go output does not compile, Rust is a stub, Java/C#/PHP do not compile, and the suite is green (P0, reproduced on the `veld init` scaffold).
3. Three independent implementations of the language exist (compiler, VS Code extension, JetBrains plugin) and already disagree; the LSP ignores imports (P1).
4. The registry's happy path works but it is not safe to deploy: 2FA bypass, storage path traversal on publish, unenforced token scopes, optional download integrity, stored XSS sinks (P0, reproduced on a live server against a disposable PostgreSQL).

Documentation overstates the product: Zod and Pydantic validation do not exist, output is not deterministic (reproduced: 1 of 6 runs differs), the documented `import @models/*` form silently discards the rest of the file, and several documented commands are not registered (26 discrepancies, chapter 05 §5.5).

## Recommended stack

Go 1.26 for the compiler, CLI, language server and registry server (ADR-001); a versioned JSON IR as the only cross-process contract (ADR-002); external emitters as executables speaking JSON over stdin/stdout with host-mediated writes, TypeScript SDK first, WebAssembly deferred (ADR-003); the registry split into its own binary and shipped as a Docker Compose deployment (ADR-004); VS Code as the primary editor on a thin LSP client, JetBrains via LSP4IJ after the Beta gate (ADR-006); a manifest-based staged output writer (ADR-007); package manifest and lockfile for shared contracts, registry data-only (ADR-008); one monorepo (ADR-010).

## Supported scope at graduation (ADR-005)

Tier 1, compiled and round-trip tested in CI: Node backend (plain and Express), TypeScript client, React hooks, Python backend (Flask), Go backend (net/http, chi), TypeScript/Python/Go service SDKs, OpenAPI export. Tier 2, compile-checked nightly: Vue, Svelte, JavaScript, types-only, Java, C#, PHP, docs, GraphQL, SQL/Prisma. Frozen or removed: Rust, Angular, Dart, Kotlin, Swift, mock, Dockerfile, CI and env generators. A full matrix would cost 75 to 110 hours per month to maintain, more than half of the team's capacity.

## Plan and capacity

Sixteen sprints from 2026-10-21 to 2027-05-30 with gates: migration decision Nov 29, Alpha Mar 21, Beta and feature freeze Apr 18, release candidate May 16, readiness May 31; June reserved for submission and defense. Planned technical work is 694 to 1,139 hours against 1,106 committed hours (five members at 12 h/week, exam-adjusted, 15% coordination) plus a 277-hour reserve that is not allocated (chapter 15). The first eight weeks are planned week by week; the first ten actions are dated (chapter 18).

## Principal risks

- The IR and diagnostics work (T1) is on the critical path for plugins, diff and the LSP; its slip is mitigated by an AST-to-IR adapter.
- The registry stream can expand without bound; chapter 13 §13.2 is the fence, and TOTP, search and the audit UI are the first cuts.
- The capacity model is an assumption; the skills survey and exam calendar in Sprint 0 may force the low-capacity cut list (chapter 15 §15.6).
- External facts in this review rest on search summaries because page fetching was blocked; re-verify before citing in the thesis (appendix).

## What to do first

Merge the hygiene pull request (a 20 MB binary and scratch directories are tracked; no LICENSE file exists), accept the ADRs, write the failing output-safety and compile tests, and fix the destructive generation path before any feature work. Nothing else in this plan is safe to build on top of the current output writer.
