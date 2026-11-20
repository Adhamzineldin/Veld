# library-package

A shared contracts package, `common`, used by two services.

```
contracts/common/money.veld      Money, Currency, PageQuery
contracts/common/errors.veld     Unavailable, PlanLimitReached
contracts/orders/                the orders service
contracts/invoices/              the invoices service
```

`veld.toml` declares the library with `[libraries.common]`. The services import from it with
`use common:money.{Money, PageQuery}` and `use common:errors.{Unavailable}`.

Because `Unavailable` is declared once, in `common`, it has one signature
(`veld:common/Unavailable@...`). Both generated servers and the client embed that same signature, so
an `Unavailable` raised by one service is recognised by code generated for the other.
`Page<T>` is built in and used by `listOrders` and `listInvoices`.

## Commands

```bash
veld check
veld generate
veld generate --dry-run
```

Output: `orders/src/generated` and `invoices/src/generated` (Node servers, the library's
declarations are under `deps/common/`) and `web/src/generated` (TypeScript client).
