# microservices (NexusBank)

A banking platform with five services and a React app, written by hand against Veld's generated
code. The contracts are the v1 NexusBank example, rewritten in
idiomatic v2. The v1 original is in `archive/v1/examples/microservices`.

| Service | Server | Consumes | What the hand-written code does |
|---|---|---|---|
| `iam` | Node, Express | | registration, login (scrypt), token pairs, profiles |
| `accounts` | Node, Express | `iam`, `transactions` | accounts per user; a statement asks the Java service for the history |
| `transactions` | **Java**, JDK HTTP server | `iam`, `accounts` | an in-memory ledger; a transfer asks `accounts` to check the source account |
| `cards` | Node, Fastify | `iam`, `accounts` | cards; a card is only issued for an account that `accounts` confirms |
| `notifications` | Node, Hono | `iam` | notifications and preferences |
| `web` | React 18, Vite | all of them | sign in, accounts with statement, transfer |

`accounts` and `transactions` consume each other, in two languages. Cycles are legal in Veld. A typed
error crosses every hop: `transfer` on the Java service from an unknown account calls the Node
`accounts` service, which raises `NotFound`; Java turns it into its own `NotFound`; the React app
receives the typed class.

```
contracts/<service>/*.veld              the contracts
veld.toml                               targets: five servers (4 Node, 1 Java) and the web client
backend/<svc>-service/src/main.ts       hand-written Node implementations
backend/<svc>-service/src/generated/    generated  ->  package @veld/<svc>-api
backend/transaction-service/            Maven project; hand-written Java in src/main/java/.../app,
                                        generated Java in the rest of src/main/java
web/src/*.tsx                           hand-written React app; web/src/generated -> @veld/web
docker-compose.yml, Dockerfile*         build and run everything
```

## Generate and wire

```bash
veld generate
veld setup                                  # @veld/<target> into each package.json
for d in backend/*-service web; do [ -f $d/package.json ] && (cd $d && npm install); done
```

### How the code imports the generated code

**Node backends and the web app** import `@veld/<target>` (see `veld setup`), never a relative
path into `src/generated`. The clients of the consumed services come through the same package,
as namespaces (re-exported by the generated barrel): in `accounts`,

```ts
import { createAccountsHandler, iam, transactions } from '@veld/accounts-api';

const profile = await new iam.IamClient({ auth: { bearer: () => token } }).getProfile({});
const page = await new transactions.TransactionsClient({ auth: { bearer: () => token } })
  .getHistory({ accountId, query: { page: 1, size: 5 } });
```

The web app imports `@veld/web` and the per-service hooks, `@veld/web/accounts/react` and so on.

**Java** cannot use npm-style names. There `veld setup` adds the generated sources to the Maven or
Gradle build (here the output is already Maven's standard `src/main/java`, so it has nothing to
add; for another folder it adds a `build-helper-maven-plugin` `add-source` execution, or a
`sourceSets.main.java.srcDir(...)` line for Gradle), and the hand-written Java imports the generated
packages like any other:

```java
import com.nexusbank.transactions.clients.accounts.accounts.client.AccountsClient;
import com.nexusbank.transactions.transactions.server.TransactionsService;
```

`TransactionsServiceImpl` implements the generated `TransactionsService` and `TransactionServiceMain`
starts it with the generated `JdkHttpServerAdapter`.

## Run it

With Docker (builds everything; the web app is at http://localhost:8080):

```bash
docker compose up --build
```

Without Docker, one terminal each (every service reads `PORT` and the `VELD_<SERVICE>_URL`
variables for the services it calls):

```bash
export VELD_IAM_URL=http://127.0.0.1:3001 VELD_ACCOUNTS_URL=http://127.0.0.1:3002 \
       VELD_TRANSACTIONS_URL=http://127.0.0.1:3003 VELD_CARDS_URL=http://127.0.0.1:3004 \
       VELD_NOTIFICATIONS_URL=http://127.0.0.1:3005
(cd backend/iam-service          && PORT=3001 npm start)
(cd backend/account-service      && PORT=3002 npm start)
(cd backend/card-service         && PORT=3004 npm start)
(cd backend/notification-service && PORT=3005 npm start)
(cd backend/transaction-service  && mvn -q package && PORT=3003 java -jar target/transaction-service.jar)
(cd web && npm run dev)                     # http://localhost:5173, proxies /api/<service>
```

New accounts get 1000.00 of demo money; the Java service keeps a ledger and answers
`InsufficientFunds` when a transfer exceeds it. Accounts whose email ends in `@nexusbank.example`
are staff and may call `banAccount`.

## Tests

`cargo test -p veld --test examples_e2e` regenerates (nothing may change), runs `npm ci` and
`tsc --noEmit` on every app, compiles the Java service with `javac`, boots all five services on free
ports, drives them over HTTP (sign up, open an account, statement, transfer, typed errors across
services and languages, cards, notifications, logout) and renders the React app with
`react-test-renderer` against them. `npm test` in `web/` does the render step by hand.

Other commands: `veld generate --dry-run`, `veld generate transactions-api` (one target),
`veld export openapi` (OpenAPI 3.1 for the whole workspace).
