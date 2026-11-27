# `veld.toml` — v2

Status: **DRAFT — frozen together with `language.md`.**

A project is a **workspace**: a root `veld.toml` that lists the members and holds what is shared, and one small `veld.toml` per member (a service, an app or a library) in the member's own directory. A project with one member keeps one file. `veld` finds the file by searching upward from the current directory. Paths in a file are resolved relative to that file. Any unknown key is an error (`E0701`), and the message lists the closest valid key. The layout of earlier betas (`[project]`, `[services.*]`, `[libraries.*]`, `[[targets]]`) is an error (`E0709`) that says what to write instead.

`workspace.md` is the specification of the layout: the files, `members`, defaults, member names and target names, `-p`, `--affected` and `owners`. This file holds what does not depend on the layout: frameworks and options, `veld setup`, the lock files, generated output and registry packages. The sections "Layout in short" and "Rules" below repeat what a user needs from `workspace.md`.

## Layout in short

One file, one service (the **single file** form):

```toml
[service]
name = "shop"              # package name of the service; required in a root file
contracts = "contracts"    # package root (default "contracts")

[server]
emitter = "node"
framework = "express"      # express | fastify | hono | node-http (node server)
out = "../api/src/generated"

[client]
emitter = "typescript"
out = "../web/src/generated"
```

A workspace (microservices). The root file:

```toml
[workspace]
name = "nexusbank"
members = ["services/*", "apps/*", "libs/*"]

[workspace.defaults.server.node]     # a target of that role and emitter starts from these
framework = "express"

[plugins.kotlin]
command = "veld-emit-kotlin"         # resolved on PATH or relative path
version = "1.2.0"
entry = "plugins/kotlin/main.js"     # optional: the script an interpreter `command` runs
sha256 = "…"                         # of `entry` if set, else of the executable; mismatch = refuse (E0705)

[registry]
url = "https://registry.example.com"

[dependencies]
"@acme/billing" = "^1.2"             # resolved and pinned in veld.lock
```

A service, `services/orders/veld.toml`:

```toml
[service]
consumes = ["iam", "payments"]   # cycles allowed; `name` defaults to the directory name
base-url = "http://orders:3002"
owners = ["team-orders"]

[server]
name = "orders-api"              # optional; used by `veld generate <name>`
emitter = "node"
out = "src/generated"            # relative to this file; default depends on the emitter
# For each service in `consumes`, a client in the same emitter's language is generated
# under <out>/clients/<service>/. Set `clients = false` to opt out.
```

An app, `apps/web/veld.toml`, and a library, `libs/common/veld.toml`:

```toml
[app]
uses = ["iam", "orders"]         # default: every service

[client]
emitter = "typescript"
framework = "react"
out = "src/generated"
```

```toml
[library]                        # shared contracts, importable as `use common:money.{Money}`
```

A Java server with options:

```toml
[server]
emitter = "java"
framework = "spring"             # jdk | spring

[server.options]                 # emitter-specific, validated by the emitter's typed Options
package = "com.acme.payments"    # java: the base Java package
```

`[workspace]`, `[registry]`, `[dependencies]`, `[plugins.<name>]`, `[lint]` and `[hooks]` are allowed only in the root file (`E0711` in a member file). A target is a `[server]` or `[client]` table, or an array of them (`[[server]]`, `[[client]]`) for more than one of a role; the table name is the role.

## Frameworks and options per emitter

`framework` is optional and is validated by the generator for the target's role. An unknown value is `E0803`, and the message lists the valid values. A role that takes no framework rejects any value.

| Emitter | Role | `framework` values (versions) | `[server.options]` or `[client.options]` keys |
|---|---|---|---|
| `node` | server | `express`, `fastify`, `hono`, `node-http` (unversioned), `nest` (`"10"`) | `typescript`, `framework-version`, `package-name`, `testing`, `messaging` |
| `typescript` | client | `react` (`"18"`), `vue` (`"3"`); none is the plain client | `typescript`, `framework-version`, `package-name` |
| `java` | server | `jdk` (unversioned), `spring` (`"3"`) | `package`, `java`, `framework-version`, `validate`, `testing` |
| `java` | client | none | `package`, `java` |
| `rust` | server | `axum` (`"0.8"`, the default) | `crate-name`, `transport`, `validate`, `testing` |
| `rust` | client | `reqwest` (`"0.13"`, the default), `ureq` (`"2"`, blocking) | `crate-name`, `validate` |

`messaging` (`node` server): `"kafka"`, `"rabbitmq"` or a list of both generates the broker adapters and names their library as a peer dependency (`spec/events.md` section 10.7).

Streaming (`get ... -> stream`, `ws`) works with every framework above, except that a `rust` client on `ureq` reports `E0801` for `ws` endpoints (the blocking transport has no WebSocket client; use `reqwest`) and that a `java` server with `framework = "jdk"` reports `E0801` for `ws` endpoints (the JDK has no WebSocket server; use `spring`). A `node` server with `ws` endpoints adds a `ws` peer dependency to its `package.json` and needs `ws` installed by the application; a `spring` server with `ws` endpoints needs `spring-boot-starter-websocket`.

The frameworks of a built-in emitter come from its adapter registry, one slice per emitter, so this table, the `E0803` message and the generated adapters cannot drift apart.

### Language version and framework version

Three options are resolved **once**, centrally, into a `Profile` before the generator runs. Generators query the profile (`java_at_least(21)`) and never compare raw version strings.

```toml
# services/orders/veld.toml
[server]
emitter = "java"
framework = "spring"
out = "src/main/java"

[server.options]
java = "21"                # the language: "17" (default, the oldest supported) or "21"
framework-version = "3"    # the framework version: Spring Boot 3
```

| Option | Emitters | Values | Default |
|---|---|---|---|
| `java` | `java` | `"17"`, `"21"` | `"17"` |
| `typescript` | `typescript`, `node` | `"es2020"`, `"es2022"` | `"es2020"` |
| `framework-version` | any emitter whose framework has versions | the framework's versions (`spring`: `"3"`, `nest`: `"10"`, `react`: `"18"`, `vue`: `"3"`) | the first version |

Checks, all reported before generating, with a message that lists the valid values:

- An unknown language version is `E0802`.
- A `framework-version` outside the adapter's list, a `framework-version` without a `framework`, or one for an unversioned framework is `E0803`.
- A language older than the framework's `min_language` is `E0803`.

The language and `framework-version` keys belong to the core: they are removed before the emitter's own typed options are checked. For a `plugin:<name>` emitter the language key is the language `id` of the plugin's `hello` (`kotlin = "2.0"`), see `plugin-protocol.md`. Options are checked against the emitter's typed options: an unknown key is `E0802`. A `plugin:<name>` emitter declares its own values and options.

## Importable packages and `veld setup`

Every `node` and `typescript` target writes a `package.json` into its `out`: name `@veld/<target-name>` (override with the target option `package-name`), `private`, no dependencies, `main` and `types` pointing at `index.ts`. A `react`, `vue` or `nest` target declares its framework as a `peerDependency` only. Consumed-service clients of a server are re-exported by the same package as namespaces (`import { transactions } from '@veld/accounts-api'`).

`veld setup [target] [--dry-run]` wires each target into the app that consumes it. The app is the target's `app = "path"` key (relative to the member's `veld.toml`), else the nearest ancestor of `out`, up to the workspace root, that has a `package.json` (TypeScript) or a `pom.xml`, `build.gradle.kts` or `build.gradle` (Java).

- **TypeScript:** adds `"@veld/<name>": "file:<relative out>"` to the app's `dependencies`, then run `npm install`. A `file:` dependency is ordinary package resolution, so it works for `tsc`, Node (`tsx`), Vite and test runners alike; a `tsconfig` `paths` alias would only serve `tsc`.
- **Maven:** adds a `build-helper-maven-plugin` `add-source` execution `veld-<target>` for `out` (nothing to do when `out` is `src/main/java`).
- **Gradle:** appends a `// veld:begin <id>` ... `// veld:end <id>` block with `sourceSets.main.java.srcDir(...)`.

Edits keep the file's formatting, are idempotent, and never touch unrelated keys. When a file cannot be edited safely, setup prints the exact snippet to paste. `veld generate` prints a one-line hint while a target is not wired yet.

## Rules

- Every `[service]` and `[library]` member defines one contracts package, named like the member. A single file with one `[service]` is a complete project.
- `consumes` entries MUST name existing services (`E0702`). Listing the service itself is a warning (`W0701`). Cycles are allowed. `consumes = ["*"]` means every other service (it can be combined with named services and is expanded when the config is loaded).
- `prefix` (optional, in `[workspace]` and in `[service]`) is a route prefix such as `/api` that is prepended to every service prefix of the service in the IR, so every target and export agrees on the full route. The workspace prefix comes first: `[workspace] prefix = "/api"` and `[service] prefix = "/v1"` give `/api/v1/<service prefix>`. A prefix MUST start with `/`; trailing slashes are dropped.
- `[lint]` maps a lint rule name to `"off"`, `"warn"` or `"error"` (`veld check` runs the rules; see `spec/diagnostics.md`, codes `L####`). An unknown rule name is a configuration error that lists the valid names; rules not listed keep their default level (`warn`). `veld check` exits 1 when a rule set to `error` finds something.
- `[hooks]` (optional): `post-generate` is a shell command or a list of commands. `veld generate` runs them in order, from the directory of the root `veld.toml`, once per run, after a successful write (never on `--dry-run` or `--check`); their output is streamed. A failing hook is a warning and generate still exits 0, unless `fail = true` is set, which makes it exit 1. An unknown key under `[hooks]` is `E0701`.
- A target needs an `emitter`. Its role is the table name (`[server]` or `[client]`), and the services it serves come from the member (`workspace.md`, section 6).
  - `out` is relative to the member's directory. It has a default for each built-in emitter (`src/generated` for `node` and `typescript`, `src/main/java` for `java`, `generated` for `rust`); a plugin must set it (`E0700`).
  - `app` (optional) is the directory of the app that consumes the output, for `veld setup`.
  - Two targets MUST NOT share an `out` directory (`E0703`). The message names both targets and the directory.
  - `out` MUST NOT be the workspace root or a contracts directory (`E0704`).
  - A target's `name` selects it in `veld generate <name>` and names the package `@veld/<name>`. It defaults to the member's name, to `<member>-server` and `<member>-client` when the member has both, and must be set when a member has several targets of one role (`E0710`).
- `[plugins.x].entry` is optional. When set, Veld runs `command` with the entry path appended (unless `command` already contains it), and `sha256` is the hash of the entry file, checked before every run. Without `entry`, `sha256` is the hash of the executable that `command` resolves to. For a script plugin always set `entry`, because hashing the interpreter protects nothing.
- `emitter` is either a built-in id (`node`, `typescript`, `java`, `rust`) or `plugin:<name>`.
- `base-url` resolution in generated clients, highest priority first:
  1. the constructor argument
  2. the env var `VELD_<SERVICE>_URL` (service name in UPPER_SNAKE)
  3. the config `base-url`
  4. otherwise an error at client construction
- Generated output never contains absolute paths, user names, hostnames or timestamps.

## `veld.lock.ir.json`

`veld generate` (for every member and every target, not a dry run, no conflicts: a run with `-p`, `--affected` or inside one member does not write it) writes `veld.lock.ir.json` next to the root `veld.toml`: the compiled IR as pretty JSON with sorted, deterministic content. It is a snapshot of the contract as last generated. **Commit it.** `veld diff --lock` (the default of `veld diff`) compares the current contracts against this snapshot and reports breaking and non-breaking changes, so a pull request shows exactly what a contract change does to the wire. The file is rewritten only when its content changes. It is distinct from `veld.lock` (pinned registry packages).

## Generated output

Each `out` directory holds a manifest, `.veld-manifest.json`, listing the files Veld wrote with their content hashes.

- `veld generate` never deletes or overwrites a file that Veld did not write (it reports it as a conflict).
- A listed file whose content differs from its manifest hash is **user-modified**. If generating would overwrite or delete it, `generate` refuses and names it ("you edited generated file ..."). A dry run reports it too. `--force` overwrites it.
- `veld clean` removes unmodified generated files and keeps user-modified ones. It leaves a manifest that lists only the kept files with their original hashes, so a later `generate` still reports them as user-modified.
- A write starts by removing stale `.veld-staging-*` directories in `out`, left by an interrupted run. Generated paths may not use that prefix.
- `veld generate --check` writes nothing and exits 1 when any target would create, update or delete a file, or has a user-modified or unowned file in the way. Before writing, `generate` compares the contracts with `veld.lock.ir.json`: a breaking change, or a possibly breaking one (a rule whose expression changed, a non-additive change to an event or a channel: the tool cannot tell the direction, as in `veld diff --strict`), is confirmed on a terminal, fails off a terminal or with `--strict`, and passes with `--accept-breaking` (`--check` and `--dry-run` only print the list).
- `veld export openapi -o FILE` writes a file the user owns: staged and contained like generated output, but with no manifest, so `generate` and `clean` never touch it. A differing existing file is refused unless `--force`.

## Registry packages

- `[registry].url` is the registry; the flags `--registry` and `VELD_REGISTRY` override it. Registry URLs must be `https`, except for `localhost`/`127.0.0.1` (or `--allow-insecure`).
- `veld add @org/name[@range]` writes `[dependencies]`, resolves the whole dependency graph, installs it and pins it in `veld.lock`. `veld install` installs exactly the locked graph, direct and transitive, and verifies every checksum. A checksum mismatch is a hard error.
- `veld.lock` is TOML. Each `[[package]]` entry has `name`, `version`, `sha256` of the tarball, `registry` and `dependencies`, the sorted names of the packages that entry depends on. Every package in the graph has an entry, so the lock is closed: a dependency that is not locked makes the lock invalid. The output is deterministic (sorted by name).
- **Transitive dependencies.** A package's own `[dependencies]` are written to its `veld-package.toml` by `veld publish` (a `[dependencies]` table of `"@org/name" = "range"`), and the registry validates names and ranges at publish time and returns them as `dependencies` in `GET /api/v1/packages/{org}/{name}/versions`. The resolver reads them from that listing, so resolving never downloads a package.
  - **One version per package.** The resolver picks the highest version that satisfies every range in the graph (the version already pinned in `veld.lock` is kept when it still fits, so `veld add` does not upgrade unrelated packages). Two versions of a package are never installed side by side, so `use @org/name:...` is unambiguous. Deprecated versions are skipped. Prerelease versions match only ranges that name a prerelease of the same `major.minor.patch`.
  - **Conflicts** are a hard error that shows the dependency paths, for example `veld.toml → @acme/billing@1.0.0 → @acme/money@^1` next to `veld.toml → @acme/payroll@1.0.0 → @acme/money@^2`. Nothing is changed.
  - **Cycles** between packages are legal (contract imports may cycle by design) and resolve as long as the versions are consistent; an inconsistent cycle is reported as a conflict marked `(dependency cycle)`.
  - `veld add` updates `veld.toml`, `veld.lock` and the cache as a unit: on any failure nothing is changed. `veld install` and `veld add` also check that every installed package's manifest declares the dependencies the lock records for it, and that each locked version satisfies the ranges of the packages that depend on it.
  - **Visibility.** The project may `use` only the packages listed in its own `[dependencies]`; a registry package may `use` only the packages in its own `[dependencies]`. Importing a package that is merely installed because something else needs it is `E0206`, with a help line saying where to declare it.
- Installed packages live in `.veld/packages/@org/name/<version>/` (gitignored). `veld check` and `veld generate` install what `veld.lock` pins and the cache lacks; without a lock they never touch the network.
- To publish, the `[service]` or `[library]` table of a member sets `package = "@org/name"`, `version` (semver) and optionally `description`. `veld publish [--package <name>]` packs `contracts/**.veld` plus `veld-package.toml` (`name`, `version`, `language = "2"`, `[files]` sha256 checksums) into a deterministic tar.gz.
- API tokens are stored per registry URL in `~/.veld/credentials.toml` (`$VELD_HOME` overrides the directory) and are never printed.
