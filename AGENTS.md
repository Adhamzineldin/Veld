# AGENTS.md: rules for every contributor and AI agent working in this repository

These rules exist so that work by any team member or AI agent moves Veld toward the agreed end state and never away from it. The end state is defined in `docs/review-2026-10/19-expected-state-at-submission.md` and `20-extended-scope-plan.md`; the decisions behind it are the ADRs in `docs/review-2026-10/adr/`. Read the review index (`docs/review-2026-10/00-INDEX.md`) before changing architecture. `CLAUDE.md` is older than this file and contains statements the review found to be false (listed at the end); where they conflict, this file wins.

## 1. Non-negotiable guarantees (never regress these)

1. **Generation is never destructive.** No code path may call `os.RemoveAll` on a user-chosen directory. All writes go through the staged writer and the `.veld-manifest.json`; only manifest-listed files are ever deleted. The previous output survives a failed run. `--dry-run` writes nothing.
2. **Output is deterministic.** Never iterate a Go map when producing output; sort first. CI generates every fixture twice on two operating systems and byte-compares.
3. **Zero runtime dependencies in generated types, clients and validators.** Framework adapters may import only the framework the user chose. The only declared exceptions are the WebSocket adapters (`ws`, `websockets`, one Go library), and they must be printed in the capability manifest and docs.
4. **The IR is the only cross-process contract.** Emitters, plugins and tools consume `internal/ir`; nothing outside `internal/syntax` and `internal/semantic` may import the lexer, parser or AST. The import-boundary test enforces this; do not add exceptions.
5. **`internal/language` is the single source of truth** for keywords, directives, scalar types and HTTP methods. The lexer, parser, LSP and editor grammars derive from it; never hard-code a list elsewhere.
6. **Tiers are earned by CI.** An emitter is tier 1 only if its output compiles and round-trips on three operating systems for every conformance fixture. The README support matrix is generated from CI; never hand-edit a support claim.
7. **Unsupported semantics fail loudly.** If an emitter cannot represent a construct, it reports `E_UNSUPPORTED_FEATURE` with the span; it never emits silently wrong code.
8. **Security defaults stay on.** Validation on by default; checksum required on pull; `https` required unless explicitly overridden; path containment on every archive and plugin write; authorization through the single helper in the registry; parameterized SQL only; no secrets in the repository or logs.
9. **Versioned contracts are bumped, not mutated.** Changes to the IR schema, plugin protocol, registry API, manifest or lockfile formats follow ADR-009 and require an ADR.
10. **Historical plans are not instructions.** `docs/EXECUTION_PLAN*.md`, `docs/roadmap.md` and `docs/*PROPOSAL*.md` are superseded; do not take timelines, emitter counts or team assignments from them.

## 2. Definition of done for any change

A change is done only when all of the following hold, and the pull request description says how each was verified:

- Tests: unit tests for new logic; a conformance fixture for any language or semantic change (added **before** the implementation); golden output plus a compile test for any emitter change; a round-trip test for tier-1 behaviour changes; `httptest`-based tests for any registry handler; a scripted-session test for any LSP change.
- Quality gates green locally and in CI: `go build ./...`, `go vet ./...`, `go test -race ./...`, `golangci-lint` (config in repo), `govulncheck`, the import-boundary test, the determinism job, the language-metadata drift job.
- Coverage: the package you touched does not drop below its current statement coverage, and new packages start at 70 percent or higher. Coverage is a floor, not a target; a compile or round-trip test is worth more than coverage.
- Documentation: user-facing behaviour documented in `docs/`; every documented claim traceable to a test or CI job; `CHANGELOG.md` entry under Unreleased; an ADR if a boundary, format or guarantee changed.
- Backlog: the PR title starts with the backlog ID (`B-023:`) and the commit messages reference the finding IDs they close (`F-NODE-05`).
- Size: one logical change per commit, ordered so every commit builds and passes tests; prefer pull requests under about 400 changed lines; split otherwise.
- Review: at least one reviewer who is not the author, from the owner matrix in `docs/review-2026-10/16-team-allocation.md` (`CODEOWNERS` encodes it); security-relevant changes (registry, archives, plugin host, auth) need T1 or T3 as reviewer.

## 3. Code rules (lean code, SOLID applied to Go)

- Small interfaces at the boundary, concrete types inside. An interface is introduced when there are two implementations or a test needs a fake, not before.
- Dependency direction: `cmd` to `compiler`/`emit`/`pkg`/`lsp`; `compiler` to `semantic` to `syntax` to `language`; `emit` to `ir`. Never upward, never sideways between emitters.
- Single responsibility per package and per function. Functions over about 60 lines or with more than three levels of nesting are split. Emitters share identifier, type-mapping and import logic through `internal/lang`; duplicated emitter logic is a review blocker.
- Open for extension through the emitter interface and the plugin protocol; closed for modification of the compiler by emitters. An emitter that needs a compiler change proposes an IR change in an ADR.
- Errors are never ignored. Every `error` return is handled or wrapped with context (`fmt.Errorf("unpack %s: %w", name, err)`). No `_ = f()` on I/O. No `panic` in library code; `recover` only at process boundaries (CLI command, LSP dispatch, plugin host).
- `context.Context` is the first parameter of anything that does I/O or can be slow; cancellation is honoured.
- No global mutable state except the emitter registry populated in `init()`. No `time.Now()`, random values, absolute paths, user names or hostnames in generated output.
- Generated code is formatted by the emitter deterministically; hand-written Go is `gofmt`-ed and `golangci-lint`-clean.
- Names: identifiers derived through `internal/lang` per language with reserved-word escaping; wire names are preserved as metadata.
- Delete dead code; do not comment it out. Frozen emitters live in git history under `frozen/<name>` tags.

## 4. Testing rules

- Table-driven tests; descriptive case names; one assertion topic per case.
- Assertions on generated code are structural or behavioural (compiles, parses, routes count equals operations, validator rejects X), never substring-only.
- No network, no real registries, no time-based sleeps in unit tests. Registry tests use `httptest` and a fake storage; the PostgreSQL job is the only place a database runs.
- Fuzz targets (`FuzzLex`, `FuzzParse`, `FuzzValidate`, `FuzzDiff`, `FuzzUnpack`) stay in the tree and run nightly; a fuzz crash is a P0.
- A failing test is fixed by fixing the code or correcting a wrong expectation with a justification in the commit message. Never delete, skip or weaken a test to make CI pass. `t.Skip` is allowed only for a missing toolchain, with the toolchain named.
- Security tests are regression tests: every finding with security severity medium or higher in `docs/review-2026-10/csv/defect-register.csv` has a test that fails if it reappears.

## 5. Rules specific to AI agents

- Before touching architecture, read `docs/review-2026-10/00-INDEX.md`, the relevant chapter and ADR, and the backlog item. If the task is not in the backlog, stop and ask for a backlog entry rather than inventing scope.
- Work from the backlog's acceptance criteria; implement fixture, then code, then tests, then docs, in separate commits.
- Run the full fast CI equivalent locally before reporting completion, and report the exact commands and results. A green unit suite is not proof that generated code compiles; run the compile and round-trip tests for the emitters you touched.
- Report failures as failures. Do not claim determinism, compilation or security properties you did not test in this session.
- Never edit generated files, `.veld-manifest.json`, lockfiles or anything under `docs/review-2026-10/` except to append a dated amendment. Never edit `AGENTS.md` or `CLAUDE.md` without a team decision recorded in an ADR.
- Never add a dependency to generated output. Adding a dependency to the compiler itself requires an ADR (current allowed set: cobra, lib/pq, x/crypto, and `extism/go-sdk` once Q11 lands).
- Never delete or move frozen code, examples or fixtures without an ADR; never rewrite history on `master`.
- Never run `veld generate`, `clean` or `push` against a path outside a scratch directory or the example projects; never contact a public registry from tests.
- Respect the gates: after the core feature freeze (2027-04-18) only defect fixes and queue items with a backlog ID; after the extended freeze (2027-05-02) only P0/P1 fixes; after the RC gate (2027-05-16) only P0 fixes with T1 approval.
- When an estimate is exceeded by 1.5 times, stop, write a one-page note in `docs/notes/` describing the problem and two options, and ask the owner before continuing.
- Keep context small: prefer reading the backlog item, the fixture and the package you change over scanning the repository.

## 6. Security rules

- Validate every external input at the boundary: contract files, config, archives, plugin responses, HTTP bodies. Enforce size limits before reading.
- Path containment is checked on the cleaned absolute path with a trailing-separator prefix test, never with a string prefix on the entry name.
- Archives: allow-list entry types (regular files, directories), reject symlinks and devices, cap entries, per-entry size, total size and compression ratio, extract to a temporary directory and rename.
- Registry: every handler calls the authorization helper; private is the default visibility; uniform 404 for unauthorized reads; tokens hashed; sessions revocable; rate limits on authentication endpoints; `Content-Security-Policy` and escaping in the UI.
- Plugins: never execute a plugin that is not pinned in `veld.config.json`; native plugins are documented as unsandboxed; the Wasm transport has no filesystem or network capabilities.
- Report suspected vulnerabilities through `SECURITY.md`, not in a public issue.

## 7. Process

- Branches `feat/B-023-short-name`, `fix/F-NODE-05-short-name`; squash is not used, history stays as small ordered commits.
- ADRs: `docs/adr/ADR-NNN-title.md`, status proposed to accepted; a proposed ADR is open for one sprint of comments.
- Integration owner rotates each sprint (T1, T2, T3, T4, T5) and runs the vertical slice from a clean clone at the sprint review.
- Triage: labels `P0` to `P3` as defined in `docs/review-2026-10/05-defect-register-and-documentation-discrepancies.md`; P0 blocks the sprint.
- Docs are validated by the component owner named in chapter 16 before they are published.

## 8. Known-stale statements in `CLAUDE.md` (do not act on them until the file is rewritten)

- "Zod schemas (Node) and Pydantic schemas (Python) are generated": neither exists; validators are generated code.
- "Output is deterministic": false at 6a47f81 (F-LEAD-01) until B-012 lands.
- "Veld never writes outside the --out directory" and "never touches developer business logic files": false until ADR-007 lands.
- Top-level `veld login/logout/push/pull/serve` and `veld doctor`: only `veld registry ...` exists and `doctor` is unregistered until B-046.
- The `tools` config block is parsed but ignored until Q8.
- Rust is Axum (not Actix); PHP is Laravel (not PSR-15); Python output layout is `models/ interfaces/ routes/ middleware/`.
- "No separate registry binary exists": superseded by ADR-004.
- "only AST JSON passes between stages": superseded by ADR-002 (IR is the contract).
