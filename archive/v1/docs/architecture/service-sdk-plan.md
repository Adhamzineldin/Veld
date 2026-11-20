# Service SDK Generation — Architecture & Implementation Plan

**Status:** Planned  
**Author:** Auto-generated  
**Date:** April 5, 2026  
**Priority:** Critical — Elevates Veld from code generator to microservices platform

---

## 1. Executive Summary

Generate **typed, language-native HTTP client SDKs** so any backend service can call any other backend service with full type safety and zero runtime dependencies. This is the "frontend SDK, but for every backend language."

**Core idea:** When workspace entry `transactions` (Python) declares `"consumes": ["iam"]`, Veld parses the IAM `.veld` contract, then generates a Python HTTP client (`sdk/iam_client.py`) inside the Transactions service's output directory, using only native `urllib`.

### Before (today)
```
Transaction Service (Python) → needs to call IAM Service (Node.js)
❌ Developer manually writes HTTP calls, no type safety, no contract enforcement
```

### After (with Service SDK)
```
Transaction Service (Python) → imports generated/sdk/iam_client.py
✅ Fully typed, auto-generated, contract-enforced, zero dependencies
```

---

## 2. Design Principles

| # | Principle | Rationale |
|---|-----------|-----------|
| 1 | **Zero runtime deps** | Generated SDKs use native HTTP clients: `fetch` (Node), `urllib` (Python), `net/http` (Go), etc. |
| 2 | **Contract-driven** | SDKs are generated from `.veld` contracts, not runtime service discovery |
| 3 | **Config-only dependencies** | `consumes` lives in `veld.config.json`, not in `.veld` syntax — dependencies are operational, not contractual |
| 4 | **Language-native idioms** | Each SDK follows the conventions of its target language (dataclasses in Python, structs in Go, records in Java) |
| 5 | **Deterministic output** | Same input always produces identical SDK files |
| 6 | **Backward compatible** | Existing `serverSdk: true` continues working; `consumes` is opt-in |

---

## 3. Configuration Design

### 3.1 `veld.config.json` — `consumes` field

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
      "name": "accounts",
      "input": "services/accounts/modules/accounts.veld",
      "backend": "node",
      "frontend": "none",
      "out": "../backend/account-service/generated",
      "baseUrl": "http://account-service:3002",
      "consumes": ["iam"]
    },
    {
      "name": "transactions",
      "input": "services/transactions/modules/transactions.veld",
      "backend": "python",
      "frontend": "none",
      "out": "../backend/transaction-service/generated",
      "baseUrl": "http://transaction-service:3003",
      "consumes": ["iam", "accounts"]
    },
    {
      "name": "cards",
      "input": "services/cards/modules/cards.veld",
      "backend": "go",
      "frontend": "none",
      "out": "../backend/card-service/generated",
      "baseUrl": "http://card-service:3004",
      "consumes": ["iam", "accounts"]
    }
  ]
}
```

**Rules:**
- `consumes` is an array of workspace entry names
- Each consumed service must exist as a workspace entry
- Circular consumption is an error (A→B→A)
- Self-consumption is an error (A→A)
- A consumed service's `baseUrl` becomes the default in the generated client

### 3.2 `WorkspaceEntry` struct changes

```go
type WorkspaceEntry struct {
    Name      string   `json:"name"`
    Input     string   `json:"input"`
    Backend   string   `json:"backend,omitempty"`
    Frontend  string   `json:"frontend,omitempty"`
    Out       string   `json:"out,omitempty"`
    BaseUrl   string   `json:"baseUrl,omitempty"`
    ServerSdk bool     `json:"serverSdk,omitempty"`
    Consumes  []string `json:"consumes,omitempty"` // NEW: workspace entry names this service depends on
}
```

### 3.3 Runtime Base URL Resolution

Generated SDK clients resolve base URLs in this order (highest priority first):

1. **Constructor argument**: `IAMClient(baseUrl="http://...")` 
2. **Environment variable**: `VELD_IAM_URL` (convention: `VELD_<UPPER_SNAKE_NAME>_URL`)
3. **Baked-in default**: The `baseUrl` from the consumed service's workspace entry (compile-time)
4. **Error**: If none of the above, throw/panic at instantiation time

---

## 4. AST & EmitOptions Changes

### 4.1 No AST changes needed

Dependencies are operational (config), not contractual (syntax). The parser and lexer remain untouched.

### 4.2 New types in `emitter.go`

```go
// ConsumedServiceInfo is the resolved representation of a consumed service,
// ready for SDK generation. Passed to emitters via EmitOptions.
type ConsumedServiceInfo struct {
    Name    string  // workspace entry name (e.g., "iam")
    AST     ast.AST // fully parsed AST of the consumed service
    BaseUrl string  // default base URL (may be empty)
}
```

### 4.3 EmitOptions addition

```go
type EmitOptions struct {
    // ...existing fields...
    ConsumedServices []ConsumedServiceInfo // services this entry depends on (for SDK generation)
}
```

---

## 5. Emitter Architecture

### 5.1 New emitter interface

```go
// ServiceSdkEmitter generates typed HTTP client SDKs for consumed services.
type ServiceSdkEmitter interface {
    Emitter
    Summarizer
    IsServiceSdk() // marker
}
```

Registered in a parallel registry: `RegisterServiceSdk(name, emitter)`, `GetServiceSdk(name)`.

### 5.2 Registration per language

| Language | Package | Registry Key |
|----------|---------|--------------|
| TypeScript/Node.js | `internal/emitter/servicesdk/node/` | `"node"` |
| Python | `internal/emitter/servicesdk/python/` | `"python"` |
| Go | `internal/emitter/servicesdk/go/` | `"go"` |
| Rust | `internal/emitter/servicesdk/rust/` | `"rust"` |
| Java | `internal/emitter/servicesdk/java/` | `"java"` |
| C# | `internal/emitter/servicesdk/csharp/` | `"csharp"` |
| PHP | `internal/emitter/servicesdk/php/` | `"php"` |

### 5.3 Emission flow in workspace loop

```
for each workspace entry:
    1. Parse .veld → AST (cached)
    2. Run backend emitter (existing)
    3. Run frontend emitter (existing)
    4. NEW: If entry.Consumes is non-empty:
        a. Look up ServiceSdkEmitter for entry.Backend language
        b. For each consumed service name:
            i.  Retrieve consumed service's cached AST
            ii. Call sdkEmitter.Emit(consumedAST, outDir, opts) 
            iii. Output goes to generated/sdk/<service_name>/
```

### 5.4 Shared helpers — `internal/emitter/sdkhelpers/`

```
sdkhelpers/
├── sdkhelpers.go      // BuildMethodName, BuildUrlInterpolation, EnvVarName
├── types.go           // CollectSdkTypes (models + enums needed by consumed AST)
└── baseurl.go         // Per-language base URL resolution templates
```

These reuse the existing `LanguageAdapter` from `internal/emitter/lang/` for type mapping and naming.

---

## 6. Generated Output Structure

### 6.1 TypeScript/Node.js (consumer: `--backend=node`)

```
generated/
├── types/...              ← existing backend types
├── interfaces/...         ← existing service interfaces  
├── routes/...             ← existing route handlers
└── sdk/                   ← NEW: service SDK clients
    ├── iam/
    │   ├── client.ts      ← IAMClient class (fetch-based)
    │   ├── types.ts       ← User, TokenPair, LoginInput (only consumed models)
    │   └── index.ts       ← barrel export
    ├── accounts/
    │   ├── client.ts      ← AccountsClient class  
    │   ├── types.ts       ← Account, CreateAccountInput
    │   └── index.ts
    └── index.ts           ← barrel: export { IAMClient } from './iam'; ...
```

**Example generated `sdk/iam/client.ts`:**

```typescript
// AUTO-GENERATED BY VELD — DO NOT EDIT
// Service SDK: IAM client for inter-service communication

import type { User, TokenPair, LoginInput, RegisterInput } from './types';

export class VeldApiError extends Error {
  constructor(public readonly status: number, public readonly body: string) {
    super(`HTTP ${status}: ${body}`);
    this.name = 'VeldApiError';
  }
}

export class IAMClient {
  private readonly base: string;
  private readonly hdrs: Record<string, string>;

  constructor(baseUrl?: string, headers?: Record<string, string>) {
    this.base = baseUrl
      ?? process.env.VELD_IAM_URL
      ?? 'http://iam-service:3001';  // baked from config
    this.hdrs = headers ?? {};
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await fetch(this.base + path, {
      method,
      headers: { 'Content-Type': 'application/json', ...this.hdrs },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) throw new VeldApiError(res.status, await res.text());
    if (res.status === 204) return undefined as T;
    return res.json() as Promise<T>;
  }

  /** POST /api/iam/register */
  register(input: RegisterInput): Promise<User> {
    return this.request('POST', '/api/iam/register', input);
  }

  /** POST /api/iam/login */
  login(input: LoginInput): Promise<TokenPair> {
    return this.request('POST', '/api/iam/login', input);
  }

  /** GET /api/iam/me */
  getProfile(): Promise<User> {
    return this.request('GET', '/api/iam/me');
  }
}
```

### 6.2 Python (consumer: `--backend=python`)

```
generated/
├── types/...
├── interfaces/...
├── routes/...
└── sdk/
    ├── iam/
    │   ├── client.py      ← IAMClient class (urllib-based)
    │   ├── types.py       ← @dataclass models
    │   └── __init__.py    ← from .client import IAMClient
    ├── accounts/
    │   ├── client.py
    │   ├── types.py
    │   └── __init__.py
    └── __init__.py
```

**Example generated `sdk/iam/client.py`:**

```python
# AUTO-GENERATED BY VELD — DO NOT EDIT
# Service SDK: IAM client for inter-service communication

from __future__ import annotations
import json
import os
from dataclasses import dataclass, asdict
from typing import Optional, Dict, Any
from urllib.request import Request, urlopen
from urllib.error import HTTPError

from .types import User, TokenPair, LoginInput, RegisterInput


class VeldApiError(Exception):
    def __init__(self, status: int, body: str):
        super().__init__(f"HTTP {status}: {body}")
        self.status = status
        self.body = body


class IAMClient:
    def __init__(
        self,
        base_url: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
    ):
        self._base = (
            base_url
            or os.environ.get("VELD_IAM_URL")
            or "http://iam-service:3001"
        )
        self._headers = headers or {}

    def _request(self, method: str, path: str, body: Any = None) -> Any:
        data = json.dumps(body).encode() if body is not None else None
        hdrs = {"Content-Type": "application/json", **self._headers}
        req = Request(self._base + path, data=data, headers=hdrs, method=method)
        try:
            with urlopen(req) as res:
                if res.status == 204:
                    return None
                return json.loads(res.read())
        except HTTPError as e:
            raise VeldApiError(e.code, e.read().decode()) from e

    def register(self, input: RegisterInput) -> User:
        """POST /api/iam/register"""
        data = self._request("POST", "/api/iam/register", asdict(input))
        return User(**data)

    def login(self, input: LoginInput) -> TokenPair:
        """POST /api/iam/login"""
        data = self._request("POST", "/api/iam/login", asdict(input))
        return TokenPair(**data)

    def get_profile(self) -> User:
        """GET /api/iam/me"""
        data = self._request("GET", "/api/iam/me")
        return User(**data)
```

### 6.3 Go (consumer: `--backend=go`)

```
generated/
├── types/...
├── handlers/...
└── sdk/
    ├── iam/
    │   ├── client.go      ← Client struct (net/http-based)
    │   ├── types.go       ← struct types
    │   └── doc.go         ← package comment
    ├── accounts/
    │   ├── client.go
    │   ├── types.go
    │   └── doc.go
    └── sdk.go             ← convenience constructors
```

**Example generated `sdk/iam/client.go`:**

```go
// Code generated by Veld — DO NOT EDIT.
// Service SDK: IAM client for inter-service communication.
package iam

import (
    "bytes"
    "context"
    "encoding/json"
    "fmt"
    "io"
    "net/http"
    "os"
)

// VeldApiError represents an HTTP error from the IAM service.
type VeldApiError struct {
    Status int
    Body   string
}

func (e *VeldApiError) Error() string {
    return fmt.Sprintf("HTTP %d: %s", e.Status, e.Body)
}

// Client communicates with the IAM service.
type Client struct {
    base   string
    client *http.Client
    hdrs   map[string]string
}

// Option configures the Client.
type Option func(*Client)

// WithHTTPClient sets a custom http.Client.
func WithHTTPClient(c *http.Client) Option {
    return func(cl *Client) { cl.client = c }
}

// WithHeaders sets default request headers.
func WithHeaders(h map[string]string) Option {
    return func(cl *Client) { cl.hdrs = h }
}

// NewClient creates an IAM service client.
// baseURL defaults to VELD_IAM_URL env var, then "http://iam-service:3001".
func NewClient(baseURL string, opts ...Option) *Client {
    if baseURL == "" {
        baseURL = os.Getenv("VELD_IAM_URL")
    }
    if baseURL == "" {
        baseURL = "http://iam-service:3001"
    }
    c := &Client{base: baseURL, client: http.DefaultClient, hdrs: map[string]string{}}
    for _, o := range opts {
        o(c)
    }
    return c
}

func (c *Client) do(ctx context.Context, method, path string, body any) ([]byte, int, error) {
    var reader io.Reader
    if body != nil {
        b, err := json.Marshal(body)
        if err != nil {
            return nil, 0, err
        }
        reader = bytes.NewReader(b)
    }
    req, err := http.NewRequestWithContext(ctx, method, c.base+path, reader)
    if err != nil {
        return nil, 0, err
    }
    req.Header.Set("Content-Type", "application/json")
    for k, v := range c.hdrs {
        req.Header.Set(k, v)
    }
    resp, err := c.client.Do(req)
    if err != nil {
        return nil, 0, err
    }
    defer resp.Body.Close()
    data, _ := io.ReadAll(resp.Body)
    if resp.StatusCode >= 400 {
        return nil, resp.StatusCode, &VeldApiError{Status: resp.StatusCode, Body: string(data)}
    }
    return data, resp.StatusCode, nil
}

// Register calls POST /api/iam/register.
func (c *Client) Register(ctx context.Context, input RegisterInput) (*User, error) {
    data, _, err := c.do(ctx, "POST", "/api/iam/register", input)
    if err != nil {
        return nil, err
    }
    var out User
    if err := json.Unmarshal(data, &out); err != nil {
        return nil, err
    }
    return &out, nil
}

// Login calls POST /api/iam/login.
func (c *Client) Login(ctx context.Context, input LoginInput) (*TokenPair, error) {
    data, _, err := c.do(ctx, "POST", "/api/iam/login", input)
    if err != nil {
        return nil, err
    }
    var out TokenPair
    if err := json.Unmarshal(data, &out); err != nil {
        return nil, err
    }
    return &out, nil
}

// GetProfile calls GET /api/iam/me.
func (c *Client) GetProfile(ctx context.Context) (*User, error) {
    data, _, err := c.do(ctx, "GET", "/api/iam/me", nil)
    if err != nil {
        return nil, err
    }
    var out User
    if err := json.Unmarshal(data, &out); err != nil {
        return nil, err
    }
    return &out, nil
}
```

### 6.4 Rust (consumer: `--backend=rust`)

```
generated/
└── sdk/
    ├── iam/
    │   ├── mod.rs         ← pub mod client; pub mod types;
    │   ├── client.rs      ← IAMClient with blocking std::net or async reqwest
    │   └── types.rs       ← #[derive(Serialize, Deserialize)] structs
    └── mod.rs             ← pub mod iam; pub mod accounts;
```

### 6.5 Java (consumer: `--backend=java`)

```
generated/
└── sdk/
    └── iam/
        ├── IAMClient.java       ← java.net.http.HttpClient-based
        ├── types/
        │   ├── User.java        ← record types
        │   ├── TokenPair.java
        │   └── LoginInput.java
        └── VeldApiError.java
```

### 6.6 C# (consumer: `--backend=csharp`)

```
generated/
└── Sdk/
    └── Iam/
        ├── IamClient.cs          ← HttpClient-based, async Task<T>
        ├── Types/
        │   ├── User.cs           ← record types
        │   └── TokenPair.cs
        └── VeldApiError.cs
```

### 6.7 PHP (consumer: `--backend=php`)

```
generated/
└── sdk/
    └── iam/
        ├── IamClient.php         ← curl-based
        ├── Types/
        │   ├── User.php          ← readonly class
        │   └── TokenPair.php
        └── VeldApiError.php
```

---

## 7. Validation Rules

New validation rules added to the workspace processing:

| Rule | Severity | Message |
|------|----------|---------|
| Unknown consumed service | **Error** | `workspace "transactions": consumes unknown service "auth" (available: iam, accounts, cards)` |
| Self-consumption | **Error** | `workspace "iam": cannot consume itself` |
| Circular dependency | **Error** | `circular service dependency: transactions → accounts → transactions` |
| No baseUrl on consumed | **Warning** | `consumed service "iam" has no baseUrl — clients must provide it at runtime or via VELD_IAM_URL` |
| consumes without workspace | **Error** | `"consumes" requires workspace mode — add a "workspace" array to veld.config.json` |

---

## 8. CLI Changes

### 8.1 New flag: `--service-sdk`

```bash
veld generate --service-sdk              # force SDK generation for all workspace siblings
veld generate --workspace transactions   # generates transactions + its consumed SDKs
```

### 8.2 Deprecation: `--server-sdk`

```
⚠ --server-sdk is deprecated. Use "consumes" in workspace config or --service-sdk flag.
```

Internally, `--server-sdk` maps to TypeScript service SDK only (backward compat).

### 8.3 New command: `veld deps`

```bash
veld deps                   # print dependency graph
veld deps --dot             # output Graphviz DOT format
veld deps --validate        # check for circular deps, missing services
```

Example output:
```
◆ Service Dependencies

  transactions → iam, accounts
  cards → iam, accounts
  lending → iam, accounts
  accounts → iam
  iam → (none)
  notifications → (none)
```

---

## 9. Shared Model Deduplication

When Service A consumes both IAM and Accounts, and both contracts reference a `User` model:

1. **Each SDK gets its own copy.** `sdk/iam/types.ts` has `User`, `sdk/accounts/types.ts` also has `User`.
2. **They are structurally identical** (generated from the same `.veld` model definition).
3. **No cross-SDK imports** — each SDK is self-contained.

This keeps SDKs independent and avoids diamond dependency issues. If a developer needs to unify them, they can create a shared types file manually and use TypeScript declaration merging or Go type aliases.

---

## 10. Phase Breakdown

### Phase 1 — Foundation (Week 1-2)
- [ ] Add `Consumes` field to `WorkspaceEntry`
- [ ] Add `ConsumedServiceInfo` type and `ConsumedServices` field to `EmitOptions`
- [ ] Implement two-pass workspace processing in `main.go` (parse all → resolve consumes)
- [ ] Add validation rules (unknown, circular, self-consumption)
- [ ] Add `ServiceSdkEmitter` interface and registry

### Phase 2 — TypeScript SDK Emitter (Week 2-3)
- [ ] Create `internal/emitter/servicesdk/node/` 
- [ ] Migrate existing `server_client.go` logic
- [ ] Generate per-consumed-service `sdk/<name>/client.ts`, `types.ts`, `index.ts`
- [ ] Generate `sdk/index.ts` barrel
- [ ] Unit tests with golden file snapshots
- [ ] Update NexusBank example

### Phase 3 — Python SDK Emitter (Week 3-4)
- [ ] Create `internal/emitter/servicesdk/python/`
- [ ] Generate `sdk/<name>/client.py` (urllib-based), `types.py` (dataclasses), `__init__.py`
- [ ] Path param interpolation, query string building
- [ ] Unit tests

### Phase 4 — Go SDK Emitter (Week 4-5)
- [ ] Create `internal/emitter/servicesdk/go/`
- [ ] Generate `sdk/<name>/client.go` (net/http), `types.go`, `doc.go`
- [ ] Functional options pattern
- [ ] Context propagation
- [ ] Unit tests

### Phase 5 — Remaining Languages (Week 5-7)
- [ ] Rust SDK emitter (`internal/emitter/servicesdk/rust/`)
- [ ] Java SDK emitter (`internal/emitter/servicesdk/java/`)
- [ ] C# SDK emitter (`internal/emitter/servicesdk/csharp/`)
- [ ] PHP SDK emitter (`internal/emitter/servicesdk/php/`)

### Phase 6 — CLI & DX (Week 7-8)
- [ ] `--service-sdk` flag
- [ ] `--server-sdk` deprecation warning
- [ ] `veld deps` command
- [ ] Dry-run support for SDK generation
- [ ] `veld doctor` checks for consumes issues

### Phase 7 — Documentation & Polish (Week 8-9)
- [ ] Update CLAUDE.md
- [ ] Create `docs/guides/service-sdk.md`
- [ ] Update NexusBank example with full consumes config
- [ ] Update architecture overview
- [ ] Update roadmap

---

## 11. File Inventory (New & Modified)

### New Files

| File | Purpose |
|------|---------|
| `internal/emitter/servicesdk/doc.go` | Package documentation |
| `internal/emitter/servicesdk/helpers.go` | Shared SDK generation helpers |
| `internal/emitter/servicesdk/node/emitter.go` | TypeScript SDK emitter |
| `internal/emitter/servicesdk/node/client.go` | TypeScript client generation |
| `internal/emitter/servicesdk/node/types.go` | TypeScript type generation |
| `internal/emitter/servicesdk/python/emitter.go` | Python SDK emitter |
| `internal/emitter/servicesdk/python/client.go` | Python client generation |
| `internal/emitter/servicesdk/python/types.go` | Python dataclass generation |
| `internal/emitter/servicesdk/go/emitter.go` | Go SDK emitter |
| `internal/emitter/servicesdk/go/client.go` | Go client generation |
| `internal/emitter/servicesdk/go/types.go` | Go struct generation |
| `internal/emitter/servicesdk/rust/emitter.go` | Rust SDK emitter |
| `internal/emitter/servicesdk/java/emitter.go` | Java SDK emitter |
| `internal/emitter/servicesdk/csharp/emitter.go` | C# SDK emitter |
| `internal/emitter/servicesdk/php/emitter.go` | PHP SDK emitter |
| `internal/validator/workspace.go` | Workspace dependency validation |
| `docs/guides/service-sdk.md` | User guide |
| `docs/architecture/service-sdk-plan.md` | This document |

### Modified Files

| File | Change |
|------|--------|
| `internal/config/config.go` | Add `Consumes` to `WorkspaceEntry`, `ConsumedServices` resolution |
| `internal/emitter/emitter.go` | Add `ConsumedServiceInfo`, `ServiceSdkEmitter` interface, SDK registry |
| `cmd/veld/main.go` | Two-pass workspace loop, `--service-sdk` flag, `veld deps` command |
| `CLAUDE.md` | Document service SDK feature |
| `docs/architecture/overview.md` | Add SDK emitter category |
| `docs/roadmap.md` | Update status |
| `examples/nexusbank/veld/veld.config.json` | Add `consumes` to workspace entries |
| `examples/nexusbank/README.md` | Document SDK usage |

---

## 12. Migration Path

| Scenario | Behavior |
|----------|----------|
| `serverSdk: true`, no `consumes` | Works exactly as today — TypeScript `server-client/api.ts` |
| `serverSdk: true` + `consumes` | New SDK pipeline takes over; `server-client/` still emitted for backward compat |
| `consumes` only (no `serverSdk`) | SDK pipeline generates language-native clients in `sdk/` |
| `--service-sdk` flag | Force SDK generation for ALL workspace siblings (no explicit `consumes` needed) |
| `--server-sdk` flag | Deprecated warning → internally maps to TypeScript SDK only |

---

## 13. Edge Cases

| Case | Handling |
|------|----------|
| Consumed service has no modules/actions | Generate empty SDK with only types (if any models exist) |
| Consumed service uses WebSocket actions | Skip WS actions in SDK (HTTP-only); add `// WebSocket actions not supported in service SDK` comment |
| Path params in consumed actions | Interpolated per-language: TS template literals, Python f-strings, Go fmt.Sprintf |
| Optional fields in models | Reflect optionality: TS `?:`, Python `Optional[T]`, Go pointer types |
| Map types | Reflect per-language: TS `Record<K,V>`, Python `Dict[K,V]`, Go `map[K]V` |
| Enum types in consumed models | Generate enum definitions in SDK types file |
| Model inheritance (`extends`) | Flatten inherited fields into the SDK type (SDK types are standalone) |
| `@deprecated` actions | Generate deprecation annotations in SDK methods |
| `@serverSet` fields | Omit from input types in SDK (same as backend interfaces) |
| `query` param in actions | Generate query string building in SDK methods |

---

## 14. Success Metrics

- [ ] `veld generate` with `consumes` produces compilable SDK code for all 7 languages
- [ ] NexusBank Transaction Service (Python) can import and call IAM Service via generated SDK
- [ ] NexusBank Card Service (Go) can import and call IAM + Accounts via generated SDK
- [ ] Zero runtime dependencies in all generated SDKs
- [ ] `veld deps` prints correct dependency graph
- [ ] All existing tests pass (backward compat)
- [ ] `--server-sdk` still works with deprecation warning

---

## 15. Future Extensions (Post-Launch)

| Feature | Description |
|---------|-------------|
| **Version pinning** | `"consumes": ["iam@1.2.0"]` with registry integration |
| **Auth header forwarding** | `withRequestHeaders(incomingReq)` helper in generated clients |
| **Retry & timeout** | Opt-in `WithRetry(3)`, `WithTimeout(5s)` options |
| **Circuit breaker** | Generated circuit breaker wrapper (opt-in) |
| **Service mesh integration** | Istio/Linkerd-aware SDK generation (skip retries if mesh handles them) |
| **gRPC support** | Generate protobuf definitions + gRPC clients from .veld contracts |
| **Event-driven** | Generate event publisher/subscriber SDKs for async communication |
| **OpenTelemetry tracing** | Opt-in tracing spans in generated SDK methods |

