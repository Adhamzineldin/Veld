<p align="center">
  <img src="https://raw.githubusercontent.com/Adhamzineldin/Veld/master/docs/assets/logo.svg" alt="Veld" width="120" />
</p>

<h1 align="center">Veld</h1>

<p align="center">
  <strong>Contract-first, multi-stack API code generator.</strong><br>
  Write <code>.veld</code> contracts &rarr; get typed backends, frontend SDKs, validation, and docs &mdash; instantly.
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#what-it-generates">What It Generates</a> &bull;
  <a href="#supported-stacks">Supported Stacks</a> &bull;
  <a href="#contract-syntax">Contract Syntax</a> &bull;
  <a href="#cli-reference">CLI Reference</a> &bull;
  <a href="#why-veld">Why Veld</a>
</p>

---

## What is Veld?

Veld is a **contract-first code generator** for APIs. You describe your data models and endpoints in `.veld` files, and Veld generates:

- **Backend service interfaces** with typed method signatures
- **Route handlers** with input validation, error handling, and correct HTTP status codes
- **Frontend SDKs** with typed API clients using native `fetch`
- **Validation schemas** (zero-dependency runtime validators)
- **OpenAPI 3.0 specs**, database schemas, Dockerfiles, CI/CD configs, and more

No runtime dependencies in generated code. No framework lock-in. Works with any compatible library.

## Quick Start

### Install

```bash
# From source
go install github.com/Adhamzineldin/Veld/cmd/veld@latest

# Or download the binary from releases
```

### Initialize a project

```bash
mkdir my-api && cd my-api
veld init
```

The interactive wizard lets you pick your backend and frontend stack:

```
  Veld — project setup

  Backend — which server runtime?
     1  csharp
     2  go
     3  java
     4  node-js
     5  node-ts (default)
     6  php
     7  python
     8  rust

  Choose [5]: █
```

This creates:

```
my-api/
├── veld/
│   ├── veld.config.json     ← configuration
│   ├── app.veld             ← entry point
│   ├── models/              ← data types
│   │   ├── user.veld
│   │   ├── auth.veld
│   │   └── common.veld
│   └── modules/             ← API endpoints
│       ├── users.veld
│       └── auth.veld
└── generated/               ← created on first generate
```

### Generate

```bash
veld generate
```

That's it. Your typed backend interfaces, route handlers, frontend SDK, and validation are ready.

### Watch mode

```bash
veld watch
```

Auto-regenerates on every file save with 500ms debounce.

## Contract Syntax

```veld
// models/user.veld

model User {
  description: "A registered user"
  id:       uuid
  email:    string
  name:     string
  role:     Role          @default(user)
  bio?:     string                        // optional
  tags:     string[]                      // array
  settings: Map<string, string>           // record/map
}

model AdminUser extends User {
  permissions: string[]
}

enum Role { admin user guest }
```

```veld
// modules/users.veld
import @models/user

module Users {
  description: "User management"
  prefix: /api/v1

  action ListUsers {
    method: GET
    path: /users
    output: User[]
    query: PaginationQuery
  }

  action GetUser {
    method: GET
    path: /users/:id
    output: User
    errors: [NotFound, Forbidden]
  }

  action CreateUser {
    method: POST
    path: /users
    input: CreateUserInput
    output: User
    errors: [Conflict, ValidationFailed]
    middleware: RequireAuth
  }

  action DeleteUser {
    method: DELETE
    path: /users/:id
    middleware: RequireAuth
  }
}
```

### Supported Types

| Veld Type | Description |
|-----------|-------------|
| `string` | Text |
| `int` | Integer number |
| `float` | Floating point number |
| `bool` | Boolean |
| `date` | Date string (YYYY-MM-DD) |
| `datetime` | ISO 8601 datetime |
| `uuid` | UUID string |
| `T[]` | Array of T |
| `Map<string, V>` | Key-value record |
| `ModelName` | Reference to another model |

### Features

- **`extends`** &mdash; model inheritance
- **`@default(value)`** &mdash; default field values
- **`@deprecated "reason"`** &mdash; deprecation on fields and actions
- **`errors: [NotFound, Forbidden]`** &mdash; typed error codes per action
- **`middleware: AuthGuard`** &mdash; middleware declarations
- **`method: WS`** &mdash; WebSocket endpoint support
- **`stream: MessageType`** &mdash; WebSocket message typing

## Supported Stacks

### Backends

| Name | Language | Output | Aliases |
|------|----------|--------|---------|
| `node-ts` | Node.js | TypeScript (`.ts`) | `node` |
| `node-js` | Node.js | JavaScript + JSDoc (`.js`) | `js`, `javascript` |
| `python` | Python | Python + Pydantic | |
| `go` | Go | Go + Chi router | |
| `rust` | Rust | Rust + Actix | |
| `java` | Java | Java + Spring | |
| `csharp` | C# | C# + ASP.NET | |
| `php` | PHP | PHP + PSR-15 | |

### Frontends

| Name | Language | Aliases |
|------|----------|---------|
| `typescript` | TypeScript fetch SDK | `ts` |
| `javascript` | JavaScript + JSDoc fetch SDK | `js` |
| `react` | React hooks | `hooks`, `react-hooks` |
| `vue` | Vue composables | |
| `angular` | Angular services | |
| `svelte` | Svelte stores | |
| `dart` | Dart HTTP client | `flutter` |
| `kotlin` | Kotlin HTTP client | |
| `swift` | Swift HTTP client | |
| `types-only` | Types only, no SDK | |
| `none` | Skip frontend generation | |

### Utility Generators

| Name | Output |
|------|--------|
| `openapi` | OpenAPI 3.0 JSON spec |
| `database` | Prisma schema + SQL DDL |
| `dockerfile` | Production Dockerfile |
| `cicd` | GitHub Actions / GitLab CI |
| `env` | Environment config templates |
| `scaffold-tests` | Test scaffolding |

## What It Generates

### Backend (node-ts example)

```
generated/
├── index.ts                    # Barrel export
├── package.json                # @veld/generated package
├── types/
│   ├── users.ts                # TypeScript interfaces per module
│   └── index.ts                # Barrel re-export
├── interfaces/
│   └── IUsersService.ts        # Service contract — you implement this
├── routes/
│   └── users.routes.ts         # Route handlers with try/catch + validation
├── errors/
│   └── users.errors.ts         # Typed error classes + factory functions
├── schemas/
│   └── schemas.ts              # Zod validation schemas
└── _validators.ts              # Zero-dep runtime validators (opt-in)
```

### Frontend (typescript example)

```
generated/client/
├── _internal.ts                # VeldApiError, HTTP helpers
├── usersApi.ts                 # Typed SDK methods
├── api.ts                      # Barrel + unified `api` object
├── types.ts                    # Re-exported types
├── errors.ts                   # Re-exported errors
└── package.json                # @veld/client package
```

### Generated route handlers

Every route handler includes:
- **try/catch** error handling
- **Input validation** (Zod, Pydantic, or zero-dep validators)
- **Correct status codes**: POST → 201, DELETE (no output) → 204, errors → appropriate 4xx/5xx
- **Path parameter extraction** from Express-style `:id` params
- **Middleware** wiring from contract declarations

### Frontend SDK usage

```typescript
import { api } from '@veld/client';
import { isErrorCode } from '@veld/client/errors';

// Typed — IDE autocomplete for methods, params, and return types
const users = await api.Users.listUsers();
const user = await api.Users.getUser('user-123');

try {
  await api.Users.createUser({ email: 'a@b.com', name: 'Alice' });
} catch (err) {
  if (isErrorCode(err, api.Users.errors.createUser.conflict)) {
    console.log('User already exists');
  }
}
```

## CLI Reference

| Command | Description |
|---------|-------------|
| `veld init` | Interactive project setup |
| `veld generate` | Generate all output |
| `veld generate --dry-run` | Preview without writing files |
| `veld watch` | Auto-regenerate on file changes |
| `veld validate` | Check contract for errors |
| `veld ast` | Dump AST as JSON |
| `veld openapi` | Export OpenAPI 3.0 spec |
| `veld schema` | Generate database schemas |
| `veld diff` | Detect breaking changes between versions |
| `veld docs` | Generate API documentation |
| `veld clean` | Remove generated output |
| `veld setup` | Auto-configure project imports |
| `veld lsp` | Start the Language Server Protocol server |

### Common flags

```bash
veld generate --backend=node-ts --frontend=react
veld generate --backend=python --frontend=dart
veld generate --out=./src/generated
veld generate --validate              # enable runtime validators
veld generate --base-url=/api/v1      # bake base URL into SDK
veld openapi -o openapi.json          # write spec to file
```

## Configuration

`veld/veld.config.json`:

```json
{
  "input": "app.veld",
  "backend": "node-ts",
  "frontend": "typescript",
  "out": "../generated",
  "baseUrl": "",
  "validate": false,
  "aliases": {
    "models": "models",
    "modules": "modules"
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `input` | *required* | Entry `.veld` file |
| `backend` | `node-ts` | Backend emitter |
| `frontend` | `typescript` | Frontend emitter |
| `out` | `./generated` | Output directory |
| `baseUrl` | `""` | Baked into frontend SDK |
| `validate` | `false` | Generate zero-dep runtime validators |
| `aliases` | built-in | Import alias mappings |

## Import System

Veld supports two import styles:

```veld
import @models/user          // Alias-based (recommended)
import "./models/user.veld"  // Relative path (legacy)
```

Aliases are resolved from the project root via the `aliases` config. Built-in aliases include: `models`, `modules`, `types`, `enums`, `schemas`, `services`, `lib`, `common`, `shared`.

## Why Veld?

| Problem | Veld's Answer |
|---------|---------------|
| Backend and frontend types drift apart | Single source of truth &mdash; one contract, all stacks |
| Writing boilerplate route handlers | Generated with validation, error handling, correct status codes |
| Frontend SDK maintenance | Auto-generated typed clients with error matching |
| Runtime shape violations | Zero-dep validators catch contract mismatches at runtime |
| OpenAPI spec goes stale | Generated from the same contract, always in sync |
| Switching frameworks | Framework-agnostic &mdash; generated code uses `router: any`, native `fetch` |

### Design Principles

1. **Zero runtime dependencies** in generated output (by default)
2. **Framework-agnostic** &mdash; works with Express, Fastify, Hono, Flask, Chi, Actix, or anything else
3. **Deterministic** &mdash; same input always produces identical output
4. **Never touches business logic** &mdash; Veld generates interfaces, you write implementations
5. **Contract-first** &mdash; the `.veld` file is the single source of truth

## IDE Support

- **VS Code**: Veld extension with syntax highlighting, diagnostics, and completions
- **JetBrains**: Plugin for IntelliJ, WebStorm, PyCharm, etc.
- **LSP**: `veld lsp` works with any LSP-compatible editor (Neovim, Helix, Sublime, etc.)

## Project Structure

```
veld/                          ← Go source
├── cmd/veld/                  ← CLI entry point (Cobra)
├── internal/
│   ├── ast/                   ← AST data structures
│   ├── lexer/                 ← Tokenizer
│   ├── parser/                ← Recursive descent parser
│   ├── validator/             ← Semantic validation
│   ├── loader/                ← File loading + import resolution
│   ├── config/                ← Config file + flag merging
│   ├── emitter/               ← Code generators
│   │   ├── backend/           ← node-ts, node-js, python, go, rust, java, csharp, php
│   │   └── frontend/          ← typescript, javascript, react, vue, angular, svelte, dart, kotlin, swift
│   ├── lsp/                   ← Language Server Protocol
│   └── schema/                ← Database schema generators
└── docs/                      ← Documentation
```

## Contributing

```bash
# Build
go build -o veld ./cmd/veld

# Test
go test ./...

# Lint
go vet ./...
```

Emitters self-register via `init()`. Adding a new backend or frontend emitter is one package + one blank import in `cmd/veld/main.go`.

## License

MIT
