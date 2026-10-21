# Veld: Architecture Review and Delivery Plan to June 2027

Reviewed revision `6a47f815f37ea4c286a6e765312df76743fa7133` (master, 2026-09-12). Review date 2026-10-21. Planning baseline 2026-10-21; readiness target 2027-05-31; submission and defense June 2027.

Prepared for the Veld graduation team (T1 to T5, D1). All architecture decision records are PROPOSED and await team acceptance in Sprint 0. Performance figures in chapter 19 are targets unless marked measured.

Combined file generated 2026-10-21 from `docs/review-2026-10/`. Part I is the report (chapters 1 to 19), Part II the decision records, Part III the diagrams, Part IV the data tables, Part V the appendices.


## Contents

- Veld architecture review and delivery plan (2026-10-21)
- 01. Executive recommendation
- 02. Review methodology, revision, environment, evidence and limitations
- 03. Product assessment and comparison with alternatives
- 04. Current architecture and verified capability inventory (6a47f81)
- 05. Prioritized defect register and documentation discrepancies
- 06. Refactor versus rewrite, and language decisions
- 07. Target architecture, repository layout, interfaces and ADRs
- 08. Compiler, language and CLI plan
- 09. Built-in emitters and generated-code quality
- 10. External Emitter System (Veld Emitter Protocol, VEP v1)
- 11. Language server and editor integrations
- 12. Package management, installation and generated-SDK distribution
- 13. Registry architecture and self-hosting plan
- 14. Security, testing, releases, open-source governance and ongoing maintenance
- 15. Dated roadmap to June 2027, dependencies, capacity and contingency
- 16. Team allocation: T1 to T5 and D1
- 17. Diagrams, academic evaluation, report outline and demonstration plan
- 18. Prioritized backlog, acceptance criteria, unresolved decisions and the first ten actions
- 19. Expected state of Veld at submission (readiness gate 2027-05-31, release 1.0.0)
- 20. Extended scope plan (decision ADR-012, 2026-10-21)
- Part II: Architecture decision records ADR-001 to ADR-010
- Part III: Diagrams D1 to D8 (Mermaid)
- Part IV: Data tables (defect register, backlog, sprint allocation)
- Part V: Appendices (external research; audit-track notes A to G)



# Part I: Report



## Veld architecture review and delivery plan (2026-10-21)

Reviewed revision: `6a47f815f37ea4c286a6e765312df76743fa7133` on `master`, working tree clean except the review brief. Planning baseline 2026-10-21; readiness target 2027-05-31; submission and defense June 2027. This directory is new and does not modify any historical plan under `docs/`.

### Report (read in order)

| # | File | Content |
|---|---|---|
| 01 | `01-executive-recommendation.md` | product direction, retain/refactor/rewrite decision, stack, scope, risks |
| 02 | `02-methodology-and-evidence.md` | revision, environment, commands, audit tracks, evidence labels, limitations |
| 03 | `03-product-assessment.md` | problem, users, journeys, alternatives, DSL justification, thesis, deliverables, non-goals, research questions, evaluation plan |
| 04 | `04-current-architecture-and-capability-inventory.md` | verified capability inventory with coverage and verification method |
| 05 | `05-defect-register-and-documentation-discrepancies.md` | scales, P0 and P1 tables, documentation discrepancies, what to preserve |
| 06 | `06-rewrite-and-language-decision-matrices.md` | weighted strategy matrix, per-component language decisions |
| 07 | `07-target-architecture-and-repository-layout.md` | boundaries, AST/IR, directory tree, interfaces, deployables, ADR index |
| 08 | `08-compiler-language-cli-plan.md` | pipeline, IR shape, language 1.0 scope, safe generation, CLI, work breakdown |
| 09 | `09-emitters-and-generated-code-quality.md` | emitter inventory, tiers, zero-dependency definition, testing beyond snapshots |
| 10 | `10-external-emitter-system.md` | plugin protocol (proposed), manifest, exchange, SDK, hello emitter journey |
| 11 | `11-language-server-and-editors.md` | LSP state and plan, VS Code primary, JetBrains limited, metadata pipeline |
| 12 | `12-packages-installation-distribution.md` | installing Veld, ecosystem packages (manifest, lockfile, resolution), SDK distribution |
| 13 | `13-registry-and-self-hosting.md` | registry findings, scope, API and storage design, threat model, self-hosting, backup/restore |
| 14 | `14-security-testing-releases-governance.md` | security posture, CI layers, releases, OSS governance, release criteria |
| 15 | `15-roadmap-to-june-2027.md` | capacity, budget check, milestones, first eight weeks, critical path, fallbacks, gates |
| 16 | `16-team-allocation.md` | skills survey, ownership matrix, per-member plans, pairing, sprint table |
| 17 | `17-diagrams-evaluation-report-and-demo.md` | diagram index, evaluation execution, thesis outline, demo and fallback |
| 18 | `18-backlog-acceptance-decisions-first-actions.md` | backlog summary, release acceptance, unresolved decisions, first ten actions |
| 19 | `19-expected-state-at-submission.md` | start state versus expected end state at 1.0.0 per dimension (features, support, performance targets, security, operations, docs), with verification methods and stated limitations |
| 20 | `20-extended-scope-plan.md` | the 2026-10-21 decision to include every deferred item as an ordered queue (constraints, schema generator, auth, unions, streaming, WebSocket, Docker/CI, OpenAPI import, signing, Wasm plugins, hosted plugins, tier-2 revival, HA) with AI-assisted capacity, per-member load and the automatic cut |
| rules | `../../AGENTS.md` (repository root) | quality and process rules for every contributor and AI agent; protects the end state; lists the stale statements in `CLAUDE.md` |
| all | `VELD-REVIEW-COMBINED.md` | every chapter, ADR, diagram, CSV table, appendix and `AGENTS.md` in one file (for conversion to Word or PDF) |

### Supporting artifacts

| Path | Content |
|---|---|
| `adr/ADR-001` to `ADR-010` | architecture decision records (all PROPOSED, for acceptance in Sprint 0) |
| `diagrams/D1` to `D8` (`.mmd`) | editable Mermaid sources; each header states purpose, owner, update trigger, and current-versus-proposed labelling |
| `csv/defect-register.csv` | 153 findings with all required columns (ID, category, evidence, severities, priority, file:line, reproducer, expected/actual, impact, root cause, recommendation, regression check, effort, owner, reviewer, dependencies, lead verification, source track) |
| `csv/backlog.csv` | 58 backlog items with stable IDs, acceptance criteria, effort, owner, reviewer, dependencies, milestone |
| `csv/sprint-allocation.csv` | 16 sprints with per-member responsibilities and integration checkpoints |
| `appendix-external-research.md` | external facts with URLs and access dates, marked primary/secondary/unverified |
| `appendix-track-A` to `G` | raw audit-track notes (command logs, case tables, reproduction logs) |

### How findings are labelled

Evidence: reproduced defect, source-supported defect, suspected risk, design limitation, documentation mismatch, unverified. Severity scales (user impact, security, delivery priority P0 to P3) are defined in chapter 05 §5.1 and kept separate.

### Headline numbers

| Item | Value |
|---|---|
| Findings | 153 (88 reproduced, 19 source-supported, 34 documentation mismatches, 8 design limitations, 4 suspected risks) |
| Priority | 25 P0, 43 P1, 36 P2, 49 P3 |
| Security high | 6 (route shadowing bypassing middleware; registry 2FA bypass, storage traversal, unenforced scopes, optional integrity, stored XSS sinks) |
| Decision | selective replacement in Go; no rewrite |
| Capacity to readiness | 1,106 committed technical hours plus 277 reserve; 148 D1 hours (assumptions in chapter 15) |
| Planned technical work | 694 to 1,139 hours |



## 01. Executive recommendation

Reviewed: Veld at commit `6a47f81` (2026-09-12), audited 2026-10-21 on Go 1.26 with Node 22, Python 3.13, Java 25 and Docker. Build, vet and all 523 tests pass. The review ran seven delegated audit tracks plus lead-reviewer verification of the highest-severity items; 153 findings are registered (chapter 05, `csv/defect-register.csv`).

### Product direction

Keep the thesis and narrow the claim. Veld's defensible position is **a minimal, fully checked contract language whose compiler emits dependency-free server interfaces, route wiring, validation and clients for a few stacks, with build-tool safety properties (deterministic, non-destructive, breaking-change aware)**. The custom `.veld` DSL is justified because it is small enough to check completely and to support in an editor; OpenAPI export is the interoperability escape hatch, OpenAPI import is a stretch goal. Do not claim eighteen targets, Docker or CI generation, GraphQL, SQL, or a hosted registry as differentiators (chapter 03).

### Retain, refactor or rewrite

**Selective replacement, no rewrite** (chapter 06, weighted score 4.25 against 3.35 for pure stabilization and 2.05 for a rewrite). Evidence: the lexer, parser and validator are fast and survived 1.67 million fuzz executions with no panic; the Node/TypeScript pair runs end to end; the registry architecture is sound. What is broken is in boundaries and tests, not in the language choice:

1. Generation is destructive: `veld generate`, `watch` and `clean` run `os.RemoveAll` on the output directory before parsing; `--out=../veld` deleted the project's own sources; the breaking-change gate never fires because the lock file is deleted first (P0, reproduced, `cmd/veld/generate.go:40-52`).
2. No test compiles generated code: Python output has a syntax error in every route file, Go output does not compile, Rust is a stub, Java/C#/PHP do not compile, and the suite is green (P0, reproduced on the `veld init` scaffold).
3. Three independent implementations of the language exist (compiler, VS Code extension, JetBrains plugin) and already disagree; the LSP ignores imports (P1).
4. The registry's happy path works but it is not safe to deploy: 2FA bypass, storage path traversal on publish, unenforced token scopes, optional download integrity, stored XSS sinks (P0, reproduced on a live server against a disposable PostgreSQL).

Documentation overstates the product: Zod and Pydantic validation do not exist, output is not deterministic (reproduced: 1 of 6 runs differs), the documented `import @models/*` form silently discards the rest of the file, and several documented commands are not registered (26 discrepancies, chapter 05 §5.5).

### Recommended stack

Go 1.26 for the compiler, CLI, language server and registry server (ADR-001); a versioned JSON IR as the only cross-process contract (ADR-002); external emitters as executables speaking JSON over stdin/stdout with host-mediated writes, TypeScript SDK first, WebAssembly deferred (ADR-003); the registry split into its own binary and shipped as a Docker Compose deployment (ADR-004); VS Code as the primary editor on a thin LSP client, JetBrains via LSP4IJ after the Beta gate (ADR-006); a manifest-based staged output writer (ADR-007); package manifest and lockfile for shared contracts, registry data-only (ADR-008); one monorepo (ADR-010).

### Supported scope at graduation (ADR-005)

Tier 1, compiled and round-trip tested in CI: Node backend (plain and Express), TypeScript client, React hooks, Python backend (Flask), Go backend (net/http, chi), TypeScript/Python/Go service SDKs, OpenAPI export. Tier 2, compile-checked nightly: Vue, Svelte, JavaScript, types-only, Java, C#, PHP, docs, GraphQL, SQL/Prisma. Frozen or removed: Rust, Angular, Dart, Kotlin, Swift, mock, Dockerfile, CI and env generators. A full matrix would cost 75 to 110 hours per month to maintain, more than half of the team's capacity.

### Plan and capacity

Sixteen sprints from 2026-10-21 to 2027-05-30 with gates: migration decision Nov 29, Alpha Mar 21, Beta and feature freeze Apr 18, release candidate May 16, readiness May 31; June reserved for submission and defense. Planned technical work is 694 to 1,139 hours against 1,106 committed hours (five members at 12 h/week, exam-adjusted, 15% coordination) plus a 277-hour reserve that is not allocated (chapter 15). The first eight weeks are planned week by week; the first ten actions are dated (chapter 18).

### Principal risks

- The IR and diagnostics work (T1) is on the critical path for plugins, diff and the LSP; its slip is mitigated by an AST-to-IR adapter.
- The registry stream can expand without bound; chapter 13 §13.2 is the fence, and TOTP, search and the audit UI are the first cuts.
- The capacity model is an assumption; the skills survey and exam calendar in Sprint 0 may force the low-capacity cut list (chapter 15 §15.6).
- External facts in this review rest on search summaries because page fetching was blocked; re-verify before citing in the thesis (appendix).

### What to do first

Merge the hygiene pull request (a 20 MB binary and scratch directories are tracked; no LICENSE file exists), accept the ADRs, write the failing output-safety and compile tests, and fix the destructive generation path before any feature work. Nothing else in this plan is safe to build on top of the current output writer.



## 02. Review methodology, revision, environment, evidence and limitations

### 2.1 Revision reviewed

| Item | Value |
|---|---|
| Repository | `D:\University\Veld` (local working copy), module `github.com/Adhamzineldin/Veld` |
| Commit | `6a47f815f37ea4c286a6e765312df76743fa7133` ("docs: add project proposal, developer guide and execution plans", 2026-09-12 01:13 +0300) |
| Branch | `master` |
| Working tree | clean except one untracked file, `docs/FABLE_PROJECT_REVIEW_PROMPT.md` (the review brief) |
| Review date | 2026-10-21 (planning baseline) |
| Reviewer | Claude Fable 5.1 acting as lead reviewer, with seven delegated audit tracks (Claude Sonnet subagents) whose raw notes are referenced in chapter 05 |

### 2.2 Environment and toolchains

| Tool | Version / status | Used for |
|---|---|---|
| Go | go1.26.0 windows/amd64 | build, vet, tests, generated Go compile |
| Node.js / npm | v22.20.0 / 10.9.3 | TypeScript compile of generated output, VS Code extension build, website build |
| Python | 3.13.7 | generated Python compile/import, Flask round trip, LSP driver script |
| Java | 25.0.1 (javac only; no Maven/Gradle verified) | static inspection only |
| Docker | 29.7.2 | disposable PostgreSQL for the registry end-to-end trace |
| PostgreSQL client (`psql`) | not installed | registry inspected via the server API and Docker |
| Rust (`cargo`), .NET SDK, PHP | not installed | Rust, C#, PHP outputs were inspected statically only (environment limitation, not a defect) |
| OS | Windows 11 Pro 10.0.26100, Git Bash and PowerShell | all commands |
| Web access | `WebSearch` available; `WebFetch` blocked by a PreToolUse hook in the session | external facts rest on search-result summaries and are marked provisional |

### 2.3 Baseline checks (run by the lead reviewer, exact commands)

```
go build ./...            # exit 0
go vet ./...              # exit 0
go test ./...             # exit 0, all packages ok; 20 packages have no test files
go run ./cmd/generate-language && git status --short editors veld-language.json   # no drift in tracked generated files
go test -cover ./...      # per-package coverage recorded in chapter 04
```

Size metrics, measured on tracked files only (`git ls-files`), excluding the 20 MB tracked binary, `website/`, `editors/` and `packages/`:

| Area | Non-test Go lines | Test Go lines |
|---|---|---|
| `internal/emitter` (all emitters + helpers) | 23,626 | 5,648 |
| `cmd/veld` | 4,729 | 534 |
| `internal/server` (registry server) | 2,598 | 0 |
| `internal/docsgen` | 1,864 | 0 |
| `internal/generators` | 1,568 | 643 |
| `internal/setup` | 1,226 | 1,071 |
| `internal/parser` | 1,154 | 1,129 |
| `internal/lsp` | 1,017 | 134 |
| `internal/language` | 838 | 0 |
| `internal/validator` | 642 | 472 |
| `internal/config` | 556 | 804 |
| `internal/diff` | 503 | 163 |
| `internal/registry` (client, credentials, tarball) | 495 | 0 |
| `internal/lexer` | 451 | 383 |
| `internal/loader` | 398 | 144 |
| all tracked `.go` | 43,886 | 12,180 |

Method: `git ls-files '<dir>/*.go' | grep -v _test.go | xargs cat | wc -l` and the `_test.go` complement. Generated files in the tree (`editors/vscode/src/veld-language-spec.ts`, `editors/jetbrains/.../VeldLanguageSpec.kt`) are not Go and are excluded. There is no vendored dependency directory.

### 2.4 Audit tracks

Seven parallel tracks were delegated, each with a written brief, a disposable working directory under the session scratchpad, and an instruction not to modify tracked files, not to publish, and not to contact any public registry. Each produced a findings file with the fixed finding format used in chapter 05.

| Track | Scope | Primary method |
|---|---|---|
| Documentation claims | README, CLAUDE.md, docs/*, test.md, editors and packages READMEs | claim inventory checked against source and the emitter registry |
| Compiler frontend | lexer, parser, ast, loader, validator, language, errors, format, lint, diff, cache | source reading plus 25+ adversarial contracts run through the built CLI |
| Node/TypeScript emitters | node backend, typescript/react/vue/angular/svelte/javascript/typesonly frontends, generate/clean/watch/dry-run | generate, `tsc --noEmit`, Express server plus generated SDK round trip, determinism and output-safety experiments |
| Other emitters and generators | python, go, rust, java, csharp, php, javascript backends; dart, kotlin, swift frontends; service SDKs; openapi, graphql, schema, docs, mock, dockerfile, cicd, envconfig | Python and Go outputs compiled and run; others static; OpenAPI validated with an external linter |
| Registry and archives | internal/registry, cmd/veld/registry.go, serve.go, internal/server/** | line-by-line archive review, crafted malicious archives against the real extraction code, authorization matrix, live server on disposable PostgreSQL |
| LSP and editors | internal/lsp, editors/vscode, editors/jetbrains, cmd/generate-language | scripted JSON-RPC session over stdio, extension build in a copy, metadata drift check |
| CLI, config, distribution, CI, website, hygiene | cmd/veld, internal/config, packages/*, .github/workflows, scripts, website, examples, test metrics | behaviour runs with exit codes, config precedence experiments, wrapper reading, website build in a copy |

The lead reviewer additionally read `internal/registry/tarball.go`, `client.go`, `credentials.go`, `internal/emitter/emitter.go`, the `pull` command in `cmd/veld/registry.go`, the registry schema in `internal/server/db/db.go`, and `docs/architecture/overview.md` directly, and independently confirmed the archive-handling findings that carry the highest security severity.

### 2.5 Evidence labels (used throughout chapters 04 and 05)

| Label | Meaning |
|---|---|
| reproduced defect | Triggered in this review with a recorded command and output |
| source-supported defect | Visible in the code at the cited lines; not executed, but the failure path is unambiguous |
| suspected risk | Plausible from code reading; needs a targeted test to confirm |
| design limitation | Works as designed, but the design does not meet the stated goal |
| documentation mismatch | Documentation asserts something the code does not do (or the reverse) |
| unverified | Could not be checked in this environment |

Severity scales are defined in chapter 05 (user impact, security severity, delivery priority P0 to P3 are kept separate).

### 2.6 Limitations

- Rust, C# and PHP generated code was not compiled; Java only inspected. Findings in those emitters are source-supported or suspected, not reproduced.
- No JetBrains Gradle build was attempted (Gradle offline availability unverified).
- External facts (competitors, versions, specifications) come from search summaries; where a primary page was not readable they are marked provisional in chapter 03 and the external-research appendix.
- No user study was run. The evaluation plan in chapter 17 separates planned measurements from results.
- Coverage percentages are Go statement coverage from `go test -cover`; they say nothing about whether generated code compiles. Only the audit tracks' compile and run steps do.
- The capacity and calendar numbers in chapters 15 and 16 are planning assumptions (12 h/week technical, 8 h/week D1, assumed exam windows) to be validated by the team in Sprint 0.



## 03. Product assessment and comparison with alternatives

External facts in this chapter were gathered on 2026-10-21. `WebFetch` was blocked by a session hook, so facts rest on search-result summaries of the cited pages and are marked **provisional** unless the summary quoted the primary page. Full notes: the external-research appendix (`appendix-external-research.md`).

### 3.1 The problem Veld addresses

Teams that build an HTTP API and one or more clients repeat the same shape three times: request and response types on the server, validation of inbound data, and a typed client. Keeping these in sync by hand is the source of a familiar class of bugs (a renamed field, a changed status code, an optional field that became required). Existing answers either require a large specification format (OpenAPI), a transport you may not want (gRPC), or lock you into one language ecosystem (tRPC).

Veld's answer: a small, readable contract language (`.veld`) from which one command produces server-side interfaces plus route wiring, validation, and native client SDKs for several languages, with no runtime library to install. The repository's own phrasing is "contract-first, multi-stack API code generator" (README.md).

### 3.2 Intended users and three journeys

Primary users: small teams (2 to 15 developers) and solo full-stack developers building JSON-over-HTTP services with a separate frontend, and student or course projects where the contract doubles as documentation.

Journey A, full-stack TypeScript team (the graduation vertical slice): write `veld/app.veld` with models and a module; `veld generate` into `backend/src/generated` (Node service interfaces, routes, validators) and `frontend/src/generated` (fetch SDK, React hooks); implement the service interface; mount routes on an Express router; call the API from React with typed hooks; change the contract, regenerate, and let the compiler show every call site that broke.

Journey B, polyglot microservices: an IAM service in Node and a transactions service in Python; the transactions workspace entry `consumes: ["iam"]`; `veld generate` produces a Python client for IAM inside the transactions service; `veld deps --validate` rejects a cycle.

Journey C, shared contracts in an organization: a platform team publishes `@acme/common` (shared models, error shapes) to a self-hosted registry; product teams `veld pull @acme/common@1.x` and `import @acme/common/Money`; the lockfile pins the version and checksum; `veld generate --strict` fails CI when a pulled update is a breaking change.

Journeys A and B exist in the codebase with the defects listed in chapter 05. Journey C's publish/pull loop exists; the lockfile, checksum requirement and scoped-registry binding do not (ADR-008).

### 3.3 Alternatives (relevant capabilities only)

| Alternative | What it is | Server interfaces + routing | Multi-language clients | Runtime deps in output | Custom DSL | Editor support | Package / registry | Overlap with Veld |
|---|---|---|---|---|---|---|---|---|
| TypeSpec (Microsoft) | TypeScript-like API DSL compiling to OpenAPI, JSON Schema, Protobuf, and clients | Preview server emitters: `@typespec/http-server-csharp` 0.58.0-alpha.33 (2026), `http-server-js` (documented, version not confirmed) | Preview client emitters JS, C#, Java, Python | Clients depend on TypeSpec runtime packages | Yes, large and expressive (decorators, templates, namespaces) | VS Code and Visual Studio extensions | npm packages | Closest competitor; its server emitters are alpha-quality, which is exactly Veld's niche. Release train 1.17 in preparation (github.com/microsoft/typespec PR "Prepare typespec publish", provisional). License reported as MIT (prior knowledge; not confirmed in-session). |
| Smithy (AWS) | Protocol-agnostic IDL with traits; Java-based build and plugins | Server generators for TypeScript (developer preview), Java (smithy-java 1.0 GA Apr 2026), Rust (work in progress) | Clients: TypeScript, Go, Kotlin, Swift, Python, Java, Rust | Yes (Smithy runtime per language) | Yes, heavy; requires JVM toolchain | smithy-language-server, VS Code, IntelliJ plugin | Maven coordinates | Strong but JVM-centric and enterprise-shaped; overkill for Veld's audience. Apache-2.0 (github.com/smithy-lang/smithy). |
| OpenAPI 3.1 / 3.2 + generators | Specification format; generators: openapi-generator (JVM, 50+ targets, vendor claim), oapi-codegen (Go), openapi-typescript/openapi-fetch, orval, Kiota, Fern, Speakeasy, Stainless | openapi-generator produces server stubs for many frameworks; quality varies by target | Very broad | Varies; many clients depend on runtime libraries | No DSL; YAML/JSON, verbose to author by hand | Good (schema-aware editors) | None for specs; artifacts via npm, Maven | The standard Veld must interoperate with, not replace. OpenAPI 3.2.0 released 2025-09 with 3.1.2 patch (spec.openapis.org, provisional). |
| Protocol Buffers / gRPC / Connect / buf | Binary IDL, RPC framework; buf adds linting, breaking-change detection, plugin execution, Buf Schema Registry | Yes (service stubs) | Connect: Go, TypeScript, Swift, Kotlin, Python | Yes (protobuf runtime) | Yes (`.proto`) | Good | BSR with dependency management and remote plugins | Best prior art for the plugin protocol (CodeGeneratorRequest/Response over stdin/stdout) and for a schema registry with breaking-change checks. Different transport (not plain JSON/HTTP). |
| tRPC, ts-rest, Hono RPC, Zod-OpenAPI, Elysia Eden | End-to-end type sharing in TypeScript | Yes, TypeScript only | TypeScript only | Yes | No (TypeScript itself) | Native | npm | Superior developer experience inside one TS monorepo; no polyglot story. (Unverified in-session; prior knowledge.) |
| GraphQL + GraphQL Code Generator | Query language with plugin-based codegen | Resolver types | Many languages | Yes | GraphQL SDL | Good | npm | Different API style; Veld exports SDL. (Unverified in-session.) |

### 3.4 Is a custom DSL justified?

Three options were compared.

| Option | Learning cost | Editor support | Migration effort for users | Expressiveness | Long-term maintenance |
|---|---|---|---|---|---|
| Keep `.veld` as the authoring language | Low: about 12 keywords, one page of syntax | Must be built by the team (chapter 11) | New users start from scratch; OpenAPI users need an importer | Deliberately small; cannot express everything OpenAPI can (content types, security schemes, callbacks) | The team owns a language: grammar, spec, versioning, tooling |
| Build on TypeSpec or OpenAPI as the source | Medium to high (TypeSpec) or high verbosity (OpenAPI YAML) | Exists | Existing users can adopt directly | Full | The team owns only emitters; the project becomes "another OpenAPI generator" |
| Keep `.veld` and offer interoperability (export OpenAPI; later import a subset of OpenAPI) | Low | Team-built | Export lets Veld users join OpenAPI toolchains; import lets OpenAPI users try Veld | Small core, escape hatch through export | Language plus an importer that must track OpenAPI versions |

Recommendation: the third option. The DSL's smallness is the product: a contract that a reviewer can read in one screen and that the compiler can check fully. That is a defensible academic position (a minimal language with precise semantics and a conformance suite) and a practical one (fast editor support is feasible for a small grammar). The cost is that Veld must export correct OpenAPI 3.1 so users are never trapped; OpenAPI import is a stretch goal, not a graduation requirement.

What would reverse this: if the user study (chapter 17) shows that participants familiar with OpenAPI cannot author a `.veld` contract after the tutorial, or if TypeSpec's server emitters leave preview with comparable zero-dependency output before the Beta gate, the product should pivot to a TypeSpec emitter set and present the compiler work as prior art.

### 3.5 Product thesis and differentiation

Thesis: **a minimal, fully checked contract language whose compiler produces dependency-free server interfaces, route wiring, validation and clients for several stacks from one source, with the same safety properties (deterministic output, breaking-change detection, non-destructive generation) a build tool must have.**

Credible differentiation, stated honestly:

1. Server-side output (interfaces plus route wiring plus validation) across several backends, where TypeSpec is alpha and openapi-generator's server stubs are uneven. Veld's own server output has the defects in chapter 05, so this claim is earned only after M1 and M2.
2. Zero runtime dependencies in generated code, defined precisely in chapter 09 (true for types and clients; validation uses generated code, not a library; framework adapters import only the framework the user already has).
3. A contract that fits on a screen, with breaking-change detection and a lockfile for shared contracts.
4. One static binary with an editor server inside it.

Not differentiators (do not claim): the number of supported languages, GraphQL or SQL export, Docker and CI generation, a hosted registry.

### 3.6 Minimum defensible graduation deliverable, target release, stretch

Minimum defensible deliverable (must pass the Beta gate, 2027-04-18):

- Language spec 1.0 with conformance fixtures; compiler with structured diagnostics, IR v1 and `veld ir`.
- Tier-1 targets: Node backend (framework adapter: Express), TypeScript client, React hooks; Python backend (Flask) and client; Go backend (net/http or chi) and client. Each compiled and round-trip tested in CI.
- Safe generation (manifest, staged writes, path checks), working breaking-change gate, deterministic output.
- External emitter protocol v1 with TypeScript SDK and the hello emitter installed from npm.
- Language server with cross-file diagnostics, completion, hover, definition; VS Code thin client.
- OpenAPI export (3.0.3 today, valid per redocly; 3.1 if cheap) validated by an external linter, with servers, security placeholders, defaults and an aligned error schema.
- Registry: hardened archive handling, atomic publish, checksum-required pull, lockfile, Docker Compose self-hosting with a tested backup/restore runbook.
- Report, evaluation results, demo.

Target open-source release (1.0.0, readiness gate 2027-05-31): the above plus tier-2 experimental targets kept compiling (chapter 09), JetBrains via LSP4IJ, npm and Homebrew installers, signed release artifacts with checksums and provenance attestations.

Stretch (only after the Beta gate, never on the critical path): OpenAPI import, Wasm plugin transport, Vue/Svelte/Angular parity, registry search and audit log UI, Open VSX publication.

### 3.7 Non-goals and deferred features (amended 2026-10-21 by ADR-012)

The original review deferred unions, WebSocket, streaming, auth schemes, per-field constraints, OpenAPI import, Docker/CI generation, plugin sandboxing, registry-hosted plugins, package signing and high availability. The project owner decided to include all of them as an ordered queue behind the core (chapter 20, ADR-012), subject to an automatic cut on 2027-05-02.

Remaining non-goals: generics beyond `Map` and arrays, conditional types, gRPC or binary transports, Kubernetes manifests, federated registries, a hosted public registry operated by the team, pagination helpers, custom scalar types, nullable as distinct from optional, enum members with explicit wire values. These stay out of 1.0.0 regardless of capacity.

### 3.8 Research and engineering questions for the report

1. Can a deliberately minimal contract language cover the HTTP APIs real projects write? Measure by re-expressing a corpus of public OpenAPI documents in `.veld` and recording the fraction of operations expressible and the constructs missing.
2. Does generated route wiring plus validation reduce contract drift compared with hand-written code? Measure on seeded contract changes: how many call sites and handlers the compiler flags versus manual review.
3. What does "zero runtime dependencies" cost in generated-code size and validation performance versus Zod or Pydantic? Measure bytes and request latency on a fixed workload.
4. Is a JSON-over-stdio plugin protocol sufficient for third-party emitters? Measure the time for an external developer to produce a working emitter from the SDK, and the overhead per invocation.
5. Can one language server serve two editors with no client-side language logic? Measure feature parity and maintenance effort (lines changed per language feature) before and after ADR-006.

### 3.9 Evaluation plan (planned measurements, no results yet)

| Question | Baseline | Representative tasks | Correctness measure | Reproducibility | Status |
|---|---|---|---|---|---|
| Expressiveness (Q1) | OpenAPI documents from public APIs (select 10 to 15 of varied size; list fixed in M5) | translate each to `.veld` | share of operations and schemas representable; list of blockers by category | scripts and corpus in `evaluation/expressiveness/` | planned |
| Drift detection (Q2) | same API hand-written in Express + fetch | apply 12 seeded changes (rename field, optional to required, status change, path change ...) | compiler errors and `veld diff` output versus ground truth | fixture repo with branches per change | planned |
| Generated-code cost (Q3) | Zod (Node), Pydantic (Python) validators for the same models | validate 10k requests, valid and invalid mix | identical accept/reject decisions; p50/p95 latency; bundle bytes | benchmark harness in `evaluation/validation/` | planned |
| Plugin protocol (Q4) | none (descriptive) | 3 to 5 developers write the hello emitter variant from the guide | time to first successful generate; defects found in the SDK | screen-recorded sessions with consent | planned |
| Editor parity (Q5) | the three implementations at 6a47f81 | 10 language-feature scenarios | features working per editor; lines changed per feature | scripted LSP session in CI | planned |
| User study (feasibility) | participants: 6 to 10 students or developers not on the team | 45-minute task: author a contract from a prose spec, generate, implement one handler, call it from the client | task completion, errors, SUS questionnaire, think-aloud notes | protocol and consent form in `evaluation/user-study/`; D1 coordinates, T4 runs | planned, M5 |

All results will be reported with their collection date, environment and scripts; no number in this review is a result.



## 04. Current architecture and verified capability inventory (6a47f81)

Every row states how it was verified. "Tests" is Go statement coverage from `go test -cover` plus the count of test functions where measured; coverage says nothing about generated-code correctness.

### 4.1 Pipeline and dependencies

The pipeline is `.veld -> lexer -> parser -> AST -> loader (imports) -> validator -> emitters -> files`, with the AST struct passed by value; there is no IR and no serialization between stages except `veld ast` (chapter 07 §7.2). External Go dependencies: `github.com/spf13/cobra v1.8.0`, `github.com/lib/pq v1.10.9`, `golang.org/x/crypto v0.24.0` (plus `pflag`, `mousetrap` indirect). `go.mod` declares `go 1.22` (out of support since Go 1.27 shipped on 2026-08-19); the code builds on 1.26 and uses only 1.22-era APIs (grep for `slices`, `maps`, `cmp`, `iter`, `min`/`max`, range-over-int found nothing; `http.ServeMux` method patterns from 1.22 are used).

Coupling observations: `cmd/veld` holds 4,729 lines across ten files (no longer monolithic: `main.go` is 109 lines since the 2026-08-27 split); emitters import `internal/emitter` helpers and `internal/ast` only (the isolation rule holds); the registry server is compiled into the CLI; the LSP re-reads keyword lists rather than using `internal/language`.

### 4.2 Capability inventory

| Area | Capability | State | Verification | Tests |
|---|---|---|---|---|
| Language | models, enums, modules, actions, `extends`, `Map`, arrays, optional, `@default`, `@deprecated`, headers block, middleware, status suffix, constants, unions (undocumented), `WS` actions with `stream`/`emit` (undocumented), HEAD/OPTIONS (undocumented) | implemented | parser read; 72 adversarial cases | parser 53.7%, lexer 60.9%, validator 53.7% |
| Imports | alias, relative, wildcard | alias and relative work; wildcard `@alias/*` is lexed as a comment (F-FE-01) | reproduced | loader 30.2% |
| Diagnostics | validator reports all errors with `file:line` and suggestions; lexer/parser first error only, line only | partial | reproduced | errors pkg 78.4% but unused by the pipeline |
| Robustness | no panic or hang in 1.67M fuzz executions; 5 MB file and 100k-field model validate under 1 s | strong | fuzzing | |
| Formatter | `veld fmt` | broken: drops comments and quotes, panics on `model }` | reproduced | 63.6% |
| Lint | 8 rules | works; error-level rules unreachable (validator rejects first); false positive on nested models | reproduced | 84.0% |
| Breaking-change detection | `diff`, `.veld.lock.json`, `--strict`, `--force`, prompt | implemented but dead in `generate` (lock deleted first); classifier shallow | reproduced | diff 55.2% |
| Incremental cache | `.veld-cache.json` | wiped on every run; keyed on `.veld` content only | source-supported | 92.1% |
| Config | nested and flat forms, aliases, workspace, hooks, `tools` block | flat overrides nested (docs say the reverse); `tools` ignored; unknown keys ignored; `--out` ignored when config sets backend out | reproduced | 74.5% |
| Generation safety | "never touches user files", "never writes outside out" | false: `RemoveAll` of output dirs before parsing; `--out` at the source dir deletes sources | reproduced | cmd/veld 0% (tests run a built binary) |
| Determinism | documented guarantee | false (export order varies) | reproduced | |
| Node backend | types, interfaces, routes, validators (opt-in), middleware interface, express/plain strategies | runs under tsx; fails strict tsc; route shadowing; 500 leaks; no Zod | reproduced | node 19.6% |
| TS client | fetch SDK, `VeldApiError`, path params | works except encoding, query arrays, strict tsc | reproduced | typescript 59.3% |
| React / Vue / Svelte | hooks, composables, stores wrapping the SDK | compile once SDK fixed | reproduced | 83.4 / 79.6 / 79.8% |
| Angular | HttpClient services | invalid output | reproduced | 72.4% |
| JavaScript targets | plain JS | frontend emits `.ts`; backend fails `node --check` | reproduced | 62.0 / 29.5% |
| Python backend | models, ABC interfaces, routes, validators, flask/plain | syntax error in routes; routes never registered; `Decimal` import missing | reproduced | 28.5% |
| Go backend | models, interfaces, routes, server, go.mod, plain/chi | `undefined: mux`; no validation; date/datetime conflated | reproduced | 46.8% |
| Rust / Java / C# / PHP backends | structs/classes, routes | none compile; Rust is a stub | reproduced (Java via javac) and source-supported | 48.5 / 58.2 / 59.1 / 47.9% |
| Dart / Kotlin / Swift clients | HTTP clients | defects by reading; not compiled | source-supported | 63.6 / 66.3 / 64.9% |
| Service SDKs (`consumes`) | 8 languages; `veld deps`, `--validate` | TS, Python, Go verified for env-var precedence; TS 24 strict errors; Python rejects its own types; others unverified | reproduced | sdkhelpers 100% |
| OpenAPI | 3.0.3 export | valid per redocly (style warnings); incomplete | reproduced | 65.6% |
| GraphQL SDL | export | invalid on header fields | reproduced | 0% |
| SQL / Prisma | export | invalid defaults, DTOs as tables | reproduced | schema 70.0%, database 79.9% |
| Docs generator | Markdown docs | untested | not exercised | 0% (1,864 lines) |
| Mock server | `internal/mock` | dead code, no command | source | 61.2% |
| Dockerfile / CI / envconfig | generators | download 404 asset; insecure compose | reproduced | 76.6 / 56.4 / 66.7% |
| LSP | stdio server: sync, diagnostics, completion, hover, definition | single-file only; byte offsets; first error; no MethodNotFound | reproduced | 33.6% |
| VS Code | extension | independent TS implementation; commands not registered; builds; no tests | reproduced | none |
| JetBrains | PSI plugin, Kotlin 4,090 lines | not built in review; hand-written duplication | source | none |
| Language metadata | `internal/language`, `cmd/generate-language` | generator works, no drift; compiler does not consume it | reproduced | 0% |
| Registry client | login, push, pull, versions, tokens | works in the happy path; integrity optional; registry chosen from map order | reproduced | 0% |
| Registry server | auth, orgs, packages, tokens, TOTP, email, SPA | works in the happy path; 2FA bypass, path traversal, scopes unenforced, XSS sinks, no limits, no health endpoint, no tests | reproduced | 0% |
| Distribution | npm, pip, homebrew, go, nuget, chocolatey, apt, composer | go install works; npm/pip need work; others placeholders | reproduced / source | none |
| CI | ubuntu-only build, vet, race tests, drift, cross-compile; release to 7 channels without signing | works for what it covers; no generated-code compile; no OS matrix | source | |
| Website | Vite 7, React 19; marketing, docs, LiveDemo with its own TS parser | builds (no lockfile, lint errors); not in CI | reproduced | |
| Examples | 10 | 9 validate; `microservices` fails (wildcard import); only `node-typescript` output compiles | reproduced | |
| Hygiene | tracked 20 MB binary, scratch dirs, `.idea`, `.docx`, stale `out/`, `test.md` | 24.8 MB tracked, 82% is the binary | reproduced | |
| Governance | LICENSE, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, CODEOWNERS, templates, CHANGELOG | absent (changelog for v0.1 only) | reproduced | |

### 4.3 Operational maturity

- No health endpoint, no migrations versioning, no backup tooling, no TLS option, no rate limiting in the registry.
- No signed releases, SBOM or provenance; wrappers do not verify checksums.
- No OS matrix in CI; Windows path handling has visible defects (config path re-basing, `sh -c` hooks).
- No telemetry (good); no crash reporting.

### 4.4 Public interfaces today

- CLI (cobra): `init`, `validate`, `ast`, `generate`, `watch`, `clean`, `openapi`, `lint`, `fmt`, `docs`, `graphql`, `schema`, `diff`, `setup`, `deps`, `ci`, `lsp`, `registry {login, logout, push, pull, versions, token, serve, ...}`; `doctor` defined but not registered; several hidden commands.
- Go interfaces: `emitter.Emitter`, `BackendEmitter`, `FrontendEmitter`, `ToolEmitter` (`internal/emitter/emitter.go:43-86`); no external protocol.
- Registry HTTP API `/api/v1` (auth, orgs, packages, tokens).
- LSP over stdio.
- Config schema `editors/veld-config.schema.json` (synchronized copies verified byte-identical).

### 4.5 Summary judgement

The repository contains a working compiler frontend of good quality, one backend and client pair (Node and TypeScript) that is close to working, a wide but unverified emitter matrix, a registry whose happy path works but whose security does not hold, and tooling claims that outrun the code. The defects cluster in four systemic causes: output written destructively without a manifest; no test compiles generated code; three independent implementations of the language; and a registry written without tests or a threat model. Chapters 06 to 16 address exactly those four causes.



## 05. Prioritized defect register and documentation discrepancies

The full register is `csv/defect-register.csv` (153 rows, one per finding, all columns required by the brief: ID, category, evidence status, user-impact severity, security severity, exploitability, priority, file:line, trigger/reproducer, expected vs actual, impact, root cause, recommendation, regression check, effort estimate, owner, reviewer, dependencies, lead-reviewer verification, source track). The raw audit-track notes with command logs are in `appendix-track-*.md`. This chapter explains the scales, lists everything at P0 and P1, and states what to preserve.

### 5.1 Scales (kept separate)

| Scale | Values | Meaning |
|---|---|---|
| Evidence status | reproduced defect, source-supported defect, suspected risk, design limitation, documentation mismatch, unverified | chapter 02 §2.5 |
| User-impact severity | critical (data loss, or the headline workflow cannot complete), high (a documented feature does not work), medium (works with a visible defect or workaround), low (cosmetic, docs) | effect on a user of the current release |
| Security severity | critical, high, medium, low, none | CVSS-like judgement: high = exploitable by an unprivileged party with meaningful impact |
| Delivery priority | P0 (fix before any other feature work; blocks the vertical slice, destroys user data, or security high), P1 (fix before the Alpha gate; documented feature broken or security medium), P2 (fix before Beta or demote the feature), P3 (fix opportunistically or close as won't-fix with a docs note) | order of work |

Counts at 6a47f81: 153 findings; by evidence: 88 reproduced, 19 source-supported, 34 documentation mismatches, 8 design limitations, 4 suspected risks; by priority: 25 P0, 43 P1, 36 P2, 49 P3; with security severity high: 6 (F-NODE-05, F-REG-01, F-REG-02, F-REG-03, F-REG-09, F-REG-11). The lead reviewer independently reproduced or confirmed at source 20 of them (column `lead_verified`).

### 5.2 P0 (25)

| ID | Finding | Evidence | Security | Location | Effort | Owner |
|---|---|---|---|---|---|---|
| F-NODE-01 / F-CLI-01 / F-CLI-03 | `generate`, `watch` and `clean` run `os.RemoveAll` on every output directory before parsing; user files inside the output directory are deleted; `clean` with `out: ./src` removed `src/business.ts` | reproduced (lead confirmed at source) | low | `cmd/veld/generate.go:40-52`, `:758-760`; `watch.go:56` | 6 to 10 h (shared) | T2 |
| F-NODE-02 / F-CLI-02 | `--out=../proj` or `--out=../veld` accepted; the project's own `.veld` sources were deleted; validation only inspects the last path segment | reproduced | low | `internal/config/config.go:371-379` | 2 to 5 h | T2 |
| F-NODE-03 | a contract with an error wipes the previous good output; `watch` does so on every save | reproduced | none | `generate.go:758-760` | 4 to 8 h | T2 |
| F-NODE-04 / F-FE-02 | `.veld.lock.json` deleted inside `runClean` before the diff: `--strict`, `--force` and the prompt never fire; a removed action passes `--strict` with exit 0 | reproduced (lead confirmed) | none | `generate.go:51`, `:1019-1035` | 1 to 6 h | T1 |
| F-CLI-05 | `veld init` overwrites existing `README.md` and `veld/*.veld` when no config exists | reproduced | none | `cmd/veld/init.go:20-33, 651-658` | 1 to 2 h | T5 |
| F-LEAD-01 | generation is not deterministic: `types/index.ts` export order varies (1 of 6 runs by the lead reviewer; 4 distinct hashes in 6 runs by the docs track) | reproduced | none | `internal/emitter/tsshared/types.go:442` | 1 to 2 h | T2 |
| F-FE-01 | `import @models/*` and `@alias/**` are lexed as a block comment; the rest of the file disappears and `validate` reports success (lead: AST shows 0 models); the shipped `examples/microservices` hits it | reproduced | none | `internal/lexer/lexer.go:240-257` | 2 to 4 h | T1 |
| F-MULTI-01 | header names such as `Idempotency-Key` become identifiers in every language; output does not compile | reproduced | none | `internal/parser/parser.go:961` and every emitter | 6 to 12 h | T1 with T4 |
| F-MULTI-02 / F-NODE-19 | all 210 emitter tests are substring checks; no test compiles generated code; the suite is green while most outputs fail to compile | reproduced | none | `internal/emitter/**/*_test.go` | 24 to 40 h (harness) | T4 |
| F-MULTI-03 / F-CLI-20 | default Python output has a `SyntaxError` in every route file with a body and registers no routes (lead reproduced on the `init` scaffold) | reproduced | none | `internal/emitter/backend/python/routes.go:139` | 8 to 14 h | T4 |
| F-MULTI-06 / F-CLI-19 | Go routes: `undefined: mux`; no validation; query and headers `nil` (lead reproduced) | reproduced | low | `backend/go/strategy/plain.go:21,52` | 12 to 20 h | T4 |
| F-NODE-05 | static routes shadowed by an earlier `GET /:id`: a middleware-guarded action is unreachable and another action answers without the guard | reproduced (lead confirmed at source) | high | `backend/node/routes.go:75-87` | 3 to 5 h | T2 |
| F-NODE-06 | generated TypeScript fails `tsc --strict` (4 errors; 5 with `--validate`); React, Vue, Svelte inherit them | reproduced | none | `frontend/typescript/client.go`, `backend/node/routes.go` | 1 to 10 h | T2 |
| F-REG-01 | pre-TOTP "partial" JWT is a full session: 2FA bypass | reproduced (lead confirmed at source) | high | `internal/server/handlers/auth.go:105-117`, `auth/middleware.go:70-100` | 1 to 2 h | T3 |
| F-REG-02 | storage key from unvalidated `name`/`version`: write outside the storage root, overwrite and delete another org's tarball | reproduced (lead confirmed) | high | `handlers/packages.go:122,177`, `storage/storage.go:33-36` | 3 to 6 h | T3 |
| F-REG-03 | token scopes unenforced: read-only token mints admin tokens and deletes versions | reproduced (lead confirmed) | high | `handlers/auth.go:393-439`, `packages.go:276-338` | 4 to 8 h | T3 |
| F-REG-09 | download integrity optional and self-referential; MITM with stripped or replaced header pulled successfully; `http://` accepted | reproduced (lead confirmed) | high | `cmd/veld/registry.go:232-236`, `internal/registry/client.go:208-210` | 6 to 16 h | T3 |
| F-REG-10 | `Unpack`: no limits (1.24 MiB to 1 GiB in 2.9 s; 100,000 files), in-place writes, errors dropped; containment holds by accident | reproduced (lead confirmed) | medium | `internal/registry/tarball.go:98-143` | 4 to 8 h | T3 |
| F-REG-11 | stored XSS sinks across the registry UI; no CSP | source-supported (payloads stored and returned) | high | `internal/server/web/index.html:335-587` | 6 to 12 h | T3 |

(Rows that merge two IDs are counted as separate entries in the CSV; the 25 P0 IDs are: F-CLI-01, 02, 03, 05, 19, 20; F-FE-01, 02; F-LEAD-01; F-MULTI-01, 02, 03, 06; F-NODE-01, 02, 03, 04, 05, 06; F-REG-01, 02, 03, 09, 10, 11.)

### 5.3 P1 (43)

Compiler and tooling (T1): F-FE-03 formatter corrupts files and panics; F-FE-05 first-error-only, line-only, sometimes wrong line; F-FE-18 breaking-change classifier misses enums, nested models, middleware, unions, status changes; F-FE-22 `internal/language` is not the source of truth; dead regex parser.

Emitters (T2, T4): F-NODE-08 validation off by default and documented Zod does not exist; F-NODE-09 `--validate` checks only `typeof`; F-NODE-10 500 responses leak exception text; F-NODE-11 path params not URL-encoded; F-NODE-15 Angular output invalid; F-NODE-17 non-atomic writes, no manifest; F-MULTI-04 Python validators miss the `Decimal` import (every decimal request returns 500); F-MULTI-05 Python service contract inconsistent (ABC returns classes, routes pass dicts; headers empty; query untyped; enum unchecked; malformed JSON gives 500); F-MULTI-07 Rust backend is a stub; F-MULTI-08 Java, C#, PHP do not compile or route; F-MULTI-10 service SDKs (Python rejects its own types; TS 24 strict errors; Go no query); F-MULTI-11 Dockerfile and CI generators download a 404 asset and bundle a default-credential Postgres.

Language server and editors (T5): F-LSP-01 imports never resolved (false errors in multi-file projects); F-LSP-11 VS Code extension re-implements the language and never uses the server; F-LSP-16 language metadata not consumed by the compiler.

Registry (T3): F-REG-04 admin can remove the sole owner; F-REG-05 token expiry unenforced; F-REG-06 private visibility inconsistent and unreachable from the API; F-REG-07 concurrent publish corrupts; F-REG-08 versions mutable, "latest" by publish time; F-REG-12 no rate limiting or TOTP replay protection; F-REG-13 no body limits or archive validation; F-REG-14 sessions not revocable, 7-day JWT; F-REG-15 no TLS option, insecure cookie, CORS `*`, no security headers; F-REG-16 default registry chosen from a Go map; F-REG-20 pull can overwrite project files; F-REG-21 no health endpoint, SIGTERM, migrations table, backups or tests.

CLI, distribution, CI (T5): F-CLI-07 flat config overrides nested, contrary to docs; F-CLI-08 `--out` ignored when config sets `backendConfig.out`; F-CLI-16 npm and pip wrappers (no checksum, `latest` download, swallowed failure, Python 3.8/3.9 syntax); F-CLI-18 CI verifies Linux unit tests only; F-CLI-23 untested registry server and strategy packages.

Documentation (D1 with validators): D-01 Zod claim false everywhere; D-02 determinism claim false; D-03 `veld login/push/pull/serve/doctor` documented as top-level (only `veld registry ...` exists; `doctor` unregistered); D-10 MIT claimed, no LICENSE file.

### 5.4 P2 and P3

P2 (36) and P3 (49) are listed in the CSV. Representative P2: F-FE-07 scalar-type names cannot be field names; F-FE-08 no nested generics; F-FE-09 path parameters never checked against fields; F-FE-13 missing imports silently ignored; F-NODE-07 models as classes; F-NODE-12 lossy query serialization; F-NODE-13 no timeout, fetch injection or auth hook; F-NODE-14 module named `index` clobbers the barrel; F-LSP-03 byte-offset positions; F-LSP-08 single-threaded, no `recover`; F-CLI-09 unknown config keys ignored; F-MULTI-12 SQL/Prisma/GraphQL invalid output. Representative P3: the 26 documentation discrepancies D-11 to D-26 (install names, stale developer guide, roadmap marking done work as TODO, wrong team list), F-CLI-21 hygiene (20 MB binary and scratch directories tracked), F-FE-29 AST JSON `null` collections, F-REG-17 plaintext credentials.

### 5.5 Documentation discrepancies (summary of D-01 to D-26)

The documentation audit inventoried 62 claims across README, CLAUDE.md, docs, the scraped hosted docs page (`test.md`), the website and the editor READMEs, and found 26 mismatches. The most consequential: Zod and Pydantic validation do not exist (D-01, D-08); output is not deterministic (D-02); documented top-level registry commands and `veld doctor` do not exist (D-03); the `tools` config block is parsed and ignored (D-04); GitLab CI generation does not exist (D-05); Rust is Axum not Actix and PHP is Laravel not PSR-15 (D-06); the module path, dependency list and SQLite claims in the architecture overview and developer guide are wrong (D-07); the Python output layout differs from the docs (D-08); VS Code commands do not exist (D-09); no LICENSE file (D-10); five different install names (D-11); getting-started snippets use wrong function names (D-12). Verified true: 8 backends and 10 frontends registered, 6 tools, all 8 backends have service SDK code, 5-platform release builds exist.

Historical plans (for the record, not to inherit): roadmap of April 2026 ran to "~16 weeks remaining" with no team named; execution plans v1 to v4 assumed 16 to 17 weeks with 5 members, planned up to 10 backends and 14 frontends (v1, v2), then cancelled all new emitters (v3), then listed 5 backends and 5 frontends as "supported" (v4, which is byte-identical to `docs/EXECUTION_PLAN.md`). The graduation proposal claims a Zod demo and 188 commits to 19 April 2026; git shows 204 commits to 12 September 2026 by four authors, with 171 of them in March and April.

### 5.6 What to preserve, and debt that may remain

Preserve (evidence that it works): the lexer and parser's structure (linear, fuzz-clean, fast); the validator's rule set and error reporting; the loader's alias mechanism; the emitter registry plumbing (`init()` plus blank import is clean); the Node emitter's route and SDK architecture; the service-SDK design (env var precedence verified for TS, Python, Go); the registry's schema and storage abstraction; the language-metadata generator; the CI drift check idea; the website build.

Debt that can safely remain through graduation: string-building emitters (no template engine needed for a small language); the cubic `extends` cycle check (26 s only at 3,000 chained models); plaintext credential storage (industry-standard for CLIs); the embedded SPA as a plain HTML file once escaping is fixed; `internal/setup` (tsconfig auto-configuration) untouched; tier-2 emitters compiling but not round-trip tested.

Remove rather than fix: `internal/mock`, the Dockerfile/CI/envconfig generators, the Rust backend, the Angular/Dart/Kotlin/Swift emitters (ADR-005), the incremental cache, the dead regex parser in `internal/language`, the tracked binary and scratch directories, `test.md`, the four `.docx` plans, the stale `internal/server/web/index.html` duplicate.



## 06. Refactor versus rewrite, and language decisions

### 6.1 Strategies compared

| Criterion (weight) | S1 Stabilize and incrementally refactor | S2 Selectively replace weak subsystems, keep working components | S3 Rewrite core or whole platform |
|---|---|---|---|
| Time to reliable delivery by 2027-05-31 (0.25) | 4: fixes are small and local, but boundaries stay weak | **5**: fixes plus four targeted replacements (output writer, formatter, VS Code client, registry hardening) fit the budget (chapter 15 §15.2) | 1: 44k lines cannot be re-implemented part-time in 32 weeks while also evaluating and writing |
| Preservation of behaviour and fixtures (0.10) | 5 | 4: conformance fixtures are written before replacements and pin behaviour | 2: behaviour must be re-derived |
| Technical risk (0.15) | 3: destructive output path and three language implementations remain structural risks | **4**: risks removed where they are, nothing else disturbed | 1: unproven foundation on the critical path |
| Team learning and contributor accessibility (0.10) | 4 | 4 | 2: new language or new codebase for everyone |
| Maintainability after graduation (0.15) | 2: emitter matrix and duplicated language logic remain | **4**: tiers, IR, single engine | 5 in theory, unknowable in practice |
| Tooling, packaging, performance (0.05) | 4 | 4 | 3 (depends on language) |
| Plugin support (0.10) | 2: hard to add without an IR | **4**: IR and host are the first replacements | 4 |
| Academic contribution (0.10) | 2: "we fixed bugs" | **4**: measurable before/after on safety, determinism, editor parity, plugin protocol | 3: "we rewrote it" with little evaluation time |
| Weighted score | 3.35 | **4.25** | 2.05 |

Scores are judgements by the lead reviewer on a 1 to 5 scale, informed by the audits; they are not measurements. Sensitivity: if the team survey shows strong Rust or TypeScript skills and weak Go, S3's learning score rises to 4 and its delivery score to 2, giving 2.5; S2 still wins. If the team has only one Go-capable member, S2's delivery score drops to 4 and S1's to 3; S2 still wins. The ranking is robust because S3 loses on the two heaviest criteria regardless of skills.

Recommendation: **S2.** Rejected: S1 because it leaves the four systemic causes (chapter 04 §4.5) in place and offers little to evaluate; S3 because it cannot be delivered and would not fix the actual problems, which are in boundaries and tests, not in the implementation language.

What would reverse it: evidence that the parser or validator is unsalvageable (the frontend audit found the opposite), or a team survey showing that fewer than two members can work in Go.

### 6.2 Language decisions per component

| Component | Current | Decision | Language and stack | Reason | Migration cost | Compatibility impact | Confidence |
|---|---|---|---|---|---|---|---|
| Compiler frontend (lexer, parser, validator, loader) | Go, hand-written | retain, refactor | Go 1.26 | fuzz-clean, fast, small grammar; defects are local (chapter 08 §8.1) | 40 to 70 h for diagnostics, spans, TypeRef | none for users; AST JSON (`veld ast`) changes and is declared unstable | high |
| Semantic IR and compiler API | absent | add | Go; JSON Schema for the serialized form | required by plugins, LSP and diff | 40 to 60 h | new `veld ir` command | high |
| Output writer and emission host | Go, `os.WriteFile` and `RemoveAll` in the CLI | replace | Go | destructive today; needed by plugins | 20 to 30 h | behaviour change: unmanaged files preserved; `clean` only removes manifest files | high |
| Built-in emitters, tier 1 | Go string builders | retain, fix, migrate to IR | Go | close to working for Node/TS; Python and Go fixable in days | 80 to 130 h | output changes (interfaces instead of classes, validation default on) are breaking for existing users and announced in the changelog | high |
| Built-in emitters, tier 2 and 3 | Go | freeze or remove per ADR-005 | Go | unverifiable within capacity | 5 to 8 h to remove | targets disappear from the binary; documented | high |
| External emitter protocol and host | absent | add | Go host; JSON over stdio; TypeScript SDK | chapter 10 | 114 to 192 h | new | medium (new work, scope controlled) |
| Formatter | Go, token-stream | rewrite (small) | Go | corrupts files | 12 to 20 h | `fmt --write` disabled until then | high |
| Diff and lock | Go | refactor onto IR | Go | classifier shallow; gate dead | 10 to 18 h | lock file format changes (versioned) | high |
| Language server | Go, in the CLI binary | retain, extend on the compiler API | Go | the only viable single engine | 60 to 100 h | protocol fixes change client behaviour positively | high |
| VS Code extension | TypeScript re-implementation (1,676 lines) | replace with thin client | TypeScript, `vscode-languageclient` | duplication is the problem | 20 to 40 h | commands finally work | high |
| JetBrains plugin | Kotlin PSI (4,090 lines) | reduce to highlighting plus LSP4IJ, post-Beta | Kotlin | third implementation | 20 to 40 h | limited support at graduation | medium (depends on reserve) |
| Registry client, tarball, credentials | Go (495 lines) | retain, harden, extend (manifest, lockfile, resolver) | Go | small, salvageable | 50 to 80 h | lockfile added; checksum required; `https` default | high |
| Registry server | Go in the CLI binary, no tests | retain architecture, harden, split binary | Go, PostgreSQL, Docker | architecture fine; implementation unsafe | 80 to 140 h | separate deployable; API additive | medium (largest unknown) |
| Registry UI | hand-written HTML/JS, XSS sinks | rewrite rendering layer or reduce to read-only | HTML/JS with escaping, CSP; no framework needed | small surface | 8 to 14 h | none | high |
| Website | Vite, React 19, TypeScript | retain; add CI; LiveDemo either compiled from the Go engine via WebAssembly (stretch) or removed | TypeScript | works; duplicates the generator | 2 to 4 h (CI) | none | high |
| Distribution wrappers | npm, pip, brew, go, nuget, chocolatey, apt, composer | keep npm, brew, go, pip; move the rest to experimental | JavaScript, Python, Ruby formula | chapter 12 | 20 to 30 h | fewer channels | high |
| CI and release | GitHub Actions, hand-written release | retain Actions; adopt GoReleaser | YAML, GoReleaser | signing, SBOM, provenance for free | 12 to 20 h | none | high |

### 6.3 If a rewrite had been recommended

It is not. For completeness, the conditions a rewrite would have needed: a precise failure list that the current implementation cannot fix in place (none found), preservation of the conformance fixtures as the specification (these must be written first in any strategy), a compatibility layer (`veld ast` output and config format), side-by-side comparison on every fixture, a viability milestone before 2026-11-29, abort criteria (fixtures not passing by that date), and a cutoff date after which the old implementation is removed (not before the Beta gate). These conditions are recorded so that any future proposal to rewrite can be judged against them.



## 07. Target architecture, repository layout, interfaces and ADRs

Decision records: ADR-001 to ADR-010 in `adr/`. Diagrams: `diagrams/D1-system-context.mmd`, `D2-compiler-pipeline.mmd`, `D3-plugin-trust.mmd`. Everything marked PROPOSED is design, not implementation.

### 7.1 Boundaries

| Boundary | Owns | Exposes | Depends on | Trust |
|---|---|---|---|---|
| Language specification and conformance fixtures | grammar reference, semantics text, `.veld` fixtures with expected IR and diagnostics, language version | `internal/spec` test suite; `docs/spec/` | nothing | trusted (repo) |
| Compiler frontend (syntax) | lexer, parser, AST with spans | `syntax.Parse(src) (File, []Diagnostic)` | `internal/language` for keyword tables | trusted |
| Semantic analysis and IR | loader (imports, aliases, packages), resolver, validator, normalization to IR | `semantic.Analyze(files, cfg) (*ir.Program, []Diagnostic)`; `ir` types; IR JSON Schema | syntax, language | trusted |
| Compiler application API | one entry point for CLI, LSP and tests: configuration resolution, overlay files, cancellation, diagnostics formatting | `compiler.Compile(ctx, Options) (*Result, error)` | semantic, config, pkg | trusted |
| CLI | cobra commands, exit codes, JSON output, hooks | `veld` binary | compiler, emit, pkg, lsp | trusted |
| Emission host | built-in emitter registry, plugin runner, output staging, manifest, determinism check | `emit.Run(ctx, *ir.Program, Plan) (Report, error)` | ir, plugin protocol types | trusted host; built-ins trusted |
| Built-in emitters and adapters | per-language generation from IR; `strategy/` adapters | `emit.Emitter` interface returning files | ir, lang helpers only (never syntax or semantic) | trusted |
| External emitter protocol, host and SDK | VEP v1 schema, process runner, TypeScript SDK, hello emitter | `--describe`, `--emit` over stdio | ir JSON Schema | plugin process: trusted by installation, not sandboxed |
| Language server and editor clients | LSP server in the binary; VS Code thin client; JetBrains LSP4IJ definition | `veld lsp` | compiler | trusted |
| Contract and package resolution | manifest, lockfile, resolver, cache, registry client, tarball pack/unpack | `pkg.Install`, `pkg.Resolve`, `pkg.Pack` | registry API | downloaded archives are untrusted input |
| Registry service, storage, database, UI | `veld-registry` binary, PostgreSQL schema, blob storage, embedded SPA | `/api/v1` | pkg (tarball, manifest) | server is a trust boundary; uploads are untrusted |
| Distribution, release tooling, docs site, examples | GoReleaser config, wrappers, website, examples | artifacts | everything | release pipeline holds secrets |

Dependency direction (allowed arrows only): `cmd/*` -> `compiler`, `emit`, `pkg`, `lsp`; `lsp` -> `compiler`; `compiler` -> `semantic` -> `syntax` -> `language`; `semantic` -> `ir`; `emit` -> `ir` (and `emit/builtin/*` -> `ir`, `lang`); `emit/plugin` -> `ir`, `emit`; `pkg` -> `ir`? no: `pkg` -> nothing in the compiler except `syntax` for manifest validation of `.veld` entry files; `registry` (server) -> `pkg` for archive and manifest code. Forbidden: any emitter importing `syntax`, `semantic` or `cmd`; `ir` importing anything above it; `registry` importing `compiler`. A Go test using `go list -deps` enforces these rules in CI.

### 7.2 AST versus IR, and where serialization happens

- The AST is syntax: it keeps declaration order, comments (for the formatter, ADR-002 and the formatter rewrite in chapter 08), spans, and unresolved names.
- The IR is semantics: every reference resolved, inheritance flattened with provenance, types as a small recursive `TypeRef` tree, routes with parsed path segments, status codes resolved, defaults typed, deprecations attached, source map to spans.
- Serialization happens only at process and public boundaries: `veld ir` (JSON), the plugin request, conformance fixtures (expected IR JSON), and the LSP wire protocol. Between lexer, parser, semantic analysis, built-in emitters and the LSP, Go values are passed directly. The current "only AST JSON passes between stages" rule in CLAUDE.md is replaced by "only IR crosses a process boundary".

### 7.3 Proposed directory tree

```
Veld/
├── cmd/
│   ├── veld/                      CLI (thin: flags -> compiler/emit/pkg calls; exit codes; --json)
│   ├── veld-registry/             registry server binary (ADR-004)
│   └── generate-language/         language metadata generator (kept)
├── internal/
│   ├── language/                  keyword, directive, type and method tables (consumed by syntax, semantic, lsp)
│   ├── spec/                      conformance suite: testdata/<case>/{input.veld, expected.ir.json, expected.diag.json}
│   ├── syntax/                    lexer.go, parser.go, ast.go, span.go, recover.go  (today: internal/lexer, internal/parser, internal/ast)
│   ├── semantic/                  loader.go (imports, aliases, packages), resolve.go, validate.go, lower.go (AST -> IR)
│   ├── ir/                        types.go, schema.go (JSON Schema embed), version.go, json.go
│   ├── diag/                      Diagnostic{Code, Severity, Span, Message, Hints}; formatters (text, json, lsp)  (today: internal/errors, unused)
│   ├── compiler/                  api.go: Compile(ctx, Options) -> Result; overlay FS; config discovery
│   ├── config/                    kept; precedence fixed (nested wins), DisallowUnknownFields warning
│   ├── emit/
│   │   ├── host.go                plan, run emitters, staging writer, manifest, determinism check, path safety
│   │   ├── emitter.go             Emitter interface returning []OutputFile; capability declaration
│   │   ├── manifest.go            .veld-manifest.json read/write/diff
│   │   ├── plugin/                VEP v1 types, --describe/--emit runner, timeouts, size caps
│   │   └── builtin/
│   │       ├── node/ typescript/ react/ python/ golang/      tier 1
│   │       ├── vue/ svelte/ javascript/ typesonly/ java/ csharp/ php/   tier 2
│   │       └── openapi/ docs/ graphql/ schema/                tools
│   ├── lang/                      per-language identifier, reserved-word, type-mapping helpers (today: internal/emitter/lang, tshelpers, tsshared, sdkhelpers)
│   ├── tools/                     lint/, format/ (rewritten on lossless tokens), diff/ (classifier + lock), cache/
│   ├── pkg/                       manifest.go, lockfile.go, resolve.go, cache.go, tarball.go (hardened), client.go, credentials.go
│   ├── lsp/                       server on compiler API; UTF-16 conversion; debounce
│   └── registry/                  server: auth/, db/ (versioned migrations), handlers/, storage/, email/, web/ (SPA)  (today: internal/server)
├── sdk/
│   └── emitter-ts/                @veld/emitter-sdk: IR types generated from internal/ir schema, defineEmitter, test harness
├── plugins/
│   ├── hello-emitter/             reference external emitter (npm)
│   └── docs-emitter/              docs generator extracted as the second reference plugin (chapter 09 §9.8)
├── editors/
│   ├── vscode/                    thin vscode-languageclient launcher + generated grammar
│   └── jetbrains/                 highlighting PSI + LSP4IJ server definition
├── deploy/
│   ├── docker-compose.yml         registry + postgres + caddy (chapter 13)
│   └── runbook/                   install, backup, restore, upgrade
├── packages/                      npm, pip, homebrew, go wrappers (others moved to packages/experimental/)
├── website/                       docs and marketing site; LiveDemo uses the Go compiler compiled to WebAssembly (stretch) or is removed
├── examples/                      one per tier-1 pair; each is generated and compiled in CI
├── docs/                          spec/, guides/, adr/, review-2026-10/ (this review), report/
├── evaluation/                    scripts and corpora for chapter 03 §3.9
├── .goreleaser.yaml, CHANGELOG.md, LICENSE, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, CODEOWNERS
└── go.mod (go 1.26), package.json (npm workspaces: editors/vscode, website, sdk/emitter-ts, plugins/*)
```

Migration from the current tree is mostly package moves with `gofmt -r`-style import rewrites; it is scheduled in M2 alongside the IR introduction so that each package moves once. Current packages that are deleted rather than moved: `internal/mock`, `internal/generators/{dockerfile,cicd,envconfig,scaffold}` (scaffold folded into `cmd/veld/init`), `internal/emitter/backend/rust`, `internal/emitter/frontend/{angular,dart,kotlin,swift}` (frozen, ADR-005).

### 7.4 Key interfaces (proposed Go signatures)

```go
// internal/compiler
type Options struct {
    ConfigPath string              // explicit, or discovered upward from WorkDir
    WorkDir    string
    Overlay    map[string]string   // absolute path -> unsaved content (LSP)
    Flags      config.FlagOverrides
}
type Result struct {
    Program     *ir.Program        // nil when errors exist
    Diagnostics []diag.Diagnostic  // all stages, sorted by file then span
    Config      config.Resolved
    Files       []string           // every .veld file read (for watch)
}
func Compile(ctx context.Context, o Options) (*Result, error)   // error only for I/O or config failures

// internal/emit
type OutputFile struct { Path string; Content []byte; Mode os.FileMode }
type Capabilities struct { Headers, Maps, RecursiveModels, Middleware, WebSocket, StatusCodes bool; Extends string /* "preserve"|"flatten" */ }
type Emitter interface {
    Name() string; Version() string; Kind() Kind; Capabilities() Capabilities
    Emit(ctx context.Context, p *ir.Program, opts Options) ([]OutputFile, []diag.Diagnostic, error)
}
type Plan struct { Targets []Target /* emitter + outDir + options */; DryRun, CheckDeterminism bool }
func Run(ctx context.Context, p *ir.Program, plan Plan) (Report, error)   // staging, manifest, swap

// internal/ir (excerpt)
type Program struct { IRVersion string; LangVersion string; Models []Model; Enums []Enum; Services []Service; Constants []Constant; SourceMap SourceMap }
type TypeRef struct { Kind TypeKind /* Scalar, Array, Map, Ref, Optional */; Scalar Scalar; Elem *TypeRef; Key *TypeRef; Ref string }
type Operation struct { Name, Method, Path string; Segments []PathSegment; PathParams []Param; Query *ModelRef; Headers []HeaderParam; Input *TypeRef; Outputs []Output; Middleware []string; Deprecated string; Span diag.Span }
```

### 7.5 Deployables, processes and products

| Product | Process | Versioned independently | Repository |
|---|---|---|---|
| `veld` CLI (includes `veld lsp`) | one process; spawns plugin processes and `postGenerate` hooks | yes (`vX.Y.Z`) | monorepo |
| `veld-registry` | one process plus PostgreSQL; packaged as Docker image | yes (`registry/vX.Y.Z`) | monorepo |
| `@veld/emitter-sdk` | library | yes (`sdk/vX.Y.Z`) | monorepo |
| VS Code extension | runs inside VS Code; spawns `veld lsp` | yes | monorepo |
| JetBrains plugin | runs inside the IDE; LSP4IJ spawns `veld lsp` | yes | monorepo |
| Website | static site | not versioned | monorepo |

Modular monolith for the compiler (one binary, strict internal boundaries enforced by tests), separate deployable for the registry, one repository (ADR-010). More separation than this would add release coordination without a corresponding benefit for a six-person team.

### 7.6 How a contributor adds an emitter or an editor integration without touching compiler semantics

- External emitter: follow chapter 10 §10.4; no change to the Veld repository at all.
- Built-in emitter (tier 2 proposal): add `internal/emit/builtin/<name>/` implementing `emit.Emitter`, register it in `internal/emit/builtin/registry.go` (one line), add golden outputs for every conformance fixture, add a compile step to the nightly job. The import-boundary test fails if the new package imports `syntax` or `semantic`.
- Editor integration: configure the editor's LSP client to run `veld lsp`; nothing in the compiler changes. Syntax highlighting grammars are generated from `internal/language` by `cmd/generate-language`, so a new editor adds one template there.

### 7.7 Versioning and compatibility

See ADR-009 for the full table. In one sentence: the language, compiler, IR, plugin protocol, registry API, package manifest, lockfile and generated artifacts each carry their own version field; the CLI is the single place that negotiates plugin compatibility, and the server is the single place that validates manifest versions.

### 7.8 ADR index

| ADR | Title | Status |
|---|---|---|
| ADR-001 | Retain Go for compiler, CLI, LSP, registry | proposed |
| ADR-002 | Separate AST from versioned IR; serialize only at process boundaries | proposed |
| ADR-003 | External emitters: JSON over stdio, host-mediated writes; Wasm deferred | proposed |
| ADR-004 | Registry server as its own binary | proposed |
| ADR-005 | Emitter support tiers enforced by CI | proposed |
| ADR-006 | LSP as single language engine; VS Code primary; JetBrains via LSP4IJ later | proposed |
| ADR-007 | Output manifest, staged writes, never RemoveAll | proposed |
| ADR-008 | Package manifest and lockfile; registry data-only | proposed |
| ADR-009 | Versioning scheme | proposed |
| ADR-010 | Monorepo, one Go module, npm workspace | proposed |

All ADRs are to be accepted or amended by the team in Sprint 0 (by 2026-11-01). D1 edits them for clarity; the deciders listed in each ADR own the content.



## 08. Compiler, language and CLI plan

Evidence: compiler-frontend audit track (72 adversarial cases, 1.67 million Go-fuzz executions over lexer, parser, validator, lint and diff with no panic or hang; F-FE-01 to F-FE-29), CLI audit (F-CLI items), lead-reviewer reproduction of F-FE-01 (`import @models/*` makes `veld validate` report a file with zero models as valid).

### 8.1 What is retained and why

| Component | Verdict | Evidence |
|---|---|---|
| Lexer (`internal/lexer`) | retain, fix | linear, no recursion, fuzz-clean; defects are local: wildcard import lexed as a block comment (`lexer.go:240-257` before the path branch at `:315`, F-FE-01), unterminated strings and comments accepted (F-FE-04), no escapes (F-FE-24), BOM rejected (F-FE-15), first error only and line-only positions (F-FE-05, F-FE-28) |
| Parser (`internal/parser`) | retain, refactor | small grammar, non-recursive on braces (200,000-deep `{` rejected cleanly); needs error recovery, spans, a recursive `TypeRef` (today `IsArray`/`IsMap` flags, `Map<string,int>[]` sets both, F-FE-08), reserved-word handling for field and enum names (F-FE-07), header identifiers (F-MULTI-01 at `parser.go:961`), directive order and duplicates (F-FE-23) |
| Validator (`internal/validator`) | retain, extend | reports all errors with correct `file:line`, suggestions, enum-default membership, extends cycles, cross-module route conflicts; gaps are missing checks: path params against fields (F-FE-09), `@default` on arrays and maps (F-FE-10), undefined union members (F-FE-11), extends field conflicts and cubic cycle check (F-FE-12), reserved model names (F-NODE-14) |
| Loader (`internal/loader`) | retain, fix | missing imports silently ignored with directory fallback (F-FE-13), `validate` and `ast` ignore aliases (F-FE-14) |
| Typed errors (`internal/errors`) | wire in | unused today (F-FE-06); becomes `internal/diag` |
| Language tables (`internal/language`) | wire in | not consumed by the compiler (F-FE-22, F-LSP-16); dead regex parser deleted |
| Formatter (`internal/format`) | rewrite | `--write` drops comments and quotes and glues tokens; corrupts the shipped example; panics on `model }` (F-FE-03) |
| Diff and lock (`internal/diff`) | refactor | classifier misses enums, nested models, middleware, unions; status change non-breaking (F-FE-18); stack overflow on cyclic extends (F-FE-19); gate dead because the lock is deleted first (F-FE-02, F-CLI-01) |
| Lint | fix | unreachable error-level rules, false positive on nested models (F-FE-20, F-CLI-12) |
| Cache | simplify | wiped by `runClean`; keyed on `.veld` content only (F-FE-21); incremental mode is not justified for inputs that compile in under a second (5 MB file validates in under 1 s); remove `--incremental` and the cache file |

No frontend rewrite is warranted. The audit's own words: the validator "is the strongest piece".

### 8.2 Pipeline and responsibilities (target)

```
sources + overlay ─► syntax.Parse (per file; error recovery; spans)
                  ─► semantic.Load (imports, aliases, packages from lockfile; cycle detection)
                  ─► semantic.Resolve (symbols, inheritance, references)
                  ─► semantic.Validate (all rules; all diagnostics collected)
                  ─► semantic.Lower (AST -> ir.Program; normalization; source map)
                  ─► compiler.Result {Program, Diagnostics}
CLI: generate ─► tools/diff against lock ─► emit.Run (built-ins + plugins) ─► manifest ─► lock written
LSP: Compile with overlay ─► diagnostics; symbols from Program for hover/definition/completion
```

Responsibilities:

- Parsing: one file at a time, never I/O; recovers at the next `model`/`enum`/`module`/`action`/`}` boundary and continues, so one file yields many diagnostics.
- Spans: every token and AST node records byte offset, line and column (0-based UTF-8 byte column; converters for UTF-16 live in `diag`).
- Name resolution: a symbol table per program; imports resolve to files through aliases and packages; a missing import is an error, never a directory scan.
- Type checking: `TypeRef` tree; scalars from `internal/language`; `Map` keys limited to `string` in lang 1.0; nested arrays and maps allowed (`string[][]`, `Map<string, T[]>`); recursion allowed through models only.
- Normalization: inheritance flattened with provenance; path segments parsed; status codes defaulted (POST 201, DELETE without output 204, else 200) and recorded explicitly in the IR so emitters never re-derive them; header wire names kept with a derived identifier.
- Diagnostics: `diag.Diagnostic{Code, Severity, Span, Message, Hints, Stage}`; codes stable (`E0101` unknown type ...); machine-readable with `--json`; text format `file:line:col: error[E0101]: message` plus a source snippet.
- Dependency graph: file import graph and workspace `consumes` graph both checked for cycles (F-FE-27).
- Emission: chapter 09 and ADR-007.

### 8.3 IR v1 (excerpt of the JSON shape)

```jsonc
{
  "irVersion": "1.0", "langVersion": "1.0",
  "enums": [{ "name": "Role", "members": ["admin", "user"], "span": {"file": "veld/models/user.veld", "line": 3, "col": 1} }],
  "models": [{
    "name": "User", "extends": ["BaseEntity"], "description": "...",
    "fields": [
      { "name": "id", "wireName": "id", "type": { "kind": "scalar", "scalar": "uuid" }, "optional": false, "from": "BaseEntity" },
      { "name": "tags", "wireName": "tags", "type": { "kind": "array", "elem": { "kind": "scalar", "scalar": "string" } }, "optional": true },
      { "name": "role", "wireName": "role", "type": { "kind": "ref", "ref": "Role" }, "default": { "kind": "enum", "value": "user" } },
      { "name": "meta", "wireName": "meta", "type": { "kind": "map", "key": { "kind": "scalar", "scalar": "string" }, "value": { "kind": "scalar", "scalar": "string" } } }
    ]
  }],
  "services": [{
    "name": "Users", "prefix": "/api",
    "operations": [{
      "name": "GetUser", "method": "GET", "path": "/users/:id",
      "segments": [{ "kind": "static", "value": "users" }, { "kind": "param", "name": "id", "type": { "kind": "scalar", "scalar": "uuid" } }],
      "query": null, "headers": [{ "wireName": "Idempotency-Key", "name": "idempotencyKey", "type": { "kind": "scalar", "scalar": "string" }, "optional": false }],
      "input": null, "outputs": [{ "status": 200, "type": { "kind": "ref", "ref": "User" } }],
      "middleware": ["AuthGuard"], "deprecated": "", "span": { "file": "...", "line": 12, "col": 3 }
    }]
  }]
}
```

The JSON Schema for this document is generated from the Go types and published at `internal/ir/schema/ir-1.0.json`; the TypeScript SDK's types are generated from the schema.

### 8.4 Language features in the graduation release (lang 1.0)

| Construct | Semantics in 1.0 | Status |
|---|---|---|
| `model` with scalar fields `string int float decimal bool date datetime uuid` | `int` is a 64-bit integer, documented as 53-bit safe for JavaScript clients; `decimal` is a decimal string on the wire; `date` is `YYYY-MM-DD`; `datetime` is RFC 3339 with offset; `uuid` is RFC 4122 text | keep; fix date/datetime distinction in Go |
| `field?: T` | absent allowed; `null` is not a valid value for an optional field (validators reject it) | keep; enforce |
| `T[]`, `Map<string, T>`, nesting | arrays of arrays and maps of arrays allowed | extend (F-FE-08) |
| `extends` | single inheritance, no field override with a different type, no cycles | keep; add override check |
| `enum` | string members; wire value equals the member name; member names may be any identifier including keywords | keep; fix F-FE-07 |
| `@default(v)` | typed literal or enum member; applies to generated validators and documentation, not to databases | keep; complete checks |
| `@deprecated "msg"` on fields and actions | emitted as doc annotations | keep |
| `module { prefix, description }` and `action { method, path, input, output(:status), query, headers, middleware }` | one success output per action; `headers` keys are wire names with derived identifiers; `middleware` is a named hook the adapter wires; path params typed from the input or query model or default `string` | keep; define rules; fix header identifiers |
| `import @alias/path`, `import "./relative.veld"`, `import @alias/*` | all three forms parse; wildcard is a real token, not a comment | fix F-FE-01 |
| constants | typed literals | keep; tighten |
| `version` header statement (`veld 1`) | optional | add |
| Deferred from 1.0 | union types (parser accepts them today with weak validation; freeze behind a warning), WebSocket actions (`WS`, `stream:`, `emit:`), error output declarations (`errors:`), per-field constraints (min, max, pattern), nullable as distinct from optional, custom scalars, generics beyond `Map`/arrays, HEAD/OPTIONS methods (parser accepts; emitters inconsistent) | spec says "reserved for a future version"; compiler emits `W_DEFERRED_FEATURE` and emitters refuse |

### 8.5 Reusable compiler APIs, determinism and configuration

- `compiler.Compile` is the only entry point used by `veld validate`, `generate`, `lint`, `fmt --check`, `diff`, `lsp` and tests. `veld validate` therefore honours aliases (fixes F-FE-14) and workspace config exactly as `generate` does.
- Cancellation: every public function takes `context.Context`; the LSP cancels superseded compiles.
- Incremental compilation: not justified (inputs compile in well under a second); removed along with `.veld-cache.json`.
- Determinism: the IR has no map-ordered collections; emitters iterate sorted slices; a CI test generates every fixture twice and on two operating systems and byte-compares (F-NODE determinism finding reproduced in `types/index.ts`).
- Portable paths: all paths in IR and manifests are forward-slash relative; absolute paths appear only in `--verbose` output.
- Environment independence: generated output never contains absolute paths, timestamps, user names, hostnames or toolchain versions beyond the Veld version header. Local toolchain diagnostics (`veld doctor`) are printed, never embedded.
- Configuration: nested form wins over flat (fixes F-CLI-07, matching the documentation); unknown keys produce a warning (F-CLI-09); `--out` overrides every configured output directory (F-CLI-08); `$schema` validated; comments and trailing commas rejected with line numbers; config discovered in `.` then `./veld` then upward to the repository root (documented).
- Machine-readable output: `--json` on `validate`, `generate`, `lint`, `diff`, `doctor`, `--version`; `NO_COLOR` and non-TTY detection; exit codes `0` ok, `1` diagnostics, `2` usage or config error, `3` plugin failure, `4` breaking change under `--strict`.
- API compatibility checks: `veld diff` uses the IR (not the AST) so enums, nested models, headers, middleware and status codes are compared (fixes F-FE-18); classification table published in the docs.

### 8.6 Safe generation (ADR-007) and the CLI surface

| Behaviour | Target |
|---|---|
| Order | compile -> diff against lock -> gate -> emit to staging -> swap -> manifest -> lock -> hooks |
| Manifest | `.veld-manifest.json`: versions, emitter list, files with SHA-256 |
| Ownership conflicts | a generated path that exists but is not in the previous manifest stops generation (`E_UNMANAGED_FILE`), unless `--overwrite-unmanaged` |
| Interrupted generation | staging directory is removed on failure; previous output untouched |
| Dry run | full pipeline, prints planned files and a diff; writes nothing, not even cache or lock (fixes F-CLI-06) |
| Clean | deletes manifest-listed files and then empty directories; refuses unmanaged non-empty directories (fixes F-CLI-03) |
| Hand-written code | never inside the output directory; the generated README says so; `veld diff` lists hand-modified generated files |
| `init` | never overwrites existing files (`O_EXCL`), fixes F-CLI-05 |
| `watch` | debounced compile; on error prints diagnostics and keeps the previous output (fixes F-NODE-03); handles SIGTERM |
| hooks | `postGenerate` run through the platform shell (`cmd /C` on Windows, fixes F-CLI-10); failure exits non-zero; `--no-hooks` |
| `doctor` | registered (fixes F-CLI-04); reports toolchains, plugins, config, tier of each configured emitter |
| Registry commands | top-level aliases `veld login/logout/push/pull` kept thin over `veld registry ...` (fixes F-CLI-14) |
| Version | from build info when not stamped (fixes F-CLI-13) |

### 8.7 Formatter rewrite (small, self-contained)

Lossless token stream (comments and strings preserved verbatim), indentation by block depth, alignment of field types within a model, idempotency test on every conformance fixture, `fmt --check` for CI. Estimate 12 to 20 h (T1 or T5), scheduled in M4 because it is not on the critical path; until then `veld fmt --write` is disabled with a message (it corrupts files today).

### 8.8 Work breakdown (compiler and CLI)

| Item | Finding(s) | Estimate | Sprint | Owner / reviewer |
|---|---|---|---|---|
| Stop `runClean` before compile; staged writer; manifest; path safety; dry-run and clean semantics | F-NODE-01..04, 16, 17; F-CLI-01..03, 05, 06 | 20 to 30 h | S1 to S2 | T2 / T1 |
| Wildcard import token; unterminated string/comment errors; BOM; escapes | F-FE-01, 04, 15, 24 | 4 to 8 h | S1 | T1 / T5 |
| Header identifier derivation at parser/IR level | F-MULTI-01 | 6 to 12 h | S1 | T1 / T4 |
| Structured diagnostics (`diag`), spans, error recovery, all-errors reporting | F-FE-05, 06, 28; F-LSP-04 | 16 to 28 h | S3 to S4 | T1 / T5 |
| `TypeRef` tree, nested generics, reserved-word identifiers | F-FE-07, 08 | 12 to 24 h | S3 to S5 | T1 / T2 |
| IR v1, lowering, JSON Schema, `veld ir`, conformance suite | ADR-002 | 40 to 60 h | S3 to S5 | T1 / T4 |
| Validator extensions (path params, defaults, unions, extends conflicts, reserved names, workspace cycles) | F-FE-09..12, 27; F-NODE-14 | 12 to 20 h | S4 to S5 | T1 / T3 |
| Loader strictness and alias consistency | F-FE-13, 14 | 4 to 7 h | S2 | T1 / T5 |
| `internal/language` as the real source of truth plus parser conformance test | F-FE-22, F-LSP-16 | 6 to 10 h | S4 | T5 / T1 |
| Diff on IR; classifier completeness; gate ordering; lock never deleted | F-FE-02, 18, 19 | 10 to 18 h | S5 (after IR) | T1 / T2 |
| Config precedence, unknown keys, `--out`, JSON errors with line numbers | F-CLI-07, 08, 09 | 5 to 9 h | S2 | T5 / T1 |
| CLI: `--json`, exit codes, `NO_COLOR`, `doctor`, aliases, version, hooks | F-CLI-04, 10, 11, 13, 14 | 10 to 16 h | S8 to S9 | T5 / T2 |
| Formatter rewrite | F-FE-03 | 12 to 20 h | S10 to S11 | T1 / T5 |
| Remove incremental cache and `internal/mock`; freeze deferred syntax behind warnings | F-FE-21, F-MULTI-14 | 3 to 5 h | S3 | T1 / T4 |

Total: 160 to 267 h. The IR and diagnostics items are the critical path for the plugin system (chapter 10) and the LSP (chapter 11).



## 09. Built-in emitters and generated-code quality

Decision record: ADR-005 (tiers), ADR-002 (IR), ADR-007 (output writer). Evidence: Node/TS and multi-emitter audit tracks; chapter 05 F-NODE and F-MULTI items. The lead reviewer reproduced the Python and Go compile failures on the `veld init` scaffold (chapter 02).

### 9.1 Inventory at 6a47f81 (verified)

| Target | Registry name | Framework adapters (`strategy/`) | Runtime deps in output | Compiles? | Runs round trip? | Validation | Notes |
|---|---|---|---|---|---|---|---|
| Node backend | `node-ts` (alias `node`), `node-js` | plain, express | none by default; `express` adapter adds `import('express')` type references | `tsc --strict` fails with 4 errors (5 with `--validate`) (F-NODE-06); runtime works under `tsx` | GET/POST 201/PUT/PATCH/DELETE 204/headers work; static routes shadowed by `/:id` (F-NODE-05); 500 leaks error text (F-NODE-10) | off by default; `--validate` emits `_validators.ts` with `typeof` checks only (F-NODE-08, 09); no Zod anywhere | models are classes with getters/setters and branded scalars (F-NODE-07) |
| TypeScript client | `typescript` | n/a | none | fails strict `tsc` (shared errors) | path params not encoded (F-NODE-11); arrays serialized as `x,y` (F-NODE-12); no timeout/abort/auth hook (F-NODE-13) | n/a | `VeldApiError` with status, body, code |
| React / Vue / Svelte | `react`, `vue`, `svelte` | n/a | `@tanstack/react-query`, `vue`, `svelte/store` as expected | compile once F-NODE-06 is fixed | wrap the TS SDK | n/a | client directory duplicated per framework |
| Angular | `angular` | n/a | `@angular/core`, HttpClient | invalid TypeScript in `models/index.ts`; headers dropped (F-NODE-15) | no | n/a | re-implements the SDK instead of wrapping it |
| JavaScript frontend / backend | `javascript`, `node-js` | plain | none | frontend still emits `.ts` files; backend fails `node --check` at `types/items.js:141` | no | no | |
| Types only | `typesonly` | n/a | none | shares the TS type emitter | n/a | n/a | |
| Python backend | `python` | plain, flask | `flask` only with the adapter | `SyntaxError` in routes (F-MULTI-03); with `--validate` a second syntax error from hyphenated header names (F-MULTI-01); `Decimal` import missing (F-MULTI-04) | after two manual patches: 201, 400, 204 correct; headers arrive empty; query untyped; unknown enum accepted; malformed JSON gives 500 (F-MULTI-05) | `--validate` emits `_validators.py`; no Pydantic anywhere despite docs | ABC returns model classes while routes require dicts |
| Go backend | `go` | plain (net/http), chi | `chi` only with the adapter; ships `go.mod` | `undefined: mux` (F-MULTI-06); header-name syntax error | after patches: 201, 204, 400 on malformed JSON; `{}` accepted (no required-field validation); query and headers passed as `nil`; `date` and `datetime` both map to `time.Time`, so `"2020-01-01"` fails to decode | none | |
| Rust backend | `rust` | axum (docs say Actix) | `axum` not even declared in Cargo.toml | cannot compile by reading (F-MULTI-07) | no | none | stub: `router()` returns `()`, empty `main` |
| Java backend | `java` | spring (per docs) | none emitted | `javac` 8 errors (headers) then 2 more (`Role role = "user"`, missing return) (F-MULTI-08) | no | none | plain classes, hardcoded package `maayn.veld.generated` |
| C# backend | `csharp` | aspnet (per docs) | none emitted | cannot compile by reading (`Role Role = "user"`; no `ControllerBase`) | no | none | |
| PHP backend | `php` | laravel (docs say PSR-15) | none | cannot compile by reading (`->validate()` on an array) | no | none | `{id}` registered before `/old` and `/tree` (shadowing) |
| Dart / Kotlin / Swift clients | `dart`, `kotlin`, `swift` | n/a | none | defects by reading: Swift path `"${id}"` literal, Kotlin `BigDecimal` in `@Serializable` without serializer and query params "omitted for brevity", Dart `fromJson` assigns strings to enums (F-MULTI-09) | no | n/a | not compiled (no toolchains in this environment) |
| Service SDKs (`consumes`) | all 8 backends | n/a | none | Go SDK builds and vets; Python SDK compiles but rejects its own model types; TS SDK 24 strict errors (F-MULTI-10); Rust/Java/C#/PHP SDKs read only | env-var precedence verified for TS, Python, Go | n/a | WebSocket actions (`method: WS`, `stream:`, `emit:`) exist in the parser and are skipped by SDKs; undocumented |
| OpenAPI | `openapi` | n/a | n/a | valid 3.0.3 per `redocly lint` (style warnings only) | n/a | n/a | no servers/security; enum defaults dropped; error shape differs from runtime (F-MULTI-13) |
| GraphQL SDL | `graphql` | n/a | n/a | `buildSchema` throws on hyphenated header field (F-MULTI-12) | n/a | n/a | |
| SQL / Prisma | `schema` | n/a | n/a | every DTO becomes a table; `DEFAULT user` unquoted (Postgres reads `current_user`); `limit` unquoted; Prisma models have no `@id` (F-MULTI-12) | n/a | n/a | |
| Docs | `docs` | n/a | n/a | untested package (1,864 lines, 0 tests) | n/a | n/a | |
| Dockerfile / CI / envconfig | `dockerfile`, `cicd`, `envconfig` | n/a | n/a | Dockerfile and CI download `releases/latest/download/veld-linux-amd64`, which returns 404; compose bundles a registry Postgres with password `changeme` on a published port (F-MULTI-11) | n/a | n/a | |
| Mock | (none) | n/a | n/a | `internal/mock` is not imported by any command (F-MULTI-14) | n/a | n/a | dead code |

Tests: 523 test functions in the repository; 210 under `internal/emitter`; every emitter test is a substring assertion; no test compiles or runs generated output (F-MULTI-02, F-NODE-19). This is the single systemic cause of the table above.

### 9.2 Sustainable supported subset

See ADR-005 for the tier table and rules. Rationale in numbers:

| Scope | Monthly maintenance (audit estimate) | Share of team capacity (about 150 h/month committed technical, chapter 15) |
|---|---|---|
| Full matrix (8 backends, 10 frontends, 6 tools) | 75 to 110 h | 50 to 73 percent |
| Tier 1 only (3 backends, 2 frontends, 3 service SDKs, OpenAPI) | 25 to 35 h | 17 to 23 percent |
| Tier 1 plus tier-2 compile-only | 40 to 55 h | 27 to 37 percent |

Assumptions: one owner per emitter, one upstream breaking change per quarter (TypeScript, Flask, chi, React Query releases), CI toolchains installed by setup actions. The tier-1 share is the maximum a team that also has to deliver a plugin system, a registry and a report can sustain.

### 9.3 Boundaries and the meaning of "zero runtime dependencies"

| Layer | What Veld generates | Dependency rule | Status today |
|---|---|---|---|
| Types | interfaces, enums, scalar aliases | none, ever | true for TS; Python/Go types have the header-name and date bugs |
| Transport (clients) | request building, path and query encoding, error type | standard library only (`fetch`, `urllib`, `net/http`) | true, with encoding bugs |
| Validation | generated validator functions per model | generated code, no library; default ON after M1 | off by default; shallow |
| Routes (language level) | handler functions that bind path, query, headers, body and call the service interface | none beyond the language's HTTP primitives | Express-shaped assumptions in Node; broken in Python and Go |
| Framework adapters | `strategy/` packages that register handlers on a specific router | may import only the framework the user already installed; never a Veld runtime | express, flask, chi present; others documented but not implemented |
| Optional integrations | React Query hooks, Vue composables, Svelte stores | import the named library; experimental tier | wrap the TS SDK |

Precise claim to publish: "Generated types, clients and validators import nothing outside the language's standard library. Route adapters import only the framework you chose."

Unsupported semantics must fail loudly: the capability declaration from the plugin manifest (chapter 10) is also used for built-ins. Each built-in declares what it supports (headers, maps, recursive models, WebSocket, middleware, status codes); the compiler reports an error, not a silent omission, when a contract uses something the chosen emitter declares unsupported. Example: `WS` actions with the Python emitter produce `E_UNSUPPORTED_FEATURE` naming the action and the emitter.

### 9.4 Required properties for every tier-1 emitter

1. Capability declaration (above).
2. Stable type mapping table (chapter 08 section 8.4) applied through `internal/ir` type references, not per-emitter string matching. `date` and `datetime` are distinct (Go: `civil.Date`-style string wrapper or `string` for date, `time.Time` for datetime); `decimal` is a string on the wire in every language; `int` is 64-bit where the language allows and documented as 53-bit safe for JavaScript.
3. Naming: identifiers derived through one function per language (`lang.Ident(name, kind)`) that escapes reserved words and non-identifier characters (`Idempotency-Key` to `idempotencyKey` plus the original wire name kept as metadata). Collisions with language globals (`Date`, `Error`, `Map`) are validator errors with a suggestion.
4. Imports computed from the IR, never hardcoded.
5. Deterministic formatting: files assembled from sorted collections; a determinism test runs each emitter twice on every fixture.
6. Route ordering: static segments before parametric ones per method, and a validator error for overlapping routes (fixes F-NODE-05 and the PHP shadowing at once because ordering lives in the IR, not in emitters).
7. Error responses: one documented shape `{ "error": { "code", "message", "details"? } }`; `500` never includes exception text.
8. Runtime version policy: Node 20+, Python 3.10+, Go 1.26+ (matches the compiler's own Go policy); TypeScript 5.x; declared in the README matrix and tested in CI.
9. Validation default ON with `--no-validate` to opt out; validators check enums, element types, integers, `uuid`/`date`/`datetime` formats, optional versus `null`, and coerce query strings.

### 9.5 Testing beyond snapshots

| Level | What | Tier | Mechanism |
|---|---|---|---|
| Conformance fixtures | 30 to 40 `.veld` inputs with expected IR JSON and expected diagnostics | all | `go test ./internal/spec` |
| Golden output | per emitter, per fixture, the generated files | 1 and 2 | golden files with `-update`; reviewed in PRs |
| Compile | `tsc --strict`, `python -m compileall` plus `mypy --strict` on types, `go vet`, `javac`, `dotnet build`, `php -l` | 1 fast, 2 nightly | one Go test per emitter that shells out, skipped when the toolchain is absent (`testing.Short`), required in CI |
| Semantic assertions | parse the generated file and assert structure (for example, route registration count equals operation count; validator rejects an invalid enum) | 1 | language-native test files emitted alongside fixtures |
| Round trip | start the generated server with a reference service implementation, call it with the generated client, assert status codes, bodies, error shape, encoding of `a/b?x` path params, array query params, headers | 1 | `test/e2e/<lang>/` harness, run in CI on three OSes |
| Cross-language interoperability | Node server called by the Python and Go clients and the reverse, using one fixture | 1 | same harness, matrix job |
| Error cases | invalid body, unknown enum, missing required, oversized body, malformed JSON | 1 | round trip |
| Reproducibility | generate twice, byte-compare; generate on Windows and Linux, byte-compare | all registered | CI job |

Estimated cost to build the harness: 40 to 60 h (T4 with T2), then 2 to 4 h per new fixture. This is the second item on the critical path after the output writer, because every later emitter fix is verified through it.

### 9.6 Placement of auxiliary generators

| Generator | Decision | Reason |
|---|---|---|
| OpenAPI | core, tier 1 | interoperability escape hatch (chapter 03); already valid 3.0.3; add servers/security/defaults, align the error schema; 3.1 output as a small follow-up |
| GraphQL SDL | optional, tier 2 | fix via the identifier function; no further investment |
| SQL / Prisma | optional, tier 2 after quoting fixes (2 to 4 h) and a `@entity`-style opt-in so DTOs are not tables | tables from DTOs is wrong by construction; keep only as an opt-in sketch |
| Docs | optional, tier 2 | add tests; useful for the report and website |
| Mock server | remove | dead code; TypeSpec, Prism and others already do this |
| Dockerfile, CI, envconfig | remove from the release binary | they encode release URLs that do not exist and bundle insecure defaults; a project template in `examples/` serves the same purpose |

### 9.7 Sequence (see chapter 15 for dates)

1. M1: output writer (ADR-007); fix F-MULTI-01 header identifiers at the parser/IR level; fix Python syntax error and `Decimal` import; fix Go `mux`; fix Node route ordering, 500 leak, path encoding, strict-mode errors; compile tests for the three tier-1 backends in CI. Target: the vertical slice compiles and round-trips in CI by 2026-11-29.
2. M2: IR migration of tier-1 emitters; validation default ON and deepened; tier-3 code removed; tier-2 nightly compile job.
3. M4: React/Vue/Svelte on the fixed TS SDK; Java/C#/PHP compile-only or demoted; OpenAPI completeness.
4. Beta gate: tier-1 round trip green on three OSes for two sprints.

### 9.8 Candidate for extraction to an external plugin (chapter 10 acceptance test)

The `docs` generator (Markdown API documentation) is the best candidate: self-contained, no toolchain, useful, and currently untested. Extracting it to `plugins/veld-emitter-docs` proves the protocol on a real emitter without touching the tier-1 path. Second candidate, post-graduation: Dart client, as a community-maintained plugin.



## 10. External Emitter System (Veld Emitter Protocol, VEP v1)

**Status:** PROPOSED DESIGN. Nothing in this chapter exists in the reviewed revision (6a47f81). The only extension point today is the in-process Go registry in `internal/emitter/emitter.go:90-118` (`RegisterBackend` / `RegisterFrontend` / `RegisterTool`, populated from `init()` functions and blank imports in `cmd/veld/main.go`). That is a built-in registry, not a plugin system: adding an emitter requires editing and rebuilding the compiler.

### 10.1 Problem addressed

- Veld's value proposition depends on supporting many targets, but every target is a Go package compiled into the binary. The team cannot maintain 20+ emitters, and third parties cannot add one without a fork.
- The emitter interface (`Emit(a ast.AST, outDir string, opts EmitOptions) error`, `emitter.go:43-45`) hands emitters the raw AST and a writable directory. Emitters write files themselves, so the host cannot enforce output ownership, determinism, or path containment.
- There is no serialized, versioned intermediate representation (IR). `veld ast` dumps the Go AST struct as JSON, which is an implementation detail, not a contract.

### 10.2 Options compared

| Option | Portability | Host complexity | Plugin-author complexity | Trust / isolation | Verdict |
|---|---|---|---|---|---|
| In-process Go plugins (`plugin` package) | Linux/macOS only; must match exact Go toolchain and dependency versions | Low | High (Go only, rebuild per Veld version) | None (same process) | Rejected. Unsupported on Windows, version-fragile. |
| HashiCorp go-plugin (gRPC over local socket) | All OS | Medium (gRPC codegen, handshake) | Medium | Process isolation only | Rejected for v1: adds protobuf/gRPC machinery; more than needed. |
| **Executable plugin, versioned JSON over stdin/stdout (protoc / buf model)** | All OS, any language | Low (exec, pipe, JSON) | Low (read JSON, write JSON) | Process isolation only; no sandbox | **Chosen for v1.** Proven by protoc (`CodeGeneratorRequest`/`Response`) and buf local plugins. |
| WebAssembly (Extism on wazero, pure Go, no cgo) | All OS; authors need a Wasm toolchain | Medium | Medium to high (PDK per language) | Real capability-based sandbox: no FS/network unless granted | Deferred to post-graduation. Keep the protocol JSON-shaped so a Wasm transport can be added without changing the schema. |
| Language-hosted extensions (embedded JS runtime loads TS plugins) | Requires Node at runtime | Medium | Low for TS authors | None | Rejected: ties the compiler to Node, contradicts single-binary distribution. |

External facts used (provisional; WebFetch was blocked during this review, so these rest on search-result summaries accessed 2026-10-21): buf local plugins are `protoc-gen-<name>` executables found on PATH that read the schema on stdin and write files on stdout, and remote plugins run on the Buf Schema Registry (buf.build/docs/generate, buf.build/docs/bsr/remote-plugins). Extism's Go SDK runs on wazero, a pure-Go runtime. WASI 0.3.0 was released 2026-06-11 (wasi.dev).

### 10.3 Design

#### 10.3.1 Roles and trust boundary

```
veld CLI (host, trusted)                     plugin process (trusted by installation, NOT sandboxed)
+------------------------------+   stdin     +-----------------------------+
| compile -> IR (JSON)         | ----------> | read EmitRequest            |
| spawn plugin, enforce timeout|             | produce files in memory     |
| read EmitResponse            | <---------- | write EmitResponse (stdout) |
| validate paths, write files  |   stdout    | logs -> stderr              |
| record manifest, diagnostics |             +-----------------------------+
+------------------------------+
```

- The plugin never touches the output directory. It returns file contents; the host performs all writes. This is what makes output mediation, path containment, the generated-files manifest and determinism checks possible.
- Process isolation is not sandboxing. A plugin runs with the invoking user's privileges and can read any file, open network connections, or spawn processes. The host does not and cannot prevent this in v1. The enforceable controls are: (a) the host refuses to write any returned path that escapes `outDir`; (b) a wall-clock timeout kills the process; (c) stdout size is capped; (d) the plugin's version and protocol version are checked before execution; (e) plugins run only when named in `veld.config.json`, never discovered implicitly from the working directory.
- Policy: plugins are trusted code, with the same trust level as an npm or Go dependency. Users pin versions. Documentation must say this plainly. A future Wasm transport is the path to running untrusted plugins.

#### 10.3.2 Discovery and installation (reuse existing package managers)

Veld does not ship its own plugin package manager. A plugin is an executable resolved in this order:

1. `command` in config (explicit path or program name), e.g. `"command": "./node_modules/.bin/veld-emitter-elixir"`.
2. `veld-emitter-<name>` on `PATH` (protoc convention).
3. `node_modules/.bin/veld-emitter-<name>` relative to the config directory when `package` is set.

```json
{
  "emitters": [
    { "name": "elixir", "package": "@acme/veld-emitter-elixir", "version": "1.2.0",
      "out": "../backend/lib/generated", "options": { "module": "MyApp.Api" } },
    { "name": "hello", "command": "./tools/veld-emitter-hello", "version": "0.1.0", "out": "./docs/api" }
  ]
}
```

Installation is the user's package manager (`npm i -D @acme/veld-emitter-elixir`, `go install`, `cargo install`, or a downloaded binary). `veld doctor` reports each configured plugin: found / not found / version mismatch / protocol mismatch.

#### 10.3.3 Manifest (returned by `--describe`)

```json
{
  "manifestVersion": 1,
  "name": "@acme/veld-emitter-elixir",
  "version": "1.2.0",
  "protocolVersions": [1],
  "irVersions": [">=1.0 <2.0"],
  "kind": "backend",
  "capabilities": {
    "headers": true, "maps": true, "extends": "flatten", "recursiveModels": true,
    "statusCodes": true, "middleware": "ignored", "deprecated": true
  },
  "optionsSchema": { "$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object",
    "properties": { "module": { "type": "string" } }, "additionalProperties": false },
  "deterministic": true
}
```

`capabilities` lets the host warn before emission when a contract uses a feature the plugin declares unsupported (`false`) or degraded (`"ignored"`, `"flatten"`). This is how unsupported semantics fail clearly instead of silently generating incorrect code.

#### 10.3.4 Exchange

Invocation: `veld-emitter-elixir --emit` with one `EmitRequest` JSON document on stdin; the plugin writes exactly one `EmitResponse` JSON document on stdout and exits 0. Non-zero exit, invalid JSON, or timeout means plugin failure: the host writes nothing for that emitter and reports the failure with the stderr tail.

```jsonc
// EmitRequest (host -> plugin)
{
  "protocolVersion": 1,
  "host": { "name": "veld", "version": "0.5.0" },
  "irVersion": "1.0",
  "ir": { /* normalized IR, see chapter 08 section 8.3 */ },
  "options": { "module": "MyApp.Api" },
  "outDir": "lib/generated",            // informational; plugin must not write here
  "env": { "os": "windows", "pathSeparator": "\\" }
}
```

```jsonc
// EmitResponse (plugin -> host)
{
  "protocolVersion": 1,
  "files": [
    { "path": "my_app/api/types.ex", "content": "defmodule ...", "encoding": "utf-8" },
    { "path": "my_app/api/router.ex", "content": "...", "encoding": "utf-8", "executable": false }
  ],
  "diagnostics": [
    { "severity": "warning", "code": "ELIXIR_MIDDLEWARE_IGNORED",
      "message": "middleware AuthGuard on Users.Create is not supported by this emitter",
      "span": { "file": "veld/modules/users.veld", "line": 14, "column": 5 } }
  ],
  "summary": [ { "dir": "my_app/api/", "files": "types.ex, router.ex" } ]
}
```

Rules enforced by the host:

- `path` must be relative, use `/`, contain no `..` segments, no absolute prefix, no drive letter, and after joining must stay inside `outDir` (checked on the cleaned absolute path with a trailing-separator prefix test, not a string prefix on the entry name).
- `files` are sorted by path before writing; duplicate paths are an error.
- Total response size cap (default 64 MiB) and timeout (default 120 s, configurable per plugin).
- Diagnostics with `severity: "error"` abort writing for that emitter.
- Writes go through the same manifest and atomic-write path as built-in emitters (chapter 08 section 8.6).

Progress and logging: stderr lines are forwarded to the user prefixed with the plugin name when `--verbose` is set, otherwise buffered and shown only on failure. No streaming protocol in v1; a plugin that needs progress reporting writes to stderr. Cancellation: the host kills the process on Ctrl-C or timeout.

#### 10.3.5 Compatibility negotiation

1. Host runs `--describe`, parses the manifest.
2. Host checks `protocolVersions` contains a version the host speaks (v1 only at graduation).
3. Host checks the host IR version satisfies the plugin's `irVersions` range.
4. Host checks `version` equals the pinned `version` in config (exact match; range syntax deliberately unsupported in v1).
5. Any mismatch is a hard error with a one-line remedy. `--allow-plugin-mismatch` downgrades steps 3 and 4 to warnings for local development.

#### 10.3.6 Determinism

The plugin contract requires identical output for identical `EmitRequest`. The host offers `veld generate --check-determinism`, which runs each emitter twice and diffs the responses; CI for the reference plugin runs this. Plugins must not embed timestamps, absolute paths, or random IDs.

### 10.4 Plugin SDK and development journey ("hello emitter")

SDK language decision: TypeScript first (`@veld/emitter-sdk`), because the people most likely to write an emitter for a web-API tool are JavaScript developers, and npm already solves distribution. The Go host's own request/response types are exported from `internal/ir` and `internal/emit/plugin` so a Go plugin can import them; a separate Go SDK package is not planned for graduation.

```ts
// hello-emitter/src/index.ts  (proposed SDK surface)
import { defineEmitter } from "@veld/emitter-sdk";

export default defineEmitter({
  name: "veld-emitter-hello",
  version: "0.1.0",
  kind: "tool",
  capabilities: { headers: true, maps: true, extends: "preserve", recursiveModels: true },
  emit(ir, ctx) {
    const lines = ["# API summary", ""];
    for (const m of ir.models) lines.push(`- model ${m.name} (${m.fields.length} fields)`);
    for (const s of ir.services) for (const op of s.operations)
      lines.push(`- ${op.method} ${s.prefix}${op.path} -> ${op.name}`);
    ctx.file("API.md", lines.join("\n") + "\n");
    if (ir.services.some(s => s.operations.some(o => o.middleware.length)))
      ctx.warn("HELLO_MIDDLEWARE_IGNORED", "middleware is not represented in API.md");
  },
});
```

Journey (target: under 30 minutes for a TypeScript developer):

1. `npm create veld-emitter@latest hello` scaffolds the package above with a `bin` entry `veld-emitter-hello`, tests, and a fixture IR.
2. `npm test` runs the SDK test harness: it loads `fixtures/*.ir.json` (shipped with the SDK, generated from the conformance suite), calls `emit`, and snapshots the `EmitResponse`.
3. `npm link`, then in a Veld project add `{ "name": "hello", "package": "veld-emitter-hello", "version": "0.1.0", "out": "./docs" }` to `veld.config.json`.
4. `veld doctor` shows the plugin as found with a compatible protocol; `veld generate` writes `docs/API.md` and lists it in the manifest.
5. `npm publish`; a user runs `npm i -D veld-emitter-hello` and the same config works.

Acceptance test for the graduation release: one reference emitter (the hello emitter above, plus one non-trivial external emitter extracted from the current built-ins; see chapter 09 for the candidate) installed from a registry into a clean project, executed by a release binary, with output byte-identical across two runs and two operating systems.

### 10.5 What is explicitly not promised

- No sandboxing of plugin filesystem or network access in v1.
- No remote plugin execution (buf-style) and no plugin hosting in the Veld registry at graduation; plugins are distributed through npm, Go, or GitHub releases. The Veld registry remains data-only (contracts) at graduation; see chapter 13.
- No incremental or streaming protocol.
- No version ranges for plugin pins.

### 10.6 Cost and sequence

| Step | Depends on | Estimate | Owner (chapter 16) |
|---|---|---|---|
| IR schema v1, JSON Schema, `veld ir` command | compiler IR work (ch. 08) | 24 to 40 h | T1 |
| Host: process runner, timeout, path mediation, manifest integration | output manifest (ch. 08 section 8.6) | 24 to 36 h | T2 |
| `--describe` handshake, compatibility checks, `veld doctor` integration | host runner | 8 to 14 h | T2 |
| TypeScript SDK, IR types generated from JSON Schema, test harness | IR schema | 24 to 40 h | T4 |
| Hello emitter and `npm create` scaffold | SDK | 8 to 16 h | T4 |
| Extract one built-in emitter to external (acceptance) | SDK, host | 16 to 30 h | T4 with T2 |
| Docs: plugin author guide, protocol reference | all above | 10 to 16 h (D1 writes, T2 validates) | D1 |

Total: 114 to 192 h of technical work, roughly 10 to 17 percent of the team's committed technical capacity (chapter 15). This is affordable only if the emitter matrix is reduced as recommended in chapter 09.

### 10.7 Success demonstration

- A plugin written in TypeScript, installed from npm into a fresh project, produces files through a release binary of Veld on Windows and Linux.
- A plugin that returns `../escape.txt` is refused with a clear error and no file is written outside `outDir`.
- A plugin that hangs is killed after the timeout, and `veld generate` exits non-zero with the plugin's stderr tail.
- A contract using a feature the plugin declares unsupported produces a diagnostic before emission.



## 11. Language server and editor integrations

Decision record: ADR-006. Evidence: LSP and editors audit track (scripted JSON-RPC session over stdio against `veld lsp`, extension build in a copy, source reading); finding IDs F-LSP-01 to F-LSP-17 in chapter 05.

### 11.1 What exists at 6a47f81 (verified)

| Item | State | Evidence |
|---|---|---|
| Transport | Content-Length framing over stdio; one message at a time on the read loop; no `recover()` | `internal/lsp/server.go:34-49, 104-137` |
| Capabilities advertised | `{"textDocumentSync":1,"completionProvider":{},"hoverProvider":true,"definitionProvider":true}` | initialize response, session log |
| Document sync | full text on `didOpen`/`didChange`/`didSave`; `didClose` not handled | `handler.go:234-243` |
| Diagnostics | first error only, whole-line range (character 0 to 1000), sometimes on the `action` line instead of the bad token | reproduced |
| Imports / config | never loaded; `rootUri` ignored; a valid `modules/auth.veld` importing `User` reports `undefined output type "User"` | reproduced, `handler.go:261-273, 324-365` |
| Positions | UTF-8 byte offsets; no `positionEncoding` negotiation; hover after an emoji fails at the UTF-16 column | reproduced |
| Completion | same 40 items after `method:`, after `input:` and at top level; hard-coded lists | `completion.go:24-75` |
| Hover / definition | work within a single file; return `null` for imported models | reproduced |
| Robustness | survived garbage input, a NUL byte, cyclic `extends`, 20,000 nested braces; unknown methods get no reply (client hangs); malformed JSON silently dropped | reproduced |
| Protocol conformance | `shutdown` reply lacks `result`; empty diagnostics sent as `null`; `exit` without `shutdown` returns 0 | reproduced |
| Performance | 200 rapid `didChange` events answered within 34 ms on a small file; a 2.4 MB document costs about 745 ms per change and blocks hover behind it | measured |
| Lexer defect visible through LSP and CLI | unterminated string literals are accepted silently | `internal/lexer/lexer.go:258-270`, reproduced |
| VS Code extension | 1,676-line TypeScript re-implementation of the language; no `vscode-languageclient`; commands and settings declared but not registered; rejects valid `headers:` and `response:`; `out/` tracked and stale; no lockfile; no tests; `npm install && npm run compile` succeed in a copy; `npm audit` reports 3 dev-only vulnerabilities | `editors/vscode/src/extension.ts`, `package.json` |
| JetBrains plugin | Gradle, intellij-platform plugin 2.14.0, Kotlin 2.1.10, target IDEA 2025.1; hand-written PSI lexer/parser/annotator (about 4,090 lines Kotlin); only `VeldLanguageSpec.kt` is generated; diagnostics via `veld validate` subprocess on disk files; not built in this review (no Gradle) | `editors/jetbrains/**` |
| Language metadata | `go run ./cmd/generate-language` produces no drift in tracked files; `veld-language.json` is gitignored; both plugin schema copies are byte-identical to `editors/veld-config.schema.json`; CI drift job uses `git diff --quiet`, which misses new untracked files | `.github/workflows/ci.yml:38-67` |

README line 429 and the docs describe rich editor support. The implementation is a basic single-file server plus two independent client-side language implementations.

### 11.2 Target design

#### 11.2.1 Compiler reuse

The server calls `compiler.Compile(ctx, CompileOptions{Overlay: map[URI]string, ConfigDir: nearest veld.config.json upward from the file})` (chapter 08). The overlay supplies unsaved buffers; everything else is read from disk through the loader. The result carries structured diagnostics with file, byte offset, length, code and severity, which the server converts to UTF-16 columns in one helper. No language rule lives in the server.

#### 11.2.2 Feature priority (value against cost)

| Feature | Value | Cost | Graduation scope |
|---|---|---|---|
| Diagnostics (all errors, exact ranges, cross-file) | highest | 16 to 30 h (F-LSP-01, 04) plus structured errors in the compiler | yes, M2 |
| Protocol correctness (MethodNotFound, ParseError, `result` on shutdown, `[]` not `null`, exit codes, `recover`, debounce) | high | 8 to 16 h (F-LSP-02, 07, 08) | yes, M2 |
| UTF-16 position handling or `positionEncoding: utf-8` negotiation | high | 3 to 6 h (F-LSP-03) | yes, M2 |
| Completion (context-aware: directive names, types, model names including imports, enum members) | high | 6 to 10 h (F-LSP-10) | yes, M2/M4 |
| Hover and definition across files | high | included in F-LSP-01 | yes, M2 |
| Document symbols | medium | 3 to 5 h | yes, M4 |
| Formatting (reuse `veld fmt`) | medium | 2 to 4 h | yes, M4 |
| References | medium | 4 to 8 h | M4 if capacity allows |
| Rename | low to medium | 6 to 10 h | deferred |
| Code actions (quick fixes for common diagnostics) | medium | 8 to 16 h | deferred |
| Semantic tokens | low (TextMate grammar is enough for a small language) | 6 to 10 h | deferred |
| Cancellation (`$/cancelRequest`) | low for a fast language, medium for large files | 3 to 5 h | deferred; debounce covers the common case |

Performance target to measure in M4: diagnostics within 100 ms for a 2,000-line project with 10 files on the reference laptop; hover unaffected by a concurrent `didChange`.

#### 11.2.3 VS Code extension (primary)

- Replace `extension.ts` with a `vscode-languageclient` launcher (about 150 lines), TextMate grammar generated by `cmd/generate-language` from `internal/language` (keywords, directives, HTTP methods, scalar types), snippets generated from the same source, and three commands that run the CLI.
- Server discovery and download: described in ADR-006 item 3. The download step is implemented in M4 and verified with a test that corrupts the checksum.
- Settings: `veld.executablePath`, `veld.trace.server`, `veld.validateOnSave` (removed if the server validates on change).
- Logs: the LSP client output channel; server logs go to stderr with a `--log-level` flag.
- Packaging: `vsce package` in CI on tags; publish to the Marketplace by a human with a token held by T5; Open VSX as a stretch goal.
- Tests: an extension integration test (`@vscode/test-electron`) that opens a fixture project and asserts one diagnostic and one completion item; runs in the scheduled CI matrix.

#### 11.2.4 JetBrains (limited)

- Keep the PSI plugin for highlighting and brace matching only; delete `VeldAnnotator.kt` validation rules and hand-written completion lists (they already disagree with the parser).
- Add an LSP4IJ language-server definition pointing to the same `veld` binary (`veld lsp`), with the binary path configurable and a PATH fallback.
- Build only in a scheduled CI job (Gradle download is slow); publish to the JetBrains Marketplace only if the Beta gate is passed with capacity to spare.
- Maintenance estimate once migrated: 2 to 4 h per quarter for IDE version bumps, against 12 to 20 h per month today (audit estimate, assumptions: one language change per month, one IDE bump per quarter).

#### 11.2.5 Canonical language metadata

`internal/language` becomes a real dependency of the lexer, parser and LSP (F-LSP-16), with a test that fails if the parser accepts a directive the spec does not list or vice versa. `cmd/generate-language` continues to emit the VS Code spec file, the Kotlin spec and the JSON file; the CI drift job switches to `git status --porcelain` so new untracked files are caught (F-LSP-17).

### 11.3 Sequence and owners

| Step | Sprint | Owner / reviewer | Exit criterion |
|---|---|---|---|
| Structured diagnostics in `internal/errors` (offset, length, code, all errors) | S3 to S4 | T1 / T5 | CLI prints all errors with `file:line:col`; JSON output available |
| Server on compiler API with overlay and import resolution; protocol fixes; UTF-16 conversion; `recover`; debounce | S4 to S5 | T5 / T1 | scripted session (`lsp-audit/drive.py` from the review, moved into the repo as a Go test) passes all cases including the multi-file project |
| Context-aware completion, document symbols, formatting | S8 to S9 | T5 / T4 | completion after `input:` lists only models including imported ones |
| VS Code thin client, binary discovery and download, commands | S9 to S10 | T5 / T2 | extension integration test green; hand-written service deleted |
| JetBrains: strip annotator, add LSP4IJ definition | S13 (if Beta gate passed) | T5 / T1 | highlighting plus LSP diagnostics in IDEA 2025.x verified manually, screenshots in the report |

Total server and VS Code work: 75 to 135 h, matching the audit's 60 to 100 h server estimate plus 20 to 40 h for the client. This is the second-largest single investment after the compiler IR and is justified because editor support is central to the thesis that a custom DSL is viable (chapter 03).



## 12. Package management, installation and generated-SDK distribution

Three different concerns are kept separate here. Evidence: CLI/distribution audit track (F-CLI-13 to F-CLI-17), ADR-008.

### 12.1 Installing Veld itself

#### 12.1.1 State at 6a47f81 (verified)

| Channel | Directory | How it obtains the binary | Checksum | Platform selection | Grade |
|---|---|---|---|---|---|
| npm | `packages/npm` | `install.js` postinstall downloads from GitHub Releases `latest`, not the wrapper's own version; `npm install` exits 0 even when the download fails, then `veld` fails (reproduced) | none, although the release publishes `checksums.txt` | no windows-arm64 asset; no proxy support | needs work |
| pip | `packages/pip` | bundles all five binaries in the wheel; `veld/__main__.py` uses `str \| None` at import time, which fails on Python 3.8/3.9 while `requires-python >= 3.8` (source-read) | none | n/a (bundled) | needs work; package name `maayn-veld` disagrees with README `veld` |
| Homebrew | `packages/homebrew` | formula with `PLACEHOLDER` sha256 values; release job edits the formula on the runner and never pushes it | placeholder | n/a | placeholder |
| Go | `packages/go`, `go install` | works from the README; binary reports version 0.1.0 (F-CLI-13) | Go module checksum DB | all | ready (version bug) |
| NuGet / Chocolatey | `packages/nuget`, `packages/chocolatey` | hardcoded 0.1.0; placeholder sha256; nuspec links a LICENSE file that does not exist | none | no windows-arm64 | needs work |
| apt (nfpm) | `packages/apt` | installs to `/usr/local/bin` | n/a | linux | needs work |
| Composer | `packages/composer` | no release job; version constant never updated; dependency post-install hooks do not run for consumers; cannot be listed on Packagist from a monorepo subdirectory | none | n/a | placeholder |
| GitHub Releases | `release.yml` | five targets, `checksums.txt`; no signing, SBOM or provenance; actions pinned by tag | yes (file published) | linux/darwin amd64+arm64, windows amd64 | partial |

#### 12.1.2 Decision: channels for graduation

| Priority | Channel | Reason | Work |
|---|---|---|---|
| 1 | GitHub Releases via GoReleaser: archives, `checksums.txt`, cosign signatures, SBOM, GitHub provenance attestation; add `windows-arm64` | source of truth for every wrapper | 8 to 12 h (T5) |
| 2 | npm wrapper | largest audience for the vertical slice; `npx veld` onboarding | pin to the wrapper's version, verify sha256 from `checksums.txt`, fail loudly, honour `HTTPS_PROXY`, skip download when `VELD_BINARY` is set; 4 to 6 h (T5) |
| 3 | Homebrew tap | macOS developers | GoReleaser `brews` publishes the formula to a tap repository; 2 to 3 h |
| 4 | `go install` | zero-maintenance | version from `debug.ReadBuildInfo`; 1 h |
| 5 | pip | Python backend users | fix 3.8/3.9 syntax or raise `requires-python` to 3.10; download-on-first-run instead of bundling five binaries; 4 to 6 h |
| defer | NuGet, Chocolatey, apt, Composer | small audiences, each costs a release job to keep honest | mark as community-maintained, remove placeholders from the repo or move to `packages/experimental/` with a README stating they are unsupported |

Wrapper versus native reimplementation: wrappers only. Every wrapper downloads the Go binary; none reimplements Veld. Uninstall is the package manager's own removal plus `~/.veld` (credentials, cache), which `veld logout --all` and `veld cache clean` must document.

Offline and interrupted installation: wrappers write to a temporary file and rename on success; a partial download never leaves a half-written `veld` on PATH; a checksum mismatch deletes the file and prints the expected and actual hash. Lifecycle-script restriction: when `npm` runs with `--ignore-scripts`, the npm wrapper's `bin` shim prints a one-line instruction to run `npx veld-install` instead of failing silently.

### 12.2 Installing Veld ecosystem artifacts (contracts and plugins)

Decision record: ADR-008. Summary of the smallest useful system:

| Concern | Decision |
|---|---|
| Package types | Contract packages (data only: `.veld`, manifest, README, LICENSE) are hosted by the Veld registry. Emitter plugins are executables and are distributed through npm, Go modules or GitHub Releases; the registry does not host executables at graduation. Different trust policies: contracts are parsed, never executed; plugins run as the user. |
| Manifest | `veld.package.json` (`manifestVersion`, `name` as `@org/name`, semver `version`, `entry`, `dependencies`, `license`, `description`). Validated on `veld push`. |
| Namespaces | `@org` scopes map one-to-one to registry organizations; each scope is bound to exactly one registry URL in `veld.config.json` (`registries`). No fallback across registries, which closes the dependency-confusion path. |
| Version constraints | exact, caret, `1.x`; semver only |
| Resolution | highest version satisfying all constraints; one version per package per project; conflicts are errors naming both requesters; transitive dependencies resolved client-side from the registry's version metadata |
| Lockfile | `veld.lock.json`: name, version, resolved URL, `sha256-` integrity, dependency tree; `veld install --frozen` for CI |
| Integrity | checksum always required before extraction (today optional, `cmd/veld/registry.go:232`); lockfile hash wins over server hash |
| Cache | `~/.veld/cache/<registry-host>/<org>/<name>/<version>.tgz` and a content-addressed `sha256/` directory; `--offline` serves from cache only |
| Local and workspace packages | `"dependencies": { "@acme/common": "file:../common" }` resolved without the registry; workspace entries can `consumes` each other as today |
| Credentials | `~/.veld/credentials.json` keyed by registry URL (as today); token sent only to its own URL; `DefaultRegistry()` must stop picking a random stored registry (`internal/registry/credentials.go:105-107` iterates a map) |

What is not built: a general-purpose package manager, mirrors, proxies, signing of packages (checksums only), and yanking semantics beyond `deprecated` (chapter 13).

Cost: manifest and pack filtering 6 to 10 h; lockfile and resolver 16 to 24 h; cache and offline 8 to 12 h; scoped registries and tests 6 to 10 h. Owner T3, reviewer T1. Scheduled in M4.

### 12.3 Publishing generated SDKs into target ecosystems

Veld generates source, not packages. The recommended path is to generate into a directory that the user's own package tooling owns:

- TypeScript: generate into `packages/api-client/src/` of the user's monorepo; the user's `package.json` publishes it. Veld emits a `package.json` only when asked (`--emit-package-json`), with `name`, `version` from the contract's declared version, `types` and `exports`, no dependencies.
- Python: generate into a package directory with `__init__.py`; the user adds `pyproject.toml`. Veld emits a `py.typed` marker.
- Go: generate a package with its own `go.mod` only in `--standalone` mode; by default generate inside the user's module.

Veld does not publish to npm, PyPI or pkg.go.dev on behalf of the user at graduation. A `veld publish-sdk` command is a post-graduation idea only.

### 12.4 Success criteria

- A clean Windows, macOS and Linux machine installs Veld through npm and through the release archive, with checksum verification demonstrated by corrupting one byte and observing the refusal.
- `veld pull` of a package from a self-hosted registry produces a lockfile; a second `veld install --frozen` on another machine yields byte-identical `veld/packages/`.
- `veld push` of a package containing a `.veld-cache.json`, a dotfile and a nested `node_modules` directory packs only the allowed files (test with the tarball listing).



## 13. Registry architecture and self-hosting plan

Evidence: registry and archive audit track (line-by-line review of `internal/registry` and `internal/server`, crafted archives run against the real extraction code, authorization matrix, live end-to-end run against `veld serve` on a disposable PostgreSQL 16 container, two MITM proxies), findings F-REG-01 to F-REG-23. The lead reviewer independently read `tarball.go`, `client.go`, `credentials.go`, the `pull` command and the schema, and confirmed F-REG-01, 02, 03, 09, 10 and 16 at the cited lines. Decision records: ADR-004 (separate binary), ADR-008 (manifest and lockfile).

### 13.1 What exists at 6a47f81 (verified)

Architecture: Go 1.22 `net/http` ServeMux server inside the CLI (`veld registry serve`), PostgreSQL via `lib/pq`, local filesystem blob storage, HMAC-SHA256 JWT (no external library), `vtk_` API tokens stored hashed, bcrypt passwords, TOTP, SMTP email, embedded single-page UI. Schema: users, orgs, org_members, packages, package_versions (UNIQUE package_id, version), tokens, email_verification_tokens (`internal/server/db/db.go:49-120`). The architecture is reasonable; the implementation is not safe to deploy.

Reproduced against the live server (any registered user can create an org; registration is open and email verification is never enforced):

| ID | Finding | Security | Who can trigger |
|---|---|---|---|
| F-REG-01 | The 5-minute "partial" JWT issued before the TOTP step is accepted as a full session: `/auth/me` returns 200 and `/tokens` mints an admin token (`handlers/auth.go:105-117`, `auth/middleware.go:70-100`; JWT has no purpose claim) | high | anyone with a victim's password; defeats 2FA |
| F-REG-02 | Publish builds the storage key from unvalidated `manifest.name` and `version` (`handlers/packages.go:177`; `storage/storage.go:33-36` cleans and joins without a containment check): wrote outside the storage root, overwrote another org's tarball, deleted it through DELETE | high | any registered user |
| F-REG-03 | Token scopes are not enforced except on publish (`packages.go:105`): a read-only token mints read/write/delete/admin tokens (`auth.go:393-439`) and deletes versions (`packages.go:276-338`) | high | holder of any token, for example a leaked read-only CI token |
| F-REG-09 | `veld pull` takes the SHA-256 from the same response and skips verification when the header is absent (`cmd/veld/registry.go:232-236`, `client.go:208-210`); both MITM proxies (header stripped; body and header replaced) pulled successfully; `http://` accepted silently | high | network attacker on http, or a malicious registry |
| F-REG-11 | Stored XSS sinks in the embedded UI (usernames, org text, versions, deprecation text; `escHtml` used on two lines only; no CSP) | high (source-supported; payloads stored and returned, not rendered in a browser) | any registered user or publisher |
| F-REG-10 | `Unpack`: no size, count or ratio limits (1.24 MiB archive expanded to 1 GiB in 2.9 s; 100,000 files from 1.4 MB); writes in place, partial files left on failure; `io.Copy` and `MkdirAll` errors dropped (`tarball.go:98-143`). Path containment held for `..`, absolute, drive-letter and backslash names; symlink, hardlink and device entries become empty regular files (safe by accident); NTFS alternate data stream names write hidden streams | medium | malicious package author or registry |
| F-REG-07 | Concurrent publish of the same version: the loser's cleanup deletes the winner's tarball while the DB row remains (10 of 25 and 16 of 25 downloads returned 404) | low security, high user impact | two CI jobs racing |
| F-REG-08 | Versions are hard-deletable and re-publishable with different content; "latest" is the most recently published, not the highest semver | medium | org owner or stolen token |
| F-REG-04, 05 | An org admin can demote or remove the sole owner; token `expires_at` never enforced | medium | org admin; expired-token holder |
| F-REG-06, 23 | Private packages: cannot be created through the API (publish hard-codes public), and when set in the database they leak through `/orgs/{org}` (anonymous, with member emails), `/packages` (any logged-in user) and `/versions` (no check); 401/403/404 split is an existence oracle | medium | anonymous or any user |
| F-REG-12 to 15 | No rate limit on login or TOTP (60 and 300 attempts, no 429); TOTP code replays; no body-size limit (200 MiB non-archive accepted as a package); JWT survives logout, 7-day lifetime, no password reset; cookie not `Secure`; CORS `*`; no CSP, X-Frame-Options or HSTS; no TLS option | medium | anonymous / network |
| F-REG-16 | `DefaultRegistry()` returns the first key of a Go map (`credentials.go:105-107`): the default registry varied 9/2/1 across 12 runs; credentials keyed by raw URL string; no scope-to-registry binding | medium | user logged in to two registries |
| F-REG-21 | No health endpoint (`/healthz` returns the SPA with 200), SIGTERM unhandled, no `schema_migrations` table (schema re-run on every start), no backup tooling, zero tests for `internal/registry` and `internal/server` | low | operator |
| F-REG-17, 18, 19, 20, 22 | plaintext credentials (standard practice; ACL inherited on Windows); `Pack` includes every `.veld` at any depth and follows file symlinks; SMTP proceeds without TLS and logs the verification token; pull can overwrite project files when `--out` points at the project and a pulled `veld.config.json` can carry a `postGenerate` hook; docs name commands that do not exist and a `--secret` example that fails the 16-character minimum | low to medium | various |

Restart persistence works. Registration, login, org creation, publish, download and re-publish refusal of an existing version work in the happy path.

### 13.2 Scope decision for graduation

Keep the registry in scope, because Journey C (shared contracts with a lockfile) is a differentiator and the existing code is a reasonable base. Reduce the feature set to what can be made correct and tested:

| Feature | Graduation | Reason |
|---|---|---|
| Users, registration, login, password hashing | yes, with rate limiting and typed JWTs (`purpose: session \| partial`), revocable sessions (server-side session id), password reset by email if SMTP is configured, otherwise operator-only reset command | required |
| TOTP | keep only if F-REG-01 and replay protection are fixed and tested; otherwise disable the endpoints for 1.0 | a broken 2FA is worse than none |
| Organizations, roles owner/admin/member, last-owner protection, ownership transfer by owner only | yes | needed for scopes |
| Scoped tokens (read, write, delete, admin; optional org binding), expiry enforced, revocation list | yes | needed for CI |
| Public and private packages | yes: private is the default for new packages; visibility enforced by one authorization helper used by every handler | tenancy is the point of self-hosting |
| Version immutability | yes: a published version can never be overwritten; delete is replaced by `yank` (hidden from resolution, still downloadable by exact version with a warning) within 72 hours of publish by an owner, and `deprecate` (message) at any time | reproducibility |
| Search | name prefix search over the user's visible packages only | cheap; full-text deferred |
| Audit events | append-only `audit_events` table for login, token create/revoke, publish, yank, deprecate, member changes; exposed as an owner-only list endpoint | low cost; useful for the report's security chapter |
| Retention | none at graduation; blobs are small | n/a |
| Web UI | keep the embedded SPA but rewrite its rendering to a templating approach with escaping by default, add CSP; or reduce to a read-only package browser | F-REG-11 |
| Email | optional; verification required for publishing only when SMTP is configured | avoids blocking local setups |

### 13.3 Target design

API boundary `/api/v1` (unchanged shape where correct, additive elsewhere):

| Endpoint | Auth | Authorization helper call |
|---|---|---|
| `POST /auth/register`, `/auth/login`, `/auth/login/totp`, `/auth/logout`, `/auth/password-reset` | none / partial / session | rate-limited by IP and account |
| `GET /auth/me`, `POST /tokens`, `DELETE /tokens/{id}` | session only (API tokens cannot mint tokens) | `requireSession` |
| `POST /orgs`, `/orgs/{org}/members`, `DELETE .../members/{user}`, `POST /orgs/{org}/transfer` | session or admin-scoped token | `requireOrgRole(owner)` for role changes and transfer; last-owner guard everywhere |
| `GET /packages?org=` | optional | visibility filter |
| `GET /packages/{org}/{name}`, `/versions`, `/{ver}/download` | optional | `canRead(package)` on every one of them |
| `POST /packages` (publish) | token with `write` | `canWrite(org)`; manifest validated: `name` matches `^[a-z0-9][a-z0-9-]{0,63}$`, `version` strict semver, `org` must equal the token's org when org-bound |
| `POST /packages/{org}/{name}/{ver}/yank`, `/deprecate` | token with `delete` / `write` | `canWrite(org)` |
| `GET /orgs/{org}/audit` | session, owner | `requireOrgRole(owner)` |
| `GET /healthz` (process up), `GET /readyz` (DB ping, storage writable) | none | n/a |

Metadata schema additions: `package_versions.sha256` (already), `size_bytes`, `yanked_at`, `dependencies jsonb`; `packages.visibility` default `private`; `sessions` table (id, user, expires, revoked); `audit_events`; `schema_migrations` with forward-only numbered migrations embedded in the binary.

Storage abstraction: `Storage.Put(key, r io.Reader, limit int64) (sha256, size, error)` writes to a temporary file, fsyncs, renames; keys are derived by the server as `sha256/<first2>/<hash>.tar.gz` (content-addressed), never from user input; the DB row stores the key. The local filesystem implementation is the only one at graduation; the interface allows an S3 implementation later.

Publication atomicity and concurrency: (1) validate manifest; (2) stream the upload through `io.LimitReader` (default 20 MiB) into storage computing the hash; (3) verify the archive is a gzip tar whose entries are all regular files with allowed names and sizes (server-side `Unpack` dry run with the hardened reader); (4) in one transaction, insert the version row with `ON CONFLICT DO NOTHING RETURNING`; if nothing returned, the version exists: respond 409 and delete only the blob written in this request if no other row references the hash; (5) respond 201 with `sha256`. Idempotency: a retry with an identical archive for an existing version returns 200 with the same `sha256`; a different archive returns 409.

Resource limits: body 20 MiB (configurable), archive entries 2,000, per-entry 2 MiB, total uncompressed 50 MiB, JSON bodies 64 KiB, login 10 per minute per IP and 5 per minute per account, TOTP 5 per 5 minutes with last-used counter stored.

Threat model and controls (proportional to graduation scope):

| Threat | Control |
|---|---|
| Malicious archive (traversal, symlinks, bombs) | server-side validation on publish and the hardened client `Unpack`: allow-list of entry types (regular file, directory), name validation, limits, temp dir then rename, all I/O errors checked |
| Malicious package author | data-only packages; nothing is executed; a pulled `veld.config.json` is never used as the project config (ADR-008 manifest replaces it; hooks live only in the consumer's own config) |
| Compromised token | scopes enforced; expiry; revocation; org binding; audit events |
| Malicious registry operator or network attacker | lockfile integrity (ADR-008); `https` required unless `--allow-insecure-registry`; checksum always required |
| Private package enumeration | one visibility helper; uniform 404 for unauthorized reads |
| Secret leakage | tokens hashed; secrets never logged; verification token not logged; config validation refuses short secrets at start |
| Supply chain of the registry itself | Docker image built by CI from the tagged source with checksums and attestation (chapter 14) |

Executable plugins are not hosted by the registry (ADR-003, ADR-008). If that changes after graduation, plugins need signing and a separate trust policy; this is stated as future work.

Hosting decision: a dedicated binary `veld-registry` (ADR-004), distributed as a Docker image and release archive; the registry UI stays embedded in that binary; the public website (`website/`) is a separate static site with no code shared and none needed.

### 13.4 Self-hosting and operations

Default deployment: single host with Docker Compose (`deploy/docker-compose.yml`): `veld-registry`, `postgres:16` with a named volume, `caddy` for TLS and the custom domain, artifact volume. Kubernetes, replication and high availability are out of scope and documented as such.

Minimum requirements (assumptions: up to 50 users, 500 package versions, 5 publishes and 500 downloads per day, artifacts under 1 MB each): 2 vCPU, 4 GB RAM, 20 GB disk, one DNS name. Operating cost: a small cloud VM in this class is in the range of 10 to 25 USD per month at common providers (order-of-magnitude assumption; verify with the chosen provider), plus backup storage of a few GB.

Operator features to build:

| Item | Design |
|---|---|
| Configuration validation | `veld-registry check-config` validates the file and environment, refuses a secret shorter than 32 characters, refuses `sslmode=disable` unless `--allow-insecure-db`, prints the effective config with secrets masked |
| Secrets | environment variables or Docker secrets; never in the image |
| TLS | terminated by Caddy; the server also accepts `--tls-cert/--tls-key` for bare-metal |
| Persistent volumes | `pgdata`, `/data/packages` |
| Bootstrap administration | `veld-registry admin create-user --owner-of <org>` for the first account; registration can be closed with `REGISTRATION=invite` |
| Health and readiness | `/healthz`, `/readyz`; compose `healthcheck` |
| Logs | JSON lines to stdout; request id; no secrets |
| Metrics | a minimal `/metrics` text endpoint (requests, publishes, errors) only if time allows; not required |
| Quotas | per-org package count and total bytes (config), enforced on publish |
| Upgrades | image tag bump; migrations run at start, forward-only, each in a transaction; the binary refuses to start if the DB schema is newer than it knows |
| Migration failure | start aborts with the failed migration number; operator restores from backup or applies the fix; application rollback to the previous image is only safe if no migration has been applied since the backup, which the `backup-manifest.json` records |
| Private networks, custom domains | Caddy config samples; air-gapped: image and compose files transferable; `veld` clients use `--registry` with the internal name; no telemetry or outbound calls exist |

Backup and restore (to be tested in M5 and timed):

1. `veld-registry backup --out /backups/<ts>/`: runs `pg_dump -Fc` through the configured DSN, archives `/data/packages`, writes `backup-manifest.json` (schema version, app version, SHA-256 of both archives, counts of users, packages, versions).
2. Off-host copy by the operator's tool (rclone, scp); the runbook shows one example.
3. `veld-registry restore --from /backups/<ts>/`: refuses if the app version in the manifest is newer than the binary; restores the DB and artifacts; runs `veld-registry verify` which recomputes the SHA-256 of every blob referenced by `package_versions` and reports missing or mismatched blobs.

Proposed objectives: RPO 24 hours (nightly backup), RTO 1 hour on a clean host with the runbook. Acceptance check for the Beta gate: a recorded restore drill on a clean VM, timed, with `verify` reporting zero mismatches and a client `veld install --frozen` succeeding against the restored registry.

Minimum operator runbook (D1 writes, T3 validates by executing every step): install, first user and org, TLS, backup, restore, upgrade, migration failure recovery, rotating the JWT secret (invalidates sessions), revoking a leaked token, closing registration.

### 13.5 Work plan and estimates (owner T3; reviewer T1 for security items)

| Item | Findings | Estimate (h) | Sprint |
|---|---|---|---|
| Hardened `Unpack` (limits, temp dir, errors, entry allow-list) and `Pack` filtering; checksum-required pull; `https` default; deterministic registry selection | F-REG-09, 10, 16, 18 | 12 to 20 | S1 |
| Typed JWTs with purpose, scope enforcement helper, expiry, last-owner guard, session revocation | F-REG-01, 03, 04, 05, 14 | 10 to 18 | S1 to S2 |
| Manifest validation, content-addressed storage, atomic publish, limits, idempotency | F-REG-02, 07, 08, 13 | 12 to 20 | S2 to S3 |
| Visibility helper and uniform 404; private default; member email removal | F-REG-06, 23 | 6 to 10 | S3 |
| Rate limiting, TOTP replay counter, security headers, cookie flags, CORS allow-list | F-REG-12, 15 | 6 to 10 | S3 to S4 |
| Separate binary, versioned migrations, `/healthz`, `/readyz`, SIGTERM, JSON logs, config check | ADR-004, F-REG-21 | 10 to 16 | S3 to S4 |
| UI rendering rewrite with escaping and CSP | F-REG-11 | 8 to 14 | S4 to S5 |
| Server test suite (`httptest`, fake storage, PostgreSQL service in CI): authz matrix, publish races, limits | F-CLI-23 | 16 to 30 | S2 to S5 |
| Manifest, lockfile, resolver, cache, scoped registries (client) | ADR-008 | 36 to 56 | S8 to S10 |
| Compose, Caddy, backup/restore/verify commands, runbook, drill | section 13.4 | 16 to 24 | S11 to S12 |
| Total | | 132 to 218 | |

The registry stream is the only one that can expand without bound; the scope table in 13.2 is the fence. If the stream exceeds its sprint budget by more than 20 percent at the M2 exit review, search, audit UI and TOTP are cut, in that order.



## 14. Security, testing, releases, open-source governance and ongoing maintenance

### 14.1 Security posture (summary of chapter 05 and 13)

Six findings carry security severity high at 6a47f81: route shadowing that bypasses middleware in generated Node routes (F-NODE-05), registry 2FA bypass (F-REG-01), storage path traversal on publish (F-REG-02), unenforced token scopes (F-REG-03), optional and self-referential download integrity (F-REG-09) and stored XSS sinks in the registry UI (F-REG-11). Twelve more are medium. None of them requires a redesign; all are fixed by the M1 to M3 items in chapters 08, 09 and 13.

Security work that is in scope for graduation:

- Threat models written down for the generator (hostile contract, hostile plugin, hostile package) and the registry (chapter 13 table), each with the controls that exist and tests that prove them.
- A `SECURITY.md` with a private reporting address, a 90-day disclosure window, and the supported versions table.
- `govulncheck` in CI; `npm audit --production` for the extension and SDK (the VS Code extension currently has 3 dev-only vulnerabilities, 2 high).
- Security self-review at the RC gate using the OWASP ASVS level 1 checklist for the registry; D1 records results, T3 and T1 sign.
- No telemetry of any kind in the CLI, server or extension (decision; documented in the privacy section of the README).

Out of scope, stated as future work: plugin sandboxing (chapter 10), package signing beyond checksums, registry-hosted plugins.

### 14.2 Testing strategy (fast versus scheduled)

| Layer | Runs on | Content | Owner |
|---|---|---|---|
| Fast CI (every push, under 10 minutes) | ubuntu, windows, macos | `go build`, `go vet`, `go test -race` (unit + conformance suite), language-metadata drift (`git status --porcelain`), import-boundary test, determinism test on fixtures, tier-1 generated-code compile (`tsc`, `compileall`, `go build`), VS Code extension compile, website build, `govulncheck` | T5 |
| Round-trip CI (every push to main and PRs touching emitters) | ubuntu, windows | tier-1 server + client harness, cross-language matrix (Node/Python/Go servers against the three clients), error cases | T4 |
| Registry CI (every push touching `internal/registry`, `internal/pkg`, `cmd/veld-registry`) | ubuntu with PostgreSQL 16 service | handler tests, authz matrix, publish race, limits, malicious archives, e2e publish/pull with lockfile | T3 |
| Scheduled nightly | ubuntu | tier-2 compile matrix (javac, dotnet, php -l, vue/svelte tsc), JetBrains Gradle build, parser and archive fuzzing (`go test -fuzz` 10 minutes each), installer smoke tests (npm, brew, release archive on a clean container), plugin protocol compatibility (hello emitter against the current host) | T4 / T5 |
| Release pipeline (tags) | ubuntu | GoReleaser build, checksums, cosign signatures, SBOM, GitHub attestation, wrapper publication, Docker image | T5 |

Parser fuzzing: the audit's 1.67 million executions found no panic; the fuzz targets written during the audit (`fe-audit/fz/`) should be moved into the repository as `FuzzLex`, `FuzzParse`, `FuzzValidate`, `FuzzDiff`, `FuzzUnpack`. Property tests: `fmt` idempotence on every fixture; `diff(a, a)` empty; `Unpack(Pack(dir))` equals `dir` for allowed files.

Generated-code conformance: the harness in chapter 09 §9.5 is the acceptance mechanism for every tier claim. A README support cell is generated from the latest CI run, never hand-edited.

Documentation examples that compile: every code block in `docs/guides` tagged `veld-example` is extracted by a script and run through `veld validate`; the tutorial project under `examples/node-typescript` is generated and compiled in fast CI (it is today the only example whose output passes `tsc --strict`).

Architecture drift detection: the import-boundary test (chapter 07 §7.1); a test that the registered emitter list equals the README matrix; the language-metadata drift job fixed to catch new files.

### 14.3 Releases

- GoReleaser configuration in the repository; tags `vX.Y.Z` (CLI), `registry/vX.Y.Z`, `sdk/vX.Y.Z`, `vscode/vX.Y.Z`.
- Each release: archives for linux/darwin/windows on amd64 and arm64, `checksums.txt`, cosign signatures (keyless), SBOM (Syft), GitHub artifact attestation (provenance), Docker image for the registry, Homebrew formula pushed to the tap, npm wrapper published with provenance, VS Code extension packaged (`vsce`) and published by a human.
- Changelog in Keep-a-Changelog format updated per sprint by the integration owner; release notes generated from it.
- Version support: the latest minor receives fixes; the previous minor receives security fixes for 3 months after the next minor; language versions follow ADR-009.
- Credentials: npm, Marketplace and tap tokens stored as GitHub environment secrets with required reviewers (two team members) for the `release` environment; no secrets in wrappers.
- Actions pinned by commit SHA (today pinned by tag).

### 14.4 Open-source governance

| Item | Decision | Owner |
|---|---|---|
| License | MIT for the whole repository (README already claims it; no LICENSE file exists). Add `LICENSE` at the root in Sprint 0; verify that `cobra` (Apache-2.0: keep its LICENSE and NOTICE in the distribution archive), `lib/pq` (MIT) and `golang.org/x/crypto` (BSD-3-Clause) are compatible (they are permissive and compatible; confirm with the SPDX pages). Generated output carries no license header and is owned by the user; state this in the README ("output of Veld is yours"). | T5 |
| University IP policy | Unresolved: confirm with the department whether student-authored open-source code requires any assignment or notice, and whether the June submission can reference a public repository. Record the answer as ADR-011. | T1 with D1 |
| Contribution terms | Developer Certificate of Origin (sign-off), no CLA. | T5 |
| Files | `CONTRIBUTING.md` (build, test, fixture-writing, emitter tiers, how to add an external emitter), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), `SECURITY.md`, `CODEOWNERS` (chapter 16 matrix), issue templates (bug with contract and `veld --version --json`; feature; emitter proposal), PR template (tests, docs, changelog, tier impact), `GOVERNANCE.md` (two-maintainer review for `internal/`, one for docs). | D1 drafts, T5 validates |
| RFC/ADR process | ADRs in `docs/adr/` with the template used in this review; an RFC is an ADR in `proposed` state open for one sprint of comments. | D1 |
| Roadmap | `ROADMAP.md` regenerated from chapter 15 at each gate. | D1 |
| Telemetry and privacy | none; documented. | T5 |
| Accessibility | website and registry UI: keyboard navigation, contrast, alt text, checked with a browser accessibility audit before the Beta gate; docs in plain language. | D1 with T5 |
| First external contributor | target: one external contribution (a tier-2 fix or a plugin) merged before the readiness gate, as evidence for the "contributor accessibility" claim; `good first issue` labels on at least ten backlog items by M4. | T5 |
| Bug triage | weekly 20-minute triage in the sprint meeting; labels `P0..P3` as in chapter 05. | integration owner |

Maintenance after graduation: the team commits (in `GOVERNANCE.md`) to security fixes for six months after the release and to responding to issues within two weeks; anything beyond that is best-effort. The tier model keeps the maintained surface small enough for one or two volunteers.

### 14.5 Release criteria (measurable) and evidence

| Criterion | Measure | Evidence |
|---|---|---|
| Tier-1 correctness | round-trip harness green on three OSes for two consecutive sprints | CI run links in the report |
| Safety | output-safety tests (unmanaged file, `--out` at project root, failing compile keeps output, interrupted write) green | CI |
| Determinism | byte-identical generation across runs and OSes for all fixtures | CI job output |
| Plugin system | hello emitter installed from npm into a clean project generates on Windows and Linux; escape, timeout and capability tests pass | CI and a recorded demo |
| Editor | VS Code integration test green; multi-file diagnostics in the scripted session | CI |
| Registry | authz matrix, race, limits and malicious-archive tests green; restore drill recorded with timing | CI and runbook appendix |
| Security | zero open P0/P1 with security severity; `govulncheck` clean; ASVS L1 checklist completed | checklist in `docs/security/` |
| Docs | every support claim generated from CI; tutorial compiles; discrepancy list from chapter 05 closed | docs CI |
| Release | signed artifacts with checksums, SBOM and attestation; wrappers install on clean machines | release page and smoke-test logs |
| Governance | LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, CODEOWNERS present; at least one external contribution merged | repository |

### 14.6 Priority order for the controls (so the checklist does not consume the project)

1. Sprint 0: LICENSE, CODEOWNERS, CHANGELOG, `go 1.26`, hygiene, CI on three OSes.
2. M1: tier-1 compile jobs, output-safety tests, malicious-archive tests, `govulncheck`.
3. M2: round-trip harness, registry CI with PostgreSQL, determinism job, import-boundary test.
4. M3: SECURITY.md, CONTRIBUTING.md, issue and PR templates (documentation work fits the exam period).
5. M4: GoReleaser with signatures, SBOM and attestations; installer smoke tests; nightly tier-2 and fuzz jobs.
6. M5: accessibility audit, ASVS checklist, runbook drill.
7. M6: release rehearsal, first external contributor outreach.



## 15. Dated roadmap to June 2027, dependencies, capacity and contingency

Planning baseline 2026-10-21 (Wednesday). Conservative readiness target 2027-05-31 (Monday). June 2027 is reserved for submission, defense and corrections and is budgeted separately (chapter 16). Calendar computed by `scratchpad/plan/calendar.py` (reproduced in the appendix of chapter 16): 222 days, 31.7 weeks, one 12-day Sprint 0 plus fifteen 14-day sprints ending 2027-05-30. If work starts later than the baseline, drop sprints from the front of the plan and apply the cut list in 15.6; do not move the readiness target.

### 15.1 Capacity assumptions (to validate in Sprint 0)

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

### 15.2 Budget check: planned work against committed capacity

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

### 15.3 Milestones

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

### 15.4 Week-by-week plan for the first eight weeks (Oct 21 to Dec 15)

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

### 15.5 Critical path and major risks

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

### 15.6 Fallback plans

- Substantially lower capacity (8 h/week): drop JetBrains entirely; drop the docs-emitter extraction (hello only); drop the formatter rewrite (keep `fmt --check` disabled); limit the package resolver to exact versions (no ranges, no transitive resolution); user study with 4 participants; tier-2 emitters compile-checked but not fixed beyond the header bug. Saves roughly 200 to 260 h.
- A technical member becomes unavailable: the backup owner takes the stream at reduced scope: T1 lost -> IR limited to what plugins need, validator extensions deferred; T2 lost -> plugin host minimal (no `--describe` capabilities warnings), emitters stay on AST adapter; T3 lost -> registry frozen at hardened tarball, checksum-required pull, compose deployment, no lockfile resolver; T4 lost -> harness limited to Node and Python, evaluation Q3 dropped; T5 lost -> VS Code client minimal (no download), release manual with checksums only.
- Failed rewrite spike: not applicable, no rewrite is planned (ADR-001). If the team nevertheless starts one, it must prove viability (lexer, parser, validator passing the conformance suite) by Nov 29 or be abandoned.
- Plugin approach proves too expensive: ship the protocol specification, the host runner and the hello emitter (minimum for the thesis); defer the SDK scaffolding (`npm create`) and the extracted emitter. If even the host runner cannot be completed by the Alpha gate, present the protocol as a validated design with the stub plugin tests and state the limitation; this is the only scenario in which chapter 10 is partially delivered.

### 15.6a Amendment 2026-10-21 (ADR-012): extended scope

Chapter 20 adds an ordered queue of fourteen items (Q1 to Q14) behind the core, starting in S8, with an extended-feature freeze on 2027-05-02 and an automatic cut order. Under the AI-assisted multiplier of 1.3 the planned work becomes 1,251 to 2,169 hours against 1,438 equivalent committed hours plus 360 reserve; the midpoint leaves 88 hours of reserve and the high estimate is 371 hours short, which is why the cut is automatic. The milestone table above remains valid for the core; the queue's sprint placement is in chapter 20 §20.3 and in `csv/sprint-allocation.csv`. The productivity multiplier is measured at the M1 exit (B-077) and the queue is truncated per chapter 20 §20.6 if it is below 1.15.

### 15.7 Gates summary

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



## 16. Team allocation: T1 to T5 and D1

No team member's skills are known to this review. Every assignment below is a planning scenario to be adjusted after the skills survey in 16.1, which must be completed in Sprint 0 (by 2026-10-30). Hours are the baseline model from chapter 15 §15.1 (12 h/week technical, 8 h/week D1, exam-adjusted, 15% coordination, 20% reserve held).

### 16.1 Skills survey (10 minutes per person)

Rate 0 (none) to 3 (can teach it): Go; TypeScript/Node; Python/Flask; React; PostgreSQL and SQL; Docker and Linux operations; HTTP API design; compilers or parsers (any coursework); VS Code extension or JetBrains plugin development; GitHub Actions; security basics (auth, hashing, input validation); technical writing; diagramming; statistics for evaluation. Also: preferred working days, exam dates for both semesters, any planned absence, and the one subsystem each person most wants to own. The lead (T1) re-balances the matrix in 16.3 using the answers; D1 records the result as a decision.

### 16.2 Capacity (baseline, to readiness target)

| Member | Role | Committed h | Reserve h | Notes |
|---|---|---|---|---|
| T1 | compiler core, lead architect | 221 | 55 | also chairs ADR decisions and the migration decision |
| T2 | emission host, output safety, Node/TS emitters, plugin host | 221 | 55 | |
| T3 | registry, package resolution, security, operations | 221 | 55 | |
| T4 | tier-1 Python/Go emitters, test harness, plugin SDK and reference plugins, evaluation | 221 | 55 | |
| T5 | LSP, VS Code, CLI/UX, distribution, release, CI | 221 | 55 | |
| D1 | documentation, diagrams, ADR editing, report, slides, study coordination | 148 | 37 | no production code; no sole technical validation |

June 2027 (submission and defense) is budgeted separately: assume 20 h per technical member and 40 h for D1 for corrections, rehearsal and defense; these are not available for implementation.

### 16.3 Component ownership matrix

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

### 16.4 Per-member plan

#### T1: compiler core (primary), diagnostics and LSP backend (secondary)

- Skills to learn: compiler engineering patterns (error recovery, spans), JSON Schema, writing conformance fixtures.
- Monthly deliverables: Oct/Nov ADRs accepted, lexer fixes, header identifiers, loader strictness; Dec `diag`, `TypeRef`, IR v0; Jan IR v1 validated on all fixtures (exam month: only review and fixture writing in S6); Feb diff on IR and gate; Mar validator extensions, formatter start; Apr formatter, evaluation Q1 support; May demo lead, report chapters 4 and 5 technical review.
- Expected effort: 221 h; heaviest in S3 to S5 (16 h per sprint committed).
- Dependencies: none upstream; everyone depends on IR by S5.
- Review obligations: emission host (T2), harness (T4), registry security review (T3).

#### T2: emission host and Node/TS emitters (primary), plugin host and OpenAPI (secondary)

- Skills to learn: TypeScript strict mode and `tsc` tooling, Express/Fastify request models, process management in Go.
- Monthly deliverables: Nov staged writer and manifest, gate ordering, path safety, Node fixes, e2e round trip; Dec emitters on `diag` and IR, tier-3 removals; Jan VEP schema frozen, host runner skeleton with stub plugin; Feb/Mar host runner complete, `--describe`, docs emitter extraction with T4, OpenAPI completeness; Apr bug fixes, determinism checks; May release hardening.
- Expected effort: 221 h; heaviest in S1 to S2 and S8 to S10.
- Dependencies: IR (T1) from S5; SDK (T4) from S8.
- Review obligations: syntax and IR (T1), VS Code (T5), lockfile (T3).

#### T3: registry, package resolution, security and operations (primary), server-side tests (secondary)

- Skills to learn: PostgreSQL migrations and transactions, Docker Compose, threat modelling, Go `httptest`.
- Monthly deliverables: Nov tarball hardening, checksum-required pull, `Pack` filtering, `DefaultRegistry` fix, authz matrix tests; Dec split binary, atomic publish, size limits, versioned migrations, CI PostgreSQL job; Jan ADR-008 design frozen (exam month); Feb/Mar manifest, lockfile, resolver, cache, scoped registries; Apr compose deployment, backup/restore drill, runbook, security self-review; May support for the demo registry.
- Expected effort: 221 h.
- Dependencies: few; IR not needed. Depends on T5 for release packaging of `veld-registry`.
- Review obligations: compiler API (T1), VEP security (T2), CI hygiene (T5).

#### T4: Python/Go emitters, test harness, plugin SDK and reference plugins, evaluation (primary)

- Skills to learn: Flask, Go net/http, GitHub Actions matrices, npm package publishing, basic experiment design.
- Monthly deliverables: Nov Python and Go compile fixes, harness v0, compile jobs, Flask and Go round trips; Dec validation default ON, determinism test, 30 fixtures; Jan fixtures and expected IR (exam month); Feb/Mar TypeScript SDK, hello emitter on npm, docs emitter extraction, `npm create` scaffold; Apr evaluation runs Q1 to Q3, user study execution with D1; May evaluation write-up support.
- Expected effort: 221 h; heaviest S8 to S12.
- Dependencies: IR schema (T1) for SDK types; host (T2) for plugin runs.
- Review obligations: IR (T1), emission host (T2), ops runbook (T3), LSP (T5).

#### T5: language server, VS Code, CLI/UX, distribution, release, CI (primary), config (secondary)

- Skills to learn: LSP 3.18 details, `vscode-languageclient`, GoReleaser, cosign and GitHub attestations.
- Monthly deliverables: Nov hygiene PR, `go 1.26`, CI compile jobs and OS matrix, config precedence, `doctor`; Dec LSP on compiler API, protocol fixes, UTF-16, imports; Jan GoReleaser with checksums (exam month); Feb/Mar VS Code thin client with discovery and download, completion/symbols/formatting, `--json` and exit codes, npm/pip/brew wrappers; Apr website CI, JetBrains LSP4IJ only if reserve allows; May release 1.0.0, signed artifacts.
- Expected effort: 221 h.
- Dependencies: `diag` (T1) by S4; compiler API by S5.
- Review obligations: Node/TS emitters (T2), SDK (T4), registry server (T3, non-security aspects).

#### D1: documentation, diagrams, research organization, report, presentation

- Recurring work every sprint: update current/target diagrams (D1 to D8, with the technical owner named on each); requirements traceability matrix (requirement -> ADR -> backlog item -> test); ADR editing after decisions; installation and tutorial documentation (validated by the component owner); API and plugin documentation (validated by T2/T4); meeting decisions log; evaluation evidence folder; report chapters; slides; demo script.
- Monthly deliverables: Oct/Nov skills survey, ADR set published, tutorial for Journey A, report chapter 1 draft; Dec spec pages, safe-generation and support-matrix pages, chapter 2 and 3 drafts; Jan plugin author guide draft (from the frozen spec), chapter 4 draft; Feb/Mar user-study protocol and consent, recruitment with T4, chapters 5 and 6; Apr study sessions, operator runbook editing, chapters 7 to 9; May final report, slides, demo video script, defense rehearsal coordination.
- Validation rule: every technical statement in the docs and the report cites a test, a CI run, a command output or a finding ID, and is signed off by the validator named in 16.3. D1 may coordinate usability feedback but T4 runs the technical part of the study.

### 16.5 Pairing and knowledge-transfer checkpoints

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

### 16.6 Sprint responsibility table

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



## 17. Diagrams, academic evaluation, report outline and demonstration plan

### 17.1 Diagrams (Mermaid sources in `diagrams/`)

| File | Purpose | Shows current vs proposed | Owner (content / drawing) | Update trigger |
|---|---|---|---|---|
| `D1-system-context.mmd` | deployables and who talks to whom | solid = current, dashed = proposed (plugin process, `veld-registry`) | T1 / D1 | any ADR that adds or removes a deployable |
| `D2-compiler-pipeline.mmd` | pipeline stages and the allowed dependency direction | proposed, with current package names in brackets | T1 / D1 | changes to `internal/compiler`, `internal/ir`, layout |
| `D3-plugin-trust.mmd` | host and plugin exchange, what the host enforces and does not | proposed | T2 / D1 | VEP schema or host-rule change |
| `D4-registry-publish-consume.mmd` | publish and pull sequence, where integrity is verified, where atomicity is added | current with proposed steps marked [P] | T3 / D1 | changes to `cmd/veld/registry.go`, `internal/registry`, `handlers/packages.go` |
| `D5-registry-er.mmd` | tables and relations | current schema plus proposed `audit_events` | T3 / D1 | any migration |
| `D6-editor-lsp.mmd` | editor, server, compiler API interaction incl. unsaved buffers | current transport, proposed engine | T5 / D1 | changes to `internal/lsp` or editor clients |
| `D7-selfhost-backup.mmd` | compose topology and backup/restore flow | proposed | T3 / D1 | changes to `deploy/` or the runbook |
| `D8-roadmap-critical-path.mmd` | milestones, gates, critical path | proposed | T1 / D1 | every sprint review |

Redundant diagram types (class diagrams per package, per-command flowcharts) are deliberately not produced; D1 adds a diagram only when it explains a decision in the report.

### 17.2 Evaluation (planned; no results exist)

The plan in chapter 03 §3.9 is the authoritative list. Execution responsibilities: T4 runs Q1 to Q3 and Q5 with scripts under `evaluation/`; T2 runs Q4 with the SDK users; D1 coordinates the user study, consent and scheduling; T4 runs the technical part of each session. Results are recorded with collection date, commit hash, machine description and raw data files, and are reported in the thesis with their limitations (small N, student participants, single environment).

Baselines must be fixed before the Alpha gate: the OpenAPI corpus list (Q1), the hand-written Express + fetch reference app (Q2), the Zod and Pydantic validators (Q3), the three implementations at 6a47f81 (Q5).

### 17.3 Graduation report outline (mapped to evidence and contributors)

| Chapter | Content | Evidence sources | Contributors |
|---|---|---|---|
| 1 Introduction and problem definition | contract drift problem, thesis statement, research questions (ch. 03 §3.5, §3.8), contributions | this review ch. 03 | D1 writes; T1 validates |
| 2 Background and related work | OpenAPI, TypeSpec, Smithy, protobuf/buf, TS-only tools, LSP, plugin architectures | appendix-external-research (re-verified with primary pages) | D1; T1, T2 validate |
| 3 Requirements | functional and quality requirements, traceability to ADRs and backlog | ch. 03 §3.6, ch. 18 | D1; all owners |
| 4 Architecture | boundaries, IR, plugin protocol, LSP engine, registry; ADRs | ch. 07 to 13, diagrams | T1, T2, T3, T5 write sections; D1 edits |
| 5 Implementation | compiler changes, emitters, host and SDK, server, VS Code | pull requests, CHANGELOG | each owner |
| 6 Testing and quality assurance | conformance suite, compile and round-trip matrix, fuzzing, registry tests, CI | CI run links, coverage | T4, T5 |
| 7 Evaluation | Q1 to Q5 results, user study | `evaluation/` data | T4; D1 presents |
| 8 Security | threat models, fixed findings with before/after evidence, ASVS checklist | ch. 05 register, ch. 13, ch. 14 | T3; T1 reviews |
| 9 Limitations and future work | tiers not reached, Wasm plugins, OpenAPI import, registry-hosted plugins, JetBrains | ch. 03 §3.7, ch. 10 §10.5 | D1; T1 validates |
| 10 Project management | plan versus actual, capacity model, gates, risks that materialized | ch. 15, ch. 16, sprint reviews | D1; T1 validates |
| 11 Conclusion | | | D1 |
| Appendices | ADRs, defect register before/after, runbook, user-study protocol, generated-code samples | this review | D1 |

Every chapter draft is due per the D1 schedule in chapter 16 §16.4; each technical statement cites a test, CI run, finding ID or command output.

### 17.4 Live demonstration scenario (defensible) and offline fallback

Live (12 minutes, from a clean VM image prepared in M7, no network needed except the local registry):

1. `npx veld init` in an empty folder; open in VS Code; show the extension starting `veld lsp`; introduce a typo in a type name; show the diagnostic with the exact range and the cross-file definition jump.
2. Add a `POST /users` action with a header block; `veld generate`; show the manifest and that a hand-written file in the output directory survives regeneration.
3. Run the generated Express server with a 20-line service implementation; call it with the generated TypeScript client; show 201, a 400 with validation details, and a 404 for a path with special characters encoded correctly.
4. Switch the backend to Python in the config; regenerate; run Flask; call it with the same TypeScript client (cross-language round trip).
5. Remove a field; `veld generate --strict` fails with the breaking change named; `--force` proceeds and the lockfile updates.
6. Install the hello emitter from npm; add it to the config; `veld doctor` shows it; generate; show `API.md`; show a plugin returning `../escape.txt` being refused.
7. `veld push` the contract to the local self-hosted registry (compose, already running); `veld pull` it into a second project; show `veld.lock.json`; tamper with the archive on disk through the local proxy and show the integrity failure.

Offline fallback: a recorded video of the same script, plus a pre-generated folder for each step so that any step can be shown from static files if the VM fails. The recording is produced at the RC gate and refreshed at the readiness gate.

Questions the demo is designed to pre-empt: "is this just OpenAPI with extra steps?" (step 3 to 5: server interfaces, validation, breaking-change gate), "can anyone extend it?" (step 6), "is the registry safe to self-host?" (step 7 and the security chapter).



## 18. Prioritized backlog, acceptance criteria, unresolved decisions and the first ten actions

### 18.1 Backlog

The implementation-ready backlog is `csv/backlog.csv` (58 items, stable IDs B-001 to B-058). Every item has an outcome, priority, acceptance criteria, effort range, owner, reviewer, dependencies, target milestone and the findings or ADRs it closes. Items for M0 to M2 are specified to the level of a pull request; items from M4 onward are coarser and will be split at sprint planning. The sprint responsibility table is `csv/sprint-allocation.csv`.

M0 and M1 items (implementation-ready, in dependency order): B-001 hygiene, B-002 Go 1.26, B-004 ADRs and survey, B-005 fixture skeleton, B-003 governance files, B-010 lexer fixes, B-006 staged writer, B-007 gate ordering, B-008 path safety, B-009 dry-run/clean/init, B-011 header identifiers, B-012 determinism, B-013 Python, B-014 Go, B-015 Node, B-016 compile jobs, B-018 client integrity, B-019 hardened unpack, B-020 registry auth, B-017 harness v0.

Priority definitions are those of chapter 05 §5.1. Per-sprint load is checked in chapter 16 §16.6.

### 18.2 Acceptance criteria for the three release levels

| Level | Date | Criteria (all must hold) |
|---|---|---|
| Alpha | 2027-03-21 | B-001 to B-040 accepted; vertical slice (Journeys A and B) in CI on two OSes; external plugin acceptance test; `veld install --frozen` reproducible; LSP multi-file session; registry e2e in CI |
| Beta (feature freeze) | 2027-04-18 | B-041 to B-054 accepted or formally cut; tier-1 round trip green on three OSes for two sprints; restore drill recorded; evaluation data collected; docs generated from CI evidence |
| Release 1.0.0 (readiness) | 2027-05-31 | chapter 14 §14.5 table all green; zero open P0/P1; signed artifacts; report, slides, demo video complete |

### 18.3 Unresolved decisions (only those that change the plan)

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

### 18.4 First ten actions (this week and next, dated)

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

### 18.5 Consistency check performed on this plan

- Every mandatory component (writer, IR, plugin host and SDK, LSP, VS Code client, registry hardening, lockfile, compose, harness, evaluation) has an implementation owner, a reviewer and a milestone in `csv/backlog.csv` and a capacity line in chapter 16.
- No downstream item starts before its interface exists: the writer API (S1) precedes emitter migration; `diag` (S3) precedes LSP work; IR schema (S5) precedes the SDK; VEP schema (S6) precedes the host runner completion; the lockfile format (S7) precedes the resolver.
- Budget: planned technical work 694 to 1,139 h against 1,106 h committed plus 277 h reserve (chapter 15 §15.2); the exam sprints carry only review, freeze and documentation tasks.
- Release criteria (chapter 14 §14.5) map one-to-one to backlog items B-016, B-006, B-012, B-040, B-042, B-033, B-051, B-056, B-054, B-057, B-003.



## 19. Expected state of Veld at submission (readiness gate 2027-05-31, release 1.0.0)

This chapter states, dimension by dimension, where Veld is at the reviewed revision (6a47f81, verified in chapters 04 and 05) and where it is expected to be when the graduation release ships, if the plan in chapters 15 to 18 is executed at baseline capacity. Every "end state" row is a target with a verification method, not a result. Performance numbers are targets to be measured in M4 to M6 on the reference laptop defined in the evaluation protocol; none has been measured yet except where marked "measured today".

### 19.1 Product and positioning

| Dimension | Start (6a47f81) | End (1.0.0) | Verified by |
|---|---|---|---|
| Thesis | "contract-first, multi-stack generator for 7+ backends and 4+ frontends" with claims that outrun the code | "a minimal, fully checked contract language whose compiler emits dependency-free server interfaces, routes, validation and clients for a few stacks, with build-tool safety" | README, thesis chapter 1, support matrix generated from CI |
| Claimed targets | 8 backends, 10 frontends, 6 tools; 2 of 18 compile | tier 1: node-ts, typescript, react, python (Flask), go (net/http, chi), 3 service SDKs, openapi; tier 2 compile-checked nightly; the rest removed | `veld doctor` and README matrix generated from the latest CI run |
| Differentiators that can be demonstrated | none without caveats | server interfaces plus route wiring plus validation across three backends; zero-dependency output; breaking-change gate; non-destructive generation; external plugin protocol; single binary with LSP | demo script chapter 17 §17.4 |
| Interoperability | OpenAPI 3.0.3 export (valid, incomplete) | OpenAPI export complete (servers, security placeholder, defaults, aligned error schema), 3.1 if cheap; OpenAPI import documented as future work | redocly lint zero errors in CI |

### 19.2 Language and compiler

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Language definition | syntax described in README/CLAUDE.md; undocumented unions, WS, HEAD/OPTIONS accepted | written specification `docs/spec/` for lang 1.0 (models, enums, modules, actions, extends, Map, arrays, optional, defaults, deprecations, headers, middleware, status codes, imports incl. wildcard, constants, optional `veld 1` header); unions, WS, errors, constraints marked "reserved" and refused by emitters with a diagnostic | conformance suite: 30 to 40 fixtures with expected IR and diagnostics |
| Diagnostics | validator all errors with file:line; lexer/parser first error only, line only, sometimes wrong line; unterminated strings accepted | all errors from all stages, `file:line:col`, stable codes, snippet, hints, `--json`; error recovery continues after a syntax error | fixture expectations; CLI snapshot tests |
| Type system | flat `IsArray`/`IsMap` flags, no nesting; scalar names unusable as field names | recursive `TypeRef`; nested arrays and maps; reserved words usable as field and enum names; `date` and `datetime` distinct in every target | fixtures |
| IR | none (Go AST passed to emitters; `veld ast` dumps it) | `irVersion 1.0` with JSON Schema; `veld ir` command; the only cross-process contract | schema validation of every fixture's IR in CI |
| Imports | alias and relative work; `@alias/*` deletes the rest of the file silently; missing imports ignored | all three forms work; missing import is an error; aliases honoured by every command | fixtures; `veld validate` on `examples/microservices` |
| Robustness | fuzz-clean (measured today: 1.67M executions, no panic); 5 MB file under 1 s (measured today) | fuzz targets in CI nightly; same bounds kept | nightly fuzz job |
| Performance targets | not measured except the above | compile (parse, resolve, validate, lower) of a 2,000-line, 10-file project under 100 ms; `veld generate` for 50 models and 100 operations with the three tier-1 backends under 1 s excluding plugins; memory under 100 MB | benchmark in `evaluation/perf/` run at RC |
| Formatter | corrupts files, panics | lossless, idempotent, `fmt --check` | idempotence test on all fixtures |
| Breaking-change detection | dead in `generate`; shallow classifier | works in `generate --strict` (exit 4), `diff --json`; classifier covers models, nested models, enums, headers, middleware, status codes, paths, methods | fixture pairs per breaking class |
| Language metadata | `internal/language` unused by the compiler | single source of truth consumed by lexer, parser, LSP, grammars; drift test | CI |

### 19.3 CLI and generation safety

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Output writing | `RemoveAll` of output dirs before parsing; user files and even sources deleted; non-atomic | staged writer, `.veld-manifest.json`, swap on success, unmanaged files refused, atomic per file, previous output kept on failure | output-safety tests in CI |
| `clean`, `dry-run`, `init`, `watch` | `clean` deletes any directory; `dry-run` writes cache; `init` overwrites; `watch` wipes on bad saves | `clean` removes manifest files only; `dry-run` writes nothing; `init` never overwrites; `watch` keeps last good output, handles SIGTERM | CLI tests |
| Determinism | not deterministic (measured today: 1 of 6 runs differs) | byte-identical across runs and across Windows/Linux/macOS | CI determinism job |
| Config | flat beats nested; unknown keys ignored; `--out` sometimes ignored; `sh -c` hooks | nested wins; unknown keys warn; `--out` overrides; hooks via platform shell, failures fail the run, `--no-hooks` | config tests |
| Machine readability | none; colour always; exit 1 for everything | `--json` on validate/generate/lint/diff/doctor/version; `NO_COLOR`; exit codes 0/1/2/3/4 documented | CLI tests |
| Commands | `doctor` unregistered; registry commands only under `veld registry` | `doctor` reports toolchains, plugins, tiers; top-level `login/logout/push/pull` aliases | tests |
| Incremental cache | exists, always wiped | removed | n/a |

### 19.4 Generated code (tier 1)

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Node backend | runs under tsx; fails `tsc --strict`; `/:id` shadows static routes (middleware bypass); 500 leaks exception text; validation off and shallow | compiles under `tsc --strict`; static-before-param ordering from the IR; one error shape, no leak; validation on by default with enum, element type, integer, format, optional/null checks and query coercion; interfaces instead of classes; Express adapter and plain mode | compile + round trip in CI on 3 OSes |
| TypeScript client | path params unencoded; arrays lossy; no timeout/auth hook; strict errors | `encodeURIComponent`; repeated keys for arrays; `fetch` injection, headers callback, `timeoutMs`, per-call `RequestInit`; strict clean | round trip incl. `a/b?x` and arrays |
| React hooks | compile once SDK fixed | wrap the client; typed query keys | compile + a hook test |
| Python backend and client | routes have a syntax error; no registration; `Decimal` import missing; headers empty; malformed JSON gives 500 | `compileall` and `mypy --strict` on types clean; Flask adapter and plain mode; validators with `Decimal`; headers and typed query bound; 400 on malformed JSON; service receives models, not dicts; client SDK accepts its own models | compile + round trip |
| Go backend and client | `undefined: mux`; no validation; query/headers nil; date and datetime conflated | builds and vets; net/http and chi adapters; generated validators; query and header binding; `date` as a string-wrapped type, `datetime` as `time.Time`; client with query support | compile + round trip |
| Cross-language | not tested | Node, Python and Go servers each called by the three clients on one fixture | matrix job |
| Zero-dependency claim | true only for TS types/clients | precise and true for types, clients, validators in all tier-1 languages; adapters import only the user's framework | dependency grep in CI |
| Tier 2 (vue, svelte, javascript, typesonly, java, csharp, php, docs, graphql, schema) | mostly uncompilable | compile-checked nightly or demoted; emit a warning banner; README says "experimental" | nightly job |
| Removed | rust, angular, dart, kotlin, swift, mock, dockerfile, cicd, envconfig | absent from the binary; history tagged `frozen/<name>` | `veld doctor` |

### 19.5 Extensibility (plugins)

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Mechanism | in-process Go registry only | VEP v1: executables speaking JSON over stdio, `--describe` manifest, capability declarations, host-mediated writes, timeout and size caps, version pinning | protocol tests with a stub plugin |
| SDK | none | `@veld/emitter-sdk` (TypeScript) with IR types generated from the schema, `defineEmitter`, test harness, `npm create veld-emitter` | SDK tests; hello emitter |
| Reference plugins | none | `veld-emitter-hello` on npm; `veld-emitter-docs` extracted from the built-in docs generator (if B-041 lands) | acceptance test on Windows and Linux from a clean project |
| Performance target | n/a | plugin invocation overhead (spawn, handshake, 1 MB IR transfer) under 200 ms on the reference laptop | measured at RC |
| Trust | n/a | documented: plugins are trusted code; no sandbox; path containment, timeout, pinning enforced | docs and tests |

### 19.6 Editor support

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Language server | single file; byte offsets; first error; no imports; no MethodNotFound; hangs clients on unknown methods | compiler-API based; overlay buffers; cross-file diagnostics, completion (context-aware, imported models), hover, definition, document symbols, formatting; references if time; UTF-16 correct; protocol conformant; debounced; `recover` | scripted session test in CI |
| Performance target | measured today: 34 ms per change on a small file; 745 ms on 2.4 MB blocking hover | diagnostics under 100 ms for the 2,000-line, 10-file project; hover unaffected by concurrent edits | measured at M4 |
| VS Code | 1,676-line TypeScript re-implementation, commands dead, no tests | thin `vscode-languageclient` launcher; generated grammar and snippets; commands wired; binary discovery with checksum-verified download; integration test; published to the Marketplace | extension test in CI; Marketplace listing |
| JetBrains | 4,090-line hand-written PSI plugin, drift | highlighting only plus LSP4IJ server definition (limited support), built nightly; Marketplace publication only if reserve allows | manual verification with screenshots |

### 19.7 Registry, packages and self-hosting

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Deployable | `veld registry serve` inside the CLI | `veld-registry` binary and Docker image; CLI no longer links PostgreSQL, SMTP or the UI | binary dependency check |
| Security | 2FA bypass; storage traversal; scopes unenforced; XSS sinks; no limits; no rate limits; sessions unrevocable | typed JWTs; scopes enforced by one helper; expiry; revocation; last-owner guard; content-addressed storage; manifest validation; limits; rate limiting; TOTP replay protection (or TOTP disabled); escaping and CSP; security headers; Secure cookie; CORS allow-list | authz matrix and abuse tests in CI; ASVS L1 checklist |
| Integrity | optional, self-referential checksum; `http://` accepted | checksum always required, anchored by `veld.lock.json`; `https` default | MITM tests |
| Packages | no manifest, no lockfile, "latest" by publish time | `veld.package.json`, `veld.lock.json`, semver resolution (exact, caret, x-range), cache, offline, scoped registries bound to one URL, immutable versions with yank and deprecate | reproducible `veld install --frozen` on two machines |
| Operations | no health endpoint, no migrations table, no backup, no TLS, no tests | `/healthz`, `/readyz`, versioned forward-only migrations, JSON logs, config check, Compose with Caddy TLS, `backup`/`restore`/`verify` commands, runbook | timed restore drill (target RTO 1 h, RPO 24 h) |
| Performance target | n/a | publish of a 1 MB package under 2 s; download under 500 ms on a LAN; 50 concurrent downloads without error | load test at RC |
| Scale assumptions | n/a | 50 users, 500 versions, 5 publishes and 500 downloads per day on 2 vCPU / 4 GB | documented |

### 19.8 Distribution and releases

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Channels | 8 wrappers, most placeholders; npm downloads `latest` without checksum; no LICENSE | GitHub Releases (6 platform builds incl. windows-arm64), npm, Homebrew tap, `go install`, pip; others moved to experimental | clean-machine smoke tests |
| Integrity | `checksums.txt` published, never verified | checksums verified by every wrapper; cosign signatures; SBOM; GitHub provenance attestation; actions pinned by SHA | release pipeline |
| Versioning | `0.1.0` hardcoded; tags to v0.3.24 | `1.0.0` stamped by GoReleaser; changelog; per-product tags | `veld --version --json` |

### 19.9 Quality, testing and CI

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Tests | 523 functions, substring assertions; 20 packages untested incl. the registry server | conformance suite; golden outputs; compile tests; round-trip harness; cross-language matrix; determinism; output-safety; registry handler/authz/race/limit tests; LSP scripted session; extension integration test; fuzz targets | CI |
| CI | ubuntu-only unit tests | fast job on 3 OSes under 10 minutes; round-trip job; registry job with PostgreSQL; nightly tier-2, fuzz, installers, JetBrains, plugin compatibility | workflow files |
| Coverage (indicative) | parser 54%, validator 54%, node 20%, python 29%, registry 0% | not a target in itself; the requirement is that every tier-1 claim has a compile and round-trip test | n/a |
| Docs correctness | 26 documented mismatches; examples that do not compile | support matrix generated from CI; tutorial examples extracted and validated; getting-started snippets taken from generated output | docs CI |

### 19.10 Governance, documentation and academic artifacts

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Repository | 24.8 MB tracked, 82% a stray binary; scratch dirs; no LICENSE, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, CODEOWNERS, CHANGELOG | clean tree under 5 MB; MIT LICENSE; all governance files; DCO sign-off; CODEOWNERS; ADRs in `docs/adr/`; roadmap | repository |
| Documentation | README, scraped docs page, stale developer guide, four plan versions | spec, tutorials for the three journeys, plugin author guide, operator runbook, API reference, support matrix, architecture overview matching the code | docs site built in CI |
| Academic | proposal documents with inconsistent numbers | thesis report (11 chapters, chapter 17 §17.3) with evidence links; evaluation results for Q1 to Q5 and the user study; slides; live demo and recorded fallback | submission folder |
| Community | 4 git authors; no external contributions | at least one external contribution merged; `good first issue` set; six-month security-fix commitment | repository |

### 19.11 Extended end state (added 2026-10-21 by ADR-012; see chapter 20)

The project owner added the previously deferred items as an ordered queue behind the core. If the queue completes, the end state additionally contains, each at the tier-1 bar:

| Addition | End state | Verified by |
|---|---|---|
| Per-field constraints | `@min/@max/@minLength/@maxLength/@pattern/@format` enforced by generated validators in TS, Python, Go and reflected in OpenAPI | boundary-value fixtures |
| Database schema | `@entity`-based SQL (PostgreSQL, SQLite) and Prisma output with relations, indexes, quoted defaults, migration diffs | schema applied in CI |
| Auth schemes | bearer, API key, basic, OAuth2 metadata in the contract; OpenAPI security; middleware wiring; client auth config | protected-route round trip |
| Discriminated unions | typed unions across the three stacks with validators and OpenAPI `oneOf` | fixtures |
| Streaming | SSE outputs, chunked downloads, multipart uploads with size limits | round trips |
| WebSocket | typed bidirectional actions for Node, Python, Go servers and the three clients (declared adapter dependencies) | echo round trips |
| Docker, CI and env generators | tier-2 tools whose images build nightly | nightly job |
| OpenAPI import | subset importer with an unsupported-constructs report; export/import round trip on fixtures | round-trip test |
| Package signing | Sigstore keyless or minisign; verified on pull when the lockfile demands it | tamper test |
| WebAssembly plugins | sandboxed transport with a Wasm hello emitter | sandbox and latency tests |
| Registry-hosted plugins | `veld plugin install` with signature verification and trust policy | local registry test |
| Tier-2 revival | Rust backend compiles with a real router; Angular, Dart, Kotlin, Swift compile nightly; `veld mock` works | nightly job |
| High availability | replicated Compose variant with S3-compatible storage and a failover drill | drill record |

Performance targets added: WebSocket echo round trip under 5 ms locally; SSE first byte under 50 ms; Wasm plugin overhead under 2 times the stdio transport; HA failover under 60 s.

### 19.11a What will still be missing at submission (stated limitations, revised)

- Any queue item removed by the automatic cut on 2027-05-02 (order in chapter 20 §20.6); the thesis lists them as future work with the reason.
- Tier-2 targets compile but are not round-trip tested.
- JetBrains support is limited to highlighting plus LSP4IJ, possibly unpublished.
- Evaluation is small-N with student participants; performance figures are from one reference machine.
- Kubernetes manifests, federated registries, gRPC, generics beyond `Map` and arrays are out by decision.

### 19.12 Completeness and production-readiness estimate (judgement with explicit weights)

Two different percentages are asked for and they are not the same thing. "Completeness" measures how much of the 1.0.0 end state (sections 19.1 to 19.10) exists and works at 6a47f81. "Production readiness" measures how much of what a production user needs (correctness, safety, security, tests, operability) is in place for that area. Scores are the lead reviewer's judgement from the verified inventory in chapters 04 and 05; the weights express importance to the thesis. Another reviewer could reasonably move any cell by 10 points; the totals are robust to that.

| Area | Weight | Complete vs end state | Production ready | Basis |
|---|---|---|---|---|
| Language, lexer, parser, validator, loader | 15% | 65% | 55% | fuzz-clean, fast, all semantic errors reported; missing spans, all-errors syntax reporting, nested generics, IR, wildcard import, formatter |
| Generation safety and CLI | 10% | 40% | 15% | commands exist; output path destructive, gate dead, non-deterministic, config precedence wrong |
| Tier-1 emitters (Node/TS/React, Python, Go) | 20% | 50% | 25% | Node/TS about 70% and runs; Python about 35% and Go about 40% (do not compile); no compile tests |
| Validation of inputs | 5% | 30% | 20% | opt-in, `typeof` only; documented Zod/Pydantic absent |
| External plugin system | 10% | 0% | 0% | nothing exists |
| Language server and editors | 10% | 30% | 20% | single-file LSP; VS Code works on its own engine; JetBrains hand-written |
| Registry and package resolution | 10% | 45% | 10% | publish/pull happy path works; six security-high defects; no lockfile, manifest or immutability |
| Self-hosting and operations | 5% | 10% | 5% | no health endpoint, migrations table, backups, TLS, tests |
| Distribution and releases | 5% | 40% | 25% | `go install` works, release builds exist; npm/pip need work, no checksums verified, no signing |
| Testing and CI | 5% | 35% | 20% | 523 unit tests; nothing compiles generated code; ubuntu only |
| Documentation and governance | 5% | 35% | 15% | extensive docs with 26 verified mismatches; no LICENSE or governance files |
| **Weighted total** | 100% | **about 40%** | **about 20%** | |

Reading: roughly two fifths of the graduation end state exists, and roughly one fifth of what production use requires. Measured against the current README's claims (18 targets, Zod, deterministic, never touches user files) the completeness would be lower, around 30%, because several claimed capabilities do not work. The single most valuable slice, Node backend plus TypeScript client, is individually about 70% complete and about 45% production ready: it runs end to end, but on a generator that can delete the user's files and with a route-ordering defect that bypasses middleware.

What moves the numbers fastest (chapter 15, M1): the staged output writer, the gate ordering fix, the header-identifier fix, the Python and Go compile fixes and the compile-test CI job. Those items alone lift production readiness of the vertical slice from about 45% to about 75% within four weeks, because they remove the data-loss and does-not-compile failures rather than adding features.

### 19.13 One-paragraph summary

At 6a47f81 Veld is a promising compiler frontend attached to a destructive output path, an unverified emitter matrix and an unsafe registry, described by documentation that claims more than exists. At 1.0.0 it should be a small, specified language with a conformance suite and a versioned IR; a safe, deterministic generator with three verified backends, three verified clients and a real plugin protocol with a published SDK; one language engine serving VS Code and, in limited form, JetBrains; a hardened registry with lockfile-based reproducibility and a tested self-hosting path; signed releases through a few trustworthy channels; and an evaluation that measures the claims. The difference between the two states is the content of the thesis.



## 20. Extended scope plan (decision ADR-012, 2026-10-21)

This chapter is authoritative for everything the project owner added back on 2026-10-21. It extends, and where it conflicts overrides, chapters 03, 09, 10, 13, 15, 16, 18 and 19. The core plan (M0 to M3, the chapter 19 end state) is unchanged and protected; the additions form an ordered queue behind it.

### 20.1 What is added, with the design each item must follow

Every item below ships only when it meets the tier-1 bar: compiles on three OSes, round-trip tested, deterministic, declared in the emitter capability manifest, documented from CI evidence. Zero-runtime-dependency exceptions are listed explicitly.

| # | Item | Language or product surface | Design constraints (binding for implementers and agents) | Zero-dep exception | Estimate (h) |
|---|---|---|---|---|---|
| Q1 | Per-field constraints | `@min(n) @max(n) @minLength(n) @maxLength(n) @pattern("re") @format(email\|uri\|...)` on fields; IR `constraints`; validators in TS, Python, Go; OpenAPI mapping; `veld lint` warns on unsatisfiable combinations | validators generated, no library; regex dialect limited to the RE2-compatible subset so the three languages agree; tests include boundary values | none | 30 to 50 |
| Q2 | Database schema generator (proper) | `@entity` opt-in on models; `@id`, `@unique`, `@index`, `@table("name")`, `@relation(Model, field)`; dialects PostgreSQL and SQLite; Prisma output; quoted identifiers and defaults; `veld schema --migrations` writes a timestamped diff against the previous manifest (full schema always emitted) | DTOs are never tables unless `@entity`; `Map` and arrays become JSON columns with a documented note; relations only one-to-many and many-to-one in 1.0 | none | 30 to 60 |
| Q3 | Authentication schemes in the contract | module or action `auth: bearer` / `apiKey(header: "X-Key")` / `basic` / `oauth2(authorizationCode, scopes: [...])`; OpenAPI `securitySchemes`; server adapters wire the existing `middleware` hook with a typed `AuthContext`; clients get an `auth` config (static token, callback) | Veld never issues or validates tokens itself; it wires the user's handler | none | 30 to 60 |
| Q4 | Discriminated unions | `type Shape = Circle \| Square @discriminator(kind)`; IR `union`; TypeScript discriminated union; Python `Union` with a validator switch; Go interface plus custom `UnmarshalJSON`; OpenAPI `oneOf` with `discriminator`; validators check the discriminator first | undiscriminated unions are a validator error in 1.0 | none | 40 to 70 |
| Q5 | Streaming | `output: Event[] @stream(sse)` generates a server generator/iterator and a client async iterator; `output: file` chunked download with content-type; `input: file` multipart upload with a size limit from `@maxSize(n)` | all three stdlibs support SSE and multipart; uploads stream to a temp file, never to memory | none | 40 to 80 |
| Q6 | WebSocket actions (Node first) | `action Chat { method: WS path: /ws input: ClientMsg emit: ServerMsg }`; JSON messages typed both directions; validators applied per message; server adapter for Node; browser client uses the native `WebSocket` API | server adapter imports `ws` (declared, documented exception) | Node server: `ws` | 40 to 70 |
| Q7 | WebSocket for Python and Go, plus clients | Python adapter (`websockets`), Go adapter (`golang.org/x/net/websocket` or `nhooyr.io/websocket`), Python and Go clients; echo and typed round-trip tests | declared exceptions | Python: `websockets`; Go: one declared library | 40 to 80 |
| Q8 | Dockerfile, CI and env generators (fixed, tested) | templates per tier-1 stack; pinned Veld version with checksum verification in the Dockerfile; no bundled registry Postgres; nightly job builds each image and hits the health endpoint | tier-2 tools; never run by default; `tools` config block finally honoured | none | 15 to 30 |
| Q9 | OpenAPI import | `veld import openapi <file>` writes `.veld` files plus a report of unsupported constructs; subset: object/primitive/array/enum schemas, `allOf` single inheritance, `oneOf` with discriminator (to unions), JSON request bodies, path/query/header params, status codes, security schemes (to Q3) | import is lossy by design and says so; round trip `import(export(x)) == x` holds for every conformance fixture | none | 40 to 80 |
| Q10 | Package signing | `veld push --sign` with Sigstore keyless (GitHub or Google OIDC) or a minisign key pair; registry stores signatures; lockfile `"signed": true` makes `veld pull` require a valid signature; `veld-registry` verifies on publish | checksum path stays the default; signing is opt-in per package | none in the CLI (uses the Sigstore HTTP API) | 30 to 60 |
| Q11 | WebAssembly plugin transport (sandboxing) | manifest `transport: "wasm"`; host runs the module with Extism on wazero, no `AllowedPaths`, no network; same `EmitRequest`/`EmitResponse` JSON; hello emitter compiled with TinyGo and the Rust PDK; `veld doctor` shows the transport | native (stdio) plugins remain allowed but print "not sandboxed" once per run; Wasm plugins run without a trust flag | Go host dependency `extism/go-sdk` (pure Go) is a compiler dependency, not a generated-code dependency | 60 to 120 |
| Q12 | Registry-hosted plugins | package `kind: "plugin"` carrying a Wasm module (or per-platform binaries); `veld plugin install @org/name@ver` verifies signature (Q10) and records it in the lockfile; trust policy: Wasm runs by default, native requires `--trust-native` | depends on Q10 and Q11 | none | 40 to 80 |
| Q13 | Tier-2 revival: Rust backend (real axum router and main), Angular on the TS SDK, Dart, Kotlin and Swift client fixes, mock server wired to `veld mock` serving OpenAPI examples | all compile-checked nightly; Rust additionally `cargo check` in nightly; promoted to tier 1 only by the ADR-005 promotion rule | none | 80 to 130 |
| Q14 | High-availability hosting | Compose variant with two registry replicas behind Caddy, PostgreSQL primary plus streaming replica (or a managed database), S3-compatible storage implementation (MinIO in Compose); documented failover; drill in M6 | the single-host Compose stays the default | none | 40 to 70 |

Total additions: 555 to 1,040 hours.

### 20.2 Capacity under the AI-assisted assumption

| Quantity | Baseline (chapter 15) | Extended |
|---|---|---|
| Committed technical hours (five members, exam-adjusted, 15% coordination) | 1,106 | 1,106 actual |
| AI-assisted multiplier (1.5 on the 60% of effort that is implementation and tests; 1.0 elsewhere) | not applied | 1.3 blended |
| Equivalent committed capacity | 1,106 | 1,438 |
| Reserve (20%) | 277 | 360 |
| Planned technical work | 694 to 1,139 (mid 916) | 1,251 to 2,169 (mid 1,710) |
| Fit at the midpoint | 190 h of reserve left | 88 h of reserve left |
| Fit at the high estimate | 33 h of reserve used | 371 h short |

Sensitivity: with no AI gain (multiplier 1.0) the midpoint overshoots capacity by about 330 h, which is Q14, Q13, Q12, Q11 and Q10 in reverse order. At 16 h per week with the 1.3 multiplier, equivalent capacity is about 1,920 h and the midpoint fits with about 200 h of reserve. The multiplier is measured at the M1 exit: the integration owner records actual hours per backlog item; if the ratio of estimate to actual is below 1.15, chapter 20 §20.6 applies.

### 20.3 Ordered queue and schedule

Core (M0 to M3) is unchanged. The queue starts in S8 (2027-02-08) and is worked in this order; an item does not start until the previous one in the same owner's lane is accepted.

| Order | Item | Owner (lane) | Co-owner | Target sprints | Hard dependency |
|---|---|---|---|---|---|
| 1 | Q1 constraints | T1 (syntax, IR, validator) | T2, T4 (validators) | S8 to S9 | IR v1 (B-025) with reserved fields |
| 2 | Q2 database schema generator | T4 | T1 (annotations) | S8 to S9 | IR `entity` fields |
| 3 | Q3 auth schemes | T1 (syntax), T2 (SDK, middleware wiring, OpenAPI) | T4 (Python, Go wiring) | S9 to S10 | Q1 |
| 4 | Q4 unions | T1 (syntax, IR), T2 and T4 (emitters) | | S10 to S11 | Q1 |
| 5 | Q5 streaming | T2 (Node), T4 (Python, Go) | T1 (syntax) | S11 to S12 | IR `stream` |
| 6 | Q6 WebSocket Node | T2 | T1 (semantics) | S11 to S12 | Q5 syntax work |
| 7 | Q8 Docker/CI/env generators | T5 | | S11 | B-016 compile jobs |
| 8 | Q7 WebSocket Python, Go, clients | T4 | T2 | S12 to S13 | Q6 |
| 9 | Q9 OpenAPI import | T1 | D1 (report format) | S12 to S13 | Q3, Q4 |
| 10 | Q10 package signing | T3 | T5 (release keys) | S11 to S12 | B-044 lockfile |
| 11 | Q11 Wasm plugin transport | T2 | T4 (Wasm hello) | S12 to S13 | B-037 host runner |
| 12 | Q13 tier-2 revival (Rust, Angular, Dart, Kotlin, Swift, mock) | T4 (Rust, mock), T5 (Angular, mobile clients) | | S12 to S13 (parallel, lowest priority in each lane) | B-049 nightly job |
| 13 | Q12 registry-hosted plugins | T3 | T2 | S13 | Q10, Q11 |
| 14 | Q14 high availability | T3 | T5 | S13 | B-051 |
| freeze | extended-feature freeze and automatic cut review | all | | 2027-05-02 (end of S13) | |

Gates (updated): core feature freeze 2027-04-18 (unchanged); extended-feature freeze 2027-05-02; RC 2027-05-16; readiness 2027-05-31. Queue items accepted after May 2 are not possible; items with open P1 defects at the RC gate are demoted to tier 2 ("experimental") in the release notes rather than removed.

### 20.4 Per-member additions (midpoint hours, to validate against the survey)

| Member | Baseline midpoint (chapter 16) | Added by the queue | Total | Equivalent capacity (288 committed + 72 reserve) | Gap at midpoint |
|---|---|---|---|---|---|
| T1 | about 215 | constraints 40, unions 55, auth syntax 15, OpenAPI import 60, streaming/WS semantics 20 = 190 | 405 | 360 | 45 over |
| T2 | about 200 | streaming 50, WS Node 55, auth SDK/middleware 30, Wasm transport 90 = 225 | 425 | 360 | 65 over |
| T3 | about 200 | signing 45, hosted plugins 60, HA with S3 storage 75 = 180 | 380 | 360 | 20 over |
| T4 | about 210 | DB schema 45, WS Python/Go 60, Rust 40, mobile clients 30, mock 9, constraint and union validators 20 = 204 | 414 | 360 | 54 over |
| T5 | about 190 | Docker/CI/env 22, Angular 10, CI for all queue items 35, auth OpenAPI 10, AGENTS.md enforcement 20, nightly expansion 25 = 122 | 312 | 360 | 48 under |

Reading: at the midpoint every lane except T5 is over its equivalent capacity, which is why the queue is ordered and the cut is automatic. T5 absorbs cross-cutting CI work and is the first backup for T2 and T4 queue items. If the survey shows a member with less than 12 h per week, that lane's lowest-order queue items are dropped first.

### 20.5 Rules that protect the end state (summary; full text in `AGENTS.md`)

- No queue item starts before its hard dependency is accepted; "accepted" means merged with tests, docs and changelog, and green in CI.
- No queue item may weaken a core guarantee: output safety, determinism, zero runtime dependencies (except the declared WebSocket adapters), the IR as the only cross-process contract, the import-boundary rules, the tier promotion rule.
- Every queue item adds conformance fixtures first, then the implementation, then compile and round-trip tests, in that order, in separate small commits.
- Lean code: a feature that needs more than about 1.5 times its estimate is paused and re-designed in a one-page note before continuing.
- Tests are not optional and are never weakened to pass; coverage floors in `AGENTS.md` apply to every package touched.

### 20.6 Automatic cut order (applied without a meeting)

If the M1 exit measurement shows a multiplier below 1.15, or any core gate slips by a sprint, drop from the end of the queue: Q14, Q12, Q13, Q11, Q10, Q9, Q7, Q8, Q6, Q5, then Q4. Q1, Q2 and Q3 are kept unless the core itself is at risk, in which case the chapter 15 §15.6 fallback applies to the core first and the queue is suspended entirely.

### 20.7 Research questions added by the extension

- Q6: Does AI-assisted development change the estimate-to-actual ratio for a student team, and does the ratio differ between greenfield features (the queue) and defect fixing (M1)? Measured from the tracker; reported with the multiplier assumption it validates or refutes.
- Q7: What does a WebAssembly plugin transport cost in latency and author effort compared with the stdio transport, for the same emitter? Measured with the hello emitter in both transports.

### 20.8 Updated statement of what is not in 1.0.0

Only what fails the automatic cut, plus: Kubernetes manifests, federated registries, gRPC or binary transports, a hosted public registry operated by the team, and any emitter beyond the registered list. Everything else the project owner asked for is in the queue above.



# Part II: Architecture decision records



## ADR-001: Retain Go for the compiler, CLI, language server and registry server

- Status: PROPOSED (for team acceptance in Sprint 0, by 2026-11-01)
- Date: 2026-10-21
- Deciders: T1 (owner), T2, T3, T5; reviewer: whole team
- Evidence: chapter 06 decision matrices; chapter 02 baseline (build, vet and all tests green at 6a47f81 on Go 1.26.0)

### Context

The reviewed revision is a single Go module (`github.com/Adhamzineldin/Veld`) of 43,886 non-test and 12,180 test lines (tracked `.go` files, measured with `wc -l` on `git ls-files`). The pipeline (lexer, parser, validator, loader, emitters, LSP, registry client and server) builds and passes `go test ./...` on Go 1.26.0. The team has roughly 1,100 committed technical hours between now and the readiness target (chapter 15). The prompt asked whether a rewrite in Rust or TypeScript would be a better foundation.

### Decision

Keep Go for every process that ships as a binary: the compiler and CLI (`cmd/veld`), the language server (same binary), and the registry server (to be split into its own binary, see ADR-004). Raise `go.mod` from `go 1.22` to `go 1.26`, the oldest release still supported on 2026-10-21 (Go 1.26 released 2026-02-10 and Go 1.27 released 2026-08-19 per go.dev/doc/devel/release; each major release is supported until two newer ones exist).

TypeScript is used only where it is the native language of the host: the VS Code extension, the website, the registry SPA, and the emitter plugin SDK (ADR-003). Kotlin remains only in the JetBrains plugin.

### Reasons

1. Delivery risk. Re-implementing 44k lines in another language with part-time students in 32 weeks is not credible; a rewrite would consume the entire capacity and leave no time for evaluation and the report.
2. The weaknesses found (chapter 05) are in boundaries, output safety, registry robustness and the emitter matrix, not in the choice of Go. None of them is fixed by changing language.
3. Go's single static binary, cross-compilation and `go:embed` match the distribution model (wrappers download one binary per platform).
4. Contributor accessibility: Go's small surface helps a student team and future contributors more than Rust's learning curve; Node-based single-binary packaging is still less mature than Go's.

### Alternatives rejected

- Rust: better for a long-lived compiler with heavy performance needs; Veld's inputs are small (contracts of hundreds of lines), so performance is not a differentiator. Cost: team learning plus full rewrite.
- TypeScript: easiest for emitter authors, which is why the plugin SDK is in TypeScript, but as the compiler host it would require Node at runtime or Node SEA/bun packaging and would contradict the single-binary promise.

### Consequences

- The compiler API (chapter 08) is a Go package; LSP and CLI consume it in-process.
- Emitter authors outside Go are served by the plugin protocol, not by the host language.
- If, by the migration decision deadline (2026-11-29), the frontend audit had shown the parser to be unsalvageable, the response is a parser rewrite in Go behind the same API, not a language change. The audit (chapter 05, F-FE items) does not show that.

### What would reverse this decision

A demonstrated, measured need that Go cannot meet (none identified), or a team skills survey (chapter 16) showing that fewer than two technical members can work in Go while four or more are strong in Rust or TypeScript.



## ADR-002: Separate the syntax tree from a versioned, serializable IR

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T1 (owner), T2; reviewer: T4
- Evidence: `internal/ast/ast.go` is passed directly to every emitter (`internal/emitter/emitter.go:43-45`), `veld ast` dumps that struct as JSON, and `CollectTransitiveModels` plus per-emitter type-mapping code re-derive the same facts in each emitter (`internal/emitter/helpers.go`, `internal/emitter/lang/*`).

### Context

Today "only AST JSON passes between stages" (CLAUDE.md). The AST is a syntax-shaped Go struct (`Model.Extends` as a name, `Field.Type` as a string with `IsArray`/`IsMap` flags, status codes parsed by emitters). Every emitter resolves inheritance, path parameters and type references itself, which is where emitters diverge (chapter 09). There is no stable, versioned representation to hand to an external plugin.

### Decision

1. Keep the AST as the output of parsing, with source spans, and keep it internal.
2. Add `internal/ir`: a normalized, fully resolved, versioned representation (`irVersion` "1.0") produced by semantic analysis. It contains flattened models (inheritance resolved, with the `extends` chain preserved as metadata), structured type references (scalar, array, map, ref, optional), services and operations with pre-extracted path parameters, query and header bindings, output status codes, deprecations, and a source map from every IR node to file, line and column.
3. Publish a JSON Schema for the IR and a `veld ir` command. The IR JSON is the only thing that crosses a process boundary (plugins, tests, tooling). Inside the process, Go structs are used directly; there is no serialization between lexer, parser, validator and built-in emitters.
4. Built-in emitters migrate from AST to IR incrementally: an adapter builds IR from AST so that emitters can be moved one at a time; emitters not migrated by the Beta gate are frozen (chapter 09).

### Reasons

- One place to resolve semantics removes the per-emitter divergence that the audits found.
- Plugins need a stable contract; the AST is not one.
- Serializing only at process boundaries keeps the LSP fast and the code simple.

### Alternatives rejected

- Serialize at every stage (CLAUDE.md's "only AST JSON passes between stages" taken literally): no benefit inside one process, costs time and complexity.
- Make the AST itself the public contract: it would freeze syntax details into the plugin API and force every plugin to re-implement inheritance and path parsing.

### Consequences

- `internal/ir` and its JSON Schema are versioned artifacts (ADR-009).
- Conformance fixtures (chapter 08) pair `.veld` inputs with expected IR JSON and expected diagnostics.
- `veld ast` remains for debugging but is documented as unstable.



## ADR-003: External emitters run as executables speaking versioned JSON over stdin/stdout

- Status: PROPOSED (accept by 2026-11-01; protocol frozen at the end of M3, 2027-02-07)
- Date: 2026-10-21
- Deciders: T2 (owner), T1, T4; reviewer: T3 (security)
- Evidence: chapter 10; no plugin mechanism exists at 6a47f81 (`internal/emitter/emitter.go:90-118` is an in-process registry).

### Decision

Adopt the protoc/buf model: an emitter is an executable `veld-emitter-<name>` (or an explicit `command` in config). The host sends one `EmitRequest` JSON document (protocol version, host version, IR v1, options) on stdin; the plugin writes one `EmitResponse` JSON document (files, diagnostics, summary) on stdout and logs on stderr. The host performs all file writes after validating paths, applies timeouts and size caps, and records files in the output manifest. A `--describe` call returns the plugin manifest for compatibility checks.

WebAssembly (Extism on wazero) is deferred; the JSON schema is designed so a Wasm transport can carry the same documents later. HashiCorp go-plugin and in-process Go plugins are rejected (chapter 10, section 10.2).

### Reasons

- Any language can implement it with a JSON library; no Veld-specific runtime required.
- Process isolation and host-mediated writes give output safety without a sandbox.
- The approach is proven at scale by protoc plugins and buf local plugins (buf.build/docs/generate, accessed 2026-10-21 via search summary).

### Security statement (must be documented verbatim for users)

Plugins are trusted code. The host does not restrict a plugin's filesystem or network access. The host does guarantee that a plugin cannot write outside its configured output directory through the protocol, that a hung plugin is killed, and that a plugin is only executed when it is named with a pinned version in `veld.config.json`.

### Consequences

- Requires ADR-002 (IR) first.
- A TypeScript SDK, a hello emitter, and one extracted built-in emitter are the acceptance tests (chapter 10, section 10.4).
- The plugin protocol is versioned separately from the IR and the compiler (ADR-009).



## ADR-004: Move the registry server into its own binary (`veld-registry`) in the same repository

- Status: PROPOSED (accept by 2026-11-01; implement in M4)
- Date: 2026-10-21
- Deciders: T3 (owner), T5 (distribution); reviewer: T1
- Evidence: `cmd/veld/serve.go` wires `internal/server` into the CLI; `go.mod` links `github.com/lib/pq` into every CLI build; the embedded registry SPA (`internal/server/handlers/web.go`, `//go:embed`) ships inside the developer CLI; the tracked `veld` binary at the repository root is 20,450,930 bytes (`ls -la`, 2026-10-21). CLAUDE.md states "No separate registry binary exists" as a rule; this ADR changes that rule.

### Decision

Create `cmd/veld-registry` and move `veld serve` there. The CLI keeps `veld login/logout/push/pull` (client side). The registry is distributed as a Docker image (default self-hosting path, chapter 13) and as a standalone binary on GitHub Releases. The npm/pip/Homebrew wrappers ship only the CLI.

### Reasons

1. The developer CLI should not carry a PostgreSQL driver, SMTP code, TOTP, session handling and a SPA that no CLI user needs. Binary size, attack surface and dependency licensing all shrink.
2. Operators upgrade the registry on a different cadence from developers upgrading the CLI; one binary forces them to move together.
3. Separate binaries let CI build and test the server with a PostgreSQL service container without slowing every CLI job.

### Alternatives rejected

- Build tags (`-tags registry`) in one `cmd`: fragile, confuses wrappers that download "the" binary.
- A separate repository: adds release coordination for a six-person team; the registry shares `internal/registry` tarball and manifest code with the client. Monorepo retained (ADR-010).

### Consequences

- `CLAUDE.md` and docs must be updated (documentation currently asserts a single binary).
- The `packages/*` wrappers do not change.
- Migration cost: 6 to 10 hours (move command, update docs, add release target), excluding the hardening work in chapter 13.



## ADR-005: Emitter support tiers; an emitter is "supported" only if CI compiles and round-trips its output

- Status: PROPOSED (accept by 2026-11-01; apply in M2)
- Date: 2026-10-21
- Deciders: T4 (owner), T2; reviewer: T1
- Evidence (multi-emitter and Node audit tracks; the lead reviewer reproduced the Python and Go failures on the unmodified `veld init` scaffold):
  - Python default output: `SyntaxError` at `routes/auth_routes.py:13` (`LoginInputSchema(**body = data  # ...`); routes are never registered; the imported `schemas` module is never written (F-MULTI-03).
  - Go output: `undefined: mux` in every route file (F-MULTI-06, F-CLI-19).
  - Rust output has no router, no `axum` dependency, handler/trait type mismatches; it cannot compile (F-MULTI-07).
  - Java, C#, PHP emit framework-less classes with syntax errors (`Role role = "user"`); they neither compile nor route (F-MULTI-08).
  - Header names such as `Idempotency-Key` become identifiers in every language (F-MULTI-01, root cause `internal/parser/parser.go:961`).
  - Angular output is invalid TypeScript (F-NODE-15); Dart, Kotlin and Swift clients have compile or runtime defects (F-MULTI-09).
  - All emitter tests assert substrings; `go test ./...` is green while most outputs do not compile (F-MULTI-02, F-NODE-19).
  - Maintenance estimate for a full compile matrix of 8 backends and 10 frontends: 75 to 110 hours per month (audit estimate; assumptions: one maintainer per emitter, one breaking upstream change per quarter), which exceeds the team's entire committed capacity of roughly 35 hours per week.

### Decision

Three tiers, enforced by CI and shown by `veld doctor` and in the README support matrix.

| Tier | Meaning | CI requirement | Graduation members |
|---|---|---|---|
| 1 Supported | Output compiles and passes request/response round-trip tests for every conformance fixture on every push; documented; breaking changes announced | compile + round-trip on ubuntu, windows, macos (fast job) | Backends: `node-ts` (plain and Express adapter), `python` (Flask adapter and plain), `go` (net/http plain and chi adapter). Frontends: `typescript`, `react`. Service SDKs: TypeScript, Python, Go. Tools: `openapi`. |
| 2 Experimental | Output compiles for every fixture in a scheduled CI job; no round-trip guarantee; emitting prints a one-line warning; may ship types and interfaces only | compile-only in the nightly matrix with the needed toolchain | Frontends: `vue`, `svelte`, `javascript`, `typesonly`. Backends: `java`, `csharp`, `php`, `javascript`. Tools: `docs`, `graphql`, `schema` (SQL/Prisma) after the quoting fixes. |
| 3 Frozen / removed | Not registered in the release binary; code deleted from `main` (kept in history and tagged `frozen/<name>`) or kept behind `-tags experimental` only if a team member owns it | none | `rust` backend, `angular` frontend (until rewritten on the TS SDK), `dart`, `kotlin`, `swift` clients, `mock` (dead code), `dockerfile`, `cicd`, `envconfig` generators (download a 404 asset, bundle a default-credential Postgres), Rust/Java/C#/PHP service SDKs. |

Promotion rule: an emitter moves up a tier only when its CI requirement has been green for two consecutive sprints and it has an owner and a backup in chapter 16.

Demotion rule: a tier-1 emitter whose CI is red for more than one sprint is demoted to tier 2 for the release and the README is updated in the same pull request.

### Reasons

- The thesis is "correct server interfaces, routes, validation and clients from one contract". Shipping eighteen targets of which two work undermines it more than shipping six that are verified.
- The team cannot afford more than tier-1 round-trip plus tier-2 compile checks (chapter 09 cost table).
- Deleting frozen code reduces the surface that every cross-cutting refactor (output writer, IR migration) must touch.

### Alternatives rejected

- Keep everything and label it "beta": the labels already exist in docs and were not true; users cannot tell which targets work.
- Fix everything: 150 to 250 hours for Rust, Java, C#, PHP, Dart, Kotlin, Swift alone, with no round-trip tests, and ongoing cost that no one owns.

### Amendment 2026-10-21 (ADR-012)

The project owner's extended-scope decision moves the tier-3 items back into scope as queue items with the same promotion rule: `rust`, `angular`, `dart`, `kotlin`, `swift` and `mock` are revived as tier 2 (Q13, backlog B-071, B-072); `dockerfile`, `cicd` and `envconfig` return as tier-2 tools after being fixed and tested (Q8, B-066); WebSocket and streaming become tier-1 extended features for the three tier-1 backends (Q5 to Q7) with declared dependency exceptions. Tier 3 is therefore empty at 1.0.0 unless the automatic cut of chapter 20 §20.6 removes an item, in which case it is frozen as originally described. The tier bar and the promotion and demotion rules are unchanged.

### Consequences

- README, CLAUDE.md, website and `test.md` support claims must be rewritten (chapter 05, D-items).
- The academic report presents the matrix honestly: supported, experimental, frozen, with CI evidence for each.
- Rust and Angular are the first candidates for community contribution through the plugin protocol; Rust's current code is a useful starting point for a contributor but not for the team before the Beta gate.



## ADR-006: The language server is the single language engine; VS Code is the primary editor; JetBrains moves to LSP4IJ later

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T5 (owner), T1; reviewer: T4
- Evidence (LSP and editors audit track, chapter 05 F-LSP items):
  - `veld lsp` advertises only `textDocumentSync: 1`, completion, hover and definition (`internal/lsp/handler.go:234-243`), never resolves imports (`handler.go:261-273`, `324-365`), uses byte offsets for positions, and reports only the first error on a whole line.
  - The VS Code extension does not use the server at all: `editors/vscode/src/extension.ts:81-1675` is a 1,676-line TypeScript re-implementation of lexing, validation and completion; its three contributed commands and two settings are never registered.
  - The JetBrains plugin is a third implementation: about 4,090 lines of hand-written Kotlin PSI code (`VeldLexer.kt`, `VeldParser.kt`, `VeldAnnotator.kt`), with diagnostics from a `veld validate` subprocess on files on disk rather than the editor buffer.
  - `internal/language` is imported only by `cmd/generate-language`; the lexer, parser and LSP hard-code their own keyword lists, so the "single source of truth" claim does not hold for the compiler.

Three implementations of the language exist, and the two in editors already disagree with the compiler (both reject the valid `headers:` directive).

### Decision

1. The Go language server, built on the shared compiler API (ADR-002), is the only implementation of diagnostics, completion, hover, definition and (later) references, document symbols and formatting. Keyword and directive lists in the server come from `internal/language`, and a Go test asserts the parser accepts exactly those lists.
2. VS Code is the primary graduation editor. The extension becomes a thin `vscode-languageclient` launcher plus TextMate grammar generated from `internal/language`; the hand-written language service is deleted. Commands (`validate`, `generate`, `generate --dry-run`) are wired to the CLI via `veld.executablePath`.
3. Server discovery in VS Code: use `veld.executablePath` if set; otherwise `veld` on PATH; otherwise offer to download the matching release asset for the platform from GitHub Releases, verify its SHA-256 against the release's checksum file, and store it under the extension's global storage. Version compatibility is checked with `veld --version --json`; a mismatch shows a message with the required version. Offline with no binary: syntax highlighting only, with a status-bar hint.
4. JetBrains receives limited support at graduation: the existing PSI plugin keeps syntax highlighting only; its hand-written annotator and completion are removed to stop drift; diagnostics, completion and navigation are provided through LSP4IJ pointing at the same server. LSP4IJ 0.21.x is current and listed as compatible with all IntelliJ flavours (plugins.jetbrains.com listing, nightly 0.21.1 dated 2026-10-20, search summary accessed 2026-10-21); JetBrains' own LSP API became available to all IntelliJ IDEA users with the 2025.3 unified distribution and is being open-sourced in the 2026.2 cycle (blog.jetbrains.com, search summaries). The JetBrains work is scheduled after the Beta gate and is the first item cut if capacity drops.

### Reasons

- One engine removes two sources of drift and the ongoing cost of re-implementing every language change three times (audit estimate: 20 to 32 h/month for both editors as built versus 2 to 3 h/month for a thin VS Code client after a one-time 60 to 100 h server investment).
- VS Code has the largest audience for a TypeScript-centric tool and the cheapest client mechanism.

### Alternatives rejected

- Keep the TypeScript service as the VS Code engine: fast locally, but it is already wrong and will diverge again.
- JetBrains native LSP API as the primary JetBrains path: closed source until 2026.2 and tied to the unified distribution; LSP4IJ is open and already compatible with the 2024.2+ range.

### Consequences

- Chapter 11 sequences the server work (M2) before the VS Code rewrite (M4) and the LSP4IJ step (post-Beta).
- `editors/vscode/out/` is untracked; a lockfile is committed; a CI job compiles the extension.



## ADR-007: Generated output is written through a staged writer with a manifest; never `RemoveAll`

- Status: PROPOSED (accept by 2026-11-01; implement in M1, before any other emitter work)
- Date: 2026-10-21
- Deciders: T2 (owner), T1; reviewer: T3
- Evidence (reproduced by the Node/TS audit track and confirmed by the lead reviewer at the cited lines):
  - `cmd/veld/generate.go:40-52` (`runClean`) calls `os.RemoveAll` on every output directory, removes `.veld-cache.json` and deletes `.veld.lock.json`; it is invoked at `generate.go:758-760` before the contract is parsed.
  - Consequences reproduced: user files inside the output directory are deleted (F-NODE-01); `--out=../proj` deleted the project's own `.veld` sources because `internal/config/config.go:371-379` only inspects the last path segment (F-NODE-02); a contract with a syntax error wipes the previous good output and `veld watch` does so on every save (F-NODE-03); the breaking-change gate and `--strict` never fire because the lock is deleted before the diff runs (F-NODE-04).
  - Emitters call `os.WriteFile` directly; there is no record of which files Veld produced (F-NODE-17).

### Decision

1. Emitters (built-in and external) never write to disk. They return `[]OutputFile{Path, Content, Mode}` to the emission host (chapter 08, section 8.6).
2. The host writes every file to a staging directory beside the output directory (`<out>.veld-staging-<pid>`), then swaps: files whose content is unchanged are left untouched (mtime preserved), changed files are renamed into place one by one, and files listed in the previous manifest but absent from the new set are deleted. Files not in the previous manifest are never deleted; if a generated path collides with an unmanaged file, generation fails with the path named and `--overwrite-unmanaged` is required to proceed.
3. `.veld-manifest.json` in the output directory records `veldVersion`, `langVersion`, `irVersion`, emitter name and version, and for each file its path and SHA-256. `veld clean` deletes exactly the manifest's files and then empty directories; `veld diff` reports files modified by hand since generation.
4. Parsing and validation always complete before any filesystem change. A failed compile leaves the previous output and the lock untouched.
5. Output path safety: `out` is resolved to an absolute path and refused when it equals or contains the config directory, the input file's directory, the current working directory, a filesystem root, or the user's home directory. The resolved absolute paths are printed in `--verbose` and `--dry-run`.
6. `--dry-run` runs the whole pipeline including emitters and prints the would-be file list and a content diff against the manifest, writing nothing.
7. The breaking-change lock is written only after a successful generation and is never deleted by generate or watch; only `veld clean --lock` removes it.

### Reasons

- "Veld never touches developer business logic files" and "Veld never writes outside the --out directory" are headline promises in CLAUDE.md and README; today both are false.
- A manifest is also the mechanism that lets external plugins share the same safety guarantees (ADR-003).

### Alternatives rejected

- Keep `RemoveAll` but warn: still destroys user files on a typo.
- Per-emitter cleanup: duplicates logic in 20 packages and does not cover plugins.

### Consequences

- Every emitter's `Emit` signature changes to return files (or write through a `codegen.Writer` that collects them). This is a mechanical change across `internal/emitter/**` and is scheduled before the IR migration so the two refactors do not collide.
- Estimated cost 20 to 30 h including tests (output-safety tests become part of the conformance suite).



## ADR-008: Contract packages get a manifest and a lockfile; the registry stays data-only at graduation

- Status: PROPOSED (accept by 2026-11-01; implement in M4)
- Date: 2026-10-21
- Deciders: T3 (owner), T1; reviewer: T2
- Evidence: `veld pull` resolves "latest" as `versions[0]` from the server (`cmd/veld/registry.go:200-209`), verifies the SHA-256 only when the server sends an `X-Veld-SHA256` header (`registry.go:232-236`), and records nothing locally; `Pack` includes every `.veld` file under the directory with no manifest (`internal/registry/tarball.go:17-41`). No lockfile for pulled packages exists; `.veld.lock.json` is the breaking-change lock for the project's own contract, a different thing.

### Decision

1. Package manifest `veld.package.json` at the package root, written by the author, validated by `veld push`:
   `{ "manifestVersion": 1, "name": "@org/name", "version": "1.2.0", "description": "...", "entry": "app.veld", "dependencies": { "@org/common": "1.x" }, "license": "MIT" }`.
   Only `.veld` files, the manifest, `README.md` and `LICENSE` are packed. Dotfiles, cache and lock files are excluded.
2. Project lockfile `veld.lock.json` next to `veld.config.json`, written by `veld pull`/`veld install`: for each package, `name`, `version`, `resolved` (registry URL), `integrity` (`sha256-<hex>`), and the resolved dependency tree. `veld install --frozen` fails if the lockfile and config disagree; CI uses it.
3. Version constraints: exact, caret (`^1.2.0`) and `1.x` ranges, semver only. Resolution: highest version satisfying all constraints, single version per package per project (no nested duplicates). Conflicts are errors with both constraint sources named.
4. Integrity: the client always requires a checksum (from the lockfile when present, otherwise from the registry version record), verifies it before extraction, and extracts into a temporary directory that is renamed into place.
5. Cache: `~/.veld/cache/<registry-host>/<org>/<name>/<version>.tgz` plus a content-addressed `sha256/` directory, enabling offline `veld install --offline` from a complete cache.
6. Registry selection and dependency confusion: each `@org` scope is bound to exactly one registry in `veld.config.json` (`"registries": { "@acme": "https://registry.acme.internal" }`); unscoped lookups never fall through to another registry. Credentials are stored per registry URL (as today) and sent only to that URL.
7. Package types: the registry hosts data-only contract packages at graduation. Executable emitter plugins are distributed through npm, Go, or GitHub Releases (ADR-003); a future `kind: "plugin"` package type would require a separate trust policy and is out of scope.

### Reasons

- Reproducibility and integrity are the minimum a registry must provide; today both depend on the server's goodwill.
- The design reuses semver and lockfile conventions developers already know (npm, Go modules) and avoids inventing resolution semantics beyond "highest satisfying, single version".
- Data-only packages keep the registry's threat model manageable for a student team (chapter 13).

### Consequences

- `internal/registry` gains manifest, lockfile, resolver and cache code (chapter 12 estimates).
- Server API adds `GET /packages/{org}/{name}` metadata with per-version `sha256` and dependency lists (chapter 13).
- Transitive dependencies are resolved client-side from version metadata; the server does not run resolution.



## ADR-009: Versioning and compatibility scheme

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T1 (owner), T5 (releases); reviewer: T3
- Evidence: the repository has no CHANGELOG beyond `docs/changelog-v0.1.md`, no tagged release process in CI (chapter 05, F-CLI items), and generated files carry only a fixed header comment.

### Decision

| Artifact | Version form | Compatibility rule | Where declared |
|---|---|---|---|
| Language (syntax and semantics) | `lang 1.N` | Minor adds syntax; major removes or changes meaning. A contract may declare `veld 1` as its first statement (optional; default = compiler's current major). | `internal/spec/VERSION`, conformance fixtures |
| Compiler / CLI | SemVer `0.x.y` until the graduation release `1.0.0` | Pre-1.0: minor may break CLI flags with a deprecation warning one minor earlier. Post-1.0: SemVer. | `veld --version`, Git tag `v1.0.0`, GoReleaser |
| IR | `irVersion "MAJOR.MINOR"` | Minor adds optional fields only; major may remove or rename. Plugins declare a range. | JSON Schema `$id`, `EmitRequest.irVersion` |
| Plugin protocol (VEP) | integer `protocolVersion` | Any change to request/response shape bumps it; host lists supported versions. | `--describe` manifest, `EmitRequest.protocolVersion` |
| Registry API | URL prefix `/api/v1` | Additive changes within v1; breaking changes go to `/api/v2` and v1 is served for one more minor release of the server. | server routes, client `BaseURL + "/api/v1"` |
| Package manifest | `manifestVersion` integer | Server rejects unknown versions with a message naming the minimum client version. | `veld.package.json` |
| Project lockfile | `lockfileVersion` integer | CLI upgrades in place and tells the user. | `veld.lock.json` |
| Generated artifacts | Header `// AUTO-GENERATED BY VELD vX.Y.Z (lang 1.N, ir M.m, emitter <name>@<ver>)` and `.veld-manifest.json` with the same fields plus per-file hashes | Regeneration with a different compiler version rewrites files; the manifest makes drift visible via `veld diff`. | generated output |
| Editor extensions | Their own SemVer; declare the minimum compatible CLI version | Extension refuses to start an older server and shows the install hint. | `package.json` (VS Code), `plugin.xml` (JetBrains) |

Compatibility negotiation: the host (CLI) is the single point that checks plugin protocol and IR ranges (ADR-003); the server checks manifest versions; editor extensions check the CLI version via `veld --version --json`.

Deprecation policy: a language construct or CLI flag is first flagged by `veld lint` (warning) for at least one minor release, then removed in the next major. Deprecated emitters are marked `tier: deprecated` in `veld doctor` output for one minor release before removal.

### Consequences

- Add `CHANGELOG.md` (Keep a Changelog format) maintained per sprint by the integration owner; D1 edits for clarity.
- GoReleaser (chapter 14) stamps the version into the binary with `-ldflags`.
- Tests assert the header format so emitters cannot drift.



## ADR-010: Keep a single repository; one Go module for binaries, npm workspace for TypeScript packages

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T5 (owner), T1; reviewer: T2
- Evidence: the repository already contains the CLI, registry, editors, website and distribution wrappers; the team is six people with no release engineer.

### Decision

- One Git repository (`Veld`). One Go module for `cmd/veld`, `cmd/veld-registry`, `cmd/generate-language` and all `internal/` packages.
- One npm workspace (`package.json` with `workspaces`) for `editors/vscode`, `website`, `sdk/emitter-ts`, and `plugins/hello-emitter`, so that one `npm ci` at the root installs everything CI needs.
- The JetBrains plugin keeps its Gradle build and is built only in a scheduled CI job (chapter 11).
- Independently versioned products: `veld` (CLI), `veld-registry`, `@veld/emitter-sdk`, the VS Code extension, the JetBrains plugin. They share one repository and one changelog with per-product sections; tags are prefixed (`v1.0.0`, `registry/v1.0.0`, `sdk/v0.3.0`, `vscode/v0.5.0`).

### Reasons

- Cross-cutting changes (IR schema, SDK types, docs) land in one pull request and are tested together.
- A six-person student team cannot afford release coordination across repositories.
- Separate deployables (ADR-004) do not require separate repositories.

### Consequences

- CI is split into fast per-path jobs (Go, TypeScript, docs) using path filters, and a scheduled full matrix (chapter 14).
- `CODEOWNERS` maps directories to the owners in chapter 16.



## ADR-012: Extended scope for 1.0.0 under an AI-assisted capacity assumption, with an ordered queue and an automatic cut date

- Status: ACCEPTED by the project owner on 2026-10-21 (team members to confirm in Sprint 0)
- Date: 2026-10-21
- Deciders: project owner; T1 (plan), T2, T3, T4, T5 (owners of the added items); D1 (traceability)
- Supersedes: the "deferred" lists in chapter 03 §3.7, chapter 10 §10.5, chapter 19 §19.11, and the tier-3 removals in ADR-005 for the items named below. Chapter 20 is the authoritative extended plan.

### Context

The baseline plan (chapters 15 to 19) delivered a reduced scope that fit 1,106 committed hours. The project owner decided to include every deferred item: per-field constraints, discriminated unions, authentication schemes in the contract, WebSocket actions, streaming (SSE, chunked download, multipart upload), OpenAPI import, a proper database schema generator, Dockerfile/CI/env generators, a WebAssembly plugin transport (sandboxing), registry-hosted plugins, package signing, high-availability hosting, and the revival of the frozen emitters (Rust, Angular, Dart, Kotlin, Swift) and the mock server. The stated justification is that AI-assisted development will make the team faster, provided code stays lean, follows SOLID principles and is fully tested.

### Decision

1. All items above are in scope for 1.0.0 as an **ordered extended queue** (chapter 20 §20.3) that starts only after the core is in place (M1 to M3: output safety, compile and round-trip tests, IR, diagnostics, plugin protocol freeze, registry hardening).
2. The core end state of chapter 19 remains the graduation deliverable and is never traded for a queue item. Queue items are implemented strictly in order; an item is "in" only when it meets the same bar as tier 1 (compile, round trip, determinism, docs generated from CI).
3. **Automatic cut:** at the extended-feature freeze on 2027-05-02, every queue item not accepted is dropped from 1.0.0 without discussion and documented as future work. The core feature freeze stays at 2027-04-18.
4. Capacity is planned with an AI-assisted multiplier of 1.5 on implementation and test writing (assumed 60 percent of effort) and 1.0 on design, review, evaluation, operations drills and documentation validation, giving a blended 1.3. This is an assumption to be measured at the M1 exit (actual hours against estimates) and corrected.
5. The IR v1 schema reserves the fields the queue needs (`constraints`, `union`, `auth`, `transport`, `stream`, `entity`) from the start so that adding them later does not bump the IR major version.
6. Code quality is a gate, not a goal: `AGENTS.md` at the repository root defines the rules every contributor and AI agent must follow; CI enforces the checkable ones.

### Consequences

- Planned technical work rises from 694 to 1,139 hours to 1,251 to 2,169 hours (chapter 20 §20.2). Under the 1.3 multiplier the midpoint consumes almost the entire reserve; the high estimate is not deliverable. The automatic cut is what keeps the graduation deliverable safe.
- Zero-runtime-dependency exceptions are introduced and declared: WebSocket adapters import a WebSocket library in Node, Python and Go; everything else keeps the rule.
- The hardening window for queue items is two weeks (May 2 to May 16) instead of four; defects in queue items found after May 16 are fixed only if P0.
- The thesis gains research questions on AI-assisted development productivity (measured, not assumed) and on plugin sandboxing.

### What would reverse this decision

Measured productivity at the M1 exit (2026-11-29) below 1.15 times the baseline estimates, or any core gate missed by more than one sprint. In either case the queue is truncated at the point chapter 20 §20.6 specifies, without moving the readiness date.



# Part III: Diagrams (Mermaid sources)

Each diagram's header comment states its purpose, owner and update trigger. Render with any Mermaid-capable tool; labels distinguish current implementation from proposed design.


## D1-system-context.mmd

> D1 - System context and components. Legend: solid = exists at 6a47f81; dashed = PROPOSED.
> Purpose: show which deployables exist, who talks to whom, and what the target adds.
> Owner: D1 (diagram), validated by T1. Update trigger: any ADR that adds/removes a deployable.

```mermaid
%% D1 - System context and components. Legend: solid = exists at 6a47f81; dashed = PROPOSED.
%% Purpose: show which deployables exist, who talks to whom, and what the target adds.
%% Owner: D1 (diagram), validated by T1. Update trigger: any ADR that adds/removes a deployable.
flowchart LR
  subgraph Dev["Developer machine"]
    IDE["VS Code / JetBrains<br/>(editors/vscode, editors/jetbrains)"]
    CLI["veld CLI (Go)<br/>cmd/veld"]
    LSP["veld lsp<br/>(same binary)"]
    GEN["generated/ output"]
    PLUG["External emitter process<br/>(VEP v1)"]:::proposed
    IDE -- "stdio JSON-RPC" --> LSP
    CLI -- "writes" --> GEN
    CLI -. "stdin/stdout JSON" .-> PLUG
  end
  subgraph Reg["Self-hosted registry"]
    SRV["veld serve<br/>internal/server (in CLI binary today)"]
    SRV2["veld-registry binary<br/>cmd/veld-registry"]:::proposed
    PG[("PostgreSQL")]
    FS[("artifact storage<br/>local FS")]
    UI["embedded SPA<br/>handlers/web.go"]
    SRV --> PG
    SRV --> FS
    SRV --> UI
  end
  WEB["website/ (React)<br/>marketing + docs site"]
  CLI -- "push / pull<br/>/api/v1 (HTTP)" --> SRV
  NPM["npm / PyPI / Homebrew / Go / NuGet / Composer<br/>packages/* wrappers"] -- "install binary" --> CLI
  CI["GitHub Actions<br/>.github/workflows"] -- "build/test" --> CLI
  classDef proposed stroke-dasharray: 5 5,stroke:#b58900;

```


## D2-compiler-pipeline.mmd

> D2 - Compiler pipeline and allowed dependency direction (PROPOSED target; current packages named in brackets).
> Purpose: fix the boundary between syntax, semantics, IR and emission so CLI/LSP/plugins share one compiler API.
> Owner: T1 (content), D1 (drawing). Update trigger: any change to internal/compiler, internal/ir or package layout.

```mermaid
%% D2 - Compiler pipeline and allowed dependency direction (PROPOSED target; current packages named in brackets).
%% Purpose: fix the boundary between syntax, semantics, IR and emission so CLI/LSP/plugins share one compiler API.
%% Owner: T1 (content), D1 (drawing). Update trigger: any change to internal/compiler, internal/ir or package layout.
flowchart TB
  SRC[".veld sources + veld.config.json"] --> LEX["syntax: lexer<br/>[internal/lexer]"]
  LEX --> PAR["syntax: parser -> AST with spans<br/>[internal/parser, internal/ast]"]
  PAR --> LOAD["semantic: loader / import graph<br/>[internal/loader]"]
  LOAD --> RES["semantic: resolver + validator<br/>[internal/validator]"]
  RES --> IR["ir: normalized, versioned IR (irVersion 1.0)<br/>[NEW internal/ir]"]
  IR --> APP["compiler API: Compile(ctx, opts) -> (Program, Diagnostics)<br/>[NEW internal/compiler]"]
  APP --> CLI["cmd/veld"]
  APP --> LSPS["internal/lsp"]
  APP --> TESTS["conformance tests<br/>[NEW internal/spec/testdata]"]
  APP --> HOST["emission host: built-ins + plugin runner + output writer/manifest<br/>[internal/emit, NEW internal/emit/plugin]"]
  HOST --> BUILTIN["built-in emitters (tier 1/2)<br/>[internal/emitter/backend/*, frontend/*]"]
  HOST -. "VEP v1 JSON over stdio" .-> EXT["external emitter process"]
  HOST --> TOOLS["tools: openapi, docs, graphql, schema, mock<br/>[internal/generators, internal/*gen]"]
  BUILTIN --> OUT["generated/ + .veld-manifest.json"]
  EXT -. "files in response" .-> HOST
  TOOLS --> OUT
  %% Dependency rule (arrows point from depender to dependency):
  %% emitters/tools -> ir only (never ast/lexer/parser); lsp/cli -> compiler API only; ir -> nothing above it.

```


## D3-plugin-trust.mmd

> D3 - External plugin interaction and trust boundaries (PROPOSED, VEP v1).
> Purpose: show exactly what the host enforces (path containment, timeout, size cap) and what it does not (sandbox).
> Owner: T2 (content), D1 (drawing). Update trigger: any change to the VEP request/response schema or host rules.

```mermaid
%% D3 - External plugin interaction and trust boundaries (PROPOSED, VEP v1).
%% Purpose: show exactly what the host enforces (path containment, timeout, size cap) and what it does not (sandbox).
%% Owner: T2 (content), D1 (drawing). Update trigger: any change to the VEP request/response schema or host rules.
sequenceDiagram
  autonumber
  participant U as User
  participant H as veld CLI (host, trusted)
  participant P as plugin process (trusted by installation, NOT sandboxed)
  participant FS as output directory
  U->>H: veld generate
  H->>H: compile sources -> IR v1 + diagnostics
  H->>P: exec `veld-emitter-x --describe`
  P-->>H: manifest {protocolVersions, irVersions, version, capabilities}
  H->>H: check protocol/IR/version pin; warn on unsupported capabilities
  H->>P: exec `--emit`, write EmitRequest JSON to stdin
  Note over P: may read any file / open network (no sandbox in v1)
  P-->>H: EmitResponse JSON on stdout (files, diagnostics); logs on stderr
  H->>H: enforce timeout (default 120 s), stdout cap (64 MiB)
  H->>H: validate each path: relative, no "..", stays inside outDir
  H->>FS: atomic writes + update .veld-manifest.json
  H-->>U: summary + diagnostics; non-zero exit on error

```


## D4-registry-publish-consume.mmd

> D4 - Registry publication and package consumption. CURRENT flow with PROPOSED hardening marked [P].
> Purpose: show where integrity is (and is not) verified and where atomicity must be added.
> Owner: T3 (content), D1 (drawing). Update trigger: changes to cmd/veld/registry.go, internal/registry, handlers/packages.go.

```mermaid
%% D4 - Registry publication and package consumption. CURRENT flow with PROPOSED hardening marked [P].
%% Purpose: show where integrity is (and is not) verified and where atomicity must be added.
%% Owner: T3 (content), D1 (drawing). Update trigger: changes to cmd/veld/registry.go, internal/registry, handlers/packages.go.
sequenceDiagram
  autonumber
  participant A as Author (veld push)
  participant R as Registry server
  participant DB as PostgreSQL
  participant ST as Artifact storage
  participant C as Consumer (veld pull)
  A->>A: Pack(dir): tar.gz of *.veld + veld.config.json, sha256 (tarball.go:17)
  A->>R: POST /api/v1/packages (multipart manifest + tarball, Bearer token)
  R->>R: auth + org membership check
  R->>ST: store blob [P: write to temp, fsync, rename]
  R->>DB: insert package_version (UNIQUE package_id, version) [P: single transaction, cleanup blob on failure]
  R-->>A: 201 {version, sha256}
  C->>R: GET /packages/{org}/{name}/versions  ("latest" = versions[0], registry.go:200-209)
  C->>R: GET /packages/{org}/{name}/{ver}/download
  R-->>C: tarball + header X-Veld-SHA256
  C->>C: VerifySHA only if header present (registry.go:232) [P: always required; compare to lockfile]
  C->>C: Unpack into veld/packages/@org/name (tarball.go:98) [P: size/entry limits, temp dir + rename]
  C->>C: [P] write veld.lock.json {name, version, sha256, registry}

```


## D5-registry-er.mmd

> D5 - Registry entity-relationship model. CURRENT schema from internal/server/db/db.go:49-120; PROPOSED additions marked (P).
> Purpose: ground the authorization and immutability discussion in the real tables.
> Owner: T3 (content), D1 (drawing). Update trigger: any migration.

```mermaid
%% D5 - Registry entity-relationship model. CURRENT schema from internal/server/db/db.go:49-120; PROPOSED additions marked (P).
%% Purpose: ground the authorization and immutability discussion in the real tables.
%% Owner: T3 (content), D1 (drawing). Update trigger: any migration.
erDiagram
  USERS {
    text id PK
    text email UK
    text username UK
    text password_hash
    bool email_verified
    text totp_secret
  }
  ORGS {
    text id PK
    text name UK
  }
  ORG_MEMBERS {
    text org_id PK, FK
    text user_id PK, FK
    text role
  }
  PACKAGES {
    text id PK
    text org_id FK
    text name
    text visibility
    text created_by FK
  }
  PACKAGE_VERSIONS {
    text id PK
    text package_id FK
    text version
    text sha256
    text storage_key
    bool deprecated
    text published_by FK
  }
  TOKENS {
    text id PK
    text user_id FK
    text org_id FK
    text token_hash UK
    text scopes
    timestamp expires_at
  }
  EMAIL_VERIFICATION_TOKENS {
    text token PK
    text user_id FK
  }
  AUDIT_EVENTS_P {
    text id PK
    text actor_id FK
    text action
    text target
    timestamp at
  }
  USERS ||--o{ ORG_MEMBERS : "member of"
  ORGS ||--o{ ORG_MEMBERS : "has"
  ORGS ||--o{ PACKAGES : "owns"
  PACKAGES ||--o{ PACKAGE_VERSIONS : "has (UNIQUE package_id, version)"
  USERS ||--o{ TOKENS : "issues"
  ORGS o|--o{ TOKENS : "scoped to (optional)"
  USERS ||--o{ EMAIL_VERIFICATION_TOKENS : "verifies"
  USERS ||--o{ AUDIT_EVENTS_P : "(P) acts"

```


## D6-editor-lsp.mmd

> D6 - Editor / LSP / compiler interaction (CURRENT transport; PROPOSED: server consumes the shared compiler API).
> Purpose: show that diagnostics must come from the same compiler the CLI uses, and where unsaved buffers enter.
> Owner: T5 (content), D1 (drawing). Update trigger: changes to internal/lsp or editor clients.

```mermaid
%% D6 - Editor / LSP / compiler interaction (CURRENT transport; PROPOSED: server consumes the shared compiler API).
%% Purpose: show that diagnostics must come from the same compiler the CLI uses, and where unsaved buffers enter.
%% Owner: T5 (content), D1 (drawing). Update trigger: changes to internal/lsp or editor clients.
sequenceDiagram
  autonumber
  participant E as Editor (VS Code client / LSP4IJ)
  participant S as veld lsp (Go, stdio JSON-RPC)
  participant C as compiler API (shared with CLI) [PROPOSED internal/compiler]
  E->>S: initialize {capabilities, positionEncoding?}
  S-->>E: ServerCapabilities
  E->>S: textDocument/didOpen (full text)
  S->>C: Compile(overlay: {uri -> text}, config discovered upward from file dir)
  C-->>S: Program + Diagnostics (byte spans)
  S->>S: convert byte columns -> UTF-16 columns
  S-->>E: textDocument/publishDiagnostics
  E->>S: textDocument/didChange (unsaved edit)
  S->>C: Compile(overlay updated)  -- imports resolved from disk + overlay
  C-->>S: Diagnostics
  S-->>E: publishDiagnostics
  E->>S: completion / hover / definition
  S->>C: query symbol table of last good Program
  C-->>S: symbols
  S-->>E: result

```


## D7-selfhost-backup.mmd

> D7 - Self-hosted deployment (Docker Compose default) and backup/restore flow. PROPOSED.
> Purpose: show the minimum operator topology and the two things that must be backed up together.
> Owner: T3 (content), D1 (drawing). Update trigger: changes to deploy/compose or the backup runbook.

```mermaid
%% D7 - Self-hosted deployment (Docker Compose default) and backup/restore flow. PROPOSED.
%% Purpose: show the minimum operator topology and the two things that must be backed up together.
%% Owner: T3 (content), D1 (drawing). Update trigger: changes to deploy/compose or the backup runbook.
flowchart TB
  subgraph Host["Single host (2 vCPU, 4 GB RAM, 20 GB disk assumed)"]
    RP["reverse proxy (Caddy)<br/>TLS, custom domain"]
    REG["veld-registry container<br/>config: env/registry.config.json<br/>/healthz /readyz"]
    PG[("postgres:16 container<br/>volume: pgdata")]
    VOL[("artifact volume<br/>/data/packages")]
    RP --> REG
    REG --> PG
    REG --> VOL
  end
  OPS["Operator runbook"]
  BK["backup job (cron)<br/>1. pg_dump -Fc<br/>2. tar of /data/packages<br/>3. write backup-manifest.json {schema_version, app_version, sha256s}"]
  OBJ[("off-host backup location")]
  RST["restore procedure<br/>1. deploy same app_version<br/>2. restore DB dump<br/>3. restore artifacts<br/>4. veld-registry verify --storage (sha256 of every package_versions row)"]
  OPS --> BK --> OBJ
  OBJ --> RST --> REG
  UPG["upgrade: backup -> pull image -> migrate (versioned, forward-only) -> readyz -> smoke test pull"]
  OPS --> UPG --> REG

```


## D8-roadmap-critical-path.mmd

> D8 - Roadmap dependencies and critical path, 2026-10-21 to readiness gate 2027-05-31 (PROPOSED).
> Purpose: make the critical path (compiler IR -> plugin host -> SDK -> reference plugin -> evaluation) visible.
> Owner: T1 (content), D1 (drawing). Update trigger: end of every sprint review.

```mermaid
%% D8 - Roadmap dependencies and critical path, 2026-10-21 to readiness gate 2027-05-31 (PROPOSED).
%% Purpose: make the critical path (compiler IR -> plugin host -> SDK -> reference plugin -> evaluation) visible.
%% Owner: T1 (content), D1 (drawing). Update trigger: end of every sprint review.
gantt
  title Veld roadmap to readiness gate (May 31, 2027); June reserved for submission and defense
  dateFormat YYYY-MM-DD
  axisFormat %b %d
  section Gates
  Rewrite/migration decision deadline        :milestone, g0, 2026-11-29, 0d
  Alpha gate (plugin acceptance, registry e2e):milestone, g1, 2027-03-21, 0d
  Beta gate = feature freeze                  :milestone, g2, 2027-04-18, 0d
  Release candidate                           :milestone, g3, 2027-05-16, 0d
  Readiness gate                              :milestone, g4, 2027-05-31, 0d
  section M0 Discovery
  S0 decisions, hygiene, skills survey        :m0, 2026-10-21, 2026-11-01
  section M1 Stabilize + slice
  P0/P1 fixes, conformance skeleton, gen-code CI :crit, m1, 2026-11-02, 2026-11-29
  section M2 Core
  IR v1 + compiler API + manifest/safe writes :crit, m2a, 2026-11-30, 2027-01-10
  Emitter tiering applied, LSP on compiler API:m2b, 2026-11-30, 2027-01-10
  section M3 Exams (low capacity)
  VEP spec frozen, host runner skeleton, docs :crit, m3, 2027-01-11, 2027-02-07
  section M4 Extend + harden
  Plugin SDK + hello + extracted emitter      :crit, m4a, 2027-02-08, 2027-03-21
  Registry split binary, atomic publish, lockfile, authz tests :m4b, 2027-02-08, 2027-03-21
  section M5 Alpha to Beta
  Compose + backup/restore tested, user study, docs site :m5, 2027-03-22, 2027-04-18
  section M6 Hardening + evaluation
  Evaluation runs, RC, report chapters complete :crit, m6, 2027-04-19, 2027-05-16
  section M7 Final
  Final integration, release 1.0, defense rehearsal :crit, m7, 2027-05-17, 2027-05-30

```



# Part IV: Data tables


## Defect register (condensed; full columns in csv/defect-register.csv)


| ID | Priority | Finding | Evidence | User impact | Security | Location | Effort | Owner | Lead verified |
|---|---|---|---|---|---|---|---|---|---|
| F-CLI-01 | P0 | `generate` and `watch` delete previous output and the breaking-change lock before validating; breaking-change gate (--strict/--force/prompt) is dead | reproduced defect | critical | low | cmd/veld/generate.go:40-52, 759, 1021; cmd/veld/watch.go:56 | 6h-10h | T2 |  |
| F-CLI-02 | P0 | `--out`/`out` validation accepts the directory containing the contract sources; `generate` then deletes them | reproduced defect | critical | low | internal/config/config.go:360-377, cmd/veld/generate.go:41-48 | 3h-5h | T2 |  |
| F-CLI-03 | P0 | `veld clean` / pre-generate clean recursively delete any directory `out` points at (e.g. `src`) | reproduced defect | high | low | cmd/veld/generate.go:40-52, 1144-1180 | 4h-6h | T2 |  |
| F-CLI-05 | P0 | `veld init` overwrites existing README.md and veld/*.veld files when no config exists | reproduced defect | high | none | cmd/veld/init.go:20-33, 651-658 | 1h-2h | T5 |  |
| F-CLI-19 | P0 | Go backend (plain `net/http` strategy) generates code that does not compile | reproduced defect | critical | none | internal/emitter/backend/go/strategy/plain.go (route registration) -> generated `internal/routes/todos.go:13-18` | 1h-2h | T4 | reproduced |
| F-CLI-20 | P0 | Python backend routes contain a SyntaxError for actions with input | reproduced defect | critical | none | generated `routes/todos_routes.py:40`, emitter internal/emitter/backend/python | 1h-3h | T4 | reproduced |
| F-FE-01 | P0 | `import @alias/*` and `@alias/**` are lexed as a block comment and silently delete the rest of the file | reproduced defect | high | none | internal/lexer/lexer.go:240-251 and :253-257 (comment detection precedes the path-token branch at :315); parser consumer internal/parser/parser.go:210-215 | 2h-4h | T1 | reproduced |
| F-FE-02 | P0 | `veld generate` deletes the lock file and output before the breaking-change gate, so breaking-change detection never fires (and an invalid contract wipes the previous output) | reproduced defect | critical | none | cmd/veld/generate.go:758-760 (`runClean`), gate at :1019-1035, `computePreChanges` :1126-1140, `runClean` removing the lock at :51; same ordering in cmd/veld/watch.go:56 | 3h-6h | T1 |  |
| F-LEAD-01 | P0 | Generated output is not deterministic: types/index.ts export order varies between runs | reproduced defect | high | none | internal/emitter/tsshared/types.go:442 (range over map `claimed` builds moduleExports unsorted) | 1h-2h | T2 | reproduced |
| F-MULTI-01 | P0 | Hyphenated header names are emitted as identifiers in every language, producing uncompilable output | reproduced defect | critical | none | internal/parser/parser.go:961 (synthetic `<Action>Headers` model); consumers: python models/items.py:218, go internal/models/items.go:184, java CreateItemHeaders.java:8, csharp CreateItemHeaders.cs, php CreateItemHeaders.php:12, rust models/items.rs:211, node-js types/items.js:141, dart/kotlin/swift clients, graphql, prisma, sql | 6h-12h | T4 |  |
| F-MULTI-02 | P0 | Emitter unit tests assert substrings; generated output is never compiled, so most non-Node backends ship uncompilable code with a green suite | reproduced defect | high | none | internal/emitter/backend/*/*_test.go (e.g. rust/rust_test.go:160 asserts `pub fn router<S>() -> impl std::any::Any`) | 24h-40h | T4 |  |
| F-MULTI-03 | P0 | Default Python backend (no --validate) emits a syntax-error route file and no route registration | reproduced defect | critical | none | internal/emitter/backend/python/routes.go:139 (and strategy), registration branch in routes.go | 8h-14h | T4 | reproduced |
| F-MULTI-06 | P0 | Go routes do not compile (`mux` undefined) and run without input validation or query/header binding | reproduced defect | critical | low | internal/emitter/backend/go/strategy/plain.go:21,52; go/routes.go handler generation | 12h-20h | T4 | reproduced |
| F-NODE-01 | P0 | `veld generate` deletes the whole output directory (including user files) before every run | reproduced defect | high | low | cmd/veld/generate.go:40-50 (runClean), 757-760 (call); comment at 141-142 claims the opposite | 6h-10h | T2 | source |
| F-NODE-02 | P0 | `--out` pointing at the project/source directory deletes the contract and sources | reproduced defect | high | low | internal/config/config.go:375 (only rejects "..", ".", "./", "../."), cmd/veld/generate.go:40 | 2h-3h | T2 | source |
| F-NODE-03 | P0 | A failing generate leaves the previous output deleted; watch does this on every save | reproduced defect | high | none | cmd/veld/generate.go:757-760 (clean before parse), cmd/veld/watch.go:270 | 4h-8h (shared with F-NODE-01) | T2 |  |
| F-NODE-04 | P0 | Breaking-change gate and `--strict` never fire (lock deleted before the diff) | reproduced defect | high | none | cmd/veld/generate.go:50 (diff.DeleteLock inside runClean) vs :205 (hasLock) | 1h-2h | T2 | source |
| F-NODE-05 | P0 | Static routes shadowed by earlier parametric routes (including middleware-protected ones) | reproduced defect | high | high | internal/emitter/backend/node/routes.go:75-87 (actions emitted in declaration order) | 3h-5h | T2 | source |
| F-NODE-06 | P0 | Generated output does not compile under `tsc --strict` | reproduced defect | high | none | frontend/typescript/client.go (`query as Record<string, unknown>`, `extraHeaders: Record<string,string>`), backend/node/routes.go (headers object literal) | 1h-2h (casts) or 6h-10h (interface migration) | T2 |  |
| F-REG-01 | P0 | 2FA bypass - the pre-TOTP "partial" JWT is a fully valid session token | reproduced defect | high | high | internal/server/handlers/auth.go:105-117 (partial issued with `IssueJWT`), internal/server/auth/token.go:56-92 (no purpose claim), internal/server/auth/middleware.go:70-100 (accepts any valid JWT as a session) | 1h-2h | T3 | source |
| F-REG-02 | P0 | Storage path traversal on publish - arbitrary `*.tar.gz` write, overwrite and delete | reproduced defect | high | high | internal/server/handlers/packages.go:122,177 (unvalidated `manifest.name`/`version` into the key); internal/server/storage/storage.go:33-36 (`Clean` then `Join` with no containment check); packages.go:217 and :334 (Delete of the same key) | 3h-6h | T3 | source |
| F-REG-03 | P0 | Token scope model is not enforced; a read-only token can mint admin tokens and delete versions | reproduced defect | high | high | internal/server/handlers/auth.go:393-439 (CreateToken: no scope check, scopes/org_id accepted verbatim); handlers/packages.go:276-338 (Deprecate/Delete never call `HasScope`); only packages.go:105 checks `write`; middleware.go:57-68 (`HasScope` returns true for JWT); `read` scope never checked; `org_id` of a token never enforced | 4h-8h | T3 | source |
| F-REG-09 | P0 | Client integrity check is optional and self-referential; http registries are accepted without warning | reproduced defect | high | high | internal/registry/client.go:208-210 (sha taken from the same response; copy error ignored), cmd/veld/registry.go:232-236 (`if remoteSHA != ""`), 37-60 (no scheme check on login), credentials.go:230 (token sent over whatever scheme) | 6h-16h (signatures larger) | T3 | source |
| F-REG-10 | P0 | Unpack has no limits, writes in place, and drops I/O errors | reproduced defect | medium | medium | internal/registry/tarball.go:98-143 (see section 1 items 3-5), client.go:209 | 4h-8h | T3 | source |
| F-REG-11 | P0 | Stored XSS sinks in the web UI (usernames, org text, versions, deprecation text) | source-supported defect (hostile values stored and returned by the API - reproduced; rendering in a browser not executed) | high | high | internal/server/web/index.html:335 (`${u.username}`), 377 (`${e.message}`), 420, 456-457, 486-492, 519-524 (`v.version`, `v.deprecated`), 543-544, 564-565, 585-587 (`m.username`, `m.email`, inline `onclick="removeMember('${org.name}','${m.username}')"`), 616, 633-634; only `escHtml` uses are lines 497, 503; no CSP. | 6h-12h | T3 |  |
| D-01 | P1 | Documentation mismatch: Node backend generates Zod schemas (`schemas/schemas.ts`) and validates input with Zod, ZodError->400 | documentation mismatch | medium | none | CL:9,81,444-446,467-468; README.md:32,272,292; OV Node row; changelog-v0.1 items 1,6; GS:63-68; T:849,928-950; GP:694 (demo); website DocsPage.tsx:1608,1758 | 0.5h-2h | D1 | source |
| D-02 | P1 | Documentation mismatch: Output is deterministic (non-negotiable / evaluation criterion) | documentation mismatch | medium | none | CL:508; README.md:419; GP 14.3 | 0.5h-2h | D1 | reproduced |
| D-03 | P1 | Documentation mismatch: `veld login/logout/push/pull/serve/doctor` are top-level commands | documentation mismatch | medium | none | CL:52-58,392-399; GP 14.1; DG:381 | 0.5h-2h | D1 | source |
| D-10 | P1 | Documentation mismatch: MIT license | documentation mismatch | medium | none | README.md:538-540 | 0.5h-2h | D1 |  |
| F-CLI-07 | P1 | Config precedence: flat fields override nested, contrary to documentation | reproduced defect | medium | none | internal/config/config.go:137-150 (normalize), CLAUDE.md config section, config.go:34 comment | 1h-2h | T5 |  |
| F-CLI-08 | P1 | `--out` does not override `backendConfig.out` / `backendOut` | reproduced defect | medium | none | internal/config/config.go:312-335, 392-405 | 1h-2h | T5 |  |
| F-CLI-15 | P1 | Release workflow: Homebrew job never publishes; package jobs skipped when only wrappers change | source-supported defect | high | none | .github/workflows/release.yml:133-170 (resolve), 392-422 (update-homebrew), publish-* `needs: [release, resolve]` | 4h-8h | T5 |  |
| F-CLI-16 | P1 | npm and pip wrappers: no checksum, unpinned download, swallowed postinstall failure, Python 3.8/3.9 syntax, doc name mismatch | reproduced defect | medium | medium | packages/npm/install.js:25-45, 141-205; packages/pip/veld/__main__.py:23,61 (`str \| None`); packages/pip/pyproject.toml (requires-python >=3.8); packages/README.md | 4h-6h | T5 |  |
| F-CLI-18 | P1 | CI verifies only Linux unit tests; no signing/SBOM; no generated-code compilation; nothing builds website/editors/wrappers | design limitation | high | low | .github/workflows/ci.yml, release.yml | 12h-20h | T5 |  |
| F-CLI-23 | P1 | Test hygiene gaps (untested registry server, strategy subpackages, no generated-code compile tests, CLI coverage reads 0%) | reproduced defect | medium | medium | internal/server/**, internal/registry, internal/docsgen, internal/graphqlgen, internal/language, emitter/backend/*/strategy, cmd/veld | 16h-30h | T4 |  |
| F-FE-03 | P1 | `veld fmt --write` corrupts valid files (drops comments, string quotes, spacing) and panics on `model }` | reproduced defect | high | none | internal/format/format.go:27-33 (formats from a token stream), :187-189 (default branch writes `tok.Value` of TString without quotes), :153 (`writeIndent(baseIndent+depth)` with negative depth), :113-131 | 12h-20h | T1 |  |
| F-FE-05 | P1 | Diagnostics are first-error-only for lexer/parser, line-only, and sometimes point at the wrong line | reproduced defect | medium | none | internal/lexer/lexer.go:363 (returns `errors[0]`), internal/loader/loader.go:94-101, internal/parser/parser.go (every `return ..., err`), internal/lexer/lexer.go:195-199 (`Token{Type,Value,Line}`) | 10h-16h | T1 |  |
| F-FE-18 | P1 | Breaking-change classifier misses enums, nested models, middleware, errors and unions; status change is non-breaking | reproduced defect | high | none | internal/diff/diff.go:97-191 (module/action only), :195-217 (diff only the model directly named by an action slot), :377-382 (`typeChanged` ignores `UnionTypes`), :182-188 (status -> `Added`); internal/diff/lock.go:36-40 (lock stores Modules and Models only, no Enums/Constants/Prefix) | 8h-14h | T1 |  |
| F-FE-22 | P1 | "Single source of truth" language spec is not what the compiler uses; a dead second parser exists | documentation mismatch | low | none | internal/language/constants.go:29-110 (spec), internal/lexer/lexer.go:374-451 (hard-coded keyword table), internal/language/veld.go:82-200 (regex mini-parser, 0% coverage, unused) | 6h-10h | T5 |  |
| F-LSP-01 | P1 | LSP ignores imports, so valid multi-file projects get false errors and no cross-file features | reproduced defect | high | none | internal/lsp/handler.go:261-273, 324-365; internal/lsp/server.go (no loader use) | 16h–30h | T5 |  |
| F-LSP-11 | P1 | VS Code extension re-implements the language service in TypeScript and does not use `veld lsp` | source-supported defect | high | none | editors/vscode/src/extension.ts:81-1675; editors/vscode/package.json | 20h–40h (after F-LSP-01 to 10) | T5 |  |
| F-LSP-16 | P1 | `internal/language` is not consumed by the compiler, so "single source of truth" is false and CI cannot detect parser/spec drift | source-supported defect | medium | none | internal/language/*.go (imported only by cmd/generate-language/main.go); .github/workflows/ci.yml:38-67 | 4h–8h | T5 |  |
| F-MULTI-04 | P1 | Python validators miss the `Decimal` import; any contract with a decimal field returns 500 on every request | reproduced defect | high | none | internal/emitter/backend/python/validate.go:348 (emits isinstance(..., (str, Decimal)) with no import); output _validators.py:8 | 1h-2h | T4 |  |
| F-MULTI-05 | P1 | Python service contract is internally inconsistent (ABC returns model classes, routes require dicts; headers not extracted; query untyped; malformed JSON -> 500; enum unchecked; exception text leaked) | reproduced defect | high | low | python/routes.go:145-150 and the catch-all except block, python/types.go, validate.go | 10h-18h | T4 |  |
| F-MULTI-07 | P1 | Rust backend cannot compile and has no working router or main | source-supported defect | critical | none | internal/emitter/backend/rust/routes.go:35,100; Cargo.toml template in rust/main.go; output src/items/mod.rs, src/models/items.rs | 30h-50h to make real | T4 |  |
| F-MULTI-08 | P1 | Java, C#, PHP backends emit framework-less classes that do not compile and do not route | reproduced defect | high | none | java/types.go (enum default; package const at line 26), java/routes.go (deleteItem), csharp/types.go, csharp/routes.go, php/routes.go, php/types.go | 20h-35h each to reach a working state | T4 |  |
| F-MULTI-09 | P1 | Dart/Kotlin/Swift clients have compile or runtime defects beyond the header bug | source-supported defect | high | none | frontend/swift/client.go (`${id}` interpolation), frontend/kotlin/helpers.go:20 + client.go:107, frontend/dart/types.go (fromJson) | 10h-16h per SDK | T4 |  |
| F-MULTI-10 | P1 | Service SDKs: Python SDK rejects its own model types; TS SDK fails strict tsc; Go SDK lacks query support | reproduced defect | medium | none | python/sdk.go (body/query serialisation), node/sdk.go (shared index scalar imports), go/sdk.go | 8h-14h | T4 |  |
| F-MULTI-11 | P1 | Dockerfile and CI generators download a release asset that returns 404, and bundle an unrelated registry Postgres | reproduced defect | high | medium | internal/generators/dockerfile/dockerfile.go (download line, compose template), internal/generators/cicd/cicd.go | 4h-8h | T4 |  |
| F-NODE-08 | P1 | No input validation unless `--validate`; documented Zod/schemas do not exist | documentation mismatch | medium | medium | backend/node/routes.go:105-110, CLAUDE.md "Route handler features"/"Generated Output Structure" | 2h-4h | T2 |  |
| F-NODE-09 | P1 | `--validate` validators are shallow | reproduced defect | medium | medium | backend/node/validate.go:110-300 | 8h-14h | T2 |  |
| F-NODE-10 | P1 | Error handler leaks internal messages on 500 and mixes error formats | reproduced defect | medium | medium | backend/node/routes.go:185-191 | 2h-3h | T2 |  |
| F-NODE-11 | P1 | Path parameters are not URL-encoded in the SDK | reproduced defect | medium | medium | internal/emitter/helpers.go:25 (ToTemplateLiteral), frontend/typescript/client.go (urlExpr) | 1h-2h | T2 |  |
| F-NODE-15 | P1 | Angular emitter output is invalid and incomplete | reproduced defect | high | none | frontend/angular/emitter.go (models ~L110, service generation) | 6h-10h | T2 |  |
| F-NODE-17 | P1 | Writes are non-atomic, no manifest, stale files handled by "delete everything" | source-supported defect | medium | low | emitters call os.WriteFile directly (backend/node/routes.go, frontend/typescript/emitter.go:65); no manifest anywhere | 6h-10h (overlaps F-NODE-01/03) | T2 |  |
| F-NODE-19 | P1 | Tests never compile or run the generated TypeScript | source-supported defect | medium | none | internal/emitter/**/*_test.go | 6h-10h | T2 |  |
| F-REG-04 | P1 | Org admins can demote or remove the sole owner (org takeover) | reproduced defect | high | medium | internal/server/handlers/orgs.go:108-155 (AddMember allows an admin to set another member's role; db.go:296-303 `ON CONFLICT DO UPDATE SET role`), orgs.go:157-193 (last-owner guard only inside `if caller.Role == "owner"`, line 180) | 2h-3h | T3 |  |
| F-REG-05 | P1 | API token `expires_at` is never enforced | reproduced defect | medium | medium | internal/server/db/db.go:535-557 (GetTokenByHash returns expired rows), middleware.go:89-98 (no check) | 0.5h-1h | T3 |  |
| F-REG-06 | P1 | Private-package visibility is inconsistent across endpoints (latent leak) and unreachable from the API | reproduced defect (with `visibility` set by SQL; the API cannot set it) | medium | medium | packages.go:25-43 (any authed user gets `public`+`private`), 80-96 (ListVersions no check), orgs.go:80-103 + db.go:428 (all packages of an org listed to anonymous), packages.go:57,242 (membership of any role suffices) | 4h-8h | T3 |  |
| F-REG-07 | P1 | Concurrent/duplicate publish deletes the published tarball (DB row left pointing at nothing) | reproduced defect | high | low | packages.go:162-166 (check-then-act), 177-200 (Put renames over the existing key), 216-218 (on unique-violation: `Storage.Delete(tarballKey)` removes the winner's file) | 3h-5h | T3 |  |
| F-REG-08 | P1 | Versions are not immutable and "latest" is not the highest semver | reproduced defect | medium | medium | packages.go:311-338 (hard delete), 162 (only existing rows block); db.go:~470 (`ORDER BY published_at DESC`), cmd/veld/registry.go:199-209 (`versions[0]`), models.go:ParseSemver (unused by the CLI, tolerant parsing) | 4h-8h | T3 |  |
| F-REG-12 | P1 | No rate limiting, lockout, or TOTP replay protection | reproduced defect | medium | medium | server.go:59 (no limiter in the handler chain); auth.go:88-159; totp.go:28-38 | 4h-6h | T3 |  |
| F-REG-13 | P1 | No body-size limits and no server-side archive validation on publish | reproduced defect | medium | medium | packages.go:111 (`ParseMultipartForm(32<<20)` is only the in-memory threshold, the comment says "32 MB max"), 169-200 (streams any bytes to disk, no size cap, no gzip/tar check); auth.go:39 etc. (`json.NewDecoder(r.Body)` unbounded); server.go:59-62 | 3h-5h | T3 |  |
| F-REG-14 | P1 | Sessions cannot be revoked; no purpose/typ on JWTs; 7-day lifetime | reproduced defect | medium | medium | auth.go:162-171 (Logout only clears cookie), token.go:56-92, middleware.go:70-100 | 4h-8h | T3 |  |
| F-REG-15 | P1 | Transport/browser hardening absent (no TLS, cookie not Secure, CORS `*`, no security headers) | reproduced defect | medium | medium | server.go:56-66 (`ListenAndServe`, no TLS option), auth.go:372-381, server.go:150-163 | 2h-4h | T3 |  |
| F-REG-16 | P1 | Registry selection is nondeterministic and credentials are keyed by raw string (dependency-confusion exposure) | reproduced defect | medium | medium | internal/registry/credentials.go:259-268 (`DefaultRegistry` returns the first key of a Go map), :230-236 (exact-string key), client.go:31-43; used by `pull` (registry.go:187-196), `versions`, `token create` | 3h-6h | T3 | source |
| F-REG-20 | P1 | Pull writes untrusted files into the project and can overwrite existing files; pulled `veld.config.json` can carry a hook | suspected risk (the write was reproduced via MITM; the hook executing was not run) | medium | medium | cmd/veld/registry.go:211-216,238 (default out `veld/packages/@org/name`, `--out` unrestricted, `os.Create` overwrite); `hooks.postGenerate` in config (CLAUDE.md config section) | 2h-3h | T3 |  |
| F-REG-21 | P1 | Operational gaps: no health endpoint, SIGTERM unhandled, no schema versioning, no TLS, no backups, zero tests | reproduced defect (health, migrations table, restart), source-supported (signals) | medium | low | server.go:84-140 (catch-all route serves SPA for `/healthz`: REPRODUCED 200 text/html 84 KB), serve.go:138 (`os.Interrupt` only), db.go:43-128 (no `schema_migrations`; REPRODUCED table list), storage.go (local FS only; DB row and file are not reconciled), `find internal/server internal/registry -name '*_test.go'` -> none, CLAUDE.md "Registry Server" section (no backup/restore, no HA) | 6h-12h | T3 |  |
| D-04 | P2 | Documentation mismatch: `tools` config block toggles auxiliary generators | documentation mismatch | medium | none | CL:199-202,238 | 0.5h-2h | D1 |  |
| D-05 | P2 | Documentation mismatch: GitLab CI generated | documentation mismatch | medium | none | README.md:35,233; website SupportedStacks.tsx:33 | 0.5h-2h | D1 |  |
| D-06 | P2 | Documentation mismatch: Rust = Actix; PHP = PSR-15 | documentation mismatch | medium | none | README.md:221,224; OV; website Features.tsx:26 | 0.5h-2h | D1 |  |
| D-07 | P2 | Documentation mismatch: Module path `github.com/veld-dev/veld`; "only dependency cobra"; SQLite dep | documentation mismatch | medium | none | OV "Go Module"; DG:87-90 | 0.5h-2h | D1 |  |
| D-08 | P2 | Documentation mismatch: Python output has `types/` + `schemas/schemas.py` | documentation mismatch | medium | none | CL:456-460; T:854; changelog | 0.5h-2h | D1 |  |
| D-09 | P2 | Documentation mismatch: VS Code palette commands work | documentation mismatch | medium | none | vscode README:43-46,116-120 | 0.5h-2h | D1 |  |
| D-11 | P2 | Documentation mismatch: Install names | documentation mismatch | medium | none | README.md:46-53 vs packages/README.md:11-12 (`npm install veld`, `pip install veld`), composer README:56 (`brew install veld`), vscode README:66 (`adhamzineldin/tap/veld`); npm/pip README `@0.2.0` vs manifest 0.1.0 | 0.5h-2h | D1 |  |
| D-12 | P2 | Documentation mismatch: Getting-started snippets | documentation mismatch | medium | none | GS:61-95 | 0.5h-2h | D1 |  |
| F-CLI-10 | P2 | `hooks.postGenerate` is run via hard-coded `sh -c`; failures are ignored | design limitation | medium | low | cmd/veld/generate.go:24-36 | 2h-3h | T5 |  |
| F-CLI-17 | P2 | Distribution metadata gaps (no root LICENSE, placeholders, Windows-arm64, Composer unpublishable as-is) | source-supported defect | medium | none | packages/chocolatey/veld.nuspec (licenseUrl), packages/chocolatey/tools/chocolateyInstall.ps1 (checksum placeholder), packages/composer/*, packages/apt/nfpm.yaml | 6h-10h | T5 |  |
| F-FE-04 | P2 | Unterminated string and unterminated block comment are accepted silently | reproduced defect | medium | none | internal/lexer/lexer.go:259-270 (string stops at newline, no error), :240-251 (comment loop ends at EOF, no error) | 1h-2h | T1 |  |
| F-FE-07 | P2 | Primitive-type and verb tokens cannot be used as field names, enum values, action names or model names | reproduced defect | medium | none | internal/parser/parser.go:129-136 (`expectFieldName` allows only TIdent + `lexer.IsKeyword`), :328 (enum values require TIdent), :441, :747; internal/lexer/lexer.go:203-210 (`IsKeyword` omits TType* tokens) | 2h-4h | T1 |  |
| F-FE-08 | P2 | No nested generics; `Map<string,int>[]` is represented ambiguously | design limitation | medium | none | internal/parser/parser.go:514-556, internal/ast/ast.go:50-51 | 12h-24h (touches every emitter) | T1 |  |
| F-FE-09 | P2 | Path parameters, path shape and routes are barely validated | reproduced defect | medium | none | internal/validator/validator.go:298-322 (route conflicts), :525-535 (`normalizeRoutePath`); internal/parser/parser.go:149 (`{id}` rewrite) | 4h-6h | T1 |  |
| F-FE-13 | P2 | Loader silently ignores missing import targets and falls back to loading whole directories | reproduced defect | medium | none | internal/loader/loader.go:246-249 (missing dir ignored), :285-355 (not-found file falls back to wildcard and `strings.Contains("model "+name)` text scan), :255/:273/:341 (self-edge in `FileImports`) | 3h-5h | T1 |  |
| F-FE-14 | P2 | `veld validate` and `veld ast` ignore config `aliases` | reproduced defect | medium | none | cmd/veld/contract.go:39 and :70 (`loader.Parse(path)` with no aliases) vs `veld lint` at :101 and `generate` (`loader.Parse(rc.Input, rc.Aliases)`) | 1h-2h | T1 |  |
| F-FE-15 | P2 | UTF-8 BOM (and UTF-16) files are rejected with a confusing message | reproduced defect | medium | none | internal/loader/loader.go:90-94, internal/lexer/lexer.go:355-358 | 0.5h-1h | T1 |  |
| F-FE-17 | P2 | Synthetic models from inline `input/output/query/headers` collide and can contain invalid UTF-8 | reproduced defect | medium | none | internal/parser/parser.go:815, :851, :925, :961 (`strings.ToUpper(act.Name[:1]) + act.Name[1:]` byte slicing; the other three do not capitalise at all, so names are inconsistent) | 2h-4h | T1 |  |
| F-FE-28 | P2 | Positions are lines only; no column/offset, so no precise range diagnostics for editors | design limitation | medium | none | internal/lexer/lexer.go:195-199; internal/ast/ast.go (`Line int` fields only) | included in F-FE-05 (4h-6h extra for AST nodes) | T1 |  |
| F-LSP-02 | P2 | Unknown requests get no response; malformed JSON gets no ParseError | reproduced defect | medium | none | internal/lsp/server.go:44-46, 104-137 | 1h–2h | T5 |  |
| F-LSP-03 | P2 | Position handling uses byte offsets with no positionEncoding negotiation | reproduced defect | medium | none | internal/lsp/completion.go:17-18; internal/lsp/hover.go:130-153; internal/lsp/definition.go:112,125,138 | 3h–6h | T5 |  |
| F-LSP-04 | P2 | Diagnostics are first-error-only, whole-line, and often on the wrong line | reproduced defect | medium | none | internal/lsp/handler.go:329-395 | 8h–16h | T5 |  |
| F-LSP-05 | P2 | Lexer silently accepts unterminated string literals (CLI too) | reproduced defect | medium | none | internal/lexer/lexer.go:258-270 | 1h–2h | T5 |  |
| F-LSP-07 | P2 | JSON-RPC conformance defects (shutdown result, null diagnostics, exit code) | reproduced defect | medium | none | internal/lsp/protocol.go:410-415 (`Result ... omitempty`), :511 (`Diagnostics`); internal/lsp/server.go:113-117; handler.go:284 (nil slice) | 2h–3h | T5 |  |
| F-LSP-08 | P2 | Single-threaded, un-debounced, no recover — slow parses block everything and a panic kills the server | suspected risk | medium | low | internal/lsp/server.go:34-49; internal/lsp/handler.go (no recover anywhere in internal/lsp) | 6h–12h | T5 |  |
| F-LSP-09 | P2 | Feature surface far below the documented editor support | documentation mismatch | medium | none | internal/lsp/handler.go:234-243; README.md:429 | 24h–40h | T5 |  |
| F-LSP-10 | P2 | Completion is context-blind and uses hard-coded lists | reproduced defect | medium | none | internal/lsp/completion.go:24-75 | 6h–10h | T5 |  |
| F-LSP-12 | P2 | VS Code contributes commands and settings that do nothing | source-supported defect | medium | none | editors/vscode/package.json (commands veld.validate/generate/generateDryRun, settings veld.executablePath/validateOnSave); editors/vscode/src/extension.ts (no registerCommand, no getConfiguration) | 3h–6h | T5 |  |
| F-LSP-13 | P2 | VS Code (and JetBrains) validators reject valid action directives | source-supported defect | medium | none | editors/vscode/src/extension.ts:~497-506 (`ACTION_KEYS`: method, path, input, output, query, stream, middleware, errors, description); editors/jetbrains/.../VeldAnnotator.kt:71-72 (adds `response`, still no `headers`/`emit`) | 1h–2h | T5 |  |
| F-LSP-15 | P2 | JetBrains plugin is a second hand-written language implementation with PATH-based CLI validation | source-supported defect | medium | low | editors/jetbrains/src/main/kotlin/dev/veld/jetbrains/VeldLexer.kt:272, VeldParser.kt, VeldAnnotator.kt:71, VeldExternalAnnotator.kt:25-27 | 20h–40h (for LSP4IJ migration) | T5 |  |
| F-MULTI-12 | P2 | SQL/Prisma/GraphQL generators emit invalid output for common contracts | reproduced defect | medium | none | internal/schema/ (BuildSQL, BuildPrisma), internal/graphqlgen/ | 10h-16h | T4 |  |
| F-NODE-07 | P2 | Models are classes with getters/setters and branded scalars | source-supported defect | medium | none | internal/emitter/backend/node/types.go, tshelpers/tshelpers.go, types/_scalars.ts | 6h-10h | T2 |  |
| F-NODE-12 | P2 | Query serialization is lossy | reproduced defect | medium | none | frontend/typescript/helpers.go (buildQueryString) | 3h-5h | T2 |  |
| F-NODE-13 | P2 | SDK lacks timeout/abort, fetch injection and dynamic auth hook | design limitation | medium | low | frontend/typescript/client.go:35-45, helpers.go (VeldClientConfig) | 4h-6h | T2 |  |
| F-NODE-14 | P2 | Module named `index` clobbers types barrel; JS-global model names break output | reproduced defect | medium | none | backend/node/types.go / barrel.go (file naming); no reserved-name check in the validator | 2h-4h | T2 |  |
| F-NODE-16 | P2 | `--out`/`--input` resolution is inconsistent and can write outside the project | reproduced defect | medium | low | internal/config/config.go (BuildResolved), cmd/veld/generate.go | 3h-5h | T2 |  |
| D-13 | P3 | Documentation mismatch: "Zero runtime deps / no lock-in" universal | documentation mismatch | low | none | README.md:37,417; CL:9 | 0.5h-2h | D1 |  |
| D-14 | P3 | Documentation mismatch: Never writes outside `--out` | documentation mismatch | low | none | CL; OV; T:840 | 0.5h-2h | D1 |  |
| D-15 | P3 | Documentation mismatch: Overview lists 1 frontend, 7 backends (no node-js) | documentation mismatch | low | none | OV | 0.5h-2h | D1 |  |
| D-16 | P3 | Documentation mismatch: Hosted-docs scrape `test.md` stale: 7 backends/4 frontends, react = typescript alias, `-v` = version, 11 commands | documentation mismatch | low | none | T:17,38-53,603-617,1210-1235 | 0.5h-2h | D1 |  |
| D-17 | P3 | Documentation mismatch: Developer guide says main.go is ~4,600 lines, `internal/openapigen` exists, nine examples, sqlite | documentation mismatch | low | none | DG:30,47,56,87,373 | 0.5h-2h | D1 |  |
| D-18 | P3 | Documentation mismatch: Roadmap/service-sdk-plan show done work as TODO; cite nonexistent files | documentation mismatch | low | none | roadmap.md:33-36,470,512-516; service-sdk-plan.md:3,665-668 | 0.5h-2h | D1 |  |
| D-19 | P3 | Documentation mismatch: SDK class `IAMClient` | documentation mismatch | low | none | CL:315 | 0.5h-2h | D1 |  |
| D-20 | P3 | Documentation mismatch: PLUGINS_CI_CD_SUMMARY: publish-plugins.yml, publisher veld-dev | documentation mismatch | low | none | that file | 0.5h-2h | D1 |  |
| D-21 | P3 | Documentation mismatch: "261 source files" | documentation mismatch | low | none | GP:34,284 | 0.5h-2h | D1 |  |
| D-22 | P3 | Documentation mismatch: "~14 languages/frameworks" | documentation mismatch | low | none | PP:5 | 0.5h-2h | D1 |  |
| D-23 | P3 | Documentation mismatch: Dev span ended 19 Apr 2026, 188 commits | documentation mismatch | low | none | GP:664 | 0.5h-2h | D1 |  |
| D-24 | P3 | Documentation mismatch: Types/HTTP-method lists incomplete | documentation mismatch | low | none | README.md:183-194; CL:390 | 0.5h-2h | D1 |  |
| D-25 | P3 | Documentation mismatch: Website team list | documentation mismatch | low | none | Contributors.tsx | 0.5h-2h | D1 |  |
| D-26 | P3 | Documentation mismatch: EXEC v4 snapshot numbers (server 1,699 lines; main.go 4,610; .vsix tracked) | documentation mismatch | low | none | EXEC v4 4.2,7.1,8.1,8.3 | 0.5h-2h | D1 |  |
| F-CLI-04 | P3 | `veld doctor` is advertised but not registered | reproduced defect | low | none | cmd/veld/tooling.go:195, cmd/veld/main.go:88-101, 77 | 0.5h-1h | T5 | source |
| F-CLI-06 | P3 | `generate --dry-run` writes `.veld-cache.json` | reproduced defect | low | none | cmd/veld/generate.go:190-200 | 0.5h-1h | T2 |  |
| F-CLI-09 | P3 | Path-base inconsistency and silent unknown config keys | reproduced defect | low | none | internal/config/config.go:283-308, 324-327; RawConfig decoding at :296 | 3h-5h | T5 |  |
| F-CLI-11 | P3 | Hidden commands and lack of machine-readable output / exit-code taxonomy | design limitation | low | none | cmd/veld/main.go:105, contract.go:64,601 | 6h-10h | T5 |  |
| F-CLI-12 | P3 | Error-level lint rules are unreachable | reproduced defect | low | none | cmd/veld/contract.go:96-103, internal/lint/lint.go | 0.5h-1h | T5 |  |
| F-CLI-13 | P3 | Version reporting wrong for `go install` and source builds; wrappers pinned to 0.1.0 | reproduced defect | low | none | cmd/veld/main.go:37, packages/*/ (version 0.1.0), release.yml:304 | 1h-2h | T5 |  |
| F-CLI-14 | P3 | Documentation lists top-level `veld login\|logout\|push\|pull\|serve`; they exist only under `veld registry` | documentation mismatch | low | none | cmd/veld/registry.go:373, CLAUDE.md "Build & Test Commands", README | 1h-2h | T5 |  |
| F-CLI-21 | P3 | Repository hygiene: 20MB binary and scratch dirs tracked; stale duplicate registry UI | reproduced defect | low | none | `veld` (root), test-gen/, test-gen2/, test-headers/, test-constants/, test.md, .idea/, docs/*.docx, internal/server/web/index.html, examples/node-react/veld/packages/@maayn/..., examples/*SUMMARY*.md | 1h-2h | T5 |  |
| F-CLI-22 | P3 | Website: no lockfile (npm ci fails), lint errors, unbuilt in CI, LiveDemo duplicates the generator | reproduced defect | low | none | website/package.json, .gitignore (package-lock.json), website/src/components/sections/LiveDemo.tsx:41,857 | 2h-4h (WASM: 16h+) | T5 |  |
| F-FE-06 | P3 | Typed error package is dead code; errors are strings, stage is not machine-readable, file names are basenames | source-supported defect | low | none | internal/errors/errors.go:1-128 (unused by pipeline), internal/validator/validator.go:30-45 (`filepath.Base`), cmd/veld/util.go:45-75 (regex-like `SplitN(msg, ":", 3)` parsing, cache keyed by basename) | 6h-10h | T1 |  |
| F-FE-10 | P3 | `@default` validation is incomplete | reproduced defect | low | none | internal/validator/validator.go:194-196 (skipped for Map and union fields via the `continue` at :165 and :181), :416-467 (switch on `f.Type` ignores `IsArray`, model-typed fields, `long/time/bytes/any/json`) | 3h-5h | T1 |  |
| F-FE-11 | P3 | Undefined union member types are silently accepted | reproduced defect | low | none | internal/validator/validator.go:169-182; internal/parser/parser.go:561-573 (string literal and identifier both stored as bare text) | 2h-3h | T1 |  |
| F-FE-12 | P3 | `extends` validation misses field-override conflicts; cycle check is cubic | reproduced defect | low | none | internal/validator/validator.go:122-150 | 2h-4h | T1 |  |
| F-FE-16 | P3 | Success/error status literals are silently dropped when not a plain integer | reproduced defect | low | none | internal/parser/parser.go:873-875, :898-900, :1045-1050 (`if code, convErr := strconv.Atoi(...); convErr == nil`); internal/validator/validator.go:286-294 | 1h-2h | T1 |  |
| F-FE-19 | P3 | Diff overflows the stack on a cyclic `extends` when run on an unvalidated AST | suspected risk | low | low | internal/diff/diff.go:355-375 (`allFields` recursion without visited set); caller cmd/veld/generate.go:1126-1139 (`computePreChanges` parses but does not validate) | 0.5h-1h | T1 |  |
| F-FE-20 | P3 | Lint false positives and unreachable rules | reproduced defect | low | none | internal/lint/lint.go:80-111 (`checkUnusedModels` counts only action slots and extends), :162-190 and :192-211 (duplicate-route/-action unreachable after `Validate`; route key does not normalise `:param`) | 2h-3h | T1 |  |
| F-FE-21 | P3 | Incremental cache is bypassed by `runClean`, keyed only on `.veld` content, and misses enum/constants-only changes | source-supported defect | low | none | internal/cache/cache.go:65-82; cmd/veld/generate.go:758-760 (`runClean` deletes `.veld-cache.json` even with `--incremental`), :91-113 (only Models' and Modules' source files select target modules; an enum-only or constants-only file change selects none and then marks the cache up to date) | 3h-5h | T1 |  |
| F-FE-23 | P3 | Directive handling is lax and order-dependent | reproduced defect | low | none | internal/parser/parser.go:700-726 (module header order: description, then prefix, then baseUrl), :763-1058 (action directives assigned unconditionally) | 2h-3h | T1 |  |
| F-FE-24 | P3 | Strings have no escape sequences; descriptions cannot contain a double quote | design limitation | low | none | internal/lexer/lexer.go:259-270 | 1h-2h | T1 |  |
| F-FE-25 | P3 | Query/headers/input shapes are unconstrained | design limitation | low | none | internal/validator/validator.go:216-247 (only name existence) | 3h-5h | T1 |  |
| F-FE-26 | P3 | Constants validation is weak and loses literal quoting | reproduced defect | low | none | internal/parser/parser.go:406-423 (TString and TNumber both stored as raw text), internal/validator/validator.go:537-577 | 2h-3h | T1 |  |
| F-FE-27 | P3 | Workspace `consumes` has no cycle detection and `Foo` model/module name collision is not checked | source-supported defect | low | none | internal/validator/workspace.go:10-65 (self and unknown only; CLAUDE.md says "circular"), internal/validator/validator.go:203-207 (module names only checked against modules) | 1h-2h | T1 |  |
| F-FE-29 | P3 | AST JSON emits `null` for empty collections and has a fixed version | reproduced defect | low | none | internal/ast/ast.go:5-13, internal/parser/parser.go:27 | 0.5h | T1 |  |
| F-LSP-06 | P3 | Last good AST is retained when the buffer stops parsing | source-supported defect | low | none | internal/lsp/handler.go:261-273 | 1h–3h | T5 |  |
| F-LSP-14 | P3 | Committed `editors/vscode/out/` is stale and the extension has no tests or lockfile | reproduced defect | low | low | editors/vscode/out/*.js (tracked); no package-lock.json tracked | 2h–4h | T5 |  |
| F-LSP-17 | P3 | CI drift check cannot see new untracked files and the JSON output is untracked | suspected risk | low | none | .github/workflows/ci.yml:58-66; .gitignore:69 | 0.5h–1h | T5 |  |
| F-MULTI-13 | P3 | OpenAPI output is valid 3.0.3 but incomplete (no servers/security, error shape mismatch, defaults dropped) | reproduced defect | low | low | internal/generators/openapi/ | 4h-8h | T2 |  |
| F-MULTI-14 | P3 | `internal/mock` is unwired dead code; docs list a feature with no command | documentation mismatch | low | none | internal/mock/ (284 LOC); CLAUDE.md architecture table | 1h-6h | T4 |  |
| F-MULTI-15 | P3 | Documentation mismatches (Rust framework, Python Pydantic, WS undocumented, hardcoded post-generate hints, HTTP method list) | documentation mismatch | low | none | docs/architecture/overview.md:37 (Actix vs Axum in code), CLAUDE.md type-mapping table (Pydantic), CLAUDE.md HTTP methods (code also supports HEAD, OPTIONS, WS with required `stream:`/optional `emit:`), cmd/veld/generate.go summary hints (UserSchema, IUsersService, maayn package) | 2h-4h | T4 |  |
| F-NODE-18 | P3 | Doc/UX mismatches in CLI output and CLAUDE.md | documentation mismatch | low | none | cmd/veld/generate.go (import-instructions block near the end), CLAUDE.md | 2h-3h | T2 |  |
| F-REG-17 | P3 | Credentials stored in plaintext; file permissions are not enforced on Windows | reproduced defect | low | low | credentials.go:182-188 (`~/.veld/credentials.json`), 214-227 (`WriteFile(..., 0600)`, dir 0700) | 2h-4h | T3 |  |
| F-REG-18 | P3 | `Pack` publishes more than the package and ignores errors | source-supported defect | low | low | tarball.go:20,22-34,63-80 | 2h-4h | T3 |  |
| F-REG-19 | P3 | Email handling: SMTP proceeds without TLS, verification never enforced, token logged, HTML injection | source-supported defect (SMTP not exercised; not configured) | low | low | email.go:97-106 (STARTTLS optional, then `loginAuth` has no TLS requirement: lines 33-35 vs stdlib PlainAuth), email.go:68 (To header from user input; `Rcpt` rejects CR/LF so injection is blocked in practice), auth.go:363 (full verify URL with token logged), auth.go:43-50 (email format unvalidated), email.go template `fmt.Sprintf` with raw username (HTML injection into the email), whole `email_verified` flag unused for authorization (REPRODUCED: unverified account publishes). | 3h-5h | T3 |  |
| F-REG-22 | P3 | Documentation and config mismatches | documentation mismatch | low | low | CLAUDE.md lines 54-58, 392-399 (`veld login\|push\|pull\|serve`, `--secret mysecret`); cmd/veld/main.go:99 (only `veld registry ...` exists); serve.go:97-108; server.go:38 vs CLAUDE.md (secret "32+ chars"); registry.config.example.json (`secret` is a guessable placeholder); `RegistryConfig.Enabled` in config.go:97 is never read (`grep` finds no use) | 1h-2h | T3 |  |
| F-REG-23 | P3 | Member emails and user IDs exposed to anonymous callers | reproduced defect | low | low | orgs.go:80-103, db.go:323-340 (`ListOrgMembers` returns email), models.go OrgMember json tags | 1h --------------------------------------------------------------------------------------------------- | T3 |  |


## Defect register: P0 and P1 details (reproducer, expected vs actual, root cause, recommendation, regression check)


### F-CLI-01 (P0): `generate` and `watch` delete previous output and the breaking-change lock before validating; breaking-change gate (--strict/--force/prompt) is dead

- **Category:** output-safety
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** low
- **Location:** cmd/veld/generate.go:40-52, 759, 1021; cmd/veld/watch.go:56
- **Trigger / reproducer:** (a) generate OK, append `garbage {{{` to a .veld file, run `veld generate` -> exit 1 and `generated/`, `veld/generated`, `.veld.lock.json`, `.veld-cache.json` all gone. (b) generate OK, remove `action GetUser`, run `veld generate --strict` -> exit 0, no breaking-change report; same with --force and plain. `veld ci --strict` on the identical state exits 1 with "action GetUser removed" (ci does not call runClean).
- **Expected vs actual:** expected a failed/invalid generation to leave prior output intact and `--strict` to block breaking changes (documented CI gate). Actual: `runClean(rc)` (RemoveAll of every output dir plus `diff.DeleteLock`) runs at generate.go:759 before `computePreChanges` (generate.go:1021), so the lock baseline is already deleted and `HasBreaking` is always false; and any parse/validate/unknown-backend error leaves the user with an empty output dir. watch.go:56 does the same on start and so also loses lock/diff.
- **Impact:** CI breaking-change protection advertised in CLAUDE.md does not work for `veld generate`; any typo wipes generated code that the developer's own non-generated files import; the interactive prompt (`promptContinue`) is unreachable.
- **Root cause:** ordering - clean precedes parse/validate/diff; lock deletion is bundled into runClean.
- **Recommendation:** refactor - parse+validate+diff first, emit into a temp dir, swap on success, and stop deleting the lock in runClean.
- **Regression check:** test that a failing generate preserves out dir and lock; test `generate --strict` exits 1 when an action is removed.
- **Effort:** 6h-10h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007 staged writer
- **Source track:** cli-dist-ci-audit

### F-CLI-02 (P0): `--out`/`out` validation accepts the directory containing the contract sources; `generate` then deletes them

- **Category:** output-safety
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** low
- **Location:** internal/config/config.go:360-377, cmd/veld/generate.go:41-48
- **Trigger / reproducer:** project `proj/veld/{veld.config.json,app.veld,...}`; run `veld generate --out=../veld` (in `proj/`). validateOutPath only rejects basenames `..`, `.`, `/`, `\`. `../veld` resolves to the config dir; runClean `os.RemoveAll` deleted the entire `veld/` tree (sources + config), then failed `reading ...app.veld`.
- **Expected vs actual:** expected refusal when the output dir contains/equals the config dir, the input file's dir, cwd, or a parent of any of them. Actual: accepted and wiped. Also accepted: `--out=C:\Users\x`, any absolute path.
- **Impact:** irrecoverable loss of the user's contract files from one mis-typed flag or config value; same path is hit by `veld clean` and by workspace entries (not validated at all).
- **Root cause:** shallow lexical basename check; RemoveAll with no containment, marker-file, or ownership check.
- **Recommendation:** fix - refuse any out dir that is, contains, or is contained by config dir / input dir / cwd / home / drive root, and only delete dirs carrying a `.veld-generated` marker.
- **Regression check:** tests for out=configDir, out=parent-of-configDir, out=inputDir, workspace entry out=".".
- **Effort:** 3h-5h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007 staged writer
- **Source track:** cli-dist-ci-audit

### F-CLI-03 (P0): `veld clean` / pre-generate clean recursively delete any directory `out` points at (e.g. `src`)

- **Category:** clean
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** low
- **Location:** cmd/veld/generate.go:40-52, 1144-1180
- **Trigger / reproducer:** config `{"out":"./src"}`, `src/business.ts` present, `veld clean` -> prints "Cleaned ...\src", exit 0, `src/` is gone including the user file. No prompt, no manifest, no `--yes`.
- **Expected vs actual:** only files Veld generated should be removed (CLAUDE.md: "Veld never touches developer business logic"). Actual: whole directory.
- **Impact:** data loss for projects that mix generated and handwritten code in one folder (Veld's own docs allow `out` anywhere).
- **Root cause:** no generated-file manifest or marker; `os.RemoveAll(dir)`.
- **Recommendation:** refactor - write a manifest (or `.veld-generated` marker) and delete only manifest files (plus empty dirs); refuse to clean unmarked non-empty dirs without `--force`.
- **Regression check:** clean on a dir with foreign files leaves them; clean on an unmarked dir refuses.
- **Effort:** 4h-6h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007
- **Source track:** cli-dist-ci-audit

### F-CLI-05 (P0): `veld init` overwrites existing README.md and veld/*.veld files when no config exists

- **Category:** cli
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** cmd/veld/init.go:20-33, 651-658
- **Trigger / reproducer:** directory with `README.md` ("MY README") and `veld/app.veld` ("// mine") but no veld.config.json; `veld init` exit 0 -> README.md now "# My Veld Project", app.veld replaced by scaffold.
- **Expected vs actual:** CLAUDE.md "veld init never overwrites files"; the guard only checks for config files, then `os.WriteFile` unconditionally.
- **Impact:** silent loss of a project README and any pre-existing contract files.
- **Root cause:** existence check limited to two config paths; no per-file check.
- **Recommendation:** fix - skip or abort on any existing target path (O_EXCL).
- **Regression check:** init in dir with README.md leaves it byte-identical.
- **Effort:** 1h-2h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** cli-dist-ci-audit

### F-CLI-19 (P0): Go backend (plain `net/http` strategy) generates code that does not compile

- **Category:** examples
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** none
- **Location:** internal/emitter/backend/go/strategy/plain.go (route registration) -> generated `internal/routes/todos.go:13-18`
- **Trigger / reproducer:** `examples/go-svelte` copy: `veld generate` then `cd generated && go vet ./...` -> `internal\routes\todos.go:14:2: undefined: mux` (9 uses). The function parameter is named `r *http.ServeMux` but the body calls `mux.HandleFunc`.
- **Expected vs actual:** compiles; actual: build failure for the default Go backend.
- **Impact:** Go backend unusable out of the box.
- **Root cause:** param name vs variable name mismatch in the plain strategy template.
- **Recommendation:** fix - emit consistent receiver name; add compile test.
- **Regression check:** go build of generated output in CI (F-CLI-18).
- **Effort:** 1h-2h
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** cli-dist-ci-audit

### F-CLI-20 (P0): Python backend routes contain a SyntaxError for actions with input

- **Category:** examples
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** none
- **Location:** generated `routes/todos_routes.py:40`, emitter internal/emitter/backend/python
- **Trigger / reproducer:** `examples/python-vue` copy: `veld generate` (default config, no validate flag) then `python -I -m compileall generated` -> `input = CreateTodoInputSchema(**body = data  # CreateTodoInputSchema).model_dump()` SyntaxError (also users_routes.py:40).
- **Expected vs actual:** valid Python; actual: syntax error, module cannot import.
- **Impact:** Python backend route files unusable for any action with a body.
- **Root cause:** string composition bug in the Python route emitter (looks like a template arg mix-up; not root-caused here).
- **Recommendation:** fix - repair the template and add py_compile test.
- **Regression check:** compileall on generated output.
- **Effort:** 1h-3h
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** cli-dist-ci-audit

### F-FE-01 (P0): `import @alias/*` and `@alias/**` are lexed as a block comment and silently delete the rest of the file

- **Category:** lexer
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/lexer/lexer.go:240-251 and :253-257 (comment detection precedes the path-token branch at :315); parser consumer internal/parser/parser.go:210-215
- **Trigger / reproducer:** `fe-audit/cases/66-wildcard-comment-swallow/main.veld` containing `import @models/*` followed by `model Foo {...}` and `module M {...}`; run `veld.exe validate main.veld` and `veld.exe ast main.veld`. Also the shipped example `examples/microservices/veld/app.veld` (`import @services/**`): `veld ast veld/app.veld` returns `"models": null, "modules": null`.
- **Expected vs actual:** Expected the wildcard import to load all files in `models/` and keep the remainder of the file. Actual: the `/*` after `@models` opens a block comment that runs to the next `*/` or EOF; `Foo` and `M` vanish, validation prints "Contract is valid". The documented forms in docs/architecture/overview.md:55 and docs/DEVELOPER_GUIDE.md:189 (`import @models/*`) are the broken ones; `import /models/*` and `from @models import *` work.
- **Impact:** silent loss of models/modules, generated code missing endpoints with no diagnostic.
- **Root cause:** `/` followed by `*` is tested for comment start before the lexer knows it is inside an import path; `//` in paths has the same problem (`path: //x`, extra case e19).
- **Recommendation:** fix, make comment recognition context-aware (or lex `@alias/…` import paths as one token) and add an "unterminated block comment" error.
- **Regression check:** loader test with `import @models/*` + model afterwards asserting model count; lexer test that `@a/*` and `@a/**` yield TPath; microservices example must yield non-empty AST.
- **Effort:** 2h-4h
- **Owner:** T1
- **Reviewer:** T2
- **Source track:** frontend-audit

### F-FE-02 (P0): `veld generate` deletes the lock file and output before the breaking-change gate, so breaking-change detection never fires (and an invalid contract wipes the previous output)

- **Category:** diff
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** none
- **Location:** cmd/veld/generate.go:758-760 (`runClean`), gate at :1019-1035, `computePreChanges` :1126-1140, `runClean` removing the lock at :51; same ordering in cmd/veld/watch.go:56
- **Trigger / reproducer:** project in `fe-audit/gate/`: `veld generate` once with a model `A{x:int,y:string}` and `POST /a`; then change to `A{x:int}` and `PUT /b` (field removed, method and path changed) and run `veld generate --strict`. Exit code 0, no breaking-change output; `.veld.lock.json` was deleted by `runClean` before `computePreChanges` read it. Second repro: put `model A extends A {}`; `veld generate` fails validation but `generated/`, `.veld.lock.json`, `.veld-cache.json` are already gone.
- **Expected vs actual:** CLAUDE.md says "pre-emit gate: interactive prompt by default, `--strict` exits 1 (CI), `--force` skips prompt". Actual: gate input (`LoadLock`) is always empty for a non-dry-run, non-incremental generate; `--strict` can never fail.
- **Impact:** the CI safety feature advertised as a differentiator is a no-op; developers lose their previous generated output when the contract has any error.
- **Root cause:** `runClean(rc)` unconditionally precedes the gate and also deletes the lock (generate.go:51), and runs before parse/validate.
- **Recommendation:** fix, validate first, run the gate against the lock, only then clean/emit; never delete the lock in generate.
- **Regression check:** CLI test: generate, introduce each breaking change, assert `generate --strict` exits non-zero and leaves output intact; test that a validation failure leaves `generated/` untouched.
- **Effort:** 3h-6h
- **Owner:** T1
- **Reviewer:** T2
- **Dependencies:** IR v1 (ADR-002)
- **Source track:** frontend-audit

### F-LEAD-01 (P0): Generated output is not deterministic: types/index.ts export order varies between runs

- **Category:** determinism
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/emitter/tsshared/types.go:442 (range over map `claimed` builds moduleExports unsorted)
- **Trigger / reproducer:** veld init; run `veld generate` six times; sha256 of the tree: run 3 differed in types/index.ts (lead reviewer); docs track saw 4 distinct hashes in 6 runs
- **Expected vs actual:** Expected identical output; actual export order changes
- **Impact:** CI diffs, cache busting, breaks the documented determinism guarantee
- **Root cause:** map iteration order
- **Recommendation:** fix - sort moduleExports; add a determinism test over every emitter
- **Regression check:** generate twice on every fixture and byte-compare (CI)
- **Effort:** 1h-2h
- **Owner:** T2
- **Reviewer:** T1
- **Source track:** lead-reviewer

### F-MULTI-01 (P0): Hyphenated header names are emitted as identifiers in every language, producing uncompilable output

- **Category:** emitter-python
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** none
- **Location:** internal/parser/parser.go:961 (synthetic `<Action>Headers` model); consumers: python models/items.py:218, go internal/models/items.go:184, java CreateItemHeaders.java:8, csharp CreateItemHeaders.cs, php CreateItemHeaders.php:12, rust models/items.rs:211, node-js types/items.js:141, dart/kotlin/swift clients, graphql, prisma, sql
- **Trigger / reproducer:** `headers: { "Idempotency-Key": string }` (the form shown in CLAUDE.md contract syntax). `veld generate --backend=python --backend-framework=flask --validate` then `python -m compileall out/pyflask` -> SyntaxError. Same via go build, javac, node --check, graphql buildSchema.
- **Expected vs actual:** valid identifier (`idempotency_key`) with wire name kept in a string literal vs `idempotency-_key` / `Idempotency-key`.
- **Impact:** any contract using the documented headers feature with a standard header name cannot compile in any non-TS language.
- **Root cause:** header keys flow into Field.Name unchanged; each emitter's case-converter passes '-' through.
- **Recommendation:** fix -- sanitise header field names once in the parser (keep original as wire name) and have emitters use the wire name only in string literals.
- **Regression check:** compile-of-generated-code CI job per language with a contract containing a hyphenated header.
- **Effort:** 6h-12h
- **Owner:** T4
- **Reviewer:** T2
- **Dependencies:** F-MULTI-01; harness F-MULTI-02
- **Source track:** multi-emitter-audit

### F-MULTI-02 (P0): Emitter unit tests assert substrings; generated output is never compiled, so most non-Node backends ship uncompilable code with a green suite

- **Category:** emitter-rust
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/emitter/backend/*/*_test.go (e.g. rust/rust_test.go:160 asserts `pub fn router<S>() -> impl std::any::Any`)
- **Trigger / reproducer:** `go test ./internal/emitter/...` passes; building generated output fails (see run log).
- **Expected vs actual:** tests should fail when output does not compile; they enshrine broken stubs.
- **Impact:** regressions and pre-existing breakage are invisible; claims of working multi-stack generation are unsupported.
- **Root cause:** no golden-file or toolchain compile harness (docs/EXECUTION_PLAN_v1.md already lists "harness" as M2).
- **Recommendation:** fix -- add per-language compile smoke tests (go build, python compileall+import, javac, tsc, node --check; dockerised cargo/dotnet/php) driven from one rich contract.
- **Regression check:** CI matrix job fails on current main for rust/java/csharp/php/go/python.
- **Effort:** 24h-40h
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** multi-emitter-audit

### F-MULTI-03 (P0): Default Python backend (no --validate) emits a syntax-error route file and no route registration

- **Category:** emitter-python
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** none
- **Location:** internal/emitter/backend/python/routes.go:139 (and strategy), registration branch in routes.go
- **Trigger / reproducer:** `veld generate --backend=python --frontend=none`; compileall -> `input = CreateItemSchema(**body = data  # CreateItemSchema).model_dump()`.
- **Expected vs actual:** Pydantic-validated route calling the service and registered with Flask vs syntax error, missing `from flask import request`, missing schemas module (not emitted), `# register:` comment instead of add_url_rule, `raise RuntimeError(body)` instead of returning a response.
- **Impact:** the primary documented Python flow (CLAUDE.md "Pydantic validation") produces nothing runnable; only `--backend-framework=flask --validate` works (after F-MULTI-01/04 are fixed).
- **Root cause:** a strategy placeholder (ExtractBody) is interpolated as code; the Pydantic schema emitter named in python.go's header no longer exists.
- **Recommendation:** rewrite -- make Flask the default strategy and always emit the zero-dependency validators; drop Pydantic claims.
- **Regression check:** compileall + import + run Flask smoke test for default flags.
- **Effort:** 8h-14h
- **Owner:** T4
- **Reviewer:** T2
- **Dependencies:** F-MULTI-01; harness F-MULTI-02
- **Source track:** multi-emitter-audit

### F-MULTI-06 (P0): Go routes do not compile (`mux` undefined) and run without input validation or query/header binding

- **Category:** emitter-go
- **Evidence:** reproduced defect
- **User impact:** critical
- **Security severity:** low
- **Location:** internal/emitter/backend/go/strategy/plain.go:21,52; go/routes.go handler generation
- **Trigger / reproducer:** `go build ./...` in out/go -> `internal/routes/items.go:14:2: undefined: mux` (after the F-MULTI-01 rename). After patching: `{}` body -> 201; role "bogus" accepted; GetItem receives nil query; CreateItem receives nil headers; date field "2020-01-01" fails to decode.
- **Expected vs actual:** compile + validate vs compile error and zero validation.
- **Impact:** Go backend unusable out of the box; once patched, required fields/enums are unenforced and auth headers unreachable.
- **Root cause:** strategy emits `mux.` while the function parameter is named `r`; no validation emitter for Go.
- **Recommendation:** fix -- name the param consistently, add generated Validate() methods, bind query/headers, map date to a date-aware type.
- **Regression check:** go build + vet of generated output; httptest-based smoke test.
- **Effort:** 12h-20h
- **Owner:** T4
- **Reviewer:** T2
- **Dependencies:** F-MULTI-01; harness F-MULTI-02
- **Source track:** multi-emitter-audit

### F-NODE-01 (P0): `veld generate` deletes the whole output directory (including user files) before every run

- **Category:** generate-cli
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** low
- **Location:** cmd/veld/generate.go:40-50 (runClean), 757-760 (call); comment at 141-142 claims the opposite
- **Trigger / reproducer:** put my-notes.ts, usersub/a.txt, routes/stale.routes.ts in the out dir; `veld generate --force --out=./o1`. All gone.
- **Expected vs actual:** "user files are never touched; use `veld clean`" vs `os.RemoveAll(outDir)` on every non-dry-run generate (also workspace and watch).
- **Impact:** hand-added files, node_modules, co-located code in the out dir are lost; all mtimes reset, defeating tsc/vite incremental caches.
- **Root cause:** runClean invoked unconditionally; no manifest of generated files.
- **Recommendation:** refactor: write a manifest of emitted files and delete only stale manifest entries, never RemoveAll.
- **Regression check:** foreign file in out dir survives two generates; a removed module's file is deleted.
- **Effort:** 6h-10h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007
- **Source track:** node-ts-audit

### F-NODE-02 (P0): `--out` pointing at the project/source directory deletes the contract and sources

- **Category:** output-safety
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** low
- **Location:** internal/config/config.go:375 (only rejects "..", ".", "./", "../."), cmd/veld/generate.go:40
- **Trigger / reproducer:** sandbox sb/proj containing app.veld and notes.txt: `veld generate --input=app.veld --out=../proj`. On Windows RemoveAll partially failed (cwd in use) but app.veld and notes.txt were deleted, then the run errored "reading app.veld". Unix would remove everything. `--out=.`/`..` are refused. Root (`/`) deliberately not tested.
- **Expected vs actual:** refuse an out dir that contains the input, config, cwd or a filesystem root vs lexical check only.
- **Impact:** irreversible deletion of the user's source of truth; with F-NODE-01 any `--out=src` wipes code.
- **Root cause:** textual validation; recursive clean.
- **Recommendation:** fix: resolve to absolute and refuse if out is an ancestor-or-equal of cwd/config dir/input file or a root; fix F-NODE-01.
- **Regression check:** tests for out equal to cwd, parent, input's dir, symlinks.
- **Effort:** 2h-3h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007 staged writer
- **Source track:** node-ts-audit

### F-NODE-03 (P0): A failing generate leaves the previous output deleted; watch does this on every save

- **Category:** watch
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** cmd/veld/generate.go:757-760 (clean before parse), cmd/veld/watch.go:270
- **Trigger / reproducer:** generate OK, append garbage to the contract, regenerate: parse error printed and the out dir no longer exists. In `veld watch`, saving a half-edited contract wipes generated code (debounce is 300ms; errors are printed and the loop continues, which is good), breaking the dev server until the contract is valid again.
- **Expected vs actual:** failed run is a no-op vs destructive.
- **Impact:** dev-loop breakage; CI leaves an empty out dir after a failure.
- **Root cause:** clean precedes parse; no staging directory.
- **Recommendation:** refactor: emit to a temp dir and swap on success only.
- **Regression check:** broken contract leaves previous output byte-identical.
- **Effort:** 4h-8h (shared with F-NODE-01)
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007
- **Source track:** node-ts-audit

### F-NODE-04 (P0): Breaking-change gate and `--strict` never fire (lock deleted before the diff)

- **Category:** generate-cli
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** cmd/veld/generate.go:50 (diff.DeleteLock inside runClean) vs :205 (hasLock)
- **Trigger / reproducer:** generate; remove action DeleteItem; `veld generate --strict` -> exit 0, nothing reported; lock re-saved without the action.
- **Expected vs actual:** exit 1 with breaking changes (CLAUDE.md "Contract Safety Features") vs silent success.
- **Impact:** the advertised CI protection is non-functional.
- **Root cause:** runClean removes .veld.lock.json and .veld-cache.json ahead of the diff.
- **Recommendation:** fix: stop deleting the lock in generate/watch.
- **Regression check:** end-to-end: remove an action, run `generate --strict`, expect exit 1.
- **Effort:** 1h-2h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007
- **Source track:** node-ts-audit

### F-NODE-05 (P0): Static routes shadowed by earlier parametric routes (including middleware-protected ones)

- **Category:** emitter-node
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** high
- **Location:** internal/emitter/backend/node/routes.go:75-87 (actions emitted in declaration order)
- **Trigger / reproducer:** `GET /items/:id` declared before `GET /items/boom`, `/items/guarded` (middleware AuthGuard), `/items/old`. Requests to each are answered by getItem; authGuard never invoked.
- **Expected vs actual:** each action reached by its own handler vs later same-method static paths unreachable and answered by another action without the middleware.
- **Impact:** dead endpoints; protected action silently replaced by an unprotected one.
- **Root cause:** no specificity ordering and no validator/lint rule for overlapping routes (lint duplicate-route does not catch it).
- **Recommendation:** fix: emit static-segment routes before parametric ones per method and add a validator error for ambiguous overlap.
- **Regression check:** `/:id` then `/me` reaches the right handler under express.
- **Effort:** 3h-5h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** F-MULTI-01; harness F-NODE-19
- **Source track:** node-ts-audit

### F-NODE-06 (P0): Generated output does not compile under `tsc --strict`

- **Category:** emitter-ts
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** frontend/typescript/client.go (`query as Record<string, unknown>`, `extraHeaders: Record<string,string>`), backend/node/routes.go (headers object literal)
- **Trigger / reproducer:** any contract with a `query:` model or `headers:` block; `tsc --noEmit --strict` (errors in section 1 step 5). Every framework wrapper inherits them.
- **Expected vs actual:** clean compile vs 4 errors (5 with --validate).
- **Impact:** users must disable strict or edit generated files.
- **Root cause:** models are classes with methods, so literals/instances are not assignable to or from Record types; no test compiles the output.
- **Recommendation:** fix: emit plain interfaces (or cast via unknown) and add a tsc compile test.
- **Regression check:** CI generates the rich contract and runs tsc --strict --noEmit.
- **Effort:** 1h-2h (casts) or 6h-10h (interface migration)
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** harness F-NODE-19
- **Source track:** node-ts-audit

### F-REG-01 (P0): 2FA bypass - the pre-TOTP "partial" JWT is a fully valid session token

- **Category:** server-auth
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** high
- **Exploitability:** anyone who knows a victim's password (the exact attacker TOTP exists to stop); no other precondition. Also usable by a stolen password from a leaked DB of another service.
- **Location:** internal/server/handlers/auth.go:105-117 (partial issued with `IssueJWT`), internal/server/auth/token.go:56-92 (no purpose claim), internal/server/auth/middleware.go:70-100 (accepts any valid JWT as a session)
- **Trigger / reproducer:** `e2e-run2.log` E7. alice enables TOTP; `POST /auth/login` returns `{"partial":"eyJ...","totp_required":true}`; `GET /auth/me` with `Authorization: Bearer <partial>` -> `HTTP 200 {... "totp_enabled":true}`; `POST /tokens` with the partial -> HTTP 201 (admin API token minted). Never supplied a TOTP code.
- **Expected vs actual:** partial token should only be accepted by /auth/totp-login; actual: accepted everywhere for 5 minutes, and it can mint a permanent admin API token.
- **Impact:** two-factor authentication gives no protection at all against password compromise; attacker gets a durable `vtk_` token.
- **Root cause:** `JWTClaims` has only sub/exp/iat; the partial and the full token are indistinguishable.
- **Recommendation:** fix - add a `typ`/`purpose` claim (e.g. "2fa-pending") that `resolveAuth` rejects, and require it in totp-login.
- **Regression check:** test that a partial token gets 401 on /auth/me, /tokens, /packages (POST).
- **Effort:** 1h-2h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004 split; server tests F-CLI-23
- **Source track:** registry-audit

### F-REG-02 (P0): Storage path traversal on publish - arbitrary `*.tar.gz` write, overwrite and delete

- **Category:** server-storage
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** high
- **Exploitability:** any registered user (registration and org creation are open and unverified); becomes owner of their own org, which is the only precondition for Publish.
- **Location:** internal/server/handlers/packages.go:122,177 (unvalidated `manifest.name`/`version` into the key); internal/server/storage/storage.go:33-36 (`Clean` then `Join` with no containment check); packages.go:217 and :334 (Delete of the same key)
- **Trigger / reproducer:** `e2e-run1.log` E4. Publish `{"org":"mal9041","name":"../../traversal-9041","version":"1.0.0"}` -> 201; file created at `<scratch>\reg-audit\traversal-9041\1.0.0.tar.gz`, i.e. outside the storage root. Then publish `name="../acme9041/auth"` -> victim's `acme9041/auth/1.0.0.tar.gz` overwritten (disk sha changed from 53da33... to d94982...; victim download shows header sha 53da33... but body sha d94982...). Then mallory `DELETE /packages/mal9041/..%2Facme9041%2Fauth/1.0.0` -> 204 and victim's tarball is gone (`HTTP 404 file not found in storage`).
- **Expected vs actual:** name/version must be constrained and the key must stay under the root; actual: arbitrary relative key. Content is attacker-controlled bytes; suffix is always `.tar.gz` (limits it to overwriting/creating `*.tar.gz` files anywhere the server user can write, and deleting such files). It breaks other orgs' packages (integrity-check failure at best; if the victim version's DB sha were also attacker-controlled it would be a substitution) and can destroy any `*.tar.gz` on the host.
- **Impact:** cross-tenant integrity/availability damage; arbitrary file write/delete of `.tar.gz` outside the data dir; combined with the missing sha anchoring on the client (F-REG-09) a path to serving wrong content.
- **Root cause:** no validation of package name/version (no charset, length, semver); trust in `filepath.Clean`.
- **Recommendation:** fix - validate org/name (`^[a-z0-9][a-z0-9._-]{0,63}$`) and strict semver at the handler, derive the storage key from the DB ids (content-addressed by sha), and verify `filepath.Rel(root, p)` stays inside.
- **Regression check:** publish with `..`, `/`, `%2F`, NUL, backslash, very long, and unicode names -> 400; assert nothing outside root.
- **Effort:** 3h-6h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** F-REG-02 fix
- **Source track:** registry-audit

### F-REG-03 (P0): Token scope model is not enforced; a read-only token can mint admin tokens and delete versions

- **Category:** server-authz
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** high
- **Exploitability:** holder of any valid API token (e.g. a leaked read-only CI token).
- **Location:** internal/server/handlers/auth.go:393-439 (CreateToken: no scope check, scopes/org_id accepted verbatim); handlers/packages.go:276-338 (Deprecate/Delete never call `HasScope`); only packages.go:105 checks `write`; middleware.go:57-68 (`HasScope` returns true for JWT); `read` scope never checked; `org_id` of a token never enforced
- **Trigger / reproducer:** E2/E3. `ro` token (scopes [read]) -> `POST /tokens` with `scopes:[read,write,delete,admin]` -> token returned. Publish with `ro` -> 403 "missing 'write' scope", but with the minted token -> 201. `DELETE /packages/acme/auth/0.9.0` with the read-only token -> 204.
- **Expected vs actual:** scopes limit what a token can do; actual: trivially bypassed.
- **Impact:** least-privilege CI tokens give no protection; a leak of a "read" token is a full account compromise (publish, delete, rotate).
- **Root cause:** scopes bolted on at one handler; token management reachable with token auth.
- **Recommendation:** fix - require JWT session (or `admin` scope) for token management, validate scope enum, check scopes centrally in middleware per route, enforce `org_id`.
- **Regression check:** matrix test of (scope x endpoint).
- **Effort:** 4h-8h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004; server tests
- **Source track:** registry-audit

### F-REG-09 (P0): Client integrity check is optional and self-referential; http registries are accepted without warning

- **Category:** client
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** high
- **Exploitability:** network attacker on any `http://` registry URL; malicious or compromised registry operator; anyone able to alter response headers (misconfigured proxy/CDN).
- **Location:** internal/registry/client.go:208-210 (sha taken from the same response; copy error ignored), cmd/veld/registry.go:232-236 (`if remoteSHA != ""`), 37-60 (no scheme check on login), credentials.go:230 (token sent over whatever scheme)
- **Trigger / reproducer:** MITM proxies in section 4: stripping `X-Veld-SHA256` -> `Pulled`, malicious `veld.config.json` with `postGenerate` hook written; replacing body and header consistently -> `Pulled`. `veld registry login --registry http://localhost:18080` accepted, token stored and sent in cleartext (CLAUDE.md and help text show http examples; `ftp://` is rejected only by Go's transport).
- **Expected vs actual:** fail closed when the sha is absent; authenticity anchored out of band (lockfile/signature) and https required for non-loopback hosts; actual: all optional.
- **Impact:** tampered contracts flow straight into code generation (`.veld` models drive generated code; a pulled `veld.config.json` can carry a `postGenerate` command that runs on the next `veld generate` if it ends up as the project config, see F-REG-20). Tokens and packages travel in plaintext if the operator uses http.
- **Root cause:** checksum is transport-corruption detection, not a trust root.
- **Recommendation:** fix - require header (fail closed), refuse non-https except loopback unless `--insecure`, add a `veld.lock` with pinned sha256, and plan package signing.
- **Regression check:** pull against a server omitting the header must fail; login to `http://example.com` must fail.
- **Effort:** 6h-16h (signatures larger)
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-008
- **Source track:** registry-audit

### F-REG-10 (P0): Unpack has no limits, writes in place, and drops I/O errors

- **Category:** tarball
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** malicious package author (any registered user can publish a public package; archive content is never validated server-side) or malicious registry; victim = anyone who runs `veld registry pull`.
- **Location:** internal/registry/tarball.go:98-143 (see section 1 items 3-5), client.go:209
- **Trigger / reproducer:** section 2 table: 1.24 MiB -> 1 GiB file in 2.9 s; 1.4 MB -> 100,000 files in 4 min; partial extraction left behind; truncated file left; `MkdirAll`/`io.Copy`/`Close` errors dropped (tarball.go:130,134,139,140).
- **Expected vs actual:** cap entries (count, per-file, total), reject non-regular entries explicitly, extract to a temp dir and rename atomically, check all errors; actual: none.
- **Impact:** disk-fill/CPU denial of service on developer or CI machines; corrupt half-extracted contracts used silently; symlink-safety only by accident (item 2); stale files persist across versions.
- **Root cause:** minimal implementation.
- **Recommendation:** rewrite - small hardened extractor (regular files and dirs only, `LimitReader`, count/size caps, containment via `filepath.Rel`, temp dir + rename) shared by CLI and server validation.
- **Regression check:** the section 2 corpus as a test table.
- **Effort:** 4h-8h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-008
- **Source track:** registry-audit

### F-REG-11 (P0): Stored XSS sinks in the web UI (usernames, org text, versions, deprecation text)

- **Category:** server-web
- **Evidence:** source-supported defect (hostile values stored and returned by the API - reproduced; rendering in a browser not executed)
- **User impact:** high
- **Security severity:** high
- **Exploitability:** any registered user (username, org description/display_name, token names) and any publisher (version string, package name; version string is not validated). Victim = any visitor/member viewing the page.
- **Location:** internal/server/web/index.html:335 (`${u.username}`), 377 (`${e.message}`), 420, 456-457, 486-492, 519-524 (`v.version`, `v.deprecated`), 543-544, 564-565, 585-587 (`m.username`, `m.email`, inline `onclick="removeMember('${org.name}','${m.username}')"`), 616, 633-634; only `escHtml` uses are lines 497, 503; no CSP.
- **Trigger / reproducer:** E1/E12: org description `<img src=x onerror=alert(1)>` stored (psql shows it verbatim); username `<img src=x onerror=alert(document.domain)>` registered with 201; version `<svg onload=alert(1)>` accepted by the handler (on this Windows host the storage layer then rejected it with a 500 because `<>` is invalid in NTFS filenames; on Linux it would succeed - SOURCE).
- **Expected vs actual:** output-encode every interpolation; actual: template literals into `innerHTML`.
- **Impact:** script runs same-origin as the victim; the HttpOnly cookie is not readable but the script can call the API as the victim (mint admin API tokens, publish, delete, change org roles) and read the minted token back.
- **Root cause:** SPA built with string templates, no escaping layer, no CSP, no input constraints.
- **Recommendation:** rewrite - render via `textContent`/a small escaping template tag or a framework, add a strict CSP, and validate username/org/package/version charsets server-side.
- **Regression check:** playwright test creating hostile names and asserting no script execution; server tests for charset.
- **Effort:** 6h-12h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004
- **Source track:** registry-audit

### D-01 (P1): Documentation mismatch: Node backend generates Zod schemas (`schemas/schemas.ts`) and validates input with Zod, ZodError->400

- **Category:** docs
- **Evidence:** documentation mismatch
- **User impact:** medium
- **Security severity:** none
- **Location:** CL:9,81,444-446,467-468; README.md:32,272,292; OV Node row; changelog-v0.1 items 1,6; GS:63-68; T:849,928-950; GP:694 (demo); website DocsPage.tsx:1608,1758
- **Trigger / reproducer:** read the cited lines
- **Expected vs actual:** Documented: Node backend generates Zod schemas (`schemas/schemas.ts`) and validates input with Zod, ZodError->400 | Actual: No Zod is emitted at all; default routes do no validation; opt-in `--validate` emits zero-dep `_validators.ts`
- **Impact:** Reader is misled (MISLEADING)
- **Root cause:** docs not updated with code
- **Recommendation:** fix - correct the documentation (or the code where the doc states the intended behaviour)
- **Regression check:** docs review checklist item
- **Effort:** 0.5h-2h
- **Owner:** D1
- **Reviewer:** T1
- **Dependencies:** owner validation (ch.16)
- **Source track:** docs-claims

### D-02 (P1): Documentation mismatch: Output is deterministic (non-negotiable / evaluation criterion)

- **Category:** docs
- **Evidence:** documentation mismatch
- **User impact:** medium
- **Security severity:** none
- **Location:** CL:508; README.md:419; GP 14.3
- **Trigger / reproducer:** read the cited lines
- **Expected vs actual:** Documented: Output is deterministic (non-negotiable / evaluation criterion) | Actual: Not deterministic: `types/index.ts` export order varies per run
- **Impact:** Reader is misled (MISLEADING)
- **Root cause:** docs not updated with code
- **Recommendation:** fix - correct the documentation (or the code where the doc states the intended behaviour)
- **Regression check:** docs review checklist item
- **Effort:** 0.5h-2h
- **Owner:** D1
- **Reviewer:** T1
- **Dependencies:** owner validation (ch.16)
- **Source track:** docs-claims

### D-03 (P1): Documentation mismatch: `veld login/logout/push/pull/serve/doctor` are top-level commands

- **Category:** docs
- **Evidence:** documentation mismatch
- **User impact:** medium
- **Security severity:** none
- **Location:** CL:52-58,392-399; GP 14.1; DG:381
- **Trigger / reproducer:** read the cited lines
- **Expected vs actual:** Documented: `veld login/logout/push/pull/serve/doctor` are top-level commands | Actual: Only `veld registry <cmd>`; `doctor` unregistered, does not run
- **Impact:** Reader is misled (MISLEADING)
- **Root cause:** docs not updated with code
- **Recommendation:** fix - correct the documentation (or the code where the doc states the intended behaviour)
- **Regression check:** docs review checklist item
- **Effort:** 0.5h-2h
- **Owner:** D1
- **Reviewer:** T1
- **Dependencies:** owner validation (ch.16)
- **Source track:** docs-claims

### D-10 (P1): Documentation mismatch: MIT license

- **Category:** docs
- **Evidence:** documentation mismatch
- **User impact:** medium
- **Security severity:** none
- **Location:** README.md:538-540
- **Trigger / reproducer:** read the cited lines
- **Expected vs actual:** Documented: MIT license | Actual: No root LICENSE; chocolatey nuspec links to missing file
- **Impact:** Reader is misled (MISLEADING)
- **Root cause:** docs not updated with code
- **Recommendation:** fix - correct the documentation (or the code where the doc states the intended behaviour)
- **Regression check:** docs review checklist item
- **Effort:** 0.5h-2h
- **Owner:** D1
- **Reviewer:** T1
- **Dependencies:** owner validation (ch.16)
- **Source track:** docs-claims

### F-CLI-07 (P1): Config precedence: flat fields override nested, contrary to documentation

- **Category:** config
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** none
- **Location:** internal/config/config.go:137-150 (normalize), CLAUDE.md config section, config.go:34 comment
- **Trigger / reproducer:** `{"backend":"python","backendConfig":{"target":"node-ts"},"frontend":"none"}` -> Python generated.
- **Expected vs actual:** documented "nested takes precedence"; actual flat wins (nested only fills empty flat fields). Same pattern for framework, out, validate, postGenerate, frontend.
- **Impact:** users migrating to the nested format with a leftover flat key get the old target silently.
- **Root cause:** `&& c.Backend == ""` guards in normalize().
- **Recommendation:** fix - make nested win (or error on conflicting keys) and add a test.
- **Regression check:** unit test with both formats set.
- **Effort:** 1h-2h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** cli-dist-ci-audit

### F-CLI-08 (P1): `--out` does not override `backendConfig.out` / `backendOut`

- **Category:** config
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** none
- **Location:** internal/config/config.go:312-335, 392-405
- **Trigger / reproducer:** config with nested `backendConfig.out: ./nestout`, run `generate --dry-run --out=./flagout` -> backend -> ./nestout, frontend -> ./flagout.
- **Expected vs actual:** flag beats config everywhere; actual: only unset side follows `--out`. `FlagOverrides.BackendOut/FrontendOut` exist but generate has no `--backend-out` flag.
- **Impact:** CI overrides of the output dir silently partially ignored.
- **Root cause:** flag sets only `cfg.Out`; BackendOut non-empty takes priority.
- **Recommendation:** fix - `--out` should clear BackendOut/FrontendOut, or add explicit flags.
- **Regression check:** test flag vs nested out.
- **Effort:** 1h-2h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** cli-dist-ci-audit

### F-CLI-15 (P1): Release workflow: Homebrew job never publishes; package jobs skipped when only wrappers change

- **Category:** release
- **Evidence:** source-supported defect
- **User impact:** high
- **Security severity:** none
- **Location:** .github/workflows/release.yml:133-170 (resolve), 392-422 (update-homebrew), publish-* `needs: [release, resolve]`
- **Trigger / reproducer:** read workflow. (1) update-homebrew seds `packages/homebrew/Formula/maayn-veld.rb` on the runner and exits; no commit/push/PR to a tap, so brew users never get the formula and the repo copy keeps `PLACEHOLDER_SHA256_*`. (2) `release` runs only if `publish_binary == 'true'`; publish-npm/pip/etc. need `release`, so a tag where only `packages/npm/` changed skips the release job and therefore npm (skipped dependency). (3) `curl -sL` (no `-f`) turns a 404 into an HTML "archive" and fails later at tar. (4) `publish_composer` is computed but there is no composer job. (5) Release notes advertise `brew install veld-dev/tap/veld` while formula is `maayn-veld/tap/maayn-veld`.
- **Expected vs actual:** documented "auto-detect and publish affected targets"; actual partial.
- **Impact:** brew and composer channels never ship; wrapper-only fixes cannot be released by tag.
- **Root cause:** job dependency design and unfinished jobs.
- **Recommendation:** refactor - decouple wrapper jobs from the `release` job (`if: always() && ...`), add `curl -fsSL`, push the formula to a tap repo, add composer or drop it.
- **Regression check:** workflow dry-run (act) or a tag on a fork.
- **Effort:** 4h-8h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** cli-dist-ci-audit

### F-CLI-16 (P1): npm and pip wrappers: no checksum, unpinned download, swallowed postinstall failure, Python 3.8/3.9 syntax, doc name mismatch

- **Category:** dist-npm
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Location:** packages/npm/install.js:25-45, 141-205; packages/pip/veld/__main__.py:23,61 (`str | None`); packages/pip/pyproject.toml (requires-python >=3.8); packages/README.md
- **Trigger / reproducer:** npm: `VELD_VERSION=0.0.0 npm install ../pkg` -> install exit 0, `npx veld` exit 1 "binary not found". Source: install.js resolves `releases/latest` rather than the wrapper's own version, downloads over https with no checksum or signature, then extracts with tar/Expand-Archive; `go install ...@latest` fallback is also unpinned. pip: `def get_platform_key() -> str | None` is evaluated at import on Python <3.10. Docs say `npm install veld` / `pip install veld` but names are `@maayn/veld` / `maayn-veld`. No HTTP_PROXY support in install.js. `process.exit(result.status || 0)` masks signal exits.
- **Expected vs actual:** pinned, checksum-verified, failing install; actual otherwise.
- **Impact:** supply-chain exposure (a compromised release or MITM of redirect chain is run as developer code without verification); broken installs reported as success; pip wrapper unusable on 3.8/3.9 despite metadata.
- **Root cause:** wrapper written for convenience.
- **Recommendation:** fix - pin to wrapper version, verify `checksums.txt` sha256, fail the install loudly, add `from __future__ import annotations`, correct README names.
- **Regression check:** install tests with a local HTTP server serving a tampered archive.
- **Effort:** 4h-6h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** cli-dist-ci-audit

### F-CLI-18 (P1): CI verifies only Linux unit tests; no signing/SBOM; no generated-code compilation; nothing builds website/editors/wrappers

- **Category:** ci
- **Evidence:** design limitation
- **User impact:** high
- **Security severity:** low
- **Location:** .github/workflows/ci.yml, release.yml
- **Trigger / reproducer:** read workflows.
- **Expected vs actual:** a code generator whose product is source code should compile its output for each target; actual: only `go build/vet/test` on ubuntu plus cross-compile; no windows/macos test runner (the CLI has Windows-specific path code), no govulncheck/lint, no cosign/SBOM/attestation, actions not SHA-pinned.
- **Impact:** regressions F-CLI-19/20 shipped unnoticed.
- **Root cause:** absent jobs.
- **Recommendation:** refactor - add matrix (ubuntu/windows/macos), a job that generates every example and compiles with tsc, py_compile, go build, cargo check, javac, dotnet build, php -l; add govulncheck, SBOM, cosign.
- **Regression check:** the new job fails on current examples (proves it).
- **Effort:** 12h-20h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** cli-dist-ci-audit

### F-CLI-23 (P1): Test hygiene gaps (untested registry server, strategy subpackages, no generated-code compile tests, CLI coverage reads 0%)

- **Category:** tests
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Location:** internal/server/**, internal/registry, internal/docsgen, internal/graphqlgen, internal/language, emitter/backend/*/strategy, cmd/veld
- **Trigger / reproducer:** `go test -cover ./...` shows no test files for the listed packages; cmd/veld reports 0.0% because tests exec a built binary. Low coverage: node backend 19.6%, python 28.5%, loader 30.2%, lsp 33.6%, emitter 35.4%, validator and parser 53.7%.
- **Expected vs actual:** auth/JWT/TOTP/upload code covered; actual: zero tests.
- **Impact:** security-critical registry code (HMAC JWT, password hashing, tarball extraction/upload, TOTP) has no regression net.
- **Root cause:** prioritisation.
- **Recommendation:** fix - add handler/auth tests with httptest and a fake storage, plus generated-code compile tests.
- **Regression check:** coverage gates per package.
- **Effort:** 16h-30h
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** cli-dist-ci-audit

### F-FE-03 (P1): `veld fmt --write` corrupts valid files (drops comments, string quotes, spacing) and panics on `model }`

- **Category:** format
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/format/format.go:27-33 (formats from a token stream), :187-189 (default branch writes `tok.Value` of TString without quotes), :153 (`writeIndent(baseIndent+depth)` with negative depth), :113-131
- **Trigger / reproducer:** copy `examples/node-typescript/veld` to a temp dir and run `veld.exe fmt --write app.veld models/*.veld modules/*.veld`; then `veld validate app.veld` fails with `parsing ...todos.veld: line 4: expected "action", got "description"`. Output sample: `description:Todo management`, `import@models/todo`, `action   ListTodos\n {`. Comments are gone (`cases/52-comments-preserve`). Panic: `printf 'model }\n' > x.veld; veld fmt x.veld` (`cases/67-fmt-panic`).
- **Expected vs actual:** A formatter must preserve semantics and comments. Actual: the output no longer parses for every file that contains a quoted string or an import (6 of the 72 cases: 03, 11, 19, 20, 36b, 38 were valid before formatting; and every shipped example file).
- **Impact:** destructive in-place rewrite; the command is hidden but documented in `veld fmt` help and CLAUDE.md.
- **Root cause:** formatter is built on the lexer, which discards comments, quote characters and whitespace; no round-trip test.
- **Recommendation:** rewrite, build the formatter on a lossless CST/token stream (keep trivia and raw lexeme) and add parse-after-format and idempotency checks.
- **Regression check:** golden tests: for every `examples/**/*.veld`, `Format(Format(x)) == Format(x)`, `Parse(Format(x))` equals `Parse(x)`, and comments survive; fuzz target that `Format` never panics.
- **Effort:** 12h-20h
- **Owner:** T1
- **Reviewer:** T2
- **Dependencies:** lossless tokens (ch.08 §8.7)
- **Source track:** frontend-audit

### F-FE-05 (P1): Diagnostics are first-error-only for lexer/parser, line-only, and sometimes point at the wrong line

- **Category:** parser
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** none
- **Location:** internal/lexer/lexer.go:363 (returns `errors[0]`), internal/loader/loader.go:94-101, internal/parser/parser.go (every `return ..., err`), internal/lexer/lexer.go:195-199 (`Token{Type,Value,Line}`)
- **Trigger / reproducer:** `cases/62-multi-error-parse/main.veld` (two independent syntax errors; only one reported, on line 3 while the first mistake is `x: bad bad` on line 2); `cases/58-lone-cr/main.veld` (reports line 1, true line 3); `cases/04-unterminated-block/main.veld` (reports non-existent line 3).
- **Expected vs actual:** Expected all syntax errors with line:column and a snippet (validate reports all semantic errors but no snippet; `validate` never prints one). Actual: one error, no column, wrong lines in three of the four tested syntax cases.
- **Impact:** slow edit-compile loops; IDE/LSP squiggles cannot be precise.
- **Root cause:** no panic-mode recovery in the parser; tokens carry no column/offset; lexer errors are collected then truncated.
- **Recommendation:** refactor, add column/offset to Token, return all lexer errors, add statement-level synchronisation in the parser.
- **Regression check:** golden diagnostic tests (line AND column) for 10 malformed files; test that two errors yield two diagnostics.
- **Effort:** 10h-16h
- **Owner:** T1
- **Reviewer:** T2
- **Dependencies:** ADR-002 TypeRef where noted
- **Source track:** frontend-audit

### F-FE-18 (P1): Breaking-change classifier misses enums, nested models, middleware, errors and unions; status change is non-breaking

- **Category:** diff
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/diff/diff.go:97-191 (module/action only), :195-217 (diff only the model directly named by an action slot), :377-382 (`typeChanged` ignores `UnionTypes`), :182-188 (status -> `Added`); internal/diff/lock.go:36-40 (lock stores Modules and Models only, no Enums/Constants/Prefix)
- **Trigger / reproducer:** `fe-audit/fz/internal/diff_test.go` (`go test ./internal -run TestDiffScenarios -v` in `fe-audit/fz`). Results: enum member removed -> 0 changes; nested model `Inner{z:int}` -> `Inner{}` or `z:string` -> 0 changes; middleware added -> 0; errors list changed -> 0; union field added/changed -> only "field added"; status 201->200 -> 1 change classified **Added (non-breaking)**. Detected correctly: removed action, method change, path change, input field removed, input field type change, optional->required (input), new required input field, required->optional (output), output enum->string type change, action rename (remove + add).
- **Expected vs actual:** removal of an enum member, and any field change inside a nested model, are wire-breaking.
- **Impact:** even once F-FE-02 is fixed, the gate will pass real breaking changes (nested model and enum changes are the most common ones).
- **Root cause:** diff walks action slots one level deep and the lock omits enums.
- **Recommendation:** refactor, diff models/enums by name (full set) and recurse through type references.
- **Regression check:** the scenario table above as a permanent test with expected Breaking/Added classification.
- **Effort:** 8h-14h
- **Owner:** T1
- **Reviewer:** T2
- **Dependencies:** IR v1 (ADR-002)
- **Source track:** frontend-audit

### F-FE-22 (P1): "Single source of truth" language spec is not what the compiler uses; a dead second parser exists

- **Category:** language
- **Evidence:** documentation mismatch
- **User impact:** low
- **Security severity:** none
- **Location:** internal/language/constants.go:29-110 (spec), internal/lexer/lexer.go:374-451 (hard-coded keyword table), internal/language/veld.go:82-200 (regex mini-parser, 0% coverage, unused)
- **Trigger / reproducer:** spec lists HTTP methods `HEAD`, `OPTIONS` and annotations `required, optional, primary, autoincrement, readonly, min, max, minLength, maxLength, regex` which the parser rejects (`unknown field annotation`, parser.go:679-681; `isHTTPMethod` parser.go:1145); parser accepts `serverSet` which the spec lacks. Validator treats `long, time, bytes, any, json` as primitives (validator.go:12-26) although CLAUDE.md's type table omits them, and a model named `json` or `any` is accepted and shadows the primitive (cases 53, 65).
- **Expected vs actual:** CLAUDE.md says internal/language is the single source of truth and CI checks generated plugin files against it; the plugins therefore advertise syntax the compiler rejects.
- **Impact:** editor completion/highlighting diverges from the compiler.
- **Root cause:** constants were never wired into lexer/parser/validator.
- **Recommendation:** refactor, make lexer/parser/validator derive keyword, type, method and annotation sets from `internal/language`, and delete `veld.go`/`imports.go`.
- **Regression check:** test that every spec annotation/method parses and every parser-accepted annotation appears in the spec.
- **Effort:** 6h-10h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** frontend-audit

### F-LSP-01 (P1): LSP ignores imports, so valid multi-file projects get false errors and no cross-file features

- **Category:** lsp-diagnostics
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/lsp/handler.go:261-273, 324-365; internal/lsp/server.go (no loader use)
- **Trigger / reproducer:** didOpen `proj/veld/modules/auth.veld` (valid per `veld validate`), which has `import @models/user` and `output: User`. See session.log.
- **Expected vs actual:** expected no diagnostics; actual `line 7: module "Auth", action "GetUser": undefined output type "User"`. Hover and definition on imported `User` return null; completion omits imported models.
- **Impact:** every file in a real project that uses an import shows a spurious error. The server is unusable beyond single-file toy contracts.
- **Root cause:** `computeDiagnostics` validates one buffer in isolation; `internal/loader` and config discovery are not used; `initialize` ignores rootUri.
- **Recommendation:** refactor — load via the loader from the nearest veld.config.json with open buffers overlaid on disk.
- **Regression check:** add an LSP test on a temp project where an importing file yields zero diagnostics and definition on an imported model returns the other file's URI.
- **Effort:** 16h–30h
- **Owner:** T5
- **Reviewer:** T4
- **Dependencies:** F-FE-05 structured diagnostics
- **Source track:** lsp-editors-audit

### F-LSP-11 (P1): VS Code extension re-implements the language service in TypeScript and does not use `veld lsp`

- **Category:** vscode
- **Evidence:** source-supported defect
- **User impact:** high
- **Security severity:** none
- **Location:** editors/vscode/src/extension.ts:81-1675; editors/vscode/package.json
- **Trigger / reproducer:** grep for `languageclient`, `child_process`, `spawn`, `veld lsp` in editors/vscode finds nothing.
- **Expected vs actual:** expected a thin client over the Go compiler; actual a 1,676-line parallel parser/validator that can disagree with the CLI.
- **Impact:** two (VS Code TS, Go LSP) plus a third (JetBrains Kotlin) implementations of Veld semantics; every language change must be made three times.
- **Root cause:** the extension predates or ignores the Go LSP; the LSP is too weak to adopt (F-LSP-01).
- **Recommendation:** rewrite — once the LSP is fixed, replace the TS service with a vscode-languageclient launcher.
- **Regression check:** an extension test that starts `veld lsp` and checks a diagnostic for a known-bad contract.
- **Effort:** 20h–40h (after F-LSP-01 to 10)
- **Owner:** T5
- **Reviewer:** T4
- **Dependencies:** F-LSP-01..10 server fixes
- **Source track:** lsp-editors-audit

### F-LSP-16 (P1): `internal/language` is not consumed by the compiler, so "single source of truth" is false and CI cannot detect parser/spec drift

- **Category:** language-metadata
- **Evidence:** source-supported defect
- **User impact:** medium
- **Security severity:** none
- **Location:** internal/language/*.go (imported only by cmd/generate-language/main.go); .github/workflows/ci.yml:38-67
- **Trigger / reproducer:** `grep -rl "internal/language\"" --include=*.go` returns only `cmd/generate-language/main.go`.
- **Expected vs actual:** CLAUDE.md calls it the single source of truth; actually lexer, parser, validator, LSP completion and format each hard-code their own keyword/directive lists. The CI drift job only proves the generated editor files equal `constants.go`. `go run ./cmd/generate-language` produced no diff at HEAD, but a parser change (e.g. a new directive) would pass CI without updating editors.
- **Impact:** the editors stay silently wrong when the language changes (F-LSP-13 is a live example).
- **Root cause:** spec is documentation-grade data, not wired into the parser.
- **Recommendation:** refactor — add a Go test asserting parser-accepted directives, keywords and types equal the language spec, and have the LSP completion read it.
- **Regression check:** that test, run in `go test ./...`.
- **Effort:** 4h–8h
- **Owner:** T5
- **Reviewer:** T4
- **Source track:** lsp-editors-audit

### F-MULTI-04 (P1): Python validators miss the `Decimal` import; any contract with a decimal field returns 500 on every request

- **Category:** emitter-python
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** internal/emitter/backend/python/validate.go:348 (emits isinstance(..., (str, Decimal)) with no import); output _validators.py:8
- **Trigger / reproducer:** GET /api/items/abc -> 500 {"error":"name 'Decimal' is not defined"}; fixed by adding `from decimal import Decimal`.
- **Expected vs actual:** 200 vs 500 (swallowed by blanket except).
- **Impact:** decimal is one of the eight core scalars.
- **Root cause:** validator header emits only `from typing import Any`.
- **Recommendation:** fix -- emit the import (or avoid Decimal in validators).
- **Regression check:** contract with a decimal field exercised in the Flask smoke test.
- **Effort:** 1h-2h
- **Owner:** T4
- **Reviewer:** T2
- **Dependencies:** F-MULTI-01; harness F-MULTI-02
- **Source track:** multi-emitter-audit

### F-MULTI-05 (P1): Python service contract is internally inconsistent (ABC returns model classes, routes require dicts; headers not extracted; query untyped; malformed JSON -> 500; enum unchecked; exception text leaked)

- **Category:** emitter-python
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** low
- **Location:** python/routes.go:145-150 and the catch-all except block, python/types.go, validate.go
- **Trigger / reproducer:** see Part A run log.
- **Expected vs actual:** typed Item/ItemQuery/headers reach the service; bad JSON -> 400; unknown enum -> 400 vs dict-only returns, `{}` headers, `{'limit':'5'}`, 500 on bad JSON, `role:"bogus"` accepted, `str(e)` returned for all non-API errors.
- **Impact:** the generated interface cannot be implemented as typed; auth headers (the point of `headers:`) are dropped; information disclosure via str(e) for unexpected exceptions.
- **Root cause:** validators and models developed independently; header extraction left as a comment.
- **Recommendation:** refactor -- return dicts or convert via to_dict, bind headers/query, restrict error bodies to known exception types.
- **Regression check:** Flask smoke test asserting headers present, typed query, 400 for bad JSON/enum, generic 500 body.
- **Effort:** 10h-18h
- **Owner:** T4
- **Reviewer:** T2
- **Dependencies:** F-MULTI-01; harness F-MULTI-02
- **Source track:** multi-emitter-audit

### F-MULTI-07 (P1): Rust backend cannot compile and has no working router or main

- **Category:** emitter-rust
- **Evidence:** source-supported defect
- **User impact:** critical
- **Security severity:** none
- **Location:** internal/emitter/backend/rust/routes.go:35,100; Cargo.toml template in rust/main.go; output src/items/mod.rs, src/models/items.rs
- **Trigger / reproducer:** read generated tree (cargo unavailable: environment limitation; verdict by inspection).
- **Expected vs actual:** Axum router vs `build_router() -> impl Any { items_router() }` returning `()`; Cargo.toml lacks axum; handlers lack `use axum::{extract::{State,Path},Json,http::StatusCode}; use std::sync::Arc`; handlers return the trait result whose type (`Result<T,Box<dyn Error>>`) differs from the declared handler return; `CreateItemHeaders::default()` without a Default derive; `item_default_role() -> Role { String::from("user") }`; hyphenated identifiers; extended structs drop parent fields (Item has no id/createdAt).
- **Impact:** not a usable backend; docs disagree (Actix vs Axum).
- **Root cause:** partially stubbed emitter whose tests lock in the stub.
- **Recommendation:** freeze -- mark experimental until a cargo CI job exists; fix docs/architecture/overview.md.
- **Regression check:** cargo check in CI.
- **Effort:** 30h-50h to make real
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** multi-emitter-audit

### F-MULTI-08 (P1): Java, C#, PHP backends emit framework-less classes that do not compile and do not route

- **Category:** emitter-java
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** java/types.go (enum default; package const at line 26), java/routes.go (deleteItem), csharp/types.go, csharp/routes.go, php/routes.go, php/types.go
- **Trigger / reproducer:** javac (stub Jackson): `Item.java:22 incompatible types: String cannot be converted to Role`; `ItemsController.java:94 missing return statement`. C#: `Role Role { get; set; } = "user";`, controller without ControllerBase/Mvc using/verb attributes (csproj lacks AspNetCore). PHP: `Role $role = 'user'` fatal, `array $request` then `->validate()`, required params after optional. C#/PHP read only (no dotnet/php).
- **Expected vs actual:** working Spring/ASP.NET/Laravel integration vs plain POJOs; Java output package is the author's `maayn.veld.generated`, not configurable.
- **Impact:** three backends advertised as Spring Boot / ASP.NET Core / Laravel are neither compilable nor routable.
- **Root cause:** default-value emission treats enum default as a string literal; route emitters are framework-neutral but docs/tests claim otherwise.
- **Recommendation:** freeze -- keep model+interface output, drop controller/route claims until compile CI exists.
- **Regression check:** javac in CI now; dotnet/php in docker.
- **Effort:** 20h-35h each to reach a working state
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** multi-emitter-audit

### F-MULTI-09 (P1): Dart/Kotlin/Swift clients have compile or runtime defects beyond the header bug

- **Category:** emitter-swift
- **Evidence:** source-supported defect
- **User impact:** high
- **Security severity:** none
- **Location:** frontend/swift/client.go (`${id}` interpolation), frontend/kotlin/helpers.go:20 + client.go:107, frontend/dart/types.go (fromJson)
- **Trigger / reproducer:** generated APIClient.swift line 85 `"/api/items/${id}"`; Kotlin `val price: java.math.BigDecimal` inside @Serializable class (no serializer: compile error) and "query params omitted for brevity"; Dart `role: json['role']`, `tags: json['tags']`, `children: json['children']` (runtime cast errors); Swift types internal and Decimal cannot decode a JSON string "1.10". Toolchains unavailable: judged by reading.
- **Expected vs actual:** working SDKs vs path params literally sent as "${id}" (Swift), query/header params silently dropped (Kotlin/Swift/Dart), decimal broken.
- **Impact:** the three mobile SDKs are unusable for any non-trivial contract.
- **Root cause:** string-template emitters never compiled or run.
- **Recommendation:** freeze -- experimental label; fix after compile CI exists.
- **Regression check:** kotlinc / swift build / dart analyze jobs.
- **Effort:** 10h-16h per SDK
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** multi-emitter-audit

### F-MULTI-10 (P1): Service SDKs: Python SDK rejects its own model types; TS SDK fails strict tsc; Go SDK lacks query support

- **Category:** service-sdk
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** none
- **Location:** python/sdk.go (body/query serialisation), node/sdk.go (shared index scalar imports), go/sdk.go
- **Trigger / reproducer:** Part D: `create_item(CreateItem(...))` TypeError; `get_item("a", ItemQuery(limit=3))` AttributeError; `tsc --strict` 24 errors in sdk/shared/index.ts (UUID/ISODateTime/ISODate/Decimal undeclared), TS1263, TS2352.
- **Expected vs actual:** typed clients usable with their own models vs only raw dicts work (Python); compile errors in TS.
- **Impact:** inter-service SDK, a headline feature, needs manual patching. Positives: env var precedence (arg > VELD_<NAME>_URL > baked) verified for Python, Go, TS; WS actions skipped; error type VeldApiError; deps and deps --validate work.
- **Root cause:** SDK templates reuse model emitters whose output is not exercised.
- **Recommendation:** fix -- add to_dict conversion, import scalars in shared module, add query param to Go.
- **Regression check:** SDK smoke tests against a local fake server (as in ws/t.py).
- **Effort:** 8h-14h
- **Owner:** T4
- **Reviewer:** T2
- **Dependencies:** tier-1 emitter fixes
- **Source track:** multi-emitter-audit

### F-MULTI-11 (P1): Dockerfile and CI generators download a release asset that returns 404, and bundle an unrelated registry Postgres

- **Category:** gen-dockerfile
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** medium
- **Location:** internal/generators/dockerfile/dockerfile.go (download line, compose template), internal/generators/cicd/cicd.go
- **Trigger / reproducer:** HTTP HEAD https://github.com/Adhamzineldin/Veld/releases/latest/download/veld-linux-amd64 -> 404 (assets are .tar.gz). Generated docker-compose.yml starts `postgres:16-alpine` named "registry", POSTGRES_PASSWORD=changeme, 5432:5432 published.
- **Expected vs actual:** working build vs both the Dockerfile download and the CI "Install Veld" step fail; compose exposes a default-credential DB on the host.
- **Impact:** generated pipelines fail on first run; unpinned, unverified binary download into the build (supply chain); image runs as root; `CMD node index.js` and `/app/dist` need user-created files; Python/Go projects get Node templates unless marker files are present in --out.
- **Root cause:** copy-paste from registry compose; release naming not tested.
- **Recommendation:** fix -- pin version and checksum, correct asset name, remove DB service, add non-root USER.
- **Regression check:** `docker build` against examples/ in CI.
- **Effort:** 4h-8h
- **Owner:** T4
- **Reviewer:** T2
- **Source track:** multi-emitter-audit

### F-NODE-08 (P1): No input validation unless `--validate`; documented Zod/schemas do not exist

- **Category:** emitter-node
- **Evidence:** documentation mismatch
- **User impact:** medium
- **Security severity:** medium
- **Location:** backend/node/routes.go:105-110, CLAUDE.md "Route handler features"/"Generated Output Structure"
- **Trigger / reproducer:** POST `{"i":"notanumber"}` on default build -> 201, service receives it.
- **Expected vs actual:** "Zod .parse(); ZodError -> 400" vs raw req.body.
- **Impact:** services trust unvalidated input by default.
- **Root cause:** validation moved to opt-in zero-dep validators; docs stale.
- **Recommendation:** refactor: make validation default-on (or document opt-in) and update CLAUDE.md.
- **Regression check:** default generate emits _validators.ts; docs aligned.
- **Effort:** 2h-4h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** F-MULTI-01; harness F-NODE-19
- **Source track:** node-ts-audit

### F-NODE-09 (P1): `--validate` validators are shallow

- **Category:** emitter-node
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Location:** backend/node/validate.go:110-300
- **Trigger / reproducer:** accepted: role:'hacker' (enum), tags:[1,2], meta:{a:1}, i:1.5, i:9007199254740993, bad uuid/date/decimal, maybe:null, unknown extra keys passed through. @default not applied; missing required headers not checked; GET query values are strings typed as numbers; `?page=abc` becomes a 500 via output assertion. Recursion has no depth cap (deep JSON bounded only by body-size limit; not tested, suspected risk).
- **Expected vs actual:** runtime checks matching the TS types vs typeof-only checks.
- **Impact:** contract holes; prototype-ish keys in `meta` pass through.
- **Root cause:** validators check typeof only.
- **Recommendation:** refactor: validate enums, element types, integers, formats, optional/null, coerce query.
- **Regression check:** table-driven bad values per scalar.
- **Effort:** 8h-14h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** F-MULTI-01; harness F-NODE-19
- **Source track:** node-ts-audit

### F-NODE-10 (P1): Error handler leaks internal messages on 500 and mixes error formats

- **Category:** emitter-node
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Location:** backend/node/routes.go:185-191
- **Trigger / reproducer:** service throws Error('db password=... at /srv/x') -> 500 body has the message. `err.status` from any thrown object is trusted. Malformed JSON bypasses it (Express HTML page). The identical catch block is repeated per action.
- **Expected vs actual:** generic 500 body vs raw message.
- **Impact:** information disclosure; inconsistent error contract.
- **Root cause:** catch copies err.message regardless of status.
- **Recommendation:** fix: generic message for status >= 500 and one shared handleError.
- **Regression check:** 500 body is constant.
- **Effort:** 2h-3h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** F-MULTI-01; harness F-NODE-19
- **Source track:** node-ts-audit

### F-NODE-11 (P1): Path parameters are not URL-encoded in the SDK

- **Category:** emitter-ts
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Location:** internal/emitter/helpers.go:25 (ToTemplateLiteral), frontend/typescript/client.go (urlExpr)
- **Trigger / reproducer:** `getItem('a/b?x=1#h')` requested /items/a/b (404); '../admin' would traverse.
- **Expected vs actual:** encodeURIComponent(id) vs raw interpolation.
- **Impact:** IDs with reserved chars break; client-side path injection when IDs come from user input. Angular has the same pattern.
- **Root cause:** `${id}` emitted directly.
- **Recommendation:** fix: emit `${encodeURIComponent(id)}`.
- **Regression check:** SDK test with ids containing / ? # %.
- **Effort:** 1h-2h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** harness F-NODE-19
- **Source track:** node-ts-audit

### F-NODE-15 (P1): Angular emitter output is invalid and incomplete

- **Category:** emitter-angular
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** none
- **Location:** frontend/angular/emitter.go (models ~L110, service generation)
- **Trigger / reproducer:** generate --frontend=angular with the rich contract: models/index.ts contains `X-Trace-Id: string;` (TS1131/TS1128/TS1005); references UUID/Decimal/ISODate which are not defined or imported there; `secure(headers)` accepts headers but never passes them to http.get; baseUrl hard-coded ''; no VeldApiError; path params unencoded.
- **Expected vs actual:** working service vs compile failure and silently dropped headers.
- **Impact:** Angular target unusable with headers or branded scalars.
- **Root cause:** independent reimplementation that shares neither the TS SDK nor type emitter.
- **Recommendation:** refactor: reuse the TS type emitter and quote non-identifier keys.
- **Regression check:** compile Angular output against @angular/core@17 in CI.
- **Effort:** 6h-10h
- **Owner:** T2
- **Reviewer:** T1
- **Source track:** node-ts-audit

### F-NODE-17 (P1): Writes are non-atomic, no manifest, stale files handled by "delete everything"

- **Category:** output-safety
- **Evidence:** source-supported defect
- **User impact:** medium
- **Security severity:** low
- **Location:** emitters call os.WriteFile directly (backend/node/routes.go, frontend/typescript/emitter.go:65); no manifest anywhere
- **Trigger / reproducer:** a failure midway (read-only-dir simulation not run) leaves a partial tree; with F-NODE-01 the prior good tree is already gone.
- **Expected vs actual:** temp+rename; manifest-based cleanup.
- **Impact:** torn output after crashes or Ctrl-C.
- **Root cause:** no output abstraction (codegen package exists but node/ts do not use it).
- **Recommendation:** refactor: route all writes through a staged writer.
- **Regression check:** inject write failure; old tree intact.
- **Effort:** 6h-10h (overlaps F-NODE-01/03)
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** ADR-007 staged writer
- **Source track:** node-ts-audit

### F-NODE-19 (P1): Tests never compile or run the generated TypeScript

- **Category:** emitter-node
- **Evidence:** source-supported defect
- **User impact:** medium
- **Security severity:** none
- **Location:** internal/emitter/**/*_test.go
- **Trigger / reproducer:** no `tsc`/`exec.Command`/node in any emitter test; 210 `func Test` total (node 10, typescript 6, react 5), all substring assertions on text.
- **Expected vs actual:** compile tests would have caught F-NODE-06 and 15.
- **Impact:** regressions in generated code ship unnoticed.
- **Root cause:** string-contains test style.
- **Recommendation:** fix: add golden files plus a `tsc --strict` smoke job per target.
- **Regression check:** the CI job described.
- **Effort:** 6h-10h
- **Owner:** T2
- **Reviewer:** T1
- **Dependencies:** F-MULTI-01; harness F-NODE-19
- **Source track:** node-ts-audit

### F-REG-04 (P1): Org admins can demote or remove the sole owner (org takeover)

- **Category:** server-authz
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** medium
- **Exploitability:** org member with role `admin` (granted by an owner).
- **Location:** internal/server/handlers/orgs.go:108-155 (AddMember allows an admin to set another member's role; db.go:296-303 `ON CONFLICT DO UPDATE SET role`), orgs.go:157-193 (last-owner guard only inside `if caller.Role == "owner"`, line 180)
- **Trigger / reproducer:** E6. bob(admin) `POST /orgs/acme/members {"username":"alice","role":"member"}` -> 200, alice's role is now `member`; with alice restored, bob `DELETE /orgs/acme/members/alice` -> 204, `owners left: 0`.
- **Expected vs actual:** admins must not modify owners; org must keep >=1 owner; actual: orphaned org nobody can administer.
- **Impact:** privilege escalation/lock-out inside an org; orphaned orgs cannot publish new owners (all role changes need owner/admin).
- **Root cause:** role checks look only at caller role and requested role, not target's current role.
- **Recommendation:** fix - disallow admins touching owners and enforce last-owner invariant for every caller in a transaction.
- **Regression check:** unit tests per (caller role x target role x action).
- **Effort:** 2h-3h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004; server tests
- **Source track:** registry-audit

### F-REG-05 (P1): API token `expires_at` is never enforced

- **Category:** server-auth
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** holder of an expired token.
- **Location:** internal/server/db/db.go:535-557 (GetTokenByHash returns expired rows), middleware.go:89-98 (no check)
- **Trigger / reproducer:** E9. Token created with `expires_days:1`; DB row set to `now()-1 day`; `GET /auth/me` -> HTTP 200.
- **Expected vs actual:** 401; actual: valid forever.
- **Impact:** advertised expiry is a false guarantee.
- **Root cause:** field stored, never read.
- **Recommendation:** fix - add `expires_at IS NULL OR expires_at > now()` to the lookup.
- **Regression check:** test with a past `expires_at`.
- **Effort:** 0.5h-1h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004 split; server tests F-CLI-23
- **Source track:** registry-audit

### F-REG-06 (P1): Private-package visibility is inconsistent across endpoints (latent leak) and unreachable from the API

- **Category:** server-authz
- **Evidence:** reproduced defect (with `visibility` set by SQL; the API cannot set it)
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** anonymous (versions, org page) and any registered non-member (package list).
- **Location:** packages.go:25-43 (any authed user gets `public`+`private`), 80-96 (ListVersions no check), orgs.go:80-103 + db.go:428 (all packages of an org listed to anonymous), packages.go:57,242 (membership of any role suffices)
- **Trigger / reproducer:** E11 with acme/auth set private: anon `/packages` hides it; mallory (non-member) `/packages` LISTS it; anon `/orgs/acme` lists it and members' emails; anon `/packages/acme/auth/versions` -> 200 with full version rows including `tarball_key` and sha; anon download 401 vs non-member 403 vs absent 404 reveals existence.
- **Expected vs actual:** a private package must be invisible (404) to non-members on every endpoint; actual: 4 different leak paths. Because Publish always creates `public` packages and no endpoint changes visibility, the feature is currently unusable and the leaks are latent - until someone adds a toggle.
- **Impact:** the "private package" claim would be false; private contract names/versions/manifests exposed.
- **Root cause:** per-handler ad-hoc checks, no shared `canRead(pkg,user)`.
- **Recommendation:** refactor - one authorization helper used by every package/version/org read path, plus an explicit visibility API.
- **Regression check:** matrix test (anon/non-member/member x every GET).
- **Effort:** 4h-8h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004; server tests
- **Source track:** registry-audit

### F-REG-07 (P1): Concurrent/duplicate publish deletes the published tarball (DB row left pointing at nothing)

- **Category:** server-storage
- **Evidence:** reproduced defect
- **User impact:** high
- **Security severity:** low
- **Exploitability:** any org owner/admin (or two honest CI jobs racing).
- **Location:** packages.go:162-166 (check-then-act), 177-200 (Put renames over the existing key), 216-218 (on unique-violation: `Storage.Delete(tarballKey)` removes the winner's file)
- **Trigger / reproducer:** E5. Two concurrent publishes of the same new version: in 10/25 (run 1) and 16/25 (run 2) iterations one got 201, the other 500, and the subsequent download returned `HTTP 404 file not found in storage` although the DB row exists and `GET /versions` lists it.
- **Expected vs actual:** loser gets 409 and the winner's artifact is intact; actual: published version silently corrupted/unavailable.
- **Impact:** data loss of a "published, immutable" artifact; also leaves a pkg row created before the tarball is validated (name squatting with zero versions).
- **Root cause:** file written under its final key before the DB uniqueness decision; unconditional delete in the error path.
- **Recommendation:** fix - write to a unique temp key, insert row inside a transaction, then `link`/rename only on success (or content-address blobs by sha).
- **Regression check:** parallel-publish test (N=20) asserts exactly one 201 and a downloadable artifact.
- **Effort:** 3h-5h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** F-REG-02 fix
- **Source track:** registry-audit

### F-REG-08 (P1): Versions are not immutable and "latest" is not the highest semver

- **Category:** server-db
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** org owner (delete + republish); a malicious/compromised owner or stolen write+delete token.
- **Location:** packages.go:311-338 (hard delete), 162 (only existing rows block); db.go:~470 (`ORDER BY published_at DESC`), cmd/veld/registry.go:199-209 (`versions[0]`), models.go:ParseSemver (unused by the CLI, tolerant parsing)
- **Trigger / reproducer:** E3. Deleted 0.9.0 and republished 0.9.0 with different content -> 201 (new sha). Published 2.0.0 then 1.0.1: list order `[1.0.1 2.0.0 0.9.0 1.0.0]`, so `veld registry pull @acme/auth` resolves to 1.0.1. Deprecated versions are also eligible for "latest".
- **Expected vs actual:** npm/crates semantics (a version, once published, is never reusable; latest = highest non-prerelease); actual: pinned versions can change content; "latest" is "most recently published".
- **Impact:** supply-chain integrity: consumers who pinned `@org/pkg@1.0.0` get different bytes after delete+republish, and no consumer-side lock/sha exists to notice. No semver ranges are supported (only exact or latest).
- **Root cause:** no tombstones, no semver validation, ordering by timestamp.
- **Recommendation:** fix - tombstone deleted versions (name+version permanently reserved), validate semver on publish, compute latest by semver, record sha in a consumer lockfile.
- **Regression check:** delete-then-republish must 409; ordering test.
- **Effort:** 4h-8h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004; versioned migrations
- **Source track:** registry-audit

### F-REG-12 (P1): No rate limiting, lockout, or TOTP replay protection

- **Category:** server-auth
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** anonymous.
- **Location:** server.go:59 (no limiter in the handler chain); auth.go:88-159; totp.go:28-38
- **Trigger / reproducer:** E7/E8: 60 wrong passwords in 4.5 s, all 401, no 429; 300 consecutive wrong TOTP guesses, zero 429; the same valid TOTP code accepted twice (`first -> HTTP 200, second use of same code -> HTTP 200`); with no limiter, a 6-digit code (3 valid values per 10^6 across the +-1 window) is brute-forceable online; unmeasured, only the absence of any throttle was reproduced (and F-REG-01 makes the second factor moot anyway).
- **Impact:** online password/TOTP guessing and credential stuffing; registration spam (open, unverified).
- **Root cause:** absent middleware.
- **Recommendation:** fix - per-IP and per-account limiter with backoff, one-time TOTP counter storage.
- **Regression check:** burst test expects 429; replay test expects 401.
- **Effort:** 4h-6h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004 split; server tests F-CLI-23
- **Source track:** registry-audit

### F-REG-13 (P1): No body-size limits and no server-side archive validation on publish

- **Category:** server-storage
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** any org owner/admin (publish); anonymous for JSON endpoints.
- **Location:** packages.go:111 (`ParseMultipartForm(32<<20)` is only the in-memory threshold, the comment says "32 MB max"), 169-200 (streams any bytes to disk, no size cap, no gzip/tar check); auth.go:39 etc. (`json.NewDecoder(r.Body)` unbounded); server.go:59-62
- **Trigger / reproducer:** E13: 200 MiB of non-archive bytes published as a version -> 201 (size 209,715,200 recorded), taking 2.5 s; 60 MiB JSON login body processed (401). Multipart spill also writes a temp copy in `$TMP`, doubling disk use. A client-side `Publish` buffers the full tarball in memory (client.go:148-159).
- **Impact:** disk exhaustion, memory pressure; registry can host arbitrary blobs; invalid archives are served to consumers who then hit F-REG-10.
- **Root cause:** `ParseMultipartForm` misread as a limit; no validation stage.
- **Recommendation:** fix - `http.MaxBytesReader` (global and per route), validate the tarball with the hardened extractor before accepting, per-user quotas.
- **Regression check:** oversize and non-tar uploads -> 413/400.
- **Effort:** 3h-5h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** F-REG-02 fix
- **Source track:** registry-audit

### F-REG-14 (P1): Sessions cannot be revoked; no purpose/typ on JWTs; 7-day lifetime

- **Category:** server-auth
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** anyone holding a stolen JWT or cookie.
- **Location:** auth.go:162-171 (Logout only clears cookie), token.go:56-92, middleware.go:70-100
- **Trigger / reproducer:** E10: JWT used after `POST /auth/logout` -> `/auth/me` HTTP 200. No password-change/reset endpoint exists at all (no way to invalidate sessions or recover an account; no password reset flow).
- **Impact:** stolen session valid until expiry (7 days); no account recovery.
- **Root cause:** stateless JWTs only.
- **Recommendation:** refactor - server-side session table (or token-version column) checked in middleware; shorter TTL.
- **Regression check:** logout/password-change invalidates old tokens.
- **Effort:** 4h-8h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004 split; server tests F-CLI-23
- **Source track:** registry-audit

### F-REG-15 (P1): Transport/browser hardening absent (no TLS, cookie not Secure, CORS `*`, no security headers)

- **Category:** server-web
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** network attacker where deployed without a TLS terminator; any website for CORS (no credentials, so limited).
- **Location:** server.go:56-66 (`ListenAndServe`, no TLS option), auth.go:372-381, server.go:150-163
- **Trigger / reproducer:** E14: `Set-Cookie veld_session ... HttpOnly=true Secure=false SameSite=Lax`; preflight `ACAO="*"`; `CSP="" XFO="" XCTO="" HSTS=""`. serve.go prints `http://localhost<addr>` as the URL.
- **Impact:** session cookie and tokens in cleartext on http deployments; clickjacking; the SPA can be framed.
- **Root cause:** server assumes a reverse proxy but nothing documents or enforces it.
- **Recommendation:** fix - document proxy requirement, add `Secure` when `base_url` is https, add headers + CSP, restrict CORS to configured origins.
- **Regression check:** header assertions in a handler test.
- **Effort:** 2h-4h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-004
- **Source track:** registry-audit

### F-REG-16 (P1): Registry selection is nondeterministic and credentials are keyed by raw string (dependency-confusion exposure)

- **Category:** credentials
- **Evidence:** reproduced defect
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** user who has logged in to more than one registry (or two spellings of one).
- **Location:** internal/registry/credentials.go:259-268 (`DefaultRegistry` returns the first key of a Go map), :230-236 (exact-string key), client.go:31-43; used by `pull` (registry.go:187-196), `versions`, `token create`
- **Trigger / reproducer:** three entries (`http://127.0.0.1:18080`, `http://localhost:18080`, `http://localhost:18080/`); 12 runs of `veld registry info` printed 9x/2x/1x different "default" registries.
- **Expected vs actual:** deterministic resolution from project config (`registry.url`) or an explicit default; actual: pull of `@org/name` without `--registry` may resolve against a random logged-in registry, and `push` uses `rc.Registry.URL` only if set, else the random default. `veld.config.json` has no per-scope registry mapping (as in `.npmrc` `@scope:registry=`), so a name in a public registry can shadow a private one; there is no way to pin a scope to a registry. The token for registry X is only sent to X (client.go builds the URL from the chosen BaseURL), so no cross-registry token leak was found.
- **Impact:** wrong-registry resolution; dependency confusion for private scopes.
- **Root cause:** credential store doubles as configuration; no scope->registry mapping.
- **Recommendation:** refactor - explicit `defaultRegistry` plus `scopes` mapping, normalised URL keys.
- **Regression check:** multi-registry test with a fixed order.
- **Effort:** 3h-6h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-008
- **Source track:** registry-audit

### F-REG-20 (P1): Pull writes untrusted files into the project and can overwrite existing files; pulled `veld.config.json` can carry a hook

- **Category:** client
- **Evidence:** suspected risk (the write was reproduced via MITM; the hook executing was not run)
- **User impact:** medium
- **Security severity:** medium
- **Exploitability:** malicious package author/registry; requires the user to pass `--out` pointing at the project (e.g. `--out veld`) or otherwise to use the pulled config.
- **Location:** cmd/veld/registry.go:211-216,238 (default out `veld/packages/@org/name`, `--out` unrestricted, `os.Create` overwrite); `hooks.postGenerate` in config (CLAUDE.md config section)
- **Trigger / reproducer:** MITM proxy output wrote `veld.config.json {"input":"app.veld","hooks":{"postGenerate":"echo PWNED > pwned.txt"}}` into `veld\packages\@org\hello\`. With the default location the config is not auto-detected (CLAUDE.md lists only `./veld.config.json` and `./veld/veld.config.json`); with `--out veld` it would overwrite the real project config.
- **Impact:** code execution on next `veld generate` in that misuse; and any `.veld` content is imported into code generation.
- **Root cause:** packages ship config files; extractor overwrites silently.
- **Recommendation:** fix - exclude `veld.config.json` from packages (or ignore it), refuse overwrite without `--force`, refuse `--out` that equals/contains the project root.
- **Regression check:** pull test with a config entry.
- **Effort:** 2h-3h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** ADR-008
- **Source track:** registry-audit

### F-REG-21 (P1): Operational gaps: no health endpoint, SIGTERM unhandled, no schema versioning, no TLS, no backups, zero tests

- **Category:** ops
- **Evidence:** reproduced defect (health, migrations table, restart), source-supported (signals)
- **User impact:** medium
- **Security severity:** low
- **Exploitability:** operator.
- **Location:** server.go:84-140 (catch-all route serves SPA for `/healthz`: REPRODUCED 200 text/html 84 KB), serve.go:138 (`os.Interrupt` only), db.go:43-128 (no `schema_migrations`; REPRODUCED table list), storage.go (local FS only; DB row and file are not reconciled), `find internal/server internal/registry -name '*_test.go'` -> none, CLAUDE.md "Registry Server" section (no backup/restore, no HA)
- **Trigger / reproducer:** section 4. Restart re-ran the schema silently and data persisted (good); but a column change in the future cannot be versioned, and `ADD COLUMN IF NOT EXISTS` migrations run on every start by whichever instance starts first.
- **Impact:** orchestrators cannot probe readiness; `docker stop` kills in-flight uploads; upgrades are risky; no test net for any finding above.
- **Root cause:** prototype-level server.
- **Recommendation:** refactor - `/healthz` + `/readyz`, SIGTERM, versioned migrations, tests, documented backup of PG + blob store.
- **Regression check:** container smoke test with `docker stop`.
- **Effort:** 6h-12h
- **Owner:** T3
- **Reviewer:** T1
- **Dependencies:** chapter 13 compose
- **Source track:** registry-audit


## Backlog (csv/backlog.csv)


| ID | Priority | Outcome | Acceptance criteria | Effort | Owner | Reviewer | Depends on | Milestone | Closes |
|---|---|---|---|---|---|---|---|---|---|
| B-001 | P0 | Repository hygiene: remove tracked binary, scratch dirs, .idea, docx plans, stale vscode out/, test.md, duplicate web index | `git ls-files \| xargs du -c` under 5 MB; CI green; .gitignore covers them | 1-2h | T5 | T4 |  | M0 | F-CLI-21 |
| B-002 | P0 | Go toolchain policy: go.mod `go 1.26`, CI pinned to 1.26/1.27 | CI builds on 1.26 and 1.27; `go vet` clean | 1h | T5 | T1 |  | M0 | ADR-001 |
| B-003 | P1 | Governance files: LICENSE (MIT), CODEOWNERS, CHANGELOG, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, issue/PR templates | files present; CODEOWNERS matches ch.16 matrix; nuspec license link valid | 4-8h | T5 | D1 |  | M0 | D-10 |
| B-004 | P0 | ADR-001..010 reviewed and accepted or amended by the team; skills survey completed | each ADR has status accepted and deciders; survey results recorded | 6-10h (team) | T1 | all |  | M0 |  |
| B-005 | P0 | Conformance fixture skeleton: `internal/spec/testdata` layout, runner, 5 initial fixtures | `go test ./internal/spec` runs fixtures with expected diagnostics | 6-10h | T1 | T4 |  | M0 |  |
| B-006 | P0 | Staged output writer with manifest; no RemoveAll; swap on success; unmanaged-file refusal | output-safety tests: user file preserved; failing compile keeps previous output; manifest lists all files | 12-18h | T2 | T1 | B-005 | M1 | F-NODE-01,F-NODE-03,F-NODE-17,F-CLI-01,F-CLI-03 |
| B-007 | P0 | Gate ordering: compile -> diff -> gate -> emit; lock never deleted by generate/watch | removing an action makes `generate --strict` exit 4; lock survives failed runs | 3-6h | T1 | T2 | B-006 | M1 | F-NODE-04,F-FE-02 |
| B-008 | P0 | Output path safety: refuse out equal to or containing config dir, input dir, cwd, home, root; print resolved paths | `--out=../veld` refused with message; workspace out validated | 3-5h | T2 | T5 | B-006 | M1 | F-NODE-02,F-CLI-02 |
| B-009 | P0 | `--dry-run` writes nothing; `clean` deletes manifest files only; `init` never overwrites (O_EXCL) | before/after listings identical for dry-run; clean refuses unmanaged non-empty dir; init exits 1 on existing files | 4-7h | T2 | T5 | B-006 | M1 | F-CLI-05,F-CLI-06 |
| B-010 | P0 | Lexer: wildcard import token; unterminated string/comment errors; BOM accepted; string escapes | `import @models/*` loads the directory; unterminated string is an error; fixtures added | 4-8h | T1 | T5 |  | M1 | F-FE-01,F-FE-04,F-FE-15,F-FE-24 |
| B-011 | P0 | Header identifier derivation (wire name kept, identifier derived per language via lang helpers) | `Idempotency-Key` compiles in Node, Python, Go outputs; fixture with headers in all three | 6-12h | T1 | T4 | B-010 | M1 | F-MULTI-01 |
| B-012 | P0 | Determinism: sort moduleExports and every map iteration in emitters; CI determinism job | six consecutive generations byte-identical on ubuntu and windows | 2-4h | T2 | T4 |  | M1 | F-LEAD-01,D-02 |
| B-013 | P0 | Python emitter: route syntax fix, route registration, Decimal import, dict/model consistency, malformed JSON -> 400 | `compileall` clean; Flask round trip in CI: 201/400/204; decimal request succeeds | 10-16h | T4 | T2 | B-011 | M1 | F-MULTI-03,F-MULTI-04,F-MULTI-05,F-CLI-20 |
| B-014 | P0 | Go emitter: `mux` fix, date vs datetime, query/header binding, required-field validation | `go build` and `go vet` clean; round trip in CI; `{}` rejected with 400 | 12-20h | T4 | T1 | B-011 | M1 | F-MULTI-06,F-CLI-19 |
| B-015 | P0 | Node emitter: static-before-param route ordering (IR-level), generic 500 message, path param encodeURIComponent, strict tsc fixes | `tsc --strict` clean; guarded route unreachable test passes; `a/b?x` path param round trip | 6-12h | T2 | T5 |  | M1 | F-NODE-05,F-NODE-06,F-NODE-10,F-NODE-11 |
| B-016 | P0 | Generated-code compile jobs in CI for node-ts, python, go on every fixture; OS matrix ubuntu/windows/macos | CI red when any tier-1 output fails to compile | 8-12h | T5 | T4 | B-013,B-014,B-015 | M1 | F-MULTI-02,F-NODE-19,F-CLI-18 |
| B-017 | P0 | E2E harness v0: reference service implementations + generated clients; Express, Flask, Go servers; cross-language matrix | Journey A runs in CI; status codes, error shape, encoding asserted | 24-36h | T4 | T2 | B-016 | M1-M2 | F-MULTI-02 |
| B-018 | P0 | Registry client: checksum always required; lockfile hash wins; https default with explicit override; deterministic registry selection; Pack filtering | MITM tests (stripped header, replaced body) fail the pull; `http://` requires flag; Pack excludes dotfiles/cache | 8-14h | T3 | T1 |  | M1 | F-REG-09,F-REG-16,F-REG-18 |
| B-019 | P0 | Hardened Unpack: entry allow-list, name validation incl. ADS, size/count/ratio limits, temp dir + rename, all errors checked | malicious-archive test suite (traversal, symlink, bomb, 100k entries, truncated) passes; partial extraction leaves nothing | 6-10h | T3 | T1 |  | M1 | F-REG-10 |
| B-020 | P0 | Registry auth: typed JWT purpose claim; partial tokens rejected by middleware; scopes enforced via one helper; expiry; last-owner guard | authz matrix tests: partial token 401 on /auth/me; read-only token cannot mint or delete; expired token 401 | 10-18h | T3 | T1 |  | M1-M2 | F-REG-01,F-REG-03,F-REG-04,F-REG-05 |
| B-021 | P0 | Registry publish: manifest validation, content-addressed storage keys, atomic insert, race-safe cleanup, body and archive limits, idempotent retry | traversal name rejected 400; 25 concurrent publishes leave exactly one consistent version; 200 MiB upload rejected 413 | 12-20h | T3 | T2 | B-019 | M2 | F-REG-02,F-REG-07,F-REG-13 |
| B-022 | P0 | Registry UI escaping rewrite + CSP + security headers + Secure cookie + CORS allow-list | stored payloads render inert (browser test); headers present | 8-14h | T3 | T5 |  | M2 | F-REG-11,F-REG-15 |
| B-023 | P1 | Structured diagnostics (`diag`): codes, spans (offset,line,col,len), all errors, text+json formatters; parser error recovery | three syntax errors in one file reported together with correct lines; `--json` output schema documented | 16-28h | T1 | T5 | B-005 | M2 | F-FE-05,F-FE-06,F-FE-28,F-LSP-04 |
| B-024 | P1 | TypeRef tree; nested generics; reserved-word field/enum names; directive duplicates/order | `string[][]`, `Map<string,T[]>` fixtures pass; `date` usable as a field name | 12-24h | T1 | T2 | B-023 | M2 | F-FE-07,F-FE-08,F-FE-23 |
| B-025 | P0 | IR v1: types, lowering, JSON Schema, `veld ir`; 30 conformance fixtures with expected IR | every fixture's IR validates against the schema; schema published; `veld ir` documented | 40-60h | T1 | T4 | B-023,B-024 | M2 | ADR-002 |
| B-026 | P1 | Tier-1 emitters consume IR (via adapter first); validation default ON with deep validators (enum, element types, int, formats, optional vs null, query coercion) | round-trip error cases pass; `--no-validate` documented | 24-40h | T2 | T4 | B-025 | M2 | F-NODE-08,F-NODE-09 |
| B-027 | P1 | Tier-3 removal PR (rust, angular, dart, kotlin, swift, mock, dockerfile, cicd, envconfig, incremental cache); README matrix rewritten to tiers | targets absent from binary; docs and website updated; `veld doctor` shows tiers | 5-8h | T2 | T4 | B-004 | M2 | ADR-005,F-MULTI-07,F-MULTI-11,F-MULTI-14,F-FE-21 |
| B-028 | P2 | Validator extensions: path params vs fields, @default completeness, union members, extends overrides, reserved model names, workspace cycles, loader strictness, aliases in validate/ast | fixtures for each rule; missing import is an error | 16-27h | T1 | T3 | B-025 | M2 | F-FE-09,F-FE-10,F-FE-11,F-FE-12,F-FE-13,F-FE-14,F-FE-27,F-NODE-14 |
| B-029 | P1 | `internal/language` consumed by lexer/parser/LSP; parser-vs-spec conformance test; drift job uses porcelain | test fails when a directive is added to one side only | 6-10h | T5 | T1 |  | M2 | F-FE-22,F-LSP-16,F-LSP-17 |
| B-030 | P1 | LSP on compiler API: overlay buffers, import resolution, all diagnostics with ranges, UTF-16 conversion, MethodNotFound/ParseError, shutdown/exit conformance, recover, debounce | scripted multi-file session passes; hover after emoji correct; 2 MB doc does not block hover | 30-50h | T5 | T1 | B-023,B-025 | M2 | F-LSP-01,F-LSP-02,F-LSP-03,F-LSP-06,F-LSP-07,F-LSP-08 |
| B-031 | P1 | Registry split binary `veld-registry`; versioned migrations; /healthz /readyz; SIGTERM; JSON logs; config check | CLI binary no longer links lib/pq; migrations table present; readiness returns 503 when DB down | 10-16h | T3 | T5 |  | M2 | ADR-004,F-REG-21 |
| B-032 | P1 | Registry visibility helper; private default; uniform 404; no member emails to anonymous; rate limiting; TOTP replay counter; session revocation | authz matrix green incl. anonymous probes; 60 wrong logins -> 429 | 12-20h | T3 | T1 | B-020 | M2 | F-REG-06,F-REG-12,F-REG-14,F-REG-23 |
| B-033 | P1 | Registry server test suite with PostgreSQL service in CI (handlers, authz matrix, races, limits, e2e publish/pull) | coverage of internal/registry (server) above 60%; CI job green | 16-30h | T3 | T4 | B-031 | M2 | F-CLI-23 |
| B-034 | P1 | Config: nested wins; unknown keys warn; `--out` overrides; JSON errors with line numbers; hooks via platform shell, failure exits non-zero, --no-hooks | tests for precedence; Windows hook runs via cmd /C | 7-12h | T5 | T1 |  | M2 | F-CLI-07,F-CLI-08,F-CLI-09,F-CLI-10 |
| B-035 | P1 | GoReleaser: archives, checksums, cosign, SBOM, attestation, windows-arm64, brew tap, docker image; version from build info | release from a tag produces all artifacts; checksum verified in smoke test | 12-20h | T5 | T3 |  | M2-M4 | F-CLI-13,F-CLI-15,F-CLI-17 |
| B-036 | P0 | VEP v1 specification: request/response JSON Schema, manifest, capability vocabulary, error codes; ADR-003 accepted with schema | schema in repo with validation tests; frozen at M3 exit | 10-16h | T2 | T3 | B-025 | M3 | ADR-003 |
| B-037 | P0 | Plugin host runner: --describe handshake, --emit exchange, timeout, size cap, path mediation into the staged writer, stub plugin tests | stub plugin tests: handshake, escape refusal, timeout kill, capability warning | 24-36h | T2 | T3 | B-036,B-006 | M3-M4 |  |
| B-038 | P1 | Package manifest `veld.package.json` and lockfile format finalized (ADR-008 accepted); server metadata endpoint with sha256 and dependencies | schema documented; server returns per-version sha256 | 6-10h | T3 | T1 | B-021 | M3 | ADR-008 |
| B-039 | P0 | TypeScript emitter SDK (`@veld/emitter-sdk`): IR types from schema, defineEmitter, test harness, fixtures | hello emitter builds and passes harness; SDK published to a scoped test package | 24-40h | T4 | T5 | B-036 | M4 |  |
| B-040 | P0 | Hello emitter + `npm create veld-emitter` scaffold; acceptance test on windows and linux from a clean project | chapter 10 §10.7 checks pass in CI | 8-16h | T4 | T2 | B-039,B-037 | M4 |  |
| B-041 | P2 | Extract docs generator as external plugin `veld-emitter-docs` | same output as built-in on all fixtures; installed from npm | 16-30h | T4 | T2 | B-040 | M4 | ch.09 §9.8 |
| B-042 | P1 | VS Code thin client: languageclient launcher, generated grammar/snippets, commands wired, binary discovery + checksum-verified download, integration test, lockfile, out/ untracked | integration test green; hand-written service deleted; offline fallback message | 20-40h | T5 | T2 | B-030 | M4 | F-LSP-11,F-LSP-12,F-LSP-13,F-LSP-14 |
| B-043 | P2 | LSP features: context-aware completion incl. imported models, document symbols, formatting via fmt, references (if time) | completion after `input:` lists models only; symbols test | 12-22h | T5 | T4 | B-030 | M4 | F-LSP-09,F-LSP-10 |
| B-044 | P1 | Package resolution: lockfile write/read, resolver (exact, caret, x-range), cache, offline, scoped registries, `veld install --frozen` | two machines produce byte-identical veld/packages from the lockfile; scope-to-registry binding prevents fallback | 36-56h | T3 | T1 | B-038 | M4 | ADR-008 |
| B-045 | P1 | Wrappers: npm (pinned version, checksum, loud failure, proxy), pip (3.10+, download on first run), brew via GoReleaser; experimental wrappers moved | clean-machine install smoke tests for npm, brew, archive on three OSes | 12-20h | T5 | T3 | B-035 | M4 | F-CLI-16,F-CLI-17 |
| B-046 | P2 | CLI UX: `--json` on validate/generate/lint/diff/doctor/version, exit-code taxonomy, NO_COLOR, doctor registered, registry aliases | exit codes documented and tested; `veld doctor` lists plugins and tiers | 10-16h | T5 | T2 |  | M4 | F-CLI-04,F-CLI-11,F-CLI-14 |
| B-047 | P1 | Diff on IR: classifier covers enums, nested models, headers, middleware, unions, status; cyclic extends safe; lock versioned | fixture pairs for each breaking class; `veld diff --json` | 10-18h | T1 | T2 | B-025 | M4 | F-FE-18,F-FE-19 |
| B-048 | P2 | OpenAPI completeness: servers, security placeholder, enum defaults, error schema aligned, redocly lint clean; 3.1 output if cheap | redocly lint zero errors; fixture snapshot | 4-8h | T2 | T1 | B-025 | M4 | F-MULTI-13 |
| B-049 | P2 | Tier-2 nightly compile job (vue, svelte, javascript, typesonly, java, csharp, php) + header fix verification; SQL/Prisma/GraphQL quoting fixes | nightly job green or target demoted with README update | 10-16h | T4 | T5 | B-016 | M4 | F-MULTI-08,F-MULTI-12,F-NODE-15 |
| B-050 | P1 | Formatter rewrite on lossless tokens; idempotence test; `fmt --check` | every fixture formats idempotently; comments preserved; `model }` no panic | 12-20h | T1 | T5 | B-023 | M5 | F-FE-03 |
| B-051 | P1 | Self-hosting: docker-compose with caddy, config check, backup/restore/verify commands, runbook, timed restore drill on a clean VM | drill: RTO under 1 h, verify zero mismatches, client install --frozen succeeds against restored registry | 16-24h | T3 | T4 | B-031 | M5 | ch.13 §13.4 |
| B-052 | P1 | Evaluation harness: expressiveness corpus (Q1), drift seeds (Q2), validation benchmark (Q3), editor parity scenarios (Q5) | scripts reproduce results from raw data; results folder with dates and commit | 24-36h | T4 | T1 | B-017 | M5 | ch.03 §3.9 |
| B-053 | P1 | User study: protocol, consent, recruitment (6-10), sessions, analysis | sessions completed; data anonymized; limitations recorded | 16-24h (T4) + D1 | T4 | T3 | B-052 | M5 |  |
| B-054 | P1 | Docs and website: support matrix generated from CI; tutorials for Journeys A, B, C that compile; plugin author guide; operator runbook; website lockfile, lint, CI | docs CI extracts and validates examples; no claim without CI evidence | D1 + 6-10h technical | D1 | per ch.16 | B-027 | M5 | D-01..D-26,F-CLI-22 |
| B-055 | P3 | JetBrains: strip annotator/completion, add LSP4IJ definition, scheduled Gradle build | diagnostics from LSP shown in IDEA; nightly build green | 20-40h | T5 | T1 | B-030 | post-Beta (reserve) | F-LSP-15 |
| B-056 | P1 | Security self-review (ASVS L1), govulncheck, npm audit, SECURITY.md process test | checklist complete, zero open security P0/P1 | 8-12h | T3 | T1 | B-033 | M6 | ch.14 |
| B-057 | P0 | Release candidate and 1.0.0: signed artifacts, changelog, release notes, smoke tests, demo VM and video | readiness gate criteria (ch.14 §14.5) all green | 12-20h | T5 | T1 | B-035,B-045 | M6-M7 |  |
| B-059 | P1 | Q1 per-field constraints: @min/@max/@minLength/@maxLength/@pattern/@format syntax, IR `constraints`, validators in TS/Python/Go, OpenAPI mapping, lint for unsatisfiable combos | fixtures with boundary values pass in all three validators; OpenAPI shows constraints; RE2-subset regex agreed | 30-50h | T1 | T2 | B-025 | M4 (S8-S9) | ADR-012 Q1 |
| B-060 | P1 | Q2 database schema generator: @entity/@id/@unique/@index/@table/@relation, PostgreSQL+SQLite dialects, Prisma, quoted identifiers and defaults, --migrations diff | schema applies cleanly to PostgreSQL and SQLite in CI; DTOs without @entity produce no table; `DEFAULT 'user'` quoted | 30-60h | T4 | T1 | B-025 | M4 (S8-S9) | ADR-012 Q2, F-MULTI-12 |
| B-061 | P1 | Q3 auth schemes in the contract: bearer/apiKey/basic/oauth2 syntax, OpenAPI securitySchemes, adapter wiring with typed AuthContext, client auth config | round trip with a bearer-protected action in Node, Python, Go; OpenAPI lint clean; Veld issues no tokens | 30-60h | T1 | T2 | B-059 | M4 (S9-S10) | ADR-012 Q3 |
| B-062 | P1 | Q4 discriminated unions: `type X = A \| B @discriminator(kind)`, IR union, TS/Python/Go emission, OpenAPI oneOf+discriminator, validators | fixtures round-trip in all three stacks; undiscriminated union is a validator error | 40-70h | T1 | T4 | B-059 | M4-M5 (S10-S11) | ADR-012 Q4, F-FE-11 |
| B-063 | P2 | Q5 streaming: SSE outputs, chunked file download, multipart upload with @maxSize; server generators and client async iterators | SSE and upload round trips in Node, Python, Go; uploads stream to temp files | 40-80h | T2 | T4 | B-025 | M5 (S11-S12) | ADR-012 Q5 |
| B-064 | P2 | Q6 WebSocket actions for Node: typed messages both directions, per-message validation, `ws` adapter (declared exception), browser client | typed echo round trip in CI; capability manifest declares the exception | 40-70h | T2 | T1 | B-063 | M5 (S11-S12) | ADR-012 Q6 |
| B-065 | P2 | Q7 WebSocket for Python and Go plus clients | typed echo round trip for both stacks; cross-language WS matrix | 40-80h | T4 | T2 | B-064 | M5-M6 (S12-S13) | ADR-012 Q7 |
| B-066 | P2 | Q8 Dockerfile/CI/env generators fixed: pinned version with checksum, no bundled registry Postgres, templates per tier-1 stack, nightly image build with health check, `tools` config honoured | nightly job builds each image and hits /health; no 404 download | 15-30h | T5 | T2 | B-016 | M5 (S11) | ADR-012 Q8, F-MULTI-11, D-04 |
| B-067 | P2 | Q9 OpenAPI import: `veld import openapi` producing .veld plus an unsupported-constructs report; import(export(x)) == x on fixtures | round trip holds for every conformance fixture; report lists each unsupported construct with the JSON path | 40-80h | T1 | D1 | B-061,B-062 | M5-M6 (S12-S13) | ADR-012 Q9 |
| B-068 | P2 | Q10 package signing: Sigstore keyless or minisign, registry stores and verifies signatures, lockfile `signed` requirement enforced on pull | tampered signature fails pull; unsigned package pulls only when lockfile allows | 30-60h | T3 | T5 | B-044 | M5 (S11-S12) | ADR-012 Q10 |
| B-069 | P2 | Q11 WebAssembly plugin transport: Extism on wazero, no AllowedPaths, no network, same JSON exchange, hello emitter in TinyGo and Rust PDK, doctor shows transport | Wasm hello emits identical output to stdio hello; sandbox test proves no file or network access; latency measured | 60-120h | T2 | T4 | B-037,B-040 | M5-M6 (S12-S13) | ADR-012 Q11 |
| B-070 | P3 | Q12 registry-hosted plugins: kind=plugin packages, `veld plugin install`, signature verification, trust policy (Wasm default, --trust-native) | install from the local registry runs the Wasm plugin; native plugin refused without the flag | 40-80h | T3 | T2 | B-068,B-069 | M6 (S13) | ADR-012 Q12 |
| B-071 | P3 | Q13a tier-2 revival: Rust backend real axum router and main, cargo check nightly; mock server wired to `veld mock` | cargo check green nightly; `veld mock` serves OpenAPI examples for every fixture | 50-70h | T4 | T5 | B-049 | M5-M6 (S12-S13) | ADR-012 Q13, F-MULTI-07, F-MULTI-14 |
| B-072 | P3 | Q13b tier-2 revival: Angular on the TS SDK; Dart, Kotlin, Swift client fixes; nightly compile with their toolchains | nightly compile green for all four | 30-60h | T5 | T4 | B-049 | M5-M6 (S12-S13) | ADR-012 Q13, F-NODE-15, F-MULTI-09 |
| B-073 | P3 | Q14 high availability: two registry replicas behind Caddy, PostgreSQL replica, S3-compatible storage implementation (MinIO), failover drill | drill: kill one replica and the primary DB, service continues; documented RPO/RTO | 40-70h | T3 | T5 | B-051 | M6 (S13) | ADR-012 Q14 |
| B-074 | P0 | IR v1 reserved extension fields (constraints, union, auth, transport, stream, entity) present in the schema from the first release | schema validates fixtures with and without the fields; no IR major bump needed for Q1-Q7 | 4-8h | T1 | T2 | B-025 | M2 (S5) | ADR-012 |
| B-075 | P1 | AGENTS.md enforcement in CI: PR template with definition-of-done checklist, golangci-lint config, coverage floors per package, import-boundary and determinism jobs required, backlog-ID check on PR titles | a PR without a backlog ID or with a coverage drop fails CI | 12-20h | T5 | T1 | B-016 | M1 (S1-S2) | ADR-012, AGENTS.md |
| B-076 | P0 | Extended-feature freeze review (2027-05-02): apply the automatic cut order of chapter 20 §20.6 to unaccepted queue items; update release notes and README matrix | every queue item is either accepted or listed as future work; no partially merged queue code remains on master | 4h (team) | T1 | all | B-059..B-073 | May 2, 2027 |  |
| B-077 | P1 | AI-productivity measurement: actual hours per backlog item recorded from S1; estimate-to-actual ratio reported at M1 exit and each gate; multiplier corrected | ratio published at Nov 29 with the decision it triggers under chapter 20 §20.6 | 2h per sprint | integration owner | T1 |  | M1 onward | ADR-012, RQ6 |
| B-058 | P0 | Final report, slides, demo script, defense rehearsals | report submitted; two rehearsals held | D1 + 10h per T | D1 | T1 | all | M6-M7 |  |


## Sprint allocation (csv/sprint-allocation.csv)


| Sprint | Start | End | Milestone | T1 | T2 | T3 | T4 | T5 | D1 | Checkpoint |
|---|---|---|---|---|---|---|---|---|---|---|
| S0 | 2026-10-21 | 2026-11-01 | M0 decisions | ADR workshop; fixture skeleton | writer design; reproduce F-NODE-01..04 | tarball reproduction; unpack design | reproduce Python/Go failures; harness layout | hygiene PR; Go 1.26; governance files | survey; ADR editing; notes system | Oct 30: ADRs accepted; hygiene merged (T1) |
| S1 | 2026-11-02 | 2026-11-15 | M1 slice | wildcard import, unterminated strings, BOM, header identifiers | staged writer, manifest, gate ordering, path safety, dry-run, clean | checksum-required pull, Pack filtering, DefaultRegistry, archive tests | Python/Go compile fixes, compile jobs, harness v0 | CI compile jobs, windows runner, config precedence | tutorial A, diagram D1 | Nov 13: --strict gate demo; compile status in CI (T2) |
| S2 | 2026-11-16 | 2026-11-29 | M1 slice | loader strictness, aliases | e2e Express round trip; tier-3 removal PR | authz matrix tests; PostgreSQL CI job | Flask and Go round trips; determinism test | GoReleaser skeleton; version from build info; doctor | README tiers draft; chapter 1 draft | Nov 27: Journey A in CI on two OSes; Nov 29 migration decision (T3) |
| S3 | 2026-11-30 | 2026-12-13 | M2 core | diag, spans, error recovery | emitters on diag; OutputFile API | registry split binary; atomic publish | validator extensions; 15 fixtures | LSP structured diagnostics; recover; MethodNotFound | spec draft; diagram D2 | Dec 11: all-errors output; IR v0 for one fixture (T4) |
| S4 | 2026-12-14 | 2026-12-27 | M2 core (75%) | TypeRef, nested generics | IR lowering v0 | size limits; versioned migrations | fixtures to 30 | LSP imports with overlay; UTF-16 | safe-generation page; chapter 2 draft | Dec 23: LSP multi-file session passes (T5) |
| S5 | 2026-12-28 | 2027-01-10 | M2 exit (65%) | IR v1 + schema + veld ir; diff on IR | tier-1 emitters on IR | registry e2e in CI | validation default ON | compiler API in LSP; CI OS matrix | chapter 3 draft | Jan 8: M2 exit review; IR validates all fixtures (T1) |
| S6 | 2027-01-11 | 2027-01-24 | M3 freeze (30%) | review only | VEP schema draft | ADR-008 design | fixture review | GoReleaser checksums | plugin guide outline | Jan 22: VEP schema reviewed (T2) |
| S7 | 2027-01-25 | 2027-02-07 | M3 freeze (70%) | gate tests | host runner skeleton; stub plugin tests | lockfile format; resolver design doc | SDK type generation from schema | VS Code client skeleton | chapter 4 draft | Feb 5: stub plugin handshake, escape refusal, timeout (T3) |
| S8 | 2027-02-08 | 2027-02-21 | M4 extend + queue start | validator extensions; Q1 constraints syntax/IR/validator | host runner complete; --describe; capabilities | manifest, lockfile, resolver | TypeScript SDK; test harness; Q2 schema generator start | VS Code client; binary discovery; CI for queue items | plugin author guide | Feb 19: SDK runs hello against host locally; Q1 fixtures merged (T4) |
| S9 | 2027-02-22 | 2027-03-07 | M4 extend + queue | Q1 constraints done; Q3 auth syntax | docs emitter extraction; Q1 TS validators; Q3 SDK auth config | cache, offline, scoped registries | hello emitter on npm; npm create; Q2 schema generator done; Q1 Python/Go validators | completion, symbols, formatting in LSP; Q3 OpenAPI security | chapter 5 draft | Mar 5: veld install --frozen reproducible; Q1 and Q2 accepted (T5) |
| S10 | 2027-03-08 | 2027-03-21 | M4 Alpha + queue | diff classifier complete; Q4 unions syntax/IR | OpenAPI completeness; Q3 middleware wiring; Q4 TS emission | registry authz hardening leftovers | extracted docs emitter acceptance; Q3 Python/Go wiring | --json, exit codes, wrappers fixed | chapter 6 draft; study protocol | Mar 19 Alpha gate: plugin acceptance on two OSes; Q3 accepted (T1) |
| S11 | 2027-03-22 | 2027-04-04 | M5 Beta prep (93%) + queue | formatter rewrite; Q4 unions done; Q5/Q6 syntax | Q5 streaming Node; Q6 WebSocket Node start | compose deployment; backup/restore drill; Q10 signing start | evaluation Q1, Q2 runs; Q4 Python/Go unions | website CI; docs site; Q8 Docker/CI/env generators | study recruitment; runbook editing | Apr 2: restore drill timed; Q4 accepted (T2) |
| S12 | 2027-04-05 | 2027-04-18 | M5 Beta (82%) core freeze + queue | core bug fixes; Q9 OpenAPI import start | Q6 WebSocket Node done; Q11 Wasm transport start | runbook final; security self-review; Q10 signing done | user study sessions; evaluation Q3; Q7 WS Python/Go start; Q13a Rust start | JetBrains LSP4IJ if reserve; Q13b Angular/mobile clients | study sessions; chapters 7 and 8 | Apr 16 core feature freeze; Q5, Q6, Q8, Q10 accepted (T3) |
| S13 | 2027-04-19 | 2027-05-02 | M6 harden + queue end | core P0/P1 fixes; Q9 OpenAPI import done | core P0/P1 fixes; Q11 Wasm transport done | core P0/P1 fixes; Q12 hosted plugins; Q14 HA drill | evaluation analysis; Q7 done; Q13a Rust/mock done | signed release pipeline; Q13b done; nightly matrix complete | chapter 9; evaluation chapter | Apr 30: demo rehearsal 1; May 2 extended freeze and automatic cut review (T4) |
| S14 | 2027-05-03 | 2027-05-16 | M6 RC | report technical review | RC fixes | RC fixes | RC fixes | RC tagged | report draft complete | May 14 RC gate (T5) |
| S15 | 2027-05-17 | 2027-05-30 | M7 final (55%) | demo lead | fixes only | demo registry | fixes only | release 1.0.0 | final report, slides, video | May 28 readiness review; May 31 gate (T1) |


# Part V: Appendices



## Appendix: External facts (accessed 2026-10-21)

Method: `WebFetch` was blocked by a PreToolUse hook for the whole session, so no page was read in full. Facts come from `WebSearch` result summaries that quote the cited pages. Each item is marked **primary** (the summary quoted the primary page), **secondary**, or **unverified** (prior knowledge only). Re-verify primary items by opening the URL before citing them in the graduation report. The first pass was delegated to a research track; the lead reviewer repeated the decision-relevant searches with domain restrictions.

### A.1 TypeSpec (Microsoft)

- TypeSpec 1.0 is GA; `@typespec/openapi3` and `@typespec/json-schema` stable; protobuf and client/server emitters preview. typespec.io/blog/typespec-1-0-GA-release (primary).
- Emitters are TypeScript npm packages built on `@typespec/compiler`; emitter framework moved to `@typespec/asset-emitter` (April 2025). typespec.io/docs/extending-typespec/emitters-basics (primary).
- Server emitters: `@typespec/http-server-csharp` 0.58.0-alpha.33 released on GitHub (2026); `http-server-js` exists (documentation PR merged 2026-10-16). A "Prepare typespec publish" PR moves packages to 1.17.0 / 0.87.0. github.com/microsoft/typespec/releases, PRs #12264, #12250, #12134 (primary, version numbers from release and PR titles).
- License: repository page lists MIT. github.com/microsoft/typespec (primary for the repo; npm package license not confirmed).
- VS Code / Visual Studio extensions exist (prior knowledge, unverified in-session).

### A.2 Smithy (AWS)

- Apache-2.0 license. github.com/smithy-lang/smithy (primary).
- Generators: smithy-typescript (Apache-2.0, "in development"), smithy-go (Apache-2.0), smithy-kotlin, smithy-python, smithy-java (1.0.0 GA April 2026 per a secondary blog), smithy-rs (server generator work in progress per awesome-smithy). github.com/smithy-lang, github.com/smithy-lang/awesome-smithy (primary and secondary as noted).
- smithy-language-server and smithy-intellij exist. github.com/awslabs/smithy-language-server, github.com/awslabs/smithy-intellij (primary).
- Distribution via Maven Central (smithy-codegen-core 1.72.1, July 2026). mvnrepository.com (secondary).

### A.3 OpenAPI and generators

- OpenAPI 3.2.0 released September 2025 with a 3.1.2 patch; adds QUERY method, streaming media types, tag hierarchy. spec.openapis.org/oas/v3.2 (primary via summary).
- openapi-generator: latest stable v7.26.0 tagged 2026-10-06; JVM (Java 11+); generates clients, server stubs, docs. github.com/OpenAPITools/openapi-generator/tags (primary).
- oapi-codegen: Apache-2.0, Go clients and servers. openapi-typescript: MIT, types only. Kiota: MIT, clients. openapi-typescript-codegen: MIT, clients. Hey API openapi-ts: MIT, SDKs plus Zod schemas. github.com pages for each (primary). orval, openapi-fetch licenses not found (unverified).
- Fern (Postman acquisition January 2026) and Stainless acquisition claims are vendor or single-source statements (unverified).

### A.4 Protocol Buffers, buf, Connect

- Plugin protocol: compiler sends a `CodeGeneratorRequest` on stdin, plugin writes a `CodeGeneratorResponse` on stdout, compiler writes files. buf.build/docs/generate (primary).
- buf local plugins: `plugins.local` executable on PATH (`protoc-gen-<name>`); remote plugins pinned as `buf.build/<org>/<plugin>:<version>` and executed on the BSR; both mixable in `buf.gen.yaml`. buf.build/docs/generate, buf.build/docs/bsr/remote-plugins (primary).
- `plugin.proto`: github.com/protocolbuffers/protobuf/blob/main/src/google/protobuf/compiler/plugin.proto (unverified URL, prior knowledge).
- Connect RPC languages: Go, TypeScript/JavaScript, Swift, Kotlin, Python; CNCF project. connectrpc.com (primary).

### A.5 TypeScript-only type-sharing tools

tRPC, ts-rest, Hono RPC, zod-openapi, Zodios, Elysia Eden: TypeScript-only, types inferred from code, no multi-language generation (zod-openapi exports OpenAPI). Unverified in-session (prior knowledge).

### A.6 GraphQL Code Generator

Plugin-based codegen from GraphQL schemas and operations, many languages. the-guild.dev/graphql/codegen. Unverified in-session.

### A.7 Plugin technologies for a Go host

- Extism: Go SDK runs on wazero; manifest `AllowedPaths` maps host directories into the guest (requires `EnableWasi: true`); network access is not restricted by the SDK itself, hosts implement allow-lists; a downstream project pins `github.com/extism/go-sdk v1.7.1`. github.com/extism/go-sdk, extism.org/docs/concepts/host-functions (primary; version from a downstream pin, secondary).
- wazero: pure Go, no cgo, 1.0 since March 2023 with API stability promise. github.com/wazero/wazero (primary).
- WASI 0.3.0 released 2026-06-11. wasi.dev/releases/wasi-p3 (primary).
- HashiCorp go-plugin: separate OS process over gRPC, process isolation only (unverified version).

### A.8 Language Server Protocol and JetBrains

- LSP 3.18 is the current specification (site navigation: 3.18 current, 3.19 upcoming); changelog dates 3.18.0 to 2026-06-04 (date format inferred). microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification (primary).
- `positionEncoding` introduced in 3.17; client offers `general.positionEncodings`, server selects; UTF-16 mandatory default. 3.17 specification page (primary).
- JetBrains: LSP API available to all IntelliJ IDEA users via the unified distribution (2025.3); Community Edition sunset; the LSP client API is being open-sourced in the 2026.2 cycle (planned for 2026.1.4 stable). blog.jetbrains.com/platform/2025/09/..., blog.jetbrains.com/platform/2026/06/open-sourcing-the-lsp-client-api-in-intellij-idea-2026-2/ (primary).
- LSP4IJ (Red Hat): stable 0.21.0, nightly 0.21.1-20261020; listed as compatible with "all flavours of IntelliJ"; minimum 2024.2+. plugins.jetbrains.com/plugin/23257-lsp4ij, github.com/redhat-developer/lsp4ij/releases (primary; the GitHub releases page is stale at 0.19.3).

### A.9 Go and GoReleaser

- Go 1.26 released 2026-02-10; Go 1.27 released 2026-08-19; patch releases go1.27.2 (2026-10-08) and go1.26.10 (2026-10-19). Policy: each major release supported until two newer major releases exist. go.dev/doc/devel/release (primary). Therefore on 2026-10-21 supported: 1.26 and 1.27. Veld's `go 1.22` is out of support.
- GoReleaser v2.18: Syft SBOMs, cosign signing (cosign v3 `--bundle`), Homebrew, Scoop, Winget publishing; npm publishing Pro-only. goreleaser.com (primary via summary).

### A.10 Supply chain

- SLSA v1.2 current; Build track levels L1 to L3. slsa.dev (primary via summary).
- GitHub artifact attestations: `actions/attest-build-provenance` v4; needs `id-token: write` and `attestations: write`; free for public repositories. docs.github.com (primary via summary).
- cosign, npm provenance, OpenSSF Scorecard: unverified in-session.

### A.11 Licenses

MIT, Apache-2.0 and BSD-3-Clause are permissive and mutually compatible; Apache-2.0 requires preserving LICENSE and NOTICE on redistribution. Veld's dependencies: cobra (Apache-2.0), lib/pq (MIT), golang.org/x/crypto (BSD-3-Clause). SPDX pages: spdx.org/licenses/MIT.html, Apache-2.0.html, BSD-3-Clause.html (unverified in-session; standard knowledge).

### A.12 Who already covers server interfaces and route wiring

TypeSpec (preview C# and JS server emitters), Smithy (TypeScript developer-preview server, Java GA, Rust WIP), openapi-generator (server stubs for many frameworks), oapi-codegen (Go servers), Connect/buf (RPC server stubs). Veld's breadth across eight backend emitters is wider on paper, but chapter 05 shows two of them emitting uncompilable code at 6a47f81, so the claim must be earned through the compile matrix in chapter 09.



## Appendix: audit-track notes (A-docs-claims)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Veld documentation-claims audit (commit 6a47f81, reviewed 2026-10-21)

Method: read docs, then verified against source and by building `veld` from HEAD into the scratchpad
(`veldbin.exe`, not the tracked `veld`) and running `veld init` + `veld generate` in a scratch project
(`scratchpad/proj`). No tracked file modified. Status words: verified / partially true / mismatch / unverifiable.
Abbreviations: CL = CLAUDE.md; OV = docs/architecture/overview.md; GP = docs/GRADUATION_PROPOSAL.md;
PP = docs/PROJECT_PROPOSAL.md; DG = docs/DEVELOPER_GUIDE.md; GS = docs/guides/getting-started.md;
T = test.md; EXEC v1..v4 = docs/EXECUTION_PLAN_v*.md. Note: docs/README.md and overview.md and changelog-v0.1.md
are short; where I cite a section name instead of a line, the file has no stable line anchor for that item.

### 0. Ground truth from the code (the yardstick)

- Registered backends (8): csharp, go, java, node-js, node-ts, php, python, rust (`internal/emitter/backend/*/main.go` `RegisterBackend`; blank imports `cmd/veld/main.go:10-17`).
- Registered frontends (10): angular, dart, javascript, kotlin, react, svelte, swift, typescript, types-only, vue (`internal/emitter/frontend/*/emitter.go`; `cmd/veld/main.go:18-27`).
- Registered tool emitters (6): cicd, database, dockerfile, env, openapi, scaffold-tests (`internal/generators/*`). They run only when passed as `--backend=<tool>` (verified: `--backend=dockerfile` wrote Dockerfile, .dockerignore, docker-compose.yml). `--backend` help lists them (generate.go:1097).
- Aliases (`internal/config/config.go:252-279`): backend node, node-typescript -> node-ts; js, javascript, node-javascript -> node-js. Frontend flutter -> dart, ts -> typescript, hooks/react-hooks -> react, js -> javascript. `"target": "node-ts"` is the canonical name and is accepted.
- Framework strategies present: node (express, plain), javascript (express, plain), python (flask, fastapi, plain), go (chi, gin, plain), rust (axum, plain), java (spring, plain), csharp (aspnet, plain), php (laravel, plain).
- Top-level commands actually registered (`cmd/veld/main.go:88-102`): init, generate, watch, clean, validate, setup, ci, lint, diff, deps, ast (hidden), fmt (hidden), openapi, graphql, schema, docs, export, registry, lsp, completion. `login/logout/push/pull/serve` exist ONLY as `veld registry <name>` (`cmd/veld/registry.go:373`). `newDoctorCmd` is defined (`cmd/veld/tooling.go:195`, Hidden) but never added to the root; `veld doctor` -> "unknown command". Binary run confirmed for login, logout, push, pull, serve, doctor.
- go.mod (`go.mod:1-13`): module `github.com/Adhamzineldin/Veld`, go 1.22; direct deps cobra v1.8.0, lib/pq v1.10.9, x/crypto v0.24.0.
- `var Version = "0.1.0"` (`cmd/veld/main.go:37`), overridden by release ldflags (`.github/workflows/release.yml:304`). 47 git tags, latest v0.3.24.
- Language spec (`internal/language/constants.go:44-100`): HTTP methods include HEAD, OPTIONS, WS; builtin types string,int,long,float,decimal,bool,date,datetime,time,uuid,bytes,json,any; special List, Map; annotations include primary, autoincrement, readonly, min, max, minLength, maxLength, example, relation.
- Empirical generation (default `veld init` project, node-ts + typescript): backend output = `index.ts, package.json, README.md, interfaces/, routes/, types/ (+_scalars.ts, index.ts), middleware/IMiddleware.ts`. NO `schemas/schemas.ts`, NO Zod (`grep -rni zod internal/emitter/backend/node` -> only a comment at routes.go:137). `--validate` instead emits `_validators.ts` (zero-dep). Frontend went to `./veld/generated/client/` (not `generated/client/`). Default route handlers do no input validation (`service.createUser(req.body)`). Python output dirs: `models/`, `interfaces/`, `routes/`, `middleware/` (no `types/`, no `schemas/`).
- Determinism test: 6 consecutive `veld generate --force` runs on identical input produced 4 distinct output hashes. Cause: `types/index.ts` export-line ordering varies (`export type { LoginInput, RegisterInput, AuthToken }` vs `{ AuthToken, LoginInput, RegisterInput }`) because `internal/emitter/tsshared/types.go:438-443` iterates `for typeName, owner := range claimed` (Go map, unsorted).

### 1. Claims inventory

| # | Claim | Source | Category | Verification | Status | Evidence |
|---|---|---|---|---|---|---|
| 1 | 8 backends: node-ts,node-js,python,go,rust,java,csharp,php | README.md:215-224; GP:34; vscode README:129 | support matrix | registry grep | verified | cmd/veld/main.go:10-17 |
| 2 | 10 frontends incl. types-only, none | README.md:239-251 | support matrix | registry grep | verified | cmd/veld/main.go:18-27 |
| 3 | 18 targets (8+10) + 6 auxiliary generators | GP:34,213 | support matrix | count | verified | registries; internal/generators/* |
| 4 | Rust backend is "Actix" | README.md:221; OV Backend table; website Features.tsx:26 | feature | read emitter | mismatch | Rust emitter is Axum: backend/rust/main.go:2, errors.go:1, strategy/axum.go; no actix |
| 5 | PHP backend is "PSR-15" | README.md:224 | feature | read emitter | mismatch | Laravel: backend/php/main.go:18, strategy/laravel.go |
| 6 | Node backend emits Zod schemas `schemas/schemas.ts`; Zod validation in routes; ZodError->400 | CL:9,81,444-446,467-468; README.md:32,272,292; OV; changelog-v0.1 items 1,6; GS:63-68; T:849,928-950; GP:694; website DocsPage.tsx:1608,1758 | feature | generated + inspected | mismatch | no zod in node emitter; schemas/ not produced; default routes pass req.body through; `--validate` -> `_validators.ts` |
| 7 | Python backend emits `types/` and `schemas/schemas.py` (Pydantic) | CL:456-460; T:854; changelog items 8-9 | feature | generated + inspected | mismatch | python output has models/ interfaces/ routes/ middleware/; python/routes.go:55 imports `..schemas.schemas` which nothing writes (GP C3) |
| 8 | Zero runtime dependencies in generated output (by default) | CL:9; README.md:37,417; GP App.B | quality | inspected node-ts (+ --validate) | partially true | TS/Node path has no third-party imports; other stacks emit chi/gin/Spring/ASP.NET/Laravel/Axum/Flask/Pydantic unless `plain` strategy |
| 9 | Framework agnostic, `router: any` | CL:11; README.md:413,418 | quality | read generated routes | verified (node-ts) | `usersRouter(router: any, service)` in generated routes |
| 10 | Deterministic output | CL:508; README.md:419; OV Key Rules 4; GP 14.3; T:11 | quality | 6 runs, hash compare | mismatch | 4 hashes/6 runs; tsshared/types.go:438-443 |
| 11 | Veld never writes outside `--out` | CL Hard Rules; OV Key Rules 5; T:840 | quality | read code | partially true | setup edits tsconfig/package.json (internal/setup/setup.go:362-515); lock beside config (generate.go:209); `hooks.postGenerate` runs shell (generate.go:24-28); frontend default dir is `veld/generated` not `--out` |
| 12 | `go install github.com/Adhamzineldin/Veld/cmd/veld@latest` | README.md:56 | install | go.mod | verified | go.mod:1 |
| 13 | Go module `github.com/veld-dev/veld` | OV "Go Module" | architecture | go.mod | mismatch | go.mod:1 |
| 14 | "Only external dependency: cobra" | OV "Go Module" | architecture | go.mod | mismatch | go.mod:5-8 (+lib/pq, x/crypto) |
| 15 | Deps are cobra, x/crypto, modernc.org/sqlite | DG:87-90 | architecture | go.mod | mismatch | no sqlite; CL says no SQLite |
| 16 | Three direct third-party deps | GP:354, App.B | architecture | go.mod | verified | go.mod:5-8 |
| 17 | Top-level `veld login/logout/push/pull/serve/doctor` | CL:52-58,392-399; GP 14.1; DG:381 | feature | built binary | mismatch | registry.go:373; doctor unregistered (tooling.go:195, main.go:88-102) |
| 18 | `veld ast`, `veld fmt` exist | CL, README.md:336 | feature | binary | verified (hidden from `--help`) | contract.go:64,601 |
| 19 | README CLI table and generate flags (--dry-run, --incremental, --strict, --validate, --setup, --force) | README.md:318-354 | feature | binary | verified | main.go:88-102; generate.go:1097-1119 |
| 20 | Config fields backendConfig/frontendConfig/hooks/workspace/aliases/baseUrl | CL:176-260 | feature | config.go | verified | config.go:22-80 |
| 21 | `tools: {...}` config toggles auxiliary generators | CL:199-202,238 | feature | grep | mismatch | `ToolsConfig` parsed (config.go:44-55,76) never read anywhere in cmd/ internal/ |
| 22 | `backendConfig.target` "node-ts" accepted; `--backend=node` accepted | CL:14,182 | feature | config.go | verified | config.go:252-259 |
| 23 | "7+ backend languages and 4+ frontend targets" | T:17; T:1210-1235 tables (7 backends, 4 frontends) | support matrix | count | partially true (understated; table omits node-js, 6 frontends) | registries |
| 24 | "13+ target stacks" vs "8+ backend, 10+ frontend" | website Hero.tsx:24; DocsPage.tsx:236 | support matrix | count | verified but inconsistent with each other and with 18 | |
| 25 | "~14 languages/frameworks" | PP:5 | support matrix | count | mismatch vs GP:34 (18) | |
| 26 | `react` is alias of `typescript` frontend | T:1229 | support matrix | alias table | mismatch | react/emitter.go:18 own emitter |
| 27 | Only TypeScript frontend; no node-js backend | OV Frontend/Backend tables | support matrix | registry | mismatch | 10 frontends, 8 backends |
| 28 | GitLab CI generation | README.md:35,233; website SupportedStacks.tsx:33 | feature | `grep -rli gitlab internal cmd` | mismatch | zero hits; cicd emits GitHub Actions only |
| 29 | Supported types list | README.md:183-194 | feature | language spec | partially true | omits decimal,long,time,bytes,json,any (constants.go:49-62) |
| 30 | HTTP methods `GET POST PUT DELETE PATCH` | CL:390 | feature | language spec | partially true | + HEAD, OPTIONS, WS (constants.go:44-47) |
| 31 | Service SDKs exist for all 8 backends | CL table; GP 10.2 | feature | ls | verified | backend/*/sdk.go (8 files); user guide shows only TS/Python/Go |
| 32 | Rust/Java/C#/PHP SDK emitters not done | roadmap.md:33-36; service-sdk-plan.md:665-668 | timeline | ls | mismatch (stale) | sdk.go present |
| 33 | SDK class `IAMClient` | CL:315 | feature | helper | mismatch | sdkhelpers.ServiceClassName -> "IamClient"; guide service-sdk.md:39 uses IamClient |
| 34 | SDK file `sdk/iam_client.py`; emitters in `internal/emitter/servicesdk/<lang>/` | service-sdk-plan.md:14,173-176,703-706 | architecture | ls | mismatch | dir absent; CL:320-324 says `sdk/iam/client.py` |
| 35 | Env var `VELD_<NAME>_URL` | CL:316-318 | feature | helper | verified | sdkhelpers EnvVarName |
| 36 | main.go ~4,600 lines / 33 commands | DG:30,373-377; EXEC v4 4.2,8.1 | architecture | wc -l | mismatch (stale) | cmd/veld/main.go is 109 lines; split 2026-08-27 |
| 37 | `internal/openapigen` | DG:~47; PP:70; GP 12.3 | architecture | ls | mismatch | only internal/generators/openapi (CL says single impl) |
| 38 | `internal/server` has zero tests | PP:39; EXEC v4 7.1 | quality | find | verified | no _test.go; now 2,598 LOC (EXEC says 1,699) |
| 39 | 52 test files; ~56,000 Go lines; 22 internal pkgs; "261 source files" | GP:34 | architecture | wc | partially true | 268 tracked .go (216 non-test + 52 test); 56,066 lines; 22 pkgs |
| 40 | mock pkg unwired; internal/errors unused; `--quiet` unused; ToolsConfig unused | GP 11.1-11.2 | feature | grep | verified (still true) | no imports of internal/mock or internal/errors outside self; main.go:41,87 |
| 41 | VS Code palette commands Validate/Generate/DryRun | vscode README:43-46,116-120; EXEC v4 8.3 | feature | grep | mismatch | 0 `registerCommand` in editors/vscode/src/extension.ts |
| 42 | Install names: npm `@maayn/veld`, pip `maayn-veld`, brew `maayn-veld/tap/maayn-veld` | README.md:46-53 | install | manifests | partially true | match packages/npm/package.json:2, pip pyproject; publication to registries unverifiable; other docs give different names (D-15) |
| 43 | `npm install @maayn/veld@0.2.0`, `pip install maayn-veld==0.2.0` | packages/npm/README.md:37; pip README:35 | install | manifests | mismatch | manifests are 0.1.0 (rewritten at release) |
| 44 | `veld --version` -> `veld v0.1.0`; `-v, --version`; 11 commands in help | T:30-53,603-617 | feature | binary | mismatch | prints `veld version 0.1.0`; `-v`=verbose (main.go:86); help lists 18 commands |
| 45 | VS Code publisher `veld-dev`; `.github/workflows/publish-plugins.yml` | docs/ci-cd/PLUGINS_CI_CD_SUMMARY.md | architecture | package.json:6; ls | mismatch | publisher `adhamzineldin`; only ci.yml, release.yml |
| 46 | CI: build + vet + race tests + 5-platform cross-compile | PP:31; GP 13.1; ci-cd/guide.md:136 | quality | read ci.yml | verified | ci.yml matrix linux/darwin x amd64/arm64 + windows amd64; ci.yml:52-62 also checks generated language files |
| 47 | "MIT" license | README.md:538-540 | quality | ls | mismatch | no root LICENSE file (section 4) |
| 48 | Contributors: Adham Zineldin + "Eyad Gamal" | website Contributors.tsx:7-23 | team | git shortlog | mismatch | authors: Adham Zineldin (~187 across 3 identities), mohamed.el-kot (13), Ayman Elfeky (3), Mohamed Elkot (1) |
| 49 | Dev span 27 Feb - 19 Apr 2026, 188 commits | GP:664 | timeline | git log | partially true | start true; HEAD=204 commits, to 2026-09-12 (Feb 17, Mar 86, Apr 85, May 1, Aug 8, Sep 7) |
| 50 | Go emitter "Next (In Planning)", updated Apr 5 2026; refs plan.md etc. | roadmap.md:3-4,11,415,470,512-516 | timeline | ls | mismatch | all backends exist; plan.md, PHASE_1_SETUP.md, IMPLEMENTATION_ROADMAP.md, testapp/ absent |
| 51 | docs/README links `../test/README.md` | docs/README.md:37 | architecture | ls | mismatch | no test/ dir (packages/README.md does exist) |
| 52 | "Single types/types.ts, no per-module files" | changelog-v0.1.md item 2 | feature | generated | mismatch (superseded) | per-module types/*.ts + barrel (agrees with CL:513) |
| 53 | "Nine worked examples" | DG:56 | quality | ls | mismatch | 10 dirs under examples/ (GP says ten) |
| 54 | Getting-started code: `registerUsersRoutes`, `List/GetById`, `api.auth.Login`, schemas/, client/ under generated/ | GS:61-95 | feature | generated | mismatch | export is `usersRouter`; init prints `usersRoutes`; methods `listUsers`; SDK `api.Users.listUsers()` (README.md:304); no schemas/ |
| 55 | Default frontend is typescript; default out `./generated` | README.md:379-380 | feature | config.go:346-372 | partially true | frontend defaults to typescript only for node-ts/node-js, else `none`; out default verified |
| 56 | Lint has 8 rules | CL; DG:431-433; GP:10.4 | feature | not re-derived | unverifiable | internal/lint/lint.go |
| 57 | Built-in aliases models,modules,types,enums,schemas,services,lib,common,shared | README.md:402; OV | feature | not diffed | unverifiable | config.go DefaultAliases |
| 58 | FastAPI selectable via `--backend-framework` | GP 11.2 states no routes registered | feature | not run | unverifiable | python/strategy/fastapi.go exists; help text omits fastapi |
| 59 | .vsix 0.1.0 checked in | EXEC v4 8.3 | architecture | git ls-files | mismatch (stale) | no .vsix tracked; `editors/vscode/out/*` is tracked |
| 60 | Go toolchain missing on dev machine (2026-08-26) | EXEC v4 4.1 | architecture | n/a | unverifiable | machine-specific |
| 61 | Compatibility matrix / version detection / `veld doctor` | EXEC v4 5.x,12 | feature | grep | mismatch (planned, undelivered) | no such command or matrix |
| 62 | `docs/index.html`: 8 backend languages, 10 frontend targets | docs/index.html | support matrix | registries | verified | |

### 2. Discrepancies (reader-facing)

Severity: MISLEADING = reader would form a wrong picture of what ships; MINOR = stale or cosmetic.

| ID | Claim | Source | Reality | Evidence | Severity |
|---|---|---|---|---|---|
| D-01 | Node backend generates Zod schemas (`schemas/schemas.ts`) and validates input with Zod, ZodError->400 | CL:9,81,444-446,467-468; README.md:32,272,292; OV Node row; changelog-v0.1 items 1,6; GS:63-68; T:849,928-950; GP:694 (demo); website DocsPage.tsx:1608,1758 | No Zod is emitted at all; default routes do no validation; opt-in `--validate` emits zero-dep `_validators.ts` | scratch output tree; `grep -rni zod internal/emitter/backend/node` | MISLEADING |
| D-02 | Output is deterministic (non-negotiable / evaluation criterion) | CL:508; README.md:419; GP 14.3 | Not deterministic: `types/index.ts` export order varies per run | 4 hashes in 6 runs; internal/emitter/tsshared/types.go:438-443 | MISLEADING |
| D-03 | `veld login/logout/push/pull/serve/doctor` are top-level commands | CL:52-58,392-399; GP 14.1; DG:381 | Only `veld registry <cmd>`; `doctor` unregistered, does not run | cmd/veld/main.go:88-102; registry.go:373; tooling.go:195-200 | MISLEADING |
| D-04 | `tools` config block toggles auxiliary generators | CL:199-202,238 | Parsed, never read; generators run only via `--backend=<tool>` | config.go:44-55,76 | MISLEADING |
| D-05 | GitLab CI generated | README.md:35,233; website SupportedStacks.tsx:33 | Only GitHub Actions | `grep -rli gitlab internal cmd` -> none | MISLEADING |
| D-06 | Rust = Actix; PHP = PSR-15 | README.md:221,224; OV; website Features.tsx:26 | Rust = Axum, PHP = Laravel (plus plain) | backend/rust/strategy/axum.go; backend/php/strategy/laravel.go | MISLEADING |
| D-07 | Module path `github.com/veld-dev/veld`; "only dependency cobra"; SQLite dep | OV "Go Module"; DG:87-90 | module `github.com/Adhamzineldin/Veld`; cobra + lib/pq + x/crypto; no sqlite | go.mod:1-13 | MISLEADING |
| D-08 | Python output has `types/` + `schemas/schemas.py` | CL:456-460; T:854; changelog | Layout is models/ interfaces/ routes/ middleware/; routes import a schemas module nothing writes | scratch output; python/routes.go:55 | MISLEADING |
| D-09 | VS Code palette commands work | vscode README:43-46,116-120 | none registered | extension.ts: 0 registerCommand | MISLEADING |
| D-10 | MIT license | README.md:538-540 | No root LICENSE; chocolatey nuspec links to missing file | `ls LICENSE*`; packages/chocolatey/veld.nuspec:9 | MISLEADING |
| D-11 | Install names | README.md:46-53 vs packages/README.md:11-12 (`npm install veld`, `pip install veld`), composer README:56 (`brew install veld`), vscode README:66 (`adhamzineldin/tap/veld`); npm/pip README `@0.2.0` vs manifest 0.1.0 | five variants for brew/npm/pip | packages/*/README.md; package.json:3 | MISLEADING |
| D-12 | Getting-started snippets | GS:61-95 | wrong function/method names, wrong output tree | generated output | MISLEADING |
| D-13 | "Zero runtime deps / no lock-in" universal | README.md:37,417; CL:9 | true for TS/Node path; other stacks import a framework unless `plain` strategy chosen | strategy dirs | MINOR |
| D-14 | Never writes outside `--out` | CL; OV; T:840 | setup/hook/lock/cache/frontend default dir write elsewhere | setup.go:362-515; generate.go:24,209 | MINOR |
| D-15 | Overview lists 1 frontend, 7 backends (no node-js) | OV | 10 frontends, 8 backends | registries | MINOR |
| D-16 | Hosted-docs scrape `test.md` stale: 7 backends/4 frontends, react = typescript alias, `-v` = version, 11 commands | T:17,38-53,603-617,1210-1235 | stale vs binary | binary run | MINOR (but tracked at repo root) |
| D-17 | Developer guide says main.go is ~4,600 lines, `internal/openapigen` exists, nine examples, sqlite | DG:30,47,56,87,373 | main.go 109 lines (split 2026-08-27, before the 2026-09-12 guide commit); no openapigen; 10 examples | cmd/veld/*.go; ls examples | MINOR |
| D-18 | Roadmap/service-sdk-plan show done work as TODO; cite nonexistent files | roadmap.md:33-36,470,512-516; service-sdk-plan.md:3,665-668 | SDKs exist for all 8; plan.md etc. absent | backend/*/sdk.go | MINOR |
| D-19 | SDK class `IAMClient` | CL:315 | `IamClient` | sdkhelpers | MINOR |
| D-20 | PLUGINS_CI_CD_SUMMARY: publish-plugins.yml, publisher veld-dev | that file | neither | .github/workflows; package.json:6 | MINOR |
| D-21 | "261 source files" | GP:34,284 | 216 non-test + 52 test = 268 tracked .go | git ls-files | MINOR |
| D-22 | "~14 languages/frameworks" | PP:5 | 18 targets (+6 tools) | registries | MINOR |
| D-23 | Dev span ended 19 Apr 2026, 188 commits | GP:664 | 204 commits; work to 2026-09-12 | git log | MINOR |
| D-24 | Types/HTTP-method lists incomplete | README.md:183-194; CL:390 | extra types/methods exist | language/constants.go:44-62 | MINOR |
| D-25 | Website team list | Contributors.tsx | author "Eyad Gamal" not in git history; 3 other committers not listed | git shortlog | MINOR |
| D-26 | EXEC v4 snapshot numbers (server 1,699 lines; main.go 4,610; .vsix tracked) | EXEC v4 4.2,7.1,8.1,8.3 | server 2,598 LOC; main.go 109; no .vsix | wc; git ls-files | MINOR |

### 3. Historical plans summary (do NOT inherit silently)

Git context: first commit 2026-02-27; HEAD 6a47f81 (2026-09-12); 204 commits; per month Feb 17, Mar 86, Apr 85, May 1, Aug 8, Sep 7 (gap May-Aug). Authors: Adham Zineldin (~187), mohamed.el-kot (13), Ayman Elfeky (3), Mohamed Elkot (1).

#### docs/roadmap.md (Last Updated April 5, 2026; "Version 1.0"; 536 lines)
- Scope: Go, Rust, Java, C#, PHP backends (5 new), VS Code + IntelliJ plugins, npm/pip/Homebrew wrappers, service SDKs (roadmap.md:11).
- Dates: Phase 1 done "Feb 28, 2026" (estimated 2-3 weeks, finished same day, "ahead of schedule"); Phase 2 Go Mar 1-28 "Next (In Planning)"; Phase 3 backends Mar 29-May 17; Phase 4 plugins May 18-Jun 29; Phase 5 packages Jun 30-Jul 18; "~16 weeks remaining" (roadmap.md:467-476).
- Estimates: 2 weeks each for Rust/Java/C#/PHP; VS Code 2w; IntelliJ 3w. Tests claimed: 28 Go-adapter + 13 codegen = 41 (roadmap.md:133-164).
- Service SDK section: TS, Python, Go checked; Rust, Java, C#, PHP unchecked (roadmap.md:19-36). Reality: all done.
- No team or names. Cites nonexistent plan.md, PHASE_1_SETUP.md, IMPLEMENTATION_ROADMAP.md, testapp/.
- Self-contradiction: dated April 5 but lists March items as "next"; "Zero breaking changes to TypeScript/Python emitters" (only two emitters recognised).

#### docs/architecture/service-sdk-plan.md (Apr 5, 2026; "Status: Planned"; "Author: Auto-generated"; 779 lines)
- Plans TS/Python/Go first (weeks 2-5), Rust/Java/C#/PHP "Phase 5" weeks 5-7, CLI weeks 7-8, docs weeks 8-9 (lines 634-690): about 9 weeks.
- Paths/names differ from implementation (`internal/emitter/servicesdk/...`, `sdk/iam_client.py`, `ServiceSdkEmitter`); implemented as `BackendEmitter.EmitServiceSdk` and `backend/<lang>/sdk.go`.

#### docs/changelog-v0.1.md (77 lines)
- "All 16 features" done; no dates/names. Contains superseded claims (single types.ts; Zod wiring that is not in current code). "Future" list (Go, Rust, Java/Kotlin, C#, PHP, plugins, npm/pip/brew) was all later delivered. Uses module `veld-dev/veld`.

#### docs/PROJECT_PROPOSAL.md (98 lines, undated, committed 2026-09-12)
- Dual goal: capstone + real product. "~14 languages/frameworks" (conflicts with 18).
- Phases A registry tests + versioned migrations (golang-migrate/goose); B CONTRIBUTING.md + examples cleanup; C backfill tests, fix imports.go:79; D commercial framing (hosting, licensing, security review). No dates or weeks.
- Team: unspecified; "1-2 people" Phase A, "1 person" Phase B, "remaining team" Phase C.
- Facts to re-check: no server tests (still true), no CONTRIBUTING (still true), examples clutter (still true), `internal/openapigen` (false).

#### docs/GRADUATION_PROPOSAL.md (799 lines, graduation-committee framing)
- Numbers: Go 1.22; ~56,000 LOC; "261 files" (actual 268); 22 packages; 52 test files; 8+10=18 targets + 6 aux; 10 examples; 5-platform CI.
- Timeline: development 27 Feb-19 Apr 2026, 188 commits; Phase 8 Hardening "current" (GP:664-677). Reality: 204 commits to Sep 2026.
- Defect register C1-C11 critical (Rust Axum doesn't compile; Go plain `mux`; Python schemas module never written; `fmt --write` deletes `prefix:`; registry partial-JWT 2FA bypass, private-package enumeration, path traversal on publish, token expiry unenforced; PHP HEAD; Swift `${id}`; C# scaffold) + high/medium/low. I re-confirmed only: Python schemas gap, server has no tests, unwired mock/errors/quiet/ToolsConfig. Others not re-verified; they may have been fixed or not.
- Claims contradicted by code: demo says "Zod schemas, Express wiring" (GP:694); lists `veld doctor/login/push/pull/serve` as commands (GP 14.1).
- Team: unnamed; audience "a committee".

#### docs/EXECUTION_PLAN_v1.md (17 weeks = 13 core + 4 buffer; 5 members; 512 lines)
- Role split: M1 core/CLI/tooling; M2 7 non-Rust backends + generators + 2 new backends (Ruby, Kotlin); M3 frontends + registry + production + 3 new frontends (SolidJS, Python client, Go client); M4 Rust only (+Actix strategy, Rust client); M5 docs only.
- Phases: P0 W1, P1 W2-4 (C1-C11), P2 W5-8, P3 W9-12, P4 W13, buffer W14-17.
- Emitter counts: "8 new targets" stated but table lists 7 items; ends at 10 backends / 14 frontends. No names, no calendar dates.

#### docs/EXECUTION_PLAN_v2.md (17 weeks; 5 members; 578 lines)
- Same duration; re-split by type-system family (M1 TS/web, M2 dynamic/systems, M3 JVM/.NET/mobile, M4 Rust, M5 docs). Same new targets (Ruby, Kotlin backend, SolidJS, Python/Go/Rust clients); 10/14 target.

#### docs/EXECUTION_PLAN_v3.md (16 weeks = 12 core + 4 buffer; 5 members; 998 lines)
- Principle P1 NO new emitters/features: all seven v2 additions cancelled. Tier 1 backends node-ts, python, go, java + rust (Rust = Member 4 only); node-js, csharp, php "frozen, not deleted" (Tier 2).
- Six two-week sprints S1-S6 + buffer B1-B2; registry programme S5; plugin programme S6. Counts: stays at 8 backends / 10 frontends.

#### docs/EXECUTION_PLAN_v4.md == docs/EXECUTION_PLAN.md (diff shows byte-identical; "Version 4", current)
- 16 weeks, 6 phases, 16 weekly checkpoints, 5 members (4 implementing, 1 docs). P1 stabilisation W1-2; P2 version/compat matrix W3-4; P3 adapters W5-10 (5 backends W5-7: node-ts, python, go, java, rust; 5 frontends W8-10: typescript, react, vue, angular, svelte); P4 registry W11-12; P5 plugins/CLI W13-14; P6 hardening/release W15-16.
- Stale facts: Go "not installed" (2026-08-26), main.go 4,610 lines/33 commands, server 1,699 lines, .vsix tracked. Only calendar date is 2026-08-26; weeks are not anchored to a start date.
- Wording "Confirmed by you" indicates an AI-drafted plan for a requester. A1-A4 are open assumptions (java vs csharp; Rust exclusivity; 16 weeks; weekly vs two-week tasks).
- Also tracked as Word copies: docs/Veld_Execution_Plan_v1..v4.docx.

Cross-document conflicts to carry forward carefully
- Target counts: PP "~14"; GP/code 18 (8+10); EXEC v1/v2 plan 24 (10+14); EXEC v3/v4 keep 8+10 but only 5+5 "supported"; website "13+".
- Team: GP unnamed; plans "5 members" with M1-M5; website 2 named; git 4 human authors.
- Timeline: GP says dev ended 19 Apr 2026; roadmap (Apr 5) shows Go as next; v4 written Aug-Sep 2026 with an unanchored 16-week schedule.
- EXECUTION_PLAN.md (unversioned) is a duplicate of v4.

### 4. Open-source hygiene

- Root LICENSE: ABSENT (not in `git ls-files`; `ls LICENSE*` fails). README.md:538-540 says "MIT". Chocolatey nuspec (`packages/chocolatey/veld.nuspec:9`) links to `/blob/master/LICENSE`.
- Declared licenses: packages/npm/package.json:22 MIT; packages/composer/composer.json:5 MIT; packages/pip/pyproject.toml:10 MIT; packages/nuget/VeldCli.csproj:20 MIT; editors/vscode/package.json:7 MIT; website/package.json: none (private, v0.0.0); editors/jetbrains/build.gradle: no license field (but editors/jetbrains/LICENSE exists). LICENSE files exist in editors/vscode, editors/jetbrains, packages/npm, packages/pip; the latter two say "Copyright (c) 2024 Adhamzineldin" (repo's first commit is 2026-02-27).
- ABSENT: CONTRIBUTING.md (PP:41,62 acknowledges), CODE_OF_CONDUCT.md, SECURITY.md, CHANGELOG (only docs/changelog-v0.1.md), CODEOWNERS (v2 plan says it will create one), .github/ISSUE_TEMPLATE, PULL_REQUEST_TEMPLATE. `.github/` holds PLUGINS_SETUP.md, VSCODE_PUBLISHING_TROUBLESHOOTING.md, workflows/ci.yml, workflows/release.yml.
- Tracked at HEAD but should not be (`git ls-tree -r -l HEAD`; 732 files, 24,845,147 bytes total, `.git` 36 MB):
  - `veld` at repo root: ELF binary, 20,450,930 bytes, blob 0c2abda, added 2026-05-09 in caabde3, still tracked despite `.gitignore` `/veld` and the 2026-08-27 commit "remove committed build artifacts". About 82% of tracked bytes.
  - `test-gen/` 6 files 2,523 B; `test-gen2/` 4 files 1,551 B; `test-headers/` 4 files 2,631 B; `test-constants/` 3 files 1,225 B (DG calls them throwaway).
  - `test.md` 30,318 B (stale scrape of hosted docs).
  - `.idea/` 11 files 6,319 B (.gitignore:62 deliberately keeps it; contains personal plugin state such as copilot, csv-editor, material theme).
  - Also: `.claude/settings.local.json`; root `.veld.lock.json` and `.veld-cache.json`; `editors/vscode/out/*` compiled JS/sourcemaps; 4 `docs/Veld_Execution_Plan_v*.docx`; 11 meta-docs at examples/ root (COMPLETION-REPORT.md, FINAL-SUMMARY.md, START-HERE.md, ...).
- 10 largest tracked blobs (bytes): 1 veld 20,450,930; 2 resources/logo.png 321,310; 3 resources/favicon.ico 125,619; 4 website/src/pages/DocsPage.tsx 105,129; 5 internal/server/handlers/web/index.html 84,102; 6 editors/vscode/src/extension.ts 80,368; 7 docs/Veld_Execution_Plan_v3.docx 71,804; 8 docs/GRADUATION_PROPOSAL.md 66,631; 9 docs/Veld_Execution_Plan_v2.docx 61,756; 10 docs/Veld_Execution_Plan_v4.docx 59,182.
- Versioning: 47 tags, latest v0.3.24; source default `0.1.0`, package manifests 0.1.0, plugin READMEs 0.2.0. Release workflow rewrites versions at tag time (release.yml:304,679-680), so the working tree never shows the released version.

### 5. Things the code has that the docs do not mention

- Commands: `veld ci` (registry.go:433), `veld export`, `veld agents`, `veld registry info|list|versions|token create|init`; `veld deps` is only in CLAUDE.md/roadmap, not README's CLI table; `ast`/`fmt` are hidden from `--help`.
- Framework strategies: Gin (go) and FastAPI (python) and `plain` strategies for every backend; `--backend-framework` help omits gin/fastapi (generate.go:1113).
- Language: types long, time, bytes, json, any, decimal; methods HEAD, OPTIONS; annotations primary, autoincrement, readonly, min, max, minLength, maxLength; `@serverSet` (DG:119; GP 12.2); unions and constants (GP:317); `emit` directive.
- Frontend: `VeldWebSocket<TReceive,TSend>` (GP 10.2); `--frontend-framework`; `--server-sdk` (config `serverSdk`) distinct from `--service-sdk`.
- Generated extras: `middleware/IMiddleware.ts`, `types/_scalars.ts` branded scalar types, generated `README.md`.
- Packaging channels beyond README: chocolatey, apt (nfpm), nuget (`Maayn.Veld`).
- Other repo content: `docs/index.html` landing page, `registry.config.example.json`, `veld-language.json`, `scripts/`.



## Appendix: audit-track notes (B-compiler-frontend)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Veld compiler frontend audit

Commit 6a47f815f37ea4c286a6e765312df76743fa7133, reviewed 2026-10-21. No tracked file was modified.
Scratch artefacts: `fe-audit/cases/` (72 case dirs), `fe-audit/results/` (raw stdout/stderr/rc per case), `fe-audit/fz/` (copy of the packages used for a 150 s / 1.67M-exec Go fuzz run and diff scenario tests), `fe-audit/veld.exe`.

Notes on method: `veld validate` takes a positional file (`veld validate main.veld`), not `--input`. `veld lint` takes no file; it was run with a per-case `veld.config.json`. `veld fmt` is hidden and only writes with `--write`; idempotency was tested by running `fmt --write` twice on a copy.

### 1. Package-by-package summary

#### internal/lexer (451 lines)
- Public API: `New(src) *Lexer`, `Tokenize() ([]Token, error)`, `Errors()`, `IsKeyword`, `TokenType.String`. Token = {Type, Value, Line}. **Line only; no column, no offset.**
- Source is converted to `[]rune`; lines are counted on `\n` only (a lone `\r` never advances the line: case 58 reports line 1 for an error on line 3).
- Error recovery: unknown characters are collected and skipped, but `Tokenize` returns only `errors[0]` and the loader discards the rest (lexer.go:355-364).
- No limits (file size, token count). 5 MB file and 100k-field model lex+validate in under 1 s. Lexer is O(n), no recursion.
- Identifiers: `unicode.IsLetter/IsDigit` + `_` (Unicode letters OK: `Usuario` with accent passes; `$`, `-`, `.` rejected; a leading digit lexes as a number, so `1abc` becomes `1` then `abc`). `unicode.IsDigit` also accepts non-ASCII digits in numbers.
- Strings: no escapes, end at `"` or newline; an unterminated string is silently accepted (F-FE-04). Unterminated `/* ...` silently swallows the rest of file.
- `/` always starts a path token except when followed by `*` or `/`, where it starts a comment. This breaks `import @models/*` (F-FE-01).
- BOM U+FEFF is rejected as "unexpected character" (F-FE-15).
- Works well: tokenisation of all documented constructs, CRLF, tabs, emoji in strings, 200k-char brace runs (no crash), contextual keywords (`type`, `default`, `delete`, `class`, `import` as field names all work).

#### internal/parser (1154 lines)
- Public API: `New(tokens)`, `Parse() (ast.AST, error)`. Hand-written recursive descent, **but effectively non-recursive** (blocks do not nest, generics are one level), so there is no stack-depth risk: a 2000-deep and a 200000-deep `{` run produce a normal error (cases 41, 41b).
- **Stops at the first error** (every function returns on first failure). Messages are plain `fmt.Errorf("line N: ...")`, with ad-hoc prefix wrapping ("model name: line 1: ...").
- Positions: token line only. Error line is the line of the *offending token as consumed*, which is sometimes the wrong line (case 62 reports line 3 for a mistake on line 2; unterminated `model A {` reports the nonexistent EOF line instead of the opening brace).
- Types: `T`, `T[]`, `List<T>`, `Map<string,V>`, unions `A | B | "lit"`. **No nesting**: `Map<string,Map<..>>`, `Map<string,T[]>`, `T[][]`, `List<List<T>>` are all parse errors. `Map<string,int>[]` parses as `{isMap:true,isArray:true}` which cannot be distinguished from a hypothetical array-of-maps vs map-of-arrays (F-FE-08).
- Optional is `name?: type` only; there is no nullable concept. `= value` and `@default(v)` both accepted; both together is an error. Annotations: default, deprecated, example, unique, index, relation, serverSet. Unknown annotation = error.
- Actions: contextual keywords `method path description input output response query headers stream emit middleware errors @deprecated`; duplicates silently last-wins (F-FE-23). `output: T[]:201` parsed; non-integer/overflow status silently dropped (F-FE-16). `{id}` is rewritten to `:id` at parse time (parser.go:149). Inline `input/output/query/headers: { ... }` create synthetic models named `<Action>Input|Output|Query|<Action>Headers` (F-FE-17).
- Enums: bare identifiers only, optional commas; no explicit values (case 45 errors); keyword-like values (`GET`, `model`, `from`, `string`) rejected (F-FE-07).
- Imports: `import @a/b`, `import /a/*`, `import a/b`, `import "file.veld"`, `from @a import X, Y`.

#### internal/ast (104 lines)
- Plain structs with JSON tags; `Line`/`SourceFile` are `json:"-"`. Empty slices serialise as `null` (`"models": null`) (F-FE-29). `FileImports` map is not serialised. No column data anywhere.

#### internal/loader (398 lines)
- API: `Parse(path, aliases...) (ast.AST, []string, error)`. Reads file, lex, parse, then recursively resolves imports with a `seen` map keyed on absolute path. Cycles and self-imports are silently tolerated (case 19, 20 valid). Merge order is imports first, then the importing file; ReadDir/WalkDir order is lexical so it is deterministic.
- Errors are wrapped strings ("lexing <path>: ...", "parsing <path>: ..."); paths are as given, absolute for nested imports; no import-statement line.
- Missing wildcard directory is silently ignored (loader.go:248); a missing named alias file silently falls back to loading the whole directory (loader.go:285-355); a typo'd alias loads nothing (F-FE-13). Registry-package resolution via `rootDir/packages/@org/pkg`.
- Duplicate names across files are not handled in the loader; the validator reports the second one only, without naming the first.
- No limit on file size or import depth (bounded by `seen`). `strings.Contains(content, "model "+name)` fallback also matches prefixes (`model User` matches `model UserProfile`).
- Self-edge recorded in `FileImports[a] = append(..., a)` for every glob-loaded file (loader.go:255, 273, 341).
- Relative imports are not confined to the project root (`import "../../x.veld"`): by design for a local CLI, no security impact beyond reading `.veld` files the user can already read.

#### internal/validator (577 + 65 lines)
- API: `Validate(ast.AST) []error` returns **all** semantic errors at once (good), as `fmt.Errorf("file.veld:LINE: ...")` with the file *basename*. Workspace: `ValidateWorkspaceConsumes` (unknown/self consume, missing baseUrl warning; no cycle detection despite CLAUDE.md claiming "circular").
- Checks: duplicate enum/model/module/action/constants names, model-vs-enum-vs-constants collisions, empty enum, duplicate enum values, extends target exists, circular inheritance (self and longer cycles caught), duplicate fields, type existence with Levenshtein "did you mean", Map value type, @default type-compat for primitives and enum members, WS stream/emit rules, success status 200-299 and 204-without-output, route-conflict across modules (`:param` normalised), per-file "type used but not directly imported" check.
- Gaps: path params not checked against any model, query model shape not checked, union members unchecked, @default not checked on arrays/models/unions/maps, parent/child field override not checked, header/error/status ranges, module/model name collisions, `any/json/long/time/bytes` accepted as types and as shadowable model names. See findings.
- Complexity: circular-inheritance check is O(n^3) in chain depth (3000-deep chain: 26.6 s; 1000-deep: 1.4 s) (F-FE-12).
- Works well: multiple errors per run (case 63 reports 3), correct file:line on semantic errors, "did you mean" suggestions, enum default membership, status-code validation, cross-file direct-import enforcement.

#### internal/language (180 + 498 + 160 lines)
- `constants.go` is consumed only by `cmd/generate-language` (IDE spec output). `veld.go`/`imports.go` is a **second, regex/line-based mini-parser with its own error type and 0% test coverage** that nothing in the compiler uses. The lexer keyword table is hardcoded in `lexer.classifyWord`, not read from the spec, so the "single source of truth" claim in CLAUDE.md does not hold (F-FE-22). Spec lists `HEAD`, `OPTIONS`, and annotations (`required`, `primary`, `min`, `regex`, ...) that the parser rejects.

#### internal/errors (128 lines)
- Typed `VeldError{Kind,File,Line,Message,Cause}` and `List`. **Not used by the pipeline**: `grep` finds no call to `NewParseError`/`NewValidationError` outside the package. Lexer/parser/loader/validator all return `fmt.Errorf` strings, so stage and position are only recoverable by regex (and `cmd/veld/util.go:60` does exactly that) (F-FE-06).

#### internal/format (284 lines)
- Token-stream re-printer. Because the lexer discards comments and string quotes, **the output has no comments, no quotes around strings/imports, and glued tokens** (`description:Todo management`, `import@models/todo`). `fmt --write` on a valid example produces a file that no longer parses (F-FE-03). Also panics on `model }` (negative `strings.Repeat`). Idempotent only in the trivial sense that the corrupted output re-formats to itself.

#### internal/lint (361 lines)
- Rules: unused-model, empty-module, empty-model, empty-constants, duplicate-route, duplicate-action, missing-description (module/action), deprecated-action/field, file-naming-convention. Warning/Error severities, errors sorted first. Deterministic; 20k issues sorts in about 2 s (insertion sort).
- `unused-model` ignores models used as field types, so nested models are falsely flagged (F-FE-20). `duplicate-route`/`duplicate-action` are unreachable because lint only runs after `Validate` passes, and the lint route key does not normalise `:param`.
- On `examples/*` the only hit is `unused-model: UpdateUserInput` (8 of 9 example contracts); node-react has no hits. microservices is workspace-only (no per-directory input).

#### internal/diff (404 + 99 lines)
- API: `Diff(old,new) []Change` (Breaking | Added), `LoadLock/SaveLock/DeleteLock`. Sorted by (kind, path) after a stable sort so output is deterministic despite map iteration; lock is `json.MarshalIndent` of slices/structs (`ErrorStatuses` map keys are sorted by encoding/json) so the lock file is deterministic; temp-file + rename write.
- Detected (verified in `fz/internal/diff_test.go`): removed/added/renamed action, module removed/added, module prefix change, method change, path change, input field removed (breaking), field type change, optional->required on input, new required input field, required->optional on output, output array flip, model slot added/removed/changed.
- NOT detected: enum member removal or addition, enum/constants at all, changes inside nested model types, model removal when not referenced directly, middleware, errors list, union type changes, @default changes, WS stream/emit. Success-status change is classified non-breaking (Added).
- `allFields` recurses over `extends` with no cycle guard: a cyclic contract overflows the stack (fatal, unrecoverable) if Diff sees an unvalidated AST (F-FE-19).
- And see F-FE-02: in `veld generate` the lock is deleted before the gate runs, so none of this ever fires.

#### internal/cache (103 lines)
- Content-hash (SHA-256) per absolute file path, stored in `.veld-cache.json`; legacy mtime map is discarded. Keys are only the `.veld` source files. Not keyed on: config file, CLI version, flags/targets, aliases, or whether the output directory still exists. `generate` (even with `--incremental`) calls `runClean` first, deleting the cache (F-FE-21). Tests 92% coverage; code itself is simple and correct.

### 2. Adversarial case results

Columns: validate rc; first output line; whether `file:line` (or `line N`) appears; whether a source snippet is printed (`validate` never prints one); number of `error:` lines; Go panic; `ast` rc; `lint` rc; fmt idempotent (run twice); whether `validate` accepts the fmt output (1 = formatter broke it). Raw files: `fe-audit/results/<case>.*`.

| case | validate rc | first line | file:line | snippet | #errs | panic | ast rc | lint rc | fmt idem | fmt-out validates rc |
|---|---|---|---|---|---|---|---|---|---|---|
| 01-empty | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 02-only-comments | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 03-unterminated-string | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 04-unterminated-block | 1 | Error: parsing main.veld: line 3: expected "}", got "" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 05-nested-generics | 1 | Error: parsing main.veld: line 2: expected ">", got "<" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 06-10000-fields | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 07-5mb-file | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 08-utf8-bom | 1 | Error: lexing main.veld: line 1: unexpected character '\ufeff' | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 09-crlf | 1 | error: main.veld:3: model "A", field "y": undefined type "badtype" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 10-nonascii-ident | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 11-emoji-description | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 12-tab-indented | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 13-field-type | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 14-field-default | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 15-action-delete | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 16-model-named-string | 1 | Error: parsing main.veld: model name: line 1: expected identifier, got "string" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 17-enum-same-as-model | 1 | error: main.veld:1: name collision: "X" is defined as both model and enum | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 18-duplicate-across-files | 1 | error: main.veld:2: duplicate model name: "Foo" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 19-self-import | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 20-import-cycle | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 21-wildcard-empty-dir | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 22-missing-import | 1 | Error: import "nope.veld": reading C:\Users\adham.zaineldin\AppData\Local\Temp\claude\D--University-Veld\1d57057e-5535-488c-a1fd-0b17c38b6e12\scratchp | no | no | 0 | no | 1 | 1 | idem | 1 |
| 22b-missing-alias-file | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 22c-typo-alias-dir | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 23-extends-self | 1 | error: main.veld:1: model "A": circular inheritance detected via "A" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 24-extends-chain-override | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 25-default-notmember | 1 | error: main.veld:5: model "A", field "r": @default(notAMember) is not a valid value for enum "Role" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 26-default-str-on-int | 1 | error: main.veld:2: model "A", field "n": @default for int must be a number, got "x" | yes | no | 1 | no | 0 | 1 | idem | 0 |
| 27-path-param-missing-field | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 28-same-route-two-modules | 1 | error: main.veld:12: route conflict: GET /same in B.Y overlaps with A.X | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 29-same-action-twice | 1 | error: main.veld:10: module "A": duplicate action name: "X" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 30-status-999 | 1 | error: main.veld:5: module "M", action "A": invalid success status 999 (must be 200–299) | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 31-status-abc | 1 | Error: parsing main.veld: output status after ':': expected HTTP status code, e.g. output: Foo:201 | no | no | 0 | no | 1 | 1 | idem | 1 |
| 31b-status-1p5 | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 32-method-FETCH | 1 | Error: parsing main.veld: line 6: expected HTTP method (GET, POST, PUT, DELETE, PATCH, WS), got "FETCH" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 33-path-no-slash | 1 | Error: parsing main.veld: action path: line 7: expected path (e.g. /auth/login), got "users" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 34-path-braces | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 35-query-array-nested | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 36-headers-unquoted-dash | 1 | Error: lexing main.veld: line 9: unexpected character '-' | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 36b-headers-quoted-dash | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 37-optional-default | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 38-array-of-maps | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 39-map-int-key | 1 | Error: parsing main.veld: line 2: Map key type must be "string", got "int" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 40-numeric-field-name | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "1" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 41-deep-braces-2000 | 1 | Error: parsing main.veld: line 1: unexpected token "{" | yes | no | 0 | no | 1 | 1 | idem | 0 |
| 41b-deep-braces-in-model | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "{" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 42-string-array-2d | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "[" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 43-field-named-date | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "date" | yes | no | 0 | no | 1 | 1 | NOTIDEM | 1 |
| 44-enum-http-values | 1 | Error: parsing main.veld: enum value: line 2: expected identifier, got "GET" | yes | no | 0 | no | 1 | 1 | NOTIDEM | 0 |
| 45-enum-explicit-values | 1 | Error: parsing main.veld: enum value: line 2: expected identifier, got "=" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 46-inline-collision-across-modules | 1 | error: duplicate model name: "CreateInput" | no | no | 1 | no | 0 | 1 | idem | 1 |
| 47-inline-headers-nonascii-action | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 48-string-escape | 1 | Error: lexing main.veld: line 2: unexpected character '\\' | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 49-union-undefined | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 50-map-undefined-value | 1 | error: main.veld:2: model "A", field "s": undefined Map value type "Nope" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 51-constants-type-mismatch | 1 | error: main.veld:4: constants "K".C: expected true or false, got "yes" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 52-comments-preserve | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 53-model-named-json | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 54-path-dup-param | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 55-get-with-input | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 56-module-order | 1 | Error: parsing main.veld: line 10: expected "action", got "prefix" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 57-error-status-weird | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 58-lone-cr | 1 | error: main.veld:1: model "A", field "y": undefined type "nope" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 59-decimal-default | 1 | error: main.veld:3: model "A", field "q": @default for float must be a number, got "x" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 60-204-with-output | 1 | error: main.veld:5: module "M", action "A": status 204 (No Content) cannot have an output type | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 61-trailing-slash-conflict | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 62-multi-error-parse | 1 | Error: parsing main.veld: line 3: expected ":", got "}" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 63-multi-error-sem | 1 | error: main.veld:2: model "A", field "x": undefined type "Nope1" | yes | no | 3 | no | 0 | 1 | idem | 1 |
| 64-keyword-as-enum-val | 1 | Error: parsing main.veld: enum value: line 2: expected identifier, got "model" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 65-model-named-any | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 66-wildcard-comment-swallow | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 67-fmt-panic | 1 | Error: parsing main.veld: model name: line 1: expected identifier, got "}" | yes | no | 0 | YES | 1 | 1 | idem | 1 |

#### Notes on specific rows
- 03, 04, 08, 22b, 22c, 31b, 66, 27, 49, 53, 54, 55, 61, 15, 24, 57 are the "silently accepted" cases; each maps to a finding below.
- 31b-status-1p5 (`output: Foo:1.5`): `validate` says valid and `ast` has no `successStatus` at all (silently dropped).
- 47: `validate` passes and the AST contains a synthetic model whose name is `\xef\xbf\xbd�ditHeaders` (invalid UTF-8, produced by byte-slicing `act.Name[:1]`).
- 66 (reproduced from `import @models/*` + a following model/module): `ast` lists only `Real`; `Foo` and module `M` are gone, `validate` says "Contract is valid".
- 67 (`model }`) is the only Go panic seen via the CLI: `veld fmt` panics ("strings: negative Repeat count"). Lexer, parser, validator, lint and diff survived 1.67M fuzz executions (no panic, no hang) on valid-by-construction pipeline input; the only crashes found are the formatter panic and the diff stack overflow on a cyclic AST (F-FE-19).
- Stack overflow with nesting: none (cases 41, 41b); the parser is not recursive.
- Position checks by eye (Part C): case 09 (CRLF) reports `main.veld:3` for `y: badtype` on line 3: **correct**. Case 25 reports `main.veld:5` for the `r: Role @default(notAMember)` field on line 5: **correct**. Case 62 (`x: bad bad` on line 2) reports `line 3: expected ":", got "}"`: **wrong line**, the second `bad` on line 2 is the culprit. Case 04 (`model A {` never closed) reports `line 3` (past EOF) instead of the unclosed `{` on line 1: **poor**. Case 58 (lone CR) reports line 1 instead of 3: **wrong**. Multiple errors: validator reports all (case 63: 3 errors); lexer/parser report exactly one. Stage distinction: only by message shape ("lexing X:", "parsing X:", "error: file:line:"), not by typed kind.

### 3. Findings

#### F-FE-01: `import @alias/*` and `@alias/**` are lexed as a block comment and silently delete the rest of the file
- Category: lexer
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/lexer/lexer.go:240-251 and :253-257 (comment detection precedes the path-token branch at :315); parser consumer internal/parser/parser.go:210-215
- Trigger/reproducer: `fe-audit/cases/66-wildcard-comment-swallow/main.veld` containing `import @models/*` followed by `model Foo {...}` and `module M {...}`; run `veld.exe validate main.veld` and `veld.exe ast main.veld`. Also the shipped example `examples/microservices/veld/app.veld` (`import @services/**`): `veld ast veld/app.veld` returns `"models": null, "modules": null`.
- Expected vs actual: Expected the wildcard import to load all files in `models/` and keep the remainder of the file. Actual: the `/*` after `@models` opens a block comment that runs to the next `*/` or EOF; `Foo` and `M` vanish, validation prints "Contract is valid". The documented forms in docs/architecture/overview.md:55 and docs/DEVELOPER_GUIDE.md:189 (`import @models/*`) are the broken ones; `import /models/*` and `from @models import *` work.
- Impact: silent loss of models/modules, generated code missing endpoints with no diagnostic.
- Root cause: `/` followed by `*` is tested for comment start before the lexer knows it is inside an import path; `//` in paths has the same problem (`path: //x`, extra case e19).
- Recommendation: fix, make comment recognition context-aware (or lex `@alias/…` import paths as one token) and add an "unterminated block comment" error.
- Regression check: loader test with `import @models/*` + model afterwards asserting model count; lexer test that `@a/*` and `@a/**` yield TPath; microservices example must yield non-empty AST.
- Effort estimate: 2h-4h

#### F-FE-02: `veld generate` deletes the lock file and output before the breaking-change gate, so breaking-change detection never fires (and an invalid contract wipes the previous output)
- Category: diff
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: cmd/veld/generate.go:758-760 (`runClean`), gate at :1019-1035, `computePreChanges` :1126-1140, `runClean` removing the lock at :51; same ordering in cmd/veld/watch.go:56
- Trigger/reproducer: project in `fe-audit/gate/`: `veld generate` once with a model `A{x:int,y:string}` and `POST /a`; then change to `A{x:int}` and `PUT /b` (field removed, method and path changed) and run `veld generate --strict`. Exit code 0, no breaking-change output; `.veld.lock.json` was deleted by `runClean` before `computePreChanges` read it. Second repro: put `model A extends A {}`; `veld generate` fails validation but `generated/`, `.veld.lock.json`, `.veld-cache.json` are already gone.
- Expected vs actual: CLAUDE.md says "pre-emit gate: interactive prompt by default, `--strict` exits 1 (CI), `--force` skips prompt". Actual: gate input (`LoadLock`) is always empty for a non-dry-run, non-incremental generate; `--strict` can never fail.
- Impact: the CI safety feature advertised as a differentiator is a no-op; developers lose their previous generated output when the contract has any error.
- Root cause: `runClean(rc)` unconditionally precedes the gate and also deletes the lock (generate.go:51), and runs before parse/validate.
- Recommendation: fix, validate first, run the gate against the lock, only then clean/emit; never delete the lock in generate.
- Regression check: CLI test: generate, introduce each breaking change, assert `generate --strict` exits non-zero and leaves output intact; test that a validation failure leaves `generated/` untouched.
- Effort estimate: 3h-6h

#### F-FE-03: `veld fmt --write` corrupts valid files (drops comments, string quotes, spacing) and panics on `model }`
- Category: format
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/format/format.go:27-33 (formats from a token stream), :187-189 (default branch writes `tok.Value` of TString without quotes), :153 (`writeIndent(baseIndent+depth)` with negative depth), :113-131
- Trigger/reproducer: copy `examples/node-typescript/veld` to a temp dir and run `veld.exe fmt --write app.veld models/*.veld modules/*.veld`; then `veld validate app.veld` fails with `parsing ...todos.veld: line 4: expected "action", got "description"`. Output sample: `description:Todo management`, `import@models/todo`, `action   ListTodos\n {`. Comments are gone (`cases/52-comments-preserve`). Panic: `printf 'model }\n' > x.veld; veld fmt x.veld` (`cases/67-fmt-panic`).
- Expected vs actual: A formatter must preserve semantics and comments. Actual: the output no longer parses for every file that contains a quoted string or an import (6 of the 72 cases: 03, 11, 19, 20, 36b, 38 were valid before formatting; and every shipped example file).
- Impact: destructive in-place rewrite; the command is hidden but documented in `veld fmt` help and CLAUDE.md.
- Root cause: formatter is built on the lexer, which discards comments, quote characters and whitespace; no round-trip test.
- Recommendation: rewrite, build the formatter on a lossless CST/token stream (keep trivia and raw lexeme) and add parse-after-format and idempotency checks.
- Regression check: golden tests: for every `examples/**/*.veld`, `Format(Format(x)) == Format(x)`, `Parse(Format(x))` equals `Parse(x)`, and comments survive; fuzz target that `Format` never panics.
- Effort estimate: 12h-20h

#### F-FE-04: Unterminated string and unterminated block comment are accepted silently
- Category: lexer
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:259-270 (string stops at newline, no error), :240-251 (comment loop ends at EOF, no error)
- Trigger/reproducer: `cases/03-unterminated-string/main.veld` (`description: "abc` newline) -> `validate` rc 0 and AST description `abc`. `cases/04-unterminated-block` shows the related unclosed-brace report. An unterminated `/*` hides all following source (also part of F-FE-01).
- Expected vs actual: Expected a lexer error at the opening quote/comment with line. Actual: valid contract.
- Impact: typos produce silently truncated or missing definitions.
- Root cause: no error branch when the closing delimiter is not found.
- Recommendation: fix, emit a positioned lexer error at the opening delimiter.
- Regression check: lexer tests for `"abc\n` and `/* x` expecting an error with the opening line.
- Effort estimate: 1h-2h

#### F-FE-05: Diagnostics are first-error-only for lexer/parser, line-only, and sometimes point at the wrong line
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:363 (returns `errors[0]`), internal/loader/loader.go:94-101, internal/parser/parser.go (every `return ..., err`), internal/lexer/lexer.go:195-199 (`Token{Type,Value,Line}`)
- Trigger/reproducer: `cases/62-multi-error-parse/main.veld` (two independent syntax errors; only one reported, on line 3 while the first mistake is `x: bad bad` on line 2); `cases/58-lone-cr/main.veld` (reports line 1, true line 3); `cases/04-unterminated-block/main.veld` (reports non-existent line 3).
- Expected vs actual: Expected all syntax errors with line:column and a snippet (validate reports all semantic errors but no snippet; `validate` never prints one). Actual: one error, no column, wrong lines in three of the four tested syntax cases.
- Impact: slow edit-compile loops; IDE/LSP squiggles cannot be precise.
- Root cause: no panic-mode recovery in the parser; tokens carry no column/offset; lexer errors are collected then truncated.
- Recommendation: refactor, add column/offset to Token, return all lexer errors, add statement-level synchronisation in the parser.
- Regression check: golden diagnostic tests (line AND column) for 10 malformed files; test that two errors yield two diagnostics.
- Effort estimate: 10h-16h

#### F-FE-06: Typed error package is dead code; errors are strings, stage is not machine-readable, file names are basenames
- Category: lexer
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/errors/errors.go:1-128 (unused by pipeline), internal/validator/validator.go:30-45 (`filepath.Base`), cmd/veld/util.go:45-75 (regex-like `SplitN(msg, ":", 3)` parsing, cache keyed by basename)
- Trigger/reproducer: `grep -rn NewParseError internal cmd` (no pipeline use); two files named `user.veld` in different directories each with an error -> the snippet printer indexes by basename and can show the wrong file's line.
- Expected vs actual: CLAUDE.md says every stage returns structured typed errors with file/line context. Actual: all stages return `fmt.Errorf`.
- Impact: tooling (LSP, JSON output) cannot consume diagnostics; ambiguous locations with duplicate basenames.
- Root cause: typed errors were added but never wired in.
- Recommendation: refactor, make every stage return `*VeldError` (with file, line, column, kind) and delete string parsing in util.go.
- Regression check: test that `errors.As(err, *VeldError)` holds for a lex, parse, load and validation failure and that Kind differs.
- Effort estimate: 6h-10h

#### F-FE-07: Primitive-type and verb tokens cannot be used as field names, enum values, action names or model names
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/parser/parser.go:129-136 (`expectFieldName` allows only TIdent + `lexer.IsKeyword`), :328 (enum values require TIdent), :441, :747; internal/lexer/lexer.go:203-210 (`IsKeyword` omits TType* tokens)
- Trigger/reproducer: `cases/43-field-named-date/main.veld` (`date: date`, `int: int`) -> `field name: line 2: expected identifier, got "date"`. `cases/44-enum-http-values` (`enum Method { GET POST DELETE }`) and `cases/64-keyword-as-enum-val` (`model`, `action`) fail. `cases/16-model-named-string` fails.
- Expected vs actual: `type`, `class`, `default`, `import`, `delete` already work as field/action names (cases 13-15) but `date`, `datetime`, `string`, `int`, `uuid` do not, and an HTTP-method enum is impossible.
- Impact: a field called `date` is extremely common and cannot be expressed; error text does not say the name is reserved.
- Root cause: field-name rule is inconsistent with the contextual-keyword approach used elsewhere.
- Recommendation: fix, accept any word token in name positions and report a clear "reserved" error only where ambiguity exists.
- Regression check: parser tests for fields named after each primitive, enum values `GET`/`model`, action `delete`.
- Effort estimate: 2h-4h

#### F-FE-08: No nested generics; `Map<string,int>[]` is represented ambiguously
- Category: parser
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: none
- Location: internal/parser/parser.go:514-556, internal/ast/ast.go:50-51
- Trigger/reproducer: `cases/05-nested-generics`, `cases/42-string-array-2d`, extra `Map<string, string[]>` and `List<List<string>>` all fail with "expected >" / "expected identifier, got [". `cases/38-array-of-maps` parses to `{"type":"Map","isArray":true,"isMap":true,"mapValueType":"int"}`.
- Expected vs actual: real APIs need `string[][]`, `Map<string,string[]>`. The Field struct (Type, IsArray, IsMap, MapValueType) cannot express nesting; flag combination for array-of-maps vs map-of-arrays is undefined for emitters.
- Impact: contracts cannot model common JSON shapes; emitters guess semantics for the IsMap+IsArray combination.
- Root cause: flat Field representation instead of a recursive type expression.
- Recommendation: refactor, introduce a recursive `TypeRef` in the AST (breaking AST version bump).
- Regression check: parser round-trip tests for `T[][]`, `Map<string,T[]>`, `Map<string,Map<string,T>>`, `Map<string,T>[]` with distinct AST assertions.
- Effort estimate: 12h-24h (touches every emitter)

#### F-FE-09: Path parameters, path shape and routes are barely validated
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/validator/validator.go:298-322 (route conflicts), :525-535 (`normalizeRoutePath`); internal/parser/parser.go:149 (`{id}` rewrite)
- Trigger/reproducer: `cases/27-path-param-missing-field` (`PUT /users/:id` with input lacking `id`) valid; `cases/54-path-dup-param` (`/a/:id/:id`) valid; `cases/61-trailing-slash-conflict` (`/a` and `/a/`) valid; `cases/34-path-braces` valid; `cases/55-get-with-input` (GET with body model) valid; extra e18: `/x/:id/` vs `/x/:other/` is flagged (normalisation works for names).
- Expected vs actual: Expected an error/warning when a path parameter has no corresponding input/query field (or is declared twice), and a normalised comparison ignoring trailing slashes and the app-level prefix. Actual: none of these is checked; the conflict check also ignores the top-level `prefix:` and duplicate `:param` names.
- Impact: generated routes can reference parameters no type provides; duplicate-route detection has holes.
- Root cause: validator only compares (method, normalised path) strings.
- Recommendation: fix, add parameter-vs-input checks, duplicate-parameter check and trailing-slash normalisation.
- Regression check: validator tests for each of the five cases.
- Effort estimate: 4h-6h

#### F-FE-10: `@default` validation is incomplete
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:194-196 (skipped for Map and union fields via the `continue` at :165 and :181), :416-467 (switch on `f.Type` ignores `IsArray`, model-typed fields, `long/time/bytes/any/json`)
- Trigger/reproducer: extra e01 `ids: int[] @default(5)` valid; e02 `b: B @default(5)` (B a model) valid; `cases/37-optional-default` (`x?: int @default(5)`) valid; `float` accepts any non-quoted token including identifiers (e.g. `@default(abc)` is an ident token). Works correctly: `cases/25` (enum member), `cases/26` (`"x"` on int), `cases/59` (`"x"` on float).
- Expected vs actual: default on an array/model/map should be rejected or typed; float/int defaults that are identifiers should be rejected.
- Impact: invalid defaults pass validation and break generated code.
- Root cause: switch only handles the base primitive kinds.
- Recommendation: fix, validate defaults against the full type shape.
- Regression check: table-driven validator test for default x type matrix.
- Effort estimate: 3h-5h

#### F-FE-11: Undefined union member types are silently accepted
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:169-182; internal/parser/parser.go:561-573 (string literal and identifier both stored as bare text)
- Trigger/reproducer: `cases/49-union-undefined/main.veld` (`s: Foo | Bar` with neither defined) -> valid.
- Expected vs actual: error `undefined type Foo`. Actual: members that are not close to a known name are never reported (an error is only emitted when a Levenshtein suggestion exists).
- Impact: typos in unions become literal-string types in generated code.
- Root cause: the AST cannot distinguish `"DRAFT"` (literal) from `DRAFT` (type name) after parsing.
- Recommendation: fix, keep a literal/type flag per union member and validate type members.
- Regression check: union with unknown identifier errors; union with string literals passes.
- Effort estimate: 2h-3h

#### F-FE-12: `extends` validation misses field-override conflicts; cycle check is cubic
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:122-150
- Trigger/reproducer: `cases/24-extends-chain-override` (child `B extends A` redeclares `x: int` as `x: string`, `C extends B`) -> valid. Performance: `fe-audit/perf/chain3000.veld` (3000-model extends chain) takes 26.6 s to validate, 1000 takes 1.4 s.
- Expected vs actual: Expected a conflict error (TS `interface B extends A { x: string }` will not compile; Go flattening picks one). Actual: accepted. Self and longer cycles ARE correctly detected (cases 23, extra).
- Impact: generated TS can fail to compile; slow validation for pathological inheritance depth (not realistic).
- Root cause: no field comparison across the chain; O(n) model scan per chain step per model.
- Recommendation: fix, build a name->model map once and compare overridden field types.
- Regression check: validator test for incompatible override; benchmark with a 2000-deep chain under 100 ms.
- Effort estimate: 2h-4h

#### F-FE-13: Loader silently ignores missing import targets and falls back to loading whole directories
- Category: loader
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/loader/loader.go:246-249 (missing dir ignored), :285-355 (not-found file falls back to wildcard and `strings.Contains("model "+name)` text scan), :255/:273/:341 (self-edge in `FileImports`)
- Trigger/reproducer: `cases/22c-typo-alias-dir` (`import @modls/*`, dir does not exist) valid until a used type is missing; `cases/22b-missing-alias-file` (`import @models/nonexistent`) valid and `ast` shows `Real` loaded from the directory; `cases/21-wildcard-empty-dir` valid. A genuinely missing relative file (case 22) errors but with an absolute path and no line of the import.
- Expected vs actual: a typo'd import must be an error pointing at the import line. Actual: silent, or an unrelated directory is loaded.
- Impact: wrong files can be pulled in; errors surface later as unrelated "undefined type". Import cycles and self-imports are accepted silently (cases 19, 20); this is arguably lenient-by-design but undocumented.
- Root cause: fallback heuristics added to make `from @x import Name` work.
- Recommendation: fix, error on unresolved imports with the import line; keep the by-name lookup but make it exact (parse, not substring).
- Regression check: loader tests for missing alias dir, missing named file, and `model User` vs `model UserProfile` substring match.
- Effort estimate: 3h-5h

#### F-FE-14: `veld validate` and `veld ast` ignore config `aliases`
- Category: loader
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: cmd/veld/contract.go:39 and :70 (`loader.Parse(path)` with no aliases) vs `veld lint` at :101 and `generate` (`loader.Parse(rc.Input, rc.Aliases)`)
- Trigger/reproducer: `fe-audit/alias/` with `{"aliases":{"m":"shared"}}` and `import @m/real`: `veld validate main.veld` fails with `undefined type "Real"`, `veld lint` loads it.
- Expected vs actual: same result from every command. Actual: validate/ast disagree with lint/generate.
- Impact: a contract that generates fine is reported invalid by `validate` (CI check, editor integration).
- Root cause: `validate` resolves only the input path, not the full config.
- Recommendation: fix, route all commands through one `loadContract(rc)` helper.
- Regression check: CLI test with custom alias across validate/ast/lint/generate.
- Effort estimate: 1h-2h

#### F-FE-15: UTF-8 BOM (and UTF-16) files are rejected with a confusing message
- Category: lexer
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/loader/loader.go:90-94, internal/lexer/lexer.go:355-358
- Trigger/reproducer: `cases/08-utf8-bom/main.veld` -> `Error: lexing main.veld: line 1: unexpected character '﻿'`.
- Expected vs actual: BOM stripped. Actual: hard failure on a file that Windows Notepad / PowerShell 5 `Out-File` can produce.
- Impact: Windows users hit an invisible-character error.
- Root cause: no BOM handling.
- Recommendation: fix, strip a leading U+FEFF in loader and formatter; give a clear message for UTF-16.
- Regression check: loader test with BOM-prefixed file.
- Effort estimate: 0.5h-1h

#### F-FE-16: Success/error status literals are silently dropped when not a plain integer
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/parser/parser.go:873-875, :898-900, :1045-1050 (`if code, convErr := strconv.Atoi(...); convErr == nil`); internal/validator/validator.go:286-294
- Trigger/reproducer: `cases/31b-status-1p5` (`output: Foo:1.5`) valid, AST has no `successStatus`; `cases/57-error-status-weird` (`errors: [bad:99999, ok:200]`) valid; `Foo:0` is indistinguishable from "unset". `cases/30` (999) and `cases/60` (204 with body) are correctly rejected.
- Expected vs actual: error on non-integer or out-of-range statuses; error statuses should be 400-599.
- Impact: wrong HTTP codes silently generated.
- Root cause: ignored `Atoi` error; error statuses are never range-checked.
- Recommendation: fix, return a parse error on conversion failure and range-check `ErrorStatuses`.
- Regression check: parser tests for `:1.5`, `:99999999999`, `:0`; validator test for error status 200.
- Effort estimate: 1h-2h

#### F-FE-17: Synthetic models from inline `input/output/query/headers` collide and can contain invalid UTF-8
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/parser/parser.go:815, :851, :925, :961 (`strings.ToUpper(act.Name[:1]) + act.Name[1:]` byte slicing; the other three do not capitalise at all, so names are inconsistent)
- Trigger/reproducer: `cases/46-inline-collision-across-modules` (two modules each with `action Create { input: {..} }`) -> `error: duplicate model name: "CreateInput"` with **no file:line** (synthetic models have no Line). `cases/47-inline-headers-nonascii-action` -> valid, but AST model name is `\xef\xbf\xbd�ditHeaders`.
- Expected vs actual: inline types should be scoped per module/action, or the error should name both actions.
- Impact: common action names (`Create`, `List`) reused in two modules cannot use inline input; non-ASCII action names produce corrupted type names.
- Root cause: synthetic model names derive from action name only and are added to the global model list.
- Recommendation: fix, namespace synthetic names by module and slice by rune.
- Regression check: validator/parser tests for two modules sharing an action name with inline input and an accented action name.
- Effort estimate: 2h-4h

#### F-FE-18: Breaking-change classifier misses enums, nested models, middleware, errors and unions; status change is non-breaking
- Category: diff
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/diff/diff.go:97-191 (module/action only), :195-217 (diff only the model directly named by an action slot), :377-382 (`typeChanged` ignores `UnionTypes`), :182-188 (status -> `Added`); internal/diff/lock.go:36-40 (lock stores Modules and Models only, no Enums/Constants/Prefix)
- Trigger/reproducer: `fe-audit/fz/internal/diff_test.go` (`go test ./internal -run TestDiffScenarios -v` in `fe-audit/fz`). Results: enum member removed -> 0 changes; nested model `Inner{z:int}` -> `Inner{}` or `z:string` -> 0 changes; middleware added -> 0; errors list changed -> 0; union field added/changed -> only "field added"; status 201->200 -> 1 change classified **Added (non-breaking)**. Detected correctly: removed action, method change, path change, input field removed, input field type change, optional->required (input), new required input field, required->optional (output), output enum->string type change, action rename (remove + add).
- Expected vs actual: removal of an enum member, and any field change inside a nested model, are wire-breaking.
- Impact: even once F-FE-02 is fixed, the gate will pass real breaking changes (nested model and enum changes are the most common ones).
- Root cause: diff walks action slots one level deep and the lock omits enums.
- Recommendation: refactor, diff models/enums by name (full set) and recurse through type references.
- Regression check: the scenario table above as a permanent test with expected Breaking/Added classification.
- Effort estimate: 8h-14h

#### F-FE-19: Diff overflows the stack on a cyclic `extends` when run on an unvalidated AST
- Category: diff
- Evidence status: suspected risk
- Severity (user impact): low
- Security severity: low
- Location: internal/diff/diff.go:355-375 (`allFields` recursion without visited set); caller cmd/veld/generate.go:1126-1139 (`computePreChanges` parses but does not validate)
- Trigger/reproducer: `fe-audit/fz/internal/cyc_test.go` (`go test ./internal -run TestCyc`) -> `fatal error: stack overflow` (not recoverable). The CLI path is currently masked by F-FE-02 (lock deleted first); once F-FE-02 is fixed, a stale lock plus a cyclic contract crashes `generate` instead of reporting the validation error.
- Expected vs actual: validation error. Actual (library level): fatal runtime error.
- Impact: crash instead of diagnostic after the F-FE-02 fix; low for a local CLI.
- Root cause: diff assumes a validated AST.
- Recommendation: fix, add a visited set in `allFields` and validate before diffing.
- Regression check: diff test with `A extends A` expecting no panic.
- Effort estimate: 0.5h-1h

#### F-FE-20: Lint false positives and unreachable rules
- Category: lint
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/lint/lint.go:80-111 (`checkUnusedModels` counts only action slots and extends), :162-190 and :192-211 (duplicate-route/-action unreachable after `Validate`; route key does not normalise `:param`)
- Trigger/reproducer: `fe-audit/lintcase/m.veld` (`Out { inner: Inner }`, only `Out` used by an action) -> `unused-model: Inner`. Also the `UpdateUserInput` warning on 8 of the 9 shipped examples (the example does not use it as an input; that one is a true positive).
- Expected vs actual: models reachable through fields of used models are used.
- Impact: noise, and users are told to delete types needed by their API.
- Root cause: no transitive reachability and no `Stream`/`Emit` counting.
- Recommendation: fix, compute reachability from action slots through field types, extends, unions and maps.
- Regression check: lint test for nested model, map value, union member and stream/emit usage.
- Effort estimate: 2h-3h

#### F-FE-21: Incremental cache is bypassed by `runClean`, keyed only on `.veld` content, and misses enum/constants-only changes
- Category: cache
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/cache/cache.go:65-82; cmd/veld/generate.go:758-760 (`runClean` deletes `.veld-cache.json` even with `--incremental`), :91-113 (only Models' and Modules' source files select target modules; an enum-only or constants-only file change selects none and then marks the cache up to date)
- Trigger/reproducer: `fe-audit/inc/`: `veld generate` then edit `role.veld` (enum only) and `veld generate --incremental` -> everything regenerated (cache wiped by `runClean`), so the incremental path was not exercised via `generate`; the `watch` path calls `runClean` only at start, so the enum-only miss is reachable there but was not run (source-supported).
- Expected vs actual: cache invalidation should include config, binary version, flags and output presence. Actual: only per-file SHA-256; if `generated/` is deleted manually the next incremental run reports "no changes".
- Impact: stale output in watch mode after enum/constants edits or tool upgrades.
- Root cause: cache key is file hashes only; module-selection logic ignores enums/constants.
- Recommendation: fix, add a hash of (version, config, flags) to the cache and treat enum/constants files as global invalidators.
- Regression check: watch/incremental test that editing an enum-only file regenerates dependent modules.
- Effort estimate: 3h-5h

#### F-FE-22: "Single source of truth" language spec is not what the compiler uses; a dead second parser exists
- Category: language
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: internal/language/constants.go:29-110 (spec), internal/lexer/lexer.go:374-451 (hard-coded keyword table), internal/language/veld.go:82-200 (regex mini-parser, 0% coverage, unused)
- Trigger/reproducer: spec lists HTTP methods `HEAD`, `OPTIONS` and annotations `required, optional, primary, autoincrement, readonly, min, max, minLength, maxLength, regex` which the parser rejects (`unknown field annotation`, parser.go:679-681; `isHTTPMethod` parser.go:1145); parser accepts `serverSet` which the spec lacks. Validator treats `long, time, bytes, any, json` as primitives (validator.go:12-26) although CLAUDE.md's type table omits them, and a model named `json` or `any` is accepted and shadows the primitive (cases 53, 65).
- Expected vs actual: CLAUDE.md says internal/language is the single source of truth and CI checks generated plugin files against it; the plugins therefore advertise syntax the compiler rejects.
- Impact: editor completion/highlighting diverges from the compiler.
- Root cause: constants were never wired into lexer/parser/validator.
- Recommendation: refactor, make lexer/parser/validator derive keyword, type, method and annotation sets from `internal/language`, and delete `veld.go`/`imports.go`.
- Regression check: test that every spec annotation/method parses and every parser-accepted annotation appears in the spec.
- Effort estimate: 6h-10h

#### F-FE-23: Directive handling is lax and order-dependent
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/parser/parser.go:700-726 (module header order: description, then prefix, then baseUrl), :763-1058 (action directives assigned unconditionally)
- Trigger/reproducer: extra e15 (`output: A:200 output: A:201`) and e16 (`method: GET method: POST path: /x path: /y`) valid with last-wins; `cases/56-module-order` and e14 (`prefix` or `description` after the first action, or `description` after `prefix`) fail with `expected "action", got "prefix"`.
- Expected vs actual: duplicates should error; header directives should be accepted in any order before actions.
- Impact: silent overrides; confusing error text for reordered headers.
- Root cause: fixed-sequence header parsing and no "seen" set.
- Recommendation: fix, loop over header directives with a duplicate check.
- Regression check: parser tests for duplicate `method`/`output` and reordered module header.
- Effort estimate: 2h-3h

#### F-FE-24: Strings have no escape sequences; descriptions cannot contain a double quote
- Category: lexer
- Evidence status: design limitation
- Severity (user impact): low
- Security severity: none
- Location: internal/lexer/lexer.go:259-270
- Trigger/reproducer: `cases/48-string-escape/main.veld` (`description: "say \"hi\""`) -> `lexing main.veld: line 2: unexpected character '\\'`. Related: `cases/36-headers-unquoted-dash` (`Idempotency-Key: string`) -> `unexpected character '-'` (valid only quoted).
- Expected vs actual: `\"`, `\\`, `\n` escapes (or documented alternative); a clearer message for unquoted header names.
- Impact: documentation text and OpenAPI descriptions cannot contain quotes; emitters that embed descriptions in source must escape on their own.
- Root cause: minimal string lexer.
- Recommendation: fix, add escape handling and a hint in the header-name error.
- Regression check: lexer tests for escapes and an emitter test with quotes in a description.
- Effort estimate: 1h-2h

#### F-FE-25: Query/headers/input shapes are unconstrained
- Category: validator
- Evidence status: design limitation
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:216-247 (only name existence)
- Trigger/reproducer: `cases/35-query-array-nested` (query model with `string[]` and a nested model field) valid; `cases/55-get-with-input` (GET with `input`) valid; `cases/36b-headers-quoted-dash` header types unconstrained (`Authorization?: string` ok; a model-typed header would also pass).
- Expected vs actual: query params and headers should be scalars (or arrays of scalars with a documented encoding); GET/DELETE bodies should warn.
- Impact: emitters must decide how to serialise nested query objects; results differ per language.
- Root cause: no contract on what each slot may contain.
- Recommendation: fix, document and enforce scalar-only headers and scalar/array-of-scalar query fields; warn on GET input.
- Regression check: validator tests per slot.
- Effort estimate: 3h-5h

#### F-FE-26: Constants validation is weak and loses literal quoting
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/parser/parser.go:406-423 (TString and TNumber both stored as raw text), internal/validator/validator.go:537-577
- Trigger/reproducer: `cases/51-constants-type-mismatch/main.veld`: `A: string = 42` and `B: int = "5"` are accepted; only `C: bool = "yes"` is reported (and `bool = "true"` would pass). `uuid/date/datetime/decimal` values are never format-checked.
- Expected vs actual: type/literal mismatch errors for A and B.
- Impact: emitted constants can be wrongly typed (`const A: string = 42`).
- Root cause: AST stores `Value` without a literal kind.
- Recommendation: fix, store a literal kind and check it per declared type.
- Regression check: validator tests for mismatched literal kinds.
- Effort estimate: 2h-3h

#### F-FE-27: Workspace `consumes` has no cycle detection and `Foo` model/module name collision is not checked
- Category: validator
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/workspace.go:10-65 (self and unknown only; CLAUDE.md says "circular"), internal/validator/validator.go:203-207 (module names only checked against modules)
- Trigger/reproducer: `grep -rn -i circular cmd internal` shows no consume-cycle logic; `A consumes B, B consumes A` passes `ValidateWorkspaceConsumes`. `extra/e06.veld` (`model Foo` and `module Foo`) valid.
- Expected vs actual: documented cycle check; model/module/enum collisions in generated type/route namespaces are plausible.
- Impact: low; cycles may only matter for generation order, name collisions for emitters (not tested here).
- Root cause: checks never implemented.
- Recommendation: fix, add DFS cycle detection and a cross-kind name collision check.
- Regression check: workspace tests for A<->B and A->B->C->A.
- Effort estimate: 1h-2h

#### F-FE-28: Positions are lines only; no column/offset, so no precise range diagnostics for editors
- Category: lexer
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:195-199; internal/ast/ast.go (`Line int` fields only)
- Trigger/reproducer: every diagnostic above carries `file:line` at most; there is no column in any error. Rune vs byte vs UTF-16: lexer indexes `[]rune`, line counting is `\n` only, nothing records columns, so any future LSP/VS Code range would have to re-scan the text to get UTF-16 columns.
- Expected vs actual: editors need range diagnostics.
- Impact: blocks quality LSP and IDE work.
- Root cause: Token has no offsets.
- Recommendation: refactor, add `Offset`/`Col` to Token and `Pos` on AST nodes (see F-FE-05).
- Regression check: golden test with non-ASCII text asserting columns.
- Effort estimate: included in F-FE-05 (4h-6h extra for AST nodes)

#### F-FE-29: AST JSON emits `null` for empty collections and has a fixed version
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/ast/ast.go:5-13, internal/parser/parser.go:27
- Trigger/reproducer: `cases/01-empty` `veld ast main.veld` -> `{"astVersion":"1.0.0","models":null,"modules":null}`.
- Expected vs actual: `[]`. Actual: `null`; AST consumers (the stated pipeline contract is "only AST JSON passes between stages") must special-case null.
- Impact: low.
- Root cause: nil slices with no `omitempty`/initialisation.
- Recommendation: fix, initialise slices before encoding.
- Regression check: golden JSON for an empty file.
- Effort estimate: 0.5h

### 4. Things that work correctly (retain evidence)
- Empty file, comments-only file, CRLF, tabs, emoji in descriptions, 10,000-field model, 100,000-field model, 5 MB file, 10,000 models: all handled in under 1 s with no panic.
- No stack overflow for 2000/200000-deep `{`; parser is non-recursive.
- 1.67M fuzz executions of lexer+parser+validator+lint+diff (valid-AST path) produced no panic or hang; the lexer never panics on arbitrary bytes.
- Validator: reports all semantic errors, correct file:line on field/action errors, did-you-mean suggestions, enum default membership, circular and self inheritance detection, success-status range and 204 check, cross-module route conflicts, per-file direct-import visibility, duplicate names at every level.
- Lock file is deterministic (struct/slice JSON, sorted map keys, stable sort of changes).
- Package tests pass; coverage: lexer 61%, parser 54%, loader 30%, validator 54%, format 64%, lint 84%, diff 55%, cache 92%, language 0%.

### 5. Overall assessment
- Lexer: retain, with fixes F-FE-01, 04, 15, 24 and column tracking (F-FE-28). It is small, linear and robust.
- Parser: retain and refactor. The grammar is simple and non-recursive (safe), but needs error recovery, positions, a recursive type representation and the name-position fixes (F-FE-05, 07, 08, 16, 17, 23).
- Validator: retain; add the missing semantic checks (F-FE-09 to 12, 25 to 27).
- Formatter: rewrite (F-FE-03). Diff/gate: refactor (F-FE-02 and F-FE-18 are the highest-impact defects found). Typed errors and the language spec need wiring, not rewriting.



## Appendix: audit-track notes (C-node-typescript)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Node backend + TypeScript/framework frontend emitters: audit (Veld 6a47f81, 2026-10-21)

Working dirs: scratchpad/node-audit/e2e/{rich,v2,rsv,fw,brk,sb}. Binary: scratchpad/node-audit/veld.exe.
Nothing tracked was modified.

### 1. E2E trace log

| # | Step | Result |
|---|------|--------|
| 1 | `go build -o node-audit/veld.exe ./cmd/veld` | OK |
| 2 | `veld init` (stdin EOF) | Scaffolds veld/{app.veld,veld.config.json,models/*,modules/*}, README.md. Interactive prompts proceed on EOF. Config: backend out `../generated`, no frontend out. |
| 3 | Rich contract (8 scalars, optional, array, Map, enum @default, extends, prefix, GET+path+query, POST :201, PUT, PATCH, DELETE no output, headers, middleware, deprecated action/field, recursive model) | First attempt failed to parse: `headers: AuthHdr` with a quoted-name field model (`"X-Trace-Id": string` as a model field) gives `expected identifier`. Inline `headers: { "X-Trace-Id": string }` (the CLAUDE.md form) parses. |
| 4 | `veld generate` (config backend out `../generated`) | OK. Backend -> rich/generated, frontend -> rich/veld/generated (inside the veld source dir). No schemas/, no Zod, no errors/ dir (none declared). |
| 5 | npm i typescript@5 zod express @types/express @types/node tsx; `tsc --noEmit` strict, commonjs | **FAIL, 4 errors**: routes/items.routes.ts(85,43) TS2345 headers literal not assignable to class `SecureHeaders`; client/itemsApi.ts(30,66) and (35,61) TS2352 `SearchQuery` -> `Record<string,unknown>`; client/itemsApi.ts(60,63) TS2345 `SecureHeaders` -> `Record<string,string>`. With `--validate` a 5th: _validators.ts(55,13) TS2352. Same errors in react/vue/svelte output. |
| 6 | server.ts (express.json + Router + `itemsRouter(r, svc, mw)`) and client.ts (generated `ItemsClient`) run with tsx (tsx ignores type errors) | see below |
| 6a | GET /items/abc | 200 OK |
| 6b | GET with id `a/b?x=1#h` | **404** `Cannot GET /items/a/b`: path param not URL-encoded |
| 6c | POST create | 201 |
| 6d | DELETE | 204, SDK resolves undefined |
| 6e | PUT / PATCH | 200 |
| 6f | POST `{"i":"notanumber"}`, default build (no --validate) | **201**: no validation at all |
| 6g | same with `--validate` | 400 `{"error":"Validation failed: id: required string; ...","code":"VALIDATION_ERROR"}` (string list, not Zod issues) |
| 6h | Malformed JSON | 400 with Express HTML error page (body parser runs outside the generated try/catch) |
| 6i | GET /items/boom, /items/guarded, /items/old | **All three answered by getItem** (route shadowing). `middleware.authGuard` never ran. |
| 6j | Service throws plain Error | **500** `{"error":"db password=hunter2 at /srv/x"}` (raw message leaked). Error with .status=418,.code -> 418 `{"error":"teapot","code":"TEA"}` |
| 6k | `search({q:'a b&c=d%26e', page:2, tags:['x','y']})` | Scalar encoding correct; server received page:"2" (string) and tags:"x,y" (comma string, not array). Raw `?q=a%20b&c=d%26e` decodes correctly. |
| 6l | header action | Headers sent and mapped; with no headers sent the service is still called with undefined values and returns 200 (no check). |
| 6m | `--validate` garbage body | id:'notuuid', i:1.5, d:'notdecimal', dt:'zzz', tags:[1,2], meta:{a:1}, role:'hacker', maybe:null, extra key -> **201 accepted**. i:9007199254740993 accepted, becomes ...992. `?page=abc` -> service gets string, response assertion then 500. |

### 2. Claims verification

| Claim | Verdict | Evidence |
|-------|---------|----------|
| Zero runtime deps in generated output | **True for the default build.** Imports across types/, interfaces/, routes/, client/, index.ts, _validators.ts are relative only; no zod. `--backend-framework=express` adds `import('express')` type references (needs @types/express, types only). React/Vue/Svelte/Angular outputs import their framework (expected). | Zod is **never** generated (CLAUDE.md says Zod/Pydantic schemas are generated and lists schemas/schemas.ts: documentation mismatch). Types-only usage needs no install, but output does not compile under strict (F-NODE-06). |
| Framework agnostic (`router: any`) | **Partially.** Signature: `xRouter(router: any, service: IXService[, middleware: IMiddleware])`; calls `router.<verb>(path, [mw...], async (req:any,res:any)=>{})` and assumes Express semantics: req.params, req.query (pre-parsed), req.body (pre-parsed, needs express.json()), req.headers[lowercase], res.status(n).json(), res.status(204).end(), res.json(). | Fastify (reply.code().send(), no res.status) and Hono (c.json(), single context arg) will not work; Koa neither. It is Express-shaped, not agnostic. Middleware type is (req,res,next). Positive: no hard framework import in the default build. |
| Deterministic | **True.** Two generations into separate dirs: `diff -r` identical. Map ranges feed sorted keys or ordered outer loops (sort.Strings in validate.go:422, sortedStringKeys sdk.go:468, helpers.go:432 first-wins on ordered module loop). | Caveat: every file is rewritten each run (mtime changes even if content identical) because generate wipes and rewrites. |
| Output safety ("never writes outside --out", "user files never touched") | **False.** | F-NODE-01..04, 16, 17. |

Other checks:
- Type mapping: int/float -> number (no unsafe-int guard), decimal -> branded string, date/datetime/uuid -> branded strings (UUID, Decimal, ISODate, ISODateTime in types/_scalars.ts), enum -> string-literal union (Angular emits a real `enum`), Map -> Record<string,V>, extends -> `class X extends Base`, recursive `Node2 { children: Node2[] }` generates and terminates (no Zod, so no z.lazy question; validator recurses by call).
- Optional vs nullable: `maybe?: string` type excludes null but validator accepts null.
- Status codes: POST->201 (including POST with output Node2), DELETE no output->204, `Type:201` honored.
- SDK: VeldApiError(status, body: string, code?); parseErrorResponse gives code '' when the body has none. 204 handled; 200 with empty body would throw in res.json(). No timeout/AbortSignal, no fetch injection, only static `headers` in config (no per-request/async token hook). Content-Type: application/json is sent on GET too. DELETE sends body `{}`. resolveBase is browser-safe (globalThis.process?.env). `export const api = new VeldApiClient()` constructs at import.
- GET with `input` model: SDK serializes input into query, route passes req.query (consistent with the recent fix) but values are strings typed as the model.
- Reserved names: F-NODE-14. Actions named delete/get/new yield legal members and work.
- Per-framework frontends (all emitted into the same out dir as the TS SDK; backend also emitted because backend runs too):
  - react: core hooks import `react`; the other file imports `@tanstack/react-query`; both wrap client/itemsApi (import the TS SDK). Compiles with deps except the shared F-NODE-06 errors.
  - vue: imports `vue`; wraps TS SDK; same result. svelte: imports `svelte/store`; wraps TS SDK; same.
  - angular: **does not import the TS SDK**, uses HttpClient + rxjs; broken (F-NODE-15).
  - javascript: CJS `require`, but still emits types/*.ts, index.ts, routes/*.ts, interfaces/*.ts (mixed JS/TS). types-only: types plus leftover backend files.
  - The full client/ directory is duplicated into every framework output.

### 3. Findings

#### F-NODE-01: `veld generate` deletes the whole output directory (including user files) before every run
- Category: generate-cli
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: cmd/veld/generate.go:40-50 (runClean), 757-760 (call); comment at 141-142 claims the opposite
- Trigger/reproducer: put my-notes.ts, usersub/a.txt, routes/stale.routes.ts in the out dir; `veld generate --force --out=./o1`. All gone.
- Expected vs actual: "user files are never touched; use `veld clean`" vs `os.RemoveAll(outDir)` on every non-dry-run generate (also workspace and watch).
- Impact: hand-added files, node_modules, co-located code in the out dir are lost; all mtimes reset, defeating tsc/vite incremental caches.
- Root cause: runClean invoked unconditionally; no manifest of generated files.
- Recommendation: refactor: write a manifest of emitted files and delete only stale manifest entries, never RemoveAll.
- Regression check: foreign file in out dir survives two generates; a removed module's file is deleted.
- Effort estimate: 6h-10h

#### F-NODE-02: `--out` pointing at the project/source directory deletes the contract and sources
- Category: output-safety
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: internal/config/config.go:375 (only rejects "..", ".", "./", "../."), cmd/veld/generate.go:40
- Trigger/reproducer: sandbox sb/proj containing app.veld and notes.txt: `veld generate --input=app.veld --out=../proj`. On Windows RemoveAll partially failed (cwd in use) but app.veld and notes.txt were deleted, then the run errored "reading app.veld". Unix would remove everything. `--out=.`/`..` are refused. Root (`/`) deliberately not tested.
- Expected vs actual: refuse an out dir that contains the input, config, cwd or a filesystem root vs lexical check only.
- Impact: irreversible deletion of the user's source of truth; with F-NODE-01 any `--out=src` wipes code.
- Root cause: textual validation; recursive clean.
- Recommendation: fix: resolve to absolute and refuse if out is an ancestor-or-equal of cwd/config dir/input file or a root; fix F-NODE-01.
- Regression check: tests for out equal to cwd, parent, input's dir, symlinks.
- Effort estimate: 2h-3h

#### F-NODE-03: A failing generate leaves the previous output deleted; watch does this on every save
- Category: watch
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: cmd/veld/generate.go:757-760 (clean before parse), cmd/veld/watch.go:270
- Trigger/reproducer: generate OK, append garbage to the contract, regenerate: parse error printed and the out dir no longer exists. In `veld watch`, saving a half-edited contract wipes generated code (debounce is 300ms; errors are printed and the loop continues, which is good), breaking the dev server until the contract is valid again.
- Expected vs actual: failed run is a no-op vs destructive.
- Impact: dev-loop breakage; CI leaves an empty out dir after a failure.
- Root cause: clean precedes parse; no staging directory.
- Recommendation: refactor: emit to a temp dir and swap on success only.
- Regression check: broken contract leaves previous output byte-identical.
- Effort estimate: 4h-8h (shared with F-NODE-01)

#### F-NODE-04: Breaking-change gate and `--strict` never fire (lock deleted before the diff)
- Category: generate-cli
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: cmd/veld/generate.go:50 (diff.DeleteLock inside runClean) vs :205 (hasLock)
- Trigger/reproducer: generate; remove action DeleteItem; `veld generate --strict` -> exit 0, nothing reported; lock re-saved without the action.
- Expected vs actual: exit 1 with breaking changes (CLAUDE.md "Contract Safety Features") vs silent success.
- Impact: the advertised CI protection is non-functional.
- Root cause: runClean removes .veld.lock.json and .veld-cache.json ahead of the diff.
- Recommendation: fix: stop deleting the lock in generate/watch.
- Regression check: end-to-end: remove an action, run `generate --strict`, expect exit 1.
- Effort estimate: 1h-2h

#### F-NODE-05: Static routes shadowed by earlier parametric routes (including middleware-protected ones)
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Location: internal/emitter/backend/node/routes.go:75-87 (actions emitted in declaration order)
- Trigger/reproducer: `GET /items/:id` declared before `GET /items/boom`, `/items/guarded` (middleware AuthGuard), `/items/old`. Requests to each are answered by getItem; authGuard never invoked.
- Expected vs actual: each action reached by its own handler vs later same-method static paths unreachable and answered by another action without the middleware.
- Impact: dead endpoints; protected action silently replaced by an unprotected one.
- Root cause: no specificity ordering and no validator/lint rule for overlapping routes (lint duplicate-route does not catch it).
- Recommendation: fix: emit static-segment routes before parametric ones per method and add a validator error for ambiguous overlap.
- Regression check: `/:id` then `/me` reaches the right handler under express.
- Effort estimate: 3h-5h

#### F-NODE-06: Generated output does not compile under `tsc --strict`
- Category: emitter-ts
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: frontend/typescript/client.go (`query as Record<string, unknown>`, `extraHeaders: Record<string,string>`), backend/node/routes.go (headers object literal)
- Trigger/reproducer: any contract with a `query:` model or `headers:` block; `tsc --noEmit --strict` (errors in section 1 step 5). Every framework wrapper inherits them.
- Expected vs actual: clean compile vs 4 errors (5 with --validate).
- Impact: users must disable strict or edit generated files.
- Root cause: models are classes with methods, so literals/instances are not assignable to or from Record types; no test compiles the output.
- Recommendation: fix: emit plain interfaces (or cast via unknown) and add a tsc compile test.
- Regression check: CI generates the rich contract and runs tsc --strict --noEmit.
- Effort estimate: 1h-2h (casts) or 6h-10h (interface migration)

#### F-NODE-07: Models are classes with getters/setters and branded scalars
- Category: emitter-ts
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/emitter/backend/node/types.go, tshelpers/tshelpers.go, types/_scalars.ts
- Trigger/reproducer: `createItem({ s: 'x', ... })` is a type error (needs full `Everything` instance with getS()/setS()/toJSON()); uuid/decimal/date fields need `as UUID` casts. JSON responses are plain objects typed as the class, so methods do not exist at runtime (same mechanism as the reproduced headers error).
- Expected vs actual: CLAUDE.md says "types/interfaces"; actual class-per-model, 147 lines for 3 models.
- Impact: typing lies (res.getS() compiles, throws) and ergonomic cost.
- Root cause: class-per-model design.
- Recommendation: refactor: emit interfaces; make branded scalars optional.
- Regression check: object-literal call compiles.
- Effort estimate: 6h-10h

#### F-NODE-08: No input validation unless `--validate`; documented Zod/schemas do not exist
- Category: emitter-node
- Evidence status: documentation mismatch
- Severity (user impact): medium
- Security severity: medium
- Location: backend/node/routes.go:105-110, CLAUDE.md "Route handler features"/"Generated Output Structure"
- Trigger/reproducer: POST `{"i":"notanumber"}` on default build -> 201, service receives it.
- Expected vs actual: "Zod .parse(); ZodError -> 400" vs raw req.body.
- Impact: services trust unvalidated input by default.
- Root cause: validation moved to opt-in zero-dep validators; docs stale.
- Recommendation: refactor: make validation default-on (or document opt-in) and update CLAUDE.md.
- Regression check: default generate emits _validators.ts; docs aligned.
- Effort estimate: 2h-4h

#### F-NODE-09: `--validate` validators are shallow
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: backend/node/validate.go:110-300
- Trigger/reproducer: accepted: role:'hacker' (enum), tags:[1,2], meta:{a:1}, i:1.5, i:9007199254740993, bad uuid/date/decimal, maybe:null, unknown extra keys passed through. @default not applied; missing required headers not checked; GET query values are strings typed as numbers; `?page=abc` becomes a 500 via output assertion. Recursion has no depth cap (deep JSON bounded only by body-size limit; not tested, suspected risk).
- Expected vs actual: runtime checks matching the TS types vs typeof-only checks.
- Impact: contract holes; prototype-ish keys in `meta` pass through.
- Root cause: validators check typeof only.
- Recommendation: refactor: validate enums, element types, integers, formats, optional/null, coerce query.
- Regression check: table-driven bad values per scalar.
- Effort estimate: 8h-14h

#### F-NODE-10: Error handler leaks internal messages on 500 and mixes error formats
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: backend/node/routes.go:185-191
- Trigger/reproducer: service throws Error('db password=... at /srv/x') -> 500 body has the message. `err.status` from any thrown object is trusted. Malformed JSON bypasses it (Express HTML page). The identical catch block is repeated per action.
- Expected vs actual: generic 500 body vs raw message.
- Impact: information disclosure; inconsistent error contract.
- Root cause: catch copies err.message regardless of status.
- Recommendation: fix: generic message for status >= 500 and one shared handleError.
- Regression check: 500 body is constant.
- Effort estimate: 2h-3h

#### F-NODE-11: Path parameters are not URL-encoded in the SDK
- Category: emitter-ts
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: internal/emitter/helpers.go:25 (ToTemplateLiteral), frontend/typescript/client.go (urlExpr)
- Trigger/reproducer: `getItem('a/b?x=1#h')` requested /items/a/b (404); '../admin' would traverse.
- Expected vs actual: encodeURIComponent(id) vs raw interpolation.
- Impact: IDs with reserved chars break; client-side path injection when IDs come from user input. Angular has the same pattern.
- Root cause: `${id}` emitted directly.
- Recommendation: fix: emit `${encodeURIComponent(id)}`.
- Regression check: SDK test with ids containing / ? # %.
- Effort estimate: 1h-2h

#### F-NODE-12: Query serialization is lossy
- Category: emitter-ts
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: frontend/typescript/helpers.go (buildQueryString)
- Trigger/reproducer: `tags:['x','y']` -> `tags=x%2Cy`; server gets string "x,y", not an array; nested objects become "[object Object]".
- Expected vs actual: repeated keys or defined format vs String(value).
- Impact: array query fields unusable; server type says string[].
- Root cause: String(value) and no matching parse on routes.
- Recommendation: fix: repeat keys for arrays and normalize on the route side.
- Regression check: round-trip array/bool/int through SDK and route.
- Effort estimate: 3h-5h

#### F-NODE-13: SDK lacks timeout/abort, fetch injection and dynamic auth hook
- Category: emitter-ts
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: low
- Location: frontend/typescript/client.go:35-45, helpers.go (VeldClientConfig)
- Trigger/reproducer: config is {baseUrl, headers} only; no signal/timeout/fetch/token callback/retries; Content-Type sent on GET; DELETE sends `{}`; empty 200 body throws; VeldApiError.body is a raw string; `code` is '' not undefined when absent.
- Expected vs actual: production-ready client vs minimal wrapper.
- Impact: users wrap or fork generated code.
- Root cause: minimal request helper.
- Recommendation: refactor: extend config with fetch, headers callback, timeoutMs, per-call RequestInit.
- Regression check: SDK tests with fake fetch.
- Effort estimate: 4h-6h

#### F-NODE-14: Module named `index` clobbers types barrel; JS-global model names break output
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: backend/node/types.go / barrel.go (file naming); no reserved-name check in the validator
- Trigger/reproducer: `module index {...}` writes types/index.ts twice; final content `export * from './index'` (self-reference), all types lost; tsc: no exported member Date/Error/Object/Map/Promise. Model `Promise` yields `Promise<Promise>`.
- Expected vs actual: validation error or collision-safe naming.
- Impact: cryptic failures from valid-looking contracts.
- Root cause: file names derived from module names without reserved list; no shadowing check.
- Recommendation: fix: validator rule rejecting reserved module/model names, or suffix generated files.
- Regression check: module `index` / model `Date` yields a validation error.
- Effort estimate: 2h-4h

#### F-NODE-15: Angular emitter output is invalid and incomplete
- Category: emitter-angular
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: frontend/angular/emitter.go (models ~L110, service generation)
- Trigger/reproducer: generate --frontend=angular with the rich contract: models/index.ts contains `X-Trace-Id: string;` (TS1131/TS1128/TS1005); references UUID/Decimal/ISODate which are not defined or imported there; `secure(headers)` accepts headers but never passes them to http.get; baseUrl hard-coded ''; no VeldApiError; path params unencoded.
- Expected vs actual: working service vs compile failure and silently dropped headers.
- Impact: Angular target unusable with headers or branded scalars.
- Root cause: independent reimplementation that shares neither the TS SDK nor type emitter.
- Recommendation: refactor: reuse the TS type emitter and quote non-identifier keys.
- Regression check: compile Angular output against @angular/core@17 in CI.
- Effort estimate: 6h-10h

#### F-NODE-16: `--out`/`--input` resolution is inconsistent and can write outside the project
- Category: generate-cli
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: low
- Location: internal/config/config.go (BuildResolved), cmd/veld/generate.go
- Trigger/reproducer: (a) config in rich/veld/ with backend out "../generated": plain `veld generate` -> rich/generated; adding `--input=veld/app.veld` -> backend printed as "./../generated" and written relative to cwd = e2e/generated, outside the project. (b) `--out=./gen3` with that config changed only the frontend (rich/veld/gen3); the config backend out wins silently. (c) Scaffolded config puts frontend output inside veld/ next to sources.
- Expected vs actual: documented precedence (flags > config) and one path base.
- Impact: surprising locations; destructive with F-NODE-01.
- Root cause: ConfigDir vs cwd base differs per path; split-output precedence.
- Recommendation: fix: resolve relative paths against one base and print absolute resolved paths.
- Regression check: flag precedence and `--input` with config in a subdir.
- Effort estimate: 3h-5h

#### F-NODE-17: Writes are non-atomic, no manifest, stale files handled by "delete everything"
- Category: output-safety
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: low
- Location: emitters call os.WriteFile directly (backend/node/routes.go, frontend/typescript/emitter.go:65); no manifest anywhere
- Trigger/reproducer: a failure midway (read-only-dir simulation not run) leaves a partial tree; with F-NODE-01 the prior good tree is already gone.
- Expected vs actual: temp+rename; manifest-based cleanup.
- Impact: torn output after crashes or Ctrl-C.
- Root cause: no output abstraction (codegen package exists but node/ts do not use it).
- Recommendation: refactor: route all writes through a staged writer.
- Regression check: inject write failure; old tree intact.
- Effort estimate: 6h-10h (overlaps F-NODE-01/03)

#### F-NODE-18: Doc/UX mismatches in CLI output and CLAUDE.md
- Category: generate-cli
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/generate.go (import-instructions block near the end), CLAUDE.md
- Trigger/reproducer: "Import instructions" always print users/User examples even with no such module; generated README lists modules only; CLAUDE.md output tree (schemas/, auth.routes.ts, Zod) stale; `--backend=none` is not a valid value and silently fell back to node.
- Expected vs actual: accurate instructions.
- Impact: new-user confusion.
- Root cause: static templates.
- Recommendation: fix: derive instructions from the AST and refresh CLAUDE.md.
- Regression check: snapshot of instructions for a contract without `users`.
- Effort estimate: 2h-3h

#### F-NODE-19: Tests never compile or run the generated TypeScript
- Category: emitter-node
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/emitter/**/*_test.go
- Trigger/reproducer: no `tsc`/`exec.Command`/node in any emitter test; 210 `func Test` total (node 10, typescript 6, react 5), all substring assertions on text.
- Expected vs actual: compile tests would have caught F-NODE-06 and 15.
- Impact: regressions in generated code ship unnoticed.
- Root cause: string-contains test style.
- Recommendation: fix: add golden files plus a `tsc --strict` smoke job per target.
- Regression check: the CI job described.
- Effort estimate: 6h-10h

### 4. Emitter quality assessment

- Sizes (Go lines): backend/node 2071 total (sdk.go 475, validate.go 427, routes.go 324); frontend/typescript 950 (emitter.go 324, client.go 247); frontend/javascript 779; react emitter 453; angular 220; emitter/helpers.go 744. Manageable, but there are no templates: everything is sb.WriteString(fmt.Sprintf(...)); the identical catch block is repeated per action.
- Duplication: node-ts / node-js / TS client / JS client / Angular / server-client each re-implement request helpers, query building, path interpolation and type mapping (Angular does not reuse the type emitter). The `codegen` and `lang/*` helper packages are used by go/rust but not by node/typescript.
- Adding a new backend: implement Emit, Summary, the IsBackend marker and EmitServiceSdk (mandatory even for plain backends), register via init() and a blank import in main.go. Frontend: Emit, Summary, IsFrontend. The plumbing is small and clean; the real cost is type mapping and route emission.
- Tests: 210 test functions under internal/emitter; style is strings.Contains on output; no golden files, no compile of generated TypeScript.
- Architecture: contract -> AST -> emitter isolation is sound and output is deterministic. Risk is concentrated in generated-code semantics (route order, validation depth, class-based models, SDK ergonomics, Angular) and in the CLI's destructive write strategy, not in the pipeline.



## Appendix: audit-track notes (D-multi-emitter)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Multi-emitter audit (non-Node emitters + auxiliary generators)
Repo D:\University\Veld @ 6a47f81. Audit date 2026-10-21. Scratch: <scratchpad>/multi-audit (contract/, out/, py/, ws/, aux/).
Environment limits (not defects): no cargo, dotnet, php, psql, Maven/Gradle, Dart/Kotlin/Swift compilers. Those emitters were judged by reading only, plus `javac` for Java (with stub Jackson annotations). The command-line `curl` is blocked in this sandbox; HTTP calls were made with a Python urllib script (multi-audit/py/hit.py).

### 1. Run log
Shared contract: multi-audit/contract/{app.veld, models/all.veld, modules/items.veld} (all scalars, optional, array, Map, enum @default, extends, recursive Node, GET path+query, POST 201 + headers + middleware, PUT, PATCH, DELETE no output, @deprecated action+field). `veld validate` OK. Note: imports must be repeated in each module file that uses a type (a model imported only from app.veld is rejected with "defined in all.veld but not imported").
Build: `go build -o multi-audit/veld.exe ./cmd/veld` OK. `go test ./internal/emitter/... ./internal/generators/...` all PASS (tests only assert substrings; see F-MULTI-02).
Generate: `veld.exe generate --backend=<t> --frontend=none --out=../out/<t>` for python, go, rust, java, csharp, php, node-js; `--backend=none` is rejected (frontends generated with `--backend=node-ts --frontend=X`).

#### Part A Python (venv, flask + pydantic installed)
- `generate --backend=python` (default, no framework, no --validate): `python -m compileall` -> SyntaxError routes/items_routes.py:41 `input = CreateItemSchema(**body = data  # CreateItemSchema).model_dump()`. Also: routes file lacks `from flask import request`; imports `..schemas.schemas`, which is never emitted (no schemas/ dir, no Pydantic output despite CLAUDE.md); routes are never registered (only a comment `# register: ['GET'] /api/items/<id>`); error path does `raise RuntimeError(body)` rather than returning a response.
- `generate --backend=python --backend-framework=flask --validate`: compileall -> SyntaxError models/items.py:218 `def __init__(self, idempotency-_key: str ...` (header name "Idempotency-Key" used as identifier). Importing routes fails because routes -> interfaces -> models.
- After hand-patching only that identifier (in a copy, py/pkg) the app ran (port 5055). Results:
  - GET /api/items/abc?limit=5&q=x -> first run 500 `name 'Decimal' is not defined` (_validators.py uses Decimal, never imports it). After adding the import: 200; the service received `{'limit':'5','q':'x'}`, a plain dict of strings (not ItemQuery, limit not int). Decimal output serialised as string "1.10" by Flask >= 2.2 jsonify.
  - POST valid -> 201; service got headers `{}` (route has comment "Extracted from request headers" but extracts nothing; Idempotency-Key/Authorization never reach the service).
  - POST {"name":5} -> 400 {"code":"VALIDATION_ERROR",...} good. POST role:"bogus" -> 201 (enum not validated). POST non-JSON body -> HTTP 500 (Flask BadRequest caught by `except Exception`; body code field says 400).
  - PUT where service raises ValueError -> 500 {"error":"boom"} (all exceptions caught, message leaked to client). PATCH where service returns None -> 500 contract violation. DELETE -> 204. Static routes /old, /tree after /<id>: fine in Flask.
  - The ABC declares `-> Item` (class with get_/set_ methods) but routes require a `dict` (assert_item) and jsonify cannot serialise Item objects: a service honouring the interface type fails at runtime.
  - Model classes use snake_case attributes (created_at) while JSON/validators use camelCase (createdAt), so `from_dict(cls(**data))` breaks on wire input. Mutable default args (`tags: List[str] = []`).
  - date/datetime/uuid are plain `str`, no format validation. Recursive Node: assert_node recurses fine.

#### Part B Go
- Output ships `go.mod` (module example.com/veld-generated, go 1.22, stdlib only for default; chi adds a require without go.sum).
- `go build ./...` / `go vet ./...` on raw output: FAIL, internal/models/items.go:184 `Idempotency-key string` (syntax). After renaming the field in a copy (out/go2): FAIL `internal/routes/items.go:14:2: undefined: mux` (function param is `r *http.ServeMux` but body calls `mux.HandleFunc`, from strategy/plain.go:21). After sed `mux`->`r`: builds and vets clean; server.go compiles.
- Ran a ~30-line main implementing ItemsService: GET path param OK (200); POST valid -> 201; DELETE -> 204; invalid JSON -> 400 {"error":"invalid request body"}; `{}` body -> 201 (NO validation of required fields, enums, formats; plain json.Decoder only); role:"bogus" accepted; service error -> 500 with raw err.Error() (no typed status mapping); query `*ItemQuery` and headers `*CreateItemHeaders` are always `nil` (`svc.GetItem(r.Context(), id, nil)`).
- Mappings: decimal -> string; date AND datetime -> time.Time (a date "2020-01-01" fails decode: `parsing time "2020-01-01" as "2006-01-02T15:04:05Z07:00"`); int -> int (not int64); optional -> *T `omitempty`; Map -> map[string]string; enum -> `type Role string` + consts; extends -> embedded struct (but `NewItem` constructor omits Base fields); nil slices serialise as JSON null; recursive Node -> []Node OK.
- `--backend-framework=chi`: go.mod requires chi v5.0.12, no go.sum; `go mod tidy` (network OK) succeeds and build passes (after header rename). Go 1.22 mux resolves /old vs /{id} correctly.

#### Part C
Static inspection plus javac. See table in section 2 and F-MULTI-06..09. Java: `javac` (stub Jackson annotations) on raw output: 8 syntax errors (CreateItemHeaders hyphen); after renaming: 2 more (`Item.java:22 private Role role = "user"` String->Role; `ItemsController.java:94 missing return statement` in deleteItem). Node JS backend: `node --check types/items.js` SyntaxError line 141 `this.Idempotency-Key = ...`. Rust/C#/PHP/Dart/Kotlin/Swift read by eye; the same hyphen-identifier bug appears in all (root cause parser.go:961 builds synthetic `<Action>Headers` model with raw header names as field names).
- docs/architecture/overview.md:37 says "Actix handlers" for Rust; code is Axum-oriented (rust/errors.go:20 `use axum::`, middleware imports axum) but the emitted Cargo.toml does NOT list axum and the router is a stub. CLAUDE.md "Axum" is the closer description. docs/EXECUTION_PLAN_v1.md itself schedules "C1 axum / make it compile".

#### Part D Service SDKs
Workspace ws/veld/veld.config.json: iam (node-ts, rich contract), txn (python/flask, consumes iam), cards (go, consumes iam,txn), gw (node-ts, consumes iam,txn).
- `veld deps` prints graph (txn->iam, cards->iam,txn); `deps --validate` "Workspace dependencies valid". `generate` -> sdk/ dirs for every consumer.
- WebSocket: syntax exists (`method: WS` with required `stream: T`, optional `emit: T`; validator.go:252). CLAUDE.md lists methods GET POST PUT DELETE PATCH only; language/constants.go also lists HEAD, OPTIONS, WS. The WS action `Live` is absent from all SDKs (skipped as claimed; sdk.go:243/364/204).
- Python SDK: compileall OK, imports OK. Env var: ctor arg > `VELD_IAM_URL` > baked "http://iam:3001" (verified). Error type `VeldApiError(status, body)` verified against a fake server (404 -> status 404). BUT typed model args do not work: `create_item(CreateItem(...))` -> TypeError "not JSON serializable"; `get_item("a", ItemQuery(limit=3))` -> AttributeError 'ItemQuery' has no attribute 'items'; only raw dicts work. Header params ignored. `urlopen` without timeout.
- Go SDK: `go build ./sdk/...` and `go vet` clean. Precedence arg > env > baked. `*VeldApiError`. No query-param support (GetItem has no query arg); http.DefaultClient without timeout; inheritance flattened as documented.
- TS SDK (gw, tsc 5 --strict): 24 errors: sdk/shared/index.ts uses UUID, ISODateTime, ISODate, Decimal without importing `_scalars` (22 errors), `role!: Role = "user"` TS1263, `query as Record<string,unknown>` TS2352 (iam/client.ts:44), TS2322 in models. Env var `process.env.VELD_IAM_URL` precedence correct.

#### Part E Auxiliary generators
- OpenAPI: version 3.0.3. `redocly lint` 1.25.0: structurally valid; 10 errors are recommended-ruleset (no servers, trailing-slash path `/api/items/`, no `security`) plus 2 unused-component warnings. Representation: decimal = string/format decimal; date/date-time/uuid formats; optional omitted from required; enum $ref Role (default NOT emitted); extends = allOf [Base,{...}] with `required` beside allOf; path param + query model flattened into parameters; header model -> header params; 201 honoured; DELETE no output -> 204; every op gets 400/500 ErrorResponse ({code:int,message}, which does not match runtime shape {error}); middleware yields bare 401/403 but no securitySchemes; no servers although baseUrl exists; CreateItemHeaders/ItemQuery leak as unused schemas.
- GraphQL: `buildSchema` (graphql@16.8.1) FAILS: `Idempotency-Key: String!` invalid name. After renaming it parses. Map -> `meta: String` (lossy), uuid/date -> String (no ID/custom scalars), GET -> Query, POST/PUT/PATCH/DELETE -> Mutation, extends flattened, `Base` emitted as stand-alone type.
- Schema SQL (Postgres dialect; psql unavailable, judged by reading): every model including DTOs (ItemQuery, CreateItem, Msg, headers) becomes a table; `limit INTEGER` unquoted reserved word; `DEFAULT user` unquoted (in PostgreSQL this is current_user, silently wrong); arrays/maps as `VARCHAR(255)`; hyphen column `idempotency-_key`; pluralisation `item_querys`, `create_item_headerss`; extends flattened, no FKs. Prisma: models lacking `@id` (invalid), `@default(user)` unquoted (invalid), `Idempotency-Key String` invalid, Map as `String`.
- Docs: `veld docs` -> HTML by default, `--format=markdown` supported; markdown has method table, models, enums; `internal/docsgen` 1864 LOC, 0 tests; graphqlgen 0 tests.
- Mock: `internal/mock` (284 LOC, 12 tests) is imported by nothing outside itself; there is no `veld mock` command (`unknown command "mock"`).
- Dockerfile: Node template assumes user-owned package.json, `npm ci` (needs lockfile), `npm run build --if-present`, copies /app/dist (may not exist), `CMD ["node","index.js"]` (nonexistent entry unless user creates one), runs as root, downloads unpinned, unverified binary `.../releases/latest/download/veld-linux-amd64` which returns HTTP 404 (verified; the release publishes `.tar.gz` assets). docker-compose.yml bundles an unrelated postgres service named `registry` (password `changeme`, port 5432 published). Language detection is by marker files in the --out dir (go.mod, Cargo.toml, requirements.txt, package.json); default Node when none.
- CI workflow: same 404 install URL (fails at first step); npm-only unless marker files exist. Env: `.env.example` generic (PORT, DATABASE_URL, NODE_ENV) and not derived from the contract.
- `veld generate` post-generation hints print hardcoded names (`UserSchema`, `IUsersService`, `from pyflask.models import User`, `maayn.veld.generated.services.IUsersService`) unrelated to the contract.

### 2. Part C comparison table
| Emitter | Files emitted | Framework assumed | Runtime deps | Validation | Status codes | Path | Query | Headers | decimal/date/uuid | Optional | Enum | extends | Map | Recursive | Compile/syntax errors found | LOC (non-test) | Tests |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| node (ref) | types/interfaces/routes/schemas/client | any router | zod only if used | Zod / --validate | 201/204/overridable | yes | yes | req.headers | string/string/string | `?` | union | interface extends | Record | yes | not audited here | 1774 | 10 |
| python (flask) | models/items.py, interfaces/i_*.py, routes/*_routes.py, middleware, _validators.py (--validate), README | Flask (no registration without --validate) | flask; stdlib validators | only with --validate (shape check, enum NOT checked); default path references nonexistent Pydantic schemas | 201, 204; exceptions -> status attr or 500 | yes | untyped dict of strings | NOT extracted (empty dict) | Decimal / str / str | Optional | Literal (not validated) | class inheritance (snake/camel mismatch) | Dict | yes | models/items.py:218 (hyphen); routes:41 SyntaxError (default); Decimal NameError in validators | 2632 | 10 |
| go | go.mod, server.go, main.go, internal/{models,interfaces,routes,middleware} | stdlib net/http (chi opt-in) | none (chi opt-in) | none (JSON decode only; 400 on malformed) | 201/204/200; service errors always 500 | yes (PathValue) | passed nil | passed nil | string / time.Time / time.Time (date breaks) | *T omitempty | string type + consts | embedded struct | map[string]string | []Node | items.go:184 hyphen; routes/items.go:14 `mux` undefined | 2061 | 9 |
| rust | Cargo.toml, src/{lib,main,router,services,errors,items/mod,items_middleware,models/*}.rs | Axum (docs/architecture/overview.md says Actix: wrong) | axum used in code but NOT in Cargo.toml | none | handler return type mismatches trait return | Path<String> | no | `CreateItemHeaders::default()` (no Default derive) | String / String / String | Option | enum OK but `fn item_default_role() -> Role { String::from("user") }` mismatch | struct DROPS parent fields | HashMap | Vec<Node> OK | router() returns `()` stub, main empty, hyphen field, missing axum imports in items/mod.rs | 1991 | 8 (rust_test.go:160 asserts the stub) |
| java | build.gradle, models/**, services/, controllers/, errors/, middleware/ | none (plain class; docs say Spring Boot) | Jackson not declared in build.gradle; LocalDate needs jsr310 | none | comments only (`// 201 Created`) | method params | query model arg, not bound | headers model arg, not bound | BigDecimal / LocalDate / UUID | boxed types | enum with @JsonValue | `extends Base` | Map | List<Node> OK | CreateItemHeaders hyphen; `Role role = "user"`; deleteItem missing return; package hardcoded `maayn.veld.generated` | 2576 | 12 |
| csharp | csproj, Controllers, Models, Services, Errors, Middleware | ASP.NET (not referenced in csproj) | none declared | none | `Task<object>`, no attributes | param name `Id` | Dictionary | `[FromHeader]` model | decimal / DateTime / Guid | `T?` | enum + JsonStringEnumConverter (EnumMember ignored by System.Text.Json: wire "Admin" not "admin") | `: Base` | Dictionary | List<Node> OK | `Idempotency-Key` property; `Role Role = "user"`; controller lacks `using Microsoft.AspNetCore.Mvc`, ControllerBase, verb attributes | 1761 | 12 |
| php | composer.json, routes/api.php, app/{Http,Models,Services,Middleware,Exceptions} | Laravel (no dependency declared) | none declared | `$request->validate(...)` called on `array $request` | comments only | yes | `$request->query()` on array | `$request->headers->all()` on array | string / string / string | `?T = null` before required params | backed enum | `extends Base` (parent ctor not called) | array | array | `array $request` has no methods (fatal); `Role $role = 'user'` fatal; `idempotency-_key` param; wrong `use App\Models\CreateItem` (actual App\Models\Items\CreateItem); `/old`, `/tree` registered after `{id}` so shadowed | 1602 | 10 |
| javascript (backend) | package.json, index.js, routes/*.js, types/*.js, interfaces, middleware | any router | none | none | 201, 204 | yes | req.query | req.headers | string | undefined | JSDoc union | class extends | {} | OK | types/items.js:141 `this.Idempotency-Key =` SyntaxError; `:id` before `/old` shadowing | 1911 | 9 |
| dart | client/api_client.dart, pubspec.yaml | dart:io HttpClient | none | none | >=400 -> VeldApiError(status, body) | interpolated | query string built unencoded | param accepted but unused | String / String / String | `T?` | enum, but fromJson assigns String to enum (runtime TypeError) | flattened | Map assigned from dynamic (runtime error) | List<Node> from dynamic (runtime error) | `String Idempotency-Key` hyphen | 465 | 6 |
| kotlin | client/ApiClient.kt, build.gradle.kts | java.net.http + kotlinx.serialization | kotlinx-serialization (declared) | none | >=400 -> VeldApiError | `${id}` | "omitted for brevity" (client.go:107) | accepted, unused | `BigDecimal` in @Serializable without serializer (compile error) / String / String | `T? = null` | enum + @SerialName | flattened | Map | List<Node> OK | `val Idempotency-Key`; BigDecimal serializer | 420 | 6 |
| swift | client/APIClient.swift, Package.swift | URLSession | none | none | >=400 -> VeldApiError | BUG: `"/api/items/${id}"` (Swift needs `\(id)`; literal "${id}" sent) | accepted, unused | no | Decimal (JSONDecoder cannot decode "1.10" string into Decimal) / String / String | `T?` | String raw enum | flattened | [String:String] | [Node] OK | `var Idempotency-Key`; types are internal (no `public`) so unusable from a consuming module | 395 | 6 |

### 3. Divergence between emitters
- Validation: Node (Zod/--validate) and Python (--validate shape checks, no enum check) validate; Go/Java/C#/PHP/Rust/Kotlin/Swift/Dart do none usable (Java/C# emit validation helper files that are not wired in). CLAUDE.md "full input validation" holds only for Node/Python.
- Status codes: Node/Python/Go/JS set 201/204; Java/C# only add comments; Rust via return tuple. Errors: Go always 500 for any service error, Python/JS honour `e.status`, Java/C# rethrow generic exceptions.
- Error body shape: Node/JS/Python `{error, code?}`; Go `{"error": msg}`; OpenAPI declares `{code:int, message}`; generated Errors/ApiErrorResponse classes (Java/C#/PHP) use a third shape.
- Naming: JSON camelCase on the wire everywhere, but Python model attrs are snake_case (breaks from_dict), Rust snake_case with serde rename, Go PascalCase with `Id` not `ID`.
- extends: Node/Java/C#/PHP/Python use inheritance (PHP/Python constructors break); Go embeds; Dart/Kotlin/Swift flatten; Rust DROPS parent fields; OpenAPI allOf; SQL/GraphQL flatten.
- date/datetime: Go time.Time for both (date breaks), Java LocalDate, C# DateTime, others string.
- Query/headers binding: Node binds; Python passes untyped dict / empty dict; Go passes nil; Java/C#/PHP/Rust declare but do not bind; Kotlin/Swift/Dart clients ignore them.
- Route registration order: Node/JS/PHP register `/:id` before static `/old`, `/tree` (shadowing in order-based routers); Go 1.22 and Flask are safe.
- Header field names: every emitter reuses raw header keys as identifiers; hyphenated names break all 9 non-TS targets plus GraphQL, Prisma and SQL.

### 4. Findings
#### F-MULTI-01: Hyphenated header names are emitted as identifiers in every language, producing uncompilable output
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/parser/parser.go:961 (synthetic `<Action>Headers` model); consumers: python models/items.py:218, go internal/models/items.go:184, java CreateItemHeaders.java:8, csharp CreateItemHeaders.cs, php CreateItemHeaders.php:12, rust models/items.rs:211, node-js types/items.js:141, dart/kotlin/swift clients, graphql, prisma, sql
- Trigger/reproducer: `headers: { "Idempotency-Key": string }` (the form shown in CLAUDE.md contract syntax). `veld generate --backend=python --backend-framework=flask --validate` then `python -m compileall out/pyflask` -> SyntaxError. Same via go build, javac, node --check, graphql buildSchema.
- Expected vs actual: valid identifier (`idempotency_key`) with wire name kept in a string literal vs `idempotency-_key` / `Idempotency-key`.
- Impact: any contract using the documented headers feature with a standard header name cannot compile in any non-TS language.
- Root cause: header keys flow into Field.Name unchanged; each emitter's case-converter passes '-' through.
- Recommendation: fix -- sanitise header field names once in the parser (keep original as wire name) and have emitters use the wire name only in string literals.
- Regression check: compile-of-generated-code CI job per language with a contract containing a hyphenated header.
- Effort estimate: 6h-12h

#### F-MULTI-02: Emitter unit tests assert substrings; generated output is never compiled, so most non-Node backends ship uncompilable code with a green suite
- Category: emitter-rust
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/emitter/backend/*/*_test.go (e.g. rust/rust_test.go:160 asserts `pub fn router<S>() -> impl std::any::Any`)
- Trigger/reproducer: `go test ./internal/emitter/...` passes; building generated output fails (see run log).
- Expected vs actual: tests should fail when output does not compile; they enshrine broken stubs.
- Impact: regressions and pre-existing breakage are invisible; claims of working multi-stack generation are unsupported.
- Root cause: no golden-file or toolchain compile harness (docs/EXECUTION_PLAN_v1.md already lists "harness" as M2).
- Recommendation: fix -- add per-language compile smoke tests (go build, python compileall+import, javac, tsc, node --check; dockerised cargo/dotnet/php) driven from one rich contract.
- Regression check: CI matrix job fails on current main for rust/java/csharp/php/go/python.
- Effort estimate: 24h-40h

#### F-MULTI-03: Default Python backend (no --validate) emits a syntax-error route file and no route registration
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/emitter/backend/python/routes.go:139 (and strategy), registration branch in routes.go
- Trigger/reproducer: `veld generate --backend=python --frontend=none`; compileall -> `input = CreateItemSchema(**body = data  # CreateItemSchema).model_dump()`.
- Expected vs actual: Pydantic-validated route calling the service and registered with Flask vs syntax error, missing `from flask import request`, missing schemas module (not emitted), `# register:` comment instead of add_url_rule, `raise RuntimeError(body)` instead of returning a response.
- Impact: the primary documented Python flow (CLAUDE.md "Pydantic validation") produces nothing runnable; only `--backend-framework=flask --validate` works (after F-MULTI-01/04 are fixed).
- Root cause: a strategy placeholder (ExtractBody) is interpolated as code; the Pydantic schema emitter named in python.go's header no longer exists.
- Recommendation: rewrite -- make Flask the default strategy and always emit the zero-dependency validators; drop Pydantic claims.
- Regression check: compileall + import + run Flask smoke test for default flags.
- Effort estimate: 8h-14h

#### F-MULTI-04: Python validators miss the `Decimal` import; any contract with a decimal field returns 500 on every request
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/emitter/backend/python/validate.go:348 (emits isinstance(..., (str, Decimal)) with no import); output _validators.py:8
- Trigger/reproducer: GET /api/items/abc -> 500 {"error":"name 'Decimal' is not defined"}; fixed by adding `from decimal import Decimal`.
- Expected vs actual: 200 vs 500 (swallowed by blanket except).
- Impact: decimal is one of the eight core scalars.
- Root cause: validator header emits only `from typing import Any`.
- Recommendation: fix -- emit the import (or avoid Decimal in validators).
- Regression check: contract with a decimal field exercised in the Flask smoke test.
- Effort estimate: 1h-2h

#### F-MULTI-05: Python service contract is internally inconsistent (ABC returns model classes, routes require dicts; headers not extracted; query untyped; malformed JSON -> 500; enum unchecked; exception text leaked)
- Category: emitter-python
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: python/routes.go:145-150 and the catch-all except block, python/types.go, validate.go
- Trigger/reproducer: see Part A run log.
- Expected vs actual: typed Item/ItemQuery/headers reach the service; bad JSON -> 400; unknown enum -> 400 vs dict-only returns, `{}` headers, `{'limit':'5'}`, 500 on bad JSON, `role:"bogus"` accepted, `str(e)` returned for all non-API errors.
- Impact: the generated interface cannot be implemented as typed; auth headers (the point of `headers:`) are dropped; information disclosure via str(e) for unexpected exceptions.
- Root cause: validators and models developed independently; header extraction left as a comment.
- Recommendation: refactor -- return dicts or convert via to_dict, bind headers/query, restrict error bodies to known exception types.
- Regression check: Flask smoke test asserting headers present, typed query, 400 for bad JSON/enum, generic 500 body.
- Effort estimate: 10h-18h

#### F-MULTI-06: Go routes do not compile (`mux` undefined) and run without input validation or query/header binding
- Category: emitter-go
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: low
- Location: internal/emitter/backend/go/strategy/plain.go:21,52; go/routes.go handler generation
- Trigger/reproducer: `go build ./...` in out/go -> `internal/routes/items.go:14:2: undefined: mux` (after the F-MULTI-01 rename). After patching: `{}` body -> 201; role "bogus" accepted; GetItem receives nil query; CreateItem receives nil headers; date field "2020-01-01" fails to decode.
- Expected vs actual: compile + validate vs compile error and zero validation.
- Impact: Go backend unusable out of the box; once patched, required fields/enums are unenforced and auth headers unreachable.
- Root cause: strategy emits `mux.` while the function parameter is named `r`; no validation emitter for Go.
- Recommendation: fix -- name the param consistently, add generated Validate() methods, bind query/headers, map date to a date-aware type.
- Regression check: go build + vet of generated output; httptest-based smoke test.
- Effort estimate: 12h-20h

#### F-MULTI-07: Rust backend cannot compile and has no working router or main
- Category: emitter-rust
- Evidence status: source-supported defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/emitter/backend/rust/routes.go:35,100; Cargo.toml template in rust/main.go; output src/items/mod.rs, src/models/items.rs
- Trigger/reproducer: read generated tree (cargo unavailable: environment limitation; verdict by inspection).
- Expected vs actual: Axum router vs `build_router() -> impl Any { items_router() }` returning `()`; Cargo.toml lacks axum; handlers lack `use axum::{extract::{State,Path},Json,http::StatusCode}; use std::sync::Arc`; handlers return the trait result whose type (`Result<T,Box<dyn Error>>`) differs from the declared handler return; `CreateItemHeaders::default()` without a Default derive; `item_default_role() -> Role { String::from("user") }`; hyphenated identifiers; extended structs drop parent fields (Item has no id/createdAt).
- Impact: not a usable backend; docs disagree (Actix vs Axum).
- Root cause: partially stubbed emitter whose tests lock in the stub.
- Recommendation: freeze -- mark experimental until a cargo CI job exists; fix docs/architecture/overview.md.
- Regression check: cargo check in CI.
- Effort estimate: 30h-50h to make real

#### F-MULTI-08: Java, C#, PHP backends emit framework-less classes that do not compile and do not route
- Category: emitter-java
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: java/types.go (enum default; package const at line 26), java/routes.go (deleteItem), csharp/types.go, csharp/routes.go, php/routes.go, php/types.go
- Trigger/reproducer: javac (stub Jackson): `Item.java:22 incompatible types: String cannot be converted to Role`; `ItemsController.java:94 missing return statement`. C#: `Role Role { get; set; } = "user";`, controller without ControllerBase/Mvc using/verb attributes (csproj lacks AspNetCore). PHP: `Role $role = 'user'` fatal, `array $request` then `->validate()`, required params after optional. C#/PHP read only (no dotnet/php).
- Expected vs actual: working Spring/ASP.NET/Laravel integration vs plain POJOs; Java output package is the author's `maayn.veld.generated`, not configurable.
- Impact: three backends advertised as Spring Boot / ASP.NET Core / Laravel are neither compilable nor routable.
- Root cause: default-value emission treats enum default as a string literal; route emitters are framework-neutral but docs/tests claim otherwise.
- Recommendation: freeze -- keep model+interface output, drop controller/route claims until compile CI exists.
- Regression check: javac in CI now; dotnet/php in docker.
- Effort estimate: 20h-35h each to reach a working state

#### F-MULTI-09: Dart/Kotlin/Swift clients have compile or runtime defects beyond the header bug
- Category: emitter-swift
- Evidence status: source-supported defect
- Severity (user impact): high
- Security severity: none
- Location: frontend/swift/client.go (`${id}` interpolation), frontend/kotlin/helpers.go:20 + client.go:107, frontend/dart/types.go (fromJson)
- Trigger/reproducer: generated APIClient.swift line 85 `"/api/items/${id}"`; Kotlin `val price: java.math.BigDecimal` inside @Serializable class (no serializer: compile error) and "query params omitted for brevity"; Dart `role: json['role']`, `tags: json['tags']`, `children: json['children']` (runtime cast errors); Swift types internal and Decimal cannot decode a JSON string "1.10". Toolchains unavailable: judged by reading.
- Expected vs actual: working SDKs vs path params literally sent as "${id}" (Swift), query/header params silently dropped (Kotlin/Swift/Dart), decimal broken.
- Impact: the three mobile SDKs are unusable for any non-trivial contract.
- Root cause: string-template emitters never compiled or run.
- Recommendation: freeze -- experimental label; fix after compile CI exists.
- Regression check: kotlinc / swift build / dart analyze jobs.
- Effort estimate: 10h-16h per SDK

#### F-MULTI-10: Service SDKs: Python SDK rejects its own model types; TS SDK fails strict tsc; Go SDK lacks query support
- Category: service-sdk
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: python/sdk.go (body/query serialisation), node/sdk.go (shared index scalar imports), go/sdk.go
- Trigger/reproducer: Part D: `create_item(CreateItem(...))` TypeError; `get_item("a", ItemQuery(limit=3))` AttributeError; `tsc --strict` 24 errors in sdk/shared/index.ts (UUID/ISODateTime/ISODate/Decimal undeclared), TS1263, TS2352.
- Expected vs actual: typed clients usable with their own models vs only raw dicts work (Python); compile errors in TS.
- Impact: inter-service SDK, a headline feature, needs manual patching. Positives: env var precedence (arg > VELD_<NAME>_URL > baked) verified for Python, Go, TS; WS actions skipped; error type VeldApiError; deps and deps --validate work.
- Root cause: SDK templates reuse model emitters whose output is not exercised.
- Recommendation: fix -- add to_dict conversion, import scalars in shared module, add query param to Go.
- Regression check: SDK smoke tests against a local fake server (as in ws/t.py).
- Effort estimate: 8h-14h

#### F-MULTI-11: Dockerfile and CI generators download a release asset that returns 404, and bundle an unrelated registry Postgres
- Category: gen-dockerfile
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: medium
- Location: internal/generators/dockerfile/dockerfile.go (download line, compose template), internal/generators/cicd/cicd.go
- Trigger/reproducer: HTTP HEAD https://github.com/Adhamzineldin/Veld/releases/latest/download/veld-linux-amd64 -> 404 (assets are .tar.gz). Generated docker-compose.yml starts `postgres:16-alpine` named "registry", POSTGRES_PASSWORD=changeme, 5432:5432 published.
- Expected vs actual: working build vs both the Dockerfile download and the CI "Install Veld" step fail; compose exposes a default-credential DB on the host.
- Impact: generated pipelines fail on first run; unpinned, unverified binary download into the build (supply chain); image runs as root; `CMD node index.js` and `/app/dist` need user-created files; Python/Go projects get Node templates unless marker files are present in --out.
- Root cause: copy-paste from registry compose; release naming not tested.
- Recommendation: fix -- pin version and checksum, correct asset name, remove DB service, add non-root USER.
- Regression check: `docker build` against examples/ in CI.
- Effort estimate: 4h-8h

#### F-MULTI-12: SQL/Prisma/GraphQL generators emit invalid output for common contracts
- Category: gen-schema
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/schema/ (BuildSQL, BuildPrisma), internal/graphqlgen/
- Trigger/reproducer: `veld schema` / `veld graphql` on the shared contract; `buildSchema` throws on `Idempotency-Key: String!`.
- Expected vs actual: loadable DDL/Prisma/SDL vs DTO models (ItemQuery, CreateItem, headers) become tables; `DEFAULT user` unquoted (silently current_user in Postgres) and Prisma `@default(user)` invalid; `limit` unquoted; arrays/maps as VARCHAR(255) / String; Prisma models without `@id`; GraphQL Map -> String, ids as String not ID; no relations. graphqlgen and docsgen have 0 tests.
- Impact: schema output is a sketch, not usable DDL; the default-value bug changes semantics.
- Root cause: every Model treated as an entity; no quoting layer.
- Recommendation: refactor -- opt-in table marker, quote defaults and reserved words, skip DTO models.
- Regression check: parse generated SQL with a real parser, Prisma validate, graphql buildSchema in CI.
- Effort estimate: 10h-16h

#### F-MULTI-13: OpenAPI output is valid 3.0.3 but incomplete (no servers/security, error shape mismatch, defaults dropped)
- Category: gen-openapi
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Location: internal/generators/openapi/
- Trigger/reproducer: `veld openapi` + `redocly lint` (10 recommended-ruleset errors, 0 structural).
- Expected vs actual: servers from baseUrl, securitySchemes for middleware, enum default, ErrorResponse matching runtime `{error}`, no trailing-slash paths vs all absent or mismatched.
- Impact: documentation fidelity; downstream client generators would mislead.
- Root cause: minimal generator (361 LOC).
- Recommendation: fix -- add servers, security, defaults, align error schema across emitters.
- Regression check: redocly lint with a project ruleset in CI.
- Effort estimate: 4h-8h

#### F-MULTI-14: `internal/mock` is unwired dead code; docs list a feature with no command
- Category: gen-mock
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: internal/mock/ (284 LOC); CLAUDE.md architecture table
- Trigger/reproducer: `veld mock` -> unknown command; grep for imports of the package outside itself: none.
- Expected vs actual: runnable mock server vs unreachable package.
- Impact: maintenance burden, misleading docs.
- Root cause: command never wired or removed.
- Recommendation: remove -- or wire `veld mock` and add an integration test.
- Regression check: command present in `veld --help` test.
- Effort estimate: 1h-6h

#### F-MULTI-15: Documentation mismatches (Rust framework, Python Pydantic, WS undocumented, hardcoded post-generate hints, HTTP method list)
- Category: emitter-rust
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: docs/architecture/overview.md:37 (Actix vs Axum in code), CLAUDE.md type-mapping table (Pydantic), CLAUDE.md HTTP methods (code also supports HEAD, OPTIONS, WS with required `stream:`/optional `emit:`), cmd/veld/generate.go summary hints (UserSchema, IUsersService, maayn package)
- Trigger/reproducer: compare docs to generated trees.
- Expected vs actual: docs match behaviour vs not.
- Impact: users follow instructions that cannot work.
- Root cause: docs not generated from code.
- Recommendation: fix -- generate hints from the AST and update docs.
- Regression check: doc-lint test comparing the language spec to docs.
- Effort estimate: 2h-4h

### 5. Maintenance-cost estimate
Assumptions: CI matrix per emitter compiling and running the shared rich contract (docker images for cargo, dotnet, php, kotlin; macOS runner for Swift), one maintainer familiar with the emitter, about one breaking upstream change per emitter per quarter, triage included, feature work excluded. One-off remediation to reach green is in the findings above.
| Emitter | Hours/month (steady state) | Notes |
|---|---|---|
| node-ts (ref) | 4-6 | primary; most real usage |
| typescript/react/vue/angular/svelte frontends | 2-3 each (8-12 total) | thin wrappers over the TS SDK |
| python | 4-6 | stdlib validators; Flask/Python versions stable |
| go | 4-6 | stdlib net/http only; chi pinned |
| javascript backend/frontend | 2-3 | shares Node logic |
| java | 6-10 | Spring/Jackson/Gradle churn if framework support is claimed; 3-4 for plain models only |
| csharp | 6-10 | ASP.NET controllers + System.Text.Json quirks |
| php | 5-8 | Laravel major release yearly |
| rust | 8-12 | axum 0.x breaking releases, trait/lifetime complexity |
| dart / kotlin / swift | 3-5 each | needs macOS runner for Swift |
| openapi / docs / graphql / schema | 2-3 each | validate with external linters |
| dockerfile / cicd / envconfig / scaffold | 2-4 total | pin versions, test docker build |
| service SDKs (8 languages) | 6-10 | each sdk.go duplicates model emitters |
Total realistic: roughly 75-110 h/month for the full matrix versus roughly 25-35 h/month for tier 1 (node, python, go, TS/React) with a compile CI.

### Recommended tiers
- Tier 1 (keep, fix first): node-ts, TypeScript/React frontends, Python (Flask + validators), Go (stdlib/chi), OpenAPI, service SDKs for TS/Python/Go.
- Experimental (label; ship models/interfaces only): javascript backend, Vue/Angular/Svelte, Java, C#, PHP, Dart, Kotlin, Swift, SQL/Prisma/GraphQL, dockerfile/cicd.
- Freeze/remove: Rust backend until cargo CI exists, internal/mock (remove or wire), Pydantic claims, Rust/Java/C#/PHP service SDKs (read only, not compile-verified).



## Appendix: audit-track notes (E-registry)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Veld registry subsystem audit

Commit 6a47f815f37ea4c286a6e765312df76743fa7133, reviewed 2026-10-21. Windows 11 host, Go 1.26, Docker 29.7.2 (postgres:16 container `veld-audit-pg`, removed at end).
No tracked file was modified (`git diff` clean; only pre-existing untracked docs). No real registry was contacted. CLI runs used `USERPROFILE=<scratch>\home`, so the real `~/.veld` was untouched (verified).

Artifacts (all under `...\scratchpad\reg-audit\`): `registrycopy\` (verbatim copy of tarball.go + `extract_test.go`), `e2e\main.go` (API harness), `proxy\main.go` (MITM), logs `unpack-small.log`, `unpack-small2.log`, `unpack-many.log`, `e2e-run1.log`, `e2e-run2.log`, `server*.log`.

Evidence legend: REPRODUCED = I ran it and saw the result. SOURCE = read in code, not executed. Platform caveat: archive tests ran on Windows (GOOS=windows). Linux behaviour for backslash/drive-letter names is inferred from the code (inert there).

---------------------------------------------------------------------------------------------------

### 1. Archive handling (internal/registry/tarball.go and callers)

Callers: `Pack` <- cmd/veld/registry.go:132 (push). `Unpack` <- registry.go:238 (pull). `VerifySHA` <- registry.go:233. `UnpackReader` has no other caller. The server never opens the archive (see F-REG-13).

1. Path containment (tarball.go:116-127). `package/` prefix is stripped (117-119, not required: unprefixed names are accepted). `filepath.Clean(name)` (122), then `strings.HasPrefix(name, "..")` -> silently `continue` (123-125); then `filepath.Join(destDir, name)` (127). There is no `filepath.Rel`/prefix check against destDir after the Join, no `filepath.IsAbs`/`VolumeName` check, no symlink-evaluation of destDir.
   - `..` forms are skipped silently (REPRODUCED: `package/../../escape.txt`, `../../escape2.txt`, `package/a/../../../escape3.txt` -> nothing written outside; only `ok.veld` extracted). False positive: any legitimate file whose name starts with `..` (e.g. `..foo`) is silently dropped.
   - Absolute names: `Join` re-roots them under dest (REPRODUCED: `/tmp/abs-escape.txt` -> `dest\tmp\abs-escape.txt`). Real absolute Windows path -> `os.Create` fails with "filename ... syntax is incorrect" and Unpack returns the error (no escape). `C:\Windows\Temp\x` same (error, nothing written outside).
   - Drive-relative `C:name` (REPRODUCED): no error, creates a file `dest\C` with an NTFS alternate data stream `veld-driverel.txt`. Likewise `package/a.veld:hidden` writes a hidden ADS on `a.veld`. Contained to dest, but invisible data can be smuggled on Windows.
   - Backslash names (REPRODUCED on Windows): `package\..\..\bs-escape.txt` and `..\..\bs-escape2.txt` were skipped (Clean normalises `\`); `package\sub\file.veld` extracted to `dest\package\sub\file.veld` (prefix strip did not match because it only strips the forward-slash prefix). On Linux a backslash is a literal filename character (SOURCE, inert).
2. Entry types (tarball.go:129-141). Only `TypeDir` is special-cased. EVERYTHING else (symlink, hardlink, char/block device, FIFO, PAX global header) falls through to `os.Create` + `io.Copy`. REPRODUCED: symlink entry -> empty regular file `dest\link` (mode rw-rw-rw-, not a symlink); a following entry `link/pwned.txt` then fails (`MkdirAll` error ignored at 134, `os.Create` error returned at 136) so nothing is written through the "link". Hardlink, device, FIFO, `pax_global_header` -> empty regular files named `hl`, `dev`, `fifo`, `pax_global_header`. So symlink/hardlink attacks are neutralised by accident (never materialised), not by design. A pre-existing symlink or junction inside destDir/outDir WOULD be followed by `os.Create` (SOURCE, not tested; Windows symlink creation needs privilege). PAX/GNU long names are decoded by archive/tar into `hdr.Name` and go through the same Clean check.
3. Size limits: none. No per-entry size (`hdr.Size` unused), no total size, no entry count, no ratio check, `io.Copy(out, tr)` at 139 has no `io.LimitReader`. REPRODUCED: 1.24 MiB archive -> 1.00 GiB file written, 2.9 s, err=nil. 1.4 MB archive with 100,000 one-byte entries -> 100,000 files created in 4 m 04 s (Defender-affected NTFS), err=nil. Download is also unbounded: `Client.Download` copies the whole body to a temp file (client.go:209) and `Client.do` uses `io.ReadAll` (client.go:69).
4. Checked I/O: `tr.Next` error is checked and returned (107-113); gzip.NewReader error checked (99). NOT checked: `os.MkdirAll` (130,134), `io.Copy` (139), `out.Close` (140), `VerifySHA`'s io.Copy (153), Pack's `tw.WriteHeader` (73), `io.Copy` (74), `tw.Close/gw.Close/tmp.Close` (78-80), `Download`'s io.Copy (209). REPRODUCED: truncated gzip -> file `big.veld` left at 144,776 of 400,000 bytes, and Unpack returned `unexpected EOF` only because the next `Next()` call failed; a failure inside the LAST entry's body would be reported as success if the tar trailer were also tolerated (the copy error is dropped).
5. Partial extraction: extraction writes directly into the final outDir (registry.go:214, 238); no temp dir + rename; no cleanup. REPRODUCED: archive `good1, good2, C:\bad.txt, never` -> error, but `good1.veld` and `good2.veld` remain on disk. Re-pulling a newer version over an older one also leaves stale files from the old version (no removal), and silently overwrites files that exist in outDir.
6. Checksum: Pack computes sha256 over the written tar.gz (tarball.go:51-82) and push throws it away (`_ = sha`, registry.go:157). The SERVER recomputes the sha while streaming (packages.go:178-202) and stores it in `package_versions.tarball_sha`; the download endpoint returns it in header `X-Veld-SHA256` (packages.go:270). The client reads that header from the SAME HTTP response as the body (client.go:208), and verifies only `if remoteSHA != ""` (registry.go:232). It is verified BEFORE extraction (233 vs 238) and after the full download to a temp file. Comparison is `got != expected` string compare (tarball.go:155), not constant time (irrelevant here). Source of truth = the registry's own response header: it protects against corruption and (REPRODUCED) against a server-side on-disk tamper (`integrity check failed: SHA-256 mismatch`), but gives NO authenticity: a MITM or a malicious registry controls both header and body, and omitting the header disables verification (REPRODUCED, F-REG-09). No signature, no lockfile, no pinning of the sha in the consumer project.
7. File modes: archive modes are ignored; files are created with `os.Create` (0666 & umask), directories 0755. No setuid/exec bits applied. REPRODUCED (Windows shows rw-rw-rw- for a 04777 entry; on Unix the mode would be 0666&^umask, SOURCE).
8. `Pack` (tarball.go:17-84): walks `dir` (= `rc.ConfigDir`) recursively with `WalkDir` and includes EVERY file whose extension is `.veld` or whose basename is `veld.config.json`, at any depth (including nested projects, `node_modules`, `generated/`). The `patterns` slice is dead code (20, 32). `WalkDir` does not follow directory symlinks, but a file symlink named `x.veld` pointing anywhere is included with the TARGET's content (d.IsDir() is false for the link; `os.Stat`/`os.Open` follow it) - SOURCE, not run. Dotfiles/dot-dirs are not excluded. `.veld-cache.json`/`.veld.lock.json` are NOT included (extension filter). Entry mode is fixed 0644, ModTime = file mtime (archive bytes not reproducible, so the sha differs between identical packs). Files that cannot be stat'd/opened are silently skipped; a file that grows between stat and copy yields a corrupt tar whose write error is ignored. `veld.config.json` (which may contain `hooks.postGenerate`, `baseUrl`, etc.) is always published.

Reproduction log (section 2) follows the same numbering.

---------------------------------------------------------------------------------------------------

### 2. Reproduction log (archive tests)

Method: `registrycopy/tarball.go` is byte-identical to `internal/registry/tarball.go` (`cmp` passed); `extract_test.go` crafts archives with archive/tar (no repo imports) and calls the real `Unpack`. Run: `go test -v -run '...' .` in `registrycopy`.

| Archive | Result |
|---|---|
| `package/../../escape.txt`, `../../escape2.txt`, `package/a/../../../escape3.txt` | refused silently (skipped); nothing outside dest; `ok.veld` extracted; err=nil |
| `/tmp/abs-escape.txt` | re-rooted to `dest\tmp\abs-escape.txt` (contained) |
| real absolute path, `C:\Windows\Temp\veld-drive-escape.txt` | Unpack returned error (invalid syntax); nothing written outside; earlier entries stay (`ok1.veld`) |
| `C:veld-driverel.txt`, `package/a.veld:hidden` | accepted; file `dest\C` / `dest\a.veld` size 0 with NTFS ADS (contained, hidden data) |
| `package\..\..\bs-escape.txt`, `..\..\bs-escape2.txt` | skipped; `package\sub\file.veld` extracted as `dest\package\sub\file.veld` |
| symlink `link -> <outside dir>` then `link/pwned.txt` | symlink became empty regular file; pwned.txt NOT written outside (Unpack error: path not found); err returned |
| hardlink `-> /etc/passwd`, char device, FIFO, PAX global header | all became empty regular files |
| 04777-mode entry | mode not applied |
| good1, good2, `C:\bad.txt`, never | error returned; good1+good2 left in dest (partial extraction) |
| truncated gzip (400 KB entry) | `unexpected EOF`; truncated 144,776-byte file left |
| 1 GiB zero entry, compressed 1,302,662 B (1.24 MiB) | extracted fully: 1,073,741,824 B, 2.86 s, err=nil (BOMB) |
| 100,000 one-byte entries, 1,484,475 B archive | 100,000 files created, 4 m 04 s, err=nil |

Cleanup: all test dirs are `t.TempDir()` (auto-removed); one leftover temp dir from a failed first run was deleted manually.

---------------------------------------------------------------------------------------------------

### 3. Authorization matrix

Auth column: R = requireAuth (401 if none), O = optionalAuth, N = none. Bearer accepts a JWT (anything with 2 dots) OR a `vtk_` token; cookie `veld_session` accepted when no Authorization header (middleware.go:70-100). "Scope" = API-token scope check. Body limits: NONE anywhere (no `http.MaxBytesReader`; only `ReadTimeout` 30 s / `WriteTimeout` 120 s at server.go:59-61). Server config has no TLS.

| Endpoint | Method | Auth | Membership/role | Ownership | Input validation | Scope check | Notes |
|---|---|---|---|---|---|---|---|
| /api/v1/auth/register | POST | N | - | - | non-empty, pw>=8; email/username not validated (HTML/any bytes accepted) | - | open registration; returns full 7-day JWT immediately (auth.go:78) |
| /auth/login | POST | N | - | - | none; bcrypt cost 10 | - | no rate limit (60 wrong pw in 4.5 s, all 401); partial JWT returned if TOTP (auth.go:107) |
| /auth/logout | POST | N | - | - | - | - | only clears cookie; JWT stays valid |
| /auth/me | GET | R | - | self | - | none | |
| /auth/totp-login | POST | N | - | - | 6-digit | - | no rate limit, no replay guard |
| /auth/setup-totp, confirm-totp | POST | R | - | self | confirm needs valid code | none | accepts the partial JWT (F-REG-01) |
| /auth/totp | DELETE | R | - | self | needs password | none | |
| /auth/verify-email | POST | N | - | token | token lookup | - | verification result is never used anywhere |
| /auth/resend-verification | POST | R | - | self | - | none | 503 without SMTP |
| /tokens | GET | R | - | own tokens | - | none | hash hidden (`json:"-"`) |
| /tokens | POST | R | none | self | name required; scopes/org_id/expires NOT validated | none | read-only token can mint admin token (F-REG-03); org_id never enforced |
| /tokens/{id} | DELETE | R | - | `user_id=$2` in SQL (OK) | - | none | |
| /stats | GET | N | - | - | - | - | public counts |
| /orgs | GET | O | - | - | - | - | anon: ALL orgs; authed: only own orgs (inconsistent) |
| /orgs | POST | R | any registered user | creator = owner | name charset a-z0-9- ; length/reserved names not checked; description/display_name unescaped | none | open org creation |
| /orgs/{org} | GET | O | none | - | - | - | members incl. email + user_id, and ALL packages incl. private, to anonymous (F-REG-06) |
| /orgs/{org}/members | POST | R | caller owner/admin | - | role enum; username lookup | none | admin can overwrite an owner's role (ON CONFLICT UPDATE, db.go:296) (F-REG-04) |
| /orgs/{org}/members/{u} | DELETE | R | caller owner/admin | - | last-owner guard only when caller is owner (orgs.go:180) | none | admin can remove sole owner |
| /packages | GET | O | none | - | `q` -> ILIKE param (wildcards unescaped) | - | any authed user sees every private package (packages.go:29-32) |
| /packages | POST | R | owner/admin of org | - | org/name/version only checked non-empty; NO charset/length/semver; multipart 32 MB is a memory threshold only | `write` | creates package record before tarball is validated; no archive validation |
| /packages/{o}/{n} | GET | O | private: member | - | - | none | 404 for non-visible |
| /packages/{o}/{n}/versions | GET | O | NONE | - | - | none | no visibility check at all (packages.go:80) |
| /packages/{o}/{n}/{v}/download | GET | O | private: any member | - | - | none (no `read`) | anon 401 vs non-member 403 vs absent 404 = existence oracle |
| .../{v}/deprecate | POST | R | owner/admin | - | message unbounded | none | |
| .../{v} | DELETE | R | owner only | - | - | none (read-only token works, REPRODUCED) | deletes DB row and file; version can then be re-published |
| / (catch-all) | any | N | - | - | - | - | serves embedded index.html for every path and method; no file server |

Auth package notes: JWT HS256 (auth/token.go:56-92): header is NOT parsed, algorithm is fixed to HMAC-SHA256 by construction (no alg confusion, no `none`); signature compared with `hmac.Equal` (77) (constant time); `exp` enforced (88); no `nbf`, no `aud`/`typ`/purpose claim; secret minimum 16 chars (server.go:38; CLAUDE.md says 32+); the secret is also the only key (no rotation, no `kid`). `vtk_` token = 32 bytes `crypto/rand` base64url (token.go:19-25), stored as sha256 hex (unsalted, fine for 256-bit random), revocable by DELETE /tokens/{id}; `expires_at` is stored but never checked (F-REG-05). `GenerateID` ignores `rand.Read` error (token.go:38). bcrypt `DefaultCost` (10) (auth.go:52). TOTP: HMAC-SHA1, 6 digits, +-1 step window (totp.go), secret 160-bit crypto/rand, secret stored plaintext in DB, no replay protection, no attempt limit. Email verification token = 2x16 random bytes hex (auth.go:348-350, 256-bit), expiry 24 h enforced (289), stored plaintext, single use; but `email_verified` is never consulted for any permission. Cookie: HttpOnly yes, SameSite=Lax, Secure NO (auth.go:372-381). CORS: `*` for all (server.go:152-154), no credentials flag, `PATCH` absent; no CSP/XFO/XCTO/HSTS (REPRODUCED empty). SQL: every query is parameterised; the only two `fmt.Sprintf` in db.go (408, 415) build `$N` placeholders only. Migrations: single `CREATE TABLE IF NOT EXISTS` + `ALTER ... ADD COLUMN IF NOT EXISTS` string executed at every start (db.go:43-128); no version table (REPRODUCED: `\dt` shows 7 tables, no schema_migrations). Storage: local FS; key = `org/name/version.tar.gz` built from unvalidated input (F-REG-02); `Put` writes to `.tmp-*` then renames (atomic per file). Publish ordering: pkg row created (packages.go:155) -> tarball stored (184) -> version row inserted (216); on insert failure `Storage.Delete(tarballKey)` (217) - which deletes the winner's file (F-REG-07). Web: `handlers/web.go` serves exactly one embedded file; no path-based file access, so no traversal (REPRODUCED: `/../../../etc/passwd` -> the SPA HTML). Logging: request line only (method, path, duration); no tokens in logs EXCEPT the email-verification URL with its token (auth.go:363) and SMTP username/host (email.go). Graceful shutdown: `signal.NotifyContext(os.Interrupt)` only (serve.go:138) - SIGTERM (docker stop, systemd) is not handled; 10 s Shutdown otherwise (server.go:~66). Health endpoint: none (any path returns 200 SPA, REPRODUCED). Backup/restore: no docs, no command (none expected); state = PostgreSQL + the storage dir, which must be backed up consistently; nothing reconciles DB rows and files.

Config precedence (serve.go:97-108): addr, dsn, storage, secret implement flag > env > file > default as documented. BaseURL: env/file only (no flag). SMTP: no flags; SMTP port: file only (no SMTP_PORT env); `server.Config` documents `JWT_SECRET`. `registry.config.example.json` has a placeholder secret "replace-this-with-openssl-rand-hex-32" (37 chars, so the example file starts without error if copied unchanged, i.e. a publicly known JWT secret); sslmode=disable DSN.

---------------------------------------------------------------------------------------------------

### 4. Live end-to-end log (Part D)

Environment: `docker run -d --name veld-audit-pg -p 55432:5432 -e POSTGRES_PASSWORD=audit -e POSTGRES_DB=veld postgres:16` (Docker Desktop had to be started first). Server: `veld.exe registry serve --addr :18080 --dsn "postgres://postgres:audit@localhost:55432/veld?sslmode=disable" --secret "audit-secret-0123456789-abcdefghij" --storage <scratch>\reg-audit\storage`. NOTE the command is `veld registry serve`; `veld serve`, `veld login`, `veld push`, `veld pull` as documented in CLAUDE.md do NOT exist (`Error: unknown command "login" for "veld"`, REPRODUCED).
SMTP is NOT required: registration returns a JWT immediately, the verification email is skipped with a log line, and `email_verified` is never enforced. API tokens are created via the API (the CLI's `login` needs a `vtk_` token; password login is not exposed by the CLI - `Client.Login` exists but no command calls it).

CLI sequence (PowerShell, USERPROFILE redirected):
```
veld registry login --registry http://localhost:18080 --token vtk_...   -> Logged in ... as cli5487   (plain http accepted; no warning)
credentials.json: {"registries":{"http://localhost:18080":{"token":"vtk_DYPN...","username":"cli5487"}}}  (plaintext)
veld registry push        -> Published @cliorgcli5487/hello@1.0.0  {"sha256":"b4faa0c5...","size":285}
veld registry push (again)-> Error: registry error (409): version 1.0.0 already published   (exit 1)
veld registry pull @cliorgcli5487/hello  -> Pulled ...@1.0.0 -> veld\packages\@cliorgcli5487\hello\{app.veld, veld.config.json}
veld registry pull ...@9.9.9  -> Error: download failed: registry error (404): version not found
veld registry pull @cliorgcli5487/nopkg -> Error: could not fetch versions: registry error (404): not found
tamper (append bytes to stored .tar.gz) then pull -> Error: integrity check failed: SHA-256 mismatch: expected b4faa0c5... got 26576dbc...  (exit 1, nothing extracted)
MITM proxy :18081 (replaces body, DELETES X-Veld-SHA256) -> "Pulled" OK; evil app.veld + veld.config.json (postGenerate hook) written
MITM proxy :18082 (replaces body AND sets matching sha) -> "Pulled" OK; same
```
Restart persistence: process killed (hard; Windows has no SIGTERM path), restarted with identical args: stats before/after identical (8 users, 5 orgs, 12 packages, 64 versions), earlier API token still valid, download of old tarball 200, migration re-ran silently with no error. Rows are in PostgreSQL, files in storage dir.

API harness output (`e2e-run1.log`, `e2e-run2.log`) is quoted in the findings below. Private packages: the API offers no way to make a package private (Publish hard-codes `Visibility: "public"`, packages.go:151; no update endpoint), so for the private-package tests I set `visibility='private'` with `UPDATE packages` in psql.

Cleanup done: server and proxies stopped, `docker rm -f veld-audit-pg` run and verified (0 containers named veld-audit-pg), real `~/.veld/credentials.json` verified not to contain 18080. Leftover scratch files (storage dir, `reg-audit\traversal-*` file written by the traversal test) are inside the scratchpad.

---------------------------------------------------------------------------------------------------

### 5. Findings

#### F-REG-01: 2FA bypass - the pre-TOTP "partial" JWT is a fully valid session token
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: anyone who knows a victim's password (the exact attacker TOTP exists to stop); no other precondition. Also usable by a stolen password from a leaked DB of another service.
- Location: internal/server/handlers/auth.go:105-117 (partial issued with `IssueJWT`), internal/server/auth/token.go:56-92 (no purpose claim), internal/server/auth/middleware.go:70-100 (accepts any valid JWT as a session)
- Trigger/reproducer: `e2e-run2.log` E7. alice enables TOTP; `POST /auth/login` returns `{"partial":"eyJ...","totp_required":true}`; `GET /auth/me` with `Authorization: Bearer <partial>` -> `HTTP 200 {... "totp_enabled":true}`; `POST /tokens` with the partial -> HTTP 201 (admin API token minted). Never supplied a TOTP code.
- Expected vs actual: partial token should only be accepted by /auth/totp-login; actual: accepted everywhere for 5 minutes, and it can mint a permanent admin API token.
- Impact: two-factor authentication gives no protection at all against password compromise; attacker gets a durable `vtk_` token.
- Root cause: `JWTClaims` has only sub/exp/iat; the partial and the full token are indistinguishable.
- Recommendation: fix - add a `typ`/`purpose` claim (e.g. "2fa-pending") that `resolveAuth` rejects, and require it in totp-login.
- Regression check: test that a partial token gets 401 on /auth/me, /tokens, /packages (POST).
- Effort estimate: 1h-2h

#### F-REG-02: Storage path traversal on publish - arbitrary `*.tar.gz` write, overwrite and delete
- Category: server-storage
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: any registered user (registration and org creation are open and unverified); becomes owner of their own org, which is the only precondition for Publish.
- Location: internal/server/handlers/packages.go:122,177 (unvalidated `manifest.name`/`version` into the key); internal/server/storage/storage.go:33-36 (`Clean` then `Join` with no containment check); packages.go:217 and :334 (Delete of the same key)
- Trigger/reproducer: `e2e-run1.log` E4. Publish `{"org":"mal9041","name":"../../traversal-9041","version":"1.0.0"}` -> 201; file created at `<scratch>\reg-audit\traversal-9041\1.0.0.tar.gz`, i.e. outside the storage root. Then publish `name="../acme9041/auth"` -> victim's `acme9041/auth/1.0.0.tar.gz` overwritten (disk sha changed from 53da33... to d94982...; victim download shows header sha 53da33... but body sha d94982...). Then mallory `DELETE /packages/mal9041/..%2Facme9041%2Fauth/1.0.0` -> 204 and victim's tarball is gone (`HTTP 404 file not found in storage`).
- Expected vs actual: name/version must be constrained and the key must stay under the root; actual: arbitrary relative key. Content is attacker-controlled bytes; suffix is always `.tar.gz` (limits it to overwriting/creating `*.tar.gz` files anywhere the server user can write, and deleting such files). It breaks other orgs' packages (integrity-check failure at best; if the victim version's DB sha were also attacker-controlled it would be a substitution) and can destroy any `*.tar.gz` on the host.
- Impact: cross-tenant integrity/availability damage; arbitrary file write/delete of `.tar.gz` outside the data dir; combined with the missing sha anchoring on the client (F-REG-09) a path to serving wrong content.
- Root cause: no validation of package name/version (no charset, length, semver); trust in `filepath.Clean`.
- Recommendation: fix - validate org/name (`^[a-z0-9][a-z0-9._-]{0,63}$`) and strict semver at the handler, derive the storage key from the DB ids (content-addressed by sha), and verify `filepath.Rel(root, p)` stays inside.
- Regression check: publish with `..`, `/`, `%2F`, NUL, backslash, very long, and unicode names -> 400; assert nothing outside root.
- Effort estimate: 3h-6h

#### F-REG-03: Token scope model is not enforced; a read-only token can mint admin tokens and delete versions
- Category: server-authz
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: holder of any valid API token (e.g. a leaked read-only CI token).
- Location: internal/server/handlers/auth.go:393-439 (CreateToken: no scope check, scopes/org_id accepted verbatim); handlers/packages.go:276-338 (Deprecate/Delete never call `HasScope`); only packages.go:105 checks `write`; middleware.go:57-68 (`HasScope` returns true for JWT); `read` scope never checked; `org_id` of a token never enforced
- Trigger/reproducer: E2/E3. `ro` token (scopes [read]) -> `POST /tokens` with `scopes:[read,write,delete,admin]` -> token returned. Publish with `ro` -> 403 "missing 'write' scope", but with the minted token -> 201. `DELETE /packages/acme/auth/0.9.0` with the read-only token -> 204.
- Expected vs actual: scopes limit what a token can do; actual: trivially bypassed.
- Impact: least-privilege CI tokens give no protection; a leak of a "read" token is a full account compromise (publish, delete, rotate).
- Root cause: scopes bolted on at one handler; token management reachable with token auth.
- Recommendation: fix - require JWT session (or `admin` scope) for token management, validate scope enum, check scopes centrally in middleware per route, enforce `org_id`.
- Regression check: matrix test of (scope x endpoint).
- Effort estimate: 4h-8h

#### F-REG-04: Org admins can demote or remove the sole owner (org takeover)
- Category: server-authz
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: medium
- Exploitability: org member with role `admin` (granted by an owner).
- Location: internal/server/handlers/orgs.go:108-155 (AddMember allows an admin to set another member's role; db.go:296-303 `ON CONFLICT DO UPDATE SET role`), orgs.go:157-193 (last-owner guard only inside `if caller.Role == "owner"`, line 180)
- Trigger/reproducer: E6. bob(admin) `POST /orgs/acme/members {"username":"alice","role":"member"}` -> 200, alice's role is now `member`; with alice restored, bob `DELETE /orgs/acme/members/alice` -> 204, `owners left: 0`.
- Expected vs actual: admins must not modify owners; org must keep >=1 owner; actual: orphaned org nobody can administer.
- Impact: privilege escalation/lock-out inside an org; orphaned orgs cannot publish new owners (all role changes need owner/admin).
- Root cause: role checks look only at caller role and requested role, not target's current role.
- Recommendation: fix - disallow admins touching owners and enforce last-owner invariant for every caller in a transaction.
- Regression check: unit tests per (caller role x target role x action).
- Effort estimate: 2h-3h

#### F-REG-05: API token `expires_at` is never enforced
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: holder of an expired token.
- Location: internal/server/db/db.go:535-557 (GetTokenByHash returns expired rows), middleware.go:89-98 (no check)
- Trigger/reproducer: E9. Token created with `expires_days:1`; DB row set to `now()-1 day`; `GET /auth/me` -> HTTP 200.
- Expected vs actual: 401; actual: valid forever.
- Impact: advertised expiry is a false guarantee.
- Root cause: field stored, never read.
- Recommendation: fix - add `expires_at IS NULL OR expires_at > now()` to the lookup.
- Regression check: test with a past `expires_at`.
- Effort estimate: 0.5h-1h

#### F-REG-06: Private-package visibility is inconsistent across endpoints (latent leak) and unreachable from the API
- Category: server-authz
- Evidence status: reproduced defect (with `visibility` set by SQL; the API cannot set it)
- Severity (user impact): medium
- Security severity: medium
- Exploitability: anonymous (versions, org page) and any registered non-member (package list).
- Location: packages.go:25-43 (any authed user gets `public`+`private`), 80-96 (ListVersions no check), orgs.go:80-103 + db.go:428 (all packages of an org listed to anonymous), packages.go:57,242 (membership of any role suffices)
- Trigger/reproducer: E11 with acme/auth set private: anon `/packages` hides it; mallory (non-member) `/packages` LISTS it; anon `/orgs/acme` lists it and members' emails; anon `/packages/acme/auth/versions` -> 200 with full version rows including `tarball_key` and sha; anon download 401 vs non-member 403 vs absent 404 reveals existence.
- Expected vs actual: a private package must be invisible (404) to non-members on every endpoint; actual: 4 different leak paths. Because Publish always creates `public` packages and no endpoint changes visibility, the feature is currently unusable and the leaks are latent - until someone adds a toggle.
- Impact: the "private package" claim would be false; private contract names/versions/manifests exposed.
- Root cause: per-handler ad-hoc checks, no shared `canRead(pkg,user)`.
- Recommendation: refactor - one authorization helper used by every package/version/org read path, plus an explicit visibility API.
- Regression check: matrix test (anon/non-member/member x every GET).
- Effort estimate: 4h-8h

#### F-REG-07: Concurrent/duplicate publish deletes the published tarball (DB row left pointing at nothing)
- Category: server-storage
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Exploitability: any org owner/admin (or two honest CI jobs racing).
- Location: packages.go:162-166 (check-then-act), 177-200 (Put renames over the existing key), 216-218 (on unique-violation: `Storage.Delete(tarballKey)` removes the winner's file)
- Trigger/reproducer: E5. Two concurrent publishes of the same new version: in 10/25 (run 1) and 16/25 (run 2) iterations one got 201, the other 500, and the subsequent download returned `HTTP 404 file not found in storage` although the DB row exists and `GET /versions` lists it.
- Expected vs actual: loser gets 409 and the winner's artifact is intact; actual: published version silently corrupted/unavailable.
- Impact: data loss of a "published, immutable" artifact; also leaves a pkg row created before the tarball is validated (name squatting with zero versions).
- Root cause: file written under its final key before the DB uniqueness decision; unconditional delete in the error path.
- Recommendation: fix - write to a unique temp key, insert row inside a transaction, then `link`/rename only on success (or content-address blobs by sha).
- Regression check: parallel-publish test (N=20) asserts exactly one 201 and a downloadable artifact.
- Effort estimate: 3h-5h

#### F-REG-08: Versions are not immutable and "latest" is not the highest semver
- Category: server-db
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: org owner (delete + republish); a malicious/compromised owner or stolen write+delete token.
- Location: packages.go:311-338 (hard delete), 162 (only existing rows block); db.go:~470 (`ORDER BY published_at DESC`), cmd/veld/registry.go:199-209 (`versions[0]`), models.go:ParseSemver (unused by the CLI, tolerant parsing)
- Trigger/reproducer: E3. Deleted 0.9.0 and republished 0.9.0 with different content -> 201 (new sha). Published 2.0.0 then 1.0.1: list order `[1.0.1 2.0.0 0.9.0 1.0.0]`, so `veld registry pull @acme/auth` resolves to 1.0.1. Deprecated versions are also eligible for "latest".
- Expected vs actual: npm/crates semantics (a version, once published, is never reusable; latest = highest non-prerelease); actual: pinned versions can change content; "latest" is "most recently published".
- Impact: supply-chain integrity: consumers who pinned `@org/pkg@1.0.0` get different bytes after delete+republish, and no consumer-side lock/sha exists to notice. No semver ranges are supported (only exact or latest).
- Root cause: no tombstones, no semver validation, ordering by timestamp.
- Recommendation: fix - tombstone deleted versions (name+version permanently reserved), validate semver on publish, compute latest by semver, record sha in a consumer lockfile.
- Regression check: delete-then-republish must 409; ordering test.
- Effort estimate: 4h-8h

#### F-REG-09: Client integrity check is optional and self-referential; http registries are accepted without warning
- Category: client
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: network attacker on any `http://` registry URL; malicious or compromised registry operator; anyone able to alter response headers (misconfigured proxy/CDN).
- Location: internal/registry/client.go:208-210 (sha taken from the same response; copy error ignored), cmd/veld/registry.go:232-236 (`if remoteSHA != ""`), 37-60 (no scheme check on login), credentials.go:230 (token sent over whatever scheme)
- Trigger/reproducer: MITM proxies in section 4: stripping `X-Veld-SHA256` -> `Pulled`, malicious `veld.config.json` with `postGenerate` hook written; replacing body and header consistently -> `Pulled`. `veld registry login --registry http://localhost:18080` accepted, token stored and sent in cleartext (CLAUDE.md and help text show http examples; `ftp://` is rejected only by Go's transport).
- Expected vs actual: fail closed when the sha is absent; authenticity anchored out of band (lockfile/signature) and https required for non-loopback hosts; actual: all optional.
- Impact: tampered contracts flow straight into code generation (`.veld` models drive generated code; a pulled `veld.config.json` can carry a `postGenerate` command that runs on the next `veld generate` if it ends up as the project config, see F-REG-20). Tokens and packages travel in plaintext if the operator uses http.
- Root cause: checksum is transport-corruption detection, not a trust root.
- Recommendation: fix - require header (fail closed), refuse non-https except loopback unless `--insecure`, add a `veld.lock` with pinned sha256, and plan package signing.
- Regression check: pull against a server omitting the header must fail; login to `http://example.com` must fail.
- Effort estimate: 6h-16h (signatures larger)

#### F-REG-10: Unpack has no limits, writes in place, and drops I/O errors
- Category: tarball
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: malicious package author (any registered user can publish a public package; archive content is never validated server-side) or malicious registry; victim = anyone who runs `veld registry pull`.
- Location: internal/registry/tarball.go:98-143 (see section 1 items 3-5), client.go:209
- Trigger/reproducer: section 2 table: 1.24 MiB -> 1 GiB file in 2.9 s; 1.4 MB -> 100,000 files in 4 min; partial extraction left behind; truncated file left; `MkdirAll`/`io.Copy`/`Close` errors dropped (tarball.go:130,134,139,140).
- Expected vs actual: cap entries (count, per-file, total), reject non-regular entries explicitly, extract to a temp dir and rename atomically, check all errors; actual: none.
- Impact: disk-fill/CPU denial of service on developer or CI machines; corrupt half-extracted contracts used silently; symlink-safety only by accident (item 2); stale files persist across versions.
- Root cause: minimal implementation.
- Recommendation: rewrite - small hardened extractor (regular files and dirs only, `LimitReader`, count/size caps, containment via `filepath.Rel`, temp dir + rename) shared by CLI and server validation.
- Regression check: the section 2 corpus as a test table.
- Effort estimate: 4h-8h

#### F-REG-11: Stored XSS sinks in the web UI (usernames, org text, versions, deprecation text)
- Category: server-web
- Evidence status: source-supported defect (hostile values stored and returned by the API - reproduced; rendering in a browser not executed)
- Severity (user impact): high
- Security severity: high
- Exploitability: any registered user (username, org description/display_name, token names) and any publisher (version string, package name; version string is not validated). Victim = any visitor/member viewing the page.
- Location: internal/server/web/index.html:335 (`${u.username}`), 377 (`${e.message}`), 420, 456-457, 486-492, 519-524 (`v.version`, `v.deprecated`), 543-544, 564-565, 585-587 (`m.username`, `m.email`, inline `onclick="removeMember('${org.name}','${m.username}')"`), 616, 633-634; only `escHtml` uses are lines 497, 503; no CSP.
- Trigger/reproducer: E1/E12: org description `<img src=x onerror=alert(1)>` stored (psql shows it verbatim); username `<img src=x onerror=alert(document.domain)>` registered with 201; version `<svg onload=alert(1)>` accepted by the handler (on this Windows host the storage layer then rejected it with a 500 because `<>` is invalid in NTFS filenames; on Linux it would succeed - SOURCE).
- Expected vs actual: output-encode every interpolation; actual: template literals into `innerHTML`.
- Impact: script runs same-origin as the victim; the HttpOnly cookie is not readable but the script can call the API as the victim (mint admin API tokens, publish, delete, change org roles) and read the minted token back.
- Root cause: SPA built with string templates, no escaping layer, no CSP, no input constraints.
- Recommendation: rewrite - render via `textContent`/a small escaping template tag or a framework, add a strict CSP, and validate username/org/package/version charsets server-side.
- Regression check: playwright test creating hostile names and asserting no script execution; server tests for charset.
- Effort estimate: 6h-12h

#### F-REG-12: No rate limiting, lockout, or TOTP replay protection
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: anonymous.
- Location: server.go:59 (no limiter in the handler chain); auth.go:88-159; totp.go:28-38
- Trigger/reproducer: E7/E8: 60 wrong passwords in 4.5 s, all 401, no 429; 300 consecutive wrong TOTP guesses, zero 429; the same valid TOTP code accepted twice (`first -> HTTP 200, second use of same code -> HTTP 200`); with no limiter, a 6-digit code (3 valid values per 10^6 across the +-1 window) is brute-forceable online; unmeasured, only the absence of any throttle was reproduced (and F-REG-01 makes the second factor moot anyway).
- Impact: online password/TOTP guessing and credential stuffing; registration spam (open, unverified).
- Root cause: absent middleware.
- Recommendation: fix - per-IP and per-account limiter with backoff, one-time TOTP counter storage.
- Regression check: burst test expects 429; replay test expects 401.
- Effort estimate: 4h-6h

#### F-REG-13: No body-size limits and no server-side archive validation on publish
- Category: server-storage
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: any org owner/admin (publish); anonymous for JSON endpoints.
- Location: packages.go:111 (`ParseMultipartForm(32<<20)` is only the in-memory threshold, the comment says "32 MB max"), 169-200 (streams any bytes to disk, no size cap, no gzip/tar check); auth.go:39 etc. (`json.NewDecoder(r.Body)` unbounded); server.go:59-62
- Trigger/reproducer: E13: 200 MiB of non-archive bytes published as a version -> 201 (size 209,715,200 recorded), taking 2.5 s; 60 MiB JSON login body processed (401). Multipart spill also writes a temp copy in `$TMP`, doubling disk use. A client-side `Publish` buffers the full tarball in memory (client.go:148-159).
- Impact: disk exhaustion, memory pressure; registry can host arbitrary blobs; invalid archives are served to consumers who then hit F-REG-10.
- Root cause: `ParseMultipartForm` misread as a limit; no validation stage.
- Recommendation: fix - `http.MaxBytesReader` (global and per route), validate the tarball with the hardened extractor before accepting, per-user quotas.
- Regression check: oversize and non-tar uploads -> 413/400.
- Effort estimate: 3h-5h

#### F-REG-14: Sessions cannot be revoked; no purpose/typ on JWTs; 7-day lifetime
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: anyone holding a stolen JWT or cookie.
- Location: auth.go:162-171 (Logout only clears cookie), token.go:56-92, middleware.go:70-100
- Trigger/reproducer: E10: JWT used after `POST /auth/logout` -> `/auth/me` HTTP 200. No password-change/reset endpoint exists at all (no way to invalidate sessions or recover an account; no password reset flow).
- Impact: stolen session valid until expiry (7 days); no account recovery.
- Root cause: stateless JWTs only.
- Recommendation: refactor - server-side session table (or token-version column) checked in middleware; shorter TTL.
- Regression check: logout/password-change invalidates old tokens.
- Effort estimate: 4h-8h

#### F-REG-15: Transport/browser hardening absent (no TLS, cookie not Secure, CORS `*`, no security headers)
- Category: server-web
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: network attacker where deployed without a TLS terminator; any website for CORS (no credentials, so limited).
- Location: server.go:56-66 (`ListenAndServe`, no TLS option), auth.go:372-381, server.go:150-163
- Trigger/reproducer: E14: `Set-Cookie veld_session ... HttpOnly=true Secure=false SameSite=Lax`; preflight `ACAO="*"`; `CSP="" XFO="" XCTO="" HSTS=""`. serve.go prints `http://localhost<addr>` as the URL.
- Impact: session cookie and tokens in cleartext on http deployments; clickjacking; the SPA can be framed.
- Root cause: server assumes a reverse proxy but nothing documents or enforces it.
- Recommendation: fix - document proxy requirement, add `Secure` when `base_url` is https, add headers + CSP, restrict CORS to configured origins.
- Regression check: header assertions in a handler test.
- Effort estimate: 2h-4h

#### F-REG-16: Registry selection is nondeterministic and credentials are keyed by raw string (dependency-confusion exposure)
- Category: credentials
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: user who has logged in to more than one registry (or two spellings of one).
- Location: internal/registry/credentials.go:259-268 (`DefaultRegistry` returns the first key of a Go map), :230-236 (exact-string key), client.go:31-43; used by `pull` (registry.go:187-196), `versions`, `token create`
- Trigger/reproducer: three entries (`http://127.0.0.1:18080`, `http://localhost:18080`, `http://localhost:18080/`); 12 runs of `veld registry info` printed 9x/2x/1x different "default" registries.
- Expected vs actual: deterministic resolution from project config (`registry.url`) or an explicit default; actual: pull of `@org/name` without `--registry` may resolve against a random logged-in registry, and `push` uses `rc.Registry.URL` only if set, else the random default. `veld.config.json` has no per-scope registry mapping (as in `.npmrc` `@scope:registry=`), so a name in a public registry can shadow a private one; there is no way to pin a scope to a registry. The token for registry X is only sent to X (client.go builds the URL from the chosen BaseURL), so no cross-registry token leak was found.
- Impact: wrong-registry resolution; dependency confusion for private scopes.
- Root cause: credential store doubles as configuration; no scope->registry mapping.
- Recommendation: refactor - explicit `defaultRegistry` plus `scopes` mapping, normalised URL keys.
- Regression check: multi-registry test with a fixed order.
- Effort estimate: 3h-6h

#### F-REG-17: Credentials stored in plaintext; file permissions are not enforced on Windows
- Category: credentials
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Exploitability: local user/process with read access to the profile; backup tools.
- Location: credentials.go:182-188 (`~/.veld/credentials.json`), 214-227 (`WriteFile(..., 0600)`, dir 0700)
- Trigger/reproducer: file content `{"registries":{"http://localhost:18080":{"token":"vtk_DYPN...","username":"cli5487"}}}`; `icacls` on Windows shows inherited ACEs for other principals (e.g. a sandbox user group) - mode bits are meaningless there. Write is not atomic (no temp+rename) and read-modify-write is unlocked (concurrent logins can lose entries). On Unix 0600 applies only on creation (an existing looser file keeps its mode).
- Impact: standard "plaintext dotfile" exposure; acceptable on Unix, weaker on Windows; no OS keychain integration; `VELD_TOKEN`-style env override does not exist for CI.
- Root cause: simple file store.
- Recommendation: fix - keep file but chmod after write, atomic write, optional env token, consider keychain later.
- Regression check: perms test on Unix.
- Effort estimate: 2h-4h

#### F-REG-18: `Pack` publishes more than the package and ignores errors
- Category: tarball
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: low
- Exploitability: the person running `push` (accidental disclosure); symlinks planted in a repo by a third party.
- Location: tarball.go:20,22-34,63-80
- Trigger/reproducer: SOURCE only (not executed): any `*.veld` / `veld.config.json` at any depth under the config dir is included (nested projects, vendored `veld/packages/**` pulled from other orgs, test fixtures); a file symlink `x.veld -> ~/.ssh/id_rsa` would upload the target's content; `veld.config.json` (hooks, baseUrl, internal hostnames) always uploaded; `WriteHeader`/`io.Copy`/`Close` errors dropped; unreadable files silently skipped so a partial package can be published; non-deterministic bytes (mtime).
- Impact: accidental leakage and incomplete/corrupt packages; reproducible-build property lost.
- Root cause: dead `patterns` slice; no package manifest listing files.
- Recommendation: fix - explicit file list from the package manifest, `Lstat` and skip non-regular files, zero ModTime, check errors.
- Regression check: pack test with symlink, nested and unreadable files.
- Effort estimate: 2h-4h

#### F-REG-19: Email handling: SMTP proceeds without TLS, verification never enforced, token logged, HTML injection
- Category: server-auth
- Evidence status: source-supported defect (SMTP not exercised; not configured)
- Severity (user impact): low
- Security severity: low
- Exploitability: network attacker between server and SMTP host; log readers; any user (username in email).
- Location: email.go:97-106 (STARTTLS optional, then `loginAuth` has no TLS requirement: lines 33-35 vs stdlib PlainAuth), email.go:68 (To header from user input; `Rcpt` rejects CR/LF so injection is blocked in practice), auth.go:363 (full verify URL with token logged), auth.go:43-50 (email format unvalidated), email.go template `fmt.Sprintf` with raw username (HTML injection into the email), whole `email_verified` flag unused for authorization (REPRODUCED: unverified account publishes).
- Impact: SMTP credentials can be sent in cleartext after a stripped STARTTLS; account verification is cosmetic, so spam/squatting is unconstrained.
- Root cause: copy-paste mail code, verification is not a gate.
- Recommendation: fix - require TLS before AUTH (or implicit TLS), strip tokens from logs, escape template values, gate publishing/org creation on `email_verified` when SMTP is configured.
- Regression check: fake SMTP server that withholds STARTTLS.
- Effort estimate: 3h-5h

#### F-REG-20: Pull writes untrusted files into the project and can overwrite existing files; pulled `veld.config.json` can carry a hook
- Category: client
- Evidence status: suspected risk (the write was reproduced via MITM; the hook executing was not run)
- Severity (user impact): medium
- Security severity: medium
- Exploitability: malicious package author/registry; requires the user to pass `--out` pointing at the project (e.g. `--out veld`) or otherwise to use the pulled config.
- Location: cmd/veld/registry.go:211-216,238 (default out `veld/packages/@org/name`, `--out` unrestricted, `os.Create` overwrite); `hooks.postGenerate` in config (CLAUDE.md config section)
- Trigger/reproducer: MITM proxy output wrote `veld.config.json {"input":"app.veld","hooks":{"postGenerate":"echo PWNED > pwned.txt"}}` into `veld\packages\@org\hello\`. With the default location the config is not auto-detected (CLAUDE.md lists only `./veld.config.json` and `./veld/veld.config.json`); with `--out veld` it would overwrite the real project config.
- Impact: code execution on next `veld generate` in that misuse; and any `.veld` content is imported into code generation.
- Root cause: packages ship config files; extractor overwrites silently.
- Recommendation: fix - exclude `veld.config.json` from packages (or ignore it), refuse overwrite without `--force`, refuse `--out` that equals/contains the project root.
- Regression check: pull test with a config entry.
- Effort estimate: 2h-3h

#### F-REG-21: Operational gaps: no health endpoint, SIGTERM unhandled, no schema versioning, no TLS, no backups, zero tests
- Category: ops
- Evidence status: reproduced defect (health, migrations table, restart), source-supported (signals)
- Severity (user impact): medium
- Security severity: low
- Exploitability: operator.
- Location: server.go:84-140 (catch-all route serves SPA for `/healthz`: REPRODUCED 200 text/html 84 KB), serve.go:138 (`os.Interrupt` only), db.go:43-128 (no `schema_migrations`; REPRODUCED table list), storage.go (local FS only; DB row and file are not reconciled), `find internal/server internal/registry -name '*_test.go'` -> none, CLAUDE.md "Registry Server" section (no backup/restore, no HA)
- Trigger/reproducer: section 4. Restart re-ran the schema silently and data persisted (good); but a column change in the future cannot be versioned, and `ADD COLUMN IF NOT EXISTS` migrations run on every start by whichever instance starts first.
- Impact: orchestrators cannot probe readiness; `docker stop` kills in-flight uploads; upgrades are risky; no test net for any finding above.
- Root cause: prototype-level server.
- Recommendation: refactor - `/healthz` + `/readyz`, SIGTERM, versioned migrations, tests, documented backup of PG + blob store.
- Regression check: container smoke test with `docker stop`.
- Effort estimate: 6h-12h

#### F-REG-22: Documentation and config mismatches
- Category: config
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: low
- Exploitability: n/a
- Location: CLAUDE.md lines 54-58, 392-399 (`veld login|push|pull|serve`, `--secret mysecret`); cmd/veld/main.go:99 (only `veld registry ...` exists); serve.go:97-108; server.go:38 vs CLAUDE.md (secret "32+ chars"); registry.config.example.json (`secret` is a guessable placeholder); `RegistryConfig.Enabled` in config.go:97 is never read (`grep` finds no use)
- Trigger/reproducer: `veld login` / `veld serve` -> `unknown command`; `--secret mysecret` (8 chars) is rejected by `server.New` (min 16); BaseURL has no flag and SMTP port no env var, contrary to "flags > env > file".
- Impact: operators copy the example secret or hit errors.
- Root cause: docs not synced with code.
- Recommendation: fix - align docs, make the example secret invalid (empty) so startup fails, enforce >=32.
- Regression check: doc test running the documented commands.
- Effort estimate: 1h-2h

#### F-REG-23: Member emails and user IDs exposed to anonymous callers
- Category: server-authz
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Exploitability: anonymous.
- Location: orgs.go:80-103, db.go:323-340 (`ListOrgMembers` returns email), models.go OrgMember json tags
- Trigger/reproducer: E11: `GET /api/v1/orgs/acme9075` unauthenticated -> `hasEmail=true`. Anonymous `GET /orgs` also lists every org, and `/stats` is public.
- Impact: PII harvesting; enumeration of orgs.
- Root cause: shared DTO.
- Recommendation: fix - drop `email` from public DTOs (show to org admins only).
- Regression check: anon response must not contain `@`.
- Effort estimate: 1h

---------------------------------------------------------------------------------------------------

### 6. Threat-model summary

- Malicious package author (any registered user): can publish arbitrary bytes under their org (F-REG-13), write/overwrite/delete `*.tar.gz` outside the store and break other orgs (F-REG-02), plant stored XSS (F-REG-11), and ship decompression-bomb or many-file archives and config-with-hook files that hit consumers (F-REG-10, F-REG-20). Symlink/traversal tricks in the archive itself are neutralised by the current extractor (section 2), but by accident.
- Compromised token: scopes are decorative (F-REG-03); a read token upgrades to admin; expiry ignored (F-REG-05); a stolen JWT cannot be revoked (F-REG-14); the partial-2FA token is a full session (F-REG-01). Token storage itself is good (256-bit random, stored as sha256).
- Malicious registry operator / compromised registry: can serve any bytes and any `X-Veld-SHA256` (or none); the client has no anchor (lockfile/signature) (F-REG-09); can re-publish a version after deletion (F-REG-08). Operator also holds all tokens in transit.
- Network attacker: http registries are accepted without warning, tokens and packages travel in the clear, no cookie `Secure` flag (F-REG-09, F-REG-15); SMTP may downgrade (F-REG-19).
- Decompression bomb / resource exhaustion: reproduced on pull (1 GiB from 1.2 MiB; 100k files) and on publish (200 MiB unvalidated blob; no body limits) (F-REG-10, F-REG-13).
- Secret leakage: verification tokens logged (F-REG-19); `--secret` on the command line is visible in process listings; example config contains a known secret (F-REG-22); credentials plaintext (F-REG-17). No token appears in request logs; error messages from the client do not echo tokens.
- Private package enumeration: latent only, because private packages cannot be created through the API; but 4 inconsistent read paths would leak them (F-REG-06), plus anonymous member email listing (F-REG-23) and existence oracle via 401/403/404.

### 7. Assessment

- Server: not production-ready. Two high-severity, trivially reproducible auth/storage defects (F-REG-01, -02, -03), no abuse controls, no tests. The architecture (PostgreSQL + blob store + embedded SPA) is reasonable, but the implementation is a prototype. Recommend refactor-with-hardening of the core (auth, authz helper, storage keyed by id/sha, migrations) rather than patching individual handlers; the SPA should be rewritten or at least retrofitted with escaping + CSP.
- Client: small and understandable. `Unpack` should be rewritten (F-REG-10), verification made mandatory and anchored (lockfile, F-REG-09), registry selection made explicit (F-REG-16).
- Same binary? The registry belongs in a separate binary (e.g. `veld-registry`) or repo: it brings `lib/pq`, `x/crypto`, an HTTP server, SMTP and a web UI into every developer's CLI, enlarging its attack surface and supply chain, while the CLI only needs the ~500-line client. CLAUDE.md already states the two-binary rule differently (it counts `veld serve` inside the CLI); the code matches the docs but the separation would be cleaner.



## Appendix: audit-track notes (F-lsp-editors)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Veld LSP and editor integrations audit

Commit 6a47f81, reviewed 2026-10-21. Scratchpad: `...\scratchpad\lsp-audit\` (drive.py, extra.py, pos.py, session.log, extra.log, pos.log, proj/, vscode-copy/).

### Part A: LSP server source reading (internal/lsp, 1017 source lines, 134 test lines, 33.6% statement coverage)

- Command: `veld lsp` (cmd/veld/tooling.go:178-191) calls `lsp.NewServer().Run()`. A stale "lsp (placeholder)" comment sits at cmd/veld/contract.go:592.
- Transport (server.go:52-79): `Content-Length` framing. Header match is case-sensitive (`strings.HasPrefix(line, "Content-Length:")`, :63). Body is read with `io.ReadFull` (so partial reads are handled). Missing or zero length makes `Run()` return an error and the process exits 1 (:69-71). No JSON-RPC batch support (a body that is a JSON array fails to unmarshal and is silently skipped, :44-46). Malformed JSON is silently dropped (`continue`), with no ParseError reply.
- Dispatch (server.go:104-137): a single `switch`. Unknown methods get no reply at all. Requests with an id get no MethodNotFound, so the client hangs. `$/cancelRequest` is not handled (silently ignored). `shutdown` replies with a nil result. `exit` calls `os.Exit(0)` unconditionally (:116-117).
- Concurrency: `Run()` is one goroutine reading and dispatching synchronously (:34-49). `Handler.mu` (RWMutex) and `Server.mu` (write lock) exist but are never contended. A long parse blocks every later message (measured below). There is no `recover()` anywhere in internal/lsp (grep), so a parser/validator panic would kill the server.
- Documents (handler.go:219-259): map URI to text, plus map URI to last good AST. URIs are opaque strings and are never converted to a filesystem path. Diagnostics use the unsaved buffer (`documents[uri]`, handler.go:278), never disk. `didSave` just recomputes from the same buffer (server.go:170-181). `didClose` is not handled, so buffers are never released and diagnostics are never cleared.
- Sync: `textDocumentSync: 1` (Full) (handler.go:237); only `contentChanges[0].Text` is used (server.go:161).
- Imports: the LSP uses lexer, parser and validator on the single buffer. It never calls `internal/loader`, never looks up veld.config.json, and does not use rootUri or workspaceFolders (`Initialize()` ignores its params). Any file that references an imported model gets false "undefined type" errors, and cross-file features cannot work.
- Diagnostics (handler.go:324-396): reuses the CLI's `lexer.Tokenize`, `parser.Parse` and `validator.Validate`. Only the first lex or parse error is reported. The line is regex-extracted from the message text (`extractLine`, :367). The range is always the whole line, `character 0..1000` (:391-396). Columns are never computed, so there is no UTF-16 issue in diagnostic ranges (the range is simply coarse).
- Positions: all feature code treats `Position.Character` as a byte index into the line (completion.go:17-18, hover.go:130-153 `wordAtPosition`), and definition ranges use `len()` bytes (definition.go:112). There is no `positionEncoding` capability and no negotiation, so the server is effectively UTF-8, not the LSP default UTF-16.
- Stale AST: `parseDocument` returns early on lex/parse error without clearing `asts[uri]` (handler.go:261-273). Hover, completion and definition then use an old AST.
- Logging: none (stderr empty in every session).
- Capabilities advertised (verbatim): `{"textDocumentSync":1,"completionProvider":{},"hoverProvider":true,"definitionProvider":true}`.

### Part B: scripted session (drive.py, abbreviated pairs in session.log; edge cases in extra.log, pos.log)

Project: proj/veld/{veld.config.json, app.veld, models/user.veld, modules/auth.veld}. The CLI says "Contract is valid".

ServerCapabilities verbatim: `{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"textDocumentSync":1,"completionProvider":{},"hoverProvider":true,"definitionProvider":true}}}`

| Step | Result |
|---|---|
| didOpen app.veld (valid) | publishDiagnostics with `"diagnostics": null` (not `[]`) |
| didOpen modules/auth.veld (valid on disk) | FALSE POSITIVE: `line 7: module "Auth", action "GetUser": undefined output type "User"` (imports unresolved) |
| didChange adding `output: Unknwn` | diagnostic `line 8: ... undefined output type "Unknwn"`, range line 7 (the `action A {` line), char 0..1000. Not on the token. |
| Non-ASCII added before the error (earlier line, or same line) | Diagnostic is identical because it is line-only. No UTF-16 mismatch is possible, but no real column is given either. |
| Valid doc using local and imported models | `[]`/null. Parses with imports unresolved because both are declared locally or ignored. |
| completion after `method: ` | 40 items: all keywords, types, HTTP methods, local models. Identical to `input: ` and to top level. Not context-aware. |
| completion after `input: ` | Includes `Local` (same buffer) but NOT `User` (imported) |
| hover on `Local` in `input:` | Works (markdown with description and fields) |
| hover on imported `User` | `null` |
| definition on local `Local` | Works, same file, line 5, char 0..11 |
| definition on imported `User` | `null`. No cross-file jump. |
| references, rename, formatting, documentSymbol, semanticTokens/full, codeAction | No response at all (2 s timeout each). Server never replies with MethodNotFound. |
| unterminated string, followed by a newline (`description: "oops\n id: string`) | NO diagnostic (`null`). The lexer silently accepts it (internal/lexer/lexer.go:262-269), and the CLI `veld validate` also says "valid". |
| unterminated string at EOF / unclosed brace | Diagnostic `expected "}", got ""` on the last line. The server stays alive. |
| garbage, NUL byte, cyclic extends, 20,000-deep `model M {` | Correct errors, no crash |
| Unknown method with an id | NO reply (timeout 3 s) |
| Malformed JSON body | No reply, server survives |
| didOpen with no params | Server survives, publishes diagnostics for uri `""` |
| shutdown response | `{"jsonrpc":"2.0","id":19}`: the `result` member is missing (omitempty), a JSON-RPC 2.0 violation |
| `exit` without prior shutdown | exit code 0 (the spec requires 1) |
| shutdown then exit | exit code 0 |
| lowercase `content-length` header | Server exits 1 (`missing Content-Length header`) |
| id 0 and string ids | handled |
| hover at UTF-16 column on a line with 3 emoji before the word | `null`. At the byte column it returns the model. This confirms byte-offset semantics. |

Latency (this Windows machine, veld.exe built from HEAD):
- 200 back-to-back didChange (about 450-byte doc): 200 diagnostics notifications, last one 34 ms after the first send (send took 32 ms).
- 2.4 MB document (30,000 models) didOpen to diagnostics: 748 ms. completion on it: 184 ms.
- 5 consecutive didChange of the 2.4 MB doc: 3,722 ms total (about 744 ms each, strictly serial).
- Hover queued behind one 2.4 MB didChange: 684 ms (head-of-line blocking).
- No coalescing or debouncing of rapid changes. Every keystroke is fully re-lexed, re-parsed and re-validated.

### Part C: VS Code extension (editors/vscode)

- package.json: `main ./out/extension.js`, `activationEvents: []` (relies on the implicit onLanguage activation from `contributes.languages`, OK for engine ^1.85). It contributes language, TextMate grammar (498 lines, hand-written), semanticTokenScopes, snippets (veld.json 208 lines, veld-config.json 109 lines), jsonValidation for veld.config.json, 3 commands (`veld.validate`, `veld.generate`, `veld.generateDryRun`), and settings `veld.executablePath` and `veld.validateOnSave`.
- It does NOT use vscode-languageclient and does not launch `veld lsp`. `src/extension.ts` is 1,676 lines containing a TypeScript re-implementation of the language service: its own regex/line-based parser, diagnostics, completion, hover, definition, references, semantic tokens, document symbols (`class VeldLanguageServer`, :81; providers registered at :1550-1591). It reads veld.config.json itself to resolve aliases (`findProjectRoot` :89, `readAliases` :107).
- The three declared commands have NO handler: `registerCommand` and `child_process` never appear in extension.ts. `veld.executablePath` and `veld.validateOnSave` are never read. The extension never finds, downloads or invokes the veld binary (no PATH lookup, no download, no checksum, no version pinning). So offline behaviour is a non-issue, but the settings and the commands are dead.
- The only generated input consumed is `veld-language-spec.ts` (KEYWORDS, HTTP_METHODS, BUILTIN_TYPES, DIRECTIVES, SPECIAL_TYPES). The grammar is NOT generated from it, and completion and diagnostics keep their own hard-coded sets (e.g. `ACTION_KEYS`).
- Tests: none. Publish: `vsce package`/`vsce publish` via PUBLISHING.md only; no CI job builds the extension.
- Build check (in a copy under lsp-audit/vscode-copy, network allowed): `npm install` OK (generated a package-lock.json; the repo tracks none; `npm audit` reports 3 vulnerabilities, 2 high, in the dev-only vsce tree such as linkify-it). `npm run compile` (tsc) succeeds, no errors. The freshly compiled `out/extension.js` and `out/veld-language-spec.js` DIFFER from the committed `out/` files (the committed veld-language-spec.js is 1,142 bytes, i.e. stale).

### Part D: JetBrains plugin (editors/jetbrains)

- Build: Gradle (wrapper checked in), Kotlin 2.1.10, `org.jetbrains.intellij.platform` 2.14.0, target IntelliJ IDEA Community 2025.1, since-build 241, until-build 263.*, JVM 21. No Gradle on PATH and no offline distribution, so a build was NOT attempted (it needs network and the wrapper download).
- It does not use LSP4IJ or the native LSP API (plugin.xml has no lsp reference). It is a hand-written PSI plugin: `VeldLexer.kt` (290 lines, keywords hard-coded at :272), `VeldParser.kt` (364), `VeldAnnotator.kt` (696, in-editor heuristics, own `ACTION_KEYS` at :71), `VeldCompletionContributor.kt` (457), `VeldProjectService.kt` (457), reference contributor, structure view, documentation provider, color settings, and more.
- Kotlin: about 4,090 lines of .kt across 31 files (4,667 lines including plugin.xml, schema copy and resources). `VeldLanguageSpec.kt` (52 lines) is generated and consumed by Annotator, CompletionContributor, DocumentationProvider and ReferenceContributor. The lexer, syntax highlighter and annotator key sets are hand-maintained duplicates.
- `VeldExternalAnnotator` shells out to `veld validate` from PATH with cwd = `project.basePath` (VeldExternalAnnotator.kt:25-27) and regex-parses coloured CLI output. It validates the on-disk project, not the unsaved buffer, and the project root may not be where veld.config.json lives.

#### Maintenance cost estimate (hours/month)

Assumptions: one language change per month (new keyword, directive or annotation), one IDE/VS Code API bump per quarter, no new features.
- VS Code as it is: about 8-12 h/month. The TS re-implementation (1,676 lines) plus the grammar and snippets must be kept in step with the Go parser by hand. ACTION_KEYS drift already causes false errors (F-LSP-13).
- JetBrains as it is: about 12-20 h/month. The same duplicated language logic in Kotlin (lexer, parser, annotator, completion), plus a platform bump every half year (until-build 263), plus plugin verifier and signing.
- Both together: about 20-32 h/month and growing with every language feature, and each re-implements validation that the Go compiler already does.
- Alternative: make `veld lsp` the single implementation (a one-off 60-100 h investment to fix F-LSP-01 to 11). Then VS Code becomes a thin vscode-languageclient wrapper (about 150 lines) plus grammar and snippets (about 2-3 h/month), and JetBrains could use LSP4IJ with a small config (about 2-4 h/month) or stay on its PSI plugin for highlighting only.

### Part E: language metadata pipeline

- `cmd/generate-language/main.go` (178 lines) reads `language.GetLanguageSpec()` (internal/language/constants.go and veld.go) and writes `veld-language.json`, `editors/vscode/src/veld-language-spec.ts`, and `editors/jetbrains/.../VeldLanguageSpec.kt`.
- Consumers: VS Code (5 sets used) and JetBrains (4 files) read the generated files. `veld-language.json` is gitignored (`.gitignore:69`, `git ls-files` returns nothing), so it is not tracked and nothing in the repo reads it.
- Only `cmd/generate-language` imports `internal/language`. The Go lexer, parser, validator, LSP completion and formatter do NOT. The CLAUDE.md claim "single source of truth" is true only for the two editor specs, not for the compiler.
- Ran `go run ./cmd/generate-language`: `git status --short` and `git diff --stat` show no changes to the tracked generated files (only unrelated untracked docs/ files from another process), and `veld-language.json` is byte-identical to the earlier copy. No drift at HEAD, so nothing needed restoring.
- CI (`.github/workflows/ci.yml:38-67`, job `drift`): runs the generator and `scripts/sync-schema.sh`, then `git diff --quiet`, and exits 1 on any diff. So it does fail on drift of tracked files. Gaps: `git diff` ignores untracked new files; the check only proves that the generated files match `constants.go`, not that `constants.go` matches the parser (no test links them); no job builds or tests the editors.
- Schema: `scripts/sync-schema.sh` copies `editors/veld-config.schema.json` to `editors/vscode/veld-config.schema.json` and `editors/jetbrains/src/main/resources/schemas/veld-config.schema.json`. Both copies are identical to the canonical file (`diff` clean, sync script run produced no changes).

### Part 1: capability matrix

| Method | Implemented? | Works in scripted session? | Notes |
|---|---|---|---|
| initialize | yes | yes | Ignores params, no positionEncoding, no workspace folders |
| initialized | yes (no-op) | yes | |
| shutdown | partial | replies without `result` | Spec violation |
| exit | yes | exit 0 | Always 0 |
| textDocument/didOpen | yes | yes | |
| textDocument/didChange | full sync only | yes | First change only |
| textDocument/didSave | yes | not separately driven | Same as change |
| textDocument/didClose | no | n/a | Leaks buffers, diagnostics stay |
| publishDiagnostics | yes | yes | `null` instead of `[]`, whole-line ranges, first error only, no import resolution |
| completion | yes | yes, but context-blind | 40 identical items everywhere |
| hover | yes | local only | Byte columns |
| definition | yes | local only | Imports return null |
| references | no | no reply | |
| rename | no | no reply | |
| formatting | no | no reply | `veld fmt` exists in the CLI but is not exposed |
| documentSymbol | no | no reply | |
| semanticTokens | no | no reply | VS Code does its own |
| codeAction | no | no reply | |
| $/cancelRequest | no | ignored | Moot, requests are serial |
| workspace/didChangeWatchedFiles | no | n/a | |
| workspace folders | no | n/a | |
| unknown method | no MethodNotFound | no reply (hang) | |

### Verdicts

- Position encoding: UTF-8 byte offsets in practice (completion, hover and definition), no `positionEncoding` negotiated, so non-ASCII lines break for UTF-16 clients (reproduced for hover). Diagnostic ranges are whole-line, so they are unaffected.
- Unsaved buffer: yes for the single file, diagnostics come from the in-memory buffer. But imported files are never loaded (neither from disk nor from other open buffers), and no config is found. Multi-file projects are not supported.
- Windows URIs: URIs are opaque map keys and never converted to paths. No Windows breakage, but nothing that needs a path can work. Two spellings of one file (`file:///d%3A/...` vs `file:///D:/...`) would be two documents (suspected, not driven).

### Findings

#### F-LSP-01: LSP ignores imports, so valid multi-file projects get false errors and no cross-file features
- Category: lsp-diagnostics
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/lsp/handler.go:261-273, 324-365; internal/lsp/server.go (no loader use)
- Trigger/reproducer: didOpen `proj/veld/modules/auth.veld` (valid per `veld validate`), which has `import @models/user` and `output: User`. See session.log.
- Expected vs actual: expected no diagnostics; actual `line 7: module "Auth", action "GetUser": undefined output type "User"`. Hover and definition on imported `User` return null; completion omits imported models.
- Impact: every file in a real project that uses an import shows a spurious error. The server is unusable beyond single-file toy contracts.
- Root cause: `computeDiagnostics` validates one buffer in isolation; `internal/loader` and config discovery are not used; `initialize` ignores rootUri.
- Recommendation: refactor — load via the loader from the nearest veld.config.json with open buffers overlaid on disk.
- Regression check: add an LSP test on a temp project where an importing file yields zero diagnostics and definition on an imported model returns the other file's URI.
- Effort estimate: 16h–30h

#### F-LSP-02: Unknown requests get no response; malformed JSON gets no ParseError
- Category: lsp-robustness
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/server.go:44-46, 104-137
- Trigger/reproducer: send `foo/bar` with an id; also `references`, `rename`, `formatting`, `documentSymbol`, `codeAction`.
- Expected vs actual: expected error -32601 MethodNotFound (and -32700 for bad JSON); actual silence, so the client request never completes.
- Impact: any client that sends an optional request (many do) can hang or time out; the server hides its own unsupported surface.
- Root cause: the `switch` has no default branch and the parse error path is `continue`.
- Recommendation: fix — add a default MethodNotFound reply for requests (id present) and a ParseError reply.
- Regression check: test sends an unknown method and asserts error code -32601 within a timeout.
- Effort estimate: 1h–2h

#### F-LSP-03: Position handling uses byte offsets with no positionEncoding negotiation
- Category: lsp-positions
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/completion.go:17-18; internal/lsp/hover.go:130-153; internal/lsp/definition.go:112,125,138
- Trigger/reproducer: pos.py — line `// <3 emoji> Local`, hover at UTF-16 column 11 returns null, at byte column 17 returns the model.
- Expected vs actual: expected UTF-16 code-unit columns (the LSP default), or an advertised `positionEncoding: utf-8`; actual raw byte indexing with nothing advertised. Slicing a multi-byte rune is also possible in completion (`line[:pos.Character]`).
- Impact: hover, completion and definition misbehave after any non-ASCII character on the line (comments, descriptions). Possible index slicing into the middle of a rune.
- Root cause: string indexing by `Position.Character` without conversion.
- Recommendation: fix — convert UTF-16 columns to rune/byte offsets in one helper, or negotiate utf-8.
- Regression check: unit test with emoji and accented characters before the cursor for hover, completion and definition.
- Effort estimate: 3h–6h

#### F-LSP-04: Diagnostics are first-error-only, whole-line, and often on the wrong line
- Category: lsp-diagnostics
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/handler.go:329-395
- Trigger/reproducer: `output: Unknwn` is reported on the `action A {` line (0-based 2 or 7), not on the token; range is `character 0..1000` always.
- Expected vs actual: expected a range over `Unknwn`; actual the whole line, 1000 characters wide, and only the first lex/parse error.
- Impact: squiggles cover entire lines (and line 1 on a message with no line), and a file with several errors shows one at a time.
- Root cause: errors carry only a line number, which the server regex-extracts from `err.Error()`; the lexer tracks no columns.
- Recommendation: refactor — return structured errors (line, column, length) from the internal/errors package instead of parsing message text.
- Regression check: golden test asserting range start/end for a type error and a syntax error.
- Effort estimate: 8h–16h

#### F-LSP-05: Lexer silently accepts unterminated string literals (CLI too)
- Category: lsp-diagnostics
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:258-270
- Trigger/reproducer: `model M {\n  description: "oops\n  id: string\n}` yields no diagnostic from the LSP and `veld validate` prints "Contract is valid".
- Expected vs actual: expected an "unterminated string" error; actual the string ends at the newline silently.
- Impact: typos in descriptions or import paths pass validation; with a missing closing quote on an import the wrong path is used.
- Root cause: the closing quote is optional (`if ... == '"' { pos++ }`) with no else branch.
- Recommendation: fix — return a lexer error when a string ends at newline/EOF.
- Regression check: lexer unit test for an unterminated string at newline and at EOF.
- Effort estimate: 1h–2h

#### F-LSP-06: Last good AST is retained when the buffer stops parsing
- Category: lsp-sync
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/lsp/handler.go:261-273
- Trigger/reproducer: open a doc with model Local, then change to a buffer with a syntax error; hover, completion and definition still use the old AST.
- Expected vs actual: either empty results or an explicit "stale" behaviour; actual silently returns old model data (hover shows deleted models; positions point to old lines).
- Impact: wrong go-to-definition lines while editing.
- Root cause: early return on error without updating `asts`.
- Recommendation: fix — keep a separate stale AST only for completion names, and drop it for definition and hover.
- Regression check: test that after a parse error definition returns null, not the old location.
- Effort estimate: 1h–3h

#### F-LSP-07: JSON-RPC conformance defects (shutdown result, null diagnostics, exit code)
- Category: lsp-transport
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/protocol.go:410-415 (`Result ... omitempty`), :511 (`Diagnostics`); internal/lsp/server.go:113-117; handler.go:284 (nil slice)
- Trigger/reproducer: shutdown returns `{"jsonrpc":"2.0","id":19}` (no `result`); empty diagnostics serialize as `"diagnostics": null`; `exit` without `shutdown` returns 0.
- Expected vs actual: expected `"result": null`, `"diagnostics": []`, and exit code 1 when exit arrives without shutdown.
- Impact: strict clients (and some Neovim/Emacs/Helix configs) can reject the response or treat `null` diagnostics as an error; shell supervisors see a clean exit after an unclean shutdown.
- Root cause: `omitempty` on `Result` and a nil slice for no diagnostics; `os.Exit(0)`.
- Recommendation: fix — always emit `result` for successful replies, use empty slices, track the shutdown flag.
- Regression check: transport test that checks the raw JSON for `"result":null` and `"diagnostics":[]`.
- Effort estimate: 2h–3h

#### F-LSP-08: Single-threaded, un-debounced, no recover — slow parses block everything and a panic kills the server
- Category: lsp-robustness
- Evidence status: suspected risk
- Severity (user impact): medium
- Security severity: low
- Location: internal/lsp/server.go:34-49; internal/lsp/handler.go (no recover anywhere in internal/lsp)
- Trigger/reproducer: 2.4 MB doc: each didChange costs about 744 ms, and a hover sent after it waited 684 ms. No panic was reproduced (garbage, deep nesting and empty inputs were all handled).
- Expected vs actual: expected cancellable/debounced work with panic isolation; actual serial full re-analysis per message.
- Impact: typing in a very large contract lags; any latent parser/validator panic terminates the editor's language service (the CLI binary exits) with no log.
- Root cause: synchronous dispatch, no coalescing of changes, no `defer recover()`.
- Recommendation: fix — wrap dispatch in recover, debounce diagnostics per URI, and run requests off the read loop.
- Regression check: test feeding an input known to panic (fuzz target) and asserting the server answers the next request.
- Effort estimate: 6h–12h

#### F-LSP-09: Feature surface far below the documented editor support
- Category: lsp-features
- Evidence status: documentation mismatch
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/handler.go:234-243; README.md:429
- Trigger/reproducer: capability list shows only sync, completion, hover, definition. No didClose, references, rename, formatting, documentSymbol, semanticTokens, codeAction, workspace support.
- Expected vs actual: README advertises "Any LSP editor ... Diagnostics, completions, hover, go-to-definition"; that is technically listed, but without import resolution (F-LSP-01) and with context-blind completion (F-LSP-10) it is much weaker than the VS Code and JetBrains plugins.
- Impact: LSP-only editors (Neovim, Helix, Zed) get a weaker experience; no memory/diagnostic cleanup on close.
- Root cause: the LSP is a first-pass server and test coverage is 33.6%.
- Recommendation: refactor — decide on the LSP as the single engine and add didClose, documentSymbol, references and formatting (reusing `veld fmt`) first.
- Regression check: capability-vs-handler test asserting every advertised provider answers.
- Effort estimate: 24h–40h

#### F-LSP-10: Completion is context-blind and uses hard-coded lists
- Category: lsp-features
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/completion.go:24-75
- Trigger/reproducer: completion after `method: `, after `input: ` and at top level all return the same 40 items.
- Expected vs actual: expected HTTP methods after `method:`, model names after `input:`/`output:`, top-level keywords only at top level; actual everything everywhere, with imported models missing. Keyword/type/method lists are copied literals, not from `internal/language`, and include `from`/`default`/`WS` handling that differs from the generated spec.
- Impact: noisy completion and no real help while writing contracts.
- Root cause: `prefix` is computed but only used for the import check; there is no cursor-context classification.
- Recommendation: refactor — classify the cursor context from the line prefix and source the candidate lists from `internal/language`.
- Regression check: table test of context to expected labels.
- Effort estimate: 6h–10h

#### F-LSP-11: VS Code extension re-implements the language service in TypeScript and does not use `veld lsp`
- Category: vscode
- Evidence status: source-supported defect
- Severity (user impact): high
- Security severity: none
- Location: editors/vscode/src/extension.ts:81-1675; editors/vscode/package.json
- Trigger/reproducer: grep for `languageclient`, `child_process`, `spawn`, `veld lsp` in editors/vscode finds nothing.
- Expected vs actual: expected a thin client over the Go compiler; actual a 1,676-line parallel parser/validator that can disagree with the CLI.
- Impact: two (VS Code TS, Go LSP) plus a third (JetBrains Kotlin) implementations of Veld semantics; every language change must be made three times.
- Root cause: the extension predates or ignores the Go LSP; the LSP is too weak to adopt (F-LSP-01).
- Recommendation: rewrite — once the LSP is fixed, replace the TS service with a vscode-languageclient launcher.
- Regression check: an extension test that starts `veld lsp` and checks a diagnostic for a known-bad contract.
- Effort estimate: 20h–40h (after F-LSP-01 to 10)

#### F-LSP-12: VS Code contributes commands and settings that do nothing
- Category: vscode
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: editors/vscode/package.json (commands veld.validate/generate/generateDryRun, settings veld.executablePath/validateOnSave); editors/vscode/src/extension.ts (no registerCommand, no getConfiguration)
- Trigger/reproducer: run "Veld: Generate Code" from the palette — expected "command not found" because no handler is registered (not run in a VS Code host; established by grep).
- Expected vs actual: documented commands and settings exist; actual they are never wired. The README/PUBLISHING docs describe the binary being on PATH.
- Impact: the advertised "Validate on save" and "Generate" actions silently do not exist.
- Root cause: manifest and implementation diverged.
- Recommendation: fix — either wire the commands to the CLI (using `veld.executablePath`) or remove them.
- Regression check: a manifest-vs-registerCommand consistency test.
- Effort estimate: 3h–6h

#### F-LSP-13: VS Code (and JetBrains) validators reject valid action directives
- Category: vscode
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: editors/vscode/src/extension.ts:~497-506 (`ACTION_KEYS`: method, path, input, output, query, stream, middleware, errors, description); editors/jetbrains/.../VeldAnnotator.kt:71-72 (adds `response`, still no `headers`/`emit`)
- Trigger/reproducer: an action with `headers: { ... }` (documented in CLAUDE.md and accepted by internal/parser/parser.go:939) gets "Unknown action directive 'headers'" as an Error; same for `emit` (parser.go:985) and, in VS Code, `response`. Established by reading, not by running a VS Code host.
- Expected vs actual: expected no error since the parser accepts them; the generated `VELD_SPEC.directives` itself includes headers, response and emit.
- Impact: red squiggles on valid contracts.
- Root cause: hand-maintained key sets that are not derived from the generated spec.
- Recommendation: fix — derive the sets from `VELD_SPEC.directives` (or drop client-side validation per F-LSP-11).
- Regression check: test that every directive accepted by internal/parser appears in each editor's action key set.
- Effort estimate: 1h–2h

#### F-LSP-14: Committed `editors/vscode/out/` is stale and the extension has no tests or lockfile
- Category: vscode
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Location: editors/vscode/out/*.js (tracked); no package-lock.json tracked
- Trigger/reproducer: `npm install && npm run compile` in a copy succeeds, but `cmp` shows the fresh `out/extension.js` and `out/veld-language-spec.js` differ from the committed ones (the committed spec file is 1,142 bytes). `npm audit`: 3 vulnerabilities (2 high) in dev dependencies.
- Expected vs actual: build artifacts should not be committed, and builds should be reproducible.
- Impact: confusion about what ships; unpinned dev dependencies; no CI job would catch a broken extension.
- Root cause: `out/` committed once and never refreshed; no lockfile; no editor CI.
- Recommendation: fix — untrack `out/`, commit a lockfile and add a compile job to CI.
- Regression check: CI job running `npm ci && npm run compile` on editors/vscode.
- Effort estimate: 2h–4h

#### F-LSP-15: JetBrains plugin is a second hand-written language implementation with PATH-based CLI validation
- Category: jetbrains
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: low
- Location: editors/jetbrains/src/main/kotlin/dev/veld/jetbrains/VeldLexer.kt:272, VeldParser.kt, VeldAnnotator.kt:71, VeldExternalAnnotator.kt:25-27
- Trigger/reproducer: reading; no Gradle build was possible (Gradle not installed, no offline distribution).
- Expected vs actual: only `VeldLanguageSpec.kt` is generated; the lexer keywords, annotator key sets and completion lists are duplicated by hand. The external annotator runs `veld` from PATH (a PATH-planted binary in an untrusted project would run) in `project.basePath`, validating the saved on-disk files rather than the buffer. Not verified in an IDE.
- Impact: JetBrains drifts from the compiler and from VS Code; about 4,090 lines of Kotlin to maintain.
- Root cause: no shared engine; the plugin predates the LSP.
- Recommendation: defer — keep the PSI plugin for highlighting and move diagnostics/completion to LSP4IJ once the LSP is solid.
- Regression check: a unit test comparing the lexer's keyword set to `VeldLanguageSpec.KEYWORDS`.
- Effort estimate: 20h–40h (for LSP4IJ migration)

#### F-LSP-16: `internal/language` is not consumed by the compiler, so "single source of truth" is false and CI cannot detect parser/spec drift
- Category: language-metadata
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/language/*.go (imported only by cmd/generate-language/main.go); .github/workflows/ci.yml:38-67
- Trigger/reproducer: `grep -rl "internal/language\"" --include=*.go` returns only `cmd/generate-language/main.go`.
- Expected vs actual: CLAUDE.md calls it the single source of truth; actually lexer, parser, validator, LSP completion and format each hard-code their own keyword/directive lists. The CI drift job only proves the generated editor files equal `constants.go`. `go run ./cmd/generate-language` produced no diff at HEAD, but a parser change (e.g. a new directive) would pass CI without updating editors.
- Impact: the editors stay silently wrong when the language changes (F-LSP-13 is a live example).
- Root cause: spec is documentation-grade data, not wired into the parser.
- Recommendation: refactor — add a Go test asserting parser-accepted directives, keywords and types equal the language spec, and have the LSP completion read it.
- Regression check: that test, run in `go test ./...`.
- Effort estimate: 4h–8h

#### F-LSP-17: CI drift check cannot see new untracked files and the JSON output is untracked
- Category: schema-sync
- Evidence status: suspected risk
- Severity (user impact): low
- Security severity: none
- Location: .github/workflows/ci.yml:58-66; .gitignore:69
- Trigger/reproducer: the job uses `git diff --quiet`, which ignores untracked files; `veld-language.json` is gitignored yet CLAUDE.md lists it as a generated artifact.
- Expected vs actual: schema sync itself is fine (both plugin copies are byte-identical to editors/veld-config.schema.json, and re-running sync-schema.sh changed nothing). The risk is only that a newly added generated file would not fail CI.
- Impact: low; minor drift blind spot and an inconsistent claim about veld-language.json being a tracked artifact.
- Root cause: `git diff` vs `git status --porcelain`.
- Recommendation: fix — use `git status --porcelain` in the drift job and document that veld-language.json is a local build artifact.
- Regression check: touch a new generated file in a throwaway CI branch and confirm failure.
- Effort estimate: 0.5h–1h

### Recommendation

Primary editor: VS Code. It is the larger audience, already has the better-maintained client, and can become a thin vscode-languageclient wrapper over `veld lsp`. The one-off cost is fixing the Go LSP (F-LSP-01 to 10, roughly 60-100 h), after which VS Code drops to about 2-3 h/month. JetBrains costs more to keep alive (about 12-20 h/month for a Kotlin PSI plugin plus platform bumps) and should be deferred, then moved to LSP4IJ against the same server.



## Appendix: audit-track notes (G-cli-dist-ci)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

## Veld CLI / config / distribution / CI / website / tests audit

Commit 6a47f81, reviewed 2026-10-21. Binary built with Go 1.26 to `scratchpad/cli-audit/veld.exe`. All experiments ran on copies in `scratchpad/cli-audit/` (sandboxes p1..p6, sb, cfg, ex). No tracked file modified.
Note: every command exits 1 on any failure (cobra error -> `os.Exit(1)` in main.go:105; direct `os.Exit(1)` at contract.go:48, contract.go:471, init.go:31). There is no distinction between usage / validation / IO errors, and no `--json` flag anywhere.

### PART A - CLI inventory

Registered in main.go:88-101. Hidden = not in `--help` (but advertised in the Long help text / README).

| command | file:line | flags | reads config? | exit on failure (tested) | machine-readable | writes |
|---|---|---|---|---|---|---|
| (root) | main.go:55 | -v/--verbose, -q/--quiet (both parsed, near-unused), --version | no | no args = prints help, exit 0; `--version` -> `veld version 0.1.0` exit 0 (source builds always say 0.1.0; ldflags only in release.yml) | none | - |
| init | init.go:20 | none (fully interactive; works with stdin closed, takes defaults) | checks for existing config | 2nd run exit 1, no files touched (mtimes identical); BUT see F-CLI-05 (overwrites README/veld/*.veld when no config exists) | none | veld/*, README.md, may rewrite veld/veld.config.json (init.go:788), runs setup |
| generate | generate.go:700 | --backend --frontend --input --out --incremental --dry-run --setup --validate --strict --force --backend-framework --frontend-framework --server-sdk --service-sdk | yes (BuildResolved) | missing config exit 1; missing input exit 1; unknown backend exit 1 (but output already deleted, F-CLI-01) | none | out dirs (RemoveAll first), `<configDir>/.veld.lock.json`, `.veld-cache.json`, README.md in out |
| watch | watch.go:26 | --backend --frontend --input --out | yes | no config exit 1; generation errors do not exit (verified: bad edit printed error, next good edit regenerated). Ctrl-C handled by `signal.NotifyContext(os.Interrupt)` (SIGTERM not handled; Windows Ctrl-C not testable here, `timeout` kill gave 124) | none | same as generate (initial runClean) |
| clean | generate.go:1144 | none (ignores --out) | yes | no config exit 1 | none | deletes out dirs recursively + lock + cache |
| validate [file] | contract.go:29 | positional file | yes (or file arg) | invalid contract exit 1 (semantic via os.Exit(1), parse via error); workspace config exit 1 ("no input file") | none (stderr text) | - |
| ast [file] (hidden) | contract.go:59 | file | yes | exit 1 on error; reproduced exit 0 + JSON on valid | JSON on stdout | - |
| lint | contract.go:86 | --exit-code | yes | exit 1 only if validate fails or (error-level issue or --exit-code with any issue). Warning-only without flag exit 0 (verified). Error-level lint rules unreachable (F-CLI-12) | none | - |
| diff | contract.go:343 | --stat --exit-code | yes | no lock: prints everything as new, exit 0; with --exit-code and changes exit 1 | none | - |
| deps | contract.go:288 | --validate | yes | n/a (workspace only) | text | - |
| openapi / export openapi | contract.go:156 | -o | yes | validation failure exit 1 | JSON on stdout | -o file |
| graphql | contract.go:197 | -o | yes | exit 1 | SDL stdout | -o file |
| schema | contract.go:237 | --format prisma/sql, -o | yes | exit 1 | stdout | -o file |
| docs | contract.go:483 | --format html/markdown, -o | yes | exit 1 | file | docs.html / docs.md in cwd |
| fmt [files] (hidden) | contract.go:594 | -w | yes | exits 0 even if files "would change" (no --check) | none | rewrites .veld files with -w |
| setup | tooling.go:44 | --backend-dir --frontend-dir --backend --frontend --input --out | yes | empty dir exit 1 ("could not load config") | none | patches tsconfig/vite/package.json etc. |
| ci | registry.go:432 | --backend --frontend --input --out --strict | yes | --strict + breaking change: exit 1 (works, verified); does NOT clean first | none | generate + setup |
| lsp | tooling.go:179 | none | no | n/a | LSP stdio | - |
| completion | tooling.go:329 | shell arg | no | n/a | script on stdout | - |
| doctor | tooling.go:195 (`newDoctorCmd`) | - | - | NOT REGISTERED: `veld doctor` -> `unknown command "doctor"` exit 1 (F-CLI-04) | - | - |
| export {openapi,graphql,schema,docs,agents} | registry.go:253 | per subcommand | yes | as above | stdout/-o | -o file |
| registry {info,list,versions,login,logout,push,pull,serve,init,token create} | registry.go:297+ | --registry --token --org --name --version --out --addr --dsn --secret --storage --base-url | credentials file / config | network errors exit 1 | JSON from server only | credentials store, packages |

Top-level `veld login|logout|push|pull|serve` (documented in CLAUDE.md and README) are NOT top-level: `veld login` -> unknown command (they live under `veld registry`). Documentation mismatch (F-CLI-14).

#### Experiment log (exit codes / stderr)

| experiment | result |
|---|---|
| `veld` | help, exit 0 |
| `veld --version` | `veld version 0.1.0`, exit 0 |
| `generate` (no config) | exit 1 `no input file (use --input or create veld/veld.config.json)` |
| `generate --input=missing.veld` | exit 1, absolute-path OS error |
| `generate --backend=nonexistent` (config present) | exit 1 `unknown backend/tool ... (supported: ...)` but previous output already deleted (F-CLI-01) |
| `generate --out=..` | exit 1 rejected (basename check) |
| `generate --out=/` , `C:\`, `C:/`, `../..`, `.` | exit 1 rejected (validateOutPath, config.go:365). (Under Git Bash `/` is mangled to `C:/Program Files/Git`; with MSYS_NO_PATHCONV=1 it is rejected.) |
| `generate --out=../veld` (= the directory holding the .veld sources) | ACCEPTED; `runClean` deleted the whole `veld/` source directory, then failed `reading ...app.veld` (F-CLI-02) |
| `clean` with `out: ./src` containing `business.ts` | `src/` removed entirely, exit 0, no prompt (F-CLI-03) |
| invalid contract then `generate` | exit 1, and previous generated dirs + lock + cache are gone (F-CLI-01) |
| `generate --dry-run` | writes no output, but DOES create `veld/.veld-cache.json` (before/after listing diff) (F-CLI-06) |
| `generate --strict` / `--force` / plain with a removed action vs lock | all exit 0, nothing blocked, no prompt (gate dead, F-CLI-01) |
| `ci --strict` same scenario | exit 1, "Breaking changes detected - aborting" (works) |
| interactive prompt (stdin closed) | unreachable in practice; code path `promptContinue` (util.go:22) returns false on EOF -> "generation aborted" exit 1, no hang (source) |
| `init` twice | 2nd exit 1, mtimes + md5 unchanged |
| `init` where README.md and veld/app.veld exist but no config | exit 0, both OVERWRITTEN (F-CLI-05) |
| `validate` invalid | exit 1; unknown type and parse error both exit 1 |
| `lint --exit-code` on warnings | exit 1; without flag exit 0 |
| `doctor` | exit 1 unknown command |
| `setup` empty dir | exit 1 |
| `diff` no lock | exit 0, lists all files as new |
| `watch` 12s with edits | initial gen ok, edit -> regenerated in ~80ms, garbage edit -> error printed, process stays alive, revert -> regenerated; killed by timeout (exit 124) |

### PART B - Config

Source: internal/config/config.go (556 lines), config_test.go (804 lines, 29 tests).

Precedence (verified by experiment): **CLI flag > flat field > nested field > default**. The `normalize()` code (config.go:137-150) only copies nested values when the flat field is EMPTY, so flat wins over nested. Experiment: `{"backend":"python","backendConfig":{"target":"node-ts"}}` generated Python. This contradicts CLAUDE.md ("Nested takes precedence if both are present") and the comment at config.go:34 (F-CLI-07).
Exception: `--out` does not beat `backendConfig.out`/`backendOut` (backend dir stays at config value, only frontend follows `--out`) (F-CLI-08).

| item | finding |
|---|---|
| backend values accepted | node, node-ts, node-js, python, go, rust, java, csharp, php (+ tools: openapi, database, dockerfile, cicd, env, scaffold-tests). Aliases: node->node-ts, js/javascript/node-javascript->node-js, node-typescript->node-ts. `express`, `python-flask`, `flask`, `typescript`, `nodejs` -> exit 1 `unknown backend/tool` (framework is a separate field) |
| frontend aliases | flutter->dart, ts->typescript, hooks/react-hooks->react, js->javascript |
| default frontend | typescript for node backends, none otherwise |
| path resolution | config discovered ONLY in `./veld.config.json` or `./veld/veld.config.json` (no walk-up). `input`, `out`, `dir` are relative to the CONFIG FILE dir, not cwd (verified: `--out=x` is also relative to config dir). Exception: passing `--input` re-bases everything to cwd (config.go:324-327), so `out` from the config then resolves against cwd (verified `./o` vs `./veld/o`). Inconsistent (F-CLI-09) |
| aliases defaults | models, modules, types, enums, schemas, services, lib, common, shared (identity mapping); user entries merged |
| `$schema` | accepted and ignored |
| unknown keys | silently ignored (`bogusKey` accepted; plain `json.Unmarshal`, no DisallowUnknownFields). Typos like `"outt"` silently fall to defaults (F-CLI-09) |
| comments / trailing commas | rejected with raw encoding/json message, no line/column: `parsing veld.config.json: invalid character '/' looking for beginning of object key string` (design limitation, low) |
| wrong type | `json: cannot unmarshal number into Go struct field RawConfig.input of type string` (decent) |
| empty file | `unexpected end of JSON input` |
| `hooks.postGenerate` | executed with `exec.Command("sh","-c", cmd)` (generate.go:29), cwd = config dir, stdout/stderr inherited. NOT `cmd /C` on Windows: needs `sh` on PATH (works under Git Bash; fails on plain cmd/PowerShell machines). Hook failure only prints a warning; `exit 3` hook -> veld exit 0 (verified). Design consideration: a config file is executable code (anyone who can edit veld.config.json, e.g. a PR to a CI repo, gets shell execution on `veld generate`/`veld ci`) - same trust model as npm scripts, not a vuln but worth documenting (F-CLI-10) |
| workspace entries | `out` not validated by validateOutPath (config.go:360-377 applies only to cfg.Out/BackendOut/FrontendOut); workspaceOutputDirs (config.go:481) feeds runClean directly, so a workspace `out: "."` is deleted wholesale. Not tested destructively |
| validateOutPath | rejects only basename `..`, `.`, `/`, `\`. Accepts any other directory incl. the project root's own folder name, source dirs, absolute paths such as `C:\Users` (F-CLI-02/03) |

### PART C - Distribution wrappers

| wrapper | binary acquisition | version pin | checksum | platforms | lifecycle / offline | grade |
|---|---|---|---|---|---|---|
| npm `@maayn/veld` (packages/npm) | CI bundles `binaries/<plat>` into the tarball (release.yml publish-npm); `install.js` postinstall downloads from GitHub Releases only if the binary is missing | wrapper version 0.1.0 in repo; CI rewrites package.json. install.js asks GitHub API for `releases/latest` (NOT the wrapper's version) unless VELD_VERSION set | none | linux/darwin x64+arm64, windows x64. `arm64` on windows maps to `windows-arm64` which release.yml never builds -> 404 then `go install`. musl: static CGO_ENABLED=0 binary, fine | postinstall failure is swallowed: verified `npm install` exit 0 with bad VELD_VERSION and then `veld` prints "binary not found" exit 1. No proxy support (raw https.get, no HTTP(S)_PROXY). Falls back to `go install ...@latest` (unpinned). Uninstall: nothing outside node_modules. `process.exit(result.status \|\| 0)` returns 0 when child killed by signal | needs work |
| pip `maayn-veld` (packages/pip) | binaries bundled at CI time into `veld/binaries`; no download code | CI sed-rewrites version | n/a (bundled) | same 5 targets; wheel is `py3-none-any` containing all 5 binaries and falls back to PATH | Uses `str \| None` return annotations (evaluated at def time) while `requires-python >=3.8`: TypeError at import on 3.8/3.9 (source-supported, not run: only 3.13 available). Not a platform wheel -> every install carries ~100MB of binaries; exec bit relies on the sdist/wheel preserving mode. README says `pip install veld`, project name is `maayn-veld` (F-CLI-16) | needs work |
| Homebrew (packages/homebrew) | url+sha256 per os/arch in formula | `version "0.1.0"` hardcoded | sha256 present but PLACEHOLDER_* in repo; release.yml update-homebrew job edits the formula on the runner and never commits or pushes it, so nothing is published (F-CLI-15) | darwin/linux x64+arm64 | n/a | placeholder |
| Go (packages/go) | `go install github.com/Adhamzineldin/Veld/cmd/veld@latest` | module version via git tags (47 tags, v0.1.0..v0.3.9) | go.sum / sumdb | any | works if module path public. Binary reports `0.1.0` because Version is only set via ldflags; `go install` build prints wrong version (F-CLI-13) | ready (README only) |
| NuGet `Maayn.Veld` (packages/nuget) | downloads GitHub release archive on first run into LocalAppData/veld/<ver>/<plat> | const Version "0.1.0", CI seds it | none | windows/linux/darwin x64+arm64 (windows-arm64 asset missing) | requires network at first run; no proxy config beyond HttpClient defaults; ExtractToDirectory(overwrite) on possibly partial download not guarded (a partially written archive leaves cache dir without binary but retried next run, OK). Proxies Process exit code, good. net8.0 required | needs work |
| Chocolatey (packages/chocolatey) | Install-ChocolateyZipPackage with sha256 | version rewritten by CI | `PLACEHOLDER_SHA256_WINDOWS_AMD64` replaced in CI only; in-repo copy is a placeholder | windows-amd64 only | nuspec `licenseUrl` points to `blob/master/LICENSE`; the repo has no tracked LICENSE file (F-CLI-17) | needs work |
| apt (packages/apt nfpm) | CI builds .deb from release tarball and pushes to packagecloud | CI | n/a | amd64, arm64 | installs to /usr/local/bin/veld (non-standard for .deb, should be /usr/bin) | needs work |
| Composer `maayn/veld` (packages/composer) | bin/veld downloads on first run to cache dir; `Installer::install` hook | `VELD_VERSION` const 0.1.0, NOT touched by release.yml (no composer job; `publish_composer` output computed but unused) | none | linux/darwin/windows x64+arm64 (windows-arm64 missing) | `file_get_contents` needs allow_url_fopen, no proxy support. Composer does not run a dependency's `post-install-cmd` (only the root package's), so the Installer hook never fires for consumers; download is lazy. Packagist needs composer.json at the repo root - this is in a monorepo subdir, so it cannot be registered as-is | placeholder |

Cross-cutting: no wrapper verifies checksums (release.yml publishes `checksums.txt` but nothing consumes it); release asset naming differs in release-notes text (`brew install veld-dev/tap/veld`) vs formula (`maayn-veld/tap/maayn-veld`); wrappers all say 0.1.0 while tags reach v0.3.9.

### PART D - CI / release / scripts

| workflow | triggers | jobs | verifies | release automation | secrets |
|---|---|---|---|---|---|
| ci.yml | push/PR on master,main | test (ubuntu, Go 1.22: `go build ./...`, `go vet ./...`, `go test ./... -race -count=1`); drift (regen language spec via `go run ./cmd/generate-language` + `scripts/sync-schema.sh`, then `git diff --quiet`); build-check (cross-compile linux amd64/arm64, darwin amd64/arm64, windows amd64) | build, vet, unit tests with race, generated-spec drift, cross-compile. Does NOT: run on windows/macos, run golangci-lint/staticcheck/govulncheck, compile any generated TypeScript/Python/Go/Rust/Java/C#/PHP, build the website, run editor plugin builds, run packages/* | none | none (contents: read) |
| release.yml | tag push `v*`, workflow_dispatch | resolve (path-diff vs previous tag), test (ubuntu), build matrix (5 targets, CGO off, `-trimpath -ldflags "-s -w -X main.Version"`), release (softprops/action-gh-release; archives + `checksums.txt` sha256 + changelog), publish-npm, publish-pip, update-homebrew, publish-vscode, publish-jetbrains, publish-choco (windows), publish-apt (packagecloud), publish-nuget | tests + vet before build | tags, GitHub Release, checksums.txt. No signing (cosign/GPG), no SBOM, no provenance attestation, no goreleaser. Actions pinned by tag, not SHA. No windows-arm64 or linux/386/freebsd targets | GITHUB_TOKEN, NPM_TOKEN, PYPI_TOKEN, VSCE_PAT, JETBRAINS_PUBLISH_TOKEN (+ JETBRAINS_CERT_CHAIN/PRIVATE_KEY/KEY_PASSWORD), CHOCO_API_KEY, PACKAGECLOUD_TOKEN, NUGET_API_KEY |
| scripts/sync-schema.sh | manual / drift job | copies editors/veld-config.schema.json into vscode + jetbrains | - | - | - |

Release defects (see F-CLI-15, F-CLI-18): publish-* jobs `needs: [release]`, and `release` is skipped when no Go source changed, so "packages/npm changed -> npm" auto-detection can never publish (skipped dependency). `curl -sL` without `-f` stores a 404 HTML page as the archive. update-homebrew never pushes. Composer/choco/apt/nuget absent from the "no previous tag" branch. `${{ inputs.version }}` interpolated straight into shell (needs write access to dispatch; low).

Go version: ci.yml, release.yml pin `1.22` = `go 1.22` in go.mod (match). Local toolchain 1.26 builds and passes. Grep of cmd/ and internal/ for `slices`, `maps`, `cmp`, `iter`, `min(`/`max(` builtins, `clear(`, `range <int>`, `errors.Join`, `slog`, `strings.CutPrefix`, `sync.OnceValue`, `t.Chdir`, `os.CopyFS`, `context.WithoutCancel`: none found in Go source (only JS `Math.min`). Used: `http.ServeMux` method patterns + `r.PathValue` (1.22, OK; in internal/server and the Go-backend emitter output) and `atomic.Bool` (1.19). The `go 1.22` directive makes the 1.26 compiler enforce 1.22 language features, and it compiled, so no >1.22 language features. I could not run a real 1.22 toolchain (would download one), so stdlib API use is grep-verified only. Generated Go uses `ServeMux` patterns so generated projects themselves need Go 1.22+.

Does any job compile generated output? No. Verified consequence: examples/go-svelte and examples/python-vue generate uncompilable output (F-CLI-19, F-CLI-20).

### PART E - Website and examples

- website/: Vite 7 + React 19 + react-router 7 + framer-motion + lucide-react, TypeScript 5.9, ESLint 9. Marketing landing page (Hero, Features, HowItWorks, SupportedStacks, Installation, CodeExample, Contributors, CTA) plus a 105KB hand-written DocsPage.tsx plus `LiveDemo.tsx` (1000+ lines): an in-browser "playground" with its OWN TypeScript re-implementation of the parser and generators (`parseVeld` at LiveDemo.tsx:41, generateFrontendSDK :262, generateBackendRoutes :320, ...), not the Go engine or WASM. It will drift from the real generator (F-CLI-22).
- Build (copy under scratchpad/web): `npm ci` FAILS (no package-lock.json; `.gitignore` excludes package-lock.json). `npm install` OK (49 packages, 0 vulnerabilities); `npm run build` (tsc -b + vite build) exit 0, 558 kB JS (167 kB gzip) with the >500 kB chunk warning. `npm run lint` exit 1: 2 errors `no-useless-escape` at LiveDemo.tsx:857. No CI job builds or lints it, no deploy workflow exists in the repo.
- Relationship to registry UI: separate. The registry SPA is `internal/server/handlers/web/index.html` (84KB, embedded by handlers/web.go `//go:embed web`); `internal/server/web/index.html` (43KB) is a stale, unreferenced duplicate (differs, no Go code references it) (F-CLI-21). Neither shares code with website/. Registry page loads qrcode from cdn.jsdelivr.net without SRI.
- docs/index.html: 19KB standalone dark-theme static landing page ("Veld - Contract-First API Code Generator"), inline CSS, apparently the earlier GitHub-Pages site, now duplicated by website/.

Examples (copied to scratchpad/ex; `veld validate` / `veld generate --dry-run` from each `<example>`):

| example | validate | generate --dry-run | note |
|---|---|---|---|
| csharp-kotlin | pass | pass | |
| go-svelte | pass | pass | real generate output fails `go vet/build` (undefined: mux) |
| java-angular | pass | pass | |
| microservices | FAIL exit 1 "no input file specified" (workspace config has no top-level input; `validate` does not support workspaces) | pass | |
| node-flutter | pass | pass | |
| node-react | pass | pass | contains recursively nested junk `veld/packages/@maayn/node_react/packages/@maayn/node_react/...` (committed `veld pull` artefact) and a registry block pointing at app8080.maayn.com |
| node-typescript | pass | pass | generated TS passes `tsc --strict --noEmit` |
| php-typescript | pass | pass | |
| python-vue | pass | pass | real generate output has Python SyntaxError in routes (F-CLI-20) |
| rust-swift | pass | pass | |

examples/ root also holds 8 AI-status docs (COMPLETION-REPORT, COMPLETION-SUMMARY, FINAL-SUMMARY, FILES-INVENTORY, README-NEW, START-HERE, INDEX, QUICK-REFERENCE, SETUP-ALL-EXAMPLES, COMPLETE-GUIDE) - noise.

### PART F - Tests and hygiene

`go test -cover ./...` all green (0 FAIL, ~15s). `go vet ./...`: clean, no output. 523 test functions total.

Coverage per package (statements):
cache 92.1, sdkhelpers 100, lint 84.0, react 83.4, typesonly 82.9, database 79.9, svelte 79.8, vue 79.6, errors 78.4, lang 77.6, dockerfile 76.6, config 74.5, angular 72.4, schema 70.0, setup 68.5, envconfig 66.7, kotlin 66.3, codegen 66.5, openapi 65.6, swift 64.9, dart 63.6, format 63.6, javascript(fe) 62.0, mock 61.2, lexer 60.9, typescript(fe) 59.3, csharp 59.1, java 58.2, cicd 56.4, diff 55.2, parser 53.7, validator 53.7, rust 48.5, php 47.9, go-backend 46.8, scaffold 36.0, emitter 35.4, lsp 33.6, loader 30.2, csharp/strategy 30.4, javascript(be) 29.5, python 28.5, node 19.6, cmd/veld 0.0 (tests exec a built binary, so in-process coverage is 0), ast n/a.
Tests per package (functions): setup 56, parser 52, emitter/lang 52, validator 29, config 29, lexer 22, cmd/veld 21, emitter 14, codegen 13, mock 12, java 12, lint 11, diff 10, node 10, python 10, php 10, csharp 10, go 9, javascript 9, cache 9, lsp 8, rust 8, openapi 7, loader 6, format 6, each frontend emitter 3-6, others <=5.
Packages with ZERO tests: cmd/generate-language, internal/docsgen, internal/graphqlgen, internal/language, internal/registry, internal/server and all of its subpackages (auth, db, email, handlers, models, storage), emitter/tshelpers, emitter/tsshared, and every backend `strategy` subpackage except csharp (go, java, javascript, node, php, python, rust). The registry server (JWT, passwords, TOTP, tarball upload) is entirely untested.
Flaky patterns: none found. No `time.Sleep` in tests, no real network, no `t.Parallel`; 185 `TempDir` uses; config tests use `os.Chdir` (serial, restores via defer, acceptable); cmd/veld tests build the binary once via sync.Once. No test compiles generated code (grep for tsc/py_compile/go vet in tests: only main_test and setup_test use exec).
TODO/FIXME/XXX (informational): 23 in .go files (24 across go/ts/js/py/kt, excluding node_modules).

Tracked-file hygiene (git ls-tree sizes):

| item | tracked? | size |
|---|---|---|
| `veld` (root, ELF x86-64 Linux, debug_info, not stripped) | YES, despite `/veld` in .gitignore (committed before ignore) | 20,450,930 B (~19.5 MB) |
| test-gen/ | yes (6 files) | 2,523 B |
| test-gen2/ | yes (4 files) | 1,551 B |
| test-headers/ | yes (4 files) | 2,631 B |
| test-constants/ | yes (3 files) | 1,225 B |
| test.md | yes | 30,318 B |
| .idea/ | yes (11 files) | 6,319 B |
| docs/*.docx (4) | yes | 71,804 + 61,756 + 59,182 + 53,039 = 245,781 B |
| docs/ total (22 files incl. v1-v4 duplicated plans as .md and .docx) | yes | 652,537 B |
| editors/vscode/out/ (compiled JS + map) | yes (4 files) | ~101 KB |
| editors/jetbrains/gradle/wrapper/gradle-wrapper.jar | yes | 43,764 B (normal) |
| internal/server/web/index.html (stale duplicate) | yes | 42,680 B |
| resources/logo.png | yes | 321,310 B |
| no tracked LICENSE, CHANGELOG, SECURITY, CONTRIBUTING; no .goreleaser, dependabot, Makefile | - | - |
| `.veld.lock.json`, `.veld-cache.json` exist in repo root (ignored/untracked) | no | - |

Repository: 732 tracked files, 24,845,147 B (~23.7 MiB) tracked content (HEAD), `.git` directory 36 MB, 204 commits, 47 tags.

---

### Findings

#### F-CLI-01: `generate` and `watch` delete previous output and the breaking-change lock before validating; breaking-change gate (--strict/--force/prompt) is dead
- Category: output-safety
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: low
- Location: cmd/veld/generate.go:40-52, 759, 1021; cmd/veld/watch.go:56
- Trigger/reproducer: (a) generate OK, append `garbage {{{` to a .veld file, run `veld generate` -> exit 1 and `generated/`, `veld/generated`, `.veld.lock.json`, `.veld-cache.json` all gone. (b) generate OK, remove `action GetUser`, run `veld generate --strict` -> exit 0, no breaking-change report; same with --force and plain. `veld ci --strict` on the identical state exits 1 with "action GetUser removed" (ci does not call runClean).
- Expected vs actual: expected a failed/invalid generation to leave prior output intact and `--strict` to block breaking changes (documented CI gate). Actual: `runClean(rc)` (RemoveAll of every output dir plus `diff.DeleteLock`) runs at generate.go:759 before `computePreChanges` (generate.go:1021), so the lock baseline is already deleted and `HasBreaking` is always false; and any parse/validate/unknown-backend error leaves the user with an empty output dir. watch.go:56 does the same on start and so also loses lock/diff.
- Impact: CI breaking-change protection advertised in CLAUDE.md does not work for `veld generate`; any typo wipes generated code that the developer's own non-generated files import; the interactive prompt (`promptContinue`) is unreachable.
- Root cause: ordering - clean precedes parse/validate/diff; lock deletion is bundled into runClean.
- Recommendation: refactor - parse+validate+diff first, emit into a temp dir, swap on success, and stop deleting the lock in runClean.
- Regression check: test that a failing generate preserves out dir and lock; test `generate --strict` exits 1 when an action is removed.
- Effort estimate: 6h-10h

#### F-CLI-02: `--out`/`out` validation accepts the directory containing the contract sources; `generate` then deletes them
- Category: output-safety
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: low
- Location: internal/config/config.go:360-377, cmd/veld/generate.go:41-48
- Trigger/reproducer: project `proj/veld/{veld.config.json,app.veld,...}`; run `veld generate --out=../veld` (in `proj/`). validateOutPath only rejects basenames `..`, `.`, `/`, `\`. `../veld` resolves to the config dir; runClean `os.RemoveAll` deleted the entire `veld/` tree (sources + config), then failed `reading ...app.veld`.
- Expected vs actual: expected refusal when the output dir contains/equals the config dir, the input file's dir, cwd, or a parent of any of them. Actual: accepted and wiped. Also accepted: `--out=C:\Users\x`, any absolute path.
- Impact: irrecoverable loss of the user's contract files from one mis-typed flag or config value; same path is hit by `veld clean` and by workspace entries (not validated at all).
- Root cause: shallow lexical basename check; RemoveAll with no containment, marker-file, or ownership check.
- Recommendation: fix - refuse any out dir that is, contains, or is contained by config dir / input dir / cwd / home / drive root, and only delete dirs carrying a `.veld-generated` marker.
- Regression check: tests for out=configDir, out=parent-of-configDir, out=inputDir, workspace entry out=".".
- Effort estimate: 3h-5h

#### F-CLI-03: `veld clean` / pre-generate clean recursively delete any directory `out` points at (e.g. `src`)
- Category: clean
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: cmd/veld/generate.go:40-52, 1144-1180
- Trigger/reproducer: config `{"out":"./src"}`, `src/business.ts` present, `veld clean` -> prints "Cleaned ...\src", exit 0, `src/` is gone including the user file. No prompt, no manifest, no `--yes`.
- Expected vs actual: only files Veld generated should be removed (CLAUDE.md: "Veld never touches developer business logic"). Actual: whole directory.
- Impact: data loss for projects that mix generated and handwritten code in one folder (Veld's own docs allow `out` anywhere).
- Root cause: no generated-file manifest or marker; `os.RemoveAll(dir)`.
- Recommendation: refactor - write a manifest (or `.veld-generated` marker) and delete only manifest files (plus empty dirs); refuse to clean unmarked non-empty dirs without `--force`.
- Regression check: clean on a dir with foreign files leaves them; clean on an unmarked dir refuses.
- Effort estimate: 4h-6h

#### F-CLI-04: `veld doctor` is advertised but not registered
- Category: cli
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/tooling.go:195, cmd/veld/main.go:88-101, 77
- Trigger/reproducer: `veld doctor` -> `Error: unknown command "doctor" for "veld"`, exit 1. `newDoctorCmd` is defined (and marked Hidden) but never added to the root command; help text, init-generated README (init.go:1080) and CLAUDE.md all advertise it. main_test.go:456 tolerates any exit code so the test does not catch it.
- Expected vs actual: command runs vs unknown command.
- Impact: documented diagnostic unusable.
- Root cause: missing `newDoctorCmd()` in AddCommand list.
- Recommendation: fix - register it (and un-hide) or remove the docs.
- Regression check: assert `veld doctor` exit 0 in a valid project.
- Effort estimate: 0.5h-1h

#### F-CLI-05: `veld init` overwrites existing README.md and veld/*.veld files when no config exists
- Category: cli
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: cmd/veld/init.go:20-33, 651-658
- Trigger/reproducer: directory with `README.md` ("MY README") and `veld/app.veld` ("// mine") but no veld.config.json; `veld init` exit 0 -> README.md now "# My Veld Project", app.veld replaced by scaffold.
- Expected vs actual: CLAUDE.md "veld init never overwrites files"; the guard only checks for config files, then `os.WriteFile` unconditionally.
- Impact: silent loss of a project README and any pre-existing contract files.
- Root cause: existence check limited to two config paths; no per-file check.
- Recommendation: fix - skip or abort on any existing target path (O_EXCL).
- Regression check: init in dir with README.md leaves it byte-identical.
- Effort estimate: 1h-2h

#### F-CLI-06: `generate --dry-run` writes `.veld-cache.json`
- Category: dry-run
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/generate.go:190-200
- Trigger/reproducer: before/after `find` of a project around `veld generate --dry-run`: new `veld/.veld-cache.json`.
- Expected vs actual: nothing written vs cache file created (lock is correctly skipped at :205/:209; cache save is not guarded by DryRun).
- Impact: breaks "no side effects" promise; a stale cache could make a later `--incremental` think files were generated.
- Root cause: missing `!opts.DryRun` guard around cache update/save.
- Recommendation: fix - guard cache save.
- Regression check: directory snapshot equality around dry-run.
- Effort estimate: 0.5h-1h

#### F-CLI-07: Config precedence: flat fields override nested, contrary to documentation
- Category: config
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/config/config.go:137-150 (normalize), CLAUDE.md config section, config.go:34 comment
- Trigger/reproducer: `{"backend":"python","backendConfig":{"target":"node-ts"},"frontend":"none"}` -> Python generated.
- Expected vs actual: documented "nested takes precedence"; actual flat wins (nested only fills empty flat fields). Same pattern for framework, out, validate, postGenerate, frontend.
- Impact: users migrating to the nested format with a leftover flat key get the old target silently.
- Root cause: `&& c.Backend == ""` guards in normalize().
- Recommendation: fix - make nested win (or error on conflicting keys) and add a test.
- Regression check: unit test with both formats set.
- Effort estimate: 1h-2h

#### F-CLI-08: `--out` does not override `backendConfig.out` / `backendOut`
- Category: config
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/config/config.go:312-335, 392-405
- Trigger/reproducer: config with nested `backendConfig.out: ./nestout`, run `generate --dry-run --out=./flagout` -> backend -> ./nestout, frontend -> ./flagout.
- Expected vs actual: flag beats config everywhere; actual: only unset side follows `--out`. `FlagOverrides.BackendOut/FrontendOut` exist but generate has no `--backend-out` flag.
- Impact: CI overrides of the output dir silently partially ignored.
- Root cause: flag sets only `cfg.Out`; BackendOut non-empty takes priority.
- Recommendation: fix - `--out` should clear BackendOut/FrontendOut, or add explicit flags.
- Regression check: test flag vs nested out.
- Effort estimate: 1h-2h

#### F-CLI-09: Path-base inconsistency and silent unknown config keys
- Category: config
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/config/config.go:283-308, 324-327; RawConfig decoding at :296
- Trigger/reproducer: (1) with config in `veld/`, `veld generate --input=app.veld` from `veld/` changes the base for `out` from the config dir to cwd (`./veld/o` vs `./o`). (2) config containing `"bogusKey":1` or misspelt `"outt"` accepted silently. (3) Config only found in `.` or `./veld`, never in parents. (4) JSON with comments/trailing comma -> raw encoding/json error with no line number.
- Expected vs actual: single resolution base; unknown keys warned; discovery up the tree.
- Impact: surprising output locations, typos undetected.
- Root cause: `cfgDir` reassigned when `--input` set; plain Unmarshal.
- Recommendation: refactor - use DisallowUnknownFields (warn), keep config-dir base, optional walk-up.
- Regression check: tests for typo key warning and --input base.
- Effort estimate: 3h-5h

#### F-CLI-10: `hooks.postGenerate` is run via hard-coded `sh -c`; failures are ignored
- Category: config
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: low
- Location: cmd/veld/generate.go:24-36
- Trigger/reproducer: hook `echo HOOK-RAN > hook.txt; exit 3` -> hook ran, `warning: postGenerate hook failed: exit status 3`, veld exit 0. On Windows without `sh` the hook cannot run at all.
- Expected vs actual: documented `postGenerate: "npm run format"` works cross-platform and a failing formatter fails the CI step; actual: needs `sh`, always exit 0.
- Impact: unformatted/unverified code ships from CI; Windows users without Git Bash get a warning only. Design consideration: config is executable code (anyone able to modify veld.config.json gets shell execution on `generate`/`ci`/`watch`, like npm scripts); should be documented, and ideally `--no-hooks`.
- Root cause: `exec.Command("sh","-c")`, error swallowed.
- Recommendation: refactor - use `cmd /C` on Windows, return a non-zero exit on hook failure (or `--strict`), add `--no-hooks`.
- Regression check: hook exit code propagated test.
- Effort estimate: 2h-3h

#### F-CLI-11: Hidden commands and lack of machine-readable output / exit-code taxonomy
- Category: exit-codes
- Evidence status: design limitation
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/main.go:105, contract.go:64,601
- Trigger/reproducer: no `--json` on validate/lint/diff/generate; all failures exit 1; `fmt` exits 0 when files need formatting; `validate` mixes `os.Exit(1)` and returned errors, so output format of the two failure modes differs (`error:` vs `Error:`).
- Expected vs actual: CI-friendly JSON diagnostics and distinct codes (usage 2, validation 1, IO 3).
- Impact: tooling must scrape ANSI-colored text (colors are emitted even when stdout is not a TTY: ANSI escapes seen in redirected files).
- Root cause: none designed.
- Recommendation: refactor - add `--json`, honor NO_COLOR / non-TTY, add `fmt --check`.
- Regression check: golden JSON tests.
- Effort estimate: 6h-10h

#### F-CLI-12: Error-level lint rules are unreachable
- Category: cli
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/contract.go:96-103, internal/lint/lint.go
- Trigger/reproducer: duplicate-route / duplicate-action contracts are rejected by validator first ("contract validation failed - fix errors before linting"); `lint` never reaches its own error-level rules. CLAUDE.md lists duplicate-route/duplicate-action as lint errors.
- Expected vs actual: lint reports them; actual: validator error text and exit 1 before lint.
- Impact: duplicated logic, misleading docs.
- Root cause: layering overlap.
- Recommendation: remove - drop the unreachable lint rules or document validator ownership.
- Regression check: n/a.
- Effort estimate: 0.5h-1h

#### F-CLI-13: Version reporting wrong for `go install` and source builds; wrappers pinned to 0.1.0
- Category: release
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/main.go:37, packages/*/ (version 0.1.0), release.yml:304
- Trigger/reproducer: `veld --version` from plain `go build` -> `veld version 0.1.0`, while tags reach v0.3.9. Homebrew `test do` asserts the version string.
- Expected vs actual: version from tag/build info; actual: constant 0.1.0 unless ldflags used.
- Impact: bug reports and wrapper checks unreliable.
- Root cause: ldflags-only version.
- Recommendation: fix - fall back to `runtime/debug.ReadBuildInfo().Main.Version`.
- Regression check: version test.
- Effort estimate: 1h-2h

#### F-CLI-14: Documentation lists top-level `veld login|logout|push|pull|serve`; they exist only under `veld registry`
- Category: cli
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/registry.go:373, CLAUDE.md "Build & Test Commands", README
- Trigger/reproducer: `veld login` / `veld serve` / `veld push` -> unknown command, exit 1.
- Expected vs actual: documented top-level commands vs only `veld registry <cmd>`.
- Impact: broken copy-paste instructions.
- Root cause: command regrouping without doc update.
- Recommendation: fix - add top-level aliases or update docs.
- Regression check: doc-command smoke test.
- Effort estimate: 1h-2h

#### F-CLI-15: Release workflow: Homebrew job never publishes; package jobs skipped when only wrappers change
- Category: release
- Evidence status: source-supported defect
- Severity (user impact): high
- Security severity: none
- Location: .github/workflows/release.yml:133-170 (resolve), 392-422 (update-homebrew), publish-* `needs: [release, resolve]`
- Trigger/reproducer: read workflow. (1) update-homebrew seds `packages/homebrew/Formula/maayn-veld.rb` on the runner and exits; no commit/push/PR to a tap, so brew users never get the formula and the repo copy keeps `PLACEHOLDER_SHA256_*`. (2) `release` runs only if `publish_binary == 'true'`; publish-npm/pip/etc. need `release`, so a tag where only `packages/npm/` changed skips the release job and therefore npm (skipped dependency). (3) `curl -sL` (no `-f`) turns a 404 into an HTML "archive" and fails later at tar. (4) `publish_composer` is computed but there is no composer job. (5) Release notes advertise `brew install veld-dev/tap/veld` while formula is `maayn-veld/tap/maayn-veld`.
- Expected vs actual: documented "auto-detect and publish affected targets"; actual partial.
- Impact: brew and composer channels never ship; wrapper-only fixes cannot be released by tag.
- Root cause: job dependency design and unfinished jobs.
- Recommendation: refactor - decouple wrapper jobs from the `release` job (`if: always() && ...`), add `curl -fsSL`, push the formula to a tap repo, add composer or drop it.
- Regression check: workflow dry-run (act) or a tag on a fork.
- Effort estimate: 4h-8h

#### F-CLI-16: npm and pip wrappers: no checksum, unpinned download, swallowed postinstall failure, Python 3.8/3.9 syntax, doc name mismatch
- Category: dist-npm
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: packages/npm/install.js:25-45, 141-205; packages/pip/veld/__main__.py:23,61 (`str | None`); packages/pip/pyproject.toml (requires-python >=3.8); packages/README.md
- Trigger/reproducer: npm: `VELD_VERSION=0.0.0 npm install ../pkg` -> install exit 0, `npx veld` exit 1 "binary not found". Source: install.js resolves `releases/latest` rather than the wrapper's own version, downloads over https with no checksum or signature, then extracts with tar/Expand-Archive; `go install ...@latest` fallback is also unpinned. pip: `def get_platform_key() -> str | None` is evaluated at import on Python <3.10. Docs say `npm install veld` / `pip install veld` but names are `@maayn/veld` / `maayn-veld`. No HTTP_PROXY support in install.js. `process.exit(result.status || 0)` masks signal exits.
- Expected vs actual: pinned, checksum-verified, failing install; actual otherwise.
- Impact: supply-chain exposure (a compromised release or MITM of redirect chain is run as developer code without verification); broken installs reported as success; pip wrapper unusable on 3.8/3.9 despite metadata.
- Root cause: wrapper written for convenience.
- Recommendation: fix - pin to wrapper version, verify `checksums.txt` sha256, fail the install loudly, add `from __future__ import annotations`, correct README names.
- Regression check: install tests with a local HTTP server serving a tampered archive.
- Effort estimate: 4h-6h

#### F-CLI-17: Distribution metadata gaps (no root LICENSE, placeholders, Windows-arm64, Composer unpublishable as-is)
- Category: dist-composer
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: packages/chocolatey/veld.nuspec (licenseUrl), packages/chocolatey/tools/chocolateyInstall.ps1 (checksum placeholder), packages/composer/*, packages/apt/nfpm.yaml
- Trigger/reproducer: `git ls-files` shows no LICENSE; nuspec licenseUrl points to it. Windows ARM64 users map to `windows-arm64`, never built (npm, composer, nuget). Composer `post-install-cmd` of a dependency does not execute; Packagist cannot index a subdirectory composer.json; `VELD_VERSION` const is not updated by CI. nfpm installs to /usr/local/bin.
- Expected vs actual: publishable channels; actual: placeholders.
- Impact: Chocolatey moderation failure, Windows ARM unsupported, Composer channel nonfunctional.
- Root cause: unfinished packaging.
- Recommendation: fix - add LICENSE, add windows-arm64 target or explicit unsupported message, split Composer into its own repo.
- Regression check: per-channel install smoke test in CI.
- Effort estimate: 6h-10h

#### F-CLI-18: CI verifies only Linux unit tests; no signing/SBOM; no generated-code compilation; nothing builds website/editors/wrappers
- Category: ci
- Evidence status: design limitation
- Severity (user impact): high
- Security severity: low
- Location: .github/workflows/ci.yml, release.yml
- Trigger/reproducer: read workflows.
- Expected vs actual: a code generator whose product is source code should compile its output for each target; actual: only `go build/vet/test` on ubuntu plus cross-compile; no windows/macos test runner (the CLI has Windows-specific path code), no govulncheck/lint, no cosign/SBOM/attestation, actions not SHA-pinned.
- Impact: regressions F-CLI-19/20 shipped unnoticed.
- Root cause: absent jobs.
- Recommendation: refactor - add matrix (ubuntu/windows/macos), a job that generates every example and compiles with tsc, py_compile, go build, cargo check, javac, dotnet build, php -l; add govulncheck, SBOM, cosign.
- Regression check: the new job fails on current examples (proves it).
- Effort estimate: 12h-20h

#### F-CLI-19: Go backend (plain `net/http` strategy) generates code that does not compile
- Category: examples
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/emitter/backend/go/strategy/plain.go (route registration) -> generated `internal/routes/todos.go:13-18`
- Trigger/reproducer: `examples/go-svelte` copy: `veld generate` then `cd generated && go vet ./...` -> `internal\routes\todos.go:14:2: undefined: mux` (9 uses). The function parameter is named `r *http.ServeMux` but the body calls `mux.HandleFunc`.
- Expected vs actual: compiles; actual: build failure for the default Go backend.
- Impact: Go backend unusable out of the box.
- Root cause: param name vs variable name mismatch in the plain strategy template.
- Recommendation: fix - emit consistent receiver name; add compile test.
- Regression check: go build of generated output in CI (F-CLI-18).
- Effort estimate: 1h-2h

#### F-CLI-20: Python backend routes contain a SyntaxError for actions with input
- Category: examples
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: generated `routes/todos_routes.py:40`, emitter internal/emitter/backend/python
- Trigger/reproducer: `examples/python-vue` copy: `veld generate` (default config, no validate flag) then `python -I -m compileall generated` -> `input = CreateTodoInputSchema(**body = data  # CreateTodoInputSchema).model_dump()` SyntaxError (also users_routes.py:40).
- Expected vs actual: valid Python; actual: syntax error, module cannot import.
- Impact: Python backend route files unusable for any action with a body.
- Root cause: string composition bug in the Python route emitter (looks like a template arg mix-up; not root-caused here).
- Recommendation: fix - repair the template and add py_compile test.
- Regression check: compileall on generated output.
- Effort estimate: 1h-3h

#### F-CLI-21: Repository hygiene: 20MB binary and scratch dirs tracked; stale duplicate registry UI
- Category: hygiene
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: `veld` (root), test-gen/, test-gen2/, test-headers/, test-constants/, test.md, .idea/, docs/*.docx, internal/server/web/index.html, examples/node-react/veld/packages/@maayn/..., examples/*SUMMARY*.md
- Trigger/reproducer: `git ls-tree -r -l HEAD` (sizes in Part F). Root `veld` is a Linux ELF with debug info, tracked although `/veld` is in .gitignore. `internal/server/web/index.html` is not embedded (handlers/web.go embeds `web` under handlers/) and differs from the live copy.
- Expected vs actual: clean repo; actual: ~20.9MB of the 23.7MB tracked content is the stray binary; `.git` is 36MB (history keeps it forever).
- Impact: slow clones, confusion about the registry UI source of truth, misleading AI-generated status docs.
- Root cause: committed build artifacts.
- Recommendation: remove - `git rm` them (history rewrite optional).
- Regression check: CI check that no executable / `test-*` paths are tracked.
- Effort estimate: 1h-2h

#### F-CLI-22: Website: no lockfile (npm ci fails), lint errors, unbuilt in CI, LiveDemo duplicates the generator
- Category: website
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: website/package.json, .gitignore (package-lock.json), website/src/components/sections/LiveDemo.tsx:41,857
- Trigger/reproducer: `npm ci` -> EUSAGE (no lockfile); `npm install && npm run build` exit 0; `npm run lint` exit 1 (2x no-useless-escape at LiveDemo.tsx:857). LiveDemo contains a separate TS parser/generators.
- Expected vs actual: reproducible installs, clean lint, demo output matching the real tool.
- Impact: unreproducible dependency tree, demo drifts from real output (already shows different formatting than the Go emitters).
- Root cause: lockfile ignored; reimplementation instead of WASM.
- Recommendation: refactor - commit lockfile, fix lint, add website CI job; consider compiling the Go engine to WASM for the demo.
- Regression check: CI job `npm ci && npm run lint && npm run build`.
- Effort estimate: 2h-4h (WASM: 16h+)

#### F-CLI-23: Test hygiene gaps (untested registry server, strategy subpackages, no generated-code compile tests, CLI coverage reads 0%)
- Category: tests
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: internal/server/**, internal/registry, internal/docsgen, internal/graphqlgen, internal/language, emitter/backend/*/strategy, cmd/veld
- Trigger/reproducer: `go test -cover ./...` shows no test files for the listed packages; cmd/veld reports 0.0% because tests exec a built binary. Low coverage: node backend 19.6%, python 28.5%, loader 30.2%, lsp 33.6%, emitter 35.4%, validator and parser 53.7%.
- Expected vs actual: auth/JWT/TOTP/upload code covered; actual: zero tests.
- Impact: security-critical registry code (HMAC JWT, password hashing, tarball extraction/upload, TOTP) has no regression net.
- Root cause: prioritisation.
- Recommendation: fix - add handler/auth tests with httptest and a fake storage, plus generated-code compile tests.
- Regression check: coverage gates per package.
- Effort estimate: 16h-30h



# Part VI: Repository rules for contributors and AI agents (AGENTS.md)



## AGENTS.md: rules for every contributor and AI agent working in this repository

These rules exist so that work by any team member or AI agent moves Veld toward the agreed end state and never away from it. The end state is defined in `docs/review-2026-10/19-expected-state-at-submission.md` and `20-extended-scope-plan.md`; the decisions behind it are the ADRs in `docs/review-2026-10/adr/`. Read the review index (`docs/review-2026-10/00-INDEX.md`) before changing architecture. `CLAUDE.md` is older than this file and contains statements the review found to be false (listed at the end); where they conflict, this file wins.

### 1. Non-negotiable guarantees (never regress these)

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

### 2. Definition of done for any change

A change is done only when all of the following hold, and the pull request description says how each was verified:

- Tests: unit tests for new logic; a conformance fixture for any language or semantic change (added **before** the implementation); golden output plus a compile test for any emitter change; a round-trip test for tier-1 behaviour changes; `httptest`-based tests for any registry handler; a scripted-session test for any LSP change.
- Quality gates green locally and in CI: `go build ./...`, `go vet ./...`, `go test -race ./...`, `golangci-lint` (config in repo), `govulncheck`, the import-boundary test, the determinism job, the language-metadata drift job.
- Coverage: the package you touched does not drop below its current statement coverage, and new packages start at 70 percent or higher. Coverage is a floor, not a target; a compile or round-trip test is worth more than coverage.
- Documentation: user-facing behaviour documented in `docs/`; every documented claim traceable to a test or CI job; `CHANGELOG.md` entry under Unreleased; an ADR if a boundary, format or guarantee changed.
- Backlog: the PR title starts with the backlog ID (`B-023:`) and the commit messages reference the finding IDs they close (`F-NODE-05`).
- Size: one logical change per commit, ordered so every commit builds and passes tests; prefer pull requests under about 400 changed lines; split otherwise.
- Review: at least one reviewer who is not the author, from the owner matrix in `docs/review-2026-10/16-team-allocation.md` (`CODEOWNERS` encodes it); security-relevant changes (registry, archives, plugin host, auth) need T1 or T3 as reviewer.

### 3. Code rules (lean code, SOLID applied to Go)

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

### 4. Testing rules

- Table-driven tests; descriptive case names; one assertion topic per case.
- Assertions on generated code are structural or behavioural (compiles, parses, routes count equals operations, validator rejects X), never substring-only.
- No network, no real registries, no time-based sleeps in unit tests. Registry tests use `httptest` and a fake storage; the PostgreSQL job is the only place a database runs.
- Fuzz targets (`FuzzLex`, `FuzzParse`, `FuzzValidate`, `FuzzDiff`, `FuzzUnpack`) stay in the tree and run nightly; a fuzz crash is a P0.
- A failing test is fixed by fixing the code or correcting a wrong expectation with a justification in the commit message. Never delete, skip or weaken a test to make CI pass. `t.Skip` is allowed only for a missing toolchain, with the toolchain named.
- Security tests are regression tests: every finding with security severity medium or higher in `docs/review-2026-10/csv/defect-register.csv` has a test that fails if it reappears.

### 5. Rules specific to AI agents

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

### 6. Security rules

- Validate every external input at the boundary: contract files, config, archives, plugin responses, HTTP bodies. Enforce size limits before reading.
- Path containment is checked on the cleaned absolute path with a trailing-separator prefix test, never with a string prefix on the entry name.
- Archives: allow-list entry types (regular files, directories), reject symlinks and devices, cap entries, per-entry size, total size and compression ratio, extract to a temporary directory and rename.
- Registry: every handler calls the authorization helper; private is the default visibility; uniform 404 for unauthorized reads; tokens hashed; sessions revocable; rate limits on authentication endpoints; `Content-Security-Policy` and escaping in the UI.
- Plugins: never execute a plugin that is not pinned in `veld.config.json`; native plugins are documented as unsandboxed; the Wasm transport has no filesystem or network capabilities.
- Report suspected vulnerabilities through `SECURITY.md`, not in a public issue.

### 7. Process

- Branches `feat/B-023-short-name`, `fix/F-NODE-05-short-name`; squash is not used, history stays as small ordered commits.
- ADRs: `docs/adr/ADR-NNN-title.md`, status proposed to accepted; a proposed ADR is open for one sprint of comments.
- Integration owner rotates each sprint (T1, T2, T3, T4, T5) and runs the vertical slice from a clean clone at the sprint review.
- Triage: labels `P0` to `P3` as defined in `docs/review-2026-10/05-defect-register-and-documentation-discrepancies.md`; P0 blocks the sprint.
- Docs are validated by the component owner named in chapter 16 before they are published.

### 8. Known-stale statements in `CLAUDE.md` (do not act on them until the file is rewritten)

- "Zod schemas (Node) and Pydantic schemas (Python) are generated": neither exists; validators are generated code.
- "Output is deterministic": false at 6a47f81 (F-LEAD-01) until B-012 lands.
- "Veld never writes outside the --out directory" and "never touches developer business logic files": false until ADR-007 lands.
- Top-level `veld login/logout/push/pull/serve` and `veld doctor`: only `veld registry ...` exists and `doctor` is unregistered until B-046.
- The `tools` config block is parsed but ignored until Q8.
- Rust is Axum (not Actix); PHP is Laravel (not PSR-15); Python output layout is `models/ interfaces/ routes/ middleware/`.
- "No separate registry binary exists": superseded by ADR-004.
- "only AST JSON passes between stages": superseded by ADR-002 (IR is the contract).
