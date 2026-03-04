# Veld JetBrains Plugin

Official Veld language support for **all JetBrains IDEs** including IntelliJ IDEA, WebStorm, PyCharm, PhpStorm, GoLand, RubyMine, CLion, DataGrip, Rider, and Android Studio.

## Features

### ✅ Syntax Highlighting
- Keywords: `model`, `module`, `action`, `enum`, `import`, `extends`, `from`
- Distinct colors for each keyword type (model vs module vs action vs enum)
- Types: `string`, `int`, `float`, `bool`, `date`, `datetime`, `uuid`, `List<T>`, `Map<K,V>`
- Directives: `method`, `path`, `input`, `output`, `description`, `prefix`, `query`, `middleware`, `errors`, `deprecated`
- HTTP Methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `WS`
- Annotations: `@default`, `@example`, `@unique`, `@index`, `@relation`, `@required`, `@deprecated`, `@min`, `@max`, `@minLength`, `@maxLength`, `@regex`
- Comments: `// single-line` and `/* block comments */`
- Import paths: `@models/user`, `@modules/*`
- Path literals with param highlighting: `/users/:id`

### ✅ Context-Aware Code Completion
- **Top level**: `model`, `module`, `enum`, `import`, `from`, `prefix`
- **Inside models**: all built-in types + custom model/enum names from imports
- **Inside modules**: `action`, `description`, `prefix`
- **Inside actions**: `method`, `path`, `input`, `output`, `query`, `middleware`, `errors`, `deprecated`
- **After `method:`**: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `WS`
- **After `@`**: all annotations with parameter templates — `@default()`, `@example()`, `@unique`, `@index`, `@relation()`, `@min()`, `@max()`, etc.
- **After type colon**: built-in types + all model/enum names resolved from imports

### ✅ Cross-File Resolution
- Resolves `import @models/user` to the actual file
- Reads models and enums from imported files for completion
- Supports `@alias/path`, relative `./path`, and wildcard `@models/*` imports

### ✅ Validation
- Automatic validation on file save via `veld validate`
- Error highlighting with inline messages
- Gutter icons for validation status

### ✅ Structure View
- Navigate models, modules, enums, and actions in the Structure tool window
- Sorted by declaration order

### ✅ Documentation Provider
- Hover over models, enums, and fields to see descriptions
- Shows field types, optionality, default values

### ✅ Quick Actions
- **Veld: Validate Contract** (`Ctrl+Alt+V`)
- **Veld: Generate Code** (`Ctrl+Alt+G`)
- **Veld: Generate (Dry Run)**

### ✅ Code Style & Editing
- Comment/uncomment line: `Ctrl+/`
- Block comment toggle: `Ctrl+Shift+/` — wraps selection in `/* */`
- Brace matching for `{}`, `<>`, `()`, `[]`
- Configurable indentation (default: 2 spaces)
- Auto-close braces, brackets, and quotes

### ✅ Config Schema
JSON schema for `veld.config.json` files — autocompletion and validation for all config keys.

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

## Installation

### From JetBrains Marketplace (Recommended)

1. Open your JetBrains IDE
2. Go to **Settings/Preferences** → **Plugins**
3. Search for **"Veld"**
4. Click **Install**
5. Restart IDE

### From Disk

1. Download the plugin `.zip` file from [Releases](https://github.com/Adhamzineldin/Veld/releases)
2. Go to **Settings/Preferences** → **Plugins**
3. Click **⚙️** → **Install Plugin from Disk...**
4. Select the downloaded `.zip` file
5. Restart IDE

### Build from Source

```bash
cd editors/jetbrains
./gradlew buildPlugin
# Plugin will be in build/distributions/
```

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
- **Manual**: **Tools** → **Veld** → **Validate Contract**
- **Keyboard**: `Ctrl+Alt+V` (Windows/Linux) or `Cmd+Alt+V` (macOS)

### 3. Generate Code

- **Menu**: **Tools** → **Veld** → **Generate Code**
- **Keyboard**: `Ctrl+Alt+G` (Windows/Linux) or `Cmd+Alt+G` (macOS)
- **Context Menu**: Right-click in editor → **Veld** → **Generate Code**

Or run in terminal:
```bash
veld generate
```

## Supported IDEs

This plugin works in **all JetBrains IDEs** version 2023.1+:

| IDE | Status |
|-----|--------|
| IntelliJ IDEA (Community & Ultimate) | ✅ |
| WebStorm | ✅ |
| PyCharm (Community & Professional) | ✅ |
| PhpStorm | ✅ |
| GoLand | ✅ |
| RubyMine | ✅ |
| CLion | ✅ |
| DataGrip | ✅ |
| Rider | ✅ |
| Android Studio | ✅ |

## Keyboard Shortcuts

| Action | Windows/Linux | macOS |
|--------|---------------|-------|
| Validate | `Ctrl+Alt+V` | `Cmd+Alt+V` |
| Generate | `Ctrl+Alt+G` | `Cmd+Alt+G` |
| Line Comment | `Ctrl+/` | `Cmd+/` |
| Block Comment | `Ctrl+Shift+/` | `Cmd+Shift+/` |

## Supported Backends & Frontends

**Backends:** `node-ts`, `node-js`, `python`, `go`, `rust`, `java`, `csharp`, `php`

**Frontends:** `typescript`, `javascript`, `react`, `vue`, `angular`, `svelte`, `dart`, `kotlin`, `swift`, `types-only`, `none`

## Building the Plugin

### Prerequisites
- JDK 17+
- Gradle 8.5+

### Build
```bash
cd editors/jetbrains
./gradlew buildPlugin
```

The plugin ZIP will be in `build/distributions/`

### Run in IDE (Development)
```bash
./gradlew runIde
```

### Verify
```bash
./gradlew verifyPlugin
```

## Publishing

See [PUBLISHING.md](PUBLISHING.md) for detailed publishing instructions.

```bash
export ORG_GRADLE_PROJECT_intellijPublishToken=YOUR_TOKEN_HERE
./gradlew publishPlugin
```

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Plugin not recognizing `.veld` files | Ensure file extension is `.veld`. Try **File → Invalidate Caches / Restart** |
| Validation not working | Verify `veld --version` works. Check IDE logs (**Help → Show Log**) |
| Actions not in Tools menu | Restart IDE after installation |
| Completions missing model names | Ensure your file has `import @models/...` at the top |

## Release Notes

### 0.2.0

- Block comments `/* */` — syntax highlighting, lexer support, and `Ctrl+Shift+/` toggle
- New annotation completions: `@example`, `@index`, `@relation`
- Context-aware `@` annotation completions with parameter templates
- Cross-file model/enum resolution from imports
- Structure view for navigating declarations
- Documentation provider (hover to see descriptions)
- Updated config schema with all backend/frontend aliases and `validate` field
- Path literal highlighting with `:param` coloring

### 0.1.0 (Initial Release)

- Syntax highlighting for all Veld constructs
- Code completion for keywords, types, and directives
- Validation on save with inline diagnostics
- Quick actions for validate and generate
- Support for all JetBrains IDEs 2023.1+

## Contributing

Found a bug or want to contribute? Visit our [GitHub repository](https://github.com/Adhamzineldin/Veld).

---

**Enjoy using Veld in your favorite JetBrains IDE!** 🚀
