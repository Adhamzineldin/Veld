# Workspace layout of `veld.toml`

Status: **DRAFT. Not implemented.** When it is implemented, this text replaces the sections "Single service", "Workspace" and "Rules" of `config.md`, and the layout those sections describe is no longer accepted. Everything else in `config.md` (frameworks and options, `veld setup`, lock files, generated output, registry packages) stays as it is.

Proposal: `docs/proposals/rules-and-events.md` section 13. This spec covers the layout, member selection, `--affected` and `owners`. The typed runtime config of the same section is a separate change with its own spec, because it changes generated code and this one does not.

## 1. Why

One `veld.toml` holds every service of a project today, and the targets of each service sit in repeated `[[targets]]` blocks far from the service. In a repository with many services that file is edited by every team. This spec moves each service's settings next to its code:

- The **workspace root** lists the members and holds what is shared.
- Each **member** (a service, an app or a library) has its own small `veld.toml` in its own directory.
- A small project keeps one file: a `veld.toml` with one member and no `[workspace]` table is a complete workspace.

The loader turns either shape into the same resolved project that the compiler and the generators receive today. Nothing after the loader changes, so the generated files, the manifests and both lock files of a project are the same bytes before and after it moves to this layout (section 13).

## 2. An example

```
shop/
  veld.toml                 workspace root
  veld.lock
  veld.lock.ir.json
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
uses = ["bookings"]

[client]
emitter = "typescript"
framework = "react"
```

```toml
# libs/money/veld.toml
[library]
```

The service is named `bookings` after its directory, its contracts are in `services/bookings/contracts`, and its server is generated into `services/bookings/src/generated` with the `express` framework from the defaults.

The smallest project is one file:

```toml
[service]
name = "hello"

[server]
emitter = "node"
```

## 3. Files

A `veld.toml` holds a `[workspace]` table, one member table (`[service]`, `[app]` or `[library]`), or both.

| The file has | It is |
|---|---|
| `[workspace]` | The workspace root. It may also hold one member table: then the root directory is a member too. |
| a member table and no `[workspace]`, and no workspace above claims it | A workspace of its own with one member (the **single file** form). |
| a member table and no `[workspace]`, listed by a workspace above | A member of that workspace. |

- A file with neither a `[workspace]` table nor a member table is `E0710`.
- A file with more than one member table is `E0710`.
- A member file MUST NOT hold a `[workspace]` table unless it is the root: a workspace inside a workspace is `E0710`.
- Any unknown key is `E0701`, as today, and the message lists the closest valid key.
- Paths in a file are relative to that file's directory.

### 3.1 Finding the workspace

`veld` searches upward from the current directory for a `veld.toml` (or uses `--config <path>`), then:

1. If the file has `[workspace]`, it is the root.
2. Otherwise the file is a member file, in directory `M`. The search continues upward from the parent of `M` for a `veld.toml` that has `[workspace]`.
   - None is found: the member file is the root of a single file workspace.
   - One is found and its `members` match `M` (section 4.1): it is the root, and the member in `M` is the **current member** (section 9).
   - One is found and its `exclude` matches `M`: the member file is the root of a single file workspace.
   - One is found and neither matches: `E0710`. The message names both files and says to add the directory to `members` or to `exclude`.

## 4. The root file

```toml
[workspace]
name = "shop"                         # required
description = "The shop"              # optional
members = ["services/*", "apps/*"]    # default: none
exclude = ["services/legacy"]         # default: none
prefix = "/api"                       # optional route prefix for every service
```

- `name` is the project name. In the single file form the project name is the member's name.
- `prefix` works as `[project].prefix` does today: it is prepended to the prefix of every service, before the service's own `prefix`.

These tables are allowed only in the root file (which the single file form is): `[workspace]`, `[registry]`, `[dependencies]`, `[plugins.<name>]`, `[lint]`, `[hooks]`. Their keys and meaning do not change. In a member file below a root each of them is `E0711`, and the message says the table belongs in the workspace root and gives its path.

- Registry dependencies stay one set for the whole workspace, pinned in the root `veld.lock`: one version of a package per workspace.
- `[hooks].post-generate` runs once per `veld generate`, from the root directory, whatever members were selected.

### 4.1 `members` and `exclude`

Each entry is a path relative to the root, with `/` as the separator. A path segment may be `*`, which matches any one directory name. No other pattern syntax exists: `**`, `?`, `[...]`, a `*` inside a longer segment (`svc-*`), an absolute path and a `..` segment are `E0710`.

- An entry without `*` names one directory. It MUST exist and hold a `veld.toml` (`E0710` otherwise).
- An entry with `*` matches every directory of that shape that holds a `veld.toml`. A matching directory without a `veld.toml` is skipped.
- A directory matched by `exclude` is not a member, whatever `members` says.
- A directory matched twice is one member.
- Members are ordered by their path, compared as bytes with `/` as the separator, on every system.
- The root directory is a member exactly when the root file holds a member table. It is not written in `members`.

### 4.2 Defaults

```toml
[workspace.defaults.server.node]
framework = "express"

[workspace.defaults.client.typescript]
framework = "react"
typescript = "es2022"
```

`[workspace.defaults.<role>.<emitter>]` holds `framework` and any option key of that emitter for that role. `<role>` is `server` or `client`. `<emitter>` is a built-in id or `plugin:<name>` (quoted in TOML).

A target of that role and emitter starts from the defaults, and each key the member writes replaces the default for that key:

- `framework` in the member replaces the default `framework`. A default `framework-version` belongs to the default framework, so it is dropped when the member writes `framework`.
- Options are merged key by key: `[server.options]` in the member wins.

Defaults are checked when the workspace is loaded, whether or not a member uses them: an unknown role or emitter is `E0701`, and a framework or an option the generator rejects is `E0803` or `E0802` as for a target.

The proposal sketched the defaults one level flatter (`java = { framework = "spring" }`). They are keyed by role here because `framework` is not shared between the roles of an emitter: a `java` server takes `spring` and a `java` client takes no framework at all.

## 5. Members

Every member has a **name**.

- Below the root, the name is the directory name, unless the member table sets `name`.
- The member in a root file MUST set `name` (`E0710`). The name of the root directory is not part of the repository, so a default would make the output depend on where the repository was cloned.
- A name is not empty and uses only ASCII letters, digits, `_` and `-`, as service and library names do today. A directory name that does not fit needs `name` (`E0710`, and the message says so).
- Names are unique in the workspace (`E0710`, naming both files).

Every member table accepts `name` and `owners` (section 11).

### 5.1 `[service]`

```toml
[service]
name = "bookings"            # default: the directory name
contracts = "contracts"      # default
consumes = ["payments"]      # default: none; cycles are legal; "*" is every other service
prefix = "/v1"               # optional
base-url = "http://bookings:3001"   # optional; see below
owners = ["team-bookings"]
package = "@shop/bookings"   # publishing, with `version` and `description`, as today
version = "1.0.0"
```

- The name is the package name of the service: what `use bookings:...` and `consumes` refer to.
- `consumes` entries MUST name services of the workspace (`E0702`). Listing the service itself is a warning (`W0701`).
- `base-url` keeps today's meaning in this change. The runtime config change removes it.
- A service member may hold `[server]` and `[client]` targets (section 6).

### 5.2 `[app]`

```toml
[app]
name = "web"                 # default: the directory name
uses = ["bookings", "iam"]   # default: every service of the workspace
owners = ["team-web"]
```

An app has no contracts. It only consumes: it may hold `[client]` targets, and a `[server]` in an app is `E0711`. `uses` entries MUST name services of the workspace (`E0702`).

### 5.3 `[library]`

```toml
[library]
name = "money"               # default: the directory name
contracts = "contracts"      # default
owners = ["team-platform"]
package = "@shop/money"      # publishing, as today
version = "1.0.0"
```

A library holds shared contracts that other members import (`use money:types.{Money}`). It has no targets: a `[server]` or a `[client]` in a library is `E0711`.

## 6. Targets

A target is a `[server]` or a `[client]` table in a member file. For more than one target of a role, write an array: `[[server]]` or `[[client]]`.

```toml
[server]
name = "bookings-api"        # default: section 6.2
emitter = "node"             # required
framework = "express"        # default: the workspace default, else the emitter's
out = "src/generated"        # default: section 6.1
clients = true               # default; false generates no clients for `consumes`
app = "."                    # optional, for `veld setup`, as today

[server.options]
testing = true
```

| Target | In a `[service]` member | In an `[app]` member |
|---|---|---|
| `[server]` | The server of this service. With `clients = true` it also holds a client for each service in `consumes`, under `<out>/clients/<service>/`, as today. | `E0711` |
| `[client]` | A client of this service. | A client of each service in `uses`. |

The keys `role`, `service` and `services` of today's `[[targets]]` do not exist: the table name gives the role and the member gives the services.

### 6.1 Where the output goes

`out` is relative to the member's directory. Its default depends on the emitter:

| Emitter | Default `out` |
|---|---|
| `node`, `typescript` | `src/generated` |
| `java` | `src/main/java` |
| `rust` | `generated` |
| `plugin:<name>` | none: `out` is required (`E0700`) |

The rules for `out` do not change: two targets MUST NOT share or nest their `out` directories (`E0703`), and `out` MUST NOT be the workspace root or a contracts directory (`E0704`). A member with a `node` server and a `typescript` client therefore has to write `out` for at least one of them; the `E0703` message names both targets and the shared directory.

### 6.2 Target names

A target name selects the target in `veld generate <target>` and names the importable package (`@veld/<target-name>`, `config.md`). The default is:

- the member's name, when the member has one target;
- `<member>-server` or `<member>-client`, when it has one of each;
- none when the member has two targets of the same role: each of those MUST set `name` (`E0710`).

Target names are unique in the workspace (`E0710`, naming both files).

## 7. What the loader produces

The loader resolves a workspace to the same structure the rest of Veld reads today: the project name, the packages (one per service and library, with its contracts directory, `base-url`, `consumes` and prefix), the targets (with role, emitter, framework, service or services, `out`, `clients`, `app`, options), the plugins, the registry, the dependencies, the lint levels and the hooks. It adds, for each member, its name, kind, directory, `owners`, and the names of its targets.

- Packages are ordered as today: libraries by name, then services by name.
- Targets are ordered by member (section 4.1), then servers before clients, then by their order in the file.

### 7.1 One resolution function

Resolution is one pure function in `veld-project`, with no access to the file system:

```
resolve(root_dir, root_text, members: [(relative_dir, text)]) -> Project | ConfigError
```

- The CLI finds the root (section 3.1), expands `members` against the disk, reads each member file and calls it.
- The playground (`veld-wasm`) expands `members` against the paths of its virtual files and calls it.
- Expanding `members` is a second pure function over a list of directories that hold a `veld.toml`, so both callers match the same directories.

No other code decides which targets exist or what their names, frameworks and `out` directories are. A test generates every example and every playground preset through both callers and compares the bytes (`veld-wasm/tests/parity.rs` today).

## 8. Paths outside the member

`contracts`, `out` and `app` may point outside the member's directory (`out = "../../web/src/generated"`), as `out` may point outside the project today. The workspace root's `veld.lock`, `veld.lock.ir.json` and `.veld/` stay in the root directory.

## 9. Selecting members

A run selects members. Compiling always reads every member, because contracts import across members; the selection decides what is written or run.

| Where and how | Selected |
|---|---|
| in the root directory, or anywhere that is not inside a member | every member |
| inside a member's directory (the current member, section 3.1) | that member |
| `-p <name>`, `--package <name>` (repeatable) | the named members |
| `--workspace` | every member |
| `--affected --since <ref>` | section 10 |

- `-p` names a member, whatever its kind. An unknown name is `E0803` and the message lists the members.
- `-p`, `--workspace` and `--affected` exclude each other (a usage error).
- A member directory nested inside another member's directory belongs to the inner one.

The commands that take a selection:

| Command | What the selection narrows |
|---|---|
| `veld generate`, `veld watch` | the targets written: those of the selected members |
| `veld clean` | the targets cleaned |
| `veld setup` | the targets wired into their apps |
| `veld test` | the examples run: those written in the contracts of the selected members |

`veld generate <target>` keeps working. With a selection, the target MUST belong to a selected member (`E0803`).

`veld check`, `veld diff`, `veld fmt` and the exports cover the whole workspace and take no selection.

`veld.lock.ir.json` is written only by a run that selects every member, is not a dry run and has no conflicts, as it is written today only when every target is generated. The breaking-change gate of `veld generate` (`config.md`, "Generated output") applies to every run.

## 10. `--affected --since <ref>`

`veld generate --affected --since origin/master` selects the members that a change can have affected, so that the work of a run follows the size of the change and not the size of the repository.

### 10.1 Changed paths

The changed paths are the files that differ between the merge base of `<ref>` and `HEAD`, and the working tree: committed, staged and unstaged changes, and untracked files that are not ignored. Veld reads them from `git`.

`--affected` without `--since` and `--since` without `--affected` are usage errors. When the root is not inside a git work tree, `git` cannot be run, or `<ref>` is unknown, the run fails with `E0712` and writes nothing.

### 10.2 Changed members

A member is **changed** when a changed path is:

- its `veld.toml`,
- inside its contracts directory, or
- inside the `out` directory of one of its targets.

Every member is changed when the root `veld.toml` or `veld.lock` is a changed path.

### 10.3 Affected members

A member is **affected** when it is changed or depends on an affected member. A member depends on:

- each service in its `consumes` (a service) or its `uses` (an app), and
- each member whose package its contracts import.

The relation is followed until nothing is added. Cycles are legal and end the walk.

### 10.4 Output

Before generating, the run prints each affected member with one reason, the first that applies in this order: the root file or the lock changed, its own file changed, its contracts changed, its output changed, or the name of the affected member it depends on.

```
affected since origin/master:
  payments   contracts changed
  bookings   consumes payments
  web        uses bookings
```

With nothing affected the run prints `nothing is affected since <ref>` and exits 0. `--affected` combines with `--check` and `--dry-run`.

## 11. `owners`

`owners` is a list of free-form names (team names, handles, addresses). An entry MUST NOT be empty or hold a line break (`E0700`). Veld does not check that an owner exists.

Owners are not part of the IR and never reach generated code. They are used by:

- `veld diff`: under each package with a breaking or possibly breaking change, a line `owners: ...` for the member that declares it, and one line `affects <member> (<owners>)` for each member that depends on it (section 10.3). A member without owners prints no parentheses, and a workspace in which no member has owners prints the report it prints today.
- `veld check --graph=json`: the node of a member that has owners carries them as `owners`. A node without owners is the same bytes as today.

`veld impact` and the pull request summary of the proposal's section 14 read the same data and are not part of this spec.

## 12. The earlier layout

A `veld.toml` with `[project]`, `[services.<name>]`, `[libraries.<name>]` or `[[targets]]` is `E0709`. The message names the tables found and gives the new table for each:

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

There is no converter. Moving a project means deciding where each member's directory is, which a tool cannot decide; the table above and the guide are the migration. `veld init`, every example, every test fixture, the playground presets and the docs move in the same change.

## 13. What must not change

For every example, test fixture and playground preset that moves to this layout, with target names kept by writing `name` where the new default differs:

- every generated file and every `.veld-manifest.json` is the same bytes,
- `veld.lock` and `veld.lock.ir.json` are the same bytes (paths in the IR are relative to a contracts directory, so moving that directory does not show),
- no snapshot of generated output or of the IR changes.

Only `veld.toml` files, the directories that move with them, and text that quotes the layout (docs, error messages, `veld init`) change. A change that cannot keep this is listed for the owner, not accepted.

## 14. Diagnostics

| Code | Meaning |
|---|---|
| `E0709` | The file uses the layout of earlier betas (section 12) |
| `E0710` | Invalid workspace structure: no member table, more than one, a nested workspace, a member no workspace lists, a bad `members` pattern, a missing member file, a missing, invalid or duplicate member name, a missing or duplicate target name |
| `E0711` | A table that is not allowed in this file: a root-only table in a member file, a `[server]` in an app, a target in a library |
| `E0712` | `--affected` cannot read the changes: no git work tree, `git` not found, unknown `<ref>` |

`E0700` to `E0704` keep their meaning, with "project root" read as "workspace root".

## 15. Readers of the layout

The implementing change has to move each of these in the same series. The list is here because several have no test that fails when they are missed.

- `crates/veld-project/src/config/` (the loader) and `crates/veld-project/tests/config.rs`
- `apps/veld`: `pipeline.rs`, `cli.rs`, `commands/init/` (templates and prompts), `commands/doctor.rs`, `commands/watch.rs`, `commands/registry/` (`add` and `install` edit and read `[dependencies]`), `commands/plugin/templates.rs`, and every test under `apps/veld/tests/` that writes a `veld.toml`
- `crates/veld-registry-client`: `depsedit.rs`, `publish.rs`, `resolver.rs` (which table holds `package`, `version` and `[dependencies]`)
- `crates/veld-tools`: `deps.rs`, `lint/mod.rs`
- `crates/veld-lsp`: `workspace.rs` (finding the root from an open file) and the completion, hover and navigation that read the config
- `crates/veld-wasm`: `api.rs` and `tests/parity.rs`, which finds examples by a `veld.toml` in their root
- `xtask`: `schema.rs` (the JSON schema of `veld.toml` for editors), `codegen.rs`, `templates.rs`
- `.github/workflows/ci.yml`: the determinism job writes a `veld.toml` inline. No local test covers it
- `examples/*`, `tests/toolchain/**`, `tests/conformance/**/case.toml`, `website/src/playground/presets/*`, the website's code samples, `docs/`, `README.md`, the editor schemas

## 16. Open points

Decided in this draft, for the owner to confirm:

1. **Defaults are keyed by role and emitter** (section 4.2), one level deeper than the proposal's sketch.
2. **The member in a root file must set `name`** (section 5). The smallest project is five lines, not four.
3. **`-p` is `--package`** (section 9), after Cargo, although it also names an app, which has no contracts.
4. **No converter from the earlier layout** (section 12).
5. **`veld.lock.ir.json` stays one file for the workspace, written only by a full run** (section 9). A team that only ever runs `veld generate -p <member>` never refreshes it. One lock per member would fix that, and would move a file that exists today, so it is not in this change.
6. **A change inside a target's `out` directory marks the member as changed** (section 10.2), so that `--affected --check` catches a hand-edited generated file.
