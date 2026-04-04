# Veld

**Contract-first, multi-stack API code generator.**

Write `.veld` contract files — Veld generates typed TypeScript/JavaScript SDKs and backend service interfaces for Node, Python, Go, Rust, Java, C#, and PHP.

## Installation

```bash
dotnet tool install --global Maayn.Veld
```

## Quick Start

```bash
veld init                  # scaffold a new project
veld generate              # generate all output
veld watch                 # auto-regenerate on file changes
```

## Commands

| Command | Description |
|---------|-------------|
| `veld init` | Scaffold a new project |
| `veld generate` | Generate backend + frontend code |
| `veld validate` | Validate contract files |
| `veld lint` | Analyse contract for quality issues |
| `veld fmt` | Format `.veld` files |
| `veld watch` | Auto-regenerate on file changes |
| `veld openapi` | Export OpenAPI 3.0 spec |
| `veld graphql` | Export GraphQL SDL schema |
| `veld diff` | Show diff vs last generated output |
| `veld doctor` | Diagnose project health |

## Example Contract

```
model User {
  id:    uuid
  name:  string
  email: string
  role:  Role @default(user)
}

enum Role { admin user guest }

module Users {
  prefix: /api/users

  action GetUser {
    method: GET
    path:   /:id
    output: User
  }

  action CreateUser {
    method: POST
    path:   /
    input:  User
    output: User
  }
}
```

## Links

- [GitHub](https://github.com/Adhamzineldin/Veld)
- [Releases](https://github.com/Adhamzineldin/Veld/releases)
- [License: MIT](https://github.com/Adhamzineldin/Veld/blob/master/LICENSE)
