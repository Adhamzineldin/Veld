You are acting as a principal software architect, compiler engineer, developer-tooling maintainer, security reviewer, and graduation-project technical adviser.

Study **Veld** as both a product idea and an existing codebase. Produce a rigorous architecture review and an executable delivery plan for an open-source graduation project that must be submitted in **June 2027**. We need decisions, evidence, priorities, and a realistic division of work—not a generic list of best practices.

## 1. Our situation and your assignment

- Our submission deadline is **June 2027**, not a full year from now. The planning baseline is **October 21, 2026**, giving approximately **32–36 calendar weeks**, depending on the exact June submission date. The exact day is not yet specified: use **May 31, 2027 as the conservative submission-readiness target**, with June reserved for submission, defense, and final corrections. This readiness target is a planning assumption, not a confirmed university deadline. If this review starts later, recalculate the remaining time rather than sliding the June deadline.
- Our team has **five technical members, T1–T5**, and **one documentation and diagrams member, D1**.
- D1 owns documentation, diagrams, research organization, report preparation, and presentation material. Do not assign D1 production coding, infrastructure implementation, or sole responsibility for verifying technical correctness. Technical owners must supply and review the evidence D1 documents.
- We intend to release and maintain Veld as an open-source project.
- We are willing to refactor, replace individual subsystems, or undertake a full rewrite **if the evidence justifies it and it fits the June 2027 deadline**. Assess the time remaining for migration, integration, evaluation, and report preparation before recommending a rewrite.
- We want the best architecture for our constraints: understandable, correct, extensible, maintainable, testable, and feasible for a student team. Do not pursue an abstract “perfect architecture” at the expense of delivery.
- Evaluate the product concept, code defects, language choices, component boundaries, ecosystem, delivery plan, and academic contribution.
- This assignment is an investigation and planning exercise. Do not begin rewriting application code or publish anything. You may create the requested report artifacts if your environment supports it.

Treat these instructions as the current planning constraints. Previous repository plans are historical context. Do not silently inherit their timelines, personal task assignments, emitter counts, or scope restrictions. The June 2027 submission deadline supersedes the earlier request for a full-year plan.

## 2. Repository access and starting context

Repository: **Veld**. Local workspace, if available: `D:\University\Veld`.

Start by confirming access to the actual source. If you cannot access it, request repository access or an archive. You may provide a clearly marked provisional architecture outline, but do not claim to have audited code, reproduce defects, run commands, or know current implementation details without evidence.

Initial orientation from a preliminary inspection; verify everything against the revision you receive:

- Veld describes itself as a contract-first, multi-stack API code generator. Users write `.veld` contracts describing models, endpoints, and related API concerns.
- The core is currently Go. The inspected `go.mod` declares `github.com/Adhamzineldin/Veld` and Go 1.22; that is a repository declaration, not a recommendation for the future support policy.
- Source areas include `internal/lexer`, `internal/parser`, `internal/ast`, `internal/loader`, `internal/validator`, `internal/language`, and `internal/config`.
- Emitters and helpers live under `internal/emitter`, including backend, frontend, language-mapping, and code-generation packages. `internal/generators` contains additional generators.
- `internal/emitter/emitter.go` exposes Go interfaces and uses in-process registration. Determine whether any true external-emitter protocol exists; built-in registration alone is not an external plugin system.
- Other areas include `internal/lsp`, `internal/format`, `internal/lint`, `internal/diff`, `internal/cache`, `internal/mock`, `internal/schema`, `internal/docsgen`, and `internal/graphqlgen`.
- The CLI is under `cmd/veld`. At inspection, commands were distributed across files including `main.go`, `generate.go`, `contract.go`, `watch.go`, `registry.go`, and `serve.go`. Do not repeat old claims about a monolithic CLI without rechecking.
- Registry client and archive handling are under `internal/registry`. The registry server is under `internal/server`, with database, storage, authentication, handlers, email, and web assets. The inspected server uses PostgreSQL and local artifact storage.
- Editor integrations exist under `editors/vscode` and `editors/jetbrains`. `cmd/generate-language` generates shared language metadata for editor tooling.
- Distribution wrappers exist under `packages/npm`, `packages/pip`, `packages/homebrew`, `packages/go`, `packages/nuget`, and `packages/composer`.
- `website` contains a React/TypeScript site. Determine its relationship to the registry UI rather than assuming they are one application.
- Examples and workflows live under `examples` and `.github/workflows`.
- Read `README.md`, applicable repository guidance, `CLAUDE.md`, `docs/architecture/overview.md`, `docs/GRADUATION_PROPOSAL.md`, `docs/PROJECT_PROPOSAL.md`, `docs/roadmap.md`, and the `docs/EXECUTION_PLAN*.md` documents. Check for additional guidance and relevant source files.

Existing documents contain historical claims. Record discrepancies between documentation and implementation. Repository claims such as “framework agnostic,” “zero runtime dependencies,” “supported,” and “complete” must be checked against actual generated output and tests.

## 3. Investigation method and evidence standard

Record the reviewed commit, date, working-tree state, environment, available toolchains, and scope inspected. Distinguish tracked source, generated files, vendored dependencies, and build artifacts when calculating metrics.

Trace representative workflows end to end:

1. Clean installation → initialization → contract validation → generation → compile/type-check generated code → run a small API and call it with a generated client.
2. Invalid and adversarial contracts → useful diagnostics without crashes or partial destructive output.
3. Multiple files and services → import resolution → generated service-to-service SDKs.
4. Editor change → language server → accurate diagnostics/navigation.
5. Registry authentication → publication → resolution/download → checksum verification → safe extraction → consumption.
6. Self-hosted registry setup → restart → backup → restore → upgrade.

Run relevant existing checks where tools are available, including core build, tests, static analysis, generated-language drift checks, and selected generated-project compilation. Use disposable output directories and isolated test services. Do not contact a production registry or publish real artifacts as part of verification.

Report exact commands and meaningful results. A missing dependency or toolchain is an environment limitation, not automatically a source-code defect. A passing unit suite is not proof that emitted applications compile or behave correctly. Distinguish source compilation failures from dependency-download or network failures.

Label findings as **reproduced defect**, **source-supported defect**, **suspected risk**, **design limitation**, **documentation mismatch**, or **unverified**. Cite repository paths and line numbers from the reviewed revision. If counts or coverage are reported, explain how they were measured. Do not equate TODO markers or large files with confirmed bugs.

For changing external facts—competitor capabilities, language support, dependency versions, ecosystem tooling, and protocol specifications—use current primary sources where browsing is available. Give URLs and access dates. If browsing is unavailable, mark these conclusions provisional. Do not invent benchmarks, citations, adoption statistics, or evaluation results.

## 4. Product idea, differentiation, and graduation value

Explain the problem Veld solves, intended users, representative workflows, and why a developer would choose it.

Compare relevant alternatives such as OpenAPI with generators, TypeSpec, Protocol Buffers/gRPC, Smithy, and framework-specific type-sharing tools. Compare only relevant capabilities and identify where Veld should integrate instead of duplicating an established standard.

Challenge whether a custom DSL is justified. Compare keeping `.veld`, building on an existing specification, and offering interoperability. Assess language learning cost, editor support, migration effort, expressiveness, and long-term maintenance.

Define:

- A clear product thesis and credible differentiation.
- Primary users and three concrete user journeys.
- A minimum defensible graduation deliverable, a target open-source release, and stretch goals.
- Explicit non-goals and a list of features to defer.
- Research or engineering questions suitable for the graduation report.
- An evaluation plan: baselines, representative tasks, correctness measures, reproducibility, and a feasible user study if useful. Separate planned measurements from actual results.

## 5. Current architecture and defect report

Produce an inventory of implemented, partial, documented-only, and absent capabilities. Include dependencies, coupling, public interfaces, test coverage evidence, and operational maturity.

Audit at least:

- Lexer/parser correctness, grammar ambiguity, error recovery, malformed input, source locations, Unicode, and resource limits.
- Type representation and validation, symbols, imports, cycles, duplicate names, inheritance, nested containers, optional versus nullable fields, defaults, and constraints.
- Cross-language behavior: integer precision, decimals, dates/time zones, enums, unions, unknown values, serialization, names, reserved words, escaping, and identifier collisions.
- HTTP semantics: path/query/header/body binding, URL encoding, errors and status codes, authentication hooks, pagination, file transfer, and streaming where claimed.
- Generated-code correctness, missing imports, runtime dependencies, framework assumptions, incomplete adapters, and divergence between emitters.
- Determinism, cache invalidation, watch mode, stale outputs, safe cleanup, atomic writes, output ownership, and preservation of user-written business logic.
- CLI consistency, configuration precedence, failure exit codes, and diagnostic quality.
- LSP correctness with unsaved buffers, multi-file projects, cancellation, positions, and synchronization.
- Registry authorization, tenancy, credential handling, archive handling, integrity verification, publication consistency, database/storage lifecycle, and recovery.
- Package installers, release scripts, reproducibility, cross-platform support, documentation drift, and fragile tests.

For registry archives, inspect `internal/registry/tarball.go` and all callers for checked I/O errors, interrupted transfers, path containment, symlinks, archive entry types, size limits, and partial extraction behavior. Treat these as investigation targets; establish what is actually exploitable or incorrect before assigning severity.

Create a defect table with:

`ID | category | evidence status | severity | file:line | trigger/reproducer | expected/actual behavior | impact | root cause | fix/refactor/rewrite recommendation | regression check | estimated effort | proposed owner | dependencies`

Separate security severity, user impact, and delivery priority. Define P0–P3 consistently. Identify the highest-value fixes first, reusable parts worth preserving, and technical debt that can safely remain. Do not invent an arbitrary number of defects to fill a table.

## 6. Refactor versus rewrite and language recommendations

Compare three explicit strategies:

1. Stabilize and incrementally refactor the current implementation.
2. Selectively replace weak subsystems while retaining working components.
3. Rewrite the core or whole platform, with a specified migration path.

Evaluate time to reliable delivery, preservation of behavior, technical risk, team learning, maintainability, tooling, packaging, performance requirements, plugin support, contributor accessibility, and ongoing maintenance.

Assess Go, Rust, and TypeScript for the compiler/CLI, and include other languages only when there is a concrete reason. Make language decisions independently for the compiler, language server, registry, browser UI, editor plugins, and emitter SDK. Do not conflate the compiler implementation language with the languages it generates.

Use a weighted decision matrix with explicit weights and justified scores, then show whether the recommendation changes under different team experience assumptions. Avoid treating speculative scores as measured facts. Recommend one primary strategy, explain rejected alternatives, and state what evidence would reverse the decision.

For each component, provide:

`component | current implementation | retain/refactor/replace | proposed language and stack | reason | migration cost | compatibility impact | decision confidence`

If a rewrite is recommended, identify the precise failures it addresses, preservation strategy for fixtures/specifications/tests, compatibility layer, side-by-side comparison, milestone for proving viability, rollback/abort criteria, and a cutoff date. If a rewrite is not justified, say so clearly.

## 7. Target architecture and repository boundaries

Design explicit boundaries for:

1. Language specification and conformance fixtures.
2. Compiler frontend, semantic analysis, and normalized intermediate representation (IR).
3. Compiler application/service API and CLI.
4. Built-in emitters and framework adapters.
5. External emitter protocol, host, SDK, and development tooling.
6. Language server and editor clients.
7. Contract/package resolution, caching, manifests, and lockfiles.
8. Registry service, storage, database, and registry UI.
9. Distribution wrappers, release tooling, docs site, and examples.

Show dependency direction, runtime interactions, ownership, and trust boundaries. Explain whether syntax trees and semantic IR should be separate and whether serialization is needed at every stage or only at process/public boundaries.

Propose a concrete directory tree and interfaces sufficient to review the design. Decide modular monolith versus separate deployables where relevant, and monorepo versus multiple repositories. Distinguish logical modules, packages, repositories, processes, and independently versioned products. More separation is not automatically better.

Explain how a contributor adds an emitter or editor integration without changing compiler semantics. Give a coherent versioning scheme for the language, compiler, IR, plugin protocol, registry API, package manifests, and generated artifacts. Include compatibility negotiation and deprecation policy.

## 8. Compiler, language, and CLI plan

Specify the intended pipeline and responsibilities, including parsing, source maps/spans, name resolution, type checking, normalization, diagnostics, dependency graphs, and emission.

Decide which language features belong in the graduation release. Specify the semantics of supported constructs; explicitly defer advanced features that would consume disproportionate effort.

Cover reusable compiler APIs for CLI/LSP/tests, deterministic generation, portable paths, reproducible configuration, cancellation, incremental compilation only where justified, stable diagnostics, formatting, linting, API compatibility checks, and machine-readable CLI output.

Define safe generation behavior, manifests of generated files, ownership conflicts, interrupted generation, dry runs, clean operations, and preservation of hand-written code. Explain how configuration avoids environment-dependent output while still producing useful local toolchain diagnostics.

## 9. Built-in emitters and generated-code quality

Inventory current backends, client languages, framework adapters, and auxiliary generators. Recommend a sustainable supported subset based on user value and testing/maintenance cost.

Use explicit tiers such as fully supported, experimental, and deferred. Do not promise complete parity across every language and framework. Estimate the actual testing matrix and its cost.

Specify the boundary between language-level generation, transport behavior, framework adapters, validation libraries, and optional integrations. Resolve the meaning of “zero runtime dependencies” separately for generated types, clients, validation, routes, and framework integrations.

Include capability declarations, stable type mapping, imports/naming, deterministic formatting, unsupported-feature diagnostics, runtime version policy, and shared conformance fixtures. Require unsupported semantics to fail clearly rather than silently generate incorrect code.

Define testing beyond snapshots: generated-code compilation/type-checking, semantic assertions, request/response round trips, cross-language interoperability, error cases, and reproducibility. Evaluate whether OpenAPI, GraphQL, SQL/Prisma, mock servers, Docker, and CI generation belong in the core, optional emitters, or deferred scope.

## 10. External emitter plugins

Design a true extension system that allows third-party emitters without rebuilding the compiler.

Compare in-process native plugins, executable plugins using a versioned stdin/stdout protocol, WebAssembly/WASI, and language-specific hosted extensions. Choose a graduation-scale approach based on portability, complexity, trust, and developer experience.

Specify discovery, manifests, identity, installation, pinned versions, capability negotiation, schemas, IR access, structured diagnostics, progress/logging, cancellation, timeouts, failure handling, output mediation, deterministic behavior, and compatibility checks.

Separate process isolation from sandboxing. State exactly what filesystem/network restrictions are actually enforceable in the proposed implementation. Define trusted/untrusted execution policy without promising security controls the team will not implement.

Provide a minimal plugin manifest, illustrative request/response exchange, and a “hello emitter” development journey. Label examples as proposed designs. Include a plugin SDK, test harness, packaging flow, and one externally installed reference emitter as a potential acceptance test.

## 11. Language server and editor plugins

Recommend how the compiler is reused by an LSP implementation and how much logic stays in each editor client.

Prioritize diagnostics, completion, hover, definitions, references, rename, formatting, code actions, and semantic tokens by user value and cost. Include error-tolerant editing, unsaved documents, workspace imports, Unicode position encoding, cancellation, and performance measurements.

Evaluate VS Code and JetBrains separately, including the maintenance cost of their integration mechanisms. Decide the primary graduation editor and whether the second receives full, limited, or deferred support.

Cover server discovery/download, version compatibility, offline behavior, settings, logs, signed/checksummed distribution where applicable, packaging, and update testing. Keep canonical language metadata synchronized without manual duplication.

## 12. Packages, dependencies, and distribution

Keep these three concerns separate:

1. Installing Veld itself through npm, PyPI/pip, Homebrew, Go, NuGet, Composer, or standalone binaries.
2. Installing Veld ecosystem artifacts such as reusable contracts and emitter plugins.
3. Publishing generated SDKs into the target language's existing package ecosystem.

Recommend which channels to support first and which to defer. Explain wrapper packages versus native reimplementations, platform/architecture selection, checksums, interrupted/offline installation, permissions, proxies, lifecycle-script restrictions, and uninstall behavior.

For Veld packages, specify manifest semantics, namespaces, dependencies, version constraints, resolution strategy, lockfiles, hashes, transitive dependencies, conflicts, local/workspace packages, cache layout, registry selection, and reproducible offline operation. Decide whether contracts and executable plugins need different package types and trust policies.

Do not casually propose rebuilding npm or pip. Identify the smallest useful Veld package system and existing tooling to reuse. Explain how credentials are separated by registry and how private/public package resolution avoids dependency confusion.

## 13. Registry architecture

Review the existing registry and recommend its scope and architecture.

Cover public/private packages, organizations, roles, ownership transfer, scoped tokens, revocation, authentication, package search, version immutability, deprecation/yanking/deletion, retention, and audit events. Prioritize these rather than treating every enterprise feature as mandatory.

Define registry API boundaries, metadata schema, artifact storage abstraction, checksum verification, publication atomicity, concurrent publication, idempotency, failed upload cleanup, migrations, backup consistency, authorization tests, and resource limits.

Discuss executable plugins separately from data-only contracts. Include malicious archives, symlinks, decompression bombs, secret leakage, enumeration of private packages, and supply-chain risks in the threat model. Recommend controls proportional to graduation scope.

Decide whether registry hosting belongs inside the CLI binary, in a dedicated binary, or in a separate deployment package. Explain distribution and dependency consequences. Evaluate the registry UI independently from the public documentation site.

## 14. Self-hosting and operations

Design an achievable self-hosted deployment path, with clear minimum requirements and a clean-machine installation guide.

Evaluate a single-host deployment and Docker Compose for the default. Add Kubernetes, federation, replication, or high availability only if justified and clearly separated from the required release.

Cover configuration validation, secrets, TLS/reverse proxy, persistent volumes, database/artifact storage, bootstrap administration, optional email, health/readiness checks, logs, metrics, quotas, upgrades, migration failure, and recovery.

Specify tested backup and restore procedures for metadata and artifacts together. Set proposed recovery objectives and acceptance checks. Distinguish application rollback from restoring an incompatible database migration. Include private networks, custom domains, offline/air-gapped limitations, and the minimum operator runbook.

Provide a modest resource and operating-cost estimate with stated workload assumptions. Do not claim production reliability without supporting tests.

## 15. Open-source engineering, quality, and missing work

Identify important work we did not explicitly name. At minimum consider:

- Existing license verification, dependency license compatibility, generated-output licensing, contribution terms, and university IP-policy questions requiring confirmation.
- CONTRIBUTING, CODE_OF_CONDUCT, SECURITY policy, issue/PR templates, ownership, review rules, roadmap, and a lightweight RFC/ADR process.
- Reproducible development, CI, release automation, changelogs, semantic versioning, provenance/checksums, SBOMs where useful, and credential handling.
- Version support and deprecation, migration guides, telemetry/privacy decisions, accessibility of public UI/docs, and maintenance after graduation.
- Fast versus scheduled CI, parser fuzzing, property testing, plugin protocol compatibility, registry integration tests, installer smoke tests, and generated-code conformance.
- Documentation examples that actually compile/run, architecture drift detection, bug triage, and onboarding the first external contributor.

Prioritize these controls. Avoid a checklist that consumes the entire project before a working vertical slice exists. Supply measurable release criteria and identify the evidence needed to pass them.

## 16. Roadmap to June 2027 and scope control

Produce a dated plan from October 21, 2026 to submission in June 2027, using May 31, 2027 as the conservative readiness target. Organize it into monthly milestones and two-week sprints, with a shorter final sprint if necessary. Calculate the actual number of sprints that fit; do not use a 52-week or 26-sprint template. Include a concrete week-by-week plan for the first eight weeks. Later sprints may be forecast-level; do not imply that all details are already known. If work starts later than the baseline, compress scope and recalculate capacity without moving the deadline.

Sequence dependencies so the compiler/IR, plugin contract, package schema, registry, and editor work can progress without repeated redesign. Schedule a working end-to-end vertical slice early.

Distinguish discovery, stabilization, architecture decisions, implementation, integration, user testing, hardening, release, report completion, and defense rehearsal. Reserve visible time for exams and uncertainty; do not allocate all remaining weeks at 100% implementation capacity. Plan documentation throughout development. Protect at least the final four weeks before the readiness target for final integration, evaluation, release hardening, report completion, and demo rehearsal; schedule earlier integration continuously. Do not rely on June for core implementation.

For each milestone include deliverables, owners, dependencies, measurable exit criteria, estimated effort range, and what gets cut if it slips. Include alpha/beta/release gates, migration decision deadlines, feature freeze, and an explicit contingency budget without double-counting it.

Show the critical path and major risks. Provide a fallback plan for substantially lower capacity, a technical member becoming unavailable, a failed rewrite spike, and an emitter or plugin approach proving too expensive. Protect a coherent graduation deliverable before stretch work.

## 17. Team allocation: T1–T5 and D1

Do not invent team expertise. State any skill assumptions and give a short skills survey that could refine assignments.

Use a clearly labeled part-time capacity model. As an initial planning scenario, use 12 available hours per week per technical member and 8 for D1, then account explicitly for exams, coordination, reviews, and contingency. Calculate total available person-hours only across the actual remaining schedule to the readiness target, not a full year, and show the sensitivity to higher/lower availability. Budget June submission/defense work separately from implementation capacity. These numbers are assumptions to validate, not facts about us.

Give each workstream a primary technical owner, a backup, and a reviewer who is not the author. Distribute critical-path work and cross-cutting testing rather than making one person responsible for all integration, QA, or releases.

For every technical member specify primary domain, secondary domain, skills to learn, monthly deliverables through submission, expected effort, dependencies, and review obligations. Include pairing and knowledge-transfer checkpoints so subsystem ownership does not become a permanent silo.

Create a sprint responsibility table:

`Sprint/dates | shared milestone | T1 | T2 | T3 | T4 | T5 | D1 | integration/acceptance checkpoint`

Check that no member exceeds their capacity and that parallel tasks have stable interfaces or agreed mocks. Include a separate component ownership matrix and explain any uneven allocation.

D1's recurring work should include current/target diagrams, requirements traceability, ADR editing, installation/tutorial documentation, API/plugin documentation, meeting decisions, evaluation evidence, final report chapters, slides, and demo script. Name the technical member who validates each technical document. D1 may coordinate usability feedback, but engineering owners perform technical validation.

## 18. Diagrams and academic deliverables

Provide editable Mermaid diagrams where practical, with labels distinguishing **current implementation** from **proposed architecture**:

- System context and component/deployment views.
- Compiler pipeline and allowed dependency direction.
- External plugin interaction and trust boundaries.
- Registry publication and package consumption sequences.
- Registry entity-relationship model.
- Editor/LSP/compiler interaction.
- Self-hosted deployment and backup/restore flow.
- Roadmap dependencies and critical path.

Prefer diagrams that explain decisions; do not produce redundant diagram types just to increase their number. For each, state its purpose, owner, and update trigger.

Propose a graduation report outline covering problem definition, related work, requirements, architecture, implementation, testing, evaluation, limitations, project management, and future work. Map each chapter to evidence and team contributions. Include a defensible live-demo scenario and an offline fallback.

## 19. Required final report structure and artifacts

Deliver a complete, navigable report in this order:

1. Executive recommendation: product direction, retain/refactor/rewrite decision, recommended stack, supported scope, and principal risks.
2. Review methodology, revision, environment, evidence, and limitations.
3. Product assessment and comparison with alternatives.
4. Current architecture and verified capability inventory.
5. Prioritized defect register and documentation discrepancies.
6. Rewrite and language decision matrices.
7. Target architecture, repository layout, interfaces, and ADRs.
8. Compiler/language/CLI plan.
9. Built-in emitters and generated-code quality plan.
10. External emitter system.
11. Language server and editor integrations.
12. Package management, installation, and generated SDK distribution.
13. Registry and self-hosting plans.
14. Security, testing, releases, open-source governance, and ongoing maintenance.
15. Dated roadmap to June 2027, dependencies, capacity, and contingency.
16. T1–T5/D1 assignments and review responsibilities.
17. Diagrams, academic evaluation, report, and demonstration plan.
18. Prioritized backlog, acceptance criteria, unresolved decisions, and the first ten actions.

If file output is available, place the report in a clearly named review directory without overwriting historical plans. Include a report index, Markdown reports, Mermaid source, and CSV versions of the defect register, prioritized backlog, and sprint allocation.

Each backlog item needs a stable ID, outcome, priority, acceptance criteria, estimated effort range, owner, reviewer, dependencies, and target milestone. Make the initial milestone backlog implementation-ready; keep distant work at a sensible level of detail.

## 20. Final quality bar

Be candid and specific. Tell us which parts should be retained, fixed, redesigned, replaced, frozen, or removed, with evidence and consequences. Avoid blanket rewrites, blanket preservation, unsupported performance claims, and fashionable architecture choices.

For each major recommendation, explain the problem addressed, the chosen approach, alternatives rejected, costs, implementation sequence, and how success will be demonstrated.

Check consistency across the architecture, scope, estimates, staffing, sprint plan, and release criteria. Do not recommend a mandatory component without assigning implementation and verification capacity. Do not schedule downstream work before its required interface exists unless a clear provisional contract is defined.

Proceed with clearly stated assumptions where possible. Put only consequential unresolved questions in a final decision list; do not stop at a plan for how you would investigate. Complete the investigation and report to the extent your access allows, and distinguish verified findings from proposed future work throughout.
