# NexusBank — Veld in a Microservices Architecture

A production-shaped banking platform demonstrating Veld's workspace feature
across 6 polyglot microservices and a React frontend.

---

## Repository Layout

```
NexusBank-System/
│
├── veld/                              ← ONE Veld workspace, all 6 services + frontend
│   ├── veld.config.json               ← workspace array: 7 entries
│   ├── app.veld                       ← frontend entry: import @services/**
│   └── services/                      ← @services alias maps here by default
│       ├── iam/
│       │   ├── models/
│       │   │   └── user.veld          ← User, RegisterInput, LoginInput, TokenPair…
│       │   └── modules/
│       │       └── iam.veld           ← module IAM { prefix:/api/iam … }
│       ├── accounts/
│       │   ├── models/
│       │   │   └── account.veld       ← Account, CreateAccountInput, AccountSummary
│       │   └── modules/
│       │       └── accounts.veld      ← module Accounts { prefix:/api/accounts … }
│       ├── transactions/
│       │   ├── models/
│       │   │   └── transaction.veld   ← Transaction, TransferInput, TransactionListResult
│       │   └── modules/
│       │       └── transactions.veld  ← module Transactions { prefix:/api/transactions … }
│       ├── cards/
│       │   ├── models/
│       │   │   └── card.veld          ← Card, RequestCardInput
│       │   └── modules/
│       │       └── cards.veld         ← module Cards { prefix:/api/cards … }
│       ├── lending/
│       │   ├── models/
│       │   │   └── loan.veld          ← Loan, LoanApplicationInput, LoanPaymentInput
│       │   └── modules/
│       │       └── lending.veld       ← module Lending { prefix:/api/lending … }
│       └── notifications/
│           ├── models/
│           │   └── notification.veld  ← Notification, NotificationPreferences
│           └── modules/
│               └── notifications.veld ← module Notifications { prefix:/api/notifications … }
│
├── backend/
│   ├── iam-service/           Node.js   ← JWT auth, bcrypt, stateful sessions
│   ├── account-service/       Node.js   ← simple account CRUD, same language as IAM
│   ├── transaction-service/   Python    ← financial calculations, future ML/risk models
│   ├── card-service/          Go        ← high-throughput card authorisation, low latency
│   ├── lending-service/       Node.js   ← business logic, loan underwriting rules
│   └── notification-service/  Node.js   ← event-driven, fits Node.js async model
│
├── frontend/                  React + TanStack Query
│   └── src/
│       ├── generated/         ← veld generate --workspace frontend
│       │   ├── client/        ← iamApi.ts, accountsApi.ts, transactionsApi.ts…
│       │   ├── hooks/         ← useListAccounts, useTransfer, useMarkAsRead…
│       │   └── types/         ← User, Account, Transaction, Card, Loan…
│       └── pages/
│           ├── LoginPage.tsx
│           ├── Dashboard.tsx
│           └── TransferPage.tsx
│
├── api-gateway/
│   └── nginx.conf             ← routes /api/<service> → correct container
└── docker-compose.yml
```

---

## How the Workspace Works

### 1 — One config, many services, different languages

`veld/veld.config.json` lists every workspace entry. Each entry can specify its
own `backend`, `out`, and `baseUrl`:

```json
{
  "workspace": [
    {
      "name": "iam",
      "input": "services/iam/modules/iam.veld",
      "backendConfig": { "target": "node-ts", "framework": "express" },
      "out": "../backend/iam-service/generated",
      "baseUrl": "http://iam-service:3001"
    },
    {
      "name": "transactions",
      "input": "services/transactions/modules/transactions.veld",
      "backendConfig": { "target": "python", "framework": "flask" },
      "out": "../backend/transaction-service/generated",
      "baseUrl": "http://transaction-service:3003",
      "consumes": ["iam", "accounts"]
    },
    {
      "name": "cards",
      "input": "services/cards/modules/cards.veld",
      "backendConfig": { "target": "go" },
      "out": "../backend/card-service/generated",
      "baseUrl": "http://card-service:3004",
      "consumes": ["iam", "accounts"]
    },
    {
      "name": "frontend",
      "input": "app.veld",
      "frontendConfig": { "target": "react" },
      "out": "../frontend/src/generated",
      "baseUrl": "http://localhost:3000",
      "consumes": ["*"]
    }
  ]
}
```

`veld generate` in the `veld/` folder runs all 7 entries sequentially.

### 2 — models/ + modules/ per service

Every service is self-contained inside `veld/services/<name>/`:

```
services/iam/
├── models/
│   └── user.veld       ← data types only
└── modules/
    └── iam.veld        ← imports "../models/user.veld" + defines module IAM { … }
```

The module file's workspace entry sets rootDir to `modules/`, so the relative
import `"../models/user.veld"` works naturally — no aliases needed.

### 3 — `import @services/**` for the frontend

`app.veld` (the frontend entry) uses Veld's built-in `@services` alias which
resolves to `veld/services/`. The `**` glob loads every `.veld` file recursively,
giving the React SDK types and hooks for all 6 services in one generation pass.

### 4 — Generated output per service

Each backend service gets its own `generated/` directory:

```
backend/iam-service/
├── src/
│   ├── index.ts                         ← you write: wire routes, middleware
│   ├── middleware/AuthMiddleware.ts      ← you write: implements IMiddleware
│   └── services/IamService.ts           ← you write: implements IIAMService
└── generated/                           ← veld writes this
    ├── types/iam.ts                      ← User, TokenPair, RegisterInput…
    ├── interfaces/IIAMService.ts         ← interface you must implement
    ├── routes/iam.routes.ts              ← iamRouter(router, service, middleware)
    ├── errors/iam.errors.ts              ← iamErrors.login.unauthorized(msg)
    └── middleware/IMiddleware.ts         ← AuthGuard method contract
```

### 5 — Typed errors from the contract

Actions with `errors: [NotFound, Unauthorized]` generate typed error factories.
Throw them in your service — the route handler returns the correct HTTP status.

```ts
// In iam.veld:
//   action Login { errors: [Unauthorized] … }

// In IamService.ts — generated error, not a magic string:
throw iamErrors.login.unauthorized('Invalid credentials');
//    └─ sets res.status(401) in the generated route handler
```

### 6 — Inter-Service SDK (consumes)

When a service declares `"consumes": ["iam", "accounts"]` in the config, Veld
generates typed HTTP client SDKs in that service's language. No manual REST
calls, no guessing the API shape — it's all derived from the consumed service's
`.veld` contract.

**TypeScript** (accounts → iam):
```ts
import { IamClient } from '@veld/generated/sdk/iam';

// Forward the caller's auth token for service-to-service auth
const iam = new IamClient(undefined, { Authorization: req.headers.authorization });
const user = await iam.getProfile();       // fully typed return: User
```

**Python** (transactions → accounts):
```python
from generated.sdk.accounts.client import AccountsClient

accounts = AccountsClient()                # defaults to VELD_ACCOUNTS_URL
account = accounts.get_account(account_id) # typed: Account
```

**Go** (cards → iam, accounts):
```go
import iamsdk "example.com/veld-generated/sdk/iam"

client := iamsdk.NewClient("")  // defaults to VELD_IAM_URL env var
user, err := client.GetProfile(ctx)  // typed: *iam.User
```

**Base URL resolution** (all languages, in priority order):
1. Constructor argument: `IamClient("http://custom:3001")`
2. Environment variable: `VELD_IAM_URL` (convention: `VELD_<UPPER_NAME>_URL`)
3. Baked-in default from the consumed service's `baseUrl` in config

**Dependency graph:**
```
veld deps

  iam → (none)
  accounts → iam
  transactions → iam, accounts
  cards → iam, accounts
  lending → iam, accounts
  notifications → (none)
  frontend → iam, accounts, transactions, cards, lending, notifications
```

### 7 — API Gateway = single frontend baseUrl

All services run on different ports (3001–3006). nginx maps each module's
prefix to the right container. The frontend SDK uses one `baseUrl: http://localhost:3000`:

```
nginx :3000
  /api/iam           → iam-service:3001
  /api/accounts      → account-service:3002
  /api/transactions  → transaction-service:3003
  /api/cards         → card-service:3004
  /api/lending       → lending-service:3005
  /api/notifications → notification-service:3006
```

Because each module's `prefix` matches the nginx location, the generated hooks
hit the right service without any extra configuration in the frontend.

### 8 — Why each language was chosen

| Service             | Language | Why                                                                 |
|---------------------|----------|---------------------------------------------------------------------|
| iam-service         | Node.js  | Rich JWT/bcrypt ecosystem; session management is I/O-bound          |
| account-service     | Node.js  | Simple CRUD; same language as IAM reduces operational overhead      |
| transaction-service | Python   | Financial calculations; extensible to pandas/numpy risk models      |
| card-service        | Go       | Card authorisation is latency-critical; Go's goroutines handle burst |
| lending-service     | Node.js  | Business rule DSL fits well in TypeScript; team knows Node.js       |
| notification-service| Node.js  | Event-driven, async fan-out; Node.js event loop is a natural fit    |

---

## Generating Code

```bash
cd veld

veld generate                          # all 7 workspace entries at once
veld generate --workspace iam          # only iam-service/generated/
veld generate --workspace transactions # only transaction-service/generated/
veld generate --workspace frontend     # only frontend/src/generated/
veld watch                             # regenerate on every .veld file save
```

## Running Locally

```bash
# Generate first
cd veld && veld generate

# Option A — Docker (recommended)
docker compose up --build

# Option B — individual terminals
cd backend/iam-service         && npm install && npm run dev   # :3001
cd backend/account-service     && npm install && npm run dev   # :3002
cd backend/transaction-service && pip install -r requirements.txt && python src/app.py  # :3003
cd backend/card-service        && go run ./cmd/main.go         # :3004
cd backend/lending-service     && npm install && npm run dev   # :3005
cd backend/notification-service&& npm install && npm run dev   # :3006
cd frontend                    && npm install && npm run dev   # :5173
```

## Adding a New Microservice

1. `mkdir -p veld/services/payments/models veld/services/payments/modules`
2. Write `models/payment.veld` and `modules/payments.veld`
3. Add a workspace entry to `veld.config.json`
4. Add an nginx `location` block
5. Add a `docker-compose.yml` service entry
6. `veld generate --workspace payments`

No shared library to update, no code duplication, no drift between teams.
