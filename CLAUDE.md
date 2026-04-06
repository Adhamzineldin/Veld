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

**Module path:** `github.com/Adhamzineldin/Veld`

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
veld lint                          # Analyse contract for quality issues
veld fmt                           # Format .veld files
veld docs                          # Generate API documentation
veld graphql                       # Export GraphQL SDL schema
veld schema                        # Generate database schema
veld diff                          # Show diff vs last generated output
veld doctor                        # Diagnose project health
veld setup                         # Auto-configure tsconfig/paths
veld login --registry <url> --token vtk_...   # Authenticate with registry
veld logout                        # Remove stored credentials
veld push                          # Publish contracts to registry
veld pull @org/name[@version]      # Download contract package
veld serve                         # Start self-hosted registry server
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
| Emitter registry | `internal/emitter/emitter.go` | `BackendEmitter` / `FrontendEmitter` / `ToolEmitter` interfaces + `init()`-based registry |
| Emitter helpers | `internal/emitter/helpers.go` | Shared: `CollectTransitiveModels`, `ExtractPathParams`, `ToFlaskPath`, `ToOpenAPIPath`, etc. |
| TS helpers | `internal/emitter/tshelpers/` | Shared TypeScript type-mapping (`VeldFieldToTS`, `FormatOutputType`) |
| SDK helpers | `internal/emitter/sdkhelpers/` | Shared: `EnvVarName`, `ServiceClassName`, `ServiceFileName` |
| Node emitter | `internal/emitter/backend/node/` | Types, interfaces, routes (Zod + try/catch), schemas, barrel + **sdk.go** |
| Python emitter | `internal/emitter/backend/python/` | Types, ABC interfaces, Flask routes (Pydantic + try/except) + **sdk.go** |
| Go emitter | `internal/emitter/backend/go/` | Chi router, typed interfaces, go.mod, server.go + **sdk.go** |
| Rust emitter | `internal/emitter/backend/rust/` | Axum handlers, Serde structs, services trait + **sdk.go** |
| Java emitter | `internal/emitter/backend/java/` | Spring Boot controllers + service interfaces + **sdk.go** |
| C# emitter | `internal/emitter/backend/csharp/` | ASP.NET Core controllers + service interfaces + **sdk.go** |
| PHP emitter | `internal/emitter/backend/php/` | Laravel routes + service contracts + **sdk.go** |
| JS backend emitter | `internal/emitter/backend/javascript/` | Plain Node.js (no TypeScript) + **sdk.go** |
| TypeScript emitter | `internal/emitter/frontend/typescript/` | Fetch-based SDK with `VeldApiError`, path params, all HTTP methods |
| React emitter | `internal/emitter/frontend/react/` | React Query hooks wrapping the TS SDK |
| Vue emitter | `internal/emitter/frontend/vue/` | Vue Composables wrapping the TS SDK |
| Angular emitter | `internal/emitter/frontend/angular/` | Angular services wrapping the TS SDK |
| Svelte emitter | `internal/emitter/frontend/svelte/` | Svelte stores/functions wrapping the TS SDK |
| Dart emitter | `internal/emitter/frontend/dart/` | Dart http client SDK |
| Kotlin emitter | `internal/emitter/frontend/kotlin/` | Kotlin client SDK |
| Swift emitter | `internal/emitter/frontend/swift/` | Swift URLSession SDK |
| JS frontend emitter | `internal/emitter/frontend/javascript/` | Plain JS fetch SDK (no TypeScript) |
| Types-only emitter | `internal/emitter/frontend/typesonly/` | Types with no SDK logic |
| CI/CD generator | `internal/generators/cicd/` | GitHub Actions workflow (auto-detects language) |
| Dockerfile generator | `internal/generators/dockerfile/` | Multi-stage Dockerfile + .dockerignore (auto-detects language) |
| Database generator | `internal/generators/database/` | SQL schema generation |
| Envconfig generator | `internal/generators/envconfig/` | .env template generation |
| OpenAPI generator | `internal/generators/openapi/` | OpenAPI 3.0 spec |
| Scaffold generator | `internal/generators/scaffold/` | Project scaffold helpers |
| Workspace validator | `internal/validator/workspace.go` | Validates `consumes` declarations (circular, unknown, self) |
| Diff | `internal/diff/` | Breaking change detection + `.veld.lock.json` |
| Lint | `internal/lint/lint.go` | Contract quality rules |
| Format | `internal/format/` | .veld file formatter |
| LSP | `internal/lsp/` | Language Server Protocol (stdin/stdout) |
| Docs gen | `internal/docsgen/` | API documentation generator |
| GraphQL gen | `internal/graphqlgen/` | GraphQL SDL export |
| OpenAPI gen | `internal/openapigen/` | OpenAPI 3.0 export |
| Schema gen | `internal/schema/` | Database schema export |
| Setup | `internal/setup/` | tsconfig/paths auto-configuration |
| Registry client | `internal/registry/` | credentials.go, client.go, tarball.go (pack/unpack/verify) |
| Registry server | `internal/server/` | PostgreSQL-backed registry: auth, packages, orgs, SMTP, SPA web UI |
| Cache | `internal/cache/cache.go` | File mtime tracking for incremental builds |
| CLI | `cmd/veld/main.go` | Single binary — all 26 commands including `veld serve` |

**Key isolation rules:**
- Parser and emitters are completely independent. No emitter may import lexer/parser packages.
- Emitters self-register via `init()`. Adding a new emitter = new package + one blank import in `main.go`.
- Config resolution is decoupled from Cobra (uses `FlagOverrides` struct, not `*cobra.Command`).
- Emitters receive `EmitOptions` (BaseUrl, DryRun) — no direct config dependency.
- There is **one binary**: `cmd/veld/`. No separate registry binary exists anymore.

## Project Structure (veld init output)

```
my-project/
├── veld/                    ← all veld source (like prisma/)
│   ├── veld.config.json     ← config file (nested or flat format)
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

### Config format (recommended — nested)

```json
{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",
  "input": "app.veld",
  "description": "My API",

  "backendConfig": {
    "target": "node-ts",
    "framework": "express",
    "out": "../backend/src/generated",
    "dir": "../backend",
    "validate": true
  },

  "frontendConfig": {
    "target": "react",
    "out": "../frontend/src/generated",
    "dir": "../frontend"
  },

  "baseUrl": "/api/v1",
  "aliases": { "auth": "services/auth" },

  "tools": {
    "openapi": true,
    "dockerfile": true
  },

  "hooks": {
    "postGenerate": "npm run format"
  }
}
```

### Config format (legacy flat — still works)

```json
{
  "input": "app.veld",
  "backend": "node",
  "frontend": "typescript",
  "out": "../generated",
  "backendFramework": "express",
  "backendDir": "../backend",
  "baseUrl": "/api/v1",
  "postGenerate": "npm run format"
}
```

Both formats are **fully supported**. The parser normalizes internally. Nested takes precedence if both are present.

| Field (nested) | Field (flat, deprecated) | Default | Description |
|----------------|--------------------------|---------|-------------|
| `backendConfig.target` | `backend` | `"node"` | Backend emitter |
| `backendConfig.framework` | `backendFramework` | `""` | Framework variant |
| `backendConfig.out` | `backendOut` | `out` value | Backend output dir |
| `backendConfig.dir` | `backendDir` | `""` | Backend project dir |
| `backendConfig.validate` | `validate` | `false` | Emit validators |
| `frontendConfig.target` | `frontend` | `"typescript"` | Frontend emitter |
| `frontendConfig.out` | `frontendOut` | `out` value | Frontend output dir |
| `frontendConfig.dir` | `frontendDir` | `""` | Frontend project dir |
| `hooks.postGenerate` | `postGenerate` | `""` | Post-generate command |
| `tools` | — | `{}` | Auxiliary generators |
| `baseUrl` | `baseUrl` | `""` | Baked into SDK (empty = env var) |
| `aliases` | `aliases` | built-in defaults | `@alias` → dir mappings |
| `workspace` | `workspace` | `[]` | Multi-service entries |

### JSON Schema

Canonical source: `editors/veld-config.schema.json`  
Sync to plugins: `bash scripts/sync-schema.sh`  
Both VS Code and JetBrains plugins bundle the schema for auto-completion.

### Import System

Veld supports two import styles:

```
import @models/user          // Alias-based (recommended) — resolved from project root via aliases
import "./models/user.veld"  // Relative path (legacy) — resolved relative to current file
```

Both styles are fully supported in the CLI, VS Code extension, and JetBrains plugin.

## Service SDK Generation (Inter-Service Communication)

Veld generates **typed, language-native HTTP client SDKs** for inter-service communication in microservice workspaces. When Service A (Python) declares it `consumes` Service B (Node.js), Veld generates a Python client SDK for B's API inside A's output directory.

### Config: `consumes` field

Add `consumes` to workspace entries in `veld.config.json`:

```json
{
  "workspace": [
    {
      "name": "iam",
      "backendConfig": { "target": "node-ts" },
      "baseUrl": "http://iam:3001",
      "input": "services/iam/modules/iam.veld",
      "out": "../backend/iam-service/generated"
    },
    {
      "name": "transactions",
      "backendConfig": { "target": "python", "framework": "flask" },
      "baseUrl": "http://transactions:3003",
      "input": "services/transactions/modules/transactions.veld",
      "out": "../backend/transaction-service/generated",
      "consumes": ["iam"]
    },
    {
      "name": "frontend",
      "frontendConfig": { "target": "react" },
      "out": "../frontend/src/generated",
      "consumes": ["iam", "transactions"]
    }
  ]
}
```

### CLI flags

```bash
veld generate                    # generates SDKs for entries with consumes
veld generate --service-sdk      # generate SDKs for ALL workspace siblings
veld deps                        # print service dependency graph
veld deps --validate             # check for circular/unknown dependencies
```

### Generated output

Each consumed service gets `sdk/<service>/` in the consumer's output directory:

**TypeScript** (`--backend=node`): `sdk/iam/client.ts`, `types.ts`, `index.ts` — fetch-based  
**Python** (`--backend=python`): `sdk/iam/client.py`, `types.py`, `__init__.py` — urllib-based  
**Go** (`--backend=go`): `sdk/iam/client.go`, `types.go`, `doc.go` — net/http-based  

### Base URL resolution (per-language priority)

1. Constructor argument: `IAMClient(baseUrl="...")`
2. Environment variable: `VELD_IAM_URL` (convention: `VELD_<UPPER_SNAKE_NAME>_URL`)
3. Baked-in default from consumed service's `baseUrl` config
4. Error if none provided

### Architecture

| Package | Path | Role |
|---------|------|------|
| `BackendEmitter.EmitServiceSdk()` | `internal/emitter/emitter.go` | Required method on BackendEmitter — every backend MUST implement SDK generation |
| `ToolEmitter` interface | `internal/emitter/emitter.go` | Separate interface for non-backend generators (CI/CD, Dockerfile, etc.) |
| `ConsumedServiceInfo` | `internal/emitter/emitter.go` | Carries consumed service AST + baseUrl to emitters |
| SDK helpers | `internal/emitter/sdkhelpers/` | `EnvVarName`, `ServiceClassName`, `ServiceFileName` |
| Node SDK | `internal/emitter/backend/node/sdk.go` | TypeScript fetch client |
| Python SDK | `internal/emitter/backend/python/sdk.go` | Python urllib client |
| Go SDK | `internal/emitter/backend/go/sdk.go` | Go net/http client |
| Rust SDK | `internal/emitter/backend/rust/sdk.go` | Rust reqwest client |
| Java SDK | `internal/emitter/backend/java/sdk.go` | Java HttpClient client |
| C# SDK | `internal/emitter/backend/csharp/sdk.go` | C# HttpClient client |
| PHP SDK | `internal/emitter/backend/php/sdk.go` | PHP cURL client |
| JS SDK | `internal/emitter/backend/javascript/sdk.go` | Plain JS fetch client |
| Workspace validation | `internal/validator/workspace.go` | Circular, unknown, self-consumption checks |

### Hard rules

- SDK clients use **zero runtime dependencies** (native fetch/urllib/net/http/curl)
- `EmitServiceSdk` is **required** on all `BackendEmitter` implementations — compiler enforced
- Dependencies are **config-only** (`consumes` in workspace entries) — no parser/lexer changes
- Each SDK is **self-contained** (own types, no cross-SDK imports)
- Model inheritance is **flattened** in Go SDKs (Go has no struct inheritance)
- WebSocket actions are **skipped** in service SDKs (HTTP-only)
- Tool emitters (cicd, database, dockerfile, etc.) live in `internal/generators/`, NOT in `internal/emitter/`

## .veld Contract Syntax

```
model ModelName {
  description: "..."
  fieldName:  type         // types: string, int, float, decimal, bool, date, datetime, uuid
  optional?:  type         // optional field
  tags:       string[]     // array type
  metadata:   Map<string, string>  // map/record type
  role:       Role @default(user)  // default value
  old:        string @deprecated "use newField instead"
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
    @deprecated "use NewAction instead"
  }
}
```

HTTP methods: `GET POST PUT DELETE PATCH`

## Registry Server (`veld serve`)

The registry server is built into the main `veld` binary. Run with:

```bash
veld serve                                          # auto-detect registry.config.json
veld serve --config registry.config.json
veld serve --addr :9000 --dsn "postgres://..." --secret mysecret
```

Config file (`registry.config.json`):
```json
{
  "addr": ":8080",
  "dsn": "postgres://user:pass@localhost/veld?sslmode=disable",
  "storage": "./packages",
  "secret": "<32+ char jwt secret>",
  "base_url": "https://registry.example.com",
  "smtp": { "host": "smtp.example.com", "port": 587, "username": "...", "password": "...", "from": "..." }
}
```

Environment variables: `VELD_ADDR`, `VELD_DSN`, `VELD_SECRET`, `VELD_STORAGE`, `VELD_BASE_URL`, `SMTP_HOST`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`

Priority (highest → lowest): CLI flags > env vars > registry.config.json > defaults

Server packages in `internal/server/`:
- `server.go` — Go 1.22 ServeMux routing, CORS, logger, graceful shutdown
- `auth/token.go` — HMAC-SHA256 JWT (no external lib), `vtk_` token generation
- `auth/middleware.go` — Bearer + session-cookie auth
- `auth/totp.go` — TOTP 2FA
- `db/db.go` — PostgreSQL connection, auto-migration, CRUD queries
- `models/models.go` — User, Org, OrgMember, Package, PackageVersion, Token
- `handlers/auth.go` — Register, Login, Logout, TOTP, email verification
- `handlers/packages.go` — Publish (multipart), Download, Deprecate, Delete
- `handlers/orgs.go` — Org CRUD + member management
- `handlers/web.go` — Embedded SPA (`//go:embed web`)
- `storage/storage.go` — Storage interface + local filesystem impl
- `email/email.go` — SMTP email sending

## Generated Output Structure (Node backend)

```
generated/
├── index.ts                    # Barrel export for clean imports
├── package.json                # @veld/generated package alias
├── README.md                   # Auto-generated documentation
├── types/
│   ├── users.ts                # Types owned by Users module
│   ├── auth.ts                 # Types owned by Auth module + re-exports shared from users.ts
│   └── index.ts                # Barrel re-export of all module type files
├── interfaces/IAuthService.ts  # Service contracts (typed path params)
├── routes/auth.routes.ts       # Route handlers: try/catch, Zod validation, correct HTTP status codes
├── schemas/schemas.ts          # Zod validation schemas (supports extends)
└── client/api.ts               # Frontend SDK with VeldApiError, path params, all HTTP methods
```

### Generated Output Structure (Python backend)

```
generated/
├── __init__.py
├── types/
│   ├── users.py                # Types owned by Users module
│   ├── auth.py                 # Types owned by Auth module + re-imports shared
│   └── __init__.py             # Barrel re-import of all module type files
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
| `decimal` | `string` | `Decimal` | `z.string()` | `Decimal` |
| `bool` | `boolean` | `bool` | `z.boolean()` | `bool` |
| `date` | `string` | `str` | `z.string().date()` | `str` |
| `datetime` | `string` | `str` | `z.string().datetime()` | `str` |
| `uuid` | `string` | `str` | `z.string().uuid()` | `str` |
| `T[]` | `T[]` | `List[T]` | `z.array(TSchema)` | `List[T]` |
| `Map<string,V>` | `Record<string,V>` | `Dict[str,V]` | `z.record(z.string(),V)` | `Dict[str,V]` |

### Extended Type Mapping (all backends)

| Veld | Go | Rust | Java | C# | PHP | Dart | Kotlin | Swift | SQL | OpenAPI | GraphQL |
|------|-----|------|------|----|-----|------|--------|-------|-----|---------|---------|
| `decimal` | `string` | `String` | `BigDecimal` | `decimal` | `string` | `String` | `BigDecimal` | `Decimal` | `DECIMAL(19,4)` | `string` format:`decimal` | `String` |

## Hard Rules

- Veld **never** writes outside the `--out` directory
- Output is **deterministic** — same input always produces identical output
- `veld init` exits with code 1 if already initialised — never overwrites files
- Actions with no `input` generate no body parsing — service called with path params only
- Actions with no `output` generate `void` return type (TS) / `None` (Python)
- Validator errors include `file:line` context with source code snippet when available
- Types are emitted into per-module files (`types/users.ts`, `types/auth.ts`). Each type is **defined** in exactly one file (the first module to use it). Other modules re-export shared types. A barrel `types/index.ts` re-exports everything.
- `extends` generates TS `interface X extends Y` / Zod `.extend()` / Python class inheritance
- `Map<string, V>` generates TS `Record<string, V>` / Python `Dict[str, V]`

## Contract Safety Features

### Breaking Change Detection (`internal/diff/`)
- `diff.go` — `Diff(old, new AST) []Change`, `HasBreaking()`
- `lock.go` — `LoadLock`, `SaveLock`, `DeleteLock` → `.veld.lock.json`
- `veld generate` pre-emit gate: interactive prompt by default, `--strict` exits 1 (CI), `--force` skips prompt
- `veld clean` removes lock file

### Lint (`internal/lint/lint.go`)
- `veld lint [--exit-code]`
- Rules: unused-model, empty-module, empty-model, duplicate-route (error), duplicate-action (error), missing-description, deprecated-action, deprecated-field

### @deprecated annotation
- Syntax: `fieldName: type @deprecated "msg"` and `@deprecated "msg"` inside action body
- AST: `Field.Deprecated string`, `Action.Deprecated string`
- Emits: JSDoc `@deprecated` in Node interfaces + TS SDK; Python `.. deprecated::` docstring

## Go Modules & External Dependencies

```
module github.com/Adhamzineldin/Veld

require (
    github.com/spf13/cobra          // CLI framework
    golang.org/x/crypto             // bcrypt for password hashing
    modernc.org/sqlite              // pure-Go SQLite (fallback/testing)
)
```

PostgreSQL is the registry server's database backend (DSN required for `veld serve`).
