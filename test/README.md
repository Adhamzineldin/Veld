# Veld Test Suite

## test/good/ — Valid contract (should pass validate + generate)

Exercises **every** feature:

| Feature | Where |
|---------|-------|
| `import` system | `schema.veld` |
| `enum` definitions | `models/user.veld` — `Role`, `Status` |
| `model description` | `models/user.veld` — `User`, `AuthResponse` |
| `module description` | `modules/users.veld`, `modules/auth.veld` |
| `action description` | every action |
| `optional fields` (`?`) | `bio?`, `avatarUrl?`, `age?`, `rating?`, `birthDate?`, `message?` |
| `@default(value)` | `role: Role @default(user)`, `verified: bool @default(false)` |
| `string` type | everywhere |
| `int` type | `age`, `limit`, `offset`, `code` |
| `float` type | `rating` |
| `bool` type | `verified`, `success` |
| `date` type | `birthDate` |
| `datetime` type | `createdAt` |
| `uuid` type | `id` |
| Array suffix `Type[]` | `tags: string[]`, `friends: User[]`, `output User[]` |
| `query` keyword | `modules/users.veld` — `action List` |
| `prefix` (route prefix) | `modules/users.veld` — `prefix /api` |
| `middleware` | `AuthGuard`, `RateLimit`, `LogRequest` |
| Multiple middleware | `modules/auth.veld` — Login has `RateLimit` + `LogRequest` |
| All HTTP methods | GET, POST, PUT, DELETE, PATCH in `modules/users.veld` |
| Model references | `AuthResponse.user: User` |
| Transitive model deps | `AuthResponse` pulls in `User` which pulls in `Role`, `Status` |

### Run it

```bash
cd test/good
../../veld validate           # should print ✓ Contract is valid
../../veld generate           # node backend
../../veld generate --backend python
```

---

## test/bad/ — Invalid contracts (should each fail with a clear error)

| File | What it tests |
|------|---------------|
| `undefined_type.veld` | Unknown type `Usr` in field → "did you mean User?" |
| `duplicate_model.veld` | Two models named `User` → duplicate model error |
| `duplicate_enum.veld` | Two enums named `Role` → duplicate enum error |
| `empty_enum.veld` | Enum with zero values → empty enum error |
| `undefined_action_types.veld` | `input FakeInput` / `output FakeOutput` → undefined type |
| `undefined_query.veld` | `query FakeFilters` → undefined query type |
| `duplicate_action.veld` | Two actions named `Get` → duplicate action error |
| `bad_default_type.veld` | `@default("three")` on `int` field → type mismatch |
| `bad_default_enum.veld` | `@default(yellow)` on `Color` enum → invalid enum value |
| `missing_brace.veld` | Missing `}` → parser error |
| `unexpected_char.veld` | `$` in source → lexer error |
| `missing_http_method.veld` | Action without `method:` field → missing required field error |
| `name_collision.veld` | Enum and model share same name → collision error |
| `duplicate_field.veld` | Two fields named `id` → duplicate field error |

### Run them all

```powershell
cd test/bad
Get-ChildItem *.veld | ForEach-Object {
  Write-Host "`n=== $($_.Name) ===" -ForegroundColor Cyan
  & ..\..\veld.exe validate $_.Name 2>&1
}
```

