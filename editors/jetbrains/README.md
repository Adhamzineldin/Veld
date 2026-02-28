# Veld JetBrains Plugin

Official Veld language support for **all JetBrains IDEs** including IntelliJ IDEA, WebStorm, PyCharm, PhpStorm, GoLand, RubyMine, CLion, DataGrip, Rider, and Android Studio.

## Features

### ✅ Syntax Highlighting
- Keywords: `model`, `module`, `action`, `enum`, `import`, `extends`
- Types: `string`, `int`, `float`, `bool`, `date`, `datetime`, `uuid`, `List<T>`, `Map<K,V>`
- Directives: `method`, `path`, `input`, `output`, `description`, `prefix`
- HTTP Methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`
- Comments: `// single-line comments`

### ✅ Code Completion
- Auto-complete keywords, types, and directives
- Suggestions for HTTP methods
- Built-in type completion

### ✅ Validation
- Automatic validation on file save
- Error highlighting with inline messages
- Integrated with `veld validate` CLI

### ✅ Quick Actions
- **Veld: Validate Contract** (`Ctrl+Alt+V`)
- **Veld: Generate Code** (`Ctrl+Alt+G`)
- **Veld: Generate (Dry Run)**

### ✅ Code Style
- Configurable indentation (default: 2 spaces)
- Automatic formatting support

### ✅ IDE Features
- Comment/uncomment with `Ctrl+/`
- Brace matching for `{}` and `<>`
- File type recognition for `.veld` files

## Requirements

You must have the **Veld CLI** installed and available on your PATH:

```bash
# Install via Go
go install github.com/veld-dev/veld@latest

# Or download binary from releases
# https://github.com/veld-dev/veld/releases
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

1. Download the plugin `.zip` file from [Releases](https://github.com/veld-dev/veld/releases)
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
- **Manual**: **Tools** → **Veld** → **Validate Contract**
- **Keyboard**: `Ctrl+Alt+V` (Windows/Linux) or `Cmd+Alt+V` (macOS)

### 3. Generate Code

- **Menu**: **Tools** → **Veld** → **Generate Code**
- **Keyboard**: `Ctrl+Alt+G` (Windows/Linux) or `Cmd+Alt+G` (macOS)
- **Context Menu**: Right-click in editor → **Veld** → **Generate Code**

Or run in terminal:
```bash
veld generate --backend=go -o ./backend
```

## Supported IDEs

This plugin works in **all JetBrains IDEs** version 2023.1+:

- ✅ **IntelliJ IDEA** (Community & Ultimate)
- ✅ **WebStorm**
- ✅ **PyCharm** (Community & Professional)
- ✅ **PhpStorm**
- ✅ **GoLand**
- ✅ **RubyMine**
- ✅ **CLion**
- ✅ **DataGrip**
- ✅ **Rider**
- ✅ **Android Studio**
- ✅ **AppCode** (if still supported)

## Keyboard Shortcuts

| Action | Windows/Linux | macOS |
|--------|---------------|-------|
| Validate | `Ctrl+Alt+V` | `Cmd+Alt+V` |
| Generate | `Ctrl+Alt+G` | `Cmd+Alt+G` |
| Comment/Uncomment | `Ctrl+/` | `Cmd+/` |

## Configuration

No additional configuration needed! The plugin automatically:
- Detects `.veld` files
- Runs validation on save
- Uses the `veld` CLI from your PATH

## Supported Backends

The Veld CLI supports multiple backend languages:
- **Node.js** (Express)
- **Python** (Flask)
- **Go** (Chi/Gin)
- **Rust** (Axum/Actix) — Coming soon
- **Java/Kotlin** (Spring Boot) — Coming soon
- **C#** (ASP.NET Core) — Coming soon
- **PHP** (Laravel) — Coming soon

## Building the Plugin

### Prerequisites
- JDK 17+
- Gradle 8.5+

### Build
```bash
cd editors/jetbrains
./gradlew buildPlugin
```

The plugin ZIP will be in `build/distributions/veld-jetbrains-0.1.0.zip`

### Run in IDE (Development)
```bash
./gradlew runIde
```

This opens a new IDE instance with the plugin installed for testing.

### Verify
```bash
./gradlew verifyPlugin
```

## Publishing

See [PUBLISHING.md](PUBLISHING.md) for detailed publishing instructions.

Quick publish:
```bash
export ORG_GRADLE_PROJECT_intellijPublishToken=YOUR_TOKEN_HERE
./gradlew publishPlugin
```

## Troubleshooting

### Plugin not recognizing .veld files
- Make sure file extension is exactly `.veld`
- Try **File** → **Invalidate Caches / Restart**

### Validation not working
- Verify `veld` is installed: `veld --version`
- Make sure `veld` is on your PATH
- Check IDE logs: **Help** → **Show Log in Explorer/Finder**

### Actions not appearing in menu
- Restart IDE after installation
- Check **Tools** → **Veld** menu

### Syntax highlighting not working
- Close and reopen the `.veld` file
- Try **File** → **Invalidate Caches / Restart**

## Known Issues

- Multi-line comments are not yet supported (only `//` single-line)
- Jump-to-definition not implemented yet (planned for v0.2)
- Find usages not implemented yet (planned for v0.2)

## Contributing

Found a bug or want to contribute? Visit our [GitHub repository](https://github.com/veld-dev/veld).

## Release Notes

### 0.1.0 (Initial Release)

- Syntax highlighting for all Veld constructs
- Code completion for keywords, types, and directives
- Validation on save with inline diagnostics
- Quick actions for validate and generate
- Support for all JetBrains IDEs 2023.1+
- Keyboard shortcuts
- Code style configuration
- Brace matching and commenting

---

## Related Projects

- [Veld CLI](https://github.com/veld-dev/veld) - The main Veld compiler
- [Veld VS Code Extension](https://marketplace.visualstudio.com/items?itemName=veld-dev.veld-vscode)

---

**Enjoy using Veld in your favorite JetBrains IDE!** 🚀

