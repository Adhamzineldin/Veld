# Getting started with Veld

Veld turns `.veld` contracts into typed servers and clients. You describe the API once; Veld writes
the server skeleton you implement and the client you call, and both agree on every route, type and
error. This guide takes you from nothing to a running Node server and a typed client in about ten
minutes. Everything here is checked against Veld 2.0 (`2.0.0-beta.1`).

## 1. Install

`veld` is one self-contained binary with no runtime to install. Pick any channel:

```bash
npm install -g @maayn/veld                  # npm
pip install maayn-veld                      # pip
brew install maayn-veld/tap/maayn-veld      # Homebrew
choco install veld --pre                    # Chocolatey (Windows)
scoop install https://github.com/Adhamzineldin/Veld/releases/latest/download/veld.json   # Scoop
sudo apt install ./veld_<version>_amd64.deb # Debian and Ubuntu, from the release page (.rpm for Fedora)
cargo install --git https://github.com/Adhamzineldin/Veld veld   # from source
```

Check it works with `veld --version`. The wrappers download the release binary for your platform
and verify its SHA-256, see [`distribution/`](../../distribution).

You need Node 18 or newer for a Node server or a TypeScript client, and a JDK 17 or newer for a Java
target. Veld itself needs neither.

## 2. Create a project

```bash
mkdir shop && cd shop
veld init --yes --name shop
```

```
✓ created veld.toml, contracts/hello.veld, package.json
✓ created .gitignore (ignores .veld/ and node_modules/)

Next steps
  edit contracts/hello.veld  describe your API
  veld generate              write the generated code
  veld setup                 make it importable: @veld/<target> (npm) or a source root (Maven/Gradle)
  npm install                in each app, after `veld setup`
```

Without `--yes`, `veld init` asks what you want (server and client targets, frameworks, a
workspace). Flags answer the questions up front: `--server node:express`, `--client typescript:react`,
`--workspace users,orders`. `veld init` never overwrites an existing file.

You now have a config and one contract:

```toml
# veld.toml
[project]
name = "shop"
contracts = "contracts"

[[targets]]
name = "api"
role = "server"
emitter = "node"
framework = "express"
service = "shop"
out = "generated/server"

[[targets]]
name = "web"
role = "client"
emitter = "typescript"
out = "generated/client"
```

```veld
// contracts/hello.veld
/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Greets the person with the given name.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
```

A **model** is a record, a **service** groups endpoints under a path prefix, and an endpoint reads
as `method "path" name(params) -> success | errors`. `NotFound` is a built-in error, so you do not
declare it. The whole language is in [`spec/language.md`](../../spec/language.md); every
`veld.toml` key is in [`spec/config.md`](../../spec/config.md).

Each `[[targets]]` entry is one generated output: a `role` (`server` or `client`), an `emitter`
(`node`, `typescript` or `java`) and the folder `out` it writes into. Veld writes only inside `out`.

## 3. Check the contract

```bash
veld check
```

```
✓ 1 service, 1 endpoint, 1 type in 1ms
```

Mistakes are reported with a code, the source line and a caret, like a compiler:

```
error[E0205]: unknown type `Strng`
 --> shop/hello.veld:2:6
  |
2 |   x: Strng
  |      ^^^^^
```

`veld check` also runs the lint rules (unused models, empty models, missing descriptions and so on),
which you tune under `[lint]` in `veld.toml`. Every code is listed in
[`spec/diagnostics.md`](../../spec/diagnostics.md).

## 4. Generate

```bash
veld generate
```

```
✓ api -> generated/server: 7 created, 0 updated, 0 deleted, 0 unchanged
✓ web -> generated/client: 8 created, 0 updated, 0 deleted, 0 unchanged
2 targets, 15 files changed in 21ms
```

Add `--dry-run` to see what would change without writing. Veld never overwrites a file it did not
write and never overwrites a generated file you edited, see
[generated-output.md](generated-output.md).

## 5. Make it importable

```bash
veld setup
npm install
```

`veld setup` adds `"@veld/api": "file:generated/server"` to the `dependencies` of the app that
consumes the target (and does the same for a Maven or Gradle build), then `npm install` links it.
After that you import the generated code like any package, never by a relative path into
`generated/`. Run `veld setup --dry-run` to see exactly what it will change first.

## 6. Use the generated code

**Server.** Implement the generated `HelloService` and mount the handler on Express
(this is [`examples/hello`](../../examples/hello)):

```ts
import express from 'express';
import { NotFound, createHelloHandler, toExpress } from '@veld/api';
import type { GreetRequest, HelloService } from '@veld/api';

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
app.listen(3000);
```

Throwing a generated error answers with its status as `application/problem+json`. The handler also
validates every request, so `greet` only ever sees a well-typed `name`.

**Client.** The same contract gives a typed client:

```ts
import { HelloClient, NotFound } from '@veld/web';

const client = new HelloClient({ baseUrl: 'http://localhost:3000' });
try {
  const greeting = await client.greet({ name: 'Ada' });   // typed as Greeting
  console.log(greeting.message);
} catch (e) {
  if (NotFound.matches(e)) console.log('no such person');
  else throw e;
}
```

With `framework = "react"` (or `"vue"`) on the client target you also get hooks, such as
`useGreet({ name })`. See [generated-output.md](generated-output.md) for what every target and
framework produces and how to import it.

## 7. Watch for changes

```bash
veld generate --watch
```

Veld regenerates whenever a `.veld` file or `veld.toml` changes. A contract with an error is
reported and the last good output stays in place, so a dev server keeps running.

## 8. Everyday commands

| Command | What it does |
|---|---|
| `veld check` | Compile every contract and report errors, warnings and lint findings |
| `veld generate [target]` | Generate every target, or one by name; `--dry-run`, `--watch`, `--force` |
| `veld generate --check` | Fail if the generated files on disk are stale (for CI) |
| `veld fmt [--check]` | Format `.veld` files canonically; comments are preserved |
| `veld diff [--strict]` | Show breaking and non-breaking contract changes since the last generate |
| `veld export openapi -o FILE` | OpenAPI 3.1 for the whole project (also `graphql`, `schema`, `docs`, `ir`) |
| `veld setup` | Wire the generated packages into your apps |
| `veld clean` | Remove generated files you have not edited |
| `veld doctor` | Check that the config, the tools and the output folders are in order |

`veld --help` lists everything your build has. A few of the commands above (`generate --check`,
`doctor` and the other `export` formats) are the planned names from
[`docs/v1-parity.md`](../v1-parity.md); a build that predates them reports an unknown command or
option.

## 9. Editors

The VS Code extension and the JetBrains plugin give you diagnostics, hover, go to definition,
completion, rename and formatting from `veld lsp`, plus commands for check, generate and watch,
snippets, and completion for `veld.toml`. See [`editors/vscode`](../../editors/vscode) and
[`editors/jetbrains`](../../editors/jetbrains).

## Next steps

- [generated-output.md](generated-output.md): what each target generates and how to import it
- [microservices.md](microservices.md): several services that call each other
- [ci.md](ci.md): keep contracts, generated code and the wire format honest in CI
- [migrating-from-v1.md](migrating-from-v1.md): if you have a v1 project
- [`examples/`](../../examples): complete projects with committed output
