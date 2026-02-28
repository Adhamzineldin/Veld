# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## NON-NEGOTIABLE RULES

> These apply to every line of generated code, every emitter, every future feature.

1. **ZERO RUNTIME DEPENDENCIES in generated output** (unless explicitly opted in). Generated files must never `import` or `require` any external package by default. Zod schemas are **opt-in** via `"schemas": true` in config or `--schemas` flag. The generated folder must be installable with zero `npm install`.

2. **Always agnostic, always dynamic.** Generated code must work with any compatible framework, not one specific library. Route handlers accept `router: any` — the user wires in their own Express/Fastify/Hono/whatever. The SDK uses native `fetch`, not axios. No lock-in, ever.

3. **`--backend=node`, not `--backend=express`.** The backend emitter targets the Node.js HTTP router pattern (`.get`, `.post`, etc.), not a specific framework. The flag name reflects that.

---

## Project Overview

**Veld** is a contract-first, multi-stack API code generator written in Go. A developer writes `.veld` contract files; Veld generates a typed frontend SDK (TypeScript) and backend service interfaces + route wiring. Veld never touches developer business logic files.

The full specification lives in `veld-claude-code-prompt (2).md`. The visual blueprint is in `ross-framework-plan (1).html`.

## Build & Test Commands

```bash
go build -o veld.exe ./cmd/veld    # Build CLI binary (Windows)
go build -o veld ./cmd/veld        # Build CLI binary (Unix)
go build ./...                     # Build all packages (verify no errors)
go test ./...                      # Run all tests (~60 tests across 6 packages)
go vet ./...                       # Static analysis
```

Once the binary is built:
```bash
veld --version                     # Print version
veld init                          # Scaffold new project (creates veld/ folder)
veld validate                      # Validate contract (reads veld/veld.config.json)
veld ast                           # Dump AST JSON
veld generate                      # Generate (auto-detects veld/veld.config.json)
veld generate --schemas            # Generate with Zod validation schemas
veld generate --dry-run            # Preview without writing files
veld generate --backend=node --frontend=typescript --input=veld/app.veld --out=./generated
veld watch                         # Auto-regenerate on file changes
```

## Architecture

The pipeline is strictly linear — **only AST JSON passes between stages**:

```
.veld files → Lexer → Parser → AST → import resolver → Validator → Emitter(s) → generated/
```

| Package | Path | Role |
|---------|------|------|
| AST types | `internal/ast/ast.go` | Shared data structures with `Line` tracking; no logic |
| Lexer | `internal/lexer/lexer.go` | Tokenizes `.veld` source text |
| Parser | `internal/parser/parser.go` | Recursive descent; produces AST with line numbers |
| Validator | `internal/validator/validator.go` | Semantic checks on AST with `file:line` error context |
| Config | `internal/config/config.go` | Config file loading, flag merging, path resolution |
| Loader | `internal/loader/loader.go` | Loads .veld files, resolves imports recursively |
| Emitter registry | `internal/emitter/emitter.go` | `BackendEmitter` / `FrontendEmitter` interfaces + `init()`-based registry |
| Emitter helpers | `internal/emitter/helpers.go` | Shared functions: `CollectTransitiveModels`, `CollectUsedTypes`, etc. |
| TS helpers | `internal/emitter/tshelpers/` | Shared TypeScript type-mapping (`VeldTypeToTS`, `FormatOutputType`) |
| Node emitter | `internal/emitter/backend/node/` | Backend: TS types + interfaces + routes + opt-in Zod schemas |
| Python emitter | `internal/emitter/backend/python/` | Backend: Python TypedDict types + ABC interfaces + Flask routes |
| TypeScript emitter | `internal/emitter/frontend/typescript/` | Frontend: fetch-based SDK with `VeldApiError`, path params, all HTTP methods |
| Cache | `internal/cache/cache.go` | File mtime tracking for incremental builds |
| CLI | `cmd/veld/main.go` | Cobra commands + generation orchestration |

**Key isolation rules:**
- Parser and emitters are completely independent. No emitter may import lexer/parser packages.
- Emitters self-register via `init()`. Adding a new emitter = new package + one blank import in `main.go`.
- Config resolution is decoupled from Cobra (uses `FlagOverrides` struct, not `*cobra.Command`).
- Emitters receive `EmitOptions` (Schemas, BaseUrl, DryRun) — no direct config dependency.

## Project Structure (veld init output)

```
my-project/
├── veld/                    ← all veld source (like prisma/)
│   ├── veld.config.json     ← { input, backend, frontend, out, schemas?, baseUrl? }
│   ├── app.veld             ← entry point, imports other files
│   ├── models/              ← model definitions
│   └── modules/             ← module/action definitions
└── README.md
```

`generated/` is created automatically on first `veld generate`. No `app/` directory is
scaffolded — project layout is left to the developer.

## Import System

`.veld` files support `import "path"` to split contracts across files:
```
// veld/app.veld
import "models/auth.veld"
import "modules/auth.veld"
```
Paths are relative to the file containing the import. Circular imports are silently skipped.

## Config Auto-Detection

`veld generate` (no flags) searches for config in this order:
1. `./veld.config.json`
2. `./veld/veld.config.json`

All paths in the config file are resolved relative to the config file's directory. So `"out": "../generated"` in `veld/veld.config.json` outputs to the project root's `generated/`.

### Config fields

```json
{
  "input": "app.veld",
  "backend": "node",
  "frontend": "typescript",
  "out": "../generated",
  "schemas": true,
  "baseUrl": "/api/v1"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `input` | *required* | Entry .veld file |
| `backend` | `"node"` | Backend emitter (`node`, `python`) |
| `frontend` | `"typescript"` | Frontend emitter (`typescript`, `none`; `react` aliases to `typescript`) |
| `out` | `"./generated"` | Output directory |
| `schemas` | `false` | Generate Zod validation schemas (opt-in) |
| `baseUrl` | `""` | Baked into frontend SDK (empty = `process.env.VELD_API_URL`) |

## .veld Contract Syntax

```
model ModelName {
  fieldName: type    // types: string, int, float, bool, date, datetime, uuid
  optional?: type    // optional field
  tags: string[]     // array type
  role: Role @default(user)  // default value
}

enum Role { admin user guest }

module ModuleName {
  description: "Module description"
  prefix: /api

  action ActionName {
    description: "Action description"
    method: POST
    path: /path/:id
    input: ModelName     // optional (void if omitted)
    output: ModelName    // optional (void if omitted)
    query: QueryModel    // optional query params
    middleware: AuthGuard // optional, repeatable
  }
}
```

HTTP methods: `GET POST PUT DELETE PATCH`

## Generated Output Structure

```
generated/
├── types/auth.ts               # TypeScript interfaces for all models
├── interfaces/IAuthService.ts  # Service contract (no dependencies)
├── routes/auth.routes.ts       # Route registration fn — zero runtime deps
├── schemas/schemas.ts          # Zod validation schemas (opt-in with --schemas)
└── client/api.ts               # Frontend SDK with VeldApiError, path params, all HTTP methods
```

All generated files begin with `// AUTO-GENERATED BY VELD — DO NOT EDIT`.

### Frontend SDK features

- **VeldApiError** class with `status` and `body` fields (not plain Error)
- **Path parameter interpolation**: `/users/:id` → `` `/users/${id}` `` with typed `id: string` param
- **All HTTP methods**: `get()`, `post()`, `put()`, `patch()`, `del()` — each action uses the correct one
- **Base URL**: configurable via `baseUrl` config or `process.env.VELD_API_URL`

## Type Mapping

| Veld | TypeScript | Python |
|------|-----------|--------|
| `string` | `string` | `str` |
| `int` | `number` | `int` |
| `float` | `number` | `float` |
| `bool` | `boolean` | `bool` |
| `date` | `string` | `str` |
| `datetime` | `string` | `str` |
| `uuid` | `string` | `str` |

## Hard Rules

- Veld **never** writes outside the `--out` directory
- Output is **deterministic** — same input always produces identical output
- `veld init` exits with code 1 if already initialised — never overwrites files
- Actions with no `input` generate a handler that reads `req.user?.id` for the service call
- Actions with no `output` generate `void` return type
- Validator errors include `file:line` context when available

## Test Suite

Tests live next to the code they test (`*_test.go` files):

| Package | Tests | Coverage |
|---------|-------|----------|
| `internal/lexer` | 15 tests | Tokenization, all types, comments, line tracking |
| `internal/parser` | 20 tests | Models, modules, enums, actions, error cases |
| `internal/validator` | 17 tests | Duplicates, undefined types, defaults, suggestions, file:line |
| `internal/config` | 7 tests | Defaults, overrides, aliases, schemas/baseUrl |
| `internal/loader` | 5 tests | Single file, imports, circular, source tracking |
| `internal/emitter` | 10 tests | Helpers, registry, transitive models, snake_case |

Run with: `go test ./... -v`

## Module: `github.com/veld-dev/veld`

Only external Go dependency: `cobra`. Lexer and parser written from scratch — no parser-generator libraries.
