# 07. Target architecture, repository layout, interfaces and ADRs

Decision records: ADR-001 to ADR-010 in `adr/`. Diagrams: `diagrams/D1-system-context.mmd`, `D2-compiler-pipeline.mmd`, `D3-plugin-trust.mmd`. Everything marked PROPOSED is design, not implementation.

## 7.1 Boundaries

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

## 7.2 AST versus IR, and where serialization happens

- The AST is syntax: it keeps declaration order, comments (for the formatter, ADR-002 and the formatter rewrite in chapter 08), spans, and unresolved names.
- The IR is semantics: every reference resolved, inheritance flattened with provenance, types as a small recursive `TypeRef` tree, routes with parsed path segments, status codes resolved, defaults typed, deprecations attached, source map to spans.
- Serialization happens only at process and public boundaries: `veld ir` (JSON), the plugin request, conformance fixtures (expected IR JSON), and the LSP wire protocol. Between lexer, parser, semantic analysis, built-in emitters and the LSP, Go values are passed directly. The current "only AST JSON passes between stages" rule in CLAUDE.md is replaced by "only IR crosses a process boundary".

## 7.3 Proposed directory tree

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

## 7.4 Key interfaces (proposed Go signatures)

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

## 7.5 Deployables, processes and products

| Product | Process | Versioned independently | Repository |
|---|---|---|---|
| `veld` CLI (includes `veld lsp`) | one process; spawns plugin processes and `postGenerate` hooks | yes (`vX.Y.Z`) | monorepo |
| `veld-registry` | one process plus PostgreSQL; packaged as Docker image | yes (`registry/vX.Y.Z`) | monorepo |
| `@veld/emitter-sdk` | library | yes (`sdk/vX.Y.Z`) | monorepo |
| VS Code extension | runs inside VS Code; spawns `veld lsp` | yes | monorepo |
| JetBrains plugin | runs inside the IDE; LSP4IJ spawns `veld lsp` | yes | monorepo |
| Website | static site | not versioned | monorepo |

Modular monolith for the compiler (one binary, strict internal boundaries enforced by tests), separate deployable for the registry, one repository (ADR-010). More separation than this would add release coordination without a corresponding benefit for a six-person team.

## 7.6 How a contributor adds an emitter or an editor integration without touching compiler semantics

- External emitter: follow chapter 10 §10.4; no change to the Veld repository at all.
- Built-in emitter (tier 2 proposal): add `internal/emit/builtin/<name>/` implementing `emit.Emitter`, register it in `internal/emit/builtin/registry.go` (one line), add golden outputs for every conformance fixture, add a compile step to the nightly job. The import-boundary test fails if the new package imports `syntax` or `semantic`.
- Editor integration: configure the editor's LSP client to run `veld lsp`; nothing in the compiler changes. Syntax highlighting grammars are generated from `internal/language` by `cmd/generate-language`, so a new editor adds one template there.

## 7.7 Versioning and compatibility

See ADR-009 for the full table. In one sentence: the language, compiler, IR, plugin protocol, registry API, package manifest, lockfile and generated artifacts each carry their own version field; the CLI is the single place that negotiates plugin compatibility, and the server is the single place that validates manifest versions.

## 7.8 ADR index

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
