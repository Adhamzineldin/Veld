# Veld

**Write a `.veld` contract once. Generate typed servers and clients.**

Veld is a contract-first API code generator. You describe your API in a small language (models, enums,
errors, services). Veld compiles the contract and generates a typed server, a typed client, or both.
The generated code has no runtime dependencies, is deterministic (same contract, same bytes), and
never overwrites files you wrote.

Version 2.0 is a rewrite in Rust with a new language. The v1 Go implementation is frozen in
[`archive/v1/`](archive/v1) (git tag `v1-final`). Existing v1 projects are converted with `veld migrate`.

> Status: `2.0.0-beta.1`. The specs in [`spec/`](spec) are normative.

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
[project]
name = "shop"

[[targets]]
role = "server"
emitter = "node"
framework = "express"
service = "shop"
out = "api/src/generated"

[[targets]]
role = "client"
emitter = "typescript"
out = "web/src/generated"
```

Prelude errors (`NotFound`, `Unauthorized`, `ValidationError`, ...) and `Page<T>` need no
declaration. Veld validates requests on the server and in the client, and maps errors to RFC 9457
`application/problem+json`.

More complete projects are in [`examples/`](examples): `hello`, `microservices` and
`library-package`. Each has hand-written servers and frontends that import the generated code, and
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

## Targets

| Emitter | Role | Output | Spec |
|---|---|---|---|
| `node` | server | TypeScript: handler, service interface, adapters for `express`, `fastify`, `hono` or plain `node:http` (structural types, no imports) | [`spec/targets/typescript.md`](spec/targets/typescript.md) |
| `typescript` | client | typed client classes, models, errors, validators | [`spec/targets/typescript.md`](spec/targets/typescript.md) |
| `java` | server and client | Java 17 records, sealed interfaces, `java.net.http` client; JDK HTTP server adapter or Spring Boot 3 adapter | [`spec/targets/java.md`](spec/targets/java.md) |
| `plugin:<name>` | any | whatever an external emitter plugin produces | [`spec/plugin-protocol.md`](spec/plugin-protocol.md) |

If a target cannot represent a feature (for example streaming in Java), generation fails with
`E0801`. It never emits wrong code. The `typescript` client can add React hooks or Vue composables (`framework = "react" | "vue"`), and
the `node` server also supports `nest`. `veld export openapi` produces an OpenAPI 3.1 document.

## Microservices and mutual consumes

A workspace lists services, and each service says which others it calls:

```toml
[services.accounts]
contracts = "contracts/accounts"
consumes = ["iam", "transactions"]

[services.transactions]
contracts = "contracts/transactions"
consumes = ["iam", "accounts"]     # accounts and transactions call each other
```

Cycles are legal. Every server target also gets a generated client for each consumed service, in
the server's own language, and modules can import each other's types across services
(`use accounts:accounts.{Account}`). Shared contracts live in a `[libraries.<name>]` package.
Server targets of different languages can sit in one workspace: the
[`microservices` example](examples/microservices) has Node servers and one Java server.

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
| `veld init` | Create `veld.toml` and starter contracts. On a terminal with no flags it asks (project, single service or workspace, servers, client, output folders); flags `--name`, `--workspace a,b`, `--server node:express`, `--client typescript:react`, `--yes` answer the same questions without a TTY |
| `veld check` | Compile all contracts and report diagnostics |
| `veld setup [target]` | Wire generated targets into your apps: `@veld/<target>` `file:` dependencies in `package.json`, a source root in `pom.xml` or `build.gradle(.kts)`. `--dry-run` shows the edits; idempotent and format preserving |
| `veld generate [target]` | Generate code for every target, or one by name. `--dry-run` writes nothing. Refuses to overwrite or delete a generated file you edited (`--force` overrides) |
| `veld clean` | Remove generated files that you have not modified |
| `veld fmt` | Format `.veld` files canonically |
| `veld diff` | Show breaking and non-breaking contract changes (`--strict` exits 1 on a breaking one) |
| `veld export openapi` / `export ir` | OpenAPI 3.1, or the compiled IR as JSON |
| `veld migrate` | Convert a v1 project to v2 |
| `veld plugin new` / `plugin test` | Scaffold an emitter plugin, run the protocol conformance suite |
| `veld add` / `install` / `publish` / `login` / `logout` | Registry packages (`veld.lock` pins versions and checksums) |

Generation only ever writes inside a target's `out` directory, through a staged writer with a
manifest. Files that Veld did not write are never touched.

## Editor support

- VS Code: [`editors/vscode`](editors/vscode), a thin client for the language server in the binary
  (`veld lsp`): diagnostics, hover, go to definition, completion, rename, formatting.
- JetBrains: the v1 plugin is frozen. The v2 plan (LSP4IJ with `veld lsp`) is in
  [`editors/jetbrains`](editors/jetbrains).

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
- [`docs/emitter-guide.md`](docs/emitter-guide.md): writing an emitter
- [`CONTRIBUTING.md`](CONTRIBUTING.md), [`CHANGELOG.md`](CHANGELOG.md), [`CLAUDE.md`](CLAUDE.md)

## Migrating from v1

```bash
veld migrate --from path/to/v1/project   # writes veld.toml and contracts/ (v1 files are not touched)
veld check
veld generate
```

Constructs without a v2 equivalent are reported as warnings, and unsupported v1 backends are written
as commented-out targets. See [`CHANGELOG.md`](CHANGELOG.md).
