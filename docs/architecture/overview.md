# Veld Architecture

## Pipeline

```
.veld files → Lexer → Parser → AST → Import Resolver → Validator → Emitter(s) → generated/
```

Each stage is isolated. Only the AST struct passes between stages.

## Package Map

| Package | Path | Role |
|---------|------|------|
| CLI | `cmd/veld/main.go` | Cobra commands, config resolution, import loading |
| AST | `internal/ast/ast.go` | Shared data structures (Model, Field, Module, Action, Enum) |
| Lexer | `internal/lexer/lexer.go` | Tokenizes `.veld` source (supports `<>`, `,` for generics) |
| Parser | `internal/parser/parser.go` | Recursive descent; `extends`, `Map<K,V>`, `@default`, arrays |
| Validator | `internal/validator/validator.go` | Semantic checks: circular inheritance, undefined types, `file:line` errors |
| Config | `internal/config/config.go` | Config loading, flag merging, alias resolution |
| Loader | `internal/loader/loader.go` | Loads `.veld` files, resolves imports (`@alias` + relative), circular guard |
| Cache | `internal/cache/cache.go` | File mtime tracking for incremental builds |
| Emitter registry | `internal/emitter/emitter.go` | `BackendEmitter`/`FrontendEmitter` interfaces + `init()`-based registry |
| Emitter helpers | `internal/emitter/helpers.go` | `CollectTransitiveModels`, `ExtractPathParams`, `ToOpenAPIPath` |
| Language adapters | `internal/emitter/lang/` | Per-language type mapping, naming conventions |
| Code generation | `internal/emitter/codegen/` | Shared writer, formatter, import manager |

## Backend Emitters

All self-register via `init()`. Adding a new emitter = new package + one blank import in `main.go`.

| Emitter | Package | Output |
|---------|---------|--------|
| Node.js | `internal/emitter/backend/node/` | TS types, Zod schemas, route handlers, interfaces |
| Python | `internal/emitter/backend/python/` | TypedDict, Pydantic schemas, Flask routes, ABC interfaces |
| Go | `internal/emitter/backend/go/` | Structs, HTTP handlers, middleware |
| Rust | `internal/emitter/backend/rust/` | Structs, Actix handlers |
| Java | `internal/emitter/backend/java/` | Records, Spring controllers |
| C# | `internal/emitter/backend/csharp/` | Records, ASP.NET controllers |
| PHP | `internal/emitter/backend/php/` | Classes, Laravel routes |

## Frontend Emitter

| Emitter | Package | Output |
|---------|---------|--------|
| TypeScript | `internal/emitter/frontend/typescript/` | Fetch-based SDK with `VeldApiError`, path params |

## Import System

Two styles, both fully supported:

```
import @models/user           → alias-based (resolved from project root via config aliases)
import "./models/user.veld"   → relative path (resolved from current file's directory)
import @models/*              → wildcard (all .veld files in the alias directory)
```

Aliases are defined in `veld.config.json` → `aliases` field. Defaults: `models`, `modules`, `types`, `enums`, `schemas`, `services`, `lib`, `common`, `shared`.

## Key Rules

1. Parser and emitters are completely independent — no emitter imports lexer/parser
2. Emitters receive `EmitOptions` (BaseUrl, DryRun) — no direct config dependency
3. Config resolution is decoupled from Cobra (uses `FlagOverrides` struct)
4. Generated output is deterministic — same input always produces identical output
5. Veld never writes outside the `--out` directory

## Go Module

```
github.com/veld-dev/veld
```

Only external dependency: `cobra` (CLI framework). Lexer and parser are hand-written.

