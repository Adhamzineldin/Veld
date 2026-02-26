# My Veld Project

## Structure

| Path | Owner | Purpose |
|------|-------|--------|
| `veld/` | You | Contract source — models, modules, config |
| `veld/models/` | You | Data type definitions |
| `veld/modules/` | You | API endpoint definitions |
| `generated/` | Veld | Auto-generated — do not edit |
| `app/` | You | Business logic — never overwritten |

## Workflow

1. Edit files in `veld/models/` and `veld/modules/`
2. Run `veld generate` to regenerate `generated/`
3. Implement interfaces in `app/services/`
4. Import the SDK in your frontend from `generated/client/api.ts`

## Import system

Split your contract across as many files as you like:

```
// veld/schema.veld
import "models/auth.veld"
import "models/product.veld"
import "modules/auth.veld"
import "modules/products.veld"
```

## Commands

| Command | Description |
|---------|-------------|
| `veld generate` | Regenerate from veld/schema.veld |
| `veld validate` | Check contract for errors |
| `veld ast` | Dump AST JSON for debugging |
