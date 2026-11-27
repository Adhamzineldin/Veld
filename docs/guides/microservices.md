# Microservices with Veld

One repository, several services, one set of contracts. Veld treats a workspace as a graph: each
service has its own contracts and its own server, and every service that calls another gets a typed
client for it. A typed error raised in one service arrives as the same typed class in its caller,
even when the two are written in different languages.

The complete working example is [`examples/microservices`](../../examples/microservices) (NexusBank:
four Node services and one Java service, mutually dependent, plus a React app). The smaller
[`examples/library-package`](../../examples/library-package) shows a shared library.

## 1. Declare the services

A workspace is a root `veld.toml` that lists the members, and one `veld.toml` per member in the
member's directory. Each service is a package: a folder of `.veld` files. The root:

```toml
# veld.toml
[workspace]
name = "nexusbank"
members = ["backend/*", "web"]

# Node servers use Express unless a service says otherwise.
[workspace.defaults.server.node]
framework = "express"
```

A service, `backend/account-service/veld.toml`. accounts and transactions consume each other;
cycles are legal:

```toml
[service]
name = "accounts"                      # the directory is `account-service`
contracts = "../../contracts/accounts"
base-url = "http://account-service:3002"
consumes = ["iam", "transactions"]
owners = ["team-accounts"]
```

- The service is named after its directory unless `name` says otherwise. The name is what
  `use accounts:...` and `consumes` refer to.
- `consumes` lists the services a service calls. Each name must exist (`E0702`). A service listing
  itself is a warning (`W0701`). **Cycles are allowed**: `accounts` asks `transactions` for a statement and
  `transactions` asks `accounts` to check a source account.
- `base-url` is the default address of the service for generated clients.
- `owners` are free-form names that `veld diff` and `veld check --graph=json` show.

Start a workspace with `veld init --workspace iam,accounts,transactions`, which writes the folders,
the contracts, a `veld.toml` and a `package.json` per service. It asks which services call which
(`--consumes orders=iam`), so the `consumes` lists start out as you intend them.

## 2. One server target per service

The server of a service is a `[server]` table in the service's own `veld.toml`:

```toml
# backend/account-service/veld.toml (continued)
[server]
name = "accounts-api"          # `veld generate accounts-api` builds just this one
emitter = "node"               # the framework (express) comes from the workspace defaults
out = "src/generated"          # relative to this file
```

```toml
# backend/transaction-service/veld.toml
[service]
name = "transactions"
contracts = "../../contracts/transactions"
consumes = ["iam", "accounts"]

# The transactions service is a Java server (JDK HTTP server adapter, no dependencies).
[server]
name = "transactions-api"
emitter = "java"
framework = "jdk"
out = "src/main/java"

[server.options]
package = "com.nexusbank.transactions"
```

Servers of different languages live side by side. Only the contracts connect them. The web app is
an `[app]` member, `web/veld.toml`, whose `[client]` covers every service by default, or the ones
you list in `uses = ["iam", "accounts"]`.

Run `veld` in a member's directory and it works on that member only; `-p accounts` picks a member
from anywhere and `veld generate --affected --since origin/master` takes the members a change can
have affected. See [Workspaces](workspaces.md).

## 3. Calling another service

For each name in `consumes`, the server target also gets a client for that service, generated in
the same language. You do not configure anything else. In TypeScript the clients come through the
server's own package as namespaces; in Java they are under `<base>.clients.<service>`.

**Node (`accounts` calls `iam` and `transactions`):**

```ts
import { createAccountsHandler, iam, transactions } from '@veld/accounts-api';

const profile = await new iam.IamClient({ auth: { bearer: () => token } }).getProfile({});
const page = await new transactions.TransactionsClient({ auth: { bearer: () => token } })
  .getHistory({ accountId, query: { page: 1, size: 5 } });
```

**Java (`transactions` calls `accounts`):**

```java
import com.nexusbank.transactions.clients.accounts.accounts.client.AccountsClient;
```

Set `clients = false` on a target to turn this off.

### Where the client sends requests

Resolved at construction, highest priority first:

1. the constructor argument (`baseUrl`)
2. the environment variable `VELD_<SERVICE>_URL`, service name in UPPER_SNAKE (`VELD_ACCOUNTS_URL`)
3. the `base-url` in the service's `veld.toml`
4. otherwise the constructor fails with a clear error

So `base-url` is a development default, and each deployment overrides it with the environment, as
the example does:

```bash
export VELD_IAM_URL=http://127.0.0.1:3001 VELD_ACCOUNTS_URL=http://127.0.0.1:3002 \
       VELD_TRANSACTIONS_URL=http://127.0.0.1:3003
```

### Passing authentication on

Generated clients take an `auth` option (`{ bearer: () => token }`, also `basic` and `apiKey`) or a
`headers` function. A service that forwards the caller's identity passes the incoming token.
Because the options are plain functions, you can read it from whatever request context you use.

## 4. Errors cross service boundaries

An error is a declaration, so it has an identity that Veld computes (a signature). When `accounts`
raises `NotFound` inside a call from `transactions`, that response is `application/problem+json` whose
`type` and `sig` identify the error. `transactions` receives it as its own generated `NotFound`
and can rethrow it. `NotFound.matches(e)` (TypeScript) and `NotFound.matches(e)` or `Veld.same(a, b)`
(Java) compare signature, operation and canonical bytes, not class identity, so the answer is the
same in every language. In the example, a transfer from an unknown account travels
React, Java, Node and back as one typed `NotFound`.

A `NotFound` raised by `accounts.getAccount` is never equal to a `NotFound` raised by
`payments.getPayment`: the operation is part of the identity.

## 5. Sharing contracts: libraries and registry packages

Types and errors used by several services belong in a **library**:

```toml
# libs/common/veld.toml
[library]
```

```veld
// contracts/orders/orders.veld
use common:errors.{PlanLimitReached, Unavailable}
use common:money.{Money, PageQuery}
```

A library has no endpoints and no target. Its declarations are generated into every target that
uses them. `use <package>:<module>.{Name}` names a service or library; `use @org/name:<module>.{Name}`
names a registry package you added with `veld add @org/name` (see `spec/config.md`).
Imports may be cyclic between modules and packages: Veld collects every declaration first and
resolves names second.

## 6. Generate, wire and run

```bash
veld check                                  # every service, one compile
veld generate                               # all targets, or: veld generate accounts-api
veld setup                                  # @veld/<target> into each service's package.json
for d in backend/*-service web; do [ -f $d/package.json ] && (cd $d && npm install); done
docker compose up --build                   # or one terminal per service, see the example README
```

`veld setup` wires each target into the nearest `package.json` above its `out` (or the folder named
by `app = "path"` on the target), and a Maven or Gradle build for Java. Every service then imports
`@veld/<target>`.

To build one service while you work on it: `veld generate accounts-api`. To keep everyone honest in
CI: `veld generate --check` and `veld diff --strict`, see [ci.md](ci.md). To get one OpenAPI
document for the whole workspace: `veld export openapi -o openapi.json`
(`--service accounts` for one service).

## 7. Tips

- **One contract folder per service**, owned by the team that owns the service. Other teams depend on
  the contract, not on the code.
- **Breaking changes.** `veld diff` compares the current contracts with the last generated snapshot
  (`veld.lock.ir.json`, commit it) or with a git ref (`veld diff --against origin/main`) and lists
  breaking and non-breaking changes. Run it in CI with `--strict` so a pull request that breaks a
  consumer fails.
- **Different frameworks per service are fine.** In the example, `iam` and `accounts` use Express,
  `cards` uses Fastify, `notifications` uses Hono, and `transactions` is Java.
