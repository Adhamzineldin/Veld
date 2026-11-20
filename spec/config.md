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

| Emitter | Role | `framework` values | `[targets.options]` keys |
|---|---|---|---|
| `node` | server | `express`, `fastify`, `hono`, `node-http` | none |
| `typescript` | client | none | none |
| `java` | server | `jdk`, `spring` | `package` |
| `java` | client | none | `package` |

Options are checked against the emitter's typed options: an unknown key is `E0802`. A `plugin:<name>` emitter declares its own values and options.

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
- `veld add @org/name[@range]` writes `[dependencies]`, resolves the highest matching non-deprecated version, installs it and pins it in `veld.lock` (TOML: `name`, `version`, `sha256` of the tarball, `registry`). `veld install` installs exactly the locked versions. A checksum mismatch is a hard error.
- Installed packages live in `.veld/packages/@org/name/<version>/` (gitignored). `veld check` and `veld generate` install what `veld.lock` pins and the cache lacks; without a lock they never touch the network.
- To publish, a `[project]`, `[libraries.*]` or `[services.*]` section sets `package = "@org/name"`, `version` (semver) and optionally `description`. `veld publish [--package <name>]` packs `contracts/**.veld` plus `veld-package.toml` (`name`, `version`, `language = "2"`, `[files]` sha256 checksums) into a deterministic tar.gz.
- API tokens are stored per registry URL in `~/.veld/credentials.toml` (`$VELD_HOME` overrides the directory) and are never printed.
