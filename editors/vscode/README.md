# Veld VS Code Extension

Syntax highlighting, snippets, code completion, and validation for the [Veld contract language](https://github.com/Adhamzineldin/Veld).

## Features

### ✅ Syntax Highlighting
- Keywords: `model`, `module`, `action`, `enum`, `import`, `extends`, `from`
- Types: `string`, `int`, `long`, `float`, `decimal`, `bool`, `date`, `datetime`, `time`, `uuid`, `bytes`, `json`, `any`, `List<T>`, `Map<K,V>`
- Directives: `method`, `path`, `input`, `output`, `description`, `prefix`, `query`, `middleware`, `errors`, `deprecated`
- HTTP Methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `WS`
- Annotations: `@default`, `@example`, `@unique`, `@index`, `@relation`, `@required`, `@deprecated`, `@min`, `@max`, `@minLength`, `@maxLength`, `@regex`
- Comments: `// single-line` and `/* block comments */`
- Import paths: `@models/user`, `@modules/*`

### ✅ Code Snippets

| Prefix | Description |
|--------|-------------|
| `model` | Model declaration |
| `modeld` | Model with description |
| `modele` | Model with `extends` |
| `enum` | Enum declaration |
| `module` | Module with one action |
| `action` | Action with method, path, input/output |
| `crud` | Full CRUD action set (List, Get, Create, Update, Delete) |
| `import` | Import using `@alias` path |
| `importp` | Import using relative path |
| `from` | `from @alias import *` syntax |
| `field?` | Optional field |
| `field[]` | Array field |
| `fieldmap` | Map field |
| `fielddef` | Field with `@default` |
| `fieldunique` | Field with `@unique` |
| `fieldindex` | Field with `@index` |
| `fieldexample` | Field with `@example` |
| `fieldrelation` | Field with `@relation` |
| `block` | Block comment `/* */` |

### ✅ Validation on Save
Automatically runs `veld validate` when you save a `.veld` file and displays errors inline.

### ✅ Commands
- **Veld: Validate Contract** — Manually validate your schema
- **Veld: Generate Code** — Run code generation
- **Veld: Generate (Dry Run)** — Preview what would be generated

### ✅ Config Schema
JSON schema autocompletion and validation for `veld.config.json` files — autocompletes backend/frontend options, aliases, output paths, and all config keys.

## Requirements

You must have the **Veld CLI** installed and available on your PATH:

```bash
# npm
npm install -g @maayn/veld

# pip
pip install maayn-veld

# Go
go install github.com/Adhamzineldin/Veld/cmd/veld@latest

# Homebrew
brew install adhamzineldin/tap/veld

# Or download binary from releases
# https://github.com/Adhamzineldin/Veld/releases
```

Verify installation:
```bash
veld --version
```

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `veld.executablePath` | `veld` | Path to the veld CLI executable |
| `veld.validateOnSave` | `true` | Automatically validate `.veld` files on save |

## Usage

### 1. Create a `.veld` file

```veld
import @models/user
import @models/common

module Users {
  description: "User management"
  prefix: /api/users

  action ListUsers {
    method: GET
    path: /
    query: ListQuery
    output: User[]
  }

  action CreateUser {
    method: POST
    path: /
    input: CreateUserInput
    output: User
    middleware: RequireAuth
  }
}
```

### 2. Validate

- **Automatic**: Save the file (errors appear inline)
- **Manual**: Press `Ctrl+Shift+P` → `Veld: Validate Contract`

### 3. Generate Code

Press `Ctrl+Shift+P` → `Veld: Generate Code`

Or run in terminal:
```bash
veld generate
```

## Supported Backends & Frontends

**Backends:** `node-ts`, `node-js`, `python`, `go`, `rust`, `java`, `csharp`, `php`

**Frontends:** `typescript`, `javascript`, `react`, `vue`, `angular`, `svelte`, `dart`, `kotlin`, `swift`, `types-only`, `none`

## Release Notes

### 0.2.0

- Block comments `/* */` — syntax highlighting and toggle support
- New annotation snippets: `@example`, `@unique`, `@index`, `@relation`
- Updated config schema with all backend/frontend aliases and `validate` field
- Block comment snippet (`block`)
- Import path highlighting for `@alias/path` syntax

### 0.1.0 (Initial Release)

- Syntax highlighting for all Veld constructs
- Code snippets for common patterns
- Validation on save with inline diagnostics
- Commands for validation and generation

## Contributing

Found a bug or want to contribute? Visit our [GitHub repository](https://github.com/Adhamzineldin/Veld).

---

**Enjoy using Veld!** 🚀
