# Workspaces: one `veld.toml` per member

A Veld project is a **workspace**: a root `veld.toml` that lists the members and holds what is
shared, and one small `veld.toml` per member, next to the member's code. A member is a **service**
(contracts and servers), an **app** (clients of services) or a **library** (shared contracts). A
small project keeps one file. The full specification is [`spec/workspace.md`](../../spec/workspace.md);
the keys of each table are in [`spec/config.md`](../../spec/config.md).

## One file

A `veld.toml` with one member table and no `[workspace]` is a complete workspace:

```toml
[service]
name = "hello"

[server]
emitter = "node"
```

The service is `hello`, its contracts are in `contracts/`, and its server is generated into
`src/generated` (the default `out` of the `node` emitter). `veld init` writes this form.

## A workspace

```
shop/
  veld.toml                 workspace root
  veld.lock                 pinned registry packages
  veld.lock.ir.json         the contract snapshot of the last full generate
  services/
    bookings/   veld.toml   contracts/   src/
    payments/   veld.toml   contracts/   src/main/java/
  apps/
    web/        veld.toml   src/
  libs/
    money/      veld.toml   contracts/
```

```toml
# veld.toml (root)
[workspace]
name = "shop"
members = ["services/*", "apps/*", "libs/*"]

[workspace.defaults.server.node]
framework = "express"

[workspace.defaults.server.java]
framework = "spring"
java = "21"

[registry]
url = "https://registry.shop.dev"
```

```toml
# services/bookings/veld.toml
[service]
consumes = ["payments"]
owners = ["team-bookings"]

[server]
emitter = "node"
```

```toml
# apps/web/veld.toml
[app]
uses = ["bookings"]           # default: every service

[client]
emitter = "typescript"
framework = "react"
```

```toml
# libs/money/veld.toml
[library]
```

- `members` entries are paths relative to the root. A segment may be `*`; nothing else is a pattern.
  `exclude` removes directories again. A directory without a `veld.toml` is skipped.
- The member is named after its directory unless its table sets `name`. The member in the root file
  must set `name`. Names are unique, and `consumes`, `uses` and `use bookings:...` refer to them.
- `[registry]`, `[dependencies]`, `[plugins.<name>]`, `[lint]` and `[hooks]` belong in the root file
  only. In a member file each is `E0711`.
- Paths in a file (`contracts`, `out`, `app`) are relative to that file.

### Targets

A target is a `[server]` or `[client]` table in a member file, or an array (`[[server]]`) for more
than one. The table name is the role; the member gives the services. A server serves its service; a
client in a service covers that service; a client in an app covers the app's `uses`. An app has no
server and a library has no targets (`E0711`).

- `out` defaults to `src/generated` (`node`, `typescript`), `src/main/java` (`java`) and `generated`
  (`rust`). A plugin has no default. Two targets must not share or nest their `out` (`E0703`), so a
  member with a `node` server and a `typescript` client sets `out` on at least one.
- The target name selects it in `veld generate <name>` and names the importable package
  `@veld/<name>`. It defaults to the member's name, or `<member>-server` and `<member>-client` when
  the member has both. Several targets of one role each need a `name`. Write `name` when you move a
  project and want its package names to stay.
- `[workspace.defaults.<role>.<emitter>]` holds the `framework` and the option keys a target of that
  role and emitter starts from. What the member writes replaces the default key by key, and a
  default `framework-version` is dropped when the member writes its own `framework`.

## Working on one member

| Where and how | What `generate`, `watch`, `clean`, `setup` and `test` work on |
|---|---|
| in the root directory, or anywhere outside a member | every member |
| inside a member's directory | that member |
| `-p <name>` (`--package`, repeatable) | the named members, whatever their kind |
| `--workspace` | every member |
| `--affected --since <ref>` | the members that a change can have affected |

Compiling always reads every member, because contracts import across members; the selection decides
what is written. `veld.lock.ir.json` is written only by a run that selects every member and every
target. `veld check`, `veld diff`, `veld fmt` and the exports cover the whole workspace.

### `--affected`

```bash
veld generate --affected --since origin/master
```

```
affected since origin/master:
  payments   contracts changed
  bookings   consumes payments
  web        uses bookings
```

The changed paths are the files that differ between the merge base of the ref and `HEAD`, and the
working tree, from `git`. A member is changed when its `veld.toml`, its contracts or the `out` of one
of its targets changed, and every member is changed when the root `veld.toml` or `veld.lock` changed.
A member is affected when it is changed or depends on an affected member: through `consumes`, `uses`
or a contract `use`. Without a git work tree, or with an unknown ref, the run stops with `E0712`.
`--affected` combines with `--check` and `--dry-run`: `veld generate --affected --since origin/master
--check` is the pull request check that fails when generated code of an affected member is stale.

### `owners`

`owners = ["team-bookings"]` in a member table is a list of free-form names. Veld never checks them
and they never reach generated code. `veld diff` prints, for each package with a breaking or possibly
breaking change, its owners and the members that depend on it with theirs, and `veld check
--graph=json` adds `owners` to a service's node.

## Moving a project from the earlier layout

A `veld.toml` with `[project]`, `[services.<name>]`, `[libraries.<name>]` or `[[targets]]` is the
error `E0709`, and the message names the tables it found. There is no converter: where each member's
directory goes is a decision. The tables become:

| Earlier | Now |
|---|---|
| `[project] name` | `[workspace] name`, or the `name` of the only member |
| `[project] contracts` | `[service] contracts` (default `contracts`) |
| `[project] prefix` | `[workspace] prefix` |
| `[services.x]` | `[service]` in the `veld.toml` of member `x` |
| `[libraries.x]` | `[library]` in the `veld.toml` of member `x` |
| `[[targets]] role = "server" service = "x"` | `[server]` in the `veld.toml` of member `x` |
| `[[targets]] role = "client" services = [...]` | `[client]` in an `[app]` member with `uses = [...]`, or in the service's own file |
| `[targets.options]` | `[server.options]`, `[client.options]` |

An example. Before:

```toml
[project]
name = "shop"

[services.orders]
contracts = "contracts/orders"

[[targets]]
name = "orders-api"
role = "server"
emitter = "node"
framework = "express"
service = "orders"
out = "orders/src/generated"

[[targets]]
name = "web"
role = "client"
emitter = "typescript"
out = "web/src/generated"
```

After, in three files:

```toml
# veld.toml
[workspace]
name = "shop"
members = ["orders", "web"]
```

```toml
# orders/veld.toml
[service]
contracts = "../contracts/orders"      # the contracts may stay where they are

[server]
name = "orders-api"                    # keep the name: it is the package `@veld/orders-api`
emitter = "node"
framework = "express"
out = "src/generated"
```

```toml
# web/veld.toml
[app]

[client]
name = "web"
emitter = "typescript"
out = "src/generated"
```

Generated files, manifests and `veld.lock.ir.json` are the same bytes afterwards. Check it with
`veld generate --check`: it must report that the output is up to date.
