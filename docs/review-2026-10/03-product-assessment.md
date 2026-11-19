# 03. Product assessment and comparison with alternatives

External facts in this chapter were gathered on 2026-10-21. `WebFetch` was blocked by a session hook, so facts rest on search-result summaries of the cited pages and are marked **provisional** unless the summary quoted the primary page. Full notes: the external-research appendix (`appendix-external-research.md`).

## 3.1 The problem Veld addresses

Teams that build an HTTP API and one or more clients repeat the same shape three times: request and response types on the server, validation of inbound data, and a typed client. Keeping these in sync by hand is the source of a familiar class of bugs (a renamed field, a changed status code, an optional field that became required). Existing answers either require a large specification format (OpenAPI), a transport you may not want (gRPC), or lock you into one language ecosystem (tRPC).

Veld's answer: a small, readable contract language (`.veld`) from which one command produces server-side interfaces plus route wiring, validation, and native client SDKs for several languages, with no runtime library to install. The repository's own phrasing is "contract-first, multi-stack API code generator" (README.md).

## 3.2 Intended users and three journeys

Primary users: small teams (2 to 15 developers) and solo full-stack developers building JSON-over-HTTP services with a separate frontend, and student or course projects where the contract doubles as documentation.

Journey A, full-stack TypeScript team (the graduation vertical slice): write `veld/app.veld` with models and a module; `veld generate` into `backend/src/generated` (Node service interfaces, routes, validators) and `frontend/src/generated` (fetch SDK, React hooks); implement the service interface; mount routes on an Express router; call the API from React with typed hooks; change the contract, regenerate, and let the compiler show every call site that broke.

Journey B, polyglot microservices: an IAM service in Node and a transactions service in Python; the transactions workspace entry `consumes: ["iam"]`; `veld generate` produces a Python client for IAM inside the transactions service; `veld deps --validate` rejects a cycle.

Journey C, shared contracts in an organization: a platform team publishes `@acme/common` (shared models, error shapes) to a self-hosted registry; product teams `veld pull @acme/common@1.x` and `import @acme/common/Money`; the lockfile pins the version and checksum; `veld generate --strict` fails CI when a pulled update is a breaking change.

Journeys A and B exist in the codebase with the defects listed in chapter 05. Journey C's publish/pull loop exists; the lockfile, checksum requirement and scoped-registry binding do not (ADR-008).

## 3.3 Alternatives (relevant capabilities only)

| Alternative | What it is | Server interfaces + routing | Multi-language clients | Runtime deps in output | Custom DSL | Editor support | Package / registry | Overlap with Veld |
|---|---|---|---|---|---|---|---|---|
| TypeSpec (Microsoft) | TypeScript-like API DSL compiling to OpenAPI, JSON Schema, Protobuf, and clients | Preview server emitters: `@typespec/http-server-csharp` 0.58.0-alpha.33 (2026), `http-server-js` (documented, version not confirmed) | Preview client emitters JS, C#, Java, Python | Clients depend on TypeSpec runtime packages | Yes, large and expressive (decorators, templates, namespaces) | VS Code and Visual Studio extensions | npm packages | Closest competitor; its server emitters are alpha-quality, which is exactly Veld's niche. Release train 1.17 in preparation (github.com/microsoft/typespec PR "Prepare typespec publish", provisional). License reported as MIT (prior knowledge; not confirmed in-session). |
| Smithy (AWS) | Protocol-agnostic IDL with traits; Java-based build and plugins | Server generators for TypeScript (developer preview), Java (smithy-java 1.0 GA Apr 2026), Rust (work in progress) | Clients: TypeScript, Go, Kotlin, Swift, Python, Java, Rust | Yes (Smithy runtime per language) | Yes, heavy; requires JVM toolchain | smithy-language-server, VS Code, IntelliJ plugin | Maven coordinates | Strong but JVM-centric and enterprise-shaped; overkill for Veld's audience. Apache-2.0 (github.com/smithy-lang/smithy). |
| OpenAPI 3.1 / 3.2 + generators | Specification format; generators: openapi-generator (JVM, 50+ targets, vendor claim), oapi-codegen (Go), openapi-typescript/openapi-fetch, orval, Kiota, Fern, Speakeasy, Stainless | openapi-generator produces server stubs for many frameworks; quality varies by target | Very broad | Varies; many clients depend on runtime libraries | No DSL; YAML/JSON, verbose to author by hand | Good (schema-aware editors) | None for specs; artifacts via npm, Maven | The standard Veld must interoperate with, not replace. OpenAPI 3.2.0 released 2025-09 with 3.1.2 patch (spec.openapis.org, provisional). |
| Protocol Buffers / gRPC / Connect / buf | Binary IDL, RPC framework; buf adds linting, breaking-change detection, plugin execution, Buf Schema Registry | Yes (service stubs) | Connect: Go, TypeScript, Swift, Kotlin, Python | Yes (protobuf runtime) | Yes (`.proto`) | Good | BSR with dependency management and remote plugins | Best prior art for the plugin protocol (CodeGeneratorRequest/Response over stdin/stdout) and for a schema registry with breaking-change checks. Different transport (not plain JSON/HTTP). |
| tRPC, ts-rest, Hono RPC, Zod-OpenAPI, Elysia Eden | End-to-end type sharing in TypeScript | Yes, TypeScript only | TypeScript only | Yes | No (TypeScript itself) | Native | npm | Superior developer experience inside one TS monorepo; no polyglot story. (Unverified in-session; prior knowledge.) |
| GraphQL + GraphQL Code Generator | Query language with plugin-based codegen | Resolver types | Many languages | Yes | GraphQL SDL | Good | npm | Different API style; Veld exports SDL. (Unverified in-session.) |

## 3.4 Is a custom DSL justified?

Three options were compared.

| Option | Learning cost | Editor support | Migration effort for users | Expressiveness | Long-term maintenance |
|---|---|---|---|---|---|
| Keep `.veld` as the authoring language | Low: about 12 keywords, one page of syntax | Must be built by the team (chapter 11) | New users start from scratch; OpenAPI users need an importer | Deliberately small; cannot express everything OpenAPI can (content types, security schemes, callbacks) | The team owns a language: grammar, spec, versioning, tooling |
| Build on TypeSpec or OpenAPI as the source | Medium to high (TypeSpec) or high verbosity (OpenAPI YAML) | Exists | Existing users can adopt directly | Full | The team owns only emitters; the project becomes "another OpenAPI generator" |
| Keep `.veld` and offer interoperability (export OpenAPI; later import a subset of OpenAPI) | Low | Team-built | Export lets Veld users join OpenAPI toolchains; import lets OpenAPI users try Veld | Small core, escape hatch through export | Language plus an importer that must track OpenAPI versions |

Recommendation: the third option. The DSL's smallness is the product: a contract that a reviewer can read in one screen and that the compiler can check fully. That is a defensible academic position (a minimal language with precise semantics and a conformance suite) and a practical one (fast editor support is feasible for a small grammar). The cost is that Veld must export correct OpenAPI 3.1 so users are never trapped; OpenAPI import is a stretch goal, not a graduation requirement.

What would reverse this: if the user study (chapter 17) shows that participants familiar with OpenAPI cannot author a `.veld` contract after the tutorial, or if TypeSpec's server emitters leave preview with comparable zero-dependency output before the Beta gate, the product should pivot to a TypeSpec emitter set and present the compiler work as prior art.

## 3.5 Product thesis and differentiation

Thesis: **a minimal, fully checked contract language whose compiler produces dependency-free server interfaces, route wiring, validation and clients for several stacks from one source, with the same safety properties (deterministic output, breaking-change detection, non-destructive generation) a build tool must have.**

Credible differentiation, stated honestly:

1. Server-side output (interfaces plus route wiring plus validation) across several backends, where TypeSpec is alpha and openapi-generator's server stubs are uneven. Veld's own server output has the defects in chapter 05, so this claim is earned only after M1 and M2.
2. Zero runtime dependencies in generated code, defined precisely in chapter 09 (true for types and clients; validation uses generated code, not a library; framework adapters import only the framework the user already has).
3. A contract that fits on a screen, with breaking-change detection and a lockfile for shared contracts.
4. One static binary with an editor server inside it.

Not differentiators (do not claim): the number of supported languages, GraphQL or SQL export, Docker and CI generation, a hosted registry.

## 3.6 Minimum defensible graduation deliverable, target release, stretch

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

## 3.7 Non-goals and deferred features (amended 2026-10-21 by ADR-012)

The original review deferred unions, WebSocket, streaming, auth schemes, per-field constraints, OpenAPI import, Docker/CI generation, plugin sandboxing, registry-hosted plugins, package signing and high availability. The project owner decided to include all of them as an ordered queue behind the core (chapter 20, ADR-012), subject to an automatic cut on 2027-05-02.

Remaining non-goals: generics beyond `Map` and arrays, conditional types, gRPC or binary transports, Kubernetes manifests, federated registries, a hosted public registry operated by the team, pagination helpers, custom scalar types, nullable as distinct from optional, enum members with explicit wire values. These stay out of 1.0.0 regardless of capacity.

## 3.8 Research and engineering questions for the report

1. Can a deliberately minimal contract language cover the HTTP APIs real projects write? Measure by re-expressing a corpus of public OpenAPI documents in `.veld` and recording the fraction of operations expressible and the constructs missing.
2. Does generated route wiring plus validation reduce contract drift compared with hand-written code? Measure on seeded contract changes: how many call sites and handlers the compiler flags versus manual review.
3. What does "zero runtime dependencies" cost in generated-code size and validation performance versus Zod or Pydantic? Measure bytes and request latency on a fixed workload.
4. Is a JSON-over-stdio plugin protocol sufficient for third-party emitters? Measure the time for an external developer to produce a working emitter from the SDK, and the overhead per invocation.
5. Can one language server serve two editors with no client-side language logic? Measure feature parity and maintenance effort (lines changed per language feature) before and after ADR-006.

## 3.9 Evaluation plan (planned measurements, no results yet)

| Question | Baseline | Representative tasks | Correctness measure | Reproducibility | Status |
|---|---|---|---|---|---|
| Expressiveness (Q1) | OpenAPI documents from public APIs (select 10 to 15 of varied size; list fixed in M5) | translate each to `.veld` | share of operations and schemas representable; list of blockers by category | scripts and corpus in `evaluation/expressiveness/` | planned |
| Drift detection (Q2) | same API hand-written in Express + fetch | apply 12 seeded changes (rename field, optional to required, status change, path change ...) | compiler errors and `veld diff` output versus ground truth | fixture repo with branches per change | planned |
| Generated-code cost (Q3) | Zod (Node), Pydantic (Python) validators for the same models | validate 10k requests, valid and invalid mix | identical accept/reject decisions; p50/p95 latency; bundle bytes | benchmark harness in `evaluation/validation/` | planned |
| Plugin protocol (Q4) | none (descriptive) | 3 to 5 developers write the hello emitter variant from the guide | time to first successful generate; defects found in the SDK | screen-recorded sessions with consent | planned |
| Editor parity (Q5) | the three implementations at 6a47f81 | 10 language-feature scenarios | features working per editor; lines changed per feature | scripted LSP session in CI | planned |
| User study (feasibility) | participants: 6 to 10 students or developers not on the team | 45-minute task: author a contract from a prose spec, generate, implement one handler, call it from the client | task completion, errors, SUS questionnaire, think-aloud notes | protocol and consent form in `evaluation/user-study/`; D1 coordinates, T4 runs | planned, M5 |

All results will be reported with their collection date, environment and scripts; no number in this review is a result.
