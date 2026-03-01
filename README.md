# Veld

**Contract-first, multi-stack API code generator.**

Write `.veld` contracts once → generate typed frontend SDKs + backend service interfaces for any framework. Zero runtime dependencies.

```
.veld files → Lexer → Parser → AST → Validator → Emitter(s) → generated/
```

## Install

```bash
# Go (recommended)
go install github.com/veld-dev/veld/cmd/veld@latest

# npm
npm install veld

# pip
pip install veld

# Homebrew
brew install veld-dev/tap/veld

# Composer (PHP)
composer require veld-dev/veld
```

## Quick Start

```bash
mkdir my-api && cd my-api
veld init
veld generate
```

This creates:

```
my-api/
├── veld/                    ← your contracts
│   ├── veld.config.json
│   ├── app.veld             ← entry point
│   ├── models/              ← data types
│   └── modules/             ← API endpoints
└── generated/               ← auto-generated (don't edit)
    ├── types/               ← TypeScript interfaces
    ├── interfaces/          ← service contracts
    ├── routes/              ← route handlers with validation
    ├── schemas/             ← Zod / Pydantic schemas
    └── client/api.ts        ← frontend SDK
```

## Example Contract

```veld
// veld/models/user.veld
enum Role { admin user guest }

model User {
  id:        uuid
  email:     string
  name:      string
  role:      Role      @default(user)
  verified:  bool      @default(false)
  tags:      string[]
  metadata:  Map<string, string>
  createdAt: datetime
}

model LoginInput {
  email:    string
  password: string
}
```

```veld
// veld/modules/auth.veld
module Auth {
  description: "Authentication"

  action Login {
    method: POST
    path:   /auth/login
    input:  LoginInput
    output: User
  }

  action Me {
    method:     GET
    path:       /auth/me
    output:     User
    middleware:  AuthGuard
  }
}
```

## Commands

| Command | Description |
|---------|-------------|
| `veld init` | Scaffold a new project |
| `veld generate` | Generate all output |
| `veld generate --dry-run` | Preview without writing files |
| `veld generate --backend=go` | Generate Go backend |
| `veld watch` | Auto-regenerate on file changes |
| `veld validate` | Check contracts for errors |
| `veld clean` | Remove generated output |
| `veld openapi` | Export OpenAPI 3.0 spec |
| `veld ast` | Dump AST JSON |

## Supported Backends

| Backend | Flag | Output |
|---------|------|--------|
| Node.js (TypeScript) | `--backend=node` | Types, Zod schemas, route handlers |
| Python | `--backend=python` | TypedDict, Pydantic schemas, Flask routes |
| Go | `--backend=go` | Structs, handlers, middleware |
| Rust | `--backend=rust` | Structs, Actix handlers |
| Java | `--backend=java` | Records, Spring controllers |
| C# | `--backend=csharp` | Records, ASP.NET controllers |
| PHP | `--backend=php` | Classes, Laravel routes |

Frontend: `--frontend=typescript` generates a zero-dependency fetch SDK.

## Config

`veld/veld.config.json`:

```json
{
  "input": "app.veld",
  "backend": "node",
  "frontend": "typescript",
  "out": "../generated",
  "aliases": {
    "models": "models",
    "modules": "modules"
  }
}
```

## Import System

```veld
import @models/user           // alias-based (recommended)
import "./models/user.veld"   // relative path
import @models/*              // wildcard — all .veld files in models/
```

## Editor Support

- **VS Code** — [veld-vscode](editors/vscode/) — syntax highlighting, completions, diagnostics, go-to-definition
- **JetBrains** — [veld-jetbrains](editors/jetbrains/) — works in all JetBrains IDEs

## .veld Language Features

- **Models** — `model Name { field: type }`
- **Enums** — `enum Status { active inactive }`
- **Modules** — `module Name { action ... }`
- **Inheritance** — `model Admin extends User { }`
- **Arrays** — `tags: string[]` or `List<string>`
- **Maps** — `metadata: Map<string, string>`
- **Optional fields** — `bio?: string`
- **Defaults** — `role: Role @default(user)`
- **Annotations** — `@default`, `@required`, `@min`, `@max`, `@regex`, `@unique`, `@deprecated`
- **Descriptions** — `description: "..."`
- **Query params** — `query: FilterModel`
- **Middleware** — `middleware: AuthGuard`
- **Path params** — `path: /users/:id`

## Documentation

| Document | Description |
|----------|-------------|
| [CLAUDE.md](CLAUDE.md) | Full technical spec — architecture, type mappings, rules |
| [docs/](docs/README.md) | Documentation index |
| [docs/guides/getting-started.md](docs/guides/getting-started.md) | Install, scaffold, generate, use |
| [docs/architecture/overview.md](docs/architecture/overview.md) | Pipeline, packages, emitter system |
| [docs/roadmap.md](docs/roadmap.md) | Implementation roadmap and future phases |
| [docs/ci-cd/guide.md](docs/ci-cd/guide.md) | CI/CD pipeline — how releases work |
| [packages/README.md](packages/README.md) | Package manager wrappers |

## Build from Source

```bash
git clone https://github.com/veld-dev/veld.git
cd veld
go build -o veld ./cmd/veld
go test ./...
```

## License

MIT


