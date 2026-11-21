# v1 to v2 parity audit

Scope: every feature of the v1 Go implementation (`archive/v1/`, tag `v1-final`) compared with v2 (branch `feat/veld-2-rust`, binary `target/debug/veld`). The owner rule is that v2 has no regressions: only improvements and bug fixes.

Method: v1 sources read from `archive/v1/go/cmd/veld`, `archive/v1/go/internal/**`, `archive/v1/docs`, `archive/v1/CLAUDE.md` and the old editor plugins (`git show v1-final:editors/...`). v2 status was verified by running `veld --help` and each subcommand's `--help`, by running `init`, `check`, `generate`, `setup`, `clean`, `diff` and `export openapi` in scratch projects, and by reading the code in `crates/`, `apps/veld`, `spec/` and `editors/`.

Status words: `same`, `improved`, `missing`, `downgraded`, `superseded` (only for the three owner decisions: the removed emitters, the syntax redesign, JSON to `veld.toml`). Severity: P0 a user hits it immediately, P1 a CI or daily workflow breaks, P2 a documented v1 feature is gone, P3 convenience.

CLI look and feel is being restyled by another agent. UX rows are still listed so the restyle keeps every v1 behaviour. Line numbers for v1 are in the v1 tree under `archive/v1/go/` (or `archive/v1/` for docs and distribution).

## Summary: every `missing` or `downgraded` item, ranked

Fixes are written for v2's architecture: new subcommands in `apps/veld/src/commands`, reusable logic in `veld-tools`, generator-style output through `veld export` or a `[[targets]]` entry, config in `veld.toml`, and new codes in `crates/veld-compiler/src/codes.rs`. Every fix keeps rules 1 to 8 of `CLAUDE.md` (staged writes, determinism, no panics, tests with the change).

### P1

| # | Item | v2 status | Proposed v2-native fix |
|---|---|---|---|
| 1 | `hooks.postGenerate` (run a shell command after generate, for example prettier or `npm install`) | missing | Add `[hooks] post-generate = "cmd"` (string or list) to `veld.toml`. `veld generate` runs it after a successful, non-dry-run write, with cwd at the project root. A failure is a warning with exit 0 unless the hook sets `fail = true`. `E0701` already covers typos. `veld migrate` maps `hooks.postGenerate` to it instead of warning. |
| 2 | Breaking-change gate in `generate` (v1 compared against the lock before writing, then prompted, hard-failed with `--strict`, or continued with `--force`) | downgraded | `veld generate` diffs the IR against `veld.lock.ir.json` before writing (`veld_tools::diff` already exists), prints the breaking list, fails with `--strict` or on a non-TTY, prompts on a TTY, and `--accept-breaking` continues. `--force` stays reserved for file overwrite (rule 1). Today `generate` silently rewrites the lock (`apps/veld/src/commands/generate.rs:47`). |
| 3 | Stale-output check for CI (`veld diff --exit-code` and `--stat`: generate to a temp dir and compare with disk) | missing | `veld generate --check` (no writes, exit 1 when any target would create, update or delete a file; the staged writer's plan already has this data). `--diff` prints unified diffs. The v2 `veld diff` stays the contract-change command. Verified: `generate --dry-run` exits 0 even when files would change. |

### P2

| # | Item | v2 status | Proposed v2-native fix |
|---|---|---|---|
| 4 | `veld lint` (rules: unused-model, empty-module, empty-model, empty-constants, missing-description, deprecated-action, deprecated-field, file-naming-convention; duplicate-route and duplicate-action are now compile errors) | missing | New `veld lint [--strict]` backed by `veld-tools::lint` over the IR. Rules carry `L####` codes in `codes.rs`, configurable under `[lint]` in `veld.toml`. It also covers the naming rules that `spec/language.md` section 8 promises ("Lint (not error)") but nothing implements. `generate` prints the one-line lint hint again. |
| 5 | `veld graphql` (GraphQL SDL export) | missing | `veld export graphql [-o FILE] [--service S]`, implemented in `veld-tools::graphql` from the IR (models, enums, unions, `Page`, Query for `get`, Mutation for the rest, Subscription for `stream` and `ws`). |
| 6 | `veld schema` (Prisma and SQL DDL) and the `database` tool (TypeORM entities for the node target) | missing | `veld export schema --format prisma\|sql [-o FILE]` in `veld-tools::schema`, using `@unique`, `@index`, `@primary` (already in the IR) plus the restored `@autoincrement` and `@relation` (rows 7, 8). TypeORM can be a `plugin:` sample. |
| 7 | `@relation(Model)` annotation | missing | Re-add to `language.rs` (field target; the argument must resolve to a model, a new `E06xx` otherwise). The IR passes it through for tools such as `export schema`. `veld migrate` keeps it instead of dropping it (`crates/veld-tools/src/migrate/models.rs:185`). |
| 8 | `@autoincrement` annotation | missing | Re-add as IR-only metadata like `@primary`, valid on `int` and `long` fields. Update `veld migrate`. |
| 9 | `veld docs` (HTML and Markdown API docs, workspace-wide, model usage cross-references) | missing | `veld export docs --format html\|markdown [-o FILE]`, one deterministic file covering all services of the workspace, built from the IR docs, `@example` and `@deprecated`. |
| 10 | `veld doctor` (config found and valid, contracts compile, emitters available, output dirs writable, lint clean) | missing | `veld doctor`: checks that `veld.toml` loads, packages resolve, every `emitter` and `plugin` resolves with the right hash (`E0705`), output dirs are writable and allowed (`E0703`, `E0704`), `veld.lock` is consistent, git is present for `diff --against`, and `node` or `javac` exist for the wired targets. Exit 1 on failure. |
| 11 | "Did you mean" suggestions for unknown types | downgraded | Add `help: did you mean \`string\`?` to `E0205` and the other unknown-name codes, using the edit-distance helper in `crates/veld-project/src/config/suggest.rs`. Verified: `veld check` on `x: Strng` prints no suggestion. |
| 12 | React hooks: v1 used TanStack Query (query keys, cache, dedupe, invalidation of the module's queries after a mutation, retries) | downgraded | Keep the zero-dependency hooks as the default and add `framework = "react-query"`, whose only import is `@tanstack/react-query` (a framework adapter, allowed by rule 3). Alternative: add query keys and an `invalidate(service)` in `VeldProvider`, so a mutation refetches the active queries of its service. |
| 13 | TypeScript WebSocket client auto-reconnect with exponential backoff (1 s doubling to 30 s) | downgraded | Add `reconnect?: { initialMs, maxMs, retries }` to the `WsChannel` options in `runtime/veld.ts` (off by default so the documented semantics stay, on in the React and Vue hooks). Mirror it in the Java `WsChannel`. SSE gets the same option using `Last-Event-ID`. |
| 14 | VS Code commands: Validate Contract, Generate Code, Generate (Dry Run) | missing | Add the three commands (plus Setup and Export OpenAPI) to `editors/vscode/package.json` and `src/extension.ts`, running `veld check`, `veld generate`, `veld generate --dry-run` into an output channel. |
| 15 | VS Code snippets for contracts and config (v1: 22 contract snippets such as model, enum, module, action, crud, optional field, array, map, default, unique, index, example, relation; 5 config snippets) | missing | Ship `snippets/veld.json` (model, enum, error, union, service, endpoint variants, `crud`, field forms, annotations) and `snippets/veld-toml.json` (single service, workspace, server and client targets, plugin). |
| 16 | `veld.toml` schema, hover and key completion (v1: `veld-config.schema.json` wired through `jsonValidation` in VS Code and a schema provider in JetBrains) | missing | Generate `editors/veld.toml.schema.json` from `crates/veld-project` with `cargo xtask codegen`, register it for `veld.toml` (Even Better TOML schema association in VS Code, JSON-schema mapping in JetBrains), and add `veld.toml` keys to LSP completion. Delete the stale v1 `editors/veld-config.schema.json` (it still lists python, go and the JSON keys). |
| 17 | JetBrains actions (Tools > Veld: Validate, Generate, Generate Dry Run) and "New > Veld File" templates (App, Model, Enum, Constants, Module, Config) | missing | Add `AnAction`s that run the CLI in a run console, and file templates for `.veld` (model, enum, service, error) and `veld.toml` in `editors/jetbrains/src/main/resources/fileTemplates`. |
| 18 | `veld registry list`, `versions <@org/name>`, `info` (current registry and user), `token create --name --scopes` | missing | Add `veld registry info\|list\|versions` (read-only calls; `GET /api/v1/packages/{org}/{name}/versions` already exists) and `veld token create --name --scopes read,write,delete`. `veld login` already stores tokens. |
| 19 | Ad-hoc generate overrides (`--backend`, `--frontend`, `--input`, `--out`, `--backend-framework`, `--frontend-framework`, `--validate`) for one-off runs without a config | missing | `veld generate --emitter node --framework express --role server --out DIR [--service S]` builds one in-memory `TargetSpec`. It must still pass the output-safety rules (`E0703`, `E0704`) and the manifest writer. |
| 20 | User guides: v1 had `docs/guides/getting-started.md`, `generated-output.md`, `service-sdk.md`, `docs/ci-cd/guide.md` and `DEVELOPER_GUIDE.md` | downgraded | The website has docs pages, but `docs/` holds only `README.md` and `emitter-guide.md`. Add `docs/guides/{getting-started,generated-output,microservices,ci}.md`; the CI guide covers `generate --check`, `diff --strict` and `fmt --check`. |

### P3

| # | Item | v2 status | Proposed v2-native fix |
|---|---|---|---|
| 21 | `veld deps` and `deps --validate` (print the service consume graph) | missing | `veld deps [--format text\|dot\|json]`. Validation already runs inside `check` (`E0702`, `W0701`). |
| 22 | `veld completion bash\|zsh\|fish\|powershell` | missing | `veld completions <shell>` using `clap_complete` over `cli.rs`. |
| 23 | `veld watch` command name (v2 has `generate --watch`) | missing | Add `veld watch [TARGET]` as a visible alias for `generate --watch`. Behaviour is otherwise the same. |
| 24 | `veld ci` (generate then setup, never prompts) and the `generate --setup` flag | missing | `veld ci` = `generate` + `setup`, never prompts, exit 1 on failure; `generate --setup` runs the same setup step. |
| 25 | `veld export agents` / `AGENTS.md` generation (compact API contract for AI assistants) | missing | `veld export agents [-o AGENTS.md]` from the IR (services, endpoints, models, errors, `[project].description`). |
| 26 | Dockerfile, `.dockerignore` and compose generator (`tools.dockerfile`) | missing | A `plugin:` sample (`veld plugin new`), or `veld export dockerfile` for the node and java server targets. No absolute paths (rule 2). |
| 27 | GitHub Actions workflow generator (`tools.cicd`) | missing | `veld export ci --provider github` writing `.github/workflows/veld.yml` that runs `veld check`, `fmt --check`, `generate --check`, `diff --strict`. |
| 28 | `.env.example` generator (`tools.envconfig`) | missing | `veld export env`, listing `VELD_<SERVICE>_URL` for every consumed service plus `PORT`. |
| 29 | Test scaffold generator (`scaffold-tests`) | missing | A `plugin:` sample, or `veld export tests`. Low priority. |
| 30 | `tools` config block (toggles run on every `generate`) | missing | `[[targets]] role = "spec" emitter = "openapi"` (also `graphql`, `schema`, `docs`) so `generate` writes them into an `out` dir through the staged writer. `veld migrate` converts `tools.*` instead of warning. |
| 31 | `veld clean --lock` (remove the breaking-change baseline) | missing | `veld clean --lock` removes `veld.lock.ir.json`. Verified: `clean` keeps it and has no such flag. |
| 32 | `veld validate [file]` (check one file) | downgraded | `veld check [PATH]...` limits reporting to the given modules (the package set still compiles as a whole). |
| 33 | `--verbose` / `-v` global flag | missing | Add repeatable `-v`. `-vv` prints compile phases, write-plan entries and timings. |
| 34 | Import instructions printed after `generate` (per target, with example imports) | downgraded | Print one `import { ... } from '@veld/<target>'` or Java package line per target after generation, next to the `veld setup` hint. Keep the content when the restyle lands. |
| 35 | Generated `README.md` in each output dir | missing | Emit a deterministic `README.md` per target (how to import, how to regenerate, no paths) and list it in the manifest. |
| 36 | Richer `veld init` (v1: users CRUD, auth and common models, README.md, description prompt, project-type detection, "run setup now?" prompt) | downgraded | Keep `hello.veld` as the minimum; add `--template crud` (users, auth, pagination), write `README.md`, detect `pom.xml` or `package.json` to pre-select the target, and offer to run `setup` and `generate` at the end of an interactive init. |
| 37 | Top-level `prefix` (app-level route prefix applied to every module) | missing | `[services.<name>] prefix = "/api"` (or `[project] prefix`), prepended to every service prefix in the IR so all targets agree. `veld migrate` maps it instead of warning (`crates/veld-tools/src/migrate/mod.rs:105`). |
| 38 | Workspace `consumes = ["*"]` wildcard | missing | Accept `consumes = ["*"]` (every other service), expanded in `crates/veld-project/src/config/resolve.rs`. Verified: no wildcard handling exists. |
| 39 | Field-level literal union (`status: "A" \| "B"`) migrates as `json` | downgraded | `veld migrate` emits an `enum` named `<Model><Field>` (wire values equal the literals) instead of `json` (`crates/veld-tools/src/migrate/models.rs:113`). |
| 40 | TypeScript client aggregate (`VeldApiClient` with one member per module, and the default `api` instance) | missing | Emit `createClients(opts)` returning `{ [service]: Client }` from the client `index.ts`, taking the same `ClientOptions`. |
| 41 | Incremental generation (`--incremental`, `.veld-cache.json`) | missing | Probably not needed for speed (verified: a dry run of the six-target `microservices` example takes 0.2 s and the writer already reports `N unchanged`). Decide, then record it in `CHANGELOG.md`. If wanted, skip writes by content hash, which the manifest already holds. |
| 42 | Distribution wrappers: apt (`nfpm.yaml`), Chocolatey, Composer, NuGet, `go install` | missing | npm, pip and Homebrew are done. Add `nfpm.yaml` for apt and rpm and a Chocolatey or Scoop manifest to `distribution/` and the release workflow. Composer, NuGet and `go install` need no replacement. |
| 43 | `veld registry init` (scaffold `registry.config.json`) and `registry serve` | missing | Document `registry/` (`go run ./cmd/veld-registry`, Docker). If a CLI hook is wanted, `veld registry init` writes `registry.config.json` from `registry.config.example.json` with a generated secret. |
| 44 | JetBrains extras: color settings page, code style settings, structure view, brace matcher, commenter, documentation provider, file-change listener | downgraded | Structure view, hover and docs come from LSP4IJ via `veld lsp`; brace and comment behaviour from the TextMate `language-configuration.json`. The remaining gaps are the color settings page and the code style page. |
| 45 | Mock server (`internal/mock`: CORS mock from the contract) | missing | v1 never wired it into a command, but the code existed. Optional `veld mock [--port N]` serving `@example` data from the IR. |

## Detail tables

### 1. CLI commands and flags

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| `veld init` (interactive) | `cmd/veld/init.go:22` | same | `veld init --help`; prompts on a TTY, flags `--name --workspace --server --client -y` | P3 |
| `veld init` refuses to overwrite existing files | `init.go:620` | same | `apps/veld/src/commands/init/mod.rs` | P3 |
| `veld init` workspace mode (services, base URLs, consumes) | `init.go:315` | improved | `veld init --workspace a,b -y` writes contracts, `services/*/package.json` and `veld.toml` | P3 |
| `veld validate [file]` | `contract.go:30` | downgraded | renamed `veld check`; no file argument (summary 32) | P3 |
| `veld ast [file]` | `contract.go:60` | improved | `veld export ir` (full IR with signatures), hidden `veld debug` | P3 |
| `veld lint [--exit-code]` | `contract.go:87` | missing | no `lint` in `veld --help` (summary 4) | P2 |
| `veld openapi [-o]` | `contract.go:157` | improved | `veld export openapi -o FILE --service S`: OpenAPI 3.1 with problem+json, security schemes, servers, `x-veld-*` | P3 |
| `veld graphql [-o]` | `contract.go:198` | missing | no command (summary 5) | P2 |
| `veld schema --format prisma\|sql [-o]` | `contract.go:238` | missing | no command (summary 6) | P2 |
| `veld deps [--validate]` | `contract.go:289` | missing | validation only inside `check` (`E0702`) (summary 21) | P3 |
| `veld diff [--stat] [--exit-code]` (generated output vs fresh output) | `contract.go:344` | missing | v2 `diff` compares contracts, not output (summary 3) | P1 |
| `veld diff` as a breaking-change report (v1 only inside `generate`) | `internal/diff/diff.go:37` | improved | `veld diff [--lock\|--against REF] [--strict]`, `crates/veld-tools/src/diff` | P3 |
| `veld docs [--format html\|markdown] [-o]` | `contract.go:484` | missing | no command (summary 9) | P2 |
| `veld fmt [files] [-w]` | `contract.go:597` | improved | `veld fmt [PATHS] [--check]` writes in place; comments preserved; `--check` exits 1 | P3 |
| `veld generate` | `generate.go:591` | same | `veld generate [TARGET]` | P0 |
| `generate --dry-run` | `generate.go:747` | same | `generate --dry-run` lists create, update, delete | P3 |
| `generate --incremental` | `generate.go:745` | missing | summary 41 | P3 |
| `generate --overwrite-unmanaged` | `generate.go:749` | same | `generate --force` (also covers user-modified files) | P3 |
| `generate --setup` | `generate.go:751` | missing | `veld setup` is a separate step (summary 24) | P3 |
| `generate --validate` | `generate.go:753` | improved | validators always generated and on by default (`spec/targets/typescript.md` section 4) | P3 |
| `generate --strict` / `--force` breaking-change gate | `generate.go:755`, `generate.go:666` | downgraded | `commands/generate.rs` has no gate (summary 2) | P1 |
| `generate --backend/--frontend/--input/--out` | `generate.go:741` | missing | summary 19 | P2 |
| `generate --backend-framework/--frontend-framework` | `generate.go:759` | superseded | config moved to `veld.toml` `framework` (the one-off flag is summary 19) | P3 |
| `generate --server-sdk` / `--service-sdk` | `generate.go:763` | improved | a server target gets clients for every `consumes` service automatically (`spec/config.md`; `clients = false` opts out) | P3 |
| `generate --backend=openapi\|database\|dockerfile\|cicd\|env\|scaffold-tests` (tool emitters) | `generate.go:741` | missing | only `export openapi` remains (summary 30) | P2 |
| `veld watch` | `watch.go:25` | downgraded | `generate --watch` works (300 ms debounce, reloads `veld.toml`); the `veld watch` name is gone (summary 23) | P3 |
| `veld clean` | `generate.go:795` | improved | `clean [--dry-run]` keeps user-modified files and leaves a manifest of them | P3 |
| `veld clean --lock` | `generate.go:851` | missing | summary 31 | P3 |
| `veld setup [--backend-dir --frontend-dir]` | `tooling.go:45` | improved | `veld setup [TARGET] [--dry-run]`; `app = "path"` per target; `file:` dependency, Maven, Gradle; idempotent | P2 |
| `veld ci` | `registry.go:433` | missing | summary 24 | P3 |
| `veld lsp` | `tooling.go:180` | improved | hidden `veld lsp` (section 8) | P2 |
| `veld doctor` (hidden in v1) | `tooling.go:197` | missing | summary 10 | P2 |
| `veld completion <shell>` | `tooling.go:330` | missing | summary 22 | P3 |
| `veld login --registry --token` | `registry.go:26` | improved | `veld login [--registry --token --allow-insecure]`, token from stdin, per-registry `credentials.toml` | P3 |
| `veld logout` | `registry.go:72` | same | `veld logout [--registry]` | P3 |
| `veld push` (`--org --name --version`) | `registry.go:97` | improved | `veld publish [--package --dry-run]`; metadata from `veld.toml`; deterministic tar.gz | P3 |
| `veld pull @org/name[@ver]` | `registry.go:175` | improved | `veld add @org/name[@range]` resolves transitive deps and pins `veld.lock`; `veld install` | P3 |
| `veld export openapi\|graphql\|schema\|docs\|agents` group | `registry.go:256` | downgraded | `veld export openapi\|ir` only (summaries 5, 6, 9, 25) | P2 |
| `veld registry info\|list\|versions\|token create` | `registry.go:306` to `registry.go:386` | missing | summary 18 | P2 |
| `veld registry init` / `serve` | `registry.go:547` | missing | summary 43 | P3 |
| `--verbose` / `-v` | `main.go:86` | missing | summary 33 | P3 |
| `--quiet` / `-q` | `main.go:87` | same | `veld -q` | P3 |
| `--version` | `main.go:57` | same | `veld --version` | P3 |
| Config discovery (`veld.config.json`, `veld/veld.config.json`) | `config.go:283` | improved | `--config PATH`, upward search for `veld.toml` | P3 |
| v1 to v2 migration | (none) | improved | `veld migrate [--from --out --dry-run]` | P3 |
| Emitter plugins | (none) | improved | `veld plugin new` / `plugin test`, `spec/plugin-protocol.md` | P3 |

### 2. CLI UX (prompts, colours, banners, summaries, hints)

Restyle in progress; every v1 behaviour is listed so the restyle can keep it.

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| Banner and numbered menus with defaults (`Choose [1]:`) | `init.go:39` to `init.go:300` | same | interactive `init` on a TTY (README, `commands/init/answers.rs`) | P3 |
| Project-type detection (`pom.xml`, `package.json`, `composer.json`, `*.csproj`) | `init.go:60` | missing | `veld init` does not pre-select from existing files (summary 36) | P3 |
| Description prompt | `init.go:302` | missing | no description prompt; `[project].description` exists | P3 |
| "Runtime validation" prompt | `init.go:279` | improved | validators are always on, so the question is gone | P3 |
| "Run setup now?" prompt, backend and frontend dir prompts | `init.go:690` | missing | `init` prints `next: veld setup`; no prompt (summary 36) | P3 |
| Workspace prompts: service count, same backend, per-service name, base URL, consumes, frontend entry | `init.go:317` | same | workspace questions and `--workspace`, `--server service=...` | P3 |
| Colour helpers (green check, red cross, yellow warning, dim info) | `main.go:43`, `util.go:15` | improved | `--no-color`, `NO_COLOR`, TTY detection (`apps/veld/src/ui`) | P3 |
| Source snippet with line number in validation errors | `util.go:45` | improved | rustc-style `error[E0205]` with span and caret | P3 |
| "Did you mean" in errors | `validator.go:176`, `validator.go:188` | downgraded | summary 11 | P2 |
| Summary after generate (per dir, file groups) | `generate.go:418` | improved | `name -> out: N created, N updated, N deleted, N unchanged` | P3 |
| Import instructions after generate | `generate.go:456` | downgraded | only a `veld setup` hint (summary 34) | P3 |
| Breaking-change list ("Breaking changes detected", "Contract changes") | `generate.go:392` | downgraded | `veld diff` shows them; `generate` does not (summary 2) | P1 |
| Interactive "Generate anyway? [y/N]" | `util.go:22` | missing | summary 2 | P1 |
| Lint hint line after generate | `generate.go:157` | missing | summary 4 | P2 |
| Watch output: banner, timestamps, "config changed, reloading", "regenerated X (elapsed)" | `watch.go:46`, `watch.go:233` | same | `generate --watch` ("watching for changes") | P3 |
| Workspace progress `-> <service>` per entry | `generate.go:299` | same | one line per target | P3 |
| `hint: ... run veld setup` | `generate.go:456` | improved | `hint_setup`, only when targets are unwired | P3 |
| Spinners and progress bars | (none in v1) | same | none in v2 either | P3 |
| Exit codes (1 on any failure) | `main.go:99` | improved | `Exit::User` for user errors, see `apps/veld/src/error.rs` | P3 |

### 3. Config keys and behaviours

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| `input`, `$schema` | `config.go:51` | superseded | `[project].contracts` | P3 |
| `backend` / `backendConfig.target`, `frontend` / `frontendConfig.target` | `config.go:23`, `config.go:68` | superseded | `[[targets]] role, emitter` | P3 |
| `framework` (backend, frontend) | `config.go:25` | improved | `framework` validated by the emitter, `E0803` lists valid values; `framework-version` | P3 |
| `out`, `backendOut`, `frontendOut` | `config.go:72` | superseded | `[[targets]] out`, with overlap and forbidden-dir checks `E0703`, `E0704` | P3 |
| `backendDir`, `frontendDir` (alias `backendDirectory`) | `config.go:73` | improved | `app = "path"`, or auto-detect (`spec/config.md`) | P3 |
| `validate` (flat and nested) | `config.go:76` | improved | always on; `validateResponses` hook at runtime | P3 |
| `baseUrl` (global) | `config.go:55` | improved | per-service `base-url`, `VELD_<SERVICE>_URL`, constructor argument | P3 |
| `services` (module to base URL) | `config.go:58` | improved | `[services.x] base-url` | P3 |
| `aliases` (`@alias` to folder) | `config.go:57` | superseded | `use pkg:module.{Name}` imports | P3 |
| `description` | `config.go:54` | same | `[project].description` | P3 |
| `serverSdk` | `config.go:59` | improved | `clients = true` default for servers | P3 |
| `workspace[]` (name, input, backend, frontend, out, baseUrl, consumes, description, validate, serverSdk) | `config.go:11` | improved | `[services.*]`, `[libraries.*]`, `[[targets]] service/services` | P3 |
| `consumes` with `"*"` wildcard | `config.go:189` | missing | summary 38 | P3 |
| `consumes` validation (unknown, self, circular) | `validator/workspace.go` | improved | `E0702`, `W0701`; cycles are legal (rule 6) | P3 |
| `tools` block (openapi, dockerfile, cicd, database, scaffold, envconfig) | `config.go:41` | missing | `veld migrate` warns "auxiliary generators are replaced by `veld export openapi`" (`apps/veld/src/commands/migrate/plan.rs:60`) (summary 30) | P2 |
| `hooks.postGenerate` and flat `postGenerate` | `config.go:48`, `generate.go:23` | missing | `veld migrate` warns "post-generate hooks have no v2 equivalent" (`plan.rs:64`) (summary 1) | P1 |
| `registry` block (enabled, url, org, package, version) | `config.go:79` | improved | `[registry].url`, `package`, `version`, `[dependencies]`, `veld.lock` | P3 |
| Unknown keys | `config.go:283` | improved | `E0701` with the closest key | P3 |
| Precedence: flags over file over defaults | `config.go:310` | same | `--config`, `--registry`, env `VELD_REGISTRY` | P3 |
| Paths relative to the config file, not the cwd | `docs/DEVELOPER_GUIDE.md:424` | same | `spec/config.md`: "Paths are resolved relative to the file" | P3 |
| Output safety: refuse project root, contract dirs, home, filesystem root | `internal/output/policy.go` | improved | `E0704`, `E0703`, manifest, user-modified file protection | P0 |
| Staged output and manifest (`.veld-manifest.json`) | `internal/output/plan.go` | improved | `veld_project::output`; stale staging dirs removed | P0 |
| `.veld.lock.json` baseline | `internal/diff/lock.go` | improved | `veld.lock.ir.json` (full IR) | P3 |
| `.veld-cache.json` | `internal/cache/cache.go:28` | missing | summary 41 | P3 |
| Default aliases (`models`, `modules`, `types`, ...) | `config.go:521` | superseded | `use` imports | P3 |

### 4. Language features

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| Keywords `model`, `enum`, `import`, `extends` | `language/constants.go:29` | superseded | `model`, `enum`, `use`, `extends` (`crates/veld-compiler/src/language.rs:9`) | P3 |
| `module { prefix, description }` and `action` blocks | `ast/ast.go:54`, `ast.go:65` | superseded | `service Name at "/p" { get "/x" name() }`, `///` docs | P3 |
| `constants` groups with typed fields | `ast/ast.go:98` | superseded | `const NAME: type = literal` (grouping is gone, but constants are emitted in every target: `crates/emitters/typescript/src/models.rs:175`) | P3 |
| HTTP methods GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS, WS | `language/constants.go:38` | same | `get post put patch delete head options ws` | P3 |
| Builtin types `string int long float decimal bool date datetime time uuid bytes json` | `constants.go:52` | same | `language.rs:29` | P3 |
| `any` type | `constants.go:62` | superseded | `json` | P3 |
| `List<T>` / `T[]` | `constants.go:72`, `ast.go:43` | same | `T[]`, `T[][]` | P3 |
| `Map<string, V>` | `ast.go:45` | same | `Map<string, T>`, key must be string (`E0301`) | P3 |
| Optional fields (`?`, `@optional`, `@required`) | `constants.go:86` | same | `name?: T` | P3 |
| Model `extends` with cycle check | `validator.go:125` | improved | `E0401`, `E0402`, `E0408`, `E0006` | P3 |
| Field-level literal unions | `ast.go:41` | superseded | `enum` and `union` declarations (migrate output is `json`, summary 39) | P3 |
| `@default(v)` | `constants.go:85` | improved | `= literal` with type check `E0303`, `E0304` | P3 |
| `@unique`, `@index`, `@primary` | `constants.go:89` | same | `language.rs:190` to `language.rs:202` (IR metadata; no consumer yet, summary 6) | P3 |
| `@autoincrement` | `constants.go:92` | missing | not in `language.rs` (summary 8) | P2 |
| `@relation(Model)` | `constants.go:96` | missing | not in `language.rs`; migrate drops it (summary 7) | P2 |
| `@readonly` and `@serverSet` | `ast.go:49` | same | `@readonly`: dropped from request types, present in responses | P3 |
| `@deprecated("msg")` on fields and actions | `ast.go:44`, `ast.go:85` | improved | on any declaration, field, variant or endpoint; copied into the IR and OpenAPI | P3 |
| `@example(v)` | `ast.go:45` | same | `@example(literal)` | P3 |
| `@min`, `@max`, `@minLength`, `@maxLength`, `@regex` | `constants.go:97` to `constants.go:101` | improved | `@min @max @length(min,max) @pattern @format`, portable regex subset `E0603` | P3 |
| Enum with description | `ast.go:21` | improved | `///` docs on enum and variants, `= "WIRE"` values | P3 |
| Directives `description`, `prefix`, `method`, `path` | `constants.go:64` | superseded | service prefix, endpoint method and path | P3 |
| `input` / `output` / `response` directives (named models) | `ast.go:68` | superseded | `(body: In) -> 201 Out` | P3 |
| Inline anonymous input, output, query and header fields | `ast.go:69` | superseded | parameters are inline, result types are named | P3 |
| `query` directive | `ast.go:74` | same | `query: Model` or `@query` | P3 |
| `headers` directive | `ast.go:76` | improved | `@header("Name") p: T` per parameter | P3 |
| `stream` and `emit` directives for WS | `parser.go:985` | improved | `ws "/p" f(body: stream In) -> stream Out`, validated like HTTP | P3 |
| `middleware` directive | `ast.go:81` | improved | `@middleware(A, B)` on service and endpoint; hooks required by type | P3 |
| `errors` directive with names and per-error status | `ast.go:79` | improved | typed `error` declarations, prelude errors, signatures, problem+json | P3 |
| Explicit success status (`output: User:201`) | `ast.go:80` | improved | `-> 201 T`; bare `-> 204`; `E0507` | P3 |
| Top-level `prefix` | `ast.go:6` | missing | summary 37 | P3 |
| Module `baseUrl` | `ast.go:58` | improved | per-service `base-url` | P3 |
| Multi-file imports, circular-import guard | `internal/loader/loader.go` | improved | `use`, cycles allowed by design (`spec/language.md` section 2.1) | P3 |
| Cross-service imports (`import @iam/models/*`) | `generate.go:265` | improved | `use iam:users.{User}` | P3 |
| Registry packages in imports | `internal/registry` | improved | `use @acme/billing:invoices.{Invoice}`, `E0206` visibility | P3 |
| `Page<T>` | (none) | improved | built-in generic | P3 |
| `union` (discriminated) | (none) | improved | `union` with `@discriminator` | P3 |
| Duplicate names, unknown types, WS rules, status rules validated | `validator.go:55` to `validator.go:292` | improved | codes `E02xx` to `E05xx` in `spec/diagnostics.md` | P3 |
| `@default` and constant literal validation | `validator.go:420`, `validator.go:549` | improved | `E0303`, `E0304`, `E0305` | P3 |
| Formatter | `internal/format/format.go:1` | improved | `veld fmt`, comments preserved | P3 |
| Language and annotation list drives editor grammars | `language/constants.go:1` | improved | `language.rs` generates `editors/veld-language.json` | P3 |
| Contract lint rules (naming, unused, empty, deprecation) | `lint/lint.go:103` to `lint.go:330` | missing | summary 4 | P2 |

### 5. Generator outputs per kept target

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| Node: model types, barrel `index.ts`, `package.json` | `emitter/backend/node/barrel.go:25`, `barrel.go:52` | same | `server/src/generated/{index.ts,package.json,models/}` in `examples/hello` | P3 |
| Node: service interfaces `I<Module>Service` | `node/interfaces.go:136` | same | `<Service>Service` interface in `server/<service>.ts` | P3 |
| Node: routes per module (Express) | `node/routes.go:108` | improved | one handler per service plus adapters for express, fastify, hono, node-http, nest | P3 |
| Node: route ordering (literal before param) | `emitter/routeorder.go` | improved | router core matches exact literals first | P3 |
| Node: `_respond.ts`, error mapping, 5xx hides details | `node/respond.go:61` | improved | problem+json; undeclared errors become `Internal` without a message | P3 |
| Node: runtime validators `_validators.ts` (optional) | `node/validate.go:69` | improved | `Model.validate`, always wired, `validateResponses` | P3 |
| Node: `middleware/IMiddleware.ts` | `node/routes.go:351` | improved | typed `Hooks.middleware`, required by type | P3 |
| Node: `constants/` | `node/constants.go:58` | same | consts emitted into models | P3 |
| Node: typed errors (`errors.ts`, codes) | `node/errors.go` | improved | error classes with `matches` and signatures | P3 |
| Node: server-to-server SDK `sdk/` | `node/sdk.go:40` | improved | `clients/<service>/` per consumed service | P3 |
| Node: WS stubs | `node/routes.go` | improved | `server/ws.ts` `attachWebSocket` | P3 |
| Node: Express strategy | `node/strategy/express.go` | improved | express, fastify, hono, node-http, nest | P3 |
| Node: plain JavaScript (`node-js`) | `backend/javascript` | superseded | removed emitter | P3 |
| Node: generated `README.md` | `cmd/veld/generate.go:864`, `stagedgen.go:190` | missing | summary 35 | P3 |
| TypeScript client: per-module client classes | `frontend/typescript/client.go:13` | same | `<package>/client.ts` | P3 |
| TypeScript client: aggregate `VeldApiClient` and `api` singleton | `client.go:166` | missing | summary 40 | P3 |
| TypeScript client: `VeldApiError` and typed error codes | `client.go:62`, `tsshared/errors.go` | improved | `VeldError`, `VeldHttpError`, `VeldTransportError` | P3 |
| TypeScript client: config `baseUrl`, headers, env `VELD_API_URL` | `client.go:28` | improved | `ClientOptions` (`baseUrl`, `fetch`, `headers`, `auth`), `VELD_<SERVICE>_URL` | P3 |
| TypeScript client: GET input sent as query | `client.go:136` | improved | binding rules in `spec/language.md` section 6 | P3 |
| TypeScript client: URL-encoded path params | `client.go:121` | same | `encodeURIComponent` | P3 |
| TypeScript client: WebSocket with auto-reconnect | `typescript/websocket.go:13` | downgraded | `WsChannel` without reconnect (summary 13) | P2 |
| TypeScript client: AbortSignal | (none) | improved | `init?: { signal }` | P3 |
| TypeScript client: SSE | (none) | improved | `AsyncIterable<T>` over `fetch` | P3 |
| React hooks (TanStack Query) | `frontend/react/emitter.go:157` to `emitter.go:325` | downgraded | own hooks `{data,error,loading,refetch}` and `mutate`, no cache or invalidation (summary 12) | P2 |
| Vue composables | `frontend/vue/emitter.go:108` | improved | refs, reactive requests, `onScopeDispose` | P3 |
| Angular, Svelte, Dart, Kotlin, Swift, JS, types-only frontends | `frontend/*` | superseded | removed emitters (plugins) | P3 |
| Java: model records, enums, constants | `backend/java/types.go`, `constants.go:58` | improved | records, sealed interfaces, unions, consts | P3 |
| Java: `I<Module>Service` interfaces | `java/interfaces.go:121` | same | `<Service>Service` | P3 |
| Java: Spring controllers | `java/routes.go:85`, `strategy/spring.go` | improved | native `@RestController` per service (`spec/targets/java.md` section 1) | P3 |
| Java: Spring `GlobalExceptionHandler`, `ApiException` family | `java/errors.go:86` | improved | typed error classes, problem+json | P3 |
| Java: interceptors and `VeldAutoConfiguration` | `java/middleware.go:50`, `middleware.go:127` | improved | `<Service>Hooks` and the `Middleware` hook, injected beans | P3 |
| Java: bean validation | `java/validation.go` | improved | generated validators, always on | P3 |
| Java: plain strategy with `build.gradle` | `java/strategy/plain.go` | improved | JDK adapter (`JdkHttpServerAdapter`); `veld setup` wires Gradle or Maven | P3 |
| Java: service-to-service SDK | `java/sdk.go:96` | improved | `clients/<service>/` per consumed service | P3 |
| Java: Spring Boot 3.x / 4.x | `strategy/spring.go:19` | same | only Boot 3 (`framework-version = "3"`) is documented; v1's 4.x claim was a comment only (its pom pinned 3.2.0) | P3 |
| Python, Go, Rust, C#, PHP backends | `backend/*` | superseded | removed emitters | P3 |

### 6. Auxiliary tools

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| OpenAPI export | `generators/openapi/openapi.go:1` | improved | OpenAPI 3.1, security, problem+json, deprecated, streams | P3 |
| GraphQL SDL export | `graphqlgen/graphql.go:13` | missing | summary 5 | P2 |
| Prisma and SQL schema | `schema/prisma.go:45`, `schema/sql.go:43` | missing | summary 6 | P2 |
| Database ORM models (TypeORM, GORM, SQLAlchemy) | `generators/database/database.go:78` | missing | summary 6 (TypeORM is the only one relevant to a kept target) | P2 |
| API docs (HTML, Markdown) | `docsgen/html.go:12`, `docsgen/markdown.go:13` | missing | summary 9 | P2 |
| `AGENTS.md` | `docsgen/agents.go:13` | missing | summary 25 | P3 |
| Dockerfile, `.dockerignore`, compose | `generators/dockerfile/dockerfile.go:29` | missing | summary 26 | P3 |
| CI workflow | `generators/cicd/cicd.go:28` | missing | summary 27 | P3 |
| `.env.example` | `generators/envconfig/envconfig.go:28` | missing | summary 28 | P3 |
| Test scaffolds | `generators/scaffold/tests.go:33` | missing | summary 29 | P3 |
| Mock server | `mock/mock.go:20` | missing | summary 45 (never wired to a command in v1) | P3 |
| Lint | `lint/lint.go:28` | missing | summary 4 | P2 |
| Doctor | `cmd/veld/tooling.go:197` | missing | summary 10 | P2 |
| Deps graph | `cmd/veld/contract.go:289` | missing | summary 21 | P3 |
| Breaking-change diff | `diff/diff.go:37` | improved | `veld diff` | P3 |
| Formatter | `format/format.go` | improved | `veld fmt` | P3 |
| Setup (tsconfig paths, Vite alias, `package.json`, `pom.xml`, Gradle, `go.mod`, `Cargo.toml`, ...) | `setup/setup.go:81` | improved | `file:` dependency (works for tsc, tsx, Vite, test runners), Maven, Gradle (`spec/config.md`) | P2 |

### 7. Registry

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| Registry server (PostgreSQL, JWT, TOTP, orgs, SMTP, web UI) | `internal/server/**` (moved to `registry/`) | same | `registry/internal/server` has the same files plus `handlers/validate.go` | P3 |
| Publish: pack, upload, `vtk_` tokens | `registry/tarball.go`, `cmd/veld/registry.go:97` | improved | deterministic tar.gz, `veld-package.toml`, size limits, manifest validation | P3 |
| Install: unpack, verify checksum | `registry/tarball.go` | improved | `veld.lock` with sha256, transitive resolution, conflict paths | P3 |
| Credentials file | `registry/credentials.go` | improved | `~/.veld/credentials.toml`, `VELD_HOME`, https enforced | P3 |
| `registry list`, `versions`, `info`, `token create` | `cmd/veld/registry.go:306` | missing | summary 18 | P2 |
| `registry init`, `serve` | `cmd/veld/registry.go:547` | missing | summary 43 | P3 |

### 8. LSP and editors

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| LSP diagnostics | `internal/lsp/handler.go` | improved | `veld lsp`; config errors land on `veld.toml` too (`crates/veld-lsp/src/diag.rs`) | P2 |
| LSP completion (keywords, types, `@alias`) | `lsp/completion.go` | improved | types with auto-import, annotations with argument snippets, `use` paths, enum variants (`complete.rs`) | P2 |
| LSP hover, definition | `lsp/hover.go`, `lsp/definition.go` | improved | `hover`, `definition`, `references`, `rename`, `documentSymbol`, `formatting`, `semanticTokens` (`server.rs:279`) | P2 |
| Completion for `veld.config.json` keys (schema) | `editors/*/veld-config.schema.json` | missing | no `veld.toml` completion; stale v1 schema file still in `editors/` (summary 16) | P2 |
| VS Code: syntax highlighting, language configuration | `vscode/package.json` (`grammars`) | same | `editors/vscode/syntaxes/veld.tmLanguage.json` | P3 |
| VS Code: semantic token scopes | `package.json` (`semanticTokenScopes`) | improved | provided by the language server | P3 |
| VS Code: commands Validate, Generate, Generate (Dry Run) | `package.json` (`commands`) | missing | summary 14 | P2 |
| VS Code: snippets (contract, config) | `package.json` (`snippets`) | missing | summary 15 | P2 |
| VS Code: `veld.executablePath`, `veld.validateOnSave` | `package.json` (`configuration`) | improved | `veld.path`, `veld.trace.server`; diagnostics as you type replace validate on save | P3 |
| VS Code: JSON schema validation for config | `package.json` (`jsonValidation`) | missing | summary 16 | P2 |
| VS Code: activation | `activationEvents: []` | improved | `onLanguage:veld`, `workspaceContains:veld.toml` | P3 |
| JetBrains: syntax highlighting | `VeldSyntaxHighlighter.kt` | same | TextMate grammar | P3 |
| JetBrains: completion, hover, go to definition, references | `VeldCompletionContributor.kt`, `VeldReferenceContributor.kt` | improved | via LSP4IJ | P2 |
| JetBrains: actions Validate, Generate, Dry Run | `actions/VeldActions.kt`, `plugin.xml:177` | missing | summary 17 | P2 |
| JetBrains: New > Veld File templates | `VeldCreateFileAction.kt`, `fileTemplates/*` | missing | summary 17 | P2 |
| JetBrains: config JSON schema | `VeldConfigSchemaProviderFactory.kt` | missing | summary 16 | P2 |
| JetBrains: color settings, code style, structure view, brace matcher, commenter | `VeldColorSettingsPage.kt` and others | downgraded | summary 44 | P3 |
| JetBrains: settings page for the executable | `VeldProjectService.kt` | improved | `VeldConfigurable.kt`, `VeldExecutable.kt` | P3 |

### 9. Hooks, caching and performance

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| `hooks.postGenerate` | `cmd/veld/generate.go:23` | missing | summary 1 | P1 |
| Incremental generation and cache | `generate.go:94`, `cache/cache.go` | missing | summary 41; a six-target dry run takes 0.2 s | P3 |
| Watch debounce | `watch.go` (500 ms, `CLAUDE.md:44`) | same | 300 ms in `apps/veld/src/commands/watch.rs:18` | P3 |
| Determinism (byte-identical output) | `cmd/veld/determinism_test.go` | improved | rule 2, wire goldens in `tests/wire` | P3 |
| Atomic, staged output | `cmd/veld/stagedgen.go` | improved | `veld_project::output` | P0 |

### 10. Distribution and docs

| v1 feature | where in v1 | v2 status | evidence | severity |
|---|---|---|---|---|
| npm wrapper `@maayn/veld` | `archive/v1/distribution/` (npm at repo root in v1-final) | same | `distribution/npm` | P3 |
| pip wrapper `maayn-veld` | same | same | `distribution/pip` | P3 |
| Homebrew formula | same | same | `distribution/homebrew` | P3 |
| apt, Chocolatey, Composer, NuGet, `go install` | `archive/v1/distribution/{apt,chocolatey,composer,nuget,go}` | missing | summary 42 | P3 |
| Release workflow | `archive/v1/github-workflows/release.yml` | same | `.github/workflows/release.yml` | P3 |
| Getting started, generated-output, service-sdk, CI guides | `archive/v1/docs/guides/*`, `docs/ci-cd/guide.md` | downgraded | summary 20 (the website has docs pages, `docs/` does not) | P2 |
| Developer guide | `archive/v1/docs/DEVELOPER_GUIDE.md` | improved | `CLAUDE.md`, `CONTRIBUTING.md`, `docs/emitter-guide.md`, normative `spec/` | P3 |
| Examples | `archive/v1/examples/*` (10 stacks) | improved | three tested examples (`hello`, `microservices`, `library-package`) | P3 |

## Counts

Detail-table rows by status (216 rows): improved 96, same 40, missing 53, superseded 17, downgraded 10.

Summary checklist (distinct gaps; one gap can appear in several detail rows): 45 items, 35 missing and 10 downgraded, split as 3 at P1, 17 at P2 and 25 at P3.
