# ADR-008: Contract packages get a manifest and a lockfile; the registry stays data-only at graduation

- Status: PROPOSED (accept by 2026-11-01; implement in M4)
- Date: 2026-10-21
- Deciders: T3 (owner), T1; reviewer: T2
- Evidence: `veld pull` resolves "latest" as `versions[0]` from the server (`cmd/veld/registry.go:200-209`), verifies the SHA-256 only when the server sends an `X-Veld-SHA256` header (`registry.go:232-236`), and records nothing locally; `Pack` includes every `.veld` file under the directory with no manifest (`internal/registry/tarball.go:17-41`). No lockfile for pulled packages exists; `.veld.lock.json` is the breaking-change lock for the project's own contract, a different thing.

## Decision

1. Package manifest `veld.package.json` at the package root, written by the author, validated by `veld push`:
   `{ "manifestVersion": 1, "name": "@org/name", "version": "1.2.0", "description": "...", "entry": "app.veld", "dependencies": { "@org/common": "1.x" }, "license": "MIT" }`.
   Only `.veld` files, the manifest, `README.md` and `LICENSE` are packed. Dotfiles, cache and lock files are excluded.
2. Project lockfile `veld.lock.json` next to `veld.config.json`, written by `veld pull`/`veld install`: for each package, `name`, `version`, `resolved` (registry URL), `integrity` (`sha256-<hex>`), and the resolved dependency tree. `veld install --frozen` fails if the lockfile and config disagree; CI uses it.
3. Version constraints: exact, caret (`^1.2.0`) and `1.x` ranges, semver only. Resolution: highest version satisfying all constraints, single version per package per project (no nested duplicates). Conflicts are errors with both constraint sources named.
4. Integrity: the client always requires a checksum (from the lockfile when present, otherwise from the registry version record), verifies it before extraction, and extracts into a temporary directory that is renamed into place.
5. Cache: `~/.veld/cache/<registry-host>/<org>/<name>/<version>.tgz` plus a content-addressed `sha256/` directory, enabling offline `veld install --offline` from a complete cache.
6. Registry selection and dependency confusion: each `@org` scope is bound to exactly one registry in `veld.config.json` (`"registries": { "@acme": "https://registry.acme.internal" }`); unscoped lookups never fall through to another registry. Credentials are stored per registry URL (as today) and sent only to that URL.
7. Package types: the registry hosts data-only contract packages at graduation. Executable emitter plugins are distributed through npm, Go, or GitHub Releases (ADR-003); a future `kind: "plugin"` package type would require a separate trust policy and is out of scope.

## Reasons

- Reproducibility and integrity are the minimum a registry must provide; today both depend on the server's goodwill.
- The design reuses semver and lockfile conventions developers already know (npm, Go modules) and avoids inventing resolution semantics beyond "highest satisfying, single version".
- Data-only packages keep the registry's threat model manageable for a student team (chapter 13).

## Consequences

- `internal/registry` gains manifest, lockfile, resolver and cache code (chapter 12 estimates).
- Server API adds `GET /packages/{org}/{name}` metadata with per-version `sha256` and dependency lists (chapter 13).
- Transitive dependencies are resolved client-side from version metadata; the server does not run resolution.
