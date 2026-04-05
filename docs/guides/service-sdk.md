# Service SDK Guide — Inter-Service Communication with Veld

Veld generates **typed, language-native HTTP client SDKs** for inter-service
communication. When your Transaction Service (Python) needs to call your IAM
Service (Node.js), Veld generates a Python client SDK with full type safety
and zero runtime dependencies.

---

## Quick Start

### 1. Add `baseUrl` and `consumes` to your workspace config

```json
{
  "workspace": [
    {
      "name": "iam",
      "input": "services/iam/modules/iam.veld",
      "backend": "node",
      "frontend": "none",
      "out": "../backend/iam-service/generated",
      "baseUrl": "http://iam-service:3001"
    },
    {
      "name": "transactions",
      "input": "services/transactions/modules/transactions.veld",
      "backend": "python",
      "frontend": "none",
      "out": "../backend/transaction-service/generated",
      "baseUrl": "http://transaction-service:3003",
      "consumes": ["iam"]
    }
  ]
}
```

### 2. Generate

```bash
veld generate
```

### 3. Use the generated client

**Python (Transaction Service calling IAM)**:

```python
from generated.sdk.iam import IamClient

iam = IamClient()  # uses VELD_IAM_URL env var or baked-in default

# Fully typed — IDE autocomplete works
tokens = iam.login({"email": "user@example.com", "password": "secret"})
profile = iam.get_profile()
```

**TypeScript (Account Service calling IAM)**:

```typescript
import { IamClient } from './generated/sdk/iam';

const iam = new IamClient(); // uses VELD_IAM_URL or baked-in default

const tokens = await iam.login({ email: 'user@example.com', password: 'secret' });
const profile = await iam.getProfile();
```

**Go (Card Service calling IAM)**:

```go
import "card-service/generated/sdk/iam"

client := iam.NewClient("") // uses VELD_IAM_URL or baked-in default

tokens, err := client.Login(ctx, iam.LoginInput{
    Email:    "user@example.com",
    Password: "secret",
})
profile, err := client.GetProfile(ctx)
```

---

## How It Works

1. **You declare dependencies** via `consumes` in workspace entries
2. **Veld parses both services** — the consumer and all consumed services
3. **Veld generates a typed client** in the consumer's language, placed in `generated/sdk/<service>/`
4. **The client uses native HTTP** — `fetch` (Node), `urllib` (Python), `net/http` (Go)
5. **Zero runtime dependencies** — everything is generated, nothing to install

---

## Generated Output

### TypeScript (`--backend=node`)

```
generated/
├── types/...              ← your service's own types
├── interfaces/...         ← your service's interfaces
├── routes/...             ← your service's route handlers
└── sdk/
    ├── iam/
    │   ├── client.ts      ← IamClient class (fetch-based)
    │   ├── types.ts       ← User, TokenPair, LoginInput
    │   └── index.ts       ← barrel export
    └── index.ts           ← re-exports all service clients
```

### Python (`--backend=python`)

```
generated/
├── models/...
├── interfaces/...
├── routes/...
└── sdk/
    ├── iam/
    │   ├── client.py      ← IamClient class (urllib-based)
    │   ├── types.py       ← TypedDict models
    │   └── __init__.py
    └── __init__.py
```

### Go (`--backend=go`)

```
generated/
├── types/...
├── handlers/...
└── sdk/
    └── iam/
        ├── client.go      ← Client struct (net/http-based)
        ├── types.go       ← Go structs with json tags
        └── doc.go
```

---

## Base URL Resolution

Each generated SDK client resolves the base URL in this priority order:

| Priority | Source | Example |
|----------|--------|---------|
| 1 | Constructor argument | `IamClient(baseUrl="http://...")` |
| 2 | Environment variable | `VELD_IAM_URL=http://iam:3001` |
| 3 | Baked-in default | From the consumed service's `baseUrl` in config |
| 4 | Error | Throws if none of the above are available |

The environment variable convention is: `VELD_<UPPER_SNAKE_NAME>_URL`

| Service Name | Environment Variable |
|--------------|---------------------|
| `iam` | `VELD_IAM_URL` |
| `card-service` | `VELD_CARD_SERVICE_URL` |
| `transactions` | `VELD_TRANSACTIONS_URL` |

---

## CLI Commands

```bash
# Generate everything (including service SDKs for entries with consumes)
veld generate

# Force SDK generation for ALL workspace siblings (even without consumes)
veld generate --service-sdk

# Print the service dependency graph
veld deps

# Validate dependency declarations only
veld deps --validate
```

---

## Validation

Veld validates your `consumes` declarations and catches:

| Issue | Severity | Example |
|-------|----------|---------|
| Unknown service | Error | `consumes: ["auth"]` when no "auth" workspace entry exists |
| Self-consumption | Error | Service "iam" consuming "iam" |
| Circular dependency | Error | A → B → A |
| Missing baseUrl | Warning | Consumed service has no `baseUrl` configured |

---

## Passing Auth Headers

A common pattern is forwarding the incoming request's `Authorization` header
to downstream services:

**TypeScript:**
```typescript
const iam = new IamClient(undefined, {
  Authorization: req.headers.authorization ?? '',
});
```

**Python:**
```python
iam = IamClient(headers={"Authorization": request.headers.get("Authorization", "")})
```

**Go:**
```go
client := iam.NewClient("", iam.WithHeaders(map[string]string{
    "Authorization": r.Header.Get("Authorization"),
}))
```

---

## FAQ

**Q: Can I consume a service that's in a different language?**  
A: Yes! That's the whole point. Your Python service can consume a Node.js
service — Veld generates a Python client from the Node service's `.veld` contract.

**Q: What about WebSocket actions?**  
A: WebSocket actions are skipped in service SDKs. They're HTTP-only for now.

**Q: Do I need to install any packages?**  
A: No. All generated clients use native HTTP libraries (fetch, urllib, net/http).

**Q: What if the consumed service's contract changes?**  
A: Re-run `veld generate`. The SDK is regenerated from the contract. Veld's
breaking change detection (`veld diff`) will flag any incompatible changes.

