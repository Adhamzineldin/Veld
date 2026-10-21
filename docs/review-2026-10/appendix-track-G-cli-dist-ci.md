# Appendix: audit-track notes (G-cli-dist-ci)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Veld CLI / config / distribution / CI / website / tests audit

Commit 6a47f81, reviewed 2026-10-21. Binary built with Go 1.26 to `scratchpad/cli-audit/veld.exe`. All experiments ran on copies in `scratchpad/cli-audit/` (sandboxes p1..p6, sb, cfg, ex). No tracked file modified.
Note: every command exits 1 on any failure (cobra error -> `os.Exit(1)` in main.go:105; direct `os.Exit(1)` at contract.go:48, contract.go:471, init.go:31). There is no distinction between usage / validation / IO errors, and no `--json` flag anywhere.

## PART A - CLI inventory

Registered in main.go:88-101. Hidden = not in `--help` (but advertised in the Long help text / README).

| command | file:line | flags | reads config? | exit on failure (tested) | machine-readable | writes |
|---|---|---|---|---|---|---|
| (root) | main.go:55 | -v/--verbose, -q/--quiet (both parsed, near-unused), --version | no | no args = prints help, exit 0; `--version` -> `veld version 0.1.0` exit 0 (source builds always say 0.1.0; ldflags only in release.yml) | none | - |
| init | init.go:20 | none (fully interactive; works with stdin closed, takes defaults) | checks for existing config | 2nd run exit 1, no files touched (mtimes identical); BUT see F-CLI-05 (overwrites README/veld/*.veld when no config exists) | none | veld/*, README.md, may rewrite veld/veld.config.json (init.go:788), runs setup |
| generate | generate.go:700 | --backend --frontend --input --out --incremental --dry-run --setup --validate --strict --force --backend-framework --frontend-framework --server-sdk --service-sdk | yes (BuildResolved) | missing config exit 1; missing input exit 1; unknown backend exit 1 (but output already deleted, F-CLI-01) | none | out dirs (RemoveAll first), `<configDir>/.veld.lock.json`, `.veld-cache.json`, README.md in out |
| watch | watch.go:26 | --backend --frontend --input --out | yes | no config exit 1; generation errors do not exit (verified: bad edit printed error, next good edit regenerated). Ctrl-C handled by `signal.NotifyContext(os.Interrupt)` (SIGTERM not handled; Windows Ctrl-C not testable here, `timeout` kill gave 124) | none | same as generate (initial runClean) |
| clean | generate.go:1144 | none (ignores --out) | yes | no config exit 1 | none | deletes out dirs recursively + lock + cache |
| validate [file] | contract.go:29 | positional file | yes (or file arg) | invalid contract exit 1 (semantic via os.Exit(1), parse via error); workspace config exit 1 ("no input file") | none (stderr text) | - |
| ast [file] (hidden) | contract.go:59 | file | yes | exit 1 on error; reproduced exit 0 + JSON on valid | JSON on stdout | - |
| lint | contract.go:86 | --exit-code | yes | exit 1 only if validate fails or (error-level issue or --exit-code with any issue). Warning-only without flag exit 0 (verified). Error-level lint rules unreachable (F-CLI-12) | none | - |
| diff | contract.go:343 | --stat --exit-code | yes | no lock: prints everything as new, exit 0; with --exit-code and changes exit 1 | none | - |
| deps | contract.go:288 | --validate | yes | n/a (workspace only) | text | - |
| openapi / export openapi | contract.go:156 | -o | yes | validation failure exit 1 | JSON on stdout | -o file |
| graphql | contract.go:197 | -o | yes | exit 1 | SDL stdout | -o file |
| schema | contract.go:237 | --format prisma/sql, -o | yes | exit 1 | stdout | -o file |
| docs | contract.go:483 | --format html/markdown, -o | yes | exit 1 | file | docs.html / docs.md in cwd |
| fmt [files] (hidden) | contract.go:594 | -w | yes | exits 0 even if files "would change" (no --check) | none | rewrites .veld files with -w |
| setup | tooling.go:44 | --backend-dir --frontend-dir --backend --frontend --input --out | yes | empty dir exit 1 ("could not load config") | none | patches tsconfig/vite/package.json etc. |
| ci | registry.go:432 | --backend --frontend --input --out --strict | yes | --strict + breaking change: exit 1 (works, verified); does NOT clean first | none | generate + setup |
| lsp | tooling.go:179 | none | no | n/a | LSP stdio | - |
| completion | tooling.go:329 | shell arg | no | n/a | script on stdout | - |
| doctor | tooling.go:195 (`newDoctorCmd`) | - | - | NOT REGISTERED: `veld doctor` -> `unknown command "doctor"` exit 1 (F-CLI-04) | - | - |
| export {openapi,graphql,schema,docs,agents} | registry.go:253 | per subcommand | yes | as above | stdout/-o | -o file |
| registry {info,list,versions,login,logout,push,pull,serve,init,token create} | registry.go:297+ | --registry --token --org --name --version --out --addr --dsn --secret --storage --base-url | credentials file / config | network errors exit 1 | JSON from server only | credentials store, packages |

Top-level `veld login|logout|push|pull|serve` (documented in CLAUDE.md and README) are NOT top-level: `veld login` -> unknown command (they live under `veld registry`). Documentation mismatch (F-CLI-14).

### Experiment log (exit codes / stderr)

| experiment | result |
|---|---|
| `veld` | help, exit 0 |
| `veld --version` | `veld version 0.1.0`, exit 0 |
| `generate` (no config) | exit 1 `no input file (use --input or create veld/veld.config.json)` |
| `generate --input=missing.veld` | exit 1, absolute-path OS error |
| `generate --backend=nonexistent` (config present) | exit 1 `unknown backend/tool ... (supported: ...)` but previous output already deleted (F-CLI-01) |
| `generate --out=..` | exit 1 rejected (basename check) |
| `generate --out=/` , `C:\`, `C:/`, `../..`, `.` | exit 1 rejected (validateOutPath, config.go:365). (Under Git Bash `/` is mangled to `C:/Program Files/Git`; with MSYS_NO_PATHCONV=1 it is rejected.) |
| `generate --out=../veld` (= the directory holding the .veld sources) | ACCEPTED; `runClean` deleted the whole `veld/` source directory, then failed `reading ...app.veld` (F-CLI-02) |
| `clean` with `out: ./src` containing `business.ts` | `src/` removed entirely, exit 0, no prompt (F-CLI-03) |
| invalid contract then `generate` | exit 1, and previous generated dirs + lock + cache are gone (F-CLI-01) |
| `generate --dry-run` | writes no output, but DOES create `veld/.veld-cache.json` (before/after listing diff) (F-CLI-06) |
| `generate --strict` / `--force` / plain with a removed action vs lock | all exit 0, nothing blocked, no prompt (gate dead, F-CLI-01) |
| `ci --strict` same scenario | exit 1, "Breaking changes detected - aborting" (works) |
| interactive prompt (stdin closed) | unreachable in practice; code path `promptContinue` (util.go:22) returns false on EOF -> "generation aborted" exit 1, no hang (source) |
| `init` twice | 2nd exit 1, mtimes + md5 unchanged |
| `init` where README.md and veld/app.veld exist but no config | exit 0, both OVERWRITTEN (F-CLI-05) |
| `validate` invalid | exit 1; unknown type and parse error both exit 1 |
| `lint --exit-code` on warnings | exit 1; without flag exit 0 |
| `doctor` | exit 1 unknown command |
| `setup` empty dir | exit 1 |
| `diff` no lock | exit 0, lists all files as new |
| `watch` 12s with edits | initial gen ok, edit -> regenerated in ~80ms, garbage edit -> error printed, process stays alive, revert -> regenerated; killed by timeout (exit 124) |

## PART B - Config

Source: internal/config/config.go (556 lines), config_test.go (804 lines, 29 tests).

Precedence (verified by experiment): **CLI flag > flat field > nested field > default**. The `normalize()` code (config.go:137-150) only copies nested values when the flat field is EMPTY, so flat wins over nested. Experiment: `{"backend":"python","backendConfig":{"target":"node-ts"}}` generated Python. This contradicts CLAUDE.md ("Nested takes precedence if both are present") and the comment at config.go:34 (F-CLI-07).
Exception: `--out` does not beat `backendConfig.out`/`backendOut` (backend dir stays at config value, only frontend follows `--out`) (F-CLI-08).

| item | finding |
|---|---|
| backend values accepted | node, node-ts, node-js, python, go, rust, java, csharp, php (+ tools: openapi, database, dockerfile, cicd, env, scaffold-tests). Aliases: node->node-ts, js/javascript/node-javascript->node-js, node-typescript->node-ts. `express`, `python-flask`, `flask`, `typescript`, `nodejs` -> exit 1 `unknown backend/tool` (framework is a separate field) |
| frontend aliases | flutter->dart, ts->typescript, hooks/react-hooks->react, js->javascript |
| default frontend | typescript for node backends, none otherwise |
| path resolution | config discovered ONLY in `./veld.config.json` or `./veld/veld.config.json` (no walk-up). `input`, `out`, `dir` are relative to the CONFIG FILE dir, not cwd (verified: `--out=x` is also relative to config dir). Exception: passing `--input` re-bases everything to cwd (config.go:324-327), so `out` from the config then resolves against cwd (verified `./o` vs `./veld/o`). Inconsistent (F-CLI-09) |
| aliases defaults | models, modules, types, enums, schemas, services, lib, common, shared (identity mapping); user entries merged |
| `$schema` | accepted and ignored |
| unknown keys | silently ignored (`bogusKey` accepted; plain `json.Unmarshal`, no DisallowUnknownFields). Typos like `"outt"` silently fall to defaults (F-CLI-09) |
| comments / trailing commas | rejected with raw encoding/json message, no line/column: `parsing veld.config.json: invalid character '/' looking for beginning of object key string` (design limitation, low) |
| wrong type | `json: cannot unmarshal number into Go struct field RawConfig.input of type string` (decent) |
| empty file | `unexpected end of JSON input` |
| `hooks.postGenerate` | executed with `exec.Command("sh","-c", cmd)` (generate.go:29), cwd = config dir, stdout/stderr inherited. NOT `cmd /C` on Windows: needs `sh` on PATH (works under Git Bash; fails on plain cmd/PowerShell machines). Hook failure only prints a warning; `exit 3` hook -> veld exit 0 (verified). Design consideration: a config file is executable code (anyone who can edit veld.config.json, e.g. a PR to a CI repo, gets shell execution on `veld generate`/`veld ci`) - same trust model as npm scripts, not a vuln but worth documenting (F-CLI-10) |
| workspace entries | `out` not validated by validateOutPath (config.go:360-377 applies only to cfg.Out/BackendOut/FrontendOut); workspaceOutputDirs (config.go:481) feeds runClean directly, so a workspace `out: "."` is deleted wholesale. Not tested destructively |
| validateOutPath | rejects only basename `..`, `.`, `/`, `\`. Accepts any other directory incl. the project root's own folder name, source dirs, absolute paths such as `C:\Users` (F-CLI-02/03) |

## PART C - Distribution wrappers

| wrapper | binary acquisition | version pin | checksum | platforms | lifecycle / offline | grade |
|---|---|---|---|---|---|---|
| npm `@maayn/veld` (packages/npm) | CI bundles `binaries/<plat>` into the tarball (release.yml publish-npm); `install.js` postinstall downloads from GitHub Releases only if the binary is missing | wrapper version 0.1.0 in repo; CI rewrites package.json. install.js asks GitHub API for `releases/latest` (NOT the wrapper's version) unless VELD_VERSION set | none | linux/darwin x64+arm64, windows x64. `arm64` on windows maps to `windows-arm64` which release.yml never builds -> 404 then `go install`. musl: static CGO_ENABLED=0 binary, fine | postinstall failure is swallowed: verified `npm install` exit 0 with bad VELD_VERSION and then `veld` prints "binary not found" exit 1. No proxy support (raw https.get, no HTTP(S)_PROXY). Falls back to `go install ...@latest` (unpinned). Uninstall: nothing outside node_modules. `process.exit(result.status \|\| 0)` returns 0 when child killed by signal | needs work |
| pip `maayn-veld` (packages/pip) | binaries bundled at CI time into `veld/binaries`; no download code | CI sed-rewrites version | n/a (bundled) | same 5 targets; wheel is `py3-none-any` containing all 5 binaries and falls back to PATH | Uses `str \| None` return annotations (evaluated at def time) while `requires-python >=3.8`: TypeError at import on 3.8/3.9 (source-supported, not run: only 3.13 available). Not a platform wheel -> every install carries ~100MB of binaries; exec bit relies on the sdist/wheel preserving mode. README says `pip install veld`, project name is `maayn-veld` (F-CLI-16) | needs work |
| Homebrew (packages/homebrew) | url+sha256 per os/arch in formula | `version "0.1.0"` hardcoded | sha256 present but PLACEHOLDER_* in repo; release.yml update-homebrew job edits the formula on the runner and never commits or pushes it, so nothing is published (F-CLI-15) | darwin/linux x64+arm64 | n/a | placeholder |
| Go (packages/go) | `go install github.com/Adhamzineldin/Veld/cmd/veld@latest` | module version via git tags (47 tags, v0.1.0..v0.3.9) | go.sum / sumdb | any | works if module path public. Binary reports `0.1.0` because Version is only set via ldflags; `go install` build prints wrong version (F-CLI-13) | ready (README only) |
| NuGet `Maayn.Veld` (packages/nuget) | downloads GitHub release archive on first run into LocalAppData/veld/<ver>/<plat> | const Version "0.1.0", CI seds it | none | windows/linux/darwin x64+arm64 (windows-arm64 asset missing) | requires network at first run; no proxy config beyond HttpClient defaults; ExtractToDirectory(overwrite) on possibly partial download not guarded (a partially written archive leaves cache dir without binary but retried next run, OK). Proxies Process exit code, good. net8.0 required | needs work |
| Chocolatey (packages/chocolatey) | Install-ChocolateyZipPackage with sha256 | version rewritten by CI | `PLACEHOLDER_SHA256_WINDOWS_AMD64` replaced in CI only; in-repo copy is a placeholder | windows-amd64 only | nuspec `licenseUrl` points to `blob/master/LICENSE`; the repo has no tracked LICENSE file (F-CLI-17) | needs work |
| apt (packages/apt nfpm) | CI builds .deb from release tarball and pushes to packagecloud | CI | n/a | amd64, arm64 | installs to /usr/local/bin/veld (non-standard for .deb, should be /usr/bin) | needs work |
| Composer `maayn/veld` (packages/composer) | bin/veld downloads on first run to cache dir; `Installer::install` hook | `VELD_VERSION` const 0.1.0, NOT touched by release.yml (no composer job; `publish_composer` output computed but unused) | none | linux/darwin/windows x64+arm64 (windows-arm64 missing) | `file_get_contents` needs allow_url_fopen, no proxy support. Composer does not run a dependency's `post-install-cmd` (only the root package's), so the Installer hook never fires for consumers; download is lazy. Packagist needs composer.json at the repo root - this is in a monorepo subdir, so it cannot be registered as-is | placeholder |

Cross-cutting: no wrapper verifies checksums (release.yml publishes `checksums.txt` but nothing consumes it); release asset naming differs in release-notes text (`brew install veld-dev/tap/veld`) vs formula (`maayn-veld/tap/maayn-veld`); wrappers all say 0.1.0 while tags reach v0.3.9.

## PART D - CI / release / scripts

| workflow | triggers | jobs | verifies | release automation | secrets |
|---|---|---|---|---|---|
| ci.yml | push/PR on master,main | test (ubuntu, Go 1.22: `go build ./...`, `go vet ./...`, `go test ./... -race -count=1`); drift (regen language spec via `go run ./cmd/generate-language` + `scripts/sync-schema.sh`, then `git diff --quiet`); build-check (cross-compile linux amd64/arm64, darwin amd64/arm64, windows amd64) | build, vet, unit tests with race, generated-spec drift, cross-compile. Does NOT: run on windows/macos, run golangci-lint/staticcheck/govulncheck, compile any generated TypeScript/Python/Go/Rust/Java/C#/PHP, build the website, run editor plugin builds, run packages/* | none | none (contents: read) |
| release.yml | tag push `v*`, workflow_dispatch | resolve (path-diff vs previous tag), test (ubuntu), build matrix (5 targets, CGO off, `-trimpath -ldflags "-s -w -X main.Version"`), release (softprops/action-gh-release; archives + `checksums.txt` sha256 + changelog), publish-npm, publish-pip, update-homebrew, publish-vscode, publish-jetbrains, publish-choco (windows), publish-apt (packagecloud), publish-nuget | tests + vet before build | tags, GitHub Release, checksums.txt. No signing (cosign/GPG), no SBOM, no provenance attestation, no goreleaser. Actions pinned by tag, not SHA. No windows-arm64 or linux/386/freebsd targets | GITHUB_TOKEN, NPM_TOKEN, PYPI_TOKEN, VSCE_PAT, JETBRAINS_PUBLISH_TOKEN (+ JETBRAINS_CERT_CHAIN/PRIVATE_KEY/KEY_PASSWORD), CHOCO_API_KEY, PACKAGECLOUD_TOKEN, NUGET_API_KEY |
| scripts/sync-schema.sh | manual / drift job | copies editors/veld-config.schema.json into vscode + jetbrains | - | - | - |

Release defects (see F-CLI-15, F-CLI-18): publish-* jobs `needs: [release]`, and `release` is skipped when no Go source changed, so "packages/npm changed -> npm" auto-detection can never publish (skipped dependency). `curl -sL` without `-f` stores a 404 HTML page as the archive. update-homebrew never pushes. Composer/choco/apt/nuget absent from the "no previous tag" branch. `${{ inputs.version }}` interpolated straight into shell (needs write access to dispatch; low).

Go version: ci.yml, release.yml pin `1.22` = `go 1.22` in go.mod (match). Local toolchain 1.26 builds and passes. Grep of cmd/ and internal/ for `slices`, `maps`, `cmp`, `iter`, `min(`/`max(` builtins, `clear(`, `range <int>`, `errors.Join`, `slog`, `strings.CutPrefix`, `sync.OnceValue`, `t.Chdir`, `os.CopyFS`, `context.WithoutCancel`: none found in Go source (only JS `Math.min`). Used: `http.ServeMux` method patterns + `r.PathValue` (1.22, OK; in internal/server and the Go-backend emitter output) and `atomic.Bool` (1.19). The `go 1.22` directive makes the 1.26 compiler enforce 1.22 language features, and it compiled, so no >1.22 language features. I could not run a real 1.22 toolchain (would download one), so stdlib API use is grep-verified only. Generated Go uses `ServeMux` patterns so generated projects themselves need Go 1.22+.

Does any job compile generated output? No. Verified consequence: examples/go-svelte and examples/python-vue generate uncompilable output (F-CLI-19, F-CLI-20).

## PART E - Website and examples

- website/: Vite 7 + React 19 + react-router 7 + framer-motion + lucide-react, TypeScript 5.9, ESLint 9. Marketing landing page (Hero, Features, HowItWorks, SupportedStacks, Installation, CodeExample, Contributors, CTA) plus a 105KB hand-written DocsPage.tsx plus `LiveDemo.tsx` (1000+ lines): an in-browser "playground" with its OWN TypeScript re-implementation of the parser and generators (`parseVeld` at LiveDemo.tsx:41, generateFrontendSDK :262, generateBackendRoutes :320, ...), not the Go engine or WASM. It will drift from the real generator (F-CLI-22).
- Build (copy under scratchpad/web): `npm ci` FAILS (no package-lock.json; `.gitignore` excludes package-lock.json). `npm install` OK (49 packages, 0 vulnerabilities); `npm run build` (tsc -b + vite build) exit 0, 558 kB JS (167 kB gzip) with the >500 kB chunk warning. `npm run lint` exit 1: 2 errors `no-useless-escape` at LiveDemo.tsx:857. No CI job builds or lints it, no deploy workflow exists in the repo.
- Relationship to registry UI: separate. The registry SPA is `internal/server/handlers/web/index.html` (84KB, embedded by handlers/web.go `//go:embed web`); `internal/server/web/index.html` (43KB) is a stale, unreferenced duplicate (differs, no Go code references it) (F-CLI-21). Neither shares code with website/. Registry page loads qrcode from cdn.jsdelivr.net without SRI.
- docs/index.html: 19KB standalone dark-theme static landing page ("Veld - Contract-First API Code Generator"), inline CSS, apparently the earlier GitHub-Pages site, now duplicated by website/.

Examples (copied to scratchpad/ex; `veld validate` / `veld generate --dry-run` from each `<example>`):

| example | validate | generate --dry-run | note |
|---|---|---|---|
| csharp-kotlin | pass | pass | |
| go-svelte | pass | pass | real generate output fails `go vet/build` (undefined: mux) |
| java-angular | pass | pass | |
| microservices | FAIL exit 1 "no input file specified" (workspace config has no top-level input; `validate` does not support workspaces) | pass | |
| node-flutter | pass | pass | |
| node-react | pass | pass | contains recursively nested junk `veld/packages/@maayn/node_react/packages/@maayn/node_react/...` (committed `veld pull` artefact) and a registry block pointing at app8080.maayn.com |
| node-typescript | pass | pass | generated TS passes `tsc --strict --noEmit` |
| php-typescript | pass | pass | |
| python-vue | pass | pass | real generate output has Python SyntaxError in routes (F-CLI-20) |
| rust-swift | pass | pass | |

examples/ root also holds 8 AI-status docs (COMPLETION-REPORT, COMPLETION-SUMMARY, FINAL-SUMMARY, FILES-INVENTORY, README-NEW, START-HERE, INDEX, QUICK-REFERENCE, SETUP-ALL-EXAMPLES, COMPLETE-GUIDE) - noise.

## PART F - Tests and hygiene

`go test -cover ./...` all green (0 FAIL, ~15s). `go vet ./...`: clean, no output. 523 test functions total.

Coverage per package (statements):
cache 92.1, sdkhelpers 100, lint 84.0, react 83.4, typesonly 82.9, database 79.9, svelte 79.8, vue 79.6, errors 78.4, lang 77.6, dockerfile 76.6, config 74.5, angular 72.4, schema 70.0, setup 68.5, envconfig 66.7, kotlin 66.3, codegen 66.5, openapi 65.6, swift 64.9, dart 63.6, format 63.6, javascript(fe) 62.0, mock 61.2, lexer 60.9, typescript(fe) 59.3, csharp 59.1, java 58.2, cicd 56.4, diff 55.2, parser 53.7, validator 53.7, rust 48.5, php 47.9, go-backend 46.8, scaffold 36.0, emitter 35.4, lsp 33.6, loader 30.2, csharp/strategy 30.4, javascript(be) 29.5, python 28.5, node 19.6, cmd/veld 0.0 (tests exec a built binary, so in-process coverage is 0), ast n/a.
Tests per package (functions): setup 56, parser 52, emitter/lang 52, validator 29, config 29, lexer 22, cmd/veld 21, emitter 14, codegen 13, mock 12, java 12, lint 11, diff 10, node 10, python 10, php 10, csharp 10, go 9, javascript 9, cache 9, lsp 8, rust 8, openapi 7, loader 6, format 6, each frontend emitter 3-6, others <=5.
Packages with ZERO tests: cmd/generate-language, internal/docsgen, internal/graphqlgen, internal/language, internal/registry, internal/server and all of its subpackages (auth, db, email, handlers, models, storage), emitter/tshelpers, emitter/tsshared, and every backend `strategy` subpackage except csharp (go, java, javascript, node, php, python, rust). The registry server (JWT, passwords, TOTP, tarball upload) is entirely untested.
Flaky patterns: none found. No `time.Sleep` in tests, no real network, no `t.Parallel`; 185 `TempDir` uses; config tests use `os.Chdir` (serial, restores via defer, acceptable); cmd/veld tests build the binary once via sync.Once. No test compiles generated code (grep for tsc/py_compile/go vet in tests: only main_test and setup_test use exec).
TODO/FIXME/XXX (informational): 23 in .go files (24 across go/ts/js/py/kt, excluding node_modules).

Tracked-file hygiene (git ls-tree sizes):

| item | tracked? | size |
|---|---|---|
| `veld` (root, ELF x86-64 Linux, debug_info, not stripped) | YES, despite `/veld` in .gitignore (committed before ignore) | 20,450,930 B (~19.5 MB) |
| test-gen/ | yes (6 files) | 2,523 B |
| test-gen2/ | yes (4 files) | 1,551 B |
| test-headers/ | yes (4 files) | 2,631 B |
| test-constants/ | yes (3 files) | 1,225 B |
| test.md | yes | 30,318 B |
| .idea/ | yes (11 files) | 6,319 B |
| docs/*.docx (4) | yes | 71,804 + 61,756 + 59,182 + 53,039 = 245,781 B |
| docs/ total (22 files incl. v1-v4 duplicated plans as .md and .docx) | yes | 652,537 B |
| editors/vscode/out/ (compiled JS + map) | yes (4 files) | ~101 KB |
| editors/jetbrains/gradle/wrapper/gradle-wrapper.jar | yes | 43,764 B (normal) |
| internal/server/web/index.html (stale duplicate) | yes | 42,680 B |
| resources/logo.png | yes | 321,310 B |
| no tracked LICENSE, CHANGELOG, SECURITY, CONTRIBUTING; no .goreleaser, dependabot, Makefile | - | - |
| `.veld.lock.json`, `.veld-cache.json` exist in repo root (ignored/untracked) | no | - |

Repository: 732 tracked files, 24,845,147 B (~23.7 MiB) tracked content (HEAD), `.git` directory 36 MB, 204 commits, 47 tags.

---

## Findings

### F-CLI-01: `generate` and `watch` delete previous output and the breaking-change lock before validating; breaking-change gate (--strict/--force/prompt) is dead
- Category: output-safety
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: low
- Location: cmd/veld/generate.go:40-52, 759, 1021; cmd/veld/watch.go:56
- Trigger/reproducer: (a) generate OK, append `garbage {{{` to a .veld file, run `veld generate` -> exit 1 and `generated/`, `veld/generated`, `.veld.lock.json`, `.veld-cache.json` all gone. (b) generate OK, remove `action GetUser`, run `veld generate --strict` -> exit 0, no breaking-change report; same with --force and plain. `veld ci --strict` on the identical state exits 1 with "action GetUser removed" (ci does not call runClean).
- Expected vs actual: expected a failed/invalid generation to leave prior output intact and `--strict` to block breaking changes (documented CI gate). Actual: `runClean(rc)` (RemoveAll of every output dir plus `diff.DeleteLock`) runs at generate.go:759 before `computePreChanges` (generate.go:1021), so the lock baseline is already deleted and `HasBreaking` is always false; and any parse/validate/unknown-backend error leaves the user with an empty output dir. watch.go:56 does the same on start and so also loses lock/diff.
- Impact: CI breaking-change protection advertised in CLAUDE.md does not work for `veld generate`; any typo wipes generated code that the developer's own non-generated files import; the interactive prompt (`promptContinue`) is unreachable.
- Root cause: ordering - clean precedes parse/validate/diff; lock deletion is bundled into runClean.
- Recommendation: refactor - parse+validate+diff first, emit into a temp dir, swap on success, and stop deleting the lock in runClean.
- Regression check: test that a failing generate preserves out dir and lock; test `generate --strict` exits 1 when an action is removed.
- Effort estimate: 6h-10h

### F-CLI-02: `--out`/`out` validation accepts the directory containing the contract sources; `generate` then deletes them
- Category: output-safety
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: low
- Location: internal/config/config.go:360-377, cmd/veld/generate.go:41-48
- Trigger/reproducer: project `proj/veld/{veld.config.json,app.veld,...}`; run `veld generate --out=../veld` (in `proj/`). validateOutPath only rejects basenames `..`, `.`, `/`, `\`. `../veld` resolves to the config dir; runClean `os.RemoveAll` deleted the entire `veld/` tree (sources + config), then failed `reading ...app.veld`.
- Expected vs actual: expected refusal when the output dir contains/equals the config dir, the input file's dir, cwd, or a parent of any of them. Actual: accepted and wiped. Also accepted: `--out=C:\Users\x`, any absolute path.
- Impact: irrecoverable loss of the user's contract files from one mis-typed flag or config value; same path is hit by `veld clean` and by workspace entries (not validated at all).
- Root cause: shallow lexical basename check; RemoveAll with no containment, marker-file, or ownership check.
- Recommendation: fix - refuse any out dir that is, contains, or is contained by config dir / input dir / cwd / home / drive root, and only delete dirs carrying a `.veld-generated` marker.
- Regression check: tests for out=configDir, out=parent-of-configDir, out=inputDir, workspace entry out=".".
- Effort estimate: 3h-5h

### F-CLI-03: `veld clean` / pre-generate clean recursively delete any directory `out` points at (e.g. `src`)
- Category: clean
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: cmd/veld/generate.go:40-52, 1144-1180
- Trigger/reproducer: config `{"out":"./src"}`, `src/business.ts` present, `veld clean` -> prints "Cleaned ...\src", exit 0, `src/` is gone including the user file. No prompt, no manifest, no `--yes`.
- Expected vs actual: only files Veld generated should be removed (CLAUDE.md: "Veld never touches developer business logic"). Actual: whole directory.
- Impact: data loss for projects that mix generated and handwritten code in one folder (Veld's own docs allow `out` anywhere).
- Root cause: no generated-file manifest or marker; `os.RemoveAll(dir)`.
- Recommendation: refactor - write a manifest (or `.veld-generated` marker) and delete only manifest files (plus empty dirs); refuse to clean unmarked non-empty dirs without `--force`.
- Regression check: clean on a dir with foreign files leaves them; clean on an unmarked dir refuses.
- Effort estimate: 4h-6h

### F-CLI-04: `veld doctor` is advertised but not registered
- Category: cli
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/tooling.go:195, cmd/veld/main.go:88-101, 77
- Trigger/reproducer: `veld doctor` -> `Error: unknown command "doctor" for "veld"`, exit 1. `newDoctorCmd` is defined (and marked Hidden) but never added to the root command; help text, init-generated README (init.go:1080) and CLAUDE.md all advertise it. main_test.go:456 tolerates any exit code so the test does not catch it.
- Expected vs actual: command runs vs unknown command.
- Impact: documented diagnostic unusable.
- Root cause: missing `newDoctorCmd()` in AddCommand list.
- Recommendation: fix - register it (and un-hide) or remove the docs.
- Regression check: assert `veld doctor` exit 0 in a valid project.
- Effort estimate: 0.5h-1h

### F-CLI-05: `veld init` overwrites existing README.md and veld/*.veld files when no config exists
- Category: cli
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: cmd/veld/init.go:20-33, 651-658
- Trigger/reproducer: directory with `README.md` ("MY README") and `veld/app.veld` ("// mine") but no veld.config.json; `veld init` exit 0 -> README.md now "# My Veld Project", app.veld replaced by scaffold.
- Expected vs actual: CLAUDE.md "veld init never overwrites files"; the guard only checks for config files, then `os.WriteFile` unconditionally.
- Impact: silent loss of a project README and any pre-existing contract files.
- Root cause: existence check limited to two config paths; no per-file check.
- Recommendation: fix - skip or abort on any existing target path (O_EXCL).
- Regression check: init in dir with README.md leaves it byte-identical.
- Effort estimate: 1h-2h

### F-CLI-06: `generate --dry-run` writes `.veld-cache.json`
- Category: dry-run
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/generate.go:190-200
- Trigger/reproducer: before/after `find` of a project around `veld generate --dry-run`: new `veld/.veld-cache.json`.
- Expected vs actual: nothing written vs cache file created (lock is correctly skipped at :205/:209; cache save is not guarded by DryRun).
- Impact: breaks "no side effects" promise; a stale cache could make a later `--incremental` think files were generated.
- Root cause: missing `!opts.DryRun` guard around cache update/save.
- Recommendation: fix - guard cache save.
- Regression check: directory snapshot equality around dry-run.
- Effort estimate: 0.5h-1h

### F-CLI-07: Config precedence: flat fields override nested, contrary to documentation
- Category: config
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/config/config.go:137-150 (normalize), CLAUDE.md config section, config.go:34 comment
- Trigger/reproducer: `{"backend":"python","backendConfig":{"target":"node-ts"},"frontend":"none"}` -> Python generated.
- Expected vs actual: documented "nested takes precedence"; actual flat wins (nested only fills empty flat fields). Same pattern for framework, out, validate, postGenerate, frontend.
- Impact: users migrating to the nested format with a leftover flat key get the old target silently.
- Root cause: `&& c.Backend == ""` guards in normalize().
- Recommendation: fix - make nested win (or error on conflicting keys) and add a test.
- Regression check: unit test with both formats set.
- Effort estimate: 1h-2h

### F-CLI-08: `--out` does not override `backendConfig.out` / `backendOut`
- Category: config
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/config/config.go:312-335, 392-405
- Trigger/reproducer: config with nested `backendConfig.out: ./nestout`, run `generate --dry-run --out=./flagout` -> backend -> ./nestout, frontend -> ./flagout.
- Expected vs actual: flag beats config everywhere; actual: only unset side follows `--out`. `FlagOverrides.BackendOut/FrontendOut` exist but generate has no `--backend-out` flag.
- Impact: CI overrides of the output dir silently partially ignored.
- Root cause: flag sets only `cfg.Out`; BackendOut non-empty takes priority.
- Recommendation: fix - `--out` should clear BackendOut/FrontendOut, or add explicit flags.
- Regression check: test flag vs nested out.
- Effort estimate: 1h-2h

### F-CLI-09: Path-base inconsistency and silent unknown config keys
- Category: config
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/config/config.go:283-308, 324-327; RawConfig decoding at :296
- Trigger/reproducer: (1) with config in `veld/`, `veld generate --input=app.veld` from `veld/` changes the base for `out` from the config dir to cwd (`./veld/o` vs `./o`). (2) config containing `"bogusKey":1` or misspelt `"outt"` accepted silently. (3) Config only found in `.` or `./veld`, never in parents. (4) JSON with comments/trailing comma -> raw encoding/json error with no line number.
- Expected vs actual: single resolution base; unknown keys warned; discovery up the tree.
- Impact: surprising output locations, typos undetected.
- Root cause: `cfgDir` reassigned when `--input` set; plain Unmarshal.
- Recommendation: refactor - use DisallowUnknownFields (warn), keep config-dir base, optional walk-up.
- Regression check: tests for typo key warning and --input base.
- Effort estimate: 3h-5h

### F-CLI-10: `hooks.postGenerate` is run via hard-coded `sh -c`; failures are ignored
- Category: config
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: low
- Location: cmd/veld/generate.go:24-36
- Trigger/reproducer: hook `echo HOOK-RAN > hook.txt; exit 3` -> hook ran, `warning: postGenerate hook failed: exit status 3`, veld exit 0. On Windows without `sh` the hook cannot run at all.
- Expected vs actual: documented `postGenerate: "npm run format"` works cross-platform and a failing formatter fails the CI step; actual: needs `sh`, always exit 0.
- Impact: unformatted/unverified code ships from CI; Windows users without Git Bash get a warning only. Design consideration: config is executable code (anyone able to modify veld.config.json gets shell execution on `generate`/`ci`/`watch`, like npm scripts); should be documented, and ideally `--no-hooks`.
- Root cause: `exec.Command("sh","-c")`, error swallowed.
- Recommendation: refactor - use `cmd /C` on Windows, return a non-zero exit on hook failure (or `--strict`), add `--no-hooks`.
- Regression check: hook exit code propagated test.
- Effort estimate: 2h-3h

### F-CLI-11: Hidden commands and lack of machine-readable output / exit-code taxonomy
- Category: exit-codes
- Evidence status: design limitation
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/main.go:105, contract.go:64,601
- Trigger/reproducer: no `--json` on validate/lint/diff/generate; all failures exit 1; `fmt` exits 0 when files need formatting; `validate` mixes `os.Exit(1)` and returned errors, so output format of the two failure modes differs (`error:` vs `Error:`).
- Expected vs actual: CI-friendly JSON diagnostics and distinct codes (usage 2, validation 1, IO 3).
- Impact: tooling must scrape ANSI-colored text (colors are emitted even when stdout is not a TTY: ANSI escapes seen in redirected files).
- Root cause: none designed.
- Recommendation: refactor - add `--json`, honor NO_COLOR / non-TTY, add `fmt --check`.
- Regression check: golden JSON tests.
- Effort estimate: 6h-10h

### F-CLI-12: Error-level lint rules are unreachable
- Category: cli
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/contract.go:96-103, internal/lint/lint.go
- Trigger/reproducer: duplicate-route / duplicate-action contracts are rejected by validator first ("contract validation failed - fix errors before linting"); `lint` never reaches its own error-level rules. CLAUDE.md lists duplicate-route/duplicate-action as lint errors.
- Expected vs actual: lint reports them; actual: validator error text and exit 1 before lint.
- Impact: duplicated logic, misleading docs.
- Root cause: layering overlap.
- Recommendation: remove - drop the unreachable lint rules or document validator ownership.
- Regression check: n/a.
- Effort estimate: 0.5h-1h

### F-CLI-13: Version reporting wrong for `go install` and source builds; wrappers pinned to 0.1.0
- Category: release
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/main.go:37, packages/*/ (version 0.1.0), release.yml:304
- Trigger/reproducer: `veld --version` from plain `go build` -> `veld version 0.1.0`, while tags reach v0.3.9. Homebrew `test do` asserts the version string.
- Expected vs actual: version from tag/build info; actual: constant 0.1.0 unless ldflags used.
- Impact: bug reports and wrapper checks unreliable.
- Root cause: ldflags-only version.
- Recommendation: fix - fall back to `runtime/debug.ReadBuildInfo().Main.Version`.
- Regression check: version test.
- Effort estimate: 1h-2h

### F-CLI-14: Documentation lists top-level `veld login|logout|push|pull|serve`; they exist only under `veld registry`
- Category: cli
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/registry.go:373, CLAUDE.md "Build & Test Commands", README
- Trigger/reproducer: `veld login` / `veld serve` / `veld push` -> unknown command, exit 1.
- Expected vs actual: documented top-level commands vs only `veld registry <cmd>`.
- Impact: broken copy-paste instructions.
- Root cause: command regrouping without doc update.
- Recommendation: fix - add top-level aliases or update docs.
- Regression check: doc-command smoke test.
- Effort estimate: 1h-2h

### F-CLI-15: Release workflow: Homebrew job never publishes; package jobs skipped when only wrappers change
- Category: release
- Evidence status: source-supported defect
- Severity (user impact): high
- Security severity: none
- Location: .github/workflows/release.yml:133-170 (resolve), 392-422 (update-homebrew), publish-* `needs: [release, resolve]`
- Trigger/reproducer: read workflow. (1) update-homebrew seds `packages/homebrew/Formula/maayn-veld.rb` on the runner and exits; no commit/push/PR to a tap, so brew users never get the formula and the repo copy keeps `PLACEHOLDER_SHA256_*`. (2) `release` runs only if `publish_binary == 'true'`; publish-npm/pip/etc. need `release`, so a tag where only `packages/npm/` changed skips the release job and therefore npm (skipped dependency). (3) `curl -sL` (no `-f`) turns a 404 into an HTML "archive" and fails later at tar. (4) `publish_composer` is computed but there is no composer job. (5) Release notes advertise `brew install veld-dev/tap/veld` while formula is `maayn-veld/tap/maayn-veld`.
- Expected vs actual: documented "auto-detect and publish affected targets"; actual partial.
- Impact: brew and composer channels never ship; wrapper-only fixes cannot be released by tag.
- Root cause: job dependency design and unfinished jobs.
- Recommendation: refactor - decouple wrapper jobs from the `release` job (`if: always() && ...`), add `curl -fsSL`, push the formula to a tap repo, add composer or drop it.
- Regression check: workflow dry-run (act) or a tag on a fork.
- Effort estimate: 4h-8h

### F-CLI-16: npm and pip wrappers: no checksum, unpinned download, swallowed postinstall failure, Python 3.8/3.9 syntax, doc name mismatch
- Category: dist-npm
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: packages/npm/install.js:25-45, 141-205; packages/pip/veld/__main__.py:23,61 (`str | None`); packages/pip/pyproject.toml (requires-python >=3.8); packages/README.md
- Trigger/reproducer: npm: `VELD_VERSION=0.0.0 npm install ../pkg` -> install exit 0, `npx veld` exit 1 "binary not found". Source: install.js resolves `releases/latest` rather than the wrapper's own version, downloads over https with no checksum or signature, then extracts with tar/Expand-Archive; `go install ...@latest` fallback is also unpinned. pip: `def get_platform_key() -> str | None` is evaluated at import on Python <3.10. Docs say `npm install veld` / `pip install veld` but names are `@maayn/veld` / `maayn-veld`. No HTTP_PROXY support in install.js. `process.exit(result.status || 0)` masks signal exits.
- Expected vs actual: pinned, checksum-verified, failing install; actual otherwise.
- Impact: supply-chain exposure (a compromised release or MITM of redirect chain is run as developer code without verification); broken installs reported as success; pip wrapper unusable on 3.8/3.9 despite metadata.
- Root cause: wrapper written for convenience.
- Recommendation: fix - pin to wrapper version, verify `checksums.txt` sha256, fail the install loudly, add `from __future__ import annotations`, correct README names.
- Regression check: install tests with a local HTTP server serving a tampered archive.
- Effort estimate: 4h-6h

### F-CLI-17: Distribution metadata gaps (no root LICENSE, placeholders, Windows-arm64, Composer unpublishable as-is)
- Category: dist-composer
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: packages/chocolatey/veld.nuspec (licenseUrl), packages/chocolatey/tools/chocolateyInstall.ps1 (checksum placeholder), packages/composer/*, packages/apt/nfpm.yaml
- Trigger/reproducer: `git ls-files` shows no LICENSE; nuspec licenseUrl points to it. Windows ARM64 users map to `windows-arm64`, never built (npm, composer, nuget). Composer `post-install-cmd` of a dependency does not execute; Packagist cannot index a subdirectory composer.json; `VELD_VERSION` const is not updated by CI. nfpm installs to /usr/local/bin.
- Expected vs actual: publishable channels; actual: placeholders.
- Impact: Chocolatey moderation failure, Windows ARM unsupported, Composer channel nonfunctional.
- Root cause: unfinished packaging.
- Recommendation: fix - add LICENSE, add windows-arm64 target or explicit unsupported message, split Composer into its own repo.
- Regression check: per-channel install smoke test in CI.
- Effort estimate: 6h-10h

### F-CLI-18: CI verifies only Linux unit tests; no signing/SBOM; no generated-code compilation; nothing builds website/editors/wrappers
- Category: ci
- Evidence status: design limitation
- Severity (user impact): high
- Security severity: low
- Location: .github/workflows/ci.yml, release.yml
- Trigger/reproducer: read workflows.
- Expected vs actual: a code generator whose product is source code should compile its output for each target; actual: only `go build/vet/test` on ubuntu plus cross-compile; no windows/macos test runner (the CLI has Windows-specific path code), no govulncheck/lint, no cosign/SBOM/attestation, actions not SHA-pinned.
- Impact: regressions F-CLI-19/20 shipped unnoticed.
- Root cause: absent jobs.
- Recommendation: refactor - add matrix (ubuntu/windows/macos), a job that generates every example and compiles with tsc, py_compile, go build, cargo check, javac, dotnet build, php -l; add govulncheck, SBOM, cosign.
- Regression check: the new job fails on current examples (proves it).
- Effort estimate: 12h-20h

### F-CLI-19: Go backend (plain `net/http` strategy) generates code that does not compile
- Category: examples
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: internal/emitter/backend/go/strategy/plain.go (route registration) -> generated `internal/routes/todos.go:13-18`
- Trigger/reproducer: `examples/go-svelte` copy: `veld generate` then `cd generated && go vet ./...` -> `internal\routes\todos.go:14:2: undefined: mux` (9 uses). The function parameter is named `r *http.ServeMux` but the body calls `mux.HandleFunc`.
- Expected vs actual: compiles; actual: build failure for the default Go backend.
- Impact: Go backend unusable out of the box.
- Root cause: param name vs variable name mismatch in the plain strategy template.
- Recommendation: fix - emit consistent receiver name; add compile test.
- Regression check: go build of generated output in CI (F-CLI-18).
- Effort estimate: 1h-2h

### F-CLI-20: Python backend routes contain a SyntaxError for actions with input
- Category: examples
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: generated `routes/todos_routes.py:40`, emitter internal/emitter/backend/python
- Trigger/reproducer: `examples/python-vue` copy: `veld generate` (default config, no validate flag) then `python -I -m compileall generated` -> `input = CreateTodoInputSchema(**body = data  # CreateTodoInputSchema).model_dump()` SyntaxError (also users_routes.py:40).
- Expected vs actual: valid Python; actual: syntax error, module cannot import.
- Impact: Python backend route files unusable for any action with a body.
- Root cause: string composition bug in the Python route emitter (looks like a template arg mix-up; not root-caused here).
- Recommendation: fix - repair the template and add py_compile test.
- Regression check: compileall on generated output.
- Effort estimate: 1h-3h

### F-CLI-21: Repository hygiene: 20MB binary and scratch dirs tracked; stale duplicate registry UI
- Category: hygiene
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: `veld` (root), test-gen/, test-gen2/, test-headers/, test-constants/, test.md, .idea/, docs/*.docx, internal/server/web/index.html, examples/node-react/veld/packages/@maayn/..., examples/*SUMMARY*.md
- Trigger/reproducer: `git ls-tree -r -l HEAD` (sizes in Part F). Root `veld` is a Linux ELF with debug info, tracked although `/veld` is in .gitignore. `internal/server/web/index.html` is not embedded (handlers/web.go embeds `web` under handlers/) and differs from the live copy.
- Expected vs actual: clean repo; actual: ~20.9MB of the 23.7MB tracked content is the stray binary; `.git` is 36MB (history keeps it forever).
- Impact: slow clones, confusion about the registry UI source of truth, misleading AI-generated status docs.
- Root cause: committed build artifacts.
- Recommendation: remove - `git rm` them (history rewrite optional).
- Regression check: CI check that no executable / `test-*` paths are tracked.
- Effort estimate: 1h-2h

### F-CLI-22: Website: no lockfile (npm ci fails), lint errors, unbuilt in CI, LiveDemo duplicates the generator
- Category: website
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: website/package.json, .gitignore (package-lock.json), website/src/components/sections/LiveDemo.tsx:41,857
- Trigger/reproducer: `npm ci` -> EUSAGE (no lockfile); `npm install && npm run build` exit 0; `npm run lint` exit 1 (2x no-useless-escape at LiveDemo.tsx:857). LiveDemo contains a separate TS parser/generators.
- Expected vs actual: reproducible installs, clean lint, demo output matching the real tool.
- Impact: unreproducible dependency tree, demo drifts from real output (already shows different formatting than the Go emitters).
- Root cause: lockfile ignored; reimplementation instead of WASM.
- Recommendation: refactor - commit lockfile, fix lint, add website CI job; consider compiling the Go engine to WASM for the demo.
- Regression check: CI job `npm ci && npm run lint && npm run build`.
- Effort estimate: 2h-4h (WASM: 16h+)

### F-CLI-23: Test hygiene gaps (untested registry server, strategy subpackages, no generated-code compile tests, CLI coverage reads 0%)
- Category: tests
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: internal/server/**, internal/registry, internal/docsgen, internal/graphqlgen, internal/language, emitter/backend/*/strategy, cmd/veld
- Trigger/reproducer: `go test -cover ./...` shows no test files for the listed packages; cmd/veld reports 0.0% because tests exec a built binary. Low coverage: node backend 19.6%, python 28.5%, loader 30.2%, lsp 33.6%, emitter 35.4%, validator and parser 53.7%.
- Expected vs actual: auth/JWT/TOTP/upload code covered; actual: zero tests.
- Impact: security-critical registry code (HMAC JWT, password hashing, tarball extraction/upload, TOTP) has no regression net.
- Root cause: prioritisation.
- Recommendation: fix - add handler/auth tests with httptest and a fake storage, plus generated-code compile tests.
- Regression check: coverage gates per package.
- Effort estimate: 16h-30h
