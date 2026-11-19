# Appendix: audit-track notes (A-docs-claims)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Veld documentation-claims audit (commit 6a47f81, reviewed 2026-10-21)

Method: read docs, then verified against source and by building `veld` from HEAD into the scratchpad
(`veldbin.exe`, not the tracked `veld`) and running `veld init` + `veld generate` in a scratch project
(`scratchpad/proj`). No tracked file modified. Status words: verified / partially true / mismatch / unverifiable.
Abbreviations: CL = CLAUDE.md; OV = docs/architecture/overview.md; GP = docs/GRADUATION_PROPOSAL.md;
PP = docs/PROJECT_PROPOSAL.md; DG = docs/DEVELOPER_GUIDE.md; GS = docs/guides/getting-started.md;
T = test.md; EXEC v1..v4 = docs/EXECUTION_PLAN_v*.md. Note: docs/README.md and overview.md and changelog-v0.1.md
are short; where I cite a section name instead of a line, the file has no stable line anchor for that item.

## 0. Ground truth from the code (the yardstick)

- Registered backends (8): csharp, go, java, node-js, node-ts, php, python, rust (`internal/emitter/backend/*/main.go` `RegisterBackend`; blank imports `cmd/veld/main.go:10-17`).
- Registered frontends (10): angular, dart, javascript, kotlin, react, svelte, swift, typescript, types-only, vue (`internal/emitter/frontend/*/emitter.go`; `cmd/veld/main.go:18-27`).
- Registered tool emitters (6): cicd, database, dockerfile, env, openapi, scaffold-tests (`internal/generators/*`). They run only when passed as `--backend=<tool>` (verified: `--backend=dockerfile` wrote Dockerfile, .dockerignore, docker-compose.yml). `--backend` help lists them (generate.go:1097).
- Aliases (`internal/config/config.go:252-279`): backend node, node-typescript -> node-ts; js, javascript, node-javascript -> node-js. Frontend flutter -> dart, ts -> typescript, hooks/react-hooks -> react, js -> javascript. `"target": "node-ts"` is the canonical name and is accepted.
- Framework strategies present: node (express, plain), javascript (express, plain), python (flask, fastapi, plain), go (chi, gin, plain), rust (axum, plain), java (spring, plain), csharp (aspnet, plain), php (laravel, plain).
- Top-level commands actually registered (`cmd/veld/main.go:88-102`): init, generate, watch, clean, validate, setup, ci, lint, diff, deps, ast (hidden), fmt (hidden), openapi, graphql, schema, docs, export, registry, lsp, completion. `login/logout/push/pull/serve` exist ONLY as `veld registry <name>` (`cmd/veld/registry.go:373`). `newDoctorCmd` is defined (`cmd/veld/tooling.go:195`, Hidden) but never added to the root; `veld doctor` -> "unknown command". Binary run confirmed for login, logout, push, pull, serve, doctor.
- go.mod (`go.mod:1-13`): module `github.com/Adhamzineldin/Veld`, go 1.22; direct deps cobra v1.8.0, lib/pq v1.10.9, x/crypto v0.24.0.
- `var Version = "0.1.0"` (`cmd/veld/main.go:37`), overridden by release ldflags (`.github/workflows/release.yml:304`). 47 git tags, latest v0.3.24.
- Language spec (`internal/language/constants.go:44-100`): HTTP methods include HEAD, OPTIONS, WS; builtin types string,int,long,float,decimal,bool,date,datetime,time,uuid,bytes,json,any; special List, Map; annotations include primary, autoincrement, readonly, min, max, minLength, maxLength, example, relation.
- Empirical generation (default `veld init` project, node-ts + typescript): backend output = `index.ts, package.json, README.md, interfaces/, routes/, types/ (+_scalars.ts, index.ts), middleware/IMiddleware.ts`. NO `schemas/schemas.ts`, NO Zod (`grep -rni zod internal/emitter/backend/node` -> only a comment at routes.go:137). `--validate` instead emits `_validators.ts` (zero-dep). Frontend went to `./veld/generated/client/` (not `generated/client/`). Default route handlers do no input validation (`service.createUser(req.body)`). Python output dirs: `models/`, `interfaces/`, `routes/`, `middleware/` (no `types/`, no `schemas/`).
- Determinism test: 6 consecutive `veld generate --force` runs on identical input produced 4 distinct output hashes. Cause: `types/index.ts` export-line ordering varies (`export type { LoginInput, RegisterInput, AuthToken }` vs `{ AuthToken, LoginInput, RegisterInput }`) because `internal/emitter/tsshared/types.go:438-443` iterates `for typeName, owner := range claimed` (Go map, unsorted).

## 1. Claims inventory

| # | Claim | Source | Category | Verification | Status | Evidence |
|---|---|---|---|---|---|---|
| 1 | 8 backends: node-ts,node-js,python,go,rust,java,csharp,php | README.md:215-224; GP:34; vscode README:129 | support matrix | registry grep | verified | cmd/veld/main.go:10-17 |
| 2 | 10 frontends incl. types-only, none | README.md:239-251 | support matrix | registry grep | verified | cmd/veld/main.go:18-27 |
| 3 | 18 targets (8+10) + 6 auxiliary generators | GP:34,213 | support matrix | count | verified | registries; internal/generators/* |
| 4 | Rust backend is "Actix" | README.md:221; OV Backend table; website Features.tsx:26 | feature | read emitter | mismatch | Rust emitter is Axum: backend/rust/main.go:2, errors.go:1, strategy/axum.go; no actix |
| 5 | PHP backend is "PSR-15" | README.md:224 | feature | read emitter | mismatch | Laravel: backend/php/main.go:18, strategy/laravel.go |
| 6 | Node backend emits Zod schemas `schemas/schemas.ts`; Zod validation in routes; ZodError->400 | CL:9,81,444-446,467-468; README.md:32,272,292; OV; changelog-v0.1 items 1,6; GS:63-68; T:849,928-950; GP:694; website DocsPage.tsx:1608,1758 | feature | generated + inspected | mismatch | no zod in node emitter; schemas/ not produced; default routes pass req.body through; `--validate` -> `_validators.ts` |
| 7 | Python backend emits `types/` and `schemas/schemas.py` (Pydantic) | CL:456-460; T:854; changelog items 8-9 | feature | generated + inspected | mismatch | python output has models/ interfaces/ routes/ middleware/; python/routes.go:55 imports `..schemas.schemas` which nothing writes (GP C3) |
| 8 | Zero runtime dependencies in generated output (by default) | CL:9; README.md:37,417; GP App.B | quality | inspected node-ts (+ --validate) | partially true | TS/Node path has no third-party imports; other stacks emit chi/gin/Spring/ASP.NET/Laravel/Axum/Flask/Pydantic unless `plain` strategy |
| 9 | Framework agnostic, `router: any` | CL:11; README.md:413,418 | quality | read generated routes | verified (node-ts) | `usersRouter(router: any, service)` in generated routes |
| 10 | Deterministic output | CL:508; README.md:419; OV Key Rules 4; GP 14.3; T:11 | quality | 6 runs, hash compare | mismatch | 4 hashes/6 runs; tsshared/types.go:438-443 |
| 11 | Veld never writes outside `--out` | CL Hard Rules; OV Key Rules 5; T:840 | quality | read code | partially true | setup edits tsconfig/package.json (internal/setup/setup.go:362-515); lock beside config (generate.go:209); `hooks.postGenerate` runs shell (generate.go:24-28); frontend default dir is `veld/generated` not `--out` |
| 12 | `go install github.com/Adhamzineldin/Veld/cmd/veld@latest` | README.md:56 | install | go.mod | verified | go.mod:1 |
| 13 | Go module `github.com/veld-dev/veld` | OV "Go Module" | architecture | go.mod | mismatch | go.mod:1 |
| 14 | "Only external dependency: cobra" | OV "Go Module" | architecture | go.mod | mismatch | go.mod:5-8 (+lib/pq, x/crypto) |
| 15 | Deps are cobra, x/crypto, modernc.org/sqlite | DG:87-90 | architecture | go.mod | mismatch | no sqlite; CL says no SQLite |
| 16 | Three direct third-party deps | GP:354, App.B | architecture | go.mod | verified | go.mod:5-8 |
| 17 | Top-level `veld login/logout/push/pull/serve/doctor` | CL:52-58,392-399; GP 14.1; DG:381 | feature | built binary | mismatch | registry.go:373; doctor unregistered (tooling.go:195, main.go:88-102) |
| 18 | `veld ast`, `veld fmt` exist | CL, README.md:336 | feature | binary | verified (hidden from `--help`) | contract.go:64,601 |
| 19 | README CLI table and generate flags (--dry-run, --incremental, --strict, --validate, --setup, --force) | README.md:318-354 | feature | binary | verified | main.go:88-102; generate.go:1097-1119 |
| 20 | Config fields backendConfig/frontendConfig/hooks/workspace/aliases/baseUrl | CL:176-260 | feature | config.go | verified | config.go:22-80 |
| 21 | `tools: {...}` config toggles auxiliary generators | CL:199-202,238 | feature | grep | mismatch | `ToolsConfig` parsed (config.go:44-55,76) never read anywhere in cmd/ internal/ |
| 22 | `backendConfig.target` "node-ts" accepted; `--backend=node` accepted | CL:14,182 | feature | config.go | verified | config.go:252-259 |
| 23 | "7+ backend languages and 4+ frontend targets" | T:17; T:1210-1235 tables (7 backends, 4 frontends) | support matrix | count | partially true (understated; table omits node-js, 6 frontends) | registries |
| 24 | "13+ target stacks" vs "8+ backend, 10+ frontend" | website Hero.tsx:24; DocsPage.tsx:236 | support matrix | count | verified but inconsistent with each other and with 18 | |
| 25 | "~14 languages/frameworks" | PP:5 | support matrix | count | mismatch vs GP:34 (18) | |
| 26 | `react` is alias of `typescript` frontend | T:1229 | support matrix | alias table | mismatch | react/emitter.go:18 own emitter |
| 27 | Only TypeScript frontend; no node-js backend | OV Frontend/Backend tables | support matrix | registry | mismatch | 10 frontends, 8 backends |
| 28 | GitLab CI generation | README.md:35,233; website SupportedStacks.tsx:33 | feature | `grep -rli gitlab internal cmd` | mismatch | zero hits; cicd emits GitHub Actions only |
| 29 | Supported types list | README.md:183-194 | feature | language spec | partially true | omits decimal,long,time,bytes,json,any (constants.go:49-62) |
| 30 | HTTP methods `GET POST PUT DELETE PATCH` | CL:390 | feature | language spec | partially true | + HEAD, OPTIONS, WS (constants.go:44-47) |
| 31 | Service SDKs exist for all 8 backends | CL table; GP 10.2 | feature | ls | verified | backend/*/sdk.go (8 files); user guide shows only TS/Python/Go |
| 32 | Rust/Java/C#/PHP SDK emitters not done | roadmap.md:33-36; service-sdk-plan.md:665-668 | timeline | ls | mismatch (stale) | sdk.go present |
| 33 | SDK class `IAMClient` | CL:315 | feature | helper | mismatch | sdkhelpers.ServiceClassName -> "IamClient"; guide service-sdk.md:39 uses IamClient |
| 34 | SDK file `sdk/iam_client.py`; emitters in `internal/emitter/servicesdk/<lang>/` | service-sdk-plan.md:14,173-176,703-706 | architecture | ls | mismatch | dir absent; CL:320-324 says `sdk/iam/client.py` |
| 35 | Env var `VELD_<NAME>_URL` | CL:316-318 | feature | helper | verified | sdkhelpers EnvVarName |
| 36 | main.go ~4,600 lines / 33 commands | DG:30,373-377; EXEC v4 4.2,8.1 | architecture | wc -l | mismatch (stale) | cmd/veld/main.go is 109 lines; split 2026-08-27 |
| 37 | `internal/openapigen` | DG:~47; PP:70; GP 12.3 | architecture | ls | mismatch | only internal/generators/openapi (CL says single impl) |
| 38 | `internal/server` has zero tests | PP:39; EXEC v4 7.1 | quality | find | verified | no _test.go; now 2,598 LOC (EXEC says 1,699) |
| 39 | 52 test files; ~56,000 Go lines; 22 internal pkgs; "261 source files" | GP:34 | architecture | wc | partially true | 268 tracked .go (216 non-test + 52 test); 56,066 lines; 22 pkgs |
| 40 | mock pkg unwired; internal/errors unused; `--quiet` unused; ToolsConfig unused | GP 11.1-11.2 | feature | grep | verified (still true) | no imports of internal/mock or internal/errors outside self; main.go:41,87 |
| 41 | VS Code palette commands Validate/Generate/DryRun | vscode README:43-46,116-120; EXEC v4 8.3 | feature | grep | mismatch | 0 `registerCommand` in editors/vscode/src/extension.ts |
| 42 | Install names: npm `@maayn/veld`, pip `maayn-veld`, brew `maayn-veld/tap/maayn-veld` | README.md:46-53 | install | manifests | partially true | match packages/npm/package.json:2, pip pyproject; publication to registries unverifiable; other docs give different names (D-15) |
| 43 | `npm install @maayn/veld@0.2.0`, `pip install maayn-veld==0.2.0` | packages/npm/README.md:37; pip README:35 | install | manifests | mismatch | manifests are 0.1.0 (rewritten at release) |
| 44 | `veld --version` -> `veld v0.1.0`; `-v, --version`; 11 commands in help | T:30-53,603-617 | feature | binary | mismatch | prints `veld version 0.1.0`; `-v`=verbose (main.go:86); help lists 18 commands |
| 45 | VS Code publisher `veld-dev`; `.github/workflows/publish-plugins.yml` | docs/ci-cd/PLUGINS_CI_CD_SUMMARY.md | architecture | package.json:6; ls | mismatch | publisher `adhamzineldin`; only ci.yml, release.yml |
| 46 | CI: build + vet + race tests + 5-platform cross-compile | PP:31; GP 13.1; ci-cd/guide.md:136 | quality | read ci.yml | verified | ci.yml matrix linux/darwin x amd64/arm64 + windows amd64; ci.yml:52-62 also checks generated language files |
| 47 | "MIT" license | README.md:538-540 | quality | ls | mismatch | no root LICENSE file (section 4) |
| 48 | Contributors: Adham Zineldin + "Eyad Gamal" | website Contributors.tsx:7-23 | team | git shortlog | mismatch | authors: Adham Zineldin (~187 across 3 identities), mohamed.el-kot (13), Ayman Elfeky (3), Mohamed Elkot (1) |
| 49 | Dev span 27 Feb - 19 Apr 2026, 188 commits | GP:664 | timeline | git log | partially true | start true; HEAD=204 commits, to 2026-09-12 (Feb 17, Mar 86, Apr 85, May 1, Aug 8, Sep 7) |
| 50 | Go emitter "Next (In Planning)", updated Apr 5 2026; refs plan.md etc. | roadmap.md:3-4,11,415,470,512-516 | timeline | ls | mismatch | all backends exist; plan.md, PHASE_1_SETUP.md, IMPLEMENTATION_ROADMAP.md, testapp/ absent |
| 51 | docs/README links `../test/README.md` | docs/README.md:37 | architecture | ls | mismatch | no test/ dir (packages/README.md does exist) |
| 52 | "Single types/types.ts, no per-module files" | changelog-v0.1.md item 2 | feature | generated | mismatch (superseded) | per-module types/*.ts + barrel (agrees with CL:513) |
| 53 | "Nine worked examples" | DG:56 | quality | ls | mismatch | 10 dirs under examples/ (GP says ten) |
| 54 | Getting-started code: `registerUsersRoutes`, `List/GetById`, `api.auth.Login`, schemas/, client/ under generated/ | GS:61-95 | feature | generated | mismatch | export is `usersRouter`; init prints `usersRoutes`; methods `listUsers`; SDK `api.Users.listUsers()` (README.md:304); no schemas/ |
| 55 | Default frontend is typescript; default out `./generated` | README.md:379-380 | feature | config.go:346-372 | partially true | frontend defaults to typescript only for node-ts/node-js, else `none`; out default verified |
| 56 | Lint has 8 rules | CL; DG:431-433; GP:10.4 | feature | not re-derived | unverifiable | internal/lint/lint.go |
| 57 | Built-in aliases models,modules,types,enums,schemas,services,lib,common,shared | README.md:402; OV | feature | not diffed | unverifiable | config.go DefaultAliases |
| 58 | FastAPI selectable via `--backend-framework` | GP 11.2 states no routes registered | feature | not run | unverifiable | python/strategy/fastapi.go exists; help text omits fastapi |
| 59 | .vsix 0.1.0 checked in | EXEC v4 8.3 | architecture | git ls-files | mismatch (stale) | no .vsix tracked; `editors/vscode/out/*` is tracked |
| 60 | Go toolchain missing on dev machine (2026-08-26) | EXEC v4 4.1 | architecture | n/a | unverifiable | machine-specific |
| 61 | Compatibility matrix / version detection / `veld doctor` | EXEC v4 5.x,12 | feature | grep | mismatch (planned, undelivered) | no such command or matrix |
| 62 | `docs/index.html`: 8 backend languages, 10 frontend targets | docs/index.html | support matrix | registries | verified | |

## 2. Discrepancies (reader-facing)

Severity: MISLEADING = reader would form a wrong picture of what ships; MINOR = stale or cosmetic.

| ID | Claim | Source | Reality | Evidence | Severity |
|---|---|---|---|---|---|
| D-01 | Node backend generates Zod schemas (`schemas/schemas.ts`) and validates input with Zod, ZodError->400 | CL:9,81,444-446,467-468; README.md:32,272,292; OV Node row; changelog-v0.1 items 1,6; GS:63-68; T:849,928-950; GP:694 (demo); website DocsPage.tsx:1608,1758 | No Zod is emitted at all; default routes do no validation; opt-in `--validate` emits zero-dep `_validators.ts` | scratch output tree; `grep -rni zod internal/emitter/backend/node` | MISLEADING |
| D-02 | Output is deterministic (non-negotiable / evaluation criterion) | CL:508; README.md:419; GP 14.3 | Not deterministic: `types/index.ts` export order varies per run | 4 hashes in 6 runs; internal/emitter/tsshared/types.go:438-443 | MISLEADING |
| D-03 | `veld login/logout/push/pull/serve/doctor` are top-level commands | CL:52-58,392-399; GP 14.1; DG:381 | Only `veld registry <cmd>`; `doctor` unregistered, does not run | cmd/veld/main.go:88-102; registry.go:373; tooling.go:195-200 | MISLEADING |
| D-04 | `tools` config block toggles auxiliary generators | CL:199-202,238 | Parsed, never read; generators run only via `--backend=<tool>` | config.go:44-55,76 | MISLEADING |
| D-05 | GitLab CI generated | README.md:35,233; website SupportedStacks.tsx:33 | Only GitHub Actions | `grep -rli gitlab internal cmd` -> none | MISLEADING |
| D-06 | Rust = Actix; PHP = PSR-15 | README.md:221,224; OV; website Features.tsx:26 | Rust = Axum, PHP = Laravel (plus plain) | backend/rust/strategy/axum.go; backend/php/strategy/laravel.go | MISLEADING |
| D-07 | Module path `github.com/veld-dev/veld`; "only dependency cobra"; SQLite dep | OV "Go Module"; DG:87-90 | module `github.com/Adhamzineldin/Veld`; cobra + lib/pq + x/crypto; no sqlite | go.mod:1-13 | MISLEADING |
| D-08 | Python output has `types/` + `schemas/schemas.py` | CL:456-460; T:854; changelog | Layout is models/ interfaces/ routes/ middleware/; routes import a schemas module nothing writes | scratch output; python/routes.go:55 | MISLEADING |
| D-09 | VS Code palette commands work | vscode README:43-46,116-120 | none registered | extension.ts: 0 registerCommand | MISLEADING |
| D-10 | MIT license | README.md:538-540 | No root LICENSE; chocolatey nuspec links to missing file | `ls LICENSE*`; packages/chocolatey/veld.nuspec:9 | MISLEADING |
| D-11 | Install names | README.md:46-53 vs packages/README.md:11-12 (`npm install veld`, `pip install veld`), composer README:56 (`brew install veld`), vscode README:66 (`adhamzineldin/tap/veld`); npm/pip README `@0.2.0` vs manifest 0.1.0 | five variants for brew/npm/pip | packages/*/README.md; package.json:3 | MISLEADING |
| D-12 | Getting-started snippets | GS:61-95 | wrong function/method names, wrong output tree | generated output | MISLEADING |
| D-13 | "Zero runtime deps / no lock-in" universal | README.md:37,417; CL:9 | true for TS/Node path; other stacks import a framework unless `plain` strategy chosen | strategy dirs | MINOR |
| D-14 | Never writes outside `--out` | CL; OV; T:840 | setup/hook/lock/cache/frontend default dir write elsewhere | setup.go:362-515; generate.go:24,209 | MINOR |
| D-15 | Overview lists 1 frontend, 7 backends (no node-js) | OV | 10 frontends, 8 backends | registries | MINOR |
| D-16 | Hosted-docs scrape `test.md` stale: 7 backends/4 frontends, react = typescript alias, `-v` = version, 11 commands | T:17,38-53,603-617,1210-1235 | stale vs binary | binary run | MINOR (but tracked at repo root) |
| D-17 | Developer guide says main.go is ~4,600 lines, `internal/openapigen` exists, nine examples, sqlite | DG:30,47,56,87,373 | main.go 109 lines (split 2026-08-27, before the 2026-09-12 guide commit); no openapigen; 10 examples | cmd/veld/*.go; ls examples | MINOR |
| D-18 | Roadmap/service-sdk-plan show done work as TODO; cite nonexistent files | roadmap.md:33-36,470,512-516; service-sdk-plan.md:3,665-668 | SDKs exist for all 8; plan.md etc. absent | backend/*/sdk.go | MINOR |
| D-19 | SDK class `IAMClient` | CL:315 | `IamClient` | sdkhelpers | MINOR |
| D-20 | PLUGINS_CI_CD_SUMMARY: publish-plugins.yml, publisher veld-dev | that file | neither | .github/workflows; package.json:6 | MINOR |
| D-21 | "261 source files" | GP:34,284 | 216 non-test + 52 test = 268 tracked .go | git ls-files | MINOR |
| D-22 | "~14 languages/frameworks" | PP:5 | 18 targets (+6 tools) | registries | MINOR |
| D-23 | Dev span ended 19 Apr 2026, 188 commits | GP:664 | 204 commits; work to 2026-09-12 | git log | MINOR |
| D-24 | Types/HTTP-method lists incomplete | README.md:183-194; CL:390 | extra types/methods exist | language/constants.go:44-62 | MINOR |
| D-25 | Website team list | Contributors.tsx | author "Eyad Gamal" not in git history; 3 other committers not listed | git shortlog | MINOR |
| D-26 | EXEC v4 snapshot numbers (server 1,699 lines; main.go 4,610; .vsix tracked) | EXEC v4 4.2,7.1,8.1,8.3 | server 2,598 LOC; main.go 109; no .vsix | wc; git ls-files | MINOR |

## 3. Historical plans summary (do NOT inherit silently)

Git context: first commit 2026-02-27; HEAD 6a47f81 (2026-09-12); 204 commits; per month Feb 17, Mar 86, Apr 85, May 1, Aug 8, Sep 7 (gap May-Aug). Authors: Adham Zineldin (~187), mohamed.el-kot (13), Ayman Elfeky (3), Mohamed Elkot (1).

### docs/roadmap.md (Last Updated April 5, 2026; "Version 1.0"; 536 lines)
- Scope: Go, Rust, Java, C#, PHP backends (5 new), VS Code + IntelliJ plugins, npm/pip/Homebrew wrappers, service SDKs (roadmap.md:11).
- Dates: Phase 1 done "Feb 28, 2026" (estimated 2-3 weeks, finished same day, "ahead of schedule"); Phase 2 Go Mar 1-28 "Next (In Planning)"; Phase 3 backends Mar 29-May 17; Phase 4 plugins May 18-Jun 29; Phase 5 packages Jun 30-Jul 18; "~16 weeks remaining" (roadmap.md:467-476).
- Estimates: 2 weeks each for Rust/Java/C#/PHP; VS Code 2w; IntelliJ 3w. Tests claimed: 28 Go-adapter + 13 codegen = 41 (roadmap.md:133-164).
- Service SDK section: TS, Python, Go checked; Rust, Java, C#, PHP unchecked (roadmap.md:19-36). Reality: all done.
- No team or names. Cites nonexistent plan.md, PHASE_1_SETUP.md, IMPLEMENTATION_ROADMAP.md, testapp/.
- Self-contradiction: dated April 5 but lists March items as "next"; "Zero breaking changes to TypeScript/Python emitters" (only two emitters recognised).

### docs/architecture/service-sdk-plan.md (Apr 5, 2026; "Status: Planned"; "Author: Auto-generated"; 779 lines)
- Plans TS/Python/Go first (weeks 2-5), Rust/Java/C#/PHP "Phase 5" weeks 5-7, CLI weeks 7-8, docs weeks 8-9 (lines 634-690): about 9 weeks.
- Paths/names differ from implementation (`internal/emitter/servicesdk/...`, `sdk/iam_client.py`, `ServiceSdkEmitter`); implemented as `BackendEmitter.EmitServiceSdk` and `backend/<lang>/sdk.go`.

### docs/changelog-v0.1.md (77 lines)
- "All 16 features" done; no dates/names. Contains superseded claims (single types.ts; Zod wiring that is not in current code). "Future" list (Go, Rust, Java/Kotlin, C#, PHP, plugins, npm/pip/brew) was all later delivered. Uses module `veld-dev/veld`.

### docs/PROJECT_PROPOSAL.md (98 lines, undated, committed 2026-09-12)
- Dual goal: capstone + real product. "~14 languages/frameworks" (conflicts with 18).
- Phases A registry tests + versioned migrations (golang-migrate/goose); B CONTRIBUTING.md + examples cleanup; C backfill tests, fix imports.go:79; D commercial framing (hosting, licensing, security review). No dates or weeks.
- Team: unspecified; "1-2 people" Phase A, "1 person" Phase B, "remaining team" Phase C.
- Facts to re-check: no server tests (still true), no CONTRIBUTING (still true), examples clutter (still true), `internal/openapigen` (false).

### docs/GRADUATION_PROPOSAL.md (799 lines, graduation-committee framing)
- Numbers: Go 1.22; ~56,000 LOC; "261 files" (actual 268); 22 packages; 52 test files; 8+10=18 targets + 6 aux; 10 examples; 5-platform CI.
- Timeline: development 27 Feb-19 Apr 2026, 188 commits; Phase 8 Hardening "current" (GP:664-677). Reality: 204 commits to Sep 2026.
- Defect register C1-C11 critical (Rust Axum doesn't compile; Go plain `mux`; Python schemas module never written; `fmt --write` deletes `prefix:`; registry partial-JWT 2FA bypass, private-package enumeration, path traversal on publish, token expiry unenforced; PHP HEAD; Swift `${id}`; C# scaffold) + high/medium/low. I re-confirmed only: Python schemas gap, server has no tests, unwired mock/errors/quiet/ToolsConfig. Others not re-verified; they may have been fixed or not.
- Claims contradicted by code: demo says "Zod schemas, Express wiring" (GP:694); lists `veld doctor/login/push/pull/serve` as commands (GP 14.1).
- Team: unnamed; audience "a committee".

### docs/EXECUTION_PLAN_v1.md (17 weeks = 13 core + 4 buffer; 5 members; 512 lines)
- Role split: M1 core/CLI/tooling; M2 7 non-Rust backends + generators + 2 new backends (Ruby, Kotlin); M3 frontends + registry + production + 3 new frontends (SolidJS, Python client, Go client); M4 Rust only (+Actix strategy, Rust client); M5 docs only.
- Phases: P0 W1, P1 W2-4 (C1-C11), P2 W5-8, P3 W9-12, P4 W13, buffer W14-17.
- Emitter counts: "8 new targets" stated but table lists 7 items; ends at 10 backends / 14 frontends. No names, no calendar dates.

### docs/EXECUTION_PLAN_v2.md (17 weeks; 5 members; 578 lines)
- Same duration; re-split by type-system family (M1 TS/web, M2 dynamic/systems, M3 JVM/.NET/mobile, M4 Rust, M5 docs). Same new targets (Ruby, Kotlin backend, SolidJS, Python/Go/Rust clients); 10/14 target.

### docs/EXECUTION_PLAN_v3.md (16 weeks = 12 core + 4 buffer; 5 members; 998 lines)
- Principle P1 NO new emitters/features: all seven v2 additions cancelled. Tier 1 backends node-ts, python, go, java + rust (Rust = Member 4 only); node-js, csharp, php "frozen, not deleted" (Tier 2).
- Six two-week sprints S1-S6 + buffer B1-B2; registry programme S5; plugin programme S6. Counts: stays at 8 backends / 10 frontends.

### docs/EXECUTION_PLAN_v4.md == docs/EXECUTION_PLAN.md (diff shows byte-identical; "Version 4", current)
- 16 weeks, 6 phases, 16 weekly checkpoints, 5 members (4 implementing, 1 docs). P1 stabilisation W1-2; P2 version/compat matrix W3-4; P3 adapters W5-10 (5 backends W5-7: node-ts, python, go, java, rust; 5 frontends W8-10: typescript, react, vue, angular, svelte); P4 registry W11-12; P5 plugins/CLI W13-14; P6 hardening/release W15-16.
- Stale facts: Go "not installed" (2026-08-26), main.go 4,610 lines/33 commands, server 1,699 lines, .vsix tracked. Only calendar date is 2026-08-26; weeks are not anchored to a start date.
- Wording "Confirmed by you" indicates an AI-drafted plan for a requester. A1-A4 are open assumptions (java vs csharp; Rust exclusivity; 16 weeks; weekly vs two-week tasks).
- Also tracked as Word copies: docs/Veld_Execution_Plan_v1..v4.docx.

Cross-document conflicts to carry forward carefully
- Target counts: PP "~14"; GP/code 18 (8+10); EXEC v1/v2 plan 24 (10+14); EXEC v3/v4 keep 8+10 but only 5+5 "supported"; website "13+".
- Team: GP unnamed; plans "5 members" with M1-M5; website 2 named; git 4 human authors.
- Timeline: GP says dev ended 19 Apr 2026; roadmap (Apr 5) shows Go as next; v4 written Aug-Sep 2026 with an unanchored 16-week schedule.
- EXECUTION_PLAN.md (unversioned) is a duplicate of v4.

## 4. Open-source hygiene

- Root LICENSE: ABSENT (not in `git ls-files`; `ls LICENSE*` fails). README.md:538-540 says "MIT". Chocolatey nuspec (`packages/chocolatey/veld.nuspec:9`) links to `/blob/master/LICENSE`.
- Declared licenses: packages/npm/package.json:22 MIT; packages/composer/composer.json:5 MIT; packages/pip/pyproject.toml:10 MIT; packages/nuget/VeldCli.csproj:20 MIT; editors/vscode/package.json:7 MIT; website/package.json: none (private, v0.0.0); editors/jetbrains/build.gradle: no license field (but editors/jetbrains/LICENSE exists). LICENSE files exist in editors/vscode, editors/jetbrains, packages/npm, packages/pip; the latter two say "Copyright (c) 2024 Adhamzineldin" (repo's first commit is 2026-02-27).
- ABSENT: CONTRIBUTING.md (PP:41,62 acknowledges), CODE_OF_CONDUCT.md, SECURITY.md, CHANGELOG (only docs/changelog-v0.1.md), CODEOWNERS (v2 plan says it will create one), .github/ISSUE_TEMPLATE, PULL_REQUEST_TEMPLATE. `.github/` holds PLUGINS_SETUP.md, VSCODE_PUBLISHING_TROUBLESHOOTING.md, workflows/ci.yml, workflows/release.yml.
- Tracked at HEAD but should not be (`git ls-tree -r -l HEAD`; 732 files, 24,845,147 bytes total, `.git` 36 MB):
  - `veld` at repo root: ELF binary, 20,450,930 bytes, blob 0c2abda, added 2026-05-09 in caabde3, still tracked despite `.gitignore` `/veld` and the 2026-08-27 commit "remove committed build artifacts". About 82% of tracked bytes.
  - `test-gen/` 6 files 2,523 B; `test-gen2/` 4 files 1,551 B; `test-headers/` 4 files 2,631 B; `test-constants/` 3 files 1,225 B (DG calls them throwaway).
  - `test.md` 30,318 B (stale scrape of hosted docs).
  - `.idea/` 11 files 6,319 B (.gitignore:62 deliberately keeps it; contains personal plugin state such as copilot, csv-editor, material theme).
  - Also: `.claude/settings.local.json`; root `.veld.lock.json` and `.veld-cache.json`; `editors/vscode/out/*` compiled JS/sourcemaps; 4 `docs/Veld_Execution_Plan_v*.docx`; 11 meta-docs at examples/ root (COMPLETION-REPORT.md, FINAL-SUMMARY.md, START-HERE.md, ...).
- 10 largest tracked blobs (bytes): 1 veld 20,450,930; 2 resources/logo.png 321,310; 3 resources/favicon.ico 125,619; 4 website/src/pages/DocsPage.tsx 105,129; 5 internal/server/handlers/web/index.html 84,102; 6 editors/vscode/src/extension.ts 80,368; 7 docs/Veld_Execution_Plan_v3.docx 71,804; 8 docs/GRADUATION_PROPOSAL.md 66,631; 9 docs/Veld_Execution_Plan_v2.docx 61,756; 10 docs/Veld_Execution_Plan_v4.docx 59,182.
- Versioning: 47 tags, latest v0.3.24; source default `0.1.0`, package manifests 0.1.0, plugin READMEs 0.2.0. Release workflow rewrites versions at tag time (release.yml:304,679-680), so the working tree never shows the released version.

## 5. Things the code has that the docs do not mention

- Commands: `veld ci` (registry.go:433), `veld export`, `veld agents`, `veld registry info|list|versions|token create|init`; `veld deps` is only in CLAUDE.md/roadmap, not README's CLI table; `ast`/`fmt` are hidden from `--help`.
- Framework strategies: Gin (go) and FastAPI (python) and `plain` strategies for every backend; `--backend-framework` help omits gin/fastapi (generate.go:1113).
- Language: types long, time, bytes, json, any, decimal; methods HEAD, OPTIONS; annotations primary, autoincrement, readonly, min, max, minLength, maxLength; `@serverSet` (DG:119; GP 12.2); unions and constants (GP:317); `emit` directive.
- Frontend: `VeldWebSocket<TReceive,TSend>` (GP 10.2); `--frontend-framework`; `--server-sdk` (config `serverSdk`) distinct from `--service-sdk`.
- Generated extras: `middleware/IMiddleware.ts`, `types/_scalars.ts` branded scalar types, generated `README.md`.
- Packaging channels beyond README: chocolatey, apt (nfpm), nuget (`Maayn.Veld`).
- Other repo content: `docs/index.html` landing page, `registry.config.example.json`, `veld-language.json`, `scripts/`.
