# `veld.toml` — v2

Status: **DRAFT — frozen together with `language.md`.**

Each project has exactly one config file, `veld.toml`. It lives at the project root, and `veld` finds it by searching upward from the current directory. Paths in it are resolved relative to the file. Any unknown key is an error (`E0701`), and the message lists the closest valid key.

## Single service (minimal)

```toml
[project]
name = "shop"              # package name of the implicit default service
contracts = "contracts"    # package root (default "contracts")

[[targets]]
role = "server"
emitter = "node"
framework = "express"      # express | fastify | hono | node-http (node server)
out = "../api/src/generated"

[[targets]]
role = "client"
emitter = "typescript"
out = "../web/src/generated"
```

## Workspace (microservices)

```toml
[project]
name = "nexusbank"

[libraries.common]          # shared contracts, importable as `use common:money.{Money}`
contracts = "contracts/common"

[services.iam]
contracts = "contracts/iam"
base-url = "http://iam:3001"

[services.orders]
contracts = "contracts/orders"
base-url = "http://orders:3002"
consumes = ["iam", "payments"]   # cycles allowed

[services.payments]
contracts = "contracts/payments"
consumes = ["orders"]

[[targets]]
name = "orders-api"              # optional; used by `veld generate <name>`
service = "orders"
role = "server"
emitter = "node"
framework = "express"
out = "services/orders/src/generated"
# For each service in `consumes`, a client in the same emitter's language is generated
# under <out>/clients/<service>/. Set `clients = false` to opt out.

[[targets]]
role = "client"
services = ["iam", "orders"]     # default: all services
emitter = "typescript"
out = "web/src/generated"

[[targets]]
role = "server"
service = "payments"
emitter = "java"
framework = "spring"             # jdk | spring
out = "services/payments/src/main/java"

[targets.options]                # emitter-specific, validated by the emitter's typed Options
package = "com.acme.payments"    # java: the base Java package

[plugins.kotlin]
command = "veld-emit-kotlin"     # resolved on PATH or relative path
version = "1.2.0"
entry = "plugins/kotlin/main.js" # optional: the script an interpreter `command` runs
sha256 = "…"                     # of `entry` if set, else of the executable; mismatch = refuse (E0705)

[registry]
url = "https://registry.example.com"

[dependencies]
"@acme/billing" = "^1.2"         # resolved and pinned in veld.lock
```

## Frameworks and options per emitter

`framework` is optional and is validated by the generator for the target's role. An unknown value is `E0803`, and the message lists the valid values. A role that takes no framework rejects any value.

| Emitter | Role | `framework` values (versions) | `[targets.options]` keys |
|---|---|---|---|
| `node` | server | `express`, `fastify`, `hono`, `node-http` (unversioned) | `typescript` |
| `typescript` | client | none | `typescript` |
| `java` | server | `jdk` (unversioned), `spring` (`"3"`) | `package`, `java`, `framework-version` |
| `java` | client | none | `package`, `java` |

The frameworks of a built-in emitter come from its adapter registry, one slice per emitter, so this table, the `E0803` message and the generated adapters cannot drift apart.

### Language version and framework version

Three options are resolved **once**, centrally, into a `Profile` before the generator runs. Generators query the profile (`java_at_least(21)`) and never compare raw version strings.

```toml
[[targets]]
role = "server"
service = "orders"
emitter = "java"
framework = "spring"
out = "services/orders/src/main/java"

[targets.options]
java = "21"                # the language: "17" (default, the oldest supported) or "21"
framework-version = "3"    # the framework version: Spring Boot 3
```

| Option | Emitters | Values | Default |
|---|---|---|---|
| `java` | `java` | `"17"`, `"21"` | `"17"` |
| `typescript` | `typescript`, `node` | `"es2020"`, `"es2022"` | `"es2020"` |
| `framework-version` | any emitter whose framework has versions | the framework's versions (`spring`: `"3"`) | the first version |

Checks, all reported before generating, with a message that lists the valid values:

- An unknown language version is `E0802`.
- A `framework-version` outside the adapter's list, a `framework-version` without a `framework`, or one for an unversioned framework is `E0803`.
- A language older than the framework's `min_language` is `E0803`.

The language and `framework-version` keys belong to the core: they are removed before the emitter's own typed options are checked. For a `plugin:<name>` emitter the language key is the language `id` of the plugin's `hello` (`kotlin = "2.0"`), see `plugin-protocol.md`. Options are checked against the emitter's typed options: an unknown key is `E0802`. A `plugin:<name>` emitter declares its own values and options.

## Rules

- Exactly one of `[project].contracts` or `[services.*]` defines the packages. With no `[services]` section, the project itself is a single service named `project.name`.
- `consumes` entries MUST name existing services (`E0702`). Listing the service itself is a warning (`W0701`). Cycles are allowed.
- A target needs `role`, `emitter` and `out`.
  - `service` is required for `role = "server"`.
  - Two targets MUST NOT share an `out` directory (`E0703`).
  - `out` MUST NOT be the project root or a contracts directory (`E0704`).
- `[plugins.x].entry` is optional. When set, Veld runs `command` with the entry path appended (unless `command` already contains it), and `sha256` is the hash of the entry file, checked before every run. Without `entry`, `sha256` is the hash of the executable that `command` resolves to. For a script plugin always set `entry`, because hashing the interpreter protects nothing.
- `emitter` is either a built-in id (`node`, `typescript`, `java`) or `plugin:<name>`.
- `base-url` resolution in generated clients, highest priority first:
  1. the constructor argument
  2. the env var `VELD_<SERVICE>_URL` (service name in UPPER_SNAKE)
  3. the config `base-url`
  4. otherwise an error at client construction
- Generated output never contains absolute paths, user names, hostnames or timestamps.

## `veld.lock.ir.json`

`veld generate` (for all targets, not a dry run, no conflicts) writes `veld.lock.ir.json` next to `veld.toml`: the compiled IR as pretty JSON with sorted, deterministic content. It is a snapshot of the contract as last generated. **Commit it.** `veld diff --lock` (the default of `veld diff`) compares the current contracts against this snapshot and reports breaking and non-breaking changes, so a pull request shows exactly what a contract change does to the wire. The file is rewritten only when its content changes. It is distinct from `veld.lock` (pinned registry packages).

## Generated output

Each `out` directory holds a manifest, `.veld-manifest.json`, listing the files Veld wrote with their content hashes.

- `veld generate` never deletes or overwrites a file that Veld did not write (it reports it as a conflict).
- A listed file whose content differs from its manifest hash is **user-modified**. If generating would overwrite or delete it, `generate` refuses and names it ("you edited generated file ..."). A dry run reports it too. `--force` overwrites it.
- `veld clean` removes unmodified generated files and keeps user-modified ones. It leaves a manifest that lists only the kept files with their original hashes, so a later `generate` still reports them as user-modified.
- A write starts by removing stale `.veld-staging-*` directories in `out`, left by an interrupted run. Generated paths may not use that prefix.

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
- To publish, a `[project]`, `[libraries.*]` or `[services.*]` section sets `package = "@org/name"`, `version` (semver) and optionally `description`. `veld publish [--package <name>]` packs `contracts/**.veld` plus `veld-package.toml` (`name`, `version`, `language = "2"`, `[files]` sha256 checksums) into a deterministic tar.gz.
- API tokens are stored per registry URL in `~/.veld/credentials.toml` (`$VELD_HOME` overrides the directory) and are never printed.
