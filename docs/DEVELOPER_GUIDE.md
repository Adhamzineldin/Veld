# Veld — Developer Guide

This is the deep-dive onboarding doc for someone who is going to **write Go code in this
repo**, not just use the `veld` CLI. It walks the whole pipeline, file by file, with real
snippets pulled from the current source so nothing here is aspirational.

Read `CLAUDE.md` first — it's the source of truth for the non-negotiable rules. This doc
explains *how* the code implements those rules.

---

## 1. Mental model

Veld is a **compiler**. That's the right frame, not "a scaffolding tool."

```
.veld source files  →  Lexer  →  Parser  →  AST  →  Loader (imports)  →  Validator  →  Emitters  →  generated/
```

- The **front end** (lexer, parser, loader, validator) turns text into one validated `ast.AST` struct.
- The **back end** (emitters) turns that same `ast.AST` into files, once per target language/framework.
- Front end and back end **never share code**. An emitter only ever sees `ast.AST` — never a token, never a parser type. This is enforced by convention (checked in review), not by the Go compiler, so don't be the one who breaks it.
- Everything is **pure and deterministic**: `Emit(ast, outDir, opts) error`. No hidden global state, no network calls, no randomness. Same AST + same opts = byte-identical output, every time.

If you're adding a feature, figure out which stage it belongs to first — that tells you which package to touch and, just as importantly, which packages you must *not* touch.

---

## 2. Repo layout

```
cmd/veld/main.go        single CLI binary — all 26+ subcommands, Cobra wiring (4600+ lines)
cmd/generate-language/  codegen-the-codegen: generates internal/language/* from a spec
internal/ast/           the shared AST types — the contract between front end and back end
internal/lexer/         tokenizer
internal/parser/        recursive-descent parser
internal/loader/        resolves `import` statements across files
internal/validator/     semantic checks (circular inheritance, unknown types, workspace consumes)
internal/config/        veld.config.json loading + flag merging + alias resolution
internal/cache/         file-mtime tracking for incremental `veld generate` / `veld watch`
internal/diff/          breaking-change detection + .veld.lock.json
internal/lint/          contract quality rules (`veld lint`)
internal/format/        `.veld` file formatter (`veld fmt`)
internal/lsp/           Language Server Protocol implementation (stdin/stdout)
internal/emitter/       the plugin registry + all backend/frontend emitters (see §5)
internal/generators/    "tool" emitters: cicd, dockerfile, database, envconfig, openapi, scaffold
internal/docsgen/       `veld docs` API documentation generator
internal/graphqlgen/    `veld graphql` GraphQL SDL export
internal/openapigen/    `veld openapi` OpenAPI 3.0 export
internal/schema/        `veld schema` SQL schema export
internal/setup/         `veld setup` — tsconfig/paths auto-configuration
internal/registry/      client side of the package registry (login, push, pull)
internal/server/        server side of the package registry (`veld serve`) — auth, db, handlers, embedded SPA
internal/mock/          mock-data helpers
internal/errors/        shared error types/helpers
internal/language/      generated per-language metadata (naming conventions, type maps) — see §5.4
editors/                VS Code extension + JetBrains plugin (share editors/veld-config.schema.json)
examples/                nine full worked examples, one per stack pairing (node-react, python-vue, go-svelte, …)
packages/                distribution packaging: npm, pip, homebrew, chocolatey, apt, nuget, composer, go
website/                 marketing/docs site (separate Node project)
docs/                    architecture notes, guides, roadmap (this file included)
```

The four directories at repo root named `test-*` (`test-gen`, `test-gen2`, `test-headers`,
`test-constants`) are throwaway scratch projects used to manually exercise the CLI — not part
of the Go test suite. Treat them as disposable; don't build features that depend on their
contents.

---

## 3. Build & run

```bash
go build -o veld.exe ./cmd/veld    # Windows
go build -o veld ./cmd/veld        # Unix
go build ./...                     # compile everything, catch cross-package breaks
go test ./...                      # unit tests — every internal/ package has *_test.go
go vet ./...                       # static analysis, run before every commit
```

There is exactly **one binary**. `cmd/veld/main.go` is the composition root: it blank-imports
every emitter and tool package so their `init()` functions self-register into
`internal/emitter`'s registry (see §5.1). If you write a new emitter package and forget to
add its blank import in `main.go`, it will compile fine and then silently not exist as a CLI
target — that's the #1 mistake new contributors make here.

`go.mod` module path: `github.com/Adhamzineldin/Veld`. External deps are deliberately kept to
almost nothing: `cobra` (CLI), `golang.org/x/crypto` (bcrypt, registry server only),
`modernc.org/sqlite` (pure-Go SQLite fallback). The lexer and parser are 100% hand-written —
no parser-generator, no external tokenizer.

---

## 4. The front end, stage by stage

### 4.1 Lexer — `internal/lexer/lexer.go` (~450 lines)

Turns `.veld` source text into a token stream. Nothing fancy — it's a hand-rolled scanner
that understands identifiers, keywords (`model`, `module`, `action`, `enum`, `extends`,
`import`, HTTP method names…), string/number literals, and the punctuation Veld needs:
`{ } : ; , ? [ ] < >`. The `<` `>` `,` handling exists specifically so `Map<string, int>`
tokenizes as generics rather than comparison operators — that's the one lexer quirk worth
knowing about if you extend the type grammar.

### 4.2 Parser — `internal/parser/parser.go` (~1150 lines)

Recursive descent, one function per grammar production (`parseModel`, `parseField`,
`parseAction`, `parseModule`, `parseEnum`, `parseConstantGroup`…). Every AST node it produces
carries a `Line int` field captured from the token stream — this is what lets `veld validate`
and `veld generate` print `file:line` with a source snippet on errors. If you add a new field
to an AST node, thread the line number through from day one; retrofitting it later is
annoying.

Grammar highlights the parser handles that aren't obvious from the syntax examples:
- `model Child extends Parent { ... }` — inheritance
- `field?: type` — optional marker
- `field: Type[]` — arrays
- `field: Map<K, V>` — maps (K is always validated as `string` downstream)
- `field: Type @default(x) @deprecated("msg") @unique @index @relation(Other) @serverSet`
- inline anonymous input/output/query/header shapes directly inside an `action` block
  (these populate `InputFields`/`OutputFields`/etc. on `ast.Action` instead of a named model)
- `output: Model:201` — explicit status code suffix
- `output: Model[]:200` — array + status code combined

### 4.3 AST — `internal/ast/ast.go` (~105 lines)

This is the single most important file to read before touching anything else — it's the
contract every other package agrees on. Full current shape:

```go
type AST struct {
    ASTVersion  string
    Prefix      string              // app-level route prefix, prepended to all modules
    Models      []Model
    Modules     []Module
    Enums       []Enum
    Constants   []ConstantGroup
    Imports     []string            // not serialized — loader bookkeeping only
    FileImports map[string][]string // not serialized — sourceFile → its direct imports
}

type Model struct {
    Name, Description, Extends string
    Fields                     []Field
    SourceFile                 string // absolute path, not serialized
    Line                       int    // not serialized
}

type Field struct {
    Name, Type                     string
    UnionTypes                     []string // inline union e.g. "DRAFT"|"PENDING"
    Optional, IsArray, IsMap       bool
    MapValueType                   string
    Default, Deprecated, Example   string
    Unique, Index, ServerSet       bool
    Relation                       string
}

type Action struct {
    Name, Description, Method, Path string
    Input                            string  // named model, or "" if InputFields is used inline
    InputFields                      []Field // non-empty ⇒ input was declared inline
    Output                           string
    OutputArray                      bool
    OutputFields                     []Field
    Query, Headers                   string
    QueryFields, HeaderFields        []Field
    Stream, Emit                     string  // WebSocket: server→client / client→server payload types
    Errors                           []string
    ErrorStatuses                    map[string]int
    SuccessStatus                    int     // 0 = emitter picks the default (201/204/200)
    Middleware                       []string
    Deprecated                       string
}
```

`Model`/`Module`/`Enum`/`ConstantGroup` all carry `SourceFile`/`Line` — these are excluded
from JSON serialization (`json:"-"`) because `veld ast` output should be portable, but
they're what powers precise diagnostics internally.

### 4.4 Loader — `internal/loader/loader.go` (~400 lines)

Walks `import` statements starting from the entry file (`app.veld` by convention), resolving
both styles:

```veld
import @models/user          // alias — looked up in config.aliases, defaults to project root
import "./models/user.veld"  // relative to the *current file's* directory, not cwd
import @models/*              // wildcard — every .veld file directly in that alias dir
```

It merges every file's models/modules/enums/constants into one `ast.AST`, tracks a
circular-import guard, and records `FileImports` (used by `veld doctor` and tooling to draw
the dependency graph). This is also where `Model.SourceFile` / `Module.SourceFile` get
stamped, since the parser itself is single-file and doesn't know its own path in context.

### 4.5 Validator — `internal/validator/validator.go` (~580 lines)

Semantic pass over the merged AST — nothing here can be checked syntactically:
- circular `extends` chains
- referencing an undefined model/enum type in a field
- `Map<K, V>` where `K` isn't `string`
- duplicate model/module/action names
- workspace `consumes` issues (circular service deps, referencing an unknown workspace entry, a service consuming itself) — this half lives in `internal/validator/workspace.go`

Every error returned is a Go `error` whose message already contains `file:line` plus, where
possible, the offending source line — `cmd/veld/main.go`'s `printValidationErrors` just
prints them, it doesn't reformat.

---

## 5. The back end: emitters

### 5.1 The plugin contract — `internal/emitter/emitter.go`

Three interfaces, all embedding the base `Emitter`:

```go
type Emitter interface {
    Emit(a ast.AST, outDir string, opts EmitOptions) error
}

type BackendEmitter interface {
    Emitter
    Summarizer
    IsBackend() // no-op marker method, just for type-switching
    EmitServiceSdk(consumed []ConsumedServiceInfo, outDir string, opts EmitOptions) error
}

type FrontendEmitter interface { Emitter; Summarizer; IsFrontend() }
type ToolEmitter      interface { Emitter; Summarizer; IsTool() }
```

`EmitServiceSdk` is **required** on every backend — the interface won't compile without it —
because inter-service SDK generation (§6) must work for every language from day one, not be
bolted on later.

Registration is a global map behind a mutex, populated entirely by `init()` side effects:

```go
func init() { emitter.RegisterBackend("node-ts", New()) }
```

`cmd/veld/main.go` blank-imports every emitter package purely to trigger these `init()`
calls — that's the entire wiring mechanism. `GetBackend`, `GetFrontend`, `GetTool`,
`GetBackendOrTool` are how the CLI looks a name up at generate time.

`EmitOptions` (also in `emitter.go`) is what an emitter is allowed to know about the outside
world — `BaseUrl`, `DryRun`, `Validate`, `BackendFramework`, `FrontendFramework`, per-module
`Services` overrides, `ConsumedServices`. **No emitter imports `internal/config` directly** —
`cmd/veld/main.go` translates `ResolvedConfig` into `EmitOptions` before calling `Emit`. This
is deliberate decoupling: emitters must be testable by constructing an `ast.AST` and
`EmitOptions` by hand, no config file needed (see `node_test.go` for the pattern).

### 5.2 Anatomy of one emitter — `internal/emitter/backend/node/`

Use this package as the reference implementation when writing a new one. File-by-file:

| File | Responsibility |
|---|---|
| `main.go` | `init()` registration, `Emit()` orchestration (calls the others in order), `Summary()` |
| `types.go` | per-module TypeScript `interface`/`type` generation |
| `interfaces.go` | `I<Module>Service.ts` — the service contract the developer implements |
| `routes.go` | route handler files: try/catch, status-code logic, calls into the strategy |
| `validate.go` | Zod schema generation, gated behind `opts.Validate` |
| `constants.go` | emits `ConstantGroup`s as typed constants |
| `errors.go` | typed error classes from `Action.Errors`/`ErrorStatuses` |
| `barrel.go` | `index.ts` re-export barrel |
| `sdk.go` | `EmitServiceSdk` — the inter-service HTTP client (§6) |
| `strategy/` | framework variance (Express vs. plain) — see below |

`Emit()` itself is short and reads like a checklist — this is the shape every emitter should
converge on:

```go
func (e *NodeEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
    if opts.DryRun { return nil }
    strat := nodestrategy.New(opts.BackendFramework)
    e.emitPerModuleTypes(a, outDir)
    e.emitAllErrors(a, outDir)
    for _, mod := range a.Modules {
        e.emitInterface(a, mod, outDir)
        e.emitRoutes(a, mod, outDir, opts, strat)
    }
    e.emitMiddlewareInterface(a, outDir, strat)
    if opts.Validate { e.emitValidators(a, outDir) }
    e.emitBarrel(a, outDir)
    e.emitConstants(a, outDir)
    return nil
}
```

**The strategy pattern for framework variants** (`strategy/strategy.go`): a backend target
(`node-ts`) is one Go package, but it can still vary its *output shape* per framework
(`--backend-framework=express` vs. plain) without an if/else scattered through every file.
The interface is small and closed:

```go
type NodeFrameworkStrategy interface {
    RouterType() string          // TS type for the router param
    RequestType() string
    ResponseType() string
    ExtraImports() []string
    PackageDependencies() map[string]string
    WSImports() string
    WSRouterParam() string
}
```

`strategy.New("express")` returns `&ExpressStrategy{}`, anything else returns
`&PlainStrategy{}`. Adding a Fastify variant later is: implement the interface in a new
`fastify.go`, add a case to `New()`. Nothing else in `routes.go` changes.

### 5.3 Shared helpers — don't duplicate these per-emitter

- `internal/emitter/helpers.go` — `CollectTransitiveModels` (walk a model's field graph to
  find every model that needs importing), `ExtractPathParams` (`/users/:id` → `["id"]`),
  `ToFlaskPath`, `ToOpenAPIPath` (path-param syntax conversion between conventions).
- `internal/emitter/tshelpers/` — `VeldFieldToTS`, `FormatOutputType` — used by every
  TS-producing emitter (node backend, all ~10 frontend emitters) so the veld-type → TS-type
  mapping is defined exactly once.
- `internal/emitter/sdkhelpers/` — `EnvVarName`, `ServiceClassName`, `ServiceFileName` — used
  by every backend's `sdk.go` so the `VELD_<NAME>_URL` naming convention can't drift between
  languages.

If you find yourself reimplementing type-name mapping or path-param extraction inside a new
emitter, stop — it almost certainly belongs in one of these three packages instead.

### 5.4 Language metadata — `internal/language/`

`constants.go`, `imports.go`, `veld.go` hold per-target-language naming conventions (case
style, file extensions, import syntax) as data rather than scattered string literals. These
are generated by `cmd/generate-language/main.go` from a spec — if you need to adjust a
language's conventions in bulk, look at whether that generator should own the change instead
of hand-editing the generated file.

### 5.5 "Tool" emitters — `internal/generators/`

CI/CD workflows, Dockerfiles, `.env` templates, OpenAPI export, DB schema, and project
scaffolding are `ToolEmitter`s, not `BackendEmitter`s — they don't require `EmitServiceSdk`
and aren't offered as `--backend=` service targets in the same sense. `GetBackendOrTool` in
`emitter.go` is why `--backend=dockerfile` still resolves: the CLI tries the backend registry
first, falls back to the tool registry.

---

## 6. Multi-service workspaces & inter-service SDKs

A `veld.config.json` can declare a `workspace: [...]` array instead of a single project. Each
entry is its own mini-project (own `input`, own `backendConfig`/`frontendConfig`, own
`baseUrl`) and can declare `consumes: ["otherEntryName"]`.

When entry A consumes entry B, `runWorkspaceGenerate` in `cmd/veld/main.go` (§7) loads B's
full AST, wraps it in a `ConsumedServiceInfo{Name, AST, BaseUrl}`, and passes it to A's
emitter via `opts.ConsumedServices`. A's `EmitServiceSdk` then writes a **typed HTTP client
for B, in A's own language**, into `A_outdir/sdk/b/`. Rules that apply uniformly across every
language's `sdk.go`:

- stdlib HTTP only (`fetch`, `urllib`, `net/http`, `reqwest`, `HttpClient`, `cURL`) — zero
  new runtime deps introduced by consuming a service
- each `sdk/<service>/` is self-contained — no imports from the primary generated output or
  from other consumed SDKs
- Go SDKs flatten model inheritance (Go has no struct inheritance to map `extends` onto)
- WebSocket actions (`stream`/`emit`) are skipped — service SDKs are HTTP-only
- base URL resolves constructor-arg → `VELD_<UPPER_SNAKE_NAME>_URL` env var → B's configured
  `baseUrl` → error, in that order, implemented identically in every language

`veld deps` / `veld deps --validate` walk the same `consumes` graph for visualization and
cycle detection (backed by `internal/validator/workspace.go`).

---

## 7. `cmd/veld/main.go` — how the CLI ties it together

It's one file, ~4600 lines, organized as: constants/color helpers → a handful of shared
`run*` functions → one `new*Cmd()` function per subcommand → `main()` wiring them onto the
Cobra root. Skimming the `new*Cmd` function names tells you the full command surface:
`newValidateCmd`, `newASTCmd`, `newGenerateCmd`, `newWatchCmd`, `newCleanCmd`, `newLintCmd`,
`newOpenAPICmd`, `newGraphQLCmd`, `newSchemaCmd`, `newDepsCmd`, `newDiffCmd`, `newDocsCmd`,
`newLSPCmd`, `newFmtCmd`, `newDoctorCmd`, `newCompletionCmd`, `newInitCmd`, `newSetupCmd`,
`newLoginCmd`, `newLogoutCmd`, `newPushCmd`, `newPullCmd`, `newExportCmd`, `newAgentsCmd`,
`newRegistryCmd` (this last one hosts `veld serve`).

The functions worth knowing if you're changing generate behavior:

- **`runGenerate`** (single-project path) and **`runWorkspaceGenerate`** (multi-service path,
  §6) — both end up calling **`runGenerateWithAST`**, which is the actual
  `backend.Emit(...)` / `frontend.Emit(...)` / tool `Emit(...)` fan-out. If your change needs
  to affect *every* generation path, this is the choke point.
- **`computePreChanges`** + `internal/diff` — runs before every real `generate`, comparing
  against `.veld.lock.json`; feeds the interactive breaking-change prompt (or `--strict`
  hard-fail).
- Config resolution is **not** Cobra-aware: `config.FlagOverrides` is a plain struct built
  from flag values, then handed to `config.BuildResolved`. This is deliberate — it's what
  lets `internal/config` be unit-tested without spinning up Cobra, and it's why you should
  never see a `*cobra.Command` passed into `internal/config` or any emitter.

---

## 8. Config system — `internal/config/config.go`

Two on-disk shapes are accepted and normalized into one internal shape:

- **Nested** (recommended): `backendConfig.{target,framework,out,dir,validate}`,
  `frontendConfig.{target,out,dir}`, `hooks.postGenerate`, `tools`, `workspace`.
- **Flat** (legacy): `backend`, `backendFramework`, `backendOut`, `backendDir`, `validate`,
  `frontend`, `frontendOut`, `frontendDir`, `postGenerate`.

`RawConfig.normalize()` reconciles both into the fields the rest of the code reads; nested
wins if both are present in the same file. `BuildResolved(FlagOverrides) (ResolvedConfig, error)`
is the single entry point — it applies precedence (CLI flags > config file > built-in
defaults), resolves aliases (`DefaultAliases()`: `models`, `modules`, `types`, `enums`,
`schemas`, `services`, `lib`, `common`, `shared`, plus any user-defined `aliases` in the
config), and resolves relative output/project dirs against the config file's own location —
not the process cwd, which matters when `veld` is invoked from a subdirectory.

`ResolvedConfig.SplitOutput()` / `OutputDirs()` handle the case where backend and frontend
write to different directories (`backendConfig.out` != `frontendConfig.out`) vs. one shared
`out`.

---

## 9. Supporting systems

- **Cache** (`internal/cache/cache.go`) — tracks file mtimes so `veld generate` (incremental
  mode) and `veld watch` (500ms debounce) can skip unchanged files. Cache file:
  `veld/.veld-cache.json`.
- **Diff / lock** (`internal/diff/`) — `Diff(old, new ast.AST) []Change`, `HasBreaking()`;
  `.veld.lock.json` is the snapshot of the last successful generation, read/written by
  `lock.go`.
- **Lint** (`internal/lint/lint.go`) — rules: `unused-model`, `empty-module`, `empty-model`,
  `duplicate-route` (error), `duplicate-action` (error), `missing-description`,
  `deprecated-action`, `deprecated-field`. `veld lint --exit-code` for CI gating.
- **Format** (`internal/format/`) — canonical `.veld` pretty-printer, `veld fmt [files...]`.
- **LSP** (`internal/lsp/`) — stdin/stdout Language Server Protocol implementation; backs the
  VS Code and JetBrains editor plugins in `editors/`.
- **Registry client vs. server** — `internal/registry/` is the *client* side
  (`credentials.go`, `client.go`, `tarball.go` for pack/unpack/verify) used by
  `veld login`/`push`/`pull`. `internal/server/` is the *server* you run yourself via
  `veld serve` — PostgreSQL-backed, JWT auth (`auth/token.go`, hand-rolled HMAC-SHA256, no
  external JWT lib), TOTP 2FA, org/package CRUD, SMTP email, and an embedded SPA
  (`handlers/web.go` via `//go:embed web`). These two never import each other.

---

## 10. Adding a new backend emitter — worked checklist

1. `mkdir internal/emitter/backend/<name>`
2. Implement `Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error` plus
   `Summary([]string) []emitter.SummaryLine`, `IsBackend()`, and
   `EmitServiceSdk(consumed []emitter.ConsumedServiceInfo, outDir string, opts emitter.EmitOptions) error`
   — the interface won't let you skip the last one.
3. Register in an `init()`: `emitter.RegisterBackend("<name>", New())`.
4. Blank-import the package in `cmd/veld/main.go`'s import block (alphabetical, next to the
   other backends).
5. Reuse `internal/emitter/helpers.go`, `tshelpers`, `sdkhelpers` before writing your own
   type-mapping or path-param logic.
6. Zero new runtime deps in the generated *type/interface/route* files — validation-only
   packages (a Pydantic/Zod equivalent) are the sole sanctioned exception, and only the
   schema files may depend on them.
7. Add a `<name>_test.go` following `node_test.go`'s pattern: build an `ast.AST` by hand (no
   config, no filesystem), call `Emit` into a temp dir, assert on file contents.
8. Add or extend an example under `examples/` if the target pairs with a new frontend, so
   there's an end-to-end smoke test a human can `veld generate` and actually run.

Adding a new **frontend** emitter is symmetric, just implement `FrontendEmitter` instead
(no `EmitServiceSdk` requirement) and register with `emitter.RegisterFrontend`.

---

## 11. Where things live if you're debugging a specific symptom

| Symptom | Look here |
|---|---|
| Wrong/missing token, weird syntax error | `internal/lexer/lexer.go` |
| Parser rejects valid syntax / accepts invalid syntax | `internal/parser/parser.go` |
| `import @alias/x` doesn't resolve | `internal/loader/loader.go`, `config.DefaultAliases` |
| Validator false positive/negative | `internal/validator/validator.go` (or `workspace.go` for `consumes`) |
| Generated file has wrong type mapping | `internal/emitter/tshelpers/` or the target's own `types.go` |
| Generated file has a stray import / runtime dep | the specific emitter's `Emit()` — check it isn't leaking a framework import outside the `strategy`/opt-in path |
| `--backend=X` says "unknown backend" | missing blank import in `cmd/veld/main.go`, or typo in `RegisterBackend("X", ...)` |
| Service SDK missing/wrong base URL logic | `internal/emitter/sdkhelpers/`, the target's `sdk.go` |
| Config field ignored | `internal/config/config.go` — check both `normalize()` (flat/nested merge) and `BuildResolved` (precedence) |
| Breaking-change prompt wrong/missing | `internal/diff/diff.go`, `.veld.lock.json` handling in `lock.go` |
| `veld generate` picks up stale output | `internal/cache/cache.go` — delete `veld/.veld-cache.json` to force a clean run while debugging |

---

## 12. Non-negotiables (repeated from CLAUDE.md, because it bears repeating)

1. **Zero runtime deps** in generated types/interfaces/routes, always. Validation schemas are
   the one opt-in exception.
2. **Agnostic and dynamic** — `router: any`, native `fetch`, no framework lock-in baked into
   the primary output.
3. **`--backend=node`, not `--backend=express`** — the flag names the *pattern*
   (Node HTTP router), not a specific framework; frameworks are a `strategy` detail.

Everything else in this codebase — the plugin registry, the `EmitOptions` boundary, the
strategy pattern, the shared helper packages — exists in service of keeping those three rules
true across eight backends and ten frontends without them drifting apart.
