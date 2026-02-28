# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## NON-NEGOTIABLE RULES

> These apply to every line of generated code, every emitter, every future feature.

1. **ZERO RUNTIME DEPENDENCIES in generated output** (unless explicitly opted in). Generated files must never `import` or `require` any external package by default. Zod schemas (Node) and Pydantic schemas (Python) are generated automatically — they require `zod` / `pydantic` to be installed, but the types/interfaces/routes work without them. The generated folder must be installable with zero `npm install` for type-only usage.

2. **Always agnostic, always dynamic.** Generated code must work with any compatible framework, not one specific library. Route handlers accept `router: any` — the user wires in their own Express/Fastify/Hono/whatever. The SDK uses native `fetch`, not axios. No lock-in, ever.

3. **`--backend=node`, not `--backend=express`.** The backend emitter targets the Node.js HTTP router pattern (`.get`, `.post`, etc.), not a specific framework. The flag name reflects that.

---

## Project Overview

**Veld** is a contract-first, multi-stack API code generator written in Go. A developer writes `.veld` contract files; Veld generates a typed frontend SDK (TypeScript) and backend service interfaces + route wiring with full input validation. Veld never touches developer business logic files.

## Build & Test Commands

```bash
go build -o veld.exe ./cmd/veld    # Build CLI binary (Windows)
go build -o veld ./cmd/veld        # Build CLI binary (Unix)
go build ./...                     # Build all packages (verify no errors)
go test ./...                      # Run all tests
go vet ./...                       # Static analysis
```

Once the binary is built:
```bash
veld --version                     # Print version
veld init                          # Scaffold new project (creates veld/ folder)
veld validate                      # Validate contract
veld ast                           # Dump AST JSON
veld generate                      # Generate all output
veld generate --dry-run            # Preview without writing files
veld generate --backend=node --frontend=typescript --input=veld/app.veld --out=./generated
veld watch                         # Auto-regenerate on file changes (500ms debounce)
veld clean                         # Remove generated output directory
veld openapi                       # Export OpenAPI 3.0 JSON to stdout
veld openapi -o openapi.json       # Export OpenAPI 3.0 JSON to file
```

## Architecture

The pipeline is strictly linear — **only AST JSON passes between stages**:

```
.veld files → Lexer → Parser → AST → import resolver → Validator → Emitter(s) → generated/
```

| Package | Path | Role |
|---------|------|------|
| AST types | `internal/ast/ast.go` | Shared data structures: Model (with `Extends`), Field (with `IsMap`, `MapValueType`), Module, Action, Enum |
| Lexer | `internal/lexer/lexer.go` | Tokenizes `.veld` source text (supports `<`, `>`, `,` for generics) |
| Parser | `internal/parser/parser.go` | Recursive descent; `extends`, `Map<K,V>` syntax, line numbers |
| Validator | `internal/validator/validator.go` | Semantic checks: circular inheritance, Map value types, `file:line` error context |
| Config | `internal/config/config.go` | Config file loading, flag merging, path resolution |
| Loader | `internal/loader/loader.go` | Loads .veld files, resolves imports recursively |
| Emitter registry | `internal/emitter/emitter.go` | `BackendEmitter` / `FrontendEmitter` interfaces + `init()`-based registry |
| Emitter helpers | `internal/emitter/helpers.go` | Shared: `CollectTransitiveModels`, `ExtractPathParams`, `ToFlaskPath`, `ToOpenAPIPath`, etc. |
| TS helpers | `internal/emitter/tshelpers/` | Shared TypeScript type-mapping (`VeldFieldToTS`, `FormatOutputType`) |
| Node emitter | `internal/emitter/backend/node/` | Single types file, interfaces, routes (try/catch + Zod validation + status codes), Zod schemas, barrel + package.json |
| Python emitter | `internal/emitter/backend/python/` | Single types file, ABC interfaces, Flask routes (try/except + Pydantic + status codes), Pydantic schemas |
| TypeScript emitter | `internal/emitter/frontend/typescript/` | Frontend: fetch-based SDK with `VeldApiError`, path params, all HTTP methods |
| Cache | `internal/cache/cache.go` | File mtime tracking for incremental builds |
| CLI | `cmd/veld/main.go` | Cobra commands: generate, watch, validate, ast, init, clean, openapi |

**Key isolation rules:**
- Parser and emitters are completely independent. No emitter may import lexer/parser packages.
- Emitters self-register via `init()`. Adding a new emitter = new package + one blank import in `main.go`.
- Config resolution is decoupled from Cobra (uses `FlagOverrides` struct, not `*cobra.Command`).
- Emitters receive `EmitOptions` (BaseUrl, DryRun) — no direct config dependency.

## Project Structure (veld init output)

```
my-project/
├── veld/                    ← all veld source (like prisma/)
│   ├── veld.config.json     ← { input, backend, frontend, out, baseUrl? }
│   ├── app.veld             ← entry point, imports other files
│   ├── models/              ← model definitions
│   └── modules/             ← module/action definitions
└── README.md
```

`generated/` is created automatically on first `veld generate`. No `app/` directory is
scaffolded — project layout is left to the developer.

## Config Auto-Detection

`veld generate` (no flags) searches for config in this order:
1. `./veld.config.json`
2. `./veld/veld.config.json`

### Config fields

```json
{
  "input": "app.veld",
  "backend": "node",
  "frontend": "typescript",
  "out": "../generated",
  "baseUrl": "/api/v1"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `input` | *required* | Entry .veld file |
| `backend` | `"node"` | Backend emitter (`node`, `python`) |
| `frontend` | `"typescript"` | Frontend emitter (`typescript`, `none`; `react` aliases to `typescript`) |
| `out` | `"./generated"` | Output directory |
| `baseUrl` | `""` | Baked into frontend SDK (empty = `process.env.VELD_API_URL`) |

## .veld Contract Syntax

```
model ModelName {
  description: "..."
  fieldName:  type         // types: string, int, float, bool, date, datetime, uuid
  optional?:  type         // optional field
  tags:       string[]     // array type
  metadata:   Map<string, string>  // map/record type
  role:       Role @default(user)  // default value
}

model ChildModel extends ParentModel {
  extraField: string       // inherits all parent fields
}

enum Role { admin user guest }

module ModuleName {
  description: "Module description"
  prefix: /api

  action ActionName {
    description: "Action description"
    method: POST
    path: /path/:id
    input: ModelName
    output: ModelName
    query: QueryModel
    middleware: AuthGuard
  }
}
```

HTTP methods: `GET POST PUT DELETE PATCH`

## Generated Output Structure (Node backend)

```
generated/
├── index.ts                    # Barrel export for clean imports
├── package.json                # @veld/generated package alias
├── README.md                   # Auto-generated documentation
├── types/types.ts              # ALL TypeScript interfaces + enums (single file, no duplicates)
├── interfaces/IAuthService.ts  # Service contracts (typed path params)
├── routes/auth.routes.ts       # Route handlers: try/catch, Zod validation, correct HTTP status codes
├── schemas/schemas.ts          # Zod validation schemas (supports extends)
└── client/api.ts               # Frontend SDK with VeldApiError, path params, all HTTP methods
```

### Generated Output Structure (Python backend)

```
generated/
├── __init__.py
├── types/__init__.py           # ALL TypedDicts + Literal enums (single file)
├── interfaces/i_auth_service.py # ABC service contracts
├── routes/auth_routes.py       # Flask handlers: try/except, Pydantic validation, correct status codes
└── schemas/schemas.py          # Pydantic BaseModel schemas
```

All generated files begin with `// AUTO-GENERATED BY VELD — DO NOT EDIT` (or `#` for Python).

### Route handler features
- **try/catch** (Node) / **try/except** (Python) wrapping — no unhandled exceptions
- **Input validation** — Zod `.parse()` (Node) / Pydantic `(**data).model_dump()` (Python)
- **ZodError → 400** with validation details
- **Correct HTTP status codes**: POST → 201, DELETE with no output → 204, else 200
- **Path param extraction**: `/users/:id` → `req.params.id` (Node) / Flask `<id>` (Python)

### Frontend SDK features
- **VeldApiError** class with `status` and `body` fields
- **Path parameter interpolation**: `/users/:id` → `` `/users/${id}` `` with typed `id: string` param
- **All HTTP methods**: `get()`, `post()`, `put()`, `patch()`, `del()`
- **Base URL**: configurable via `baseUrl` config or `process.env.VELD_API_URL`

### Import aliases
Generated `package.json` enables `@veld/generated` path alias. Add to `tsconfig.json`:
```json
{ "compilerOptions": { "paths": { "@veld/*": ["./generated/*"] } } }
```

## Type Mapping

| Veld | TypeScript | Python | Zod | Pydantic |
|------|-----------|--------|-----|----------|
| `string` | `string` | `str` | `z.string()` | `str` |
| `int` | `number` | `int` | `z.number().int()` | `int` |
| `float` | `number` | `float` | `z.number()` | `float` |
| `bool` | `boolean` | `bool` | `z.boolean()` | `bool` |
| `date` | `string` | `str` | `z.string().date()` | `str` |
| `datetime` | `string` | `str` | `z.string().datetime()` | `str` |
| `uuid` | `string` | `str` | `z.string().uuid()` | `str` |
| `T[]` | `T[]` | `List[T]` | `z.array(TSchema)` | `List[T]` |
| `Map<string,V>` | `Record<string,V>` | `Dict[str,V]` | `z.record(z.string(),V)` | `Dict[str,V]` |

## Hard Rules

- Veld **never** writes outside the `--out` directory
- Output is **deterministic** — same input always produces identical output
- `veld init` exits with code 1 if already initialised — never overwrites files
- Actions with no `input` generate no body parsing — service called with path params only
- Actions with no `output` generate `void` return type (TS) / `None` (Python)
- Validator errors include `file:line` context with source code snippet when available
- Types are emitted into a single file (`types/types.ts` / `types/__init__.py`) — never duplicated across modules
- `extends` generates TS `interface X extends Y` / Zod `.extend()` / Python class inheritance
- `Map<string, V>` generates TS `Record<string, V>` / Python `Dict[str, V]`

## Module: `github.com/veld-dev/veld`

Only external Go dependency: `cobra`. Lexer and parser written from scratch — no parser-generator libraries.
