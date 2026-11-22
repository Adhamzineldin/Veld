# Changelog

All notable changes to Veld are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

Restores the v1 features that 2.0.0-beta.1 dropped and the owner kept (see [`docs/v1-parity.md`](docs/v1-parity.md)).

### Added

- **Website playground** (`/playground`): the real engine compiled to WebAssembly (`crates/veld-wasm`: compile, generate, format, OpenAPI export). Editor with live diagnostics, example presets, target picker, file tree, copy and zip download, offline after first load. `cargo test -p veld-wasm` proves every example and preset is byte-identical to the CLI, and `npm run test:wasm` replays that through the built `.wasm`.
- **Lint** inside `veld check`: every v1 rule (unused model, empty service, empty model, empty constant, missing description, deprecated endpoint and field, file naming) plus the case rules of `spec/language.md` section 8. Codes `L0001` to `L0013`, levels `off`, `warn` or `error` under `[lint]` in `veld.toml`.
- **"Did you mean" suggestions** on unknown types, modules, packages, annotations, imports and enum variants, also across modules and packages (`Usr` suggests `User` of another module together with the `use` line).
- **`prefix`** in `[project]` and `[services.<name>]`, prepended to every service prefix in the IR.
- **`consumes = ["*"]`** (every other service).
- **Dependency graph** (`veld check --graph text|dot|json`) with cycles reported.
- **Breaking-change gate** on `generate` (`--strict`, `--accept-breaking`) and a **stale-output check** (`generate --check`, with `--diff`).
- **`veld doctor`** checks the project, its contracts, targets, plugins, registry and the tools the targets need (`mvn` or `gradle` only for Java server targets that build with them).
- **Registry commands** `veld registry info`, `search`, `versions` and `token create`.
- **Post-generate hooks** (`[hooks] post-generate`).
- **Reconnect with exponential backoff** for WebSocket and SSE (`reconnect: { initialMs, maxMs, retries }`, `Last-Event-ID` for SSE) in the TypeScript and Java clients: off by default in the plain clients, on by default in the React and Vue stream hooks.
- **`validate = false`** target option for `node`, `typescript` and `java`: skips run-time request validation, keeps the types.
- **Query options without a dependency**: the target option `query-options = true` of the `typescript` client writes `<package>/queries.ts` with query keys, query options and mutation options as plain objects for TanStack Query, SWR or your own hooks. The generated code imports no query library.
- **Workspace `veld init`** asks per service for a base URL and which services it calls (`consumes`), with the flags `--base-url SERVICE=URL` and `--consumes SERVICE=a,b`. Nothing is consumed unless asked for.
- **`createClients(opts)`** in the TypeScript client barrel, and a deterministic **`README.md`** in the output of every target.
- apt/rpm (nfpm), Chocolatey and Scoop packages are built from the release archives.

### Fixed

- **`same()` no longer compares by data alone.** The TypeScript `same(a, b)` fell back to canonical bytes for ordinary values, so two different models with identical fields compared equal, against `spec/ir.md` 5.4. Values now carry their declaration signature in non-enumerable symbols (client decode, server binding, `Model.of`); `same` needs it on both sides and throws a `TypeError` naming the fix when one is missing. `Model.same(a, b)` still accepts plain objects. The Java `Veld.same` compares lists and maps element by element and never equals a value without a signature.
- The language server shows the full route (project and service prefix) in hover.
- The "Use it:" block printed after `generate` aligns its columns for any target name length.

### Decisions

- The lint rule `file-naming-convention` uses `user_model.veld` where v1 used `user.model.veld`, because a dot is not valid in a module path.

## 2.0.0-beta.1

Veld 2.0 is a rewrite in Rust with a new contract language. It is a beta: the language is specified
in [`spec/language.md`](spec/language.md) and may still change before 2.0.0.

### Breaking changes

- **New language.** v1 `module`, `action`, `import`, `description:` and `method:` blocks are replaced by
  `model`, `enum`, `error`, `union`, `service ... at "/prefix"`, endpoints such as
  `get "/{id}" getOrder(id: uuid) -> Order | NotFound`, and `use pkg:module.{Name}`. v1 syntax is
  error `E0001`.
- **New config.** `veld.config.json` is replaced by `veld.toml` ([`spec/config.md`](spec/config.md)).
  `[[targets]]` have a `role` (`server` or `client`), an `emitter` and an `out`.
- **Rust binary.** `veld` is a single self-contained binary. The Go toolchain is no longer needed.
- **Generators.** Built in: `node` (server), `typescript` (client), `java` (server and client). The v1
  backends for Python, Go, C#, PHP and Rust, and the Dart, Kotlin, Swift, Svelte, Vue and Angular
  clients are not part of 2.0. They can be written as emitter plugins.
- **Commands.** `validate` is now `check`. `ast` is gone. `veld export openapi` replaces the v1
  `openapi` command. The lock file is
  `veld.lock.ir.json`, and `.veld.lock.json` and `.veld-cache.json` are no longer used.
- **Errors are declarations.** An error is `error Name STATUS { fields }` and is listed in the response
  position of an endpoint. `NotFound`, `Unauthorized`, `ValidationError` and the other prelude errors
  need no declaration.
- **Wire format.** `long` is a decimal string on the wire, and every error response is
  `application/problem+json`.
- **Archived.** The v1 Go code, its docs and its examples moved to [`archive/v1/`](archive/v1). The
  Go, NuGet and Composer distribution channels are not maintained for 2.0; apt/rpm, Chocolatey and Scoop packages are built from the release archives.

### Highlights

- **Signature-based equality.** Every declaration has a compiler-computed signature that generators
  embed verbatim. `same()` compares signatures, operation and canonical bytes, so the same `NotFound`
  is the same in every generated copy and every language ([`spec/ir.md`](spec/ir.md) section 5).
- **Microservices.** Workspaces with `[services.*]`, `[libraries.*]` and `consumes` (cycles are legal),
  a generated client per consumed service, and servers of different languages in one workspace.
- **Zero runtime dependencies** in generated code. Framework adapters (express, fastify, hono,
  node:http, Spring Boot) use structural types or the chosen framework only.
- **Deterministic and non-destructive output.** Staged writes with a manifest. `veld generate
  --dry-run` shows what would change. Unsupported features fail loudly with `E0801`.
- **Tooling.** `fmt`, `diff` (breaking-change detection), `export openapi`, `export ir`, a language
  server (`veld lsp`) with a VS Code extension, emitter plugins (`veld plugin new`, `plugin test`) and
  a package registry (`add`, `install`, `publish`).
- **Examples** under [`examples/`](examples) with committed, test-checked output.
- **Releases** for Linux (x64, arm64), macOS (x64, arm64) and Windows (x64), with `SHA256SUMS`.


## v1 (archived, frozen at tag `v1-final`)

### Changed

- `veld generate`, `veld watch` and `veld ci` stage generated files and commit them only when the whole run succeeds. A contract that fails to parse or validate leaves the previous output, the cache and `.veld.lock.json` untouched. Veld never deletes or overwrites files it did not write. Each output directory records what Veld wrote in `.veld-manifest.json`. See [docs/guides/generated-output.md](docs/guides/generated-output.md). (B-006, ADR-007)
- `veld clean` deletes only the files listed in the manifest and keeps `.veld.lock.json` unless `--lock` is given. A non-empty output directory without a manifest is left alone and the command fails. (B-009)
- `veld generate --dry-run` lists the files it would create, update or delete per resolved output directory, and writes nothing, including `.veld-cache.json`. (B-009)
- Output directories that are or contain the config directory, a contract directory, the working directory, the home directory or a filesystem root, or that already contain `.veld` files, are refused for every command and workspace entry. The old rule in config, which only looked at the last path segment, is gone. (B-008)
- The generated `README.md` is written only into the entry's own output directories and says that files you add are never touched.
- Hand-edited generated files that are no longer produced are kept with a warning instead of being deleted.

### Added

- `veld generate --overwrite-unmanaged` replaces files in the output directory that the manifest does not list.
- `veld clean --lock` also removes the breaking-change baseline.

### Fixed

- Breaking-change detection (`--strict`, `--force`, the interactive prompt) works again: the lock is no longer deleted before the comparison. (F-NODE-04)
- `veld init` no longer overwrites an existing `README.md` or contract files; it exits 1 and names them. (F-CLI-05)
- `types/index.ts` lists exports in a stable order, so the same contract always produces byte-identical output. (B-012, F-LEAD-01)
- `veld watch` and `veld ci` use the configured framework, validation, services and server SDK settings, like `veld generate`.
- Node backend: routes with a literal segment are registered before `:param` routes, so `GET /items/me` declared after `GET /items/:id` reaches its own handler and its middleware runs. This also applies to the plain JavaScript backend. (B-015, F-NODE-05)
- Node backend: a 5xx response always says `Internal Server Error` and the error is logged on the server. A 4xx keeps its message, and `code` is sent only for errors that declare a status. One shared `routes/_respond.ts` replaces the copy in every handler. (B-015, F-NODE-10)
- JavaScript and TypeScript clients (TypeScript, React, Vue, Svelte, Angular, plain JavaScript, Node service SDKs) URL-encode path parameters. (B-015, F-NODE-11)
- Node backend, TypeScript client and React hooks compile under `tsc --strict`. Query, headers and GET input reach the service as model instances (`Model.fromJSON`), and React hooks pass the input of GET actions. (B-015, F-NODE-06)
