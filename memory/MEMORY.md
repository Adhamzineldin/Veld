# Veld Project Memory

## What is Veld
Contract-first, multi-stack API code generator written in Go. Reads `.veld` files, parses them into an AST, and generates typed backend routes + frontend SDK.

## Key Non-Negotiable Rules (in CLAUDE.md)
1. ZERO runtime dependencies in generated output
2. Always agnostic — `router: any`, native fetch, no lock-in
3. `--backend=node` (not express), `--backend=python`

## Go module path
`github.com/veld-dev/veld`

## Build & Run
```bash
export PATH="/c/Program Files/Go/bin:$PATH"
go build -o veld.exe ./cmd/veld
cd testapp && ../veld.exe generate
cd testapp/python-backend && ../../veld.exe generate   # Python output
```

## Architecture

### Pipeline
`.veld file` → Lexer → Tokens → Parser → AST → Validator → Emitter → files

### Key packages
- `internal/lexer` — tokenizes .veld source
- `internal/parser` — recursive descent parser → `ast.AST`
- `internal/validator` — semantic checks (duplicate names, undefined type refs)
- `internal/emitter/node` — generates TS types, interfaces, zero-dep routes
- `internal/emitter/typescript` — generates frontend fetch SDK (`client/api.ts`)
- `internal/emitter/python` — generates Python TypedDict, ABC interfaces, Flask routes
- `cmd/veld/main.go` — CLI (cobra), config resolution, recursive import loading

### .veld language features
- `model Name { field: type }` — types: `string`, `int`, `bool`, `[]string`, `[]int`, `[]bool`, `[]ModelName`
- `module Name { action Name METHOD /path { input X / output Y / middleware Z } }`
- `import "relative/path.veld"` — recursive imports with circular guard

### Config
Searched in order: `./veld.config.json`, `./veld/veld.config.json`
```json
{ "input": "schema.veld", "backend": "node", "frontend": "react", "out": "../generated" }
```

### testapp structure
```
testapp/
  veld/               ← contract source (veld.config.json here)
    models/auth.veld, models/food.veld
    modules/auth.veld, modules/food.veld
    schema.veld       ← imports all models+modules
  generated/          ← .gitignored, auto-generated
    types/*.ts, *.py
    interfaces/*.ts, *.py
    routes/*.ts, *.py
    client/api.ts
  backend/src/        ← Node.js (Express) test backend
    server.ts, services/AuthService.ts, services/FoodService.ts
  frontend/src/       ← TypeScript frontend test
    main.ts
  python-backend/     ← Python Flask test backend
    server.py, services/*.py, veld.config.json (backend: python)
```

## Python emitter notes
- Generates TypedDict types, ABC interfaces, Flask routes using `app.add_url_rule`
- `from flask import request, jsonify` is allowed in routes (unavoidable with Flask)
- Route registrar signature: `def register_{module}_routes(app, service)`
- `__init__.py` files created automatically so `generated/` is a Python package
