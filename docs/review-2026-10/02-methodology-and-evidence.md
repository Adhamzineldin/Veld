# 02. Review methodology, revision, environment, evidence and limitations

## 2.1 Revision reviewed

| Item | Value |
|---|---|
| Repository | `D:\University\Veld` (local working copy), module `github.com/Adhamzineldin/Veld` |
| Commit | `6a47f815f37ea4c286a6e765312df76743fa7133` ("docs: add project proposal, developer guide and execution plans", 2026-09-12 01:13 +0300) |
| Branch | `master` |
| Working tree | clean except one untracked file, `docs/FABLE_PROJECT_REVIEW_PROMPT.md` (the review brief) |
| Review date | 2026-10-21 (planning baseline) |
| Reviewer | Claude Fable 5.1 acting as lead reviewer, with seven delegated audit tracks (Claude Sonnet subagents) whose raw notes are referenced in chapter 05 |

## 2.2 Environment and toolchains

| Tool | Version / status | Used for |
|---|---|---|
| Go | go1.26.0 windows/amd64 | build, vet, tests, generated Go compile |
| Node.js / npm | v22.20.0 / 10.9.3 | TypeScript compile of generated output, VS Code extension build, website build |
| Python | 3.13.7 | generated Python compile/import, Flask round trip, LSP driver script |
| Java | 25.0.1 (javac only; no Maven/Gradle verified) | static inspection only |
| Docker | 29.7.2 | disposable PostgreSQL for the registry end-to-end trace |
| PostgreSQL client (`psql`) | not installed | registry inspected via the server API and Docker |
| Rust (`cargo`), .NET SDK, PHP | not installed | Rust, C#, PHP outputs were inspected statically only (environment limitation, not a defect) |
| OS | Windows 11 Pro 10.0.26100, Git Bash and PowerShell | all commands |
| Web access | `WebSearch` available; `WebFetch` blocked by a PreToolUse hook in the session | external facts rest on search-result summaries and are marked provisional |

## 2.3 Baseline checks (run by the lead reviewer, exact commands)

```
go build ./...            # exit 0
go vet ./...              # exit 0
go test ./...             # exit 0, all packages ok; 20 packages have no test files
go run ./cmd/generate-language && git status --short editors veld-language.json   # no drift in tracked generated files
go test -cover ./...      # per-package coverage recorded in chapter 04
```

Size metrics, measured on tracked files only (`git ls-files`), excluding the 20 MB tracked binary, `website/`, `editors/` and `packages/`:

| Area | Non-test Go lines | Test Go lines |
|---|---|---|
| `internal/emitter` (all emitters + helpers) | 23,626 | 5,648 |
| `cmd/veld` | 4,729 | 534 |
| `internal/server` (registry server) | 2,598 | 0 |
| `internal/docsgen` | 1,864 | 0 |
| `internal/generators` | 1,568 | 643 |
| `internal/setup` | 1,226 | 1,071 |
| `internal/parser` | 1,154 | 1,129 |
| `internal/lsp` | 1,017 | 134 |
| `internal/language` | 838 | 0 |
| `internal/validator` | 642 | 472 |
| `internal/config` | 556 | 804 |
| `internal/diff` | 503 | 163 |
| `internal/registry` (client, credentials, tarball) | 495 | 0 |
| `internal/lexer` | 451 | 383 |
| `internal/loader` | 398 | 144 |
| all tracked `.go` | 43,886 | 12,180 |

Method: `git ls-files '<dir>/*.go' | grep -v _test.go | xargs cat | wc -l` and the `_test.go` complement. Generated files in the tree (`editors/vscode/src/veld-language-spec.ts`, `editors/jetbrains/.../VeldLanguageSpec.kt`) are not Go and are excluded. There is no vendored dependency directory.

## 2.4 Audit tracks

Seven parallel tracks were delegated, each with a written brief, a disposable working directory under the session scratchpad, and an instruction not to modify tracked files, not to publish, and not to contact any public registry. Each produced a findings file with the fixed finding format used in chapter 05.

| Track | Scope | Primary method |
|---|---|---|
| Documentation claims | README, CLAUDE.md, docs/*, test.md, editors and packages READMEs | claim inventory checked against source and the emitter registry |
| Compiler frontend | lexer, parser, ast, loader, validator, language, errors, format, lint, diff, cache | source reading plus 25+ adversarial contracts run through the built CLI |
| Node/TypeScript emitters | node backend, typescript/react/vue/angular/svelte/javascript/typesonly frontends, generate/clean/watch/dry-run | generate, `tsc --noEmit`, Express server plus generated SDK round trip, determinism and output-safety experiments |
| Other emitters and generators | python, go, rust, java, csharp, php, javascript backends; dart, kotlin, swift frontends; service SDKs; openapi, graphql, schema, docs, mock, dockerfile, cicd, envconfig | Python and Go outputs compiled and run; others static; OpenAPI validated with an external linter |
| Registry and archives | internal/registry, cmd/veld/registry.go, serve.go, internal/server/** | line-by-line archive review, crafted malicious archives against the real extraction code, authorization matrix, live server on disposable PostgreSQL |
| LSP and editors | internal/lsp, editors/vscode, editors/jetbrains, cmd/generate-language | scripted JSON-RPC session over stdio, extension build in a copy, metadata drift check |
| CLI, config, distribution, CI, website, hygiene | cmd/veld, internal/config, packages/*, .github/workflows, scripts, website, examples, test metrics | behaviour runs with exit codes, config precedence experiments, wrapper reading, website build in a copy |

The lead reviewer additionally read `internal/registry/tarball.go`, `client.go`, `credentials.go`, `internal/emitter/emitter.go`, the `pull` command in `cmd/veld/registry.go`, the registry schema in `internal/server/db/db.go`, and `docs/architecture/overview.md` directly, and independently confirmed the archive-handling findings that carry the highest security severity.

## 2.5 Evidence labels (used throughout chapters 04 and 05)

| Label | Meaning |
|---|---|
| reproduced defect | Triggered in this review with a recorded command and output |
| source-supported defect | Visible in the code at the cited lines; not executed, but the failure path is unambiguous |
| suspected risk | Plausible from code reading; needs a targeted test to confirm |
| design limitation | Works as designed, but the design does not meet the stated goal |
| documentation mismatch | Documentation asserts something the code does not do (or the reverse) |
| unverified | Could not be checked in this environment |

Severity scales are defined in chapter 05 (user impact, security severity, delivery priority P0 to P3 are kept separate).

## 2.6 Limitations

- Rust, C# and PHP generated code was not compiled; Java only inspected. Findings in those emitters are source-supported or suspected, not reproduced.
- No JetBrains Gradle build was attempted (Gradle offline availability unverified).
- External facts (competitors, versions, specifications) come from search summaries; where a primary page was not readable they are marked provisional in chapter 03 and the external-research appendix.
- No user study was run. The evaluation plan in chapter 17 separates planned measurements from results.
- Coverage percentages are Go statement coverage from `go test -cover`; they say nothing about whether generated code compiles. Only the audit tracks' compile and run steps do.
- The capacity and calendar numbers in chapters 15 and 16 are planning assumptions (12 h/week technical, 8 h/week D1, assumed exam windows) to be validated by the team in Sprint 0.
