# Veld

**Write a `.veld` contract once. Generate typed servers and clients.**

Veld is a contract-first API code generator. You describe your API in a small language (models, enums,
errors, services). Veld compiles the contract and generates a typed server, a typed client, or both.
The generated code has no runtime dependencies, is deterministic (same contract, same bytes), and
never overwrites files you wrote.

Version 2.0 is a rewrite in Rust with a new language. The v1 Go implementation is frozen in
[`archive/v1/`](archive/v1) (git tag `v1-final`).

> Status: `2.0.0-beta.1`. The specs in [`spec/`](spec) are normative.

## Try it in the browser

The website playground (`/playground`, source in `website/`) runs the real compiler and generators, compiled to WebAssembly (`crates/veld-wasm`), with no install. Its output is byte-identical to the CLI; a parity test enforces that.

## Install

From source (needs a Rust toolchain):

```bash
git clone https://github.com/Adhamzineldin/Veld
cd Veld
cargo build -p veld --release       # produces target/release/veld, one self-contained binary
```

The npm, pip and Homebrew wrappers in [`distribution/`](distribution) download the release binary
for your platform and verify its SHA256 (they work once a release is published):

```bash
npm install -g @maayn/veld
pip install maayn-veld
brew install maayn-veld/tap/maayn-veld
```

Shell completions (bash, zsh, fish, powershell, elvish) are printed by a hidden command:

```bash
veld completions bash > ~/.local/share/bash-completion/completions/veld
veld completions zsh  > "${fpath[1]}/_veld"
veld completions fish > ~/.config/fish/completions/veld.fish
veld completions powershell | Out-String | Invoke-Expression   # add to $PROFILE
```

## A 60-second tour

```bash
veld init          # asks a few questions (or use flags); creates veld.toml and contracts
veld check         # compile the contracts
veld generate      # write the server and client
veld setup         # make them importable as @veld/<target>
```

A contract (see [`spec/language.md`](spec/language.md)):

```veld
use iam:users.{User}

/// A customer order.
model Order {
  id: uuid @readonly
  customer: User
  items: OrderItem[] @length(min: 1)
  status: Status = .pending
  note?: string @length(max: 500)
}

enum Status { pending, paid, shipped }

error OutOfStock 409 {
  sku: string
}

/// Order management.
@auth(bearer)
service Orders at "/orders" {
  /// Fetch one order.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound

  post "/" createOrder(body: CreateOrder) -> 201 Order | OutOfStock

  get "/" listOrders(query: OrderFilter) -> Page<Order>

  @auth(none)
  get "/health" health() -> 204
}
```

The project file, `veld.toml` (see [`spec/config.md`](spec/config.md)):

```toml
[service]
name = "shop"

[server]
emitter = "node"
framework = "express"
out = "api/src/generated"

[client]
emitter = "typescript"
out = "web/src/generated"
```

One file is a whole project. A larger project is a workspace: a root `veld.toml` that lists the
members, and a small `veld.toml` in the directory of each service, app or library
([Microservices](#microservices-and-mutual-consumes)).

Prelude errors (`NotFound`, `Unauthorized`, `ValidationError`, ...) and `Page<T>` need no
declaration. Veld validates requests on the server and in the client, and maps errors to RFC 9457
`application/problem+json`.

More complete projects are in [`examples/`](examples): `hello`, `bookings` (business rules),
`microservices` and `library-package`. Each has hand-written servers and frontends that import the generated code, and
end-to-end tests run them (the generated output is committed and checked to be up to date).

## Using it in your code

Generated folders are npm packages (`@veld/<target name>`), and `veld setup` wires them into the app
that consumes them, so your code never imports a relative path into `src/generated`. This is the
[`hello` example](examples/hello), exactly as it is in the repository:

```bash
veld generate        # writes server/src/generated and web/src/generated (each with a package.json)
veld setup           # adds "@veld/api" and "@veld/web" as file: dependencies of the two apps
npm install          # in server/ and web/
```

The server implements the generated `HelloService` (`server/src/main.ts`):

```ts
// The hello server: implement the generated `HelloService` and mount its handler on Express.
// `@veld/api` is the generated package (see `veld setup`), not a relative path.
import express from 'express';
import { NotFound, createHelloHandler, toExpress } from '@veld/api';
import type { GreetRequest, HelloService } from '@veld/api';

/** The business logic. Throw a generated error to answer with its HTTP status. */
export const hello: HelloService = {
  async greet({ name }: GreetRequest) {
    if (name.toLowerCase() === 'nobody') {
      throw new NotFound({ resource: name });
    }
    return { message: `Hello, ${name}!` };
  },
};

const app = express();
app.use(toExpress(createHelloHandler(hello)));

const port = Number(process.env.PORT ?? 3000);
app.listen(port, '127.0.0.1', () => {
  console.log(`hello server listening on http://127.0.0.1:${port}`);
});
```

The React app (`web/src/App.tsx` and `web/src/Greeter.tsx`) uses the generated client and hooks
(the client target has `framework = "react"`):

```tsx
import { HelloClient } from '@veld/web';
import { VeldProvider } from '@veld/web/hello/react';
import { Greeter } from './Greeter';

/** One client for the whole app. Same origin: Vite proxies `/hello` to the server in development. */
const client = new HelloClient({ baseUrl: window.location.origin });

export function App() {
  return (
    <VeldProvider clients={{ hello: client }}>
      <h1>Hello</h1>
      <Greeter />
    </VeldProvider>
  );
}
```

```tsx
import { useState } from 'react';
import { NotFound } from '@veld/web';
import { useGreet } from '@veld/web/hello/react';

/** Greets whoever you type. `nobody` makes the server answer with the contract's typed NotFound. */
export function Greeter() {
  const [name, setName] = useState('');
  // A null request holds the call back until there is a name.
  const greeting = useGreet(name === '' ? null : { name });
  return (
    <section>
      <label>
        Name <input value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      {greeting.loading && <p role="status">Loading...</p>}
      {greeting.data && <p role="status">{greeting.data.message}</p>}
      {NotFound.matches(greeting.error) && <p role="alert">Nobody called {name} was found.</p>}
    </section>
  );
}
```

For Java, `veld setup` adds the generated sources to your Maven (`build-helper-maven-plugin`) or
Gradle (`sourceSets.main.java.srcDir`) build and you import the generated packages normally. A
server also gets the clients of the services it consumes, through the same package
(`import { iam, transactions } from '@veld/accounts-api'`). See [`examples/microservices`](examples/microservices).

## Business rules

A contract can state what must always be true. A `rule` in a model, `require` and `ensure` on an endpoint,
a `flow` on an enum (the legal state changes) and a `policy` (who may call) are written once. The generated
server checks the request before your handler runs and the result after it. The generated client checks the
rules before it sends a request, and has helpers for the states and the policies. `example` blocks run
offline with `veld test`, and against the test handler of a running server with `veld test --live`.
The TypeScript, Node, Java and Rust generators support them. See the
[business rules guide](docs/guides/rules.md), [testing with examples](docs/guides/testing.md) and the
[`bookings` example](examples/bookings).

## Targets

| Emitter | Role | Output | Spec |
|---|---|---|---|
| `node` | server | TypeScript: handler, service interface, adapters for `express`, `fastify`, `hono` or plain `node:http` (structural types, no imports) | [`spec/targets/typescript.md`](spec/targets/typescript.md) |
| `typescript` | client | typed client classes, models, errors, validators | [`spec/targets/typescript.md`](spec/targets/typescript.md) |
| `java` | server and client | Java 17 records, sealed interfaces, `java.net.http` client; JDK HTTP server adapter or Spring Boot 3 adapter | [`spec/targets/java.md`](spec/targets/java.md) |
| `rust` | server and client | a Cargo crate: serde models, typed errors, an axum server (framework-agnostic core, axum adapter), a client generic over its HTTP transport (`reqwest` or blocking `ureq` adapters) | [`spec/targets/rust.md`](spec/targets/rust.md) |
| `plugin:<name>` | any | whatever an external emitter plugin produces | [`spec/plugin-protocol.md`](spec/plugin-protocol.md) |

Streaming is supported: Server-Sent Events (`get ... -> stream T`) and WebSockets (`ws`) in the `node` server, the `typescript` client (with React hooks and Vue composables), and `java` (WebSockets on the JDK server are `E0801`, they need `framework = "spring"`) and `rust` (an axum server, a `reqwest` client; a `ureq` client reports `E0801` for `ws`). The only generated-code dependency exception is WebSocket: the `ws` package for a Node server that hosts `ws` endpoints (the adapter is typed structurally and imports nothing), `spring-boot-starter-websocket` for Spring; clients use the platform `WebSocket` and `java.net.http.WebSocket`.

If a target cannot represent a feature, generation fails with
`E0801`. It never emits wrong code. The `typescript` client can add React hooks or Vue composables (`framework = "react" | "vue"`), and
the `node` server also supports `nest`. `veld export openapi` produces an OpenAPI 3.1 document.

### Query options, without a query library

With `query-options = true` on a `typescript` client target, Veld also writes `queries.ts` next to each package's client: query keys, query options and mutation options as plain objects. The generated code imports no package, so you decide what to use them with:

```toml
[client]
emitter = "typescript"
out = "web/src/generated"

[client.options]
query-options = true
```

```ts
import { ordersQueries, ordersMutations } from '@veld/web';

// TanStack Query (your dependency, not Veld's)
const order = useQuery(ordersQueries.getOrder({ id }));
const create = useMutation({
  ...ordersMutations.createOrder(),
  onSuccess: () => queryClient.invalidateQueries({ queryKey: ordersQueries.keys.all }),
});
// SWR or your own hook: key = ordersQueries.keys.getOrder({ id }), fetcher = the queryFn
```

Keys are built from the canonical bytes of the request, so equal requests share a key. Pass a client as the last argument (`ordersQueries.getOrder(req, client)`) or build a set with your own default: `createOrdersQueries(() => myClient)`. Errors keep their generated classes (`NotFound.matches(e)`); `OrdersGetOrderError` is the union a call can fail with. See [`spec/targets/typescript.md`](spec/targets/typescript.md) section 4a.1.

### Skipping run-time request validation

`validate = false` in `[server.options]` or `[client.options]` of a `node`, `typescript`, `java` or `rust` target skips run-time request validation by default and keeps the types. The client option `validate: true` (TypeScript) or `ClientOptions.validate(true)` (Java) switches it back on.

## Microservices and mutual consumes

A workspace is a root `veld.toml` that lists its members, and one `veld.toml` per member next to
its code. Each service says which others it calls:

```toml
# veld.toml (the root)
[workspace]
name = "bank"
members = ["services/*", "apps/*"]
```

```toml
# services/accounts/veld.toml
[service]
consumes = ["iam", "transactions"]

[server]
emitter = "node"
```

```toml
# services/transactions/veld.toml
[service]
consumes = ["iam", "accounts"]     # accounts and transactions call each other

[server]
emitter = "java"
```

Cycles are legal. Every server target also gets a generated client for each consumed service, in
the server's own language, and modules can import each other's types across services
(`use accounts:accounts.{Account}`). Shared contracts live in a `[library]` member.
Server targets of different languages can sit in one workspace: the
[`microservices` example](examples/microservices) has Node servers and one Java server.

Inside a member's directory `veld generate` works on that member; `-p accounts` picks one from
anywhere, `--workspace` takes them all, and `veld generate --affected --since origin/master` takes
the members that a change can have affected. See [`docs/guides/workspaces.md`](docs/guides/workspaces.md).

## Signature-based equality

Every model, enum, union and error carries a **signature** computed once by the compiler from its
wire shape: `veld:<package>/<Name>@<hash>`. Every generated target embeds the signature verbatim, and
`same(a, b)` compares signatures, the operation and the canonical value bytes, never class identity.

This gives the "same `NotFound`" guarantee: the `NotFound` raised by `orders.getOrder` is recognised
as the same error by every copy of the generated code, in any directory or language, and it is never
equal to the `NotFound` of `payments.getPayment`. See [`spec/ir.md`](spec/ir.md) section 5. The
repository tests this with generated TypeScript and Java that call each other
(`tests/toolchain`, `apps/veld/tests/toolchain.rs`).

## Commands

| Command | What it does |
|---|---|
| `veld init` | Create `veld.toml`, starter contracts and a `README.md`. On a terminal with no flags it asks (project, single service or workspace, servers, client, output folders) and ends by offering to run `generate` and `setup`; it starts from an existing `package.json` or `pom.xml`. Flags `--name`, `--workspace a,b`, `--server node:express`, `--client typescript:react`, `--template crud` (users, auth and pagination), `--base-url SERVICE=URL` and `--consumes SERVICE=a,b` (a workspace asks per service for its base URL and which services it calls; nothing is consumed unless you say so), `--yes` answer the same questions without a TTY |
| `veld check [PATH...]` | Compile all contracts, lint them (`[lint]` in `veld.toml` sets each rule to `off`, `warn` or `error`) and report diagnostics. `PATH...` limits the report to those files or folders, `--no-lint` skips lint, `--strict` makes warnings fail, `--graph [text\|dot\|json]` prints the service dependency graph |
| `veld generate [target]` | Generate code for every target, or one by name. `--dry-run` writes nothing; `--check` writes nothing and exits 1 when any output is stale (CI), `--diff` adds unified diffs to either. Before it writes over a breaking change, or a possibly breaking one such as a changed rule, it asks on a terminal, fails with `--strict` or off a terminal, and continues with `--accept-breaking`. `--setup` runs `veld setup` afterwards, `--watch` regenerates on every change (`veld watch` is a hidden alias). `--force` overwrites files you edited. `[hooks] post-generate` in `veld.toml` runs after a successful write. |
| `veld setup [target]` | Wire generated targets into your apps: `@veld/<target>` `file:` dependencies in `package.json`, a source root in `pom.xml` or `build.gradle(.kts)`. `--dry-run` shows the edits; idempotent and format preserving |
| `veld clean` | Remove generated files that you have not modified; `--lock` also removes `veld.lock.ir.json`, the breaking-change baseline |
| `veld doctor` | Check that the config loads, packages and the registry lock resolve, emitters and plugins resolve with matching hashes, output folders are writable, and that `git`, `node`, `npm`, `javac` exist for the wired targets, plus `mvn` or `gradle` for Java server targets that build with them. A fix is printed under every problem; exit 1 on failure |
| `veld fmt` | Format `.veld` files canonically |
| `veld diff` | Show breaking, possibly breaking (a rule expression changed) and non-breaking contract changes (`--strict` exits 1 on a breaking or possibly breaking one) |
| `veld test [PATH...]` | Run the `example` blocks of the contracts offline (`--filter TEXT`), or against a running test handler (`--live BASE_URL`, `--dry-run`, `--fuzz N --seed S`); see [the testing guide](docs/guides/testing.md) |
| `veld export <what>` | `openapi` prints OpenAPI 3.1 JSON to stdout or `-o FILE` (`--service S`, repeatable, limits it); `ir` prints the compiled IR for plugin authors. A file you own is written without a manifest and is never overwritten without `--force` |
| `veld plugin new` / `plugin test` | Scaffold an emitter plugin, run the protocol conformance suite |
| `veld add` / `install` / `publish` / `login` / `logout` | Registry packages (`veld.lock` pins versions and checksums) |
| `veld registry info\|search\|versions\|token create` | Look things up in the registry and create API tokens (the commands that change a project stay top-level) |

Global flags: `-v` / `-vv` show phases, write-plan entries and timings on stderr, `-q` prints only
errors and warnings, `--config PATH` names the `veld.toml`, `--no-color` turns colour off.

Generation only ever writes inside a target's `out` directory, through a staged writer with a
manifest. Files that Veld did not write are never touched.

## Editor support

- VS Code: [`editors/vscode`](editors/vscode), a thin client for the language server in the binary
  (`veld lsp`): diagnostics, hover, go to definition, completion, rename, formatting.
- JetBrains (IntelliJ IDEA, RustRover, WebStorm 2024.3+): [`editors/jetbrains`](editors/jetbrains), a
  thin client that runs `veld lsp` through LSP4IJ, with TextMate highlighting from the same grammar.

## Repository layout

| Path | What |
|---|---|
| `spec/` | Normative specs: language, IR, config, plugin protocol, targets |
| `crates/` | The Rust workspace: compiler, IR, codegen, project writer, tools, emitters |
| `apps/veld` | The CLI |
| `registry/` | The package registry server (Go) |
| `examples/` | Complete example projects |
| `tests/` | Conformance cases, toolchain round trips, wire goldens |
| `archive/v1/` | The v1 Go implementation, reference only |

## Documentation

- [`docs/`](docs/README.md): index of guides and specs
- [`docs/guides/rules.md`](docs/guides/rules.md): business rules in the contract, with the [`bookings` example](examples/bookings)
- [`docs/guides/testing.md`](docs/guides/testing.md): `example` blocks and `veld test`
- [`docs/emitter-guide.md`](docs/emitter-guide.md): writing an emitter
- [`CONTRIBUTING.md`](CONTRIBUTING.md), [`CHANGELOG.md`](CHANGELOG.md), [`CLAUDE.md`](CLAUDE.md)
