# Veld — A Contract-First, Multi-Stack API Code Generator

**Graduation Project Proposal**

---

## Contents

1. [Overview](#1-overview)
2. [The Problem](#2-the-problem)
3. [How Veld Works](#3-how-veld-works)
4. [How Veld Solves the Problem](#4-how-veld-solves-the-problem)
5. [Objectives](#5-objectives)
6. [Scope](#6-scope)
7. [Technology Stack](#7-technology-stack)
8. [Key Subsystems](#8-key-subsystems)
9. [Design Highlights](#9-design-highlights)
10. [What Is Complete](#10-what-is-complete)
11. [What Is Not Finished](#11-what-is-not-finished)
12. [Problems That Must Be Solved](#12-problems-that-must-be-solved)
13. [Methodology & Timeline](#13-methodology--timeline)
14. [Deliverables & Demonstration](#14-deliverables--demonstration)
15. [Future Work](#15-future-work)
16. [Appendices](#appendix-a--repository-map)

---

## 1. Overview

**Project name:** Veld

**Tagline:** *One contract, every stack — a contract-first compiler that generates typed backend service interfaces, route wiring, and client SDKs across eight backend and ten frontend targets.*

**Repository:** `github.com/Adhamzineldin/Veld`
**Implementation language:** Go 1.22
**Scale:** ~56,000 lines of Go across 261 source files and 22 `internal/` packages; 52 test files; 18 code generators (8 backend, 10 frontend) plus 6 auxiliary generators.

Veld is a compiler. Its input is a `.veld` contract file describing an API — its data models, its endpoints, its validation rules. Its output is working source code in up to eighteen language and framework targets simultaneously: typed service interfaces and HTTP route wiring for the server, and typed client SDKs for every consumer of that server.

This proposal describes the engineering problem Veld addresses, the mechanism by which it addresses it, and an unvarnished account of the system's current state — including a full defect register produced by a systematic audit of all 261 Go source files. That audit is reported in §12 rather than omitted, because a committee is better served by a project that knows precisely what is wrong with it than by one that claims nothing is.

---

## 2. The Problem

### 2.1 Context

Modern software systems are routinely polyglot. A single product may run a Node.js identity service, a Python service for analytics, a Go service for latency-sensitive work, and serve a React web client alongside mobile clients in Kotlin and Swift. Every one of those components must agree, exactly, on the shape of every API: the URL, the HTTP method, the request body, the response body, the status codes, and the error cases.

In current practice that agreement is maintained **by hand, and separately in every component**. There is no artefact that all of them are derived from. There is only a convention — usually a document, sometimes only a conversation — that each team is trusted to implement correctly and to keep implementing correctly as the system changes.

This produces four recurring, well-understood, and expensive failure modes.

### 2.2 Failure mode 1 — Contract drift between server and client

A backend engineer changes a response field from required to optional, or renames it, or changes its type from integer to string. The client is updated later, or partially, or not at all.

Nothing in either build catches this. The server compiles; the client compiles. The failure appears at runtime, in production, and frequently manifests not as an exception but as a silent `undefined` propagating through the UI until it surfaces somewhere unrelated to its cause. Because the two sides of the contract have no shared source of truth, **there is nothing in the system capable of detecting that they have diverged.** Drift is not caught late; it is structurally undetectable.

This is the central problem. The remaining three compound it.

### 2.3 Failure mode 2 — Inconsistent validation

Input validation is written per endpoint, per service, by whichever engineer implemented that endpoint. One endpoint validates that an email field is well-formed; the endpoint beside it does not. One service rejects malformed input with `400` and a structured error body; another lets the exception escape and returns `500` with a stack trace.

Validation coverage therefore becomes a property of individual diligence rather than a property of the system. It cannot be audited, because there is no single place where it is expressed — only a diffuse scattering of hand-written checks whose absence is invisible.

### 2.4 Failure mode 3 — Repeated boilerplate across services and languages

The code that binds an HTTP route to a typed handler, parses and validates a request body, and serialises a typed response is entirely mechanical. It is also rewritten from scratch for every endpoint, in every service, in every language.

In a six-service polyglot system, the same conceptual endpoint is implemented six times in six idioms. Each implementation is an independent opportunity for error, and each must be independently maintained. The work is neither creative nor difficult; it is simply endless, and it scales multiplicatively with the number of services times the number of endpoints.

### 2.5 Failure mode 4 — Client SDKs as a maintenance tax

A team that wants a typed client for its own API has two options: hand-write and hand-maintain one SDK per consuming language, or give up and accept untyped `fetch` calls at every call site.

Hand-written SDKs are, in practice, the first artefact to fall out of date — they are downstream of the server, nobody's primary responsibility, and their staleness is invisible until it causes an incident. The alternative, untyped calls, simply relocates the problem: every call site becomes a place where the contract can be violated without the compiler noticing.

### 2.6 Why existing tooling does not close the gap

| Approach | Why it falls short |
|---|---|
| **OpenAPI / Swagger** | Predominantly *specification-after-the-fact* — generated from annotations on server code that already exists. It therefore **documents** drift rather than preventing it: if the server changes, the spec changes with it, and the client is still on its own. Its generators additionally produce output with heavy runtime dependencies and framework lock-in. |
| **gRPC / Protobuf** | Genuinely contract-first, and solves drift correctly — but imposes a binary wire format, a runtime library, and a toolchain that many HTTP/JSON teams cannot or will not adopt. It is a solution available only to teams willing to abandon REST. |
| **Hand-written shared type packages** | Work within one language. The moment a second language enters the system, the shared package is no longer shared. |
| **JSON Schema alone** | Describes data shape but not endpoints, methods, paths, or status codes — it is half a contract, and generates nothing. |

The gap is therefore specific: **there is no contract-first, code-generating, framework-agnostic tool for teams who use ordinary HTTP and JSON across multiple languages.** That gap is what Veld occupies.

---

## 3. How Veld Works

### 3.1 The core idea

Veld inverts the direction of authority. Instead of the code being primary and the specification being derived from it, **the contract is the source artefact and all code is compiled output.**

A developer writes one `.veld` file:

```
model User {
  id:        uuid
  email:     string
  nickname?: string
  role:      Role @default(user)
  createdAt: datetime @serverSet
}

enum Role { admin user guest }

module Users {
  prefix: /users

  action GetUser {
    method: GET
    path:   /:id
    output: User:200
    errors: notFound:404
  }

  action CreateUser {
    method: POST
    path:   /
    input:  User
    output: User:201
  }
}
```

From this single file, one `veld generate` invocation produces — simultaneously, for every configured target — typed data models, service interfaces the developer implements, HTTP route wiring that binds those interfaces to the framework, runtime input validation, and typed client SDKs for every consumer.

Two constraints govern all output, enforced throughout the codebase:

- **Zero runtime dependencies by default.** Generated code uses native platform facilities — `fetch` in TypeScript, `urllib` in Python, `net/http` in Go, cURL in PHP. No generated file requires an external package for type-only usage.
- **Framework-agnostic output.** Route handlers accept an injected router rather than importing a framework, so the developer wires Veld's output into their own Express, Flask, Chi, Axum, or Spring application. **Veld never writes into developer-authored business logic files.**

### 3.2 The compilation pipeline

Veld is structured as a conventional compiler. The pipeline is strictly linear, and only the AST crosses stage boundaries:

```
                     .veld source files
                            │
                            ▼
   ┌──────────────────────────────────────────────────┐
   │  LEXER              internal/lexer               │
   │  character stream → token stream                 │
   │  hand-written scanner; recovers from bad input   │
   └──────────────────────────────────────────────────┘
                            │
                            ▼
   ┌──────────────────────────────────────────────────┐
   │  PARSER             internal/parser              │
   │  token stream → typed AST                        │
   │  recursive descent; tracks source line numbers   │
   └──────────────────────────────────────────────────┘
                            │
                            ▼
   ┌──────────────────────────────────────────────────┐
   │  LOADER             internal/loader              │
   │  per-file ASTs → one merged AST                  │
   │  recursive imports, aliases, globs, cycle guard  │
   └──────────────────────────────────────────────────┘
                            │
                            ▼
   ┌──────────────────────────────────────────────────┐
   │  VALIDATOR          internal/validator           │
   │  semantic checks; never mutates the AST          │
   │  types, inheritance, routes, import visibility   │
   └──────────────────────────────────────────────────┘
                            │
                            ▼
   ┌──────────────────────────────────────────────────┐
   │  EMITTER REGISTRY   internal/emitter             │
   │  resolves target name → emitter implementation   │
   └──────────────────────────────────────────────────┘
                            │
        ┌───────────┬───────┴───────┬──────────────┐
        ▼           ▼               ▼              ▼
   8 backend   10 frontend     6 auxiliary   inter-service
    emitters     emitters       generators       SDKs
        │           │               │              │
        └───────────┴───────┬───────┴──────────────┘
                            ▼
                       generated/
```

**Lexer.** A hand-written scanner producing a flat token slice. Rather than aborting on the first bad character, it **recovers** — recording the error, skipping the character, and continuing — so the developer receives a complete error list from one run rather than fixing errors one at a time.

A notable design detail is the treatment of **contextual keywords**. Words such as `input`, `output`, `query`, `headers`, `description`, `method`, `path`, `stream`, and `extends` are classified as ordinary identifiers by the lexer and interpreted positionally by the parser. This allows them to be used as field names inside models — a contract may legitimately declare a field called `description` or `path` — without reserving them globally.

**Parser.** A recursive-descent parser (~1,150 lines) producing the typed AST. Every node records the source line at which it was declared, which is what allows the validator to emit `file:line` diagnostics.

**Loader.** Resolves `import` statements recursively and merges the results. It supports alias imports (`import @models/user`), relative imports (`import "./models/user.veld"`), and registry-package imports (`@org/package/...`) resolved against a local `packages/` directory. Glob patterns (`*`, `**`) and compound extensions (`user.veld` → `user.model.veld`) are supported. A `seen` set keyed on absolute path terminates import cycles. Critically, the loader records a **`FileImports` map** of which file directly imported which — a structure the validator later uses for a check most generators omit (§3.3).

**Validator.** Performs semantic analysis and returns *all* errors rather than stopping at the first, each with `file:line` context.

**Emitters.** Independent, self-registering code generators. No emitter imports the lexer or parser; they consume only the AST. This isolation is what makes the target count scalable.

### 3.3 What the validator checks

- **Duplicate and colliding names** — duplicate models, enums, modules, actions, constant groups, and fields; plus cross-category collisions (a name used as both model and enum).
- **Inheritance integrity** — `extends` must name an existing model, and the chain is walked to detect cycles.
- **Type resolution** — every field type, map value type, union member, action input, output, query, header, `stream`, and `emit` type must resolve to a primitive, model, or enum.
- **Spelling suggestions** — unresolved names are matched against known types by Levenshtein distance, thresholded at half the identifier length, producing `did you mean "User"?`.
- **Default-value compatibility** — `@default` values are checked against the declared field type, including enum membership.
- **WebSocket coherence** — `WS` actions must declare a `stream` type; `stream`/`emit` are rejected on non-`WS` actions.
- **HTTP status validity** — explicit success statuses must be 200–299, and `204 No Content` may not carry an output type.
- **Cross-module route conflicts** — every `(method, path)` pair is normalised (`/users/:id` and `/users/:userId` both become `/users/:param`) and checked for collisions across module boundaries.
- **Import visibility.** The check most generators omit. Using the loader's `FileImports` map, the validator verifies that every referenced type is defined either in the same file or in a file it *directly* imports. A type that resolves only because some unrelated file transitively pulled it in is reported as an error — preventing contracts that "work by accident" and break when an unrelated import is removed.

### 3.4 The emitter registry — how one AST serves eighteen targets

The architectural centrepiece is the mechanism that lets eight backend and ten frontend emitters share a single AST contract while the core compiler has no knowledge of any of them.

Three interfaces and three registries are defined in `internal/emitter/emitter.go`:

```go
type Emitter interface {
    Emit(a ast.AST, outDir string, opts EmitOptions) error
}

type BackendEmitter interface {
    Emitter
    Summarizer
    IsBackend()   // marker method
    EmitServiceSdk(consumed []ConsumedServiceInfo, outDir string, opts EmitOptions) error
}

type FrontendEmitter interface { Emitter; Summarizer; IsFrontend() }
type ToolEmitter     interface { Emitter; Summarizer; IsTool() }
```

Each emitter registers itself from an `init()` function:

```go
// internal/emitter/backend/rust/main.go
func init() { emitter.RegisterBackend("rust", New()) }
```

and the CLI activates it with a single blank import. **Adding a new language target therefore requires no change to the compiler at all** — it is a new package plus one import line. The core resolves targets by string lookup and never references a concrete emitter type.

### 3.5 Framework variance — the strategy pattern

Within a single language, output must differ by framework: an Express handler and a bare Node.js handler differ in router type, request/response types, imports, and dependencies. Rather than branching on framework strings throughout the emitter, each backend defines a strategy interface:

```go
type NodeFrameworkStrategy interface {
    RouterType() string
    RequestType() string
    ResponseType() string
    ExtraImports() []string
    PackageDependencies() map[string]string
    WSImports() string
    WSRouterParam() string
}

func New(framework string) NodeFrameworkStrategy {
    switch framework {
    case "express": return &ExpressStrategy{}
    default:        return &PlainStrategy{}
    }
}
```

This yields a clean two-dimensional matrix — *language* × *framework* — where each axis varies independently:

| Backend | Framework strategies |
|---|---|
| node-ts / node-js | `express`, `plain` |
| python | `flask`, `fastapi`, `plain` |
| go | `chi`, `gin`, `plain` |
| rust | `axum`, `plain` |
| java | `spring`, `plain` |
| csharp | `aspnet`, `plain` |
| php | `laravel`, `plain` |

### 3.6 Multi-service workspaces

For microservice systems, a workspace configuration declares each service, its language, its base URL, and the services it `consumes`. Generation proceeds in **two passes**:

**Pass 1 — parse everything.** Each entry's contract is parsed into a cached AST. Before parsing, the loader's alias map is augmented with a **cross-service alias** for every sibling, binding each sibling's name to its service root directory — so one service's contract can import another's type definitions by name.

**Pass 2 — resolve dependencies and emit.** Each entry's `consumes` list is resolved against the pass-1 cache into `ConsumedServiceInfo` values (name + full AST + base URL). After the entry's own code is generated, `EmitServiceSdk(...)` writes a typed client for each consumed service into `sdk/<service>/` — **in the consumer's language, not the producer's.** A Python service consuming a Node.js service receives a Python client.

The two-pass structure is necessary rather than incidental: a service can only generate a client for a sibling once that sibling's AST exists, and dependency edges may point in any direction. Separating parsing from emission makes the whole graph available before any code is written, so generation order is irrelevant and mutual consumption works without special-casing.

Frontend entries receive distinct treatment: a frontend consumes *every* backend, so rather than emitting one SDK per service, Veld merges all consumed ASTs into one (`MergeASTs`) while recording each module's base URL separately. The frontend developer receives **one unified typed client** covering the whole platform, in which each call is nonetheless routed to the correct service host.

---

## 4. How Veld Solves the Problem

Each failure mode from §2 maps to a specific mechanism. The mapping is direct, and each row is verifiable in the source.

| Problem (§2) | Mechanism | Why it works |
|---|---|---|
| **Drift between server and client** | Server code and client SDK are emitted from **the same AST in the same invocation**. | Drift is not merely discouraged, it is **structurally impossible**: the two artefacts are outputs of one compilation. There is no window in which they can disagree, because there is no separate act of writing the client. |
| **Drift over time, as the contract changes** | `veld watch` regenerates on save; `.veld.lock.json` + `internal/diff` classify every change as breaking or additive; `--strict` fails the build. | The contract cannot silently decay. A breaking change is caught **at build time, before code is generated** — an unusual property for an HTTP/JSON system, normally available only in gRPC ecosystems. |
| **Inconsistent validation** | Validation is generated from the contract's type declarations, uniformly, for every endpoint. | Validation coverage becomes a property of the *contract*, not of individual diligence. An endpoint cannot be accidentally left unvalidated, because no human writes the validation. |
| **Repeated boilerplate** | Route wiring, body parsing, serialisation, and status-code selection are all emitted mechanically. | The mechanical work is done once, in the emitter, rather than once per endpoint per service per language. Adding an endpoint is an edit to one contract file. |
| **Client SDKs as maintenance tax** | Ten frontend SDK targets plus per-language inter-service SDKs, all regenerated on every build. | SDKs stop being downstream artefacts that rot. They are build outputs, refreshed automatically, and cannot be stale relative to the contract. |
| **Polyglot fragmentation** | One contract compiles to eight backend and ten frontend targets. | The contract is the lingua franca. A Go service and a Swift client agree because both are compiled from the same declaration, not because two teams remembered to. |

### 4.1 The compounding argument

These mechanisms reinforce one another. Because generation is deterministic and cheap, regenerating is the path of least resistance — so the contract stays authoritative instead of decaying into documentation. Because the contract is authoritative, the diff against the lock file is meaningful — it reflects real API evolution. And because breaking changes are detected at build time, the contract can be changed confidently, which is what keeps developers editing the contract rather than routing around it.

The failure mode Veld is designed to prevent is not any single bug. It is the gradual, unnoticed decay of agreement across a system too large for any one person to hold in mind.

---

## 5. Objectives

| # | Objective | Success criterion |
|---|---|---|
| **O1** | Design and implement a contract DSL with a complete compiler front end. | Hand-written lexer and recursive-descent parser producing a typed AST supporting models, inheritance, enums, constants, modules, actions, arrays, maps, unions, optionality, and field annotations. |
| **O2** | Generate working backend code for **eight** target languages from one contract. | `veld generate --backend=X` produces compiling service interfaces, route wiring, and types for X ∈ {node-ts, node-js, python, go, rust, java, csharp, php}. |
| **O3** | Generate client SDKs for **ten** frontend targets, synchronised by construction. | Server and client artefacts emitted from the same AST in one invocation; no manual synchronisation step exists. |
| **O4** | Support multi-service workspaces with typed inter-service clients. | A `consumes` declaration produces, in each consumer's output directory, a typed zero-dependency HTTP client for each consumed service, in the consumer's own language. |
| **O5** | Enforce contract correctness statically and detect breaking changes before they ship. | Semantic validation with `file:line` context and spelling suggestions; structural diff against a persisted lock file that can fail a CI build. |
| **O6** | Deliver the developer experience required for adoption. | Self-hosted contract registry, incremental regeneration, formatter, linter, Language Server, and editor plugins for VS Code and JetBrains. |

---

## 6. Scope

### 6.1 In scope

Compiler pipeline (lexer, parser, AST, loader, validator); 8 backend emitters with framework strategies; 10 frontend emitters; 6 auxiliary generators (OpenAPI, Dockerfile, CI/CD, SQL schema, `.env`, test scaffolding); multi-service workspace generation with inter-service SDKs; a PostgreSQL-backed contract registry with accounts, TOTP, organisations, and package publish/pull; developer tooling (incremental cache, breaking-change diff, formatter, linter, LSP, watcher); VS Code and JetBrains plugins; 10 worked examples; CI with build, vet, race-enabled tests, and 5-platform cross-compilation.

### 6.2 Explicitly out of scope

Recorded so the committee can distinguish *unbuilt by decision* from *incomplete by oversight*.

- **ORM / persistence layers.** Veld generates a SQL schema but no ORM code, migrations, or query builders. It is an API contract compiler.
- **Business logic generation.** Veld emits interfaces and stubs; the developer implements the service. This is a design commitment — Veld never writes into developer-authored files.
- **Authentication implementation.** Contracts may declare `middleware: AuthGuard`; Veld emits the wiring, the developer supplies the middleware.
- **A hosted public registry.** The registry is self-hosted; operating a public multi-tenant instance is out of scope.
- **Runtime API versioning or content negotiation.** Veld detects breaking changes at build time only.

---

## 7. Technology Stack

### 7.1 Why Go for the compiler

**Single-binary distribution.** A code generator is adopted only if it is trivial to install. Go compiles to a single statically linked executable with no runtime — critical for a tool that must run identically on a laptop and in a CI container. The CI matrix already cross-compiles for five OS/architecture pairs from one source tree.

**Structural typing.** The plugin registry depends on defining small behavioural contracts that implementers satisfy implicitly. Go's implicit interface satisfaction lets an emitter conform to `BackendEmitter` without importing anything that knows about the registry — precisely the decoupling required.

**`init()`-based self-registration.** Go's package initialisation lets an emitter register itself as a side effect of being linked in. This is what reduces "add a language target" to "add a package and one blank import," with no central switch statement.

**A near-complete standard library.** The registry server runs on `net/http` with Go 1.22 method-pattern routing; JWT uses `crypto/hmac` directly; TOTP and tarball handling are standard library. The entire project has **three** direct third-party dependencies: `spf13/cobra`, `lib/pq`, and `golang.org/x/crypto`. For a project that preaches zero runtime dependencies in its output, a lean dependency graph in the tool itself is a matter of consistency.

**Startup performance.** Compilation runs on every file save under `veld watch`. Go's fast startup keeps the edit–regenerate loop imperceptible, which a JVM's startup cost would not.

### 7.2 Why this breadth of targets

The breadth is the thesis, not padding. The claim — *contract-first generation eliminates drift in polyglot systems* — is only testable if the system genuinely spans a polyglot range. Eighteen targets stress the architecture in ways a two-target prototype cannot:

- **Type-system diversity.** Structurally typed (TypeScript), nominally typed with inheritance (Java, C#), nominally typed without (Go, Rust), and dynamically typed (Python, PHP, JavaScript). Model inheritance must become `interface X extends Y` in TypeScript, class inheritance in Python and Java, and **flattened** in Go and Rust, which have no struct inheritance. Handling this uniformly is what forced the `LanguageAdapter` abstraction.
- **Error-handling models.** Rust's `Result`, Go's explicit error returns, and exception-based handling elsewhere require genuinely different handler shapes — which is why each backend carries its own `errors.go`.
- **Concurrency models.** `async`/`await` in TypeScript and Rust, synchronous handlers in Flask.

| Layer | Targets |
|---|---|
| **Backends (8)** | TypeScript/Node, JavaScript/Node, Python, Go, Rust, Java, C#, PHP |
| **Frontends (10)** | TypeScript, JavaScript, React, Vue, Svelte, Angular, Dart, Kotlin, Swift, types-only |
| **Auxiliary (6)** | OpenAPI 3.0, Dockerfile, CI/CD workflows, SQL schema, `.env`, test scaffolding |
| **Registry** | Go + PostgreSQL, embedded SPA |
| **Editors** | VS Code (TypeScript), JetBrains (Kotlin) |

The frontend selection covers the three dominant client contexts: web frameworks, mobile (Dart, Kotlin, Swift), and framework-neutral baselines.

---

## 8. Key Subsystems

### 8.1 The `.veld` contract language

```
model User {
  description: "A registered user"
  id:        uuid
  email:     string
  nickname?: string                    // optional
  tags:      string[]                  // array
  metadata:  Map<string, string>       // map
  role:      Role @default(user)       // default value
  createdAt: datetime @serverSet       // omitted from input types
  legacyId:  string @deprecated "use id"
}

model AdminUser extends User {          // inheritance
  permissions: string[]
}

enum Role { admin user guest }
constants Limits { maxPageSize: int = 100 }

module Users {
  prefix: /users

  action GetUser {
    method: GET
    path:   /:id
    output: User:200
    query:  UserQuery
    headers: { Authorization: string }
    errors: notFound:404, forbidden:403
    middleware: AuthGuard
  }

  action WatchUsers {
    method: WS
    path:   /watch
    stream: User                        // server → client
    emit:   UserFilter                  // client → server
  }
}
```

Field annotations represented in the AST: `@default`, `@deprecated`, `@example`, `@unique`, `@index`, `@relation`, `@serverSet`. Actions support explicit success statuses, array outputs, typed error codes with per-error HTTP statuses, inline anonymous field sets, per-module prefixes and base URLs, and WebSocket actions with distinct `stream`/`emit` types.

Primitives: `string`, `int`, `long`, `float`, `decimal`, `bool`, `date`, `datetime`, `time`, `uuid`, `bytes`, `any`, `json`. The `decimal` mapping illustrates the care taken with cross-language fidelity — `string` in TypeScript and Go (avoiding floating-point loss), `Decimal` in Python and Swift, `BigDecimal` in Java and Kotlin, `decimal` in C#, `DECIMAL(19,4)` in SQL.

### 8.2 The self-hosted contract registry

A complete registry compiled into the same binary and started with `veld serve`, letting teams share `.veld` contracts the way npm shares JavaScript packages.

| Component | Implementation |
|---|---|
| Routing | Go 1.22 `http.ServeMux` with method patterns; CORS and logging middleware; graceful shutdown |
| Sessions | HMAC-SHA256 JWT implemented directly on `crypto/hmac` |
| API tokens | `vtk_`-prefixed, 32 bytes from `crypto/rand`; only the SHA-256 digest is persisted |
| Two-factor | TOTP setup, confirm, disable |
| Accounts | Registration, login, logout, email verification via SMTP |
| Organisations | Creation, listing, member management, membership-gated access |
| Packages | Multipart publish, download, version listing, deprecation, deletion |
| Storage | A `Backend` interface with a local filesystem implementation |
| Web UI | Single-page application embedded via `//go:embed` |

The client (`veld login` / `push` / `pull`) tars contract files, hashes them, uploads over multipart, and on pull verifies the server-returned digest before extracting with a path-traversal guard into `packages/@org/name/` — where the loader resolves `import @org/package/...` automatically.

### 8.3 Developer tooling

**Incremental generation.** SHA-256 **content hashes** in `.veld-cache.json`. Content hashing was chosen over modification times specifically because mtimes are unreliable in Docker builds and CI checkouts where they are reset. `veld watch` regenerates only what changed, with a 500 ms debounce.

**Breaking-change detection.** `internal/diff` compares the current AST against `.veld.lock.json`, classifying changes as `Breaking` or `Added` using real compatibility semantics: adding an *optional* field is additive, adding a *required* field is breaking; adding an action is additive, removing one or changing its route, method, or prefix is breaking.

**Linter.** Unused models, empty modules and models, duplicate routes and actions, missing descriptions, deprecated usage.

**Language Server.** LSP over stdin/stdout with completion, hover, go-to-definition, and live diagnostics — running the real lexer, parser, and validator, so editor diagnostics match CLI diagnostics exactly.

### 8.4 Editor integrations

**JetBrains plugin** (~3,700 lines Kotlin): a full PSI-based lexer, parser, and element hierarchy, plus syntax highlighter, annotator, completion contributor, reference contributor, documentation provider, structure view, and code-style settings. Its Validate and Generate actions shell out to the real `veld` binary.

**VS Code extension** (~1,680 lines TypeScript): TextMate grammar, snippets, language configuration, and JSON-schema validation for `veld.config.json`, plus a hand-rolled parallel `.veld` parser for diagnostics, completion, hover, and definition.

---

## 9. Design Highlights

Three decisions solve non-obvious problems and are directly verifiable in the source.

### 9.1 Compile-time enforcement of emitter feature parity

The obvious design is a registry of one `Emitter` interface with optional capabilities discovered at runtime by type assertion. Veld rejects this. `BackendEmitter` **embeds `EmitServiceSdk` in the interface itself**, so a backend that cannot generate inter-service clients **cannot be registered** — the build fails. Inter-service SDK support is not a feature some backends happen to have; it is a precondition of being a backend.

The three marker methods (`IsBackend()`, `IsFrontend()`, `IsTool()`) serve the complementary purpose. Because Go satisfies interfaces structurally, an emitter with only `Emit` and `Summary` would otherwise satisfy all three roles at once, and a tool generator could be silently registered as a backend. The no-op markers make the roles disjoint at compile time — giving the decoupling of a plugin system with the safety of a closed one.

### 9.2 Two-pass workspace resolution with cross-service alias injection

Generating inter-service clients poses a bootstrapping problem: service A's client for B requires B's resolved AST, but B may symmetrically consume A, and both must be able to *import each other's types* at the contract level.

Veld separates parsing from emission entirely. Pass 1 parses every entry into a cached AST and, before parsing each one, **injects a cross-service alias for every sibling**:

```go
for _, sibling := range rc.Workspace {
    if sibling.Name == entry.Name || sibling.Input == "" { continue }
    serviceRoot := filepath.Dir(filepath.Dir(siblingInput))
    crossAliases[sibling.Name] = serviceRoot
}
```

Only in pass 2, with the full graph in hand, is `consumes` resolved and `EmitServiceSdk` invoked. Generation order becomes irrelevant and mutual consumption needs no special case.

The frontend case is treated distinctly and elegantly: rather than emitting one SDK per service, Veld normalises route prefixes and **merges all consumed ASTs into one**, while recording each module's base URL separately. The result is a single unified typed client in which each call still routes to the correct service host — the microservice topology is preserved in the emitted URLs but hidden from the calling code.

### 9.3 Structural diffing against a persisted lock file

Breaking-change detection is normally done at the HTTP layer by replaying recorded traffic against a new build. Veld does it at the **contract** layer, before any code is generated.

`veld generate` persists the compiled AST to `.veld.lock.json`. On the next run, `diff.Diff(old, new)` classifies each difference by its effect on consumers, sorted breaking-first. Because this runs as a pre-emit gate, a developer is warned *before* incompatible client code exists, and `--strict` turns the same check into a CI failure.

The result is a property unusual outside gRPC ecosystems: **API compatibility becomes a build-time invariant of an HTTP/JSON system**, enforced by the same tool that produces the code, with the lock file as the reviewable record of the contract's evolution in version control.

---

## 10. What Is Complete

The following are implemented, tested, and demonstrable.

### 10.1 Compiler front end — the most mature part of the system

| Component | State |
|---|---|
| Lexer | Complete with error recovery, contextual keywords, block/line comments, generic and union delimiters. Unit-tested (383 lines of tests). |
| Parser | Complete recursive-descent implementation (~1,150 lines) with line tracking. Best-tested component in the repository (1,129 lines of tests). |
| AST | Complete type model including unions, maps, inheritance, all seven field annotations, typed errors, and WebSocket types. |
| Loader | Complete: recursive imports, alias resolution, registry-package imports, glob patterns, compound extensions, cycle detection, `FileImports` tracking. |
| Validator | Complete: 10 categories of semantic check including cross-module route conflicts, Levenshtein suggestions, and import-visibility analysis. Unit-tested. |
| Config | Complete: nested and legacy-flat formats, flag/env/file precedence, workspace entries. Unit-tested (804 lines). |

### 10.2 Code generation

- **8 backend emitters** registered and structurally complete, each with types, service interfaces, route wiring, error types, constants, framework strategies, and an inter-service SDK generator. All carry unit tests.
- **10 frontend emitters** registered and unit-tested. TypeScript is the reference implementation, including a `VeldWebSocket<TReceive, TSend>` class with automatic reconnection. React, Vue, and Svelte genuinely reuse it, layering hooks, composables, and stores on top.
- **6 auxiliary generators**: OpenAPI 3.0, Dockerfile, CI/CD workflows, SQL schema, `.env` templates, test scaffolding.
- **Inter-service SDK generation** for all eight backends, using only standard-library HTTP facilities.

### 10.3 Multi-service workspaces

Two-pass generation, cross-service alias injection, `consumes` resolution, AST merging for frontends, per-module base-URL routing, and dependency-graph validation (circular, self-referential, unknown) inspectable via `veld deps`.

### 10.4 Developer tooling

Incremental SHA-256 content-hash caching; breaking-change diffing with lock file and `--strict` CI gate; contract formatter; linter with eight rules; Language Server with completion, hover, definition, and diagnostics; file watcher with debounce; `veld doctor`; `veld setup` for tsconfig path aliases; AST JSON dump.

### 10.5 Registry

Feature-complete along the happy path: registration, login, TOTP, email verification, token management, organisations with membership, and the full package publish/download/deprecate/delete lifecycle, fronted by an embedded SPA. Client-side `login`, `push`, `pull` with digest verification.

### 10.6 Ecosystem

VS Code extension and JetBrains plugin; 10 worked examples including a six-service polyglot microservices workspace; CI running build, vet, and race-enabled tests plus five-platform cross-compilation; in-tree documentation set.

---

## 11. What Is Not Finished

Functionality that is written but unreachable, or partially implemented. Each entry was confirmed by repository-wide search for callers.

### 11.1 Fully built but never wired in

| Component | State |
|---|---|
| **Mock HTTP server** (`internal/mock`) | Complete and well-tested. Generates fake responses from the AST with path matching and CORS. **No `veld mock` command exists** — the package is imported nowhere. The functionality is finished; only the command registration is missing. |
| **Structured error type** (`internal/errors`) | Complete and tested, classifying failures by pipeline stage (`KindParse`, `KindValidation`, `KindEmit`, …) so callers can inspect errors programmatically. **Imported by no non-test file** — every pipeline stage still returns plain `fmt.Errorf`. |

### 11.2 Implemented but never called

| Component | State |
|---|---|
| Validation-annotation generators | `validation.go` exists in the rust, java, csharp, and php backends with complete per-field annotation builders — never called from any `Emit()`. |
| FastAPI route registration | `RouterSetup` / `RouteDecorator` fully implemented but never invoked; `routes.go` always emits Flask-shaped handlers. `--backend-framework fastapi` produces handler functions with **no routes registered**. |
| Python `requirements.txt` | `RequirementsEntries()` implemented per-framework, never called — no dependency manifest is written for any Python target. |
| C#/PHP language adapters | `internal/emitter/lang/{csharp,php}.go` are fully implemented and unit-tested but never called; those backends duplicate the logic inline. |
| PHP Laravel FormRequest validation | Fully built, never wired into `Emit()`. |
| Registry email/password CLI login | A complete client method exists but is never called — the CLI supports only pre-issued API tokens. |
| Private package publishing | The schema and visibility checks fully support it, but the `Publish` handler hardcodes every new package to `public`. |
| `config.ToolsConfig` | Parsed from `veld.config.json`'s `tools` block and never read — setting `tools.dockerfile: false` silently does nothing. |
| Global `--quiet` flag | Registered and documented; the variable is never read. |
| VS Code Generate/Validate commands | Declared in `package.json` with matching settings but never registered in `activate()` — invoking them does nothing. |

### 11.3 Stub or degraded implementations

| Component | State |
|---|---|
| Go Gin strategy | Self-labelled "stub"; uses deprecated stdlib API; exercised by no test. |
| WebSocket handler bodies | Only Node and Python emit usable handler bodies. Go, Rust, C#, and PHP emit `TODO` comment stubs; JavaScript emits a stub only. |
| Breaking-change diff coverage | Neither the diff nor the lock file captures **enums or constants** at all — removing an enum value a field depends on produces zero warning. |
| LSP import resolution | The LSP re-parses only the open document and never calls the loader, so a type defined in a sibling file is always flagged "undefined" in editor diagnostics for any multi-file project. |
| Distribution packaging | Homebrew and Chocolatey manifests ship literal `PLACEHOLDER_SHA256_*` values; the pip package has no binary-download mechanism at all. |

---

## 12. Problems That Must Be Solved

A systematic audit read all 261 Go source files plus the editors, website, packaging manifests, and examples. This section is the resulting defect register. It is included because a committee is better served by a project that knows precisely what is wrong with it than by one that claims nothing is.

**The finding that matters most is diagnostic rather than any single bug: the defects are not architectural.** The pipeline, the plugin registry, the AST contract, and the strategy pattern all do what they were designed to do. The failures are concentrated in the long tail of eighteen independently-maintained emitter implementations that were intended to stay in lockstep and have quietly drifted apart. That distinction determines the remediation strategy in §12.5.

### 12.1 Critical — non-compiling output, data loss, or unauthorised access

| # | Problem | Location | Consequence |
|---|---|---|---|
| C1 | **Rust + Axum output does not compile.** `build_router()` calls bare handler identifiers, but only the per-module router alias is imported — the handler functions themselves never are. | `backend/rust/{strategy/axum.go, routes.go}` | Every non-empty contract produces a project that fails `cargo build`. No test asserts on `router.rs` content. |
| C2 | **Go's default `plain` strategy does not compile.** `PlainStrategy` emits `mux.HandleFunc(...)`, but the router parameter is named `r` everywhere else — `mux` is undefined. | `backend/go/strategy/plain.go` | `veld generate --backend=go` with no framework flag — the documented zero-dependency default — produces non-compiling Go. Every test forces `chi`, so CI never exercises it. |
| C3 | **Python's default output is broken for any action with a body.** Handlers emit `from ..schemas.schemas import {X}Schema`, but **no emitter anywhere writes `schemas/schemas.py`** (confirmed: referenced once, defined nowhere). | `backend/python/routes.go` | Because `--validate` is opt-in, the *default* Python backend fails at import time for most real contracts. |
| C4 | **`veld fmt --write` silently deletes top-level `prefix:` lines.** The formatter has a `case lexer.TPrefix:` branch, but the lexer never emits `TPrefix` — `prefix` always lexes as a contextual `TIdent`. The real tokens fall through to `default: i++`, consuming them with no output. | `internal/format/format.go`, `internal/lexer/lexer.go` | **Silent source-file data loss.** Running the formatter removes the line from the developer's contract. Zero test coverage. Module-level `prefix:` is unaffected. |
| C5 | **Registry: five-minute 2FA bypass.** The partial JWT issued pending the second factor is produced by the *same* `IssueJWT` as a full session token, and `JWTClaims` has no field marking it partial. | `server/handlers/auth.go` | A partial token sent as a normal `Bearer` header is accepted as a fully authenticated session for its whole five-minute window — bypassing the second factor entirely. |
| C6 | **Registry: any authenticated user can enumerate every private package.** `ListPackages` widens visibility to `public + private` for any authenticated caller with no org-membership filter. | `server/handlers/packages.go` | Package *contents* stay protected (`GetPackage`/`Download` check membership correctly), but every private package name, org, and description across the registry is exposed to any registered account. |
| C7 | **Registry: arbitrary file write on publish.** The storage key is built from client-supplied `PkgName` and `Version`, which are checked only for non-emptiness. `storage.Local.path()` calls `filepath.Clean` but does not reject `..` segments. | `server/handlers/packages.go`, `server/storage/storage.go` | Any authenticated org admin with `write` scope can set `version` to a traversal string and write the uploaded tarball outside the storage root. |
| C8 | **Registry: API token expiry is stored but never enforced.** `expires_at` is set and persisted, but `GetTokenByHash` — called on every API-token request — never compares it to the current time. | `server/db/db.go` | An expired token authenticates successfully forever. There is no server-side way to time-limit a leaked token. |
| C9 | **PHP/Laravel: any `HEAD` action crashes the app at boot.** Routes are emitted as `Route::{lowercased-method}(...)`; Laravel's router has no `head()` method. | `backend/php/routes.go` | `routes/api.php` throws `BadMethodCallException` on every request cycle — a total failure to boot, not a degraded endpoint. |
| C10 | **Swift SDK sends the literal string `${id}`.** Path interpolation reuses the shared `ToTemplateLiteral` helper, which produces JS/Kotlin/Dart syntax; Swift requires `\(id)`. | `frontend/swift/client.go` | Every generated Swift method with a path parameter sends dead literal text in the URL. Tests check only the function signature, never the request URL. |
| C11 | **C# test scaffolding emits invalid C#.** Built via `fmt.Sprintf` using `{{`/`}}` as though it were Go-template escaping, which `Sprintf` does not implement. | `generators/scaffold/tests.go` | Generated `.cs` files literally contain doubled braces and fail to compile. |

### 12.2 High — silently wrong behaviour

| Problem | Location | Consequence |
|---|---|---|
| **`@serverSet` unenforced in four backends** | rust, java, csharp, php | The annotation's documented purpose is to omit server-owned fields (`id`, `createdAt`, `role`) from input types. These four never reference `Field.ServerSet` — every field stays client-settable. A **security-relevant** gap: clients can set their own `role`. |
| **C# swallows typed exceptions into flat 500s** | `backend/csharp/routes.go` | Only `catch (Exception e)` is generated, never a branch reading `ex.Status`/`ex.Code` — discarding the entire typed-exception system `errors.go` builds. |
| **Go silently drops query and header parameters** | `backend/go/routes.go` | `buildCallArgs` passes the literal `"nil"` for both regardless of the declared models. Compiles cleanly; request data is discarded on every request. |
| **Kotlin and Swift SDKs silently drop query parameters** | `frontend/{kotlin,swift}/client.go` | Kotlin emits `// query params omitted for brevity`; Swift never references the parameter. Both accept it in the signature and ignore it. |
| **React/Vue/Svelte/Angular generate broken code for WS actions** | those four emitters | None branch on `act.Method == "WS"`; each calls a camelCase method that does not exist (the client exports `subscribeTo<Name>`). Angular calls `this.http.ws()`, which does not exist on Angular's `HttpClient` at all. |
| **`--strict` gate skipped entirely in workspace mode** | `cmd/veld/main.go` | The pre-emit breaking-change check runs only in single-project mode. **CI safety via `--strict` silently does nothing for any microservices project** — the exact case where it matters most. |
| **Workspace logic implemented twice and already diverged** | `cmd/veld/main.go` | Two independent ~250-line implementations. The `generate` copy supports `--service-sdk`; the `watch` copy does not — so `veld watch` cannot reproduce what `veld generate --service-sdk` does. |
| **`extends` dropped in several SDKs** | rust (always); csharp, php (SDK path) | Consumed services using inheritance produce SDK models missing the parent's fields, silently. Only Java handles this correctly in both paths. |
| **C# and PHP inter-service SDKs are not actually typed** | `{csharp,php}/sdk.go` | Every client method returns raw JSON (`Task<string>` / `?array`) rather than the generated model, contradicting each file's own header comment. |
| **Go SDK never errors on unresolved base URL** | `backend/go/sdk.go` | `NewClient("")` proceeds silently with an empty base, deferring failure to a confusing malformed-URL error at request time. Node, JS, and Python all raise immediately. |
| **Diff blind to enums and constants** | `internal/diff` | Neither the diff nor the lock file captures them — removing an enum value produces zero breaking-change warning. |
| **LSP never resolves imports** | `internal/lsp/handler.go` | Cross-file types are always flagged undefined in editor diagnostics, despite compiling correctly via the CLI. |

### 12.3 Medium and low severity

**Medium.** OpenAPI is generated by two packages with no import relationship, producing measurably different specs for the same contract. The Veld-scalar→target-type mapping is hand-written in roughly ten independent switch statements, so adding a scalar requires ten coordinated edits. Route-conflict detection is implemented twice (validator and linter). A config-precedence bug lets a nested `validate:true` override an explicit flat `validate:false`. Six packages have no test coverage at all (`docsgen`, `graphqlgen`, `openapigen`, `language`, `registry`, `server`). Homebrew and Chocolatey manifests fail checksum verification today. The Docker Compose template hardcodes a plaintext Postgres password.

**Low.** A timing side-channel in login reveals whether an email is registered. Usernames are interpolated unescaped into HTML emails. Verification emails are sent from an unrecovered goroutine, so a panic would crash the server. No rate limiting exists on login, TOTP, or registration. A changed success status code is classified as non-breaking. `internal/cache` is not goroutine-safe. The LSP drops malformed JSON-RPC messages instead of returning the spec-required error. Five distribution packages hardcode versions independently with no documented bump process.

### 12.4 Why these defects exist — the systemic cause

Three structural causes account for nearly every high-severity finding, and each admits a structural fix.

1. **Tests assert on the generator, not on the generated code.** No test compiles or executes emitter output. C1, C2, C10, and C11 are all cases where the emitter runs successfully and produces broken source. Worse, C2 is invisible precisely *because* every Go test forces `chi`, so the default path is never touched.

2. **Eighteen independent implementations with no shared conformance contract.** `@serverSet` is honoured in Node and ignored in four other backends because there is no mechanism — beyond developer memory — requiring a new AST feature to be handled everywhere. The interface guarantees each emitter has an `Emit` method; nothing guarantees `Emit` handles every AST node.

3. **Duplicated logic that has drifted.** Workspace generation, OpenAPI export, route-conflict detection, and scalar type mapping each exist in two or more independent copies. In every case the copies have already diverged.

### 12.5 Remediation strategy

The ordering follows from §12.4: fix the systemic causes, and the long tail stops regenerating.

**Priority 1 — Stop shipping non-compiling output.** Fix C1–C4 (three one-to-few-line fixes plus writing the missing Python schema emitter), then add **compile-verification tests**: generate into a temp directory and invoke `cargo build`, `go build`, `tsc --noEmit`, `python -m py_compile` in CI. This closes the entire class, not just today's four instances.

**Priority 2 — Secure the registry.** Fix C5–C8: add a `partial` claim to `JWTClaims` and reject it in `resolveAuth`; filter `ListPackages` by org membership; validate `PkgName`/`Version` against a strict character class and reject `..` in `storage.path()`; compare `expires_at` in `GetTokenByHash`. Then add test coverage to `internal/server`, which currently has none on any auth-critical path.

**Priority 3 — Build a parity conformance matrix.** A table-driven suite asserting that every backend handles every AST feature — query, headers, `@serverSet`, WebSocket actions, typed errors, inheritance, maps, unions. This converts parity from a periodic manual audit into a continuously enforced property, and would have caught most of §12.2 automatically.

**Priority 4 — Deduplicate.** Collapse the two workspace implementations, the two OpenAPI generators, and the ~10 scalar-mapping switches into single sources of truth.

**Priority 5 — Wire in what is already built.** Add `veld mock`; adopt `internal/errors` across the pipeline; call the four dormant `validation.go` generators; register the VS Code commands.

---

## 13. Methodology & Timeline

### 13.1 Method of working

Development followed an **iterative, architecture-first** method. The pipeline and emitter registry were established before target breadth was pursued, so each additional language validated the abstraction rather than accreting special cases. The evidence is that adding a backend still requires only a new package and one blank import — a property that would not have survived being retrofitted.

Practices in use, all verifiable in the repository: continuous integration from early on (build, `go vet`, `go test -race`, plus five-platform cross-compilation on every push and pull request); test-alongside development (52 test files); dogfooding through ten worked examples; documentation maintained in-tree; and conventional commit messages throughout.

The §12 audit was itself part of the methodology — a systematic full-repository read undertaken specifically to establish an honest baseline before the hardening phase, rather than relying on the absence of known bugs as evidence of correctness.

### 13.2 Phases

Development spans **27 February 2026 – 19 April 2026** across 188 commits.

| Phase | Focus | Outcome |
|---|---|---|
| **1 — Language & front end** | Lexer, parser, AST, first TypeScript emitter | A working single-target generator; the AST contract everything later depends on |
| **2 — Emitter framework** | Registry, role interfaces, strategy pattern, Python as second target | Proof the abstraction generalises beyond one language |
| **3 — Target breadth** | Six further backends, ten frontends | Full polyglot coverage; extraction of shared helper layers |
| **4 — Correctness & safety** | Validator hardening, import-visibility checking, diff, lock file, linter, formatter | Contract errors caught at compile time; compatibility enforceable in CI |
| **5 — Microservices** | Workspace config, `consumes`, two-pass generation, SDKs for all eight backends | `EmitServiceSdk` promoted into the `BackendEmitter` interface |
| **6 — Ecosystem** | Registry server and client, LSP, editor plugins, auxiliary generators | Veld usable as a team tool rather than a personal one |
| **7 — Audit** | Full 261-file review | The defect register in §12 |
| **8 — Hardening** *(current)* | §12.5 remediation, in priority order | Compile-verified output; secured registry; enforced parity |

---

## 14. Deliverables & Demonstration

### 14.1 Deliverables

1. **The `veld` binary** — a single cross-platform executable with more than twenty-five commands: `init`, `validate`, `generate`, `watch`, `diff`, `lint`, `fmt`, `docs`, `openapi`, `graphql`, `schema`, `deps`, `doctor`, `setup`, `clean`, `ast`, `lsp`, `login`, `push`, `pull`, `serve`, and others.
2. **Source repository** — ~56,000 lines of Go across 22 packages, with CI.
3. **Editor plugins** — a packaged VS Code `.vsix` and a JetBrains plugin.
4. **Ten worked examples**, including the six-service polyglot microservices workspace.
5. **Documentation set** — developer guide, architecture overview, getting-started and service-SDK guides, roadmap.
6. **This proposal, the §12 audit, and the final report.**

### 14.2 The demonstration

Designed to make the central claim *visible* rather than asserted. It uses the paths verified working in §10 — `node-ts`, `react`, and `python` with `--validate`.

**Movement 1 — One contract, two stacks.** From an empty directory, `veld init` scaffolds a project. A short contract is written. A single `veld generate` produces a complete TypeScript/Node backend — types, interfaces, Zod schemas, Express wiring — *and* a React front end with typed hooks. The backend is started, the front end calls it, and the round trip works with no hand-written glue.

**Movement 2 — Drift made impossible.** A contract field is changed. `veld watch` regenerates both sides instantly; the front end's type error appears in the editor before the code is ever run. The same edit is then made in a compatibility-breaking way: `veld generate --strict` compares against `.veld.lock.json`, classifies the change as breaking, and exits non-zero — an HTTP/JSON API whose compatibility is enforced by the build.

**Movement 3 — Polyglot generation.** The *same* contract regenerates with `--backend=python --backend-framework=flask --validate`, producing an idiomatic Flask service. Diffing the output against the Node service shows one contract expressed correctly in two type systems.

**Movement 4 — Microservices and the registry.** The `microservices` example generates six polyglot services in one command. The demonstration opens the Python transaction service's `sdk/iam/` directory to show a **Python** client for a **Node.js** service, produced automatically from a `consumes` declaration, and `veld deps` prints the resolved dependency graph. Finally `veld serve` starts the registry; a contract is pushed and pulled into a second project, which imports it by name.

**Movement 5 — Honest engineering.** The audit is presented: how all 261 files were reviewed, what was found, why the defects cluster in emitter drift rather than architecture, and how the compile-verification and conformance-matrix strategies in §12.5 close entire defect classes rather than individual bugs.

### 14.3 Evaluation criteria

- **Correctness** — CI passing across 52 test files and five platforms; and, after Priority 1, generated output compiling in every demonstrated target.
- **Breadth** — eight backends and ten frontends generating from one contract.
- **Reduction in hand-written code** — generated lines versus contract source lines, measured across the worked examples.
- **Drift elimination** — demonstrating that a contract change *cannot* leave server and client inconsistent.
- **Determinism** — repeated generation from identical input producing byte-identical output.

---

## 15. Future Work

### 15.1 Immediate — the §12.5 remediation programme

Compile-verification in CI for every emitter; the four critical output bugs (C1–C4); registry security (C5–C8); the parity conformance matrix; deduplication of the four duplicated subsystems; and wiring in the already-built mock server and structured error type.

### 15.2 Feature completion

Constraint annotations (`@min`, `@max`, `@pattern`, `@email`) compiled into each backend's validation layer; WebSocket handler bodies brought to parity across all eight backends; query and header parameter support completed in Go, Rust, Kotlin, and Swift; `@serverSet` enforced everywhere; enum and constant coverage in the breaking-change diff; and import resolution in the LSP so editor diagnostics match the compiler.

### 15.3 Platform extensions

Additional targets (Elixir/Phoenix, Ruby/Rails, Kotlin as a *backend* via Ktor or Spring, SolidJS); bidirectional OpenAPI so an existing specification can be imported into a `.veld` contract, lowering adoption cost for teams with established APIs; and semantic-version inference deriving the correct version bump from the breaking-change classification the diff already computes.

### 15.4 Operational maturity

Versioned schema migrations for the registry (currently a single hand-written `CREATE TABLE IF NOT EXISTS` block with no rollback path); rate limiting on authentication endpoints; object-storage backend behind the existing `storage.Backend` interface; working checksums for the Homebrew and Chocolatey manifests and a real download mechanism for the pip package; and repository hygiene — removing development artefacts, a committed binary, and several `test-*` directories from the tree before submission.

---

## Appendix A — Repository Map

| Path | Contents |
|---|---|
| `cmd/veld/` | CLI entry point; all commands; workspace orchestration |
| `internal/lexer/` | Tokenizer with error recovery |
| `internal/parser/` | Recursive-descent parser |
| `internal/ast/` | AST type definitions |
| `internal/loader/` | Recursive imports, aliases, globs, registry packages |
| `internal/validator/` | Semantic validation; workspace `consumes` validation |
| `internal/emitter/` | Registry, role interfaces, shared helpers (`lang/`, `tshelpers/`, `tsshared/`, `sdkhelpers/`, `codegen/`) |
| `internal/emitter/backend/` | Eight backend emitters, each with a `strategy/` subpackage |
| `internal/emitter/frontend/` | Ten frontend emitters |
| `internal/generators/` | Six auxiliary generators |
| `internal/server/` | Registry server: auth, TOTP, packages, orgs, storage, email, embedded SPA |
| `internal/registry/` | Registry client: credentials, HTTP client, tarball pack/unpack/verify |
| `internal/diff/` | Breaking-change detection and `.veld.lock.json` |
| `internal/cache/` | SHA-256 content-hash incremental build cache |
| `internal/lsp/` | Language Server Protocol implementation |
| `internal/lint/`, `internal/format/` | Linter and formatter |
| `internal/docsgen/`, `internal/graphqlgen/`, `internal/openapigen/`, `internal/schema/` | Export generators |
| `internal/mock/` | Mock server *(built; not CLI-wired — §11.1)* |
| `internal/errors/` | Structured error types *(built; not adopted — §11.1)* |
| `editors/vscode/`, `editors/jetbrains/` | Editor plugins |
| `examples/` | Ten worked examples |
| `.github/workflows/` | CI: build, vet, race tests, five-platform cross-compilation |

## Appendix B — Dependencies

**Compiler and registry (direct):**

| Module | Purpose |
|---|---|
| `github.com/spf13/cobra` | CLI command framework |
| `github.com/lib/pq` | PostgreSQL driver (registry server) |
| `golang.org/x/crypto` | bcrypt password hashing |

Everything else — HTTP serving and routing, JWT signing, TOTP, tarball handling, JSON, hashing — is implemented against the Go standard library.

**Generated output:** none by default. Zod (Node) and Pydantic (Python) are required only when runtime validation is enabled; emitted types, interfaces, routes, and SDKs function with no package installation.

## Appendix C — Feature Support Matrix

Current per-target coverage. ✔ correct · ~ degraded or partial · ✗ missing or broken. Discrepancies are itemised in §12.

| Backend | Query | Headers | `@serverSet` | `@deprecated` | WS body | `--validate` | `extends` in SDK |
|---|---|---|---|---|---|---|---|
| node-ts | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ |
| node-js | ✔ | ✔ | ✗ | ~ | ✗ | ✔ | ~ |
| python | ✔ | ✔ | ✗ | ✔ | ✔ | ✔ | ✔ |
| go | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✔ |
| rust | ✗ | ~ | ✗ | ✗ | ✗ | ✗ | ✗ |
| java | ✔ | ~ | ✗ | ✗ | ~ | ✗ | ✔ |
| csharp | ~ | ~ | ✗ | ✗ | ✗ | ✗ | ✗ |
| php | ~ | ✔ | ✗ | ✗ | ✗ | ✗ | ✗ |

| Frontend | Reuses TS emitter | WS support | Query | Path params | Runtime base URL |
|---|---|---|---|---|---|
| typescript | — (is the source) | ✔ | ✔ | ✔ | ✔ |
| react / vue / svelte | ✔ | ✗ | ✔ | ✔ | ✔ |
| angular | ✗ reimplements | ✗ | ✔ | ✔ | ✗ |
| javascript | ✗ independent | ✗ | ~ | ✔ | ~ |
| dart | ✗ independent | ✔ | ✔ | ✔ | ~ |
| kotlin | ✗ independent | ~ | ✗ | ✔ | ✔ |
| swift | ✗ independent | ~ | ✗ | ✗ | ✔ |
| types-only | ~ helpers only | n/a | n/a | n/a | n/a |
