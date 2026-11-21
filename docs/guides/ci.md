# Veld in CI

Four checks keep a Veld project honest. Each one is a command that exits non-zero on failure and
never prompts, so it can run unattended:

| Check | Command | Fails when |
|---|---|---|
| Contracts compile and lint clean | `veld check` | any contract error, or a lint rule set to `error` |
| Formatting | `veld fmt --check` | a `.veld` file is not in canonical format |
| Generated code is current | `veld generate --check` | any target would create, update or delete a file |
| No unreviewed breaking change | `veld diff --strict` | a contract change is breaking for consumers |

Run them in that order, cheapest first. `veld check` includes the lint rules, there is no separate
lint command. `veld generate --check` and `veld diff --strict` need the output and snapshot you
committed, which is why the examples commit them.

> **Command names.** `veld check`, `veld fmt --check`, `veld diff --strict` and `veld generate
> --dry-run` exist in `2.0.0-beta.1`. `veld generate --check`, `generate --watch` and
> `veld doctor` are the names this guide uses for the staleness check, watching and diagnostics, as
> planned in [`docs/v1-parity.md`](../v1-parity.md). In a build that predates them, `veld generate
> --dry-run` lists what would change (read its output), and `veld --help` tells you what you have.

## 1. `veld check`: contracts and lint

```bash
veld check
```

```
✓ 1 service, 1 endpoint, 1 type in 1ms
```

Errors print rustc-style with a code, a source excerpt and a caret, and the exit code is 1. Warnings
(unused imports, shadowing a prelude name, `W0001` dangling doc comments) do not fail the run.
Lint findings carry `L####` codes and are warnings by default. To make a rule gate your build, set
its level under `[lint]` in `veld.toml`:

```toml
[lint]
unused-model = "error"          # off | warn | error
missing-description = "off"
```

The rules are `unused-model`, `empty-service`, `empty-model`, `empty-constants`,
`missing-description`, `deprecated-endpoint`, `deprecated-field`, `deprecated-declaration`,
`file-naming-convention`, `type-name-case`, `field-name-case`, `endpoint-name-case` and
`variant-name-case`. The case rules implement the naming conventions of `spec/language.md` section 8.
Only your own services and libraries are linted, never the prelude or registry packages.

## 2. `veld fmt --check`: formatting

```bash
veld fmt --check
```

```
would reformat contracts/x.veld
error: 1 file would change; run `veld fmt`
```

Nothing is changed and the exit code is 1 if any file would be. The formatter has no options, so a
developer's editor, `veld fmt` and CI always agree. Comments are preserved. Fix locally with
`veld fmt`.

## 3. `veld generate --check`: stale generated code

If you commit the generated code, CI must prove it matches the contracts. `veld generate --check`
runs the whole generation into the staging area, compares it with what is on disk and the
manifest, writes nothing, and exits 1 when any target would create, update or delete a file. It
reports user-modified generated files too, because `generate` would refuse them.

```bash
veld generate --check
```

The fix is always the same: run `veld generate` locally and commit the result. The output is
deterministic (no timestamps, host names or absolute paths), so a developer on Windows and CI on
Linux produce the same bytes.

If you do not commit generated code, CI generates it instead of checking it: `veld generate` before
the build, and `veld setup` if the build needs the packages wired (`veld setup` is idempotent).

## 4. `veld diff --strict`: breaking changes

`veld diff` compares the current contracts with a baseline and sorts every difference into breaking
and non-breaking. `--strict` exits 1 if any is breaking.

```bash
veld diff --strict                       # against veld.lock.ir.json (the last `generate`)
veld diff --against origin/main --strict # against the contracts at a git ref
```

`veld.lock.ir.json` is the compiled snapshot of the contracts as last generated (see
`spec/config.md`); commit it. `--against REF` reads the contracts at that ref with `git show` and
checks nothing out, so the job needs the history: use `fetch-depth: 0` (or fetch the base branch).
Against the base branch is the better CI check, because it asks "does this pull request break
what is on main?" and does not depend on whether the author remembered to regenerate.

Breaking changes are the ones that can break a client or server built against the old contract:
removing a declaration, field, enum variant, union arm or endpoint, changing a type, a path, a method
or a parameter binding, adding a required field or parameter, making an optional field required, or
tightening a constraint. Severity depends on where the type is used (a new required field breaks a
request type, a removed field breaks a response type), so the same edit can be breaking in one place
and not in another. Additions, loosened constraints, docs and deprecations are reported as
non-breaking. A deliberate break is accepted by merging it (the next `generate` rewrites the
snapshot), usually together with a new service version or a coordinated release of consumers.

## A GitHub Actions workflow

```yaml
name: contracts
on: pull_request

jobs:
  veld:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0                       # veld diff --against needs history
      - name: Install veld
        run: npm install -g @maayn/veld
      - run: veld check
      - run: veld fmt --check
      - run: veld generate --check
      - run: veld diff --against origin/${{ github.base_ref }} --strict
```

Pin the version in `npm install -g @maayn/veld@2.0.0-beta.1` so a new release does not change
your gate unannounced. The wrapper verifies the SHA-256 of the binary it downloads. The same
works with `pip install maayn-veld`, or by downloading the release archive and `SHA256SUMS`
from the GitHub release and checking it yourself.

GitLab CI:

```yaml
veld:
  image: node:22
  script:
    - npm install -g @maayn/veld
    - veld check
    - veld fmt --check
    - veld generate --check
    - git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME
    - veld diff --against origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME --strict
```

## Building the generated code

`veld check` proves the contracts; your own build proves the code that uses them. In CI, after
`veld generate` (or the checkout, when you commit generated code), run `veld setup` and then your
normal build and tests: `npm ci && npm test`, `mvn verify`, `./gradlew build`. Generated TypeScript
compiles under `tsc --strict`, and generated Java under `javac --release 17 -Xlint:all -Werror`.

## Watching locally, doctor on a new machine

`veld generate --watch` regenerates on every change while you work. `veld doctor` checks that
`veld.toml` loads, every emitter and plugin resolves with the right hash, output folders are
writable and allowed, and that the tools your targets need (`node`, `javac`) are present. Run it
first when a CI job behaves differently from your machine.

## Registry and plugins in CI

- `veld install` installs exactly the packages pinned in `veld.lock` and verifies every checksum; a
  mismatch is a hard error. `veld check` and `veld generate` do this for you when the cache lacks a
  locked package. Without a lock they never touch the network.
- Registry tokens belong in a secret: `veld login --token` reads it from stdin and stores it per registry.
- A plugin target runs the plugin executable, so pin it with `sha256` in `[plugins.<name>]`; a
  mismatch is refused (`E0705`).

## Releasing Veld itself

The repository's own pipeline is `.github/workflows/ci.yml` (format, clippy, the full test suite with
the toolchain tests required) and `release.yml` (tag a `v*` version to build the archives, the
`SHA256SUMS`, the deb, rpm, Chocolatey and Scoop packages). See
[`distribution/`](../../distribution) and [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
