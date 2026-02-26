# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## NON-NEGOTIABLE RULES

> These apply to every line of generated code, every emitter, every future feature.

1. **ZERO RUNTIME DEPENDENCIES in generated output.** Generated files must never `import` or `require` any external package. No express, no axios, no lodash — nothing. If a generated file needs a type, use `import type` (erased at compile time) or inline the interface. The generated folder must be installable with zero `npm install`.

2. **Always agnostic, always dynamic.** Generated code must work with any compatible framework, not one specific library. Route handlers accept `router: any` — the user wires in their own Express/Fastify/Hono/whatever. The SDK uses native `fetch`, not axios. No lock-in, ever.

3. **`--backend=node`, not `--backend=express`.** The backend emitter targets the Node.js HTTP router pattern (`.get`, `.post`, etc.), not a specific framework. The flag name reflects that.

---

## Project Overview

**Veld** is a contract-first, multi-stack API code generator written in Go. A developer writes `.veld` contract files; Veld generates a typed frontend SDK (TypeScript) and backend service interfaces + route wiring. Veld never touches developer business logic files.

The full specification lives in `veld-claude-code-prompt (2).md`. The visual blueprint is in `ross-framework-plan (1).html`.

## Build Commands

```bash
go build -o veld.exe ./cmd/veld    # Build CLI binary (Windows)
go build -o veld ./cmd/veld        # Build CLI binary (Unix)
go build ./...                     # Build all packages (verify no errors)
go test ./...                      # Run all tests
```

Once the binary is built:
```bash
veld init                          # Scaffold new project (creates veld/ folder)
veld validate                      # Validate contract (reads veld/veld.config.json)
veld ast                           # Dump AST JSON
veld generate                      # Generate (auto-detects veld/veld.config.json)
veld generate --backend=node --frontend=react --input=veld/schema.veld --out=./generated
```

Smoke test:
```bash
veld generate --backend=node --frontend=react --input=testdata/auth.veld --out=./testdata/generated
```

## Architecture

The pipeline is strictly linear — **only AST JSON passes between stages**:

```
.veld files → Lexer → Parser → AST → import resolver → Validator → Emitter(s) → generated/
```

| Package | Path | Role |
|---------|------|------|
| AST types | `internal/ast/ast.go` | Shared data structures; no logic |
| Lexer | `internal/lexer/lexer.go` | Tokenizes `.veld` source text |
| Parser | `internal/parser/parser.go` | Recursive descent; produces AST |
| Validator | `internal/validator/validator.go` | Semantic checks on AST |
| Emitter interface | `internal/emitter/emitter.go` | `Emit(ast AST, outDir string) error` |
| Node emitter | `internal/emitter/node/node.go` | Generates TS types + interfaces + routes |
| TypeScript emitter | `internal/emitter/typescript/typescript.go` | Generates frontend SDK |
| CLI | `cmd/veld/main.go` | Cobra commands + import resolver |

**Key isolation rule:** Parser and emitters are completely independent. No emitter may import lexer/parser packages.

## Project Structure (veld init output)

```
my-project/
├── veld/                    ← all veld source (like prisma/)
│   ├── veld.config.json     ← { input, backend, frontend, out }
│   ├── schema.veld          ← entry point, imports other files
│   ├── models/              ← model definitions
│   └── modules/             ← module/action definitions
├── generated/               ← veld output — never edit manually
├── app/
│   ├── services/            ← developer implements generated interfaces here
│   └── repositories/
└── README.md
```

## Import System

`.veld` files support `import "path"` to split contracts across files:
```
// veld/schema.veld
import "models/auth.veld"
import "modules/auth.veld"
```
Paths are relative to the file containing the import. Circular imports are silently skipped.

## Config Auto-Detection

`veld generate` (no flags) searches for config in this order:
1. `./veld.config.json`
2. `./veld/veld.config.json`

All paths in the config file are resolved relative to the config file's directory. So `"out": "../generated"` in `veld/veld.config.json` outputs to the project root's `generated/`.

## .veld Contract Syntax

```
model ModelName {
  fieldName: type    // types: string, int, bool
}

module ModuleName {
  action ActionName METHOD /path {
    input  ModelName    // optional
    output ModelName
    middleware Name     // optional, repeatable
  }
}
```

HTTP methods: `GET POST PUT DELETE PATCH`

## Generated Output Structure

```
generated/
├── types/auth.ts           # TypeScript interfaces for all models
├── interfaces/IAuthService.ts  # Service contract (no dependencies)
├── routes/auth.routes.ts   # Route registration fn — zero runtime deps
└── client/api.ts           # Frontend SDK using native fetch only
```

All generated files begin with `// AUTO-GENERATED BY VELD — DO NOT EDIT`.

Generated routes signature:
```typescript
// router: any — pass Express app, Fastify instance, whatever you use
export function authRouter(router: any, service: IAuthService): void
```

## Type Mapping

| Veld | TypeScript |
|------|-----------|
| `string` | `string` |
| `int` | `number` |
| `bool` | `boolean` |

## Hard Rules

- Veld **never** writes outside the `--out` directory
- Output is **deterministic** — same input always produces identical output
- `veld init` exits with code 1 if already initialised — never overwrites files
- Actions with no `input` generate a handler that reads `req.user?.id` for the service call

## Module: `github.com/veld-dev/veld`

Only external Go dependency: `cobra`. Lexer and parser written from scratch — no parser-generator libraries.
