# Migrating from Veld 1 to Veld 2

Veld 2.0 is a rewrite in Rust with a new contract language and a new config file. `veld migrate`
converts a v1 project; this guide shows what it does, what you do by hand, and how every v1 feature
maps to v2. The short version is in [`CHANGELOG.md`](../../CHANGELOG.md).

## What changed in one table

| v1 | v2 |
|---|---|
| Go binary (`go install`, npm, pip, brew, composer, nuget, choco, apt) | one self-contained Rust binary (npm, pip, Homebrew, Chocolatey, Scoop, apt/rpm, `cargo install`); no Go toolchain |
| `veld.config.json` | `veld.toml` ([`spec/config.md`](../../spec/config.md)) |
| `module`, `action`, `import @alias/x`, `description:` | `service`, endpoints, `use pkg:module.{Name}`, `///` doc comments |
| `veld validate` | `veld check` (includes lint) |
| `veld watch` | `veld generate --watch` (`veld watch` stays as an alias) |
| `veld diff --exit-code` (generated vs on disk) | `veld generate --check`; the new `veld diff` reports contract changes |
| `veld openapi`, `graphql`, `schema`, `docs`, `ast` | `veld export openapi`, `graphql`, `schema`, `docs`, `ir` |
| `.veld.lock.json`, `.veld-cache.json` | `veld.lock.ir.json` (commit it); the cache is gone |
| backends: node, python, go, java, csharp, php, rust | `node` (Express, Fastify, Hono, node:http, NestJS), `java` (JDK, Spring Boot 3); others as plugins |
| frontends: typescript, react, dart, kotlin, swift, ... | `typescript` (plain, React hooks, Vue composables) and `java` clients; others as plugins |
| errors were strings (`errors: [NotFound]`) | errors are declarations with a status and fields |
| `long` was a JSON number | `long` is a decimal string on the wire; every error is `application/problem+json` |

Veld 2 supports Node, TypeScript and Java. The v1 Python, Go, C#, PHP and Rust backends and the
Dart, Kotlin, Swift, Svelte and Angular frontends are not built in: that was a deliberate scope
decision so that each generator could be held to the same wire-level tests. They can be written as
emitter plugins, see [`docs/emitter-guide.md`](../emitter-guide.md).

## Step by step

1. **Keep v1 safe.** `veld migrate` never modifies v1 files, but commit or tag first anyway.
2. **Install Veld 2** (see [getting-started.md](getting-started.md)).
3. **Migrate.** In the directory with `veld.config.json` (or give `--from DIR`):

   ```bash
   veld migrate --dry-run         # preview
   veld migrate                   # --out DIR changes the contracts folder (default: contracts)
   ```

   ```
   ✓ wrote contracts/models/todo.veld
   ✓ wrote contracts/modules/todos.veld
   ✓ wrote veld.toml
   ✓ migrated: 5 file(s) written, 0 skipped
   → run `veld check` in veld
   warning: v1 config key `aliases` was not migrated: imports are rewritten as `use` lines, so aliases are not needed
   warning: frontend target `react` of `mg`: v2 generates the plain `typescript` client; ...
   warning: contracts: modules/users.veld: error `UserExists` is not in the v2 prelude; declared `error UserExists` with status 409 (status guessed from the name; v1 errors carry none)
   ```

   Nothing is dropped silently: whatever cannot be mapped is named in a warning and, where it has a
   place in the file, in a `// TODO(veld migrate): ...` comment.
4. **Read the warnings** and fix what they say (see "What migrate cannot decide" below).
5. **Check, format, generate:**

   ```bash
   veld check        # fix what it reports; codes are in spec/diagnostics.md
   veld fmt
   veld generate
   veld setup        # then npm install
   ```
6. **Replace the generated code in your app.** The generated API is different (below).
7. **Update scripts and CI** to the new commands, see [ci.md](ci.md).

## The contract language

v1:

```
import @models/todo

module Todos {
  description: "Todo management"
  prefix: /todos

  action GetTodo {
    description: "Get a todo by ID"
    method: GET
    path: /:id
    output: Todo
    errors: [NotFound]
  }
}
```

v2 (the real output of `veld migrate` on the v1 `node-react` example):

```veld
use models.todo.{CreateTodoInput, Todo, UpdateTodoInput}

/// Todo management
service Todos at "/todos" {
  /// Get a todo by ID
  get "/{id}" getTodo(id: string) -> Todo | NotFound
}
```

| v1 | v2 |
|---|---|
| `model X { description: "..." ... }` | `/// ...` doc comment above `model X { ... }` |
| `field?: T`, `T[]`, `Map<string,T>` | unchanged (`name?: T`, `T[]`, `Map<string, T>`) |
| `field: T = default`, `@unique`, `@index`, `@example`, `@relation`, `@autoincrement` | `field: T = default` and the same annotations (check the output: migrate may drop one it cannot map, and says so in a warning) |
| `enum X { a, b }` | `enum X { a, b }`; wire values with `a = "A"` |
| `constants` | `const NAME: type = literal` |
| `module M { prefix: /x ... }` | `service M at "/x" { ... }` |
| `action A { method: GET path: /:id input: In output: Out errors: [E] }` | `get "/{id}" a(id: string, body: In) -> Out \| E` (the input becomes `body`, `query` or path parameters, by binding) |
| path `/:id` | path `/{id}` |
| `errors: [NotFound]` | `\| NotFound` in the response position; non-prelude errors become `error Name STATUS { ... }` declarations |
| `middleware: [RequireAuth]` | `@auth(bearer)` when it looks like authentication, else `@middleware(RequireAuth)` |
| `method: WS`, `stream:` | `ws "/path" name(body: stream T) -> stream U`; `get ... -> stream T` for SSE |
| `import @models/user`, `import "./x.veld"` | `use models.user.{Name}`; v1 syntax is error `E0001` with a hint to run `veld migrate` |
| `extends`, unions | `model A extends B { }`; `union U { arm: Model }` with a `kind` discriminator |
| status codes | written in the response: `-> 201 Order`; default is 201 for `post`, else 200, 204 with no body |

Things that are new and worth adopting: `@length`, `@min`/`@max`, `@pattern`, `@format(email)`,
`@readonly`, `@wire("name")`, `@deprecated("msg")`, `@auth` per service or endpoint, the built-in
`Page<T>`, and declared error fields. The language reference is
[`spec/language.md`](../../spec/language.md).

## The config file

```json
{ "input": "app.veld", "backend": "node", "frontend": "react",
  "backendOut": "../backend/src/generated", "frontendOut": "../frontend/src/generated" }
```

becomes

```toml
[project]
name = "mg"
contracts = "contracts"

[[targets]]
name = "mg-api"
role = "server"
emitter = "node"
service = "mg"
out = "../backend/src/generated"

[[targets]]
name = "mg-client"
role = "client"
emitter = "typescript"
out = "../frontend/src/generated"
```

| v1 key | v2 |
|---|---|
| `input`, `aliases` | `[project].contracts` (a folder, all `.veld` files in it); aliases are replaced by `use` |
| `backend`, `backendOut`, `backendFramework` | a `[[targets]]` with `role = "server"`, `emitter`, `framework`, `out` |
| `frontend`, `frontendOut`, `frontendFramework` | a `[[targets]]` with `role = "client"` |
| `backendDir`, `frontendDir` | `app = "path"` on the target (for `veld setup`) |
| workspace entries with `baseUrl`, `consumes` | `[services.<name>]` with `base-url` and `consumes` ([microservices.md](microservices.md)) |
| `registry`, `dependencies` | `[registry]` and `[dependencies]` (you re-add packages with `veld add`) |
| `validate` flag | gone: validators are always generated |
| `hooks.postGenerate` | planned as `[hooks] post-generate` (see `docs/v1-parity.md`) |
| `tools` (openapi, database, dockerfile, cicd, env, scaffold-tests) | `veld export ...` and `role = "spec"` targets; the Docker, CI, `.env` and test scaffolds are planned as exports or plugin samples |

Output safety is stricter in v2: two targets may not share an `out` (`E0703`), and an `out` may not
be the project root or a contracts folder (`E0704`). A v1 project that wrote several backends into
one folder needs one folder per target.

## What migrate cannot decide

- **Backends and frontends without a v2 emitter** (Python, Go, C#, PHP, Rust, Dart, Kotlin, Swift,
  Svelte, Angular): written as commented-out targets. Keep v1 for them, or write a plugin.
- **React and Vue.** `react` maps to the `typescript` client; add `framework = "react"` (or `"vue"`)
  to the target for hooks. The v2 hooks have no dependencies; they do not use TanStack Query.
- **Error statuses.** v1 errors carried none, so a non-prelude error gets a status guessed from its
  name (`UserExists` becomes 409). Review each one.
- **Auth.** Middleware that looks like authentication becomes `@auth(bearer)`; check the scheme.
- **Registry settings and dependencies**, and the app-level `prefix` (the prefix of every module):
  re-create them in `veld.toml` or in the service paths (`service Todos at "/api/todos"`).
- **Field-level literal unions** (`status: "A" | "B"`) migrate as `json`; declare an `enum` instead.

## The generated code

You must change your hand-written code, because the output is different by design:

| v1 | v2 |
|---|---|
| route files that you registered on your app | `create<Service>Handler(impl, hooks)` plus an adapter: `toExpress`, `toFastify`, `toHono`, `toNodeHttp` or the NestJS module |
| `import { ... } from '../generated/...'` (relative paths) | `import { ... } from '@veld/<target>'` after `veld setup` |
| a generated `api` client instance, `VeldApiClient` | one client per service: `new HelloClient({ baseUrl })` |
| thrown strings or error codes | typed error classes; `NotFound.matches(e)`; throw `new NotFound({ resource })` on the server |
| React hooks on TanStack Query | zero-dependency hooks: `useGreet(req)`, `useCreateOrder()`, `VeldProvider` |
| service SDK (`consumes`) | generated automatically for every `consumes` entry |
| JSON numbers for 64-bit integers | `long` is a decimal string; `decimal` is a string |
| unvalidated by default | requests and responses are validated against the contract (`422` `ValidationError`, problem+json) |

See [generated-output.md](generated-output.md) for imports per target, and
[`examples/`](../../examples) for working servers and apps.

## Commands

| v1 | v2 |
|---|---|
| `veld init` | `veld init` (same prompts; `--yes` and flags for scripts) |
| `veld validate` | `veld check` |
| `veld generate [--dry-run] [--strict] [--force]` | `veld generate [--dry-run] [--force]`; the breaking-change gate is `veld diff --strict` and, planned, a check inside `generate` |
| `veld diff --exit-code` | `veld generate --check` |
| `veld watch` | `veld generate --watch` |
| `veld clean` | `veld clean` (keeps files you edited) |
| `veld setup` | `veld setup` (now also Maven and Gradle) |
| `veld lint` | `veld check` (lint rules, tunable under `[lint]`) |
| `veld openapi` | `veld export openapi` |
| `veld graphql`, `schema`, `docs` | `veld export graphql`, `schema`, `docs` |
| `veld ast` | `veld export ir` |
| `veld deps` | validation runs in `veld check` (`E0702`) |
| `veld ci` | `veld generate` then `veld setup` |
| `veld push` / `veld pull` | `veld publish` / `veld add`, `veld install` |
| `veld login`, `logout` | unchanged |
| `veld doctor` (hidden) | `veld doctor` |
| `veld completion` | planned |

## Editors and CI

The VS Code extension and the JetBrains plugin are rewritten as thin clients of `veld lsp`, and keep
their commands, snippets and `veld.toml` schema, see [`editors/`](../../editors). Replace your v1 CI
steps with the four checks in [ci.md](ci.md).

## If something does not migrate

Open `veld check` output first: every diagnostic has a stable code listed in
[`spec/diagnostics.md`](../../spec/diagnostics.md). The v1 sources, docs and examples stay in
[`archive/v1/`](../../archive/v1) (tag `v1-final`) for reference. They are not maintained.
