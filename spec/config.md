# `veld.toml` — v2

Status: **DRAFT — frozen together with `language.md`.**

Each project has exactly one config file, `veld.toml`. It lives at the project root, and `veld` finds it by searching upward from the current directory. Paths in it are resolved relative to the file. Any unknown key is an error (`E0701`), and the message lists the closest valid key.

## Single service (minimal)

```toml
[project]
name = "shop"              # package name of the implicit default service
contracts = "contracts"    # package root (default "contracts")

[[targets]]
role = "server"
emitter = "node"
framework = "express"      # express | fastify | hono | none
out = "../api/src/generated"

[[targets]]
role = "client"
emitter = "typescript"
out = "../web/src/generated"
```

## Workspace (microservices)

```toml
[project]
name = "nexusbank"

[libraries.common]          # shared contracts, importable as `use common:money.{Money}`
contracts = "contracts/common"

[services.iam]
contracts = "contracts/iam"
base-url = "http://iam:3001"

[services.orders]
contracts = "contracts/orders"
base-url = "http://orders:3002"
consumes = ["iam", "payments"]   # cycles allowed

[services.payments]
contracts = "contracts/payments"
consumes = ["orders"]

[[targets]]
name = "orders-api"              # optional; used by `veld generate <name>`
service = "orders"
role = "server"
emitter = "node"
framework = "express"
out = "services/orders/src/generated"
# For each service in `consumes`, a client in the same emitter's language is generated
# under <out>/clients/<service>/. Set `clients = false` to opt out.

[[targets]]
role = "client"
services = ["iam", "orders"]     # default: all services
emitter = "typescript"
framework = "react"              # none | react | vue
out = "web/src/generated"

[targets.options]                # emitter-specific, validated by the emitter's typed Options
validate = true

[plugins.kotlin]
command = "veld-emit-kotlin"     # resolved on PATH or relative path
version = "1.2.0"
sha256 = "…"                     # of the executable; mismatch = refuse to run (E0705)

[registry]
url = "https://registry.example.com"

[dependencies]
"@acme/billing" = "^1.2"         # resolved and pinned in veld.lock
```

## Rules

- Exactly one of `[project].contracts` or `[services.*]` defines the packages. With no `[services]` section, the project itself is a single service named `project.name`.
- `consumes` entries MUST name existing services (`E0702`). Listing the service itself is a warning (`W0701`). Cycles are allowed.
- A target needs `role`, `emitter` and `out`.
  - `service` is required for `role = "server"`.
  - Two targets MUST NOT share an `out` directory (`E0703`).
  - `out` MUST NOT be the project root or a contracts directory (`E0704`).
- `emitter` is either a built-in id (`node`, `typescript`, `java`) or `plugin:<name>`.
- `base-url` resolution in generated clients, highest priority first:
  1. the constructor argument
  2. the env var `VELD_<SERVICE>_URL` (service name in UPPER_SNAKE)
  3. the config `base-url`
  4. otherwise an error at client construction
- Generated output never contains absolute paths, user names, hostnames or timestamps.
