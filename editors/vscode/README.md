# Veld VS Code Extension

Syntax highlighting, snippets, and validation for the [Veld contract language](https://github.com/Adhamzineldin/Veld).

## Features

### ✅ Syntax Highlighting
- Keywords: `model`, `module`, `action`, `enum`, `import`, `extends`
- Types: `string`, `int`, `float`, `bool`, `date`, `datetime`, `uuid`, `List<T>`, `Map<K,V>`
- Directives: `@middleware`, `@query`, `@auth`
- Comments: `// single-line comments`

### ✅ Code Snippets
- **`model`** — Create a model structure
- **`modele`** — Model with inheritance (`extends`)
- **`enum`** — Define an enum
- **`module`** — Create a module with actions
- **`action`** — Define an action with HTTP method
- **`crud`** — Generate full CRUD action set
- **`import`** — Import another .veld file

### ✅ Validation on Save
Automatically runs `veld validate` when you save a `.veld` file and displays errors inline.

### ✅ Commands
- **Veld: Validate Contract** — Manually validate your schema
- **Veld: Generate Code** — Run code generation
- **Veld: Generate (Dry Run)** — Preview what would be generated

## Requirements

You must have the **Veld CLI** installed and available on your PATH:

```bash
# npm (recommended)
npm install @maayn/veld

# pip
pip install maayn-veld

# Go
go install github.com/Adhamzineldin/Veld/cmd/veld@latest

# Or download binary from releases
# https://github.com/Adhamzineldin/Veld/releases
```

Verify installation:
```bash
veld --version
```

## Extension Settings

This extension contributes the following settings:

* `veld.executablePath`: Path to the veld CLI executable (default: `veld`)
* `veld.validateOnSave`: Automatically validate `.veld` files on save (default: `true`)

## Usage

### 1. Create a `.veld` file

```veld
// models/user.veld
model User {
  id: int
  email: string
  name: string
  createdAt: datetime
}

module users {
  description: "User management"
  prefix: /api/users

  action ListUsers {
    method: GET
    path: /
    output: List<User>
  }

  action GetUser {
    method: GET
    path: /:id
    output: User
  }

  action CreateUser {
    method: POST
    path: /
    input: User
    output: User
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
veld generate --backend=go -o ./backend
```

## Supported Backends

The Veld CLI supports multiple backend languages:
- **Node.js** (Express)
- **Python** (Flask)
- **Go** (Chi/Gin)
- **Rust** (Axum/Actix) — Coming soon
- **Java/Kotlin** (Spring Boot) — Coming soon
- **C#** (ASP.NET Core) — Coming soon
- **PHP** (Laravel) — Coming soon

## Known Issues

- Multi-line comments are not yet supported (only `//` single-line)
- Jump-to-definition not implemented yet (planned for v0.2)
- Auto-completion for model names not implemented yet (planned for v0.2)

## Contributing

Found a bug or want to contribute? Visit our [GitHub repository](https://github.com/Adhamzineldin/Veld).

## Release Notes

### 0.1.0 (Initial Release)

- Syntax highlighting for all Veld constructs
- Code snippets for common patterns
- Validation on save with inline diagnostics
- Commands for validation and generation
- Configuration options for CLI path and auto-validation

---

**Enjoy using Veld!** 🚀

