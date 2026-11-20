# microservices (NexusBank)

A banking platform with five services. This is the v1 NexusBank example, migrated with
`veld migrate` and then rewritten by hand in idiomatic v2 (`uuid`, `decimal` and `datetime` types,
constraints, custom errors, `@auth` per endpoint, `Page<T>`). The v1 original is in
`archive/v1/examples/microservices`.

| Service | Server | Consumes |
|---|---|---|
| `iam` | Node, express | |
| `accounts` | Node, express | `iam`, `transactions` |
| `transactions` | **Java**, JDK HTTP server | `iam`, `accounts` |
| `cards` | Node, fastify | `iam`, `accounts` |
| `notifications` | Node, hono | `iam` |

`accounts` and `transactions` consume each other, and import each other's types
(`Statement` uses `Transaction`, `TransferReceipt` uses `Account`). Cycles are legal in Veld.
Every server gets a generated client for each service it consumes, under `<out>/clients/<service>/`
(Java: package `<base>.clients.<service>`). The `frontend` target is one TypeScript client for all
services. The v1 React hooks are not part of 2.0, so it has no framework.

## Commands

```bash
veld check
veld generate
veld generate --dry-run
veld generate transactions-api      # one target
veld export openapi                  # OpenAPI 3.1 for the whole workspace
```

Generated output is committed: `backend/*/src/generated`,
`backend/transaction-service/src/main/java`, `frontend/src/generated`.

To compile the generated code yourself: `tsc --strict --noEmit` on each TypeScript directory, and
`javac --release 17` on the Java tree.
