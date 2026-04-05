# Getting Started with Veld

## Prerequisites

- [Go 1.22+](https://go.dev/dl/) (for building from source)
- OR install via npm/pip/Homebrew (no Go needed)

## 1. Install

Pick any method:

```bash
# Go
go install github.com/Adhamzineldin/Veld/cmd/veld@latest

# npm
npm install @maayn/veld

# pip
pip install maayn-veld

# Homebrew
brew install maayn-veld/tap/maayn-veld

# Composer
composer require maayn/veld
```

Verify:
```bash
veld --version
```

## 2. Create a Project

```bash
mkdir my-api
cd my-api
veld init
```

This creates:
```
my-api/
└── veld/
    ├── veld.config.json     ← config
    ├── app.veld             ← entry point (imports everything)
    ├── models/
    │   ├── user.veld        ← User, Role, Status, LoginInput, etc.
    │   └── common.veld      ← PaginatedResponse, ErrorResponse
    └── modules/
        ├── users.veld       ← CRUD endpoints
        └── auth.veld        ← Login, Register, Me, Logout
```

## 3. Generate Code

```bash
veld generate
```

Output:
```
✓ Generated → ./generated
  types/       users.ts, auth.ts, index.ts
  interfaces/  IUsersService.ts, IAuthService.ts
  routes/      users.routes.ts, auth.routes.ts
  schemas/     schemas.ts
  client/      api.ts
```

## 4. Use the Generated Code

### Backend (Node.js)

```typescript
import { registerUsersRoutes } from './generated/routes/users.routes';
import { IUsersService } from './generated/interfaces/IUsersService';

const service: IUsersService = {
  async List(filters) { /* your logic */ },
  async GetById(id)   { /* your logic */ },
  async Update(id, input) { /* your logic */ },
  async Delete(id) { /* your logic */ },
};

registerUsersRoutes(router, service);
```

### Frontend (TypeScript SDK)

```typescript
import { api } from './generated/client/api';

const user = await api.auth.Login({ email: 'x@y.com', password: '...' });
const users = await api.users.List({ role: 'admin' });
```

## 5. Watch for Changes

During development, auto-regenerate on file save:

```bash
veld watch
```

## 6. Validate Contracts

Check for errors without generating:

```bash
veld validate
```

## 7. Export OpenAPI Spec

```bash
veld openapi -o openapi.json
```

## Next Steps

- Edit `veld/models/` to add your own types
- Edit `veld/modules/` to define your API endpoints
- Try `--backend=python` or `--backend=go` for other languages
- Read [CLAUDE.md](../CLAUDE.md) for the full language spec
- Read [docs/roadmap.md](roadmap.md) for upcoming features

