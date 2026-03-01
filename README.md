# My Veld Project

## Structure

| Path | Purpose |
|------|--------|
| `veld/` | Contract source — models, modules, config |
| `veld/models/` | Data type definitions (models, enums) |
| `veld/modules/` | API endpoint definitions |
| `generated/` | Auto-generated code — do not edit |

## Import System

Every file must explicitly import the files that define the types it uses:

```veld
// veld/app.veld — imports modules
import @modules/users
import @modules/auth
```

```veld
// veld/modules/users.veld — imports its own models
import @models/user
import @models/common

module Users { ... }
```

Import paths don't include `.veld` — the parser adds it automatically.

## Middleware

Middleware names (like `RequireAuth`, `RateLimit`) are just labels in the contract.
Veld generates typed middleware interfaces — you provide the implementations
when registering routes in your app.

## Commands

| Command | Description |
|---------|-------------|
| `veld generate` | Generate typed code |
| `veld validate` | Check contract for errors |
| `veld watch` | Auto-regenerate on file save |
| `veld clean` | Remove generated output |
| `veld openapi` | Export OpenAPI 3.0 spec |
| `veld ast` | Dump AST JSON for debugging |
