# 11. Language server and editor integrations

Decision record: ADR-006. Evidence: LSP and editors audit track (scripted JSON-RPC session over stdio against `veld lsp`, extension build in a copy, source reading); finding IDs F-LSP-01 to F-LSP-17 in chapter 05.

## 11.1 What exists at 6a47f81 (verified)

| Item | State | Evidence |
|---|---|---|
| Transport | Content-Length framing over stdio; one message at a time on the read loop; no `recover()` | `internal/lsp/server.go:34-49, 104-137` |
| Capabilities advertised | `{"textDocumentSync":1,"completionProvider":{},"hoverProvider":true,"definitionProvider":true}` | initialize response, session log |
| Document sync | full text on `didOpen`/`didChange`/`didSave`; `didClose` not handled | `handler.go:234-243` |
| Diagnostics | first error only, whole-line range (character 0 to 1000), sometimes on the `action` line instead of the bad token | reproduced |
| Imports / config | never loaded; `rootUri` ignored; a valid `modules/auth.veld` importing `User` reports `undefined output type "User"` | reproduced, `handler.go:261-273, 324-365` |
| Positions | UTF-8 byte offsets; no `positionEncoding` negotiation; hover after an emoji fails at the UTF-16 column | reproduced |
| Completion | same 40 items after `method:`, after `input:` and at top level; hard-coded lists | `completion.go:24-75` |
| Hover / definition | work within a single file; return `null` for imported models | reproduced |
| Robustness | survived garbage input, a NUL byte, cyclic `extends`, 20,000 nested braces; unknown methods get no reply (client hangs); malformed JSON silently dropped | reproduced |
| Protocol conformance | `shutdown` reply lacks `result`; empty diagnostics sent as `null`; `exit` without `shutdown` returns 0 | reproduced |
| Performance | 200 rapid `didChange` events answered within 34 ms on a small file; a 2.4 MB document costs about 745 ms per change and blocks hover behind it | measured |
| Lexer defect visible through LSP and CLI | unterminated string literals are accepted silently | `internal/lexer/lexer.go:258-270`, reproduced |
| VS Code extension | 1,676-line TypeScript re-implementation of the language; no `vscode-languageclient`; commands and settings declared but not registered; rejects valid `headers:` and `response:`; `out/` tracked and stale; no lockfile; no tests; `npm install && npm run compile` succeed in a copy; `npm audit` reports 3 dev-only vulnerabilities | `editors/vscode/src/extension.ts`, `package.json` |
| JetBrains plugin | Gradle, intellij-platform plugin 2.14.0, Kotlin 2.1.10, target IDEA 2025.1; hand-written PSI lexer/parser/annotator (about 4,090 lines Kotlin); only `VeldLanguageSpec.kt` is generated; diagnostics via `veld validate` subprocess on disk files; not built in this review (no Gradle) | `editors/jetbrains/**` |
| Language metadata | `go run ./cmd/generate-language` produces no drift in tracked files; `veld-language.json` is gitignored; both plugin schema copies are byte-identical to `editors/veld-config.schema.json`; CI drift job uses `git diff --quiet`, which misses new untracked files | `.github/workflows/ci.yml:38-67` |

README line 429 and the docs describe rich editor support. The implementation is a basic single-file server plus two independent client-side language implementations.

## 11.2 Target design

### 11.2.1 Compiler reuse

The server calls `compiler.Compile(ctx, CompileOptions{Overlay: map[URI]string, ConfigDir: nearest veld.config.json upward from the file})` (chapter 08). The overlay supplies unsaved buffers; everything else is read from disk through the loader. The result carries structured diagnostics with file, byte offset, length, code and severity, which the server converts to UTF-16 columns in one helper. No language rule lives in the server.

### 11.2.2 Feature priority (value against cost)

| Feature | Value | Cost | Graduation scope |
|---|---|---|---|
| Diagnostics (all errors, exact ranges, cross-file) | highest | 16 to 30 h (F-LSP-01, 04) plus structured errors in the compiler | yes, M2 |
| Protocol correctness (MethodNotFound, ParseError, `result` on shutdown, `[]` not `null`, exit codes, `recover`, debounce) | high | 8 to 16 h (F-LSP-02, 07, 08) | yes, M2 |
| UTF-16 position handling or `positionEncoding: utf-8` negotiation | high | 3 to 6 h (F-LSP-03) | yes, M2 |
| Completion (context-aware: directive names, types, model names including imports, enum members) | high | 6 to 10 h (F-LSP-10) | yes, M2/M4 |
| Hover and definition across files | high | included in F-LSP-01 | yes, M2 |
| Document symbols | medium | 3 to 5 h | yes, M4 |
| Formatting (reuse `veld fmt`) | medium | 2 to 4 h | yes, M4 |
| References | medium | 4 to 8 h | M4 if capacity allows |
| Rename | low to medium | 6 to 10 h | deferred |
| Code actions (quick fixes for common diagnostics) | medium | 8 to 16 h | deferred |
| Semantic tokens | low (TextMate grammar is enough for a small language) | 6 to 10 h | deferred |
| Cancellation (`$/cancelRequest`) | low for a fast language, medium for large files | 3 to 5 h | deferred; debounce covers the common case |

Performance target to measure in M4: diagnostics within 100 ms for a 2,000-line project with 10 files on the reference laptop; hover unaffected by a concurrent `didChange`.

### 11.2.3 VS Code extension (primary)

- Replace `extension.ts` with a `vscode-languageclient` launcher (about 150 lines), TextMate grammar generated by `cmd/generate-language` from `internal/language` (keywords, directives, HTTP methods, scalar types), snippets generated from the same source, and three commands that run the CLI.
- Server discovery and download: described in ADR-006 item 3. The download step is implemented in M4 and verified with a test that corrupts the checksum.
- Settings: `veld.executablePath`, `veld.trace.server`, `veld.validateOnSave` (removed if the server validates on change).
- Logs: the LSP client output channel; server logs go to stderr with a `--log-level` flag.
- Packaging: `vsce package` in CI on tags; publish to the Marketplace by a human with a token held by T5; Open VSX as a stretch goal.
- Tests: an extension integration test (`@vscode/test-electron`) that opens a fixture project and asserts one diagnostic and one completion item; runs in the scheduled CI matrix.

### 11.2.4 JetBrains (limited)

- Keep the PSI plugin for highlighting and brace matching only; delete `VeldAnnotator.kt` validation rules and hand-written completion lists (they already disagree with the parser).
- Add an LSP4IJ language-server definition pointing to the same `veld` binary (`veld lsp`), with the binary path configurable and a PATH fallback.
- Build only in a scheduled CI job (Gradle download is slow); publish to the JetBrains Marketplace only if the Beta gate is passed with capacity to spare.
- Maintenance estimate once migrated: 2 to 4 h per quarter for IDE version bumps, against 12 to 20 h per month today (audit estimate, assumptions: one language change per month, one IDE bump per quarter).

### 11.2.5 Canonical language metadata

`internal/language` becomes a real dependency of the lexer, parser and LSP (F-LSP-16), with a test that fails if the parser accepts a directive the spec does not list or vice versa. `cmd/generate-language` continues to emit the VS Code spec file, the Kotlin spec and the JSON file; the CI drift job switches to `git status --porcelain` so new untracked files are caught (F-LSP-17).

## 11.3 Sequence and owners

| Step | Sprint | Owner / reviewer | Exit criterion |
|---|---|---|---|
| Structured diagnostics in `internal/errors` (offset, length, code, all errors) | S3 to S4 | T1 / T5 | CLI prints all errors with `file:line:col`; JSON output available |
| Server on compiler API with overlay and import resolution; protocol fixes; UTF-16 conversion; `recover`; debounce | S4 to S5 | T5 / T1 | scripted session (`lsp-audit/drive.py` from the review, moved into the repo as a Go test) passes all cases including the multi-file project |
| Context-aware completion, document symbols, formatting | S8 to S9 | T5 / T4 | completion after `input:` lists only models including imported ones |
| VS Code thin client, binary discovery and download, commands | S9 to S10 | T5 / T2 | extension integration test green; hand-written service deleted |
| JetBrains: strip annotator, add LSP4IJ definition | S13 (if Beta gate passed) | T5 / T1 | highlighting plus LSP diagnostics in IDEA 2025.x verified manually, screenshots in the report |

Total server and VS Code work: 75 to 135 h, matching the audit's 60 to 100 h server estimate plus 20 to 40 h for the client. This is the second-largest single investment after the compiler IR and is justified because editor support is central to the thesis that a custom DSL is viable (chapter 03).
