# library-package

A shared contracts package, `common`, used by two services and by hand-written code that uses its
types and errors.

```
contracts/common/money.veld      Money, Currency, PageQuery
contracts/common/errors.veld     Unavailable, PlanLimitReached
contracts/orders/                the orders service
contracts/invoices/              the invoices service
orders/src/main.ts               orders server (Express), hand-written; orders/src/generated -> @veld/orders-api
invoices/src/main.ts             invoices server (Fastify), hand-written; -> @veld/invoices-api
web/src/report.ts                a typed client script;                   -> @veld/web
```

`veld.toml` declares the library with `[libraries.common]`. The services import from it with
`use common:money.{Money, PageQuery}` and `use common:errors.{Unavailable}`.

Because `Unavailable` is declared once, in `common`, it has one signature
(`veld:common/Unavailable@...`). Both generated servers and the client embed that same signature, so
an `Unavailable` raised by one service is recognised by code generated for the other.

## The shared types in real code

Both servers import `Money` (and the errors) from their generated package like any other type:

```ts
import { PlanLimitReached, Unavailable, createOrdersHandler, toExpress } from '@veld/orders-api';
import type { Money, Order, OrdersService } from '@veld/orders-api';

throw new PlanLimitReached({ limit: '3 open orders' });   // declared in common
```

- `orders` places orders with a `Money` total, raises `PlanLimitReached` (402) past three open
  orders, and `Unavailable` (503) while `ORDERS_MAINTENANCE=1`.
- `invoices` lists seeded invoices (`Money` again) and raises the same `Unavailable` once when a big
  invoice is paid, then succeeds on retry.
- `web/src/report.ts` uses `@veld/web`: it catches `PlanLimitReached` and retries on `Unavailable`.
  An error declared in a library is matched together with the service that raised it, so the code
  writes `Unavailable.matches(e, 'invoices/Invoices')`; the signature is `veld:common/...` either way.

Both servers check `Authorization: Bearer demo-token` (`API_TOKEN` overrides it).

## Commands

```bash
veld check
veld generate
veld setup
(cd orders && npm install && npm start)      # PORT=3001
(cd invoices && npm install && npm start)    # PORT=3002
(cd web && npm install && VELD_ORDERS_URL=http://127.0.0.1:3001 VELD_INVOICES_URL=http://127.0.0.1:3002 npm start)
```

`cargo test -p veld --test examples_e2e` runs this example end to end.
