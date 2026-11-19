# Appendix: audit-track notes (F-lsp-editors)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Veld LSP and editor integrations audit

Commit 6a47f81, reviewed 2026-10-21. Scratchpad: `...\scratchpad\lsp-audit\` (drive.py, extra.py, pos.py, session.log, extra.log, pos.log, proj/, vscode-copy/).

## Part A: LSP server source reading (internal/lsp, 1017 source lines, 134 test lines, 33.6% statement coverage)

- Command: `veld lsp` (cmd/veld/tooling.go:178-191) calls `lsp.NewServer().Run()`. A stale "lsp (placeholder)" comment sits at cmd/veld/contract.go:592.
- Transport (server.go:52-79): `Content-Length` framing. Header match is case-sensitive (`strings.HasPrefix(line, "Content-Length:")`, :63). Body is read with `io.ReadFull` (so partial reads are handled). Missing or zero length makes `Run()` return an error and the process exits 1 (:69-71). No JSON-RPC batch support (a body that is a JSON array fails to unmarshal and is silently skipped, :44-46). Malformed JSON is silently dropped (`continue`), with no ParseError reply.
- Dispatch (server.go:104-137): a single `switch`. Unknown methods get no reply at all. Requests with an id get no MethodNotFound, so the client hangs. `$/cancelRequest` is not handled (silently ignored). `shutdown` replies with a nil result. `exit` calls `os.Exit(0)` unconditionally (:116-117).
- Concurrency: `Run()` is one goroutine reading and dispatching synchronously (:34-49). `Handler.mu` (RWMutex) and `Server.mu` (write lock) exist but are never contended. A long parse blocks every later message (measured below). There is no `recover()` anywhere in internal/lsp (grep), so a parser/validator panic would kill the server.
- Documents (handler.go:219-259): map URI to text, plus map URI to last good AST. URIs are opaque strings and are never converted to a filesystem path. Diagnostics use the unsaved buffer (`documents[uri]`, handler.go:278), never disk. `didSave` just recomputes from the same buffer (server.go:170-181). `didClose` is not handled, so buffers are never released and diagnostics are never cleared.
- Sync: `textDocumentSync: 1` (Full) (handler.go:237); only `contentChanges[0].Text` is used (server.go:161).
- Imports: the LSP uses lexer, parser and validator on the single buffer. It never calls `internal/loader`, never looks up veld.config.json, and does not use rootUri or workspaceFolders (`Initialize()` ignores its params). Any file that references an imported model gets false "undefined type" errors, and cross-file features cannot work.
- Diagnostics (handler.go:324-396): reuses the CLI's `lexer.Tokenize`, `parser.Parse` and `validator.Validate`. Only the first lex or parse error is reported. The line is regex-extracted from the message text (`extractLine`, :367). The range is always the whole line, `character 0..1000` (:391-396). Columns are never computed, so there is no UTF-16 issue in diagnostic ranges (the range is simply coarse).
- Positions: all feature code treats `Position.Character` as a byte index into the line (completion.go:17-18, hover.go:130-153 `wordAtPosition`), and definition ranges use `len()` bytes (definition.go:112). There is no `positionEncoding` capability and no negotiation, so the server is effectively UTF-8, not the LSP default UTF-16.
- Stale AST: `parseDocument` returns early on lex/parse error without clearing `asts[uri]` (handler.go:261-273). Hover, completion and definition then use an old AST.
- Logging: none (stderr empty in every session).
- Capabilities advertised (verbatim): `{"textDocumentSync":1,"completionProvider":{},"hoverProvider":true,"definitionProvider":true}`.

## Part B: scripted session (drive.py, abbreviated pairs in session.log; edge cases in extra.log, pos.log)

Project: proj/veld/{veld.config.json, app.veld, models/user.veld, modules/auth.veld}. The CLI says "Contract is valid".

ServerCapabilities verbatim: `{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"textDocumentSync":1,"completionProvider":{},"hoverProvider":true,"definitionProvider":true}}}`

| Step | Result |
|---|---|
| didOpen app.veld (valid) | publishDiagnostics with `"diagnostics": null` (not `[]`) |
| didOpen modules/auth.veld (valid on disk) | FALSE POSITIVE: `line 7: module "Auth", action "GetUser": undefined output type "User"` (imports unresolved) |
| didChange adding `output: Unknwn` | diagnostic `line 8: ... undefined output type "Unknwn"`, range line 7 (the `action A {` line), char 0..1000. Not on the token. |
| Non-ASCII added before the error (earlier line, or same line) | Diagnostic is identical because it is line-only. No UTF-16 mismatch is possible, but no real column is given either. |
| Valid doc using local and imported models | `[]`/null. Parses with imports unresolved because both are declared locally or ignored. |
| completion after `method: ` | 40 items: all keywords, types, HTTP methods, local models. Identical to `input: ` and to top level. Not context-aware. |
| completion after `input: ` | Includes `Local` (same buffer) but NOT `User` (imported) |
| hover on `Local` in `input:` | Works (markdown with description and fields) |
| hover on imported `User` | `null` |
| definition on local `Local` | Works, same file, line 5, char 0..11 |
| definition on imported `User` | `null`. No cross-file jump. |
| references, rename, formatting, documentSymbol, semanticTokens/full, codeAction | No response at all (2 s timeout each). Server never replies with MethodNotFound. |
| unterminated string, followed by a newline (`description: "oops\n id: string`) | NO diagnostic (`null`). The lexer silently accepts it (internal/lexer/lexer.go:262-269), and the CLI `veld validate` also says "valid". |
| unterminated string at EOF / unclosed brace | Diagnostic `expected "}", got ""` on the last line. The server stays alive. |
| garbage, NUL byte, cyclic extends, 20,000-deep `model M {` | Correct errors, no crash |
| Unknown method with an id | NO reply (timeout 3 s) |
| Malformed JSON body | No reply, server survives |
| didOpen with no params | Server survives, publishes diagnostics for uri `""` |
| shutdown response | `{"jsonrpc":"2.0","id":19}`: the `result` member is missing (omitempty), a JSON-RPC 2.0 violation |
| `exit` without prior shutdown | exit code 0 (the spec requires 1) |
| shutdown then exit | exit code 0 |
| lowercase `content-length` header | Server exits 1 (`missing Content-Length header`) |
| id 0 and string ids | handled |
| hover at UTF-16 column on a line with 3 emoji before the word | `null`. At the byte column it returns the model. This confirms byte-offset semantics. |

Latency (this Windows machine, veld.exe built from HEAD):
- 200 back-to-back didChange (about 450-byte doc): 200 diagnostics notifications, last one 34 ms after the first send (send took 32 ms).
- 2.4 MB document (30,000 models) didOpen to diagnostics: 748 ms. completion on it: 184 ms.
- 5 consecutive didChange of the 2.4 MB doc: 3,722 ms total (about 744 ms each, strictly serial).
- Hover queued behind one 2.4 MB didChange: 684 ms (head-of-line blocking).
- No coalescing or debouncing of rapid changes. Every keystroke is fully re-lexed, re-parsed and re-validated.

## Part C: VS Code extension (editors/vscode)

- package.json: `main ./out/extension.js`, `activationEvents: []` (relies on the implicit onLanguage activation from `contributes.languages`, OK for engine ^1.85). It contributes language, TextMate grammar (498 lines, hand-written), semanticTokenScopes, snippets (veld.json 208 lines, veld-config.json 109 lines), jsonValidation for veld.config.json, 3 commands (`veld.validate`, `veld.generate`, `veld.generateDryRun`), and settings `veld.executablePath` and `veld.validateOnSave`.
- It does NOT use vscode-languageclient and does not launch `veld lsp`. `src/extension.ts` is 1,676 lines containing a TypeScript re-implementation of the language service: its own regex/line-based parser, diagnostics, completion, hover, definition, references, semantic tokens, document symbols (`class VeldLanguageServer`, :81; providers registered at :1550-1591). It reads veld.config.json itself to resolve aliases (`findProjectRoot` :89, `readAliases` :107).
- The three declared commands have NO handler: `registerCommand` and `child_process` never appear in extension.ts. `veld.executablePath` and `veld.validateOnSave` are never read. The extension never finds, downloads or invokes the veld binary (no PATH lookup, no download, no checksum, no version pinning). So offline behaviour is a non-issue, but the settings and the commands are dead.
- The only generated input consumed is `veld-language-spec.ts` (KEYWORDS, HTTP_METHODS, BUILTIN_TYPES, DIRECTIVES, SPECIAL_TYPES). The grammar is NOT generated from it, and completion and diagnostics keep their own hard-coded sets (e.g. `ACTION_KEYS`).
- Tests: none. Publish: `vsce package`/`vsce publish` via PUBLISHING.md only; no CI job builds the extension.
- Build check (in a copy under lsp-audit/vscode-copy, network allowed): `npm install` OK (generated a package-lock.json; the repo tracks none; `npm audit` reports 3 vulnerabilities, 2 high, in the dev-only vsce tree such as linkify-it). `npm run compile` (tsc) succeeds, no errors. The freshly compiled `out/extension.js` and `out/veld-language-spec.js` DIFFER from the committed `out/` files (the committed veld-language-spec.js is 1,142 bytes, i.e. stale).

## Part D: JetBrains plugin (editors/jetbrains)

- Build: Gradle (wrapper checked in), Kotlin 2.1.10, `org.jetbrains.intellij.platform` 2.14.0, target IntelliJ IDEA Community 2025.1, since-build 241, until-build 263.*, JVM 21. No Gradle on PATH and no offline distribution, so a build was NOT attempted (it needs network and the wrapper download).
- It does not use LSP4IJ or the native LSP API (plugin.xml has no lsp reference). It is a hand-written PSI plugin: `VeldLexer.kt` (290 lines, keywords hard-coded at :272), `VeldParser.kt` (364), `VeldAnnotator.kt` (696, in-editor heuristics, own `ACTION_KEYS` at :71), `VeldCompletionContributor.kt` (457), `VeldProjectService.kt` (457), reference contributor, structure view, documentation provider, color settings, and more.
- Kotlin: about 4,090 lines of .kt across 31 files (4,667 lines including plugin.xml, schema copy and resources). `VeldLanguageSpec.kt` (52 lines) is generated and consumed by Annotator, CompletionContributor, DocumentationProvider and ReferenceContributor. The lexer, syntax highlighter and annotator key sets are hand-maintained duplicates.
- `VeldExternalAnnotator` shells out to `veld validate` from PATH with cwd = `project.basePath` (VeldExternalAnnotator.kt:25-27) and regex-parses coloured CLI output. It validates the on-disk project, not the unsaved buffer, and the project root may not be where veld.config.json lives.

### Maintenance cost estimate (hours/month)

Assumptions: one language change per month (new keyword, directive or annotation), one IDE/VS Code API bump per quarter, no new features.
- VS Code as it is: about 8-12 h/month. The TS re-implementation (1,676 lines) plus the grammar and snippets must be kept in step with the Go parser by hand. ACTION_KEYS drift already causes false errors (F-LSP-13).
- JetBrains as it is: about 12-20 h/month. The same duplicated language logic in Kotlin (lexer, parser, annotator, completion), plus a platform bump every half year (until-build 263), plus plugin verifier and signing.
- Both together: about 20-32 h/month and growing with every language feature, and each re-implements validation that the Go compiler already does.
- Alternative: make `veld lsp` the single implementation (a one-off 60-100 h investment to fix F-LSP-01 to 11). Then VS Code becomes a thin vscode-languageclient wrapper (about 150 lines) plus grammar and snippets (about 2-3 h/month), and JetBrains could use LSP4IJ with a small config (about 2-4 h/month) or stay on its PSI plugin for highlighting only.

## Part E: language metadata pipeline

- `cmd/generate-language/main.go` (178 lines) reads `language.GetLanguageSpec()` (internal/language/constants.go and veld.go) and writes `veld-language.json`, `editors/vscode/src/veld-language-spec.ts`, and `editors/jetbrains/.../VeldLanguageSpec.kt`.
- Consumers: VS Code (5 sets used) and JetBrains (4 files) read the generated files. `veld-language.json` is gitignored (`.gitignore:69`, `git ls-files` returns nothing), so it is not tracked and nothing in the repo reads it.
- Only `cmd/generate-language` imports `internal/language`. The Go lexer, parser, validator, LSP completion and formatter do NOT. The CLAUDE.md claim "single source of truth" is true only for the two editor specs, not for the compiler.
- Ran `go run ./cmd/generate-language`: `git status --short` and `git diff --stat` show no changes to the tracked generated files (only unrelated untracked docs/ files from another process), and `veld-language.json` is byte-identical to the earlier copy. No drift at HEAD, so nothing needed restoring.
- CI (`.github/workflows/ci.yml:38-67`, job `drift`): runs the generator and `scripts/sync-schema.sh`, then `git diff --quiet`, and exits 1 on any diff. So it does fail on drift of tracked files. Gaps: `git diff` ignores untracked new files; the check only proves that the generated files match `constants.go`, not that `constants.go` matches the parser (no test links them); no job builds or tests the editors.
- Schema: `scripts/sync-schema.sh` copies `editors/veld-config.schema.json` to `editors/vscode/veld-config.schema.json` and `editors/jetbrains/src/main/resources/schemas/veld-config.schema.json`. Both copies are identical to the canonical file (`diff` clean, sync script run produced no changes).

## Part 1: capability matrix

| Method | Implemented? | Works in scripted session? | Notes |
|---|---|---|---|
| initialize | yes | yes | Ignores params, no positionEncoding, no workspace folders |
| initialized | yes (no-op) | yes | |
| shutdown | partial | replies without `result` | Spec violation |
| exit | yes | exit 0 | Always 0 |
| textDocument/didOpen | yes | yes | |
| textDocument/didChange | full sync only | yes | First change only |
| textDocument/didSave | yes | not separately driven | Same as change |
| textDocument/didClose | no | n/a | Leaks buffers, diagnostics stay |
| publishDiagnostics | yes | yes | `null` instead of `[]`, whole-line ranges, first error only, no import resolution |
| completion | yes | yes, but context-blind | 40 identical items everywhere |
| hover | yes | local only | Byte columns |
| definition | yes | local only | Imports return null |
| references | no | no reply | |
| rename | no | no reply | |
| formatting | no | no reply | `veld fmt` exists in the CLI but is not exposed |
| documentSymbol | no | no reply | |
| semanticTokens | no | no reply | VS Code does its own |
| codeAction | no | no reply | |
| $/cancelRequest | no | ignored | Moot, requests are serial |
| workspace/didChangeWatchedFiles | no | n/a | |
| workspace folders | no | n/a | |
| unknown method | no MethodNotFound | no reply (hang) | |

## Verdicts

- Position encoding: UTF-8 byte offsets in practice (completion, hover and definition), no `positionEncoding` negotiated, so non-ASCII lines break for UTF-16 clients (reproduced for hover). Diagnostic ranges are whole-line, so they are unaffected.
- Unsaved buffer: yes for the single file, diagnostics come from the in-memory buffer. But imported files are never loaded (neither from disk nor from other open buffers), and no config is found. Multi-file projects are not supported.
- Windows URIs: URIs are opaque map keys and never converted to paths. No Windows breakage, but nothing that needs a path can work. Two spellings of one file (`file:///d%3A/...` vs `file:///D:/...`) would be two documents (suspected, not driven).

## Findings

### F-LSP-01: LSP ignores imports, so valid multi-file projects get false errors and no cross-file features
- Category: lsp-diagnostics
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/lsp/handler.go:261-273, 324-365; internal/lsp/server.go (no loader use)
- Trigger/reproducer: didOpen `proj/veld/modules/auth.veld` (valid per `veld validate`), which has `import @models/user` and `output: User`. See session.log.
- Expected vs actual: expected no diagnostics; actual `line 7: module "Auth", action "GetUser": undefined output type "User"`. Hover and definition on imported `User` return null; completion omits imported models.
- Impact: every file in a real project that uses an import shows a spurious error. The server is unusable beyond single-file toy contracts.
- Root cause: `computeDiagnostics` validates one buffer in isolation; `internal/loader` and config discovery are not used; `initialize` ignores rootUri.
- Recommendation: refactor — load via the loader from the nearest veld.config.json with open buffers overlaid on disk.
- Regression check: add an LSP test on a temp project where an importing file yields zero diagnostics and definition on an imported model returns the other file's URI.
- Effort estimate: 16h–30h

### F-LSP-02: Unknown requests get no response; malformed JSON gets no ParseError
- Category: lsp-robustness
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/server.go:44-46, 104-137
- Trigger/reproducer: send `foo/bar` with an id; also `references`, `rename`, `formatting`, `documentSymbol`, `codeAction`.
- Expected vs actual: expected error -32601 MethodNotFound (and -32700 for bad JSON); actual silence, so the client request never completes.
- Impact: any client that sends an optional request (many do) can hang or time out; the server hides its own unsupported surface.
- Root cause: the `switch` has no default branch and the parse error path is `continue`.
- Recommendation: fix — add a default MethodNotFound reply for requests (id present) and a ParseError reply.
- Regression check: test sends an unknown method and asserts error code -32601 within a timeout.
- Effort estimate: 1h–2h

### F-LSP-03: Position handling uses byte offsets with no positionEncoding negotiation
- Category: lsp-positions
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/completion.go:17-18; internal/lsp/hover.go:130-153; internal/lsp/definition.go:112,125,138
- Trigger/reproducer: pos.py — line `// <3 emoji> Local`, hover at UTF-16 column 11 returns null, at byte column 17 returns the model.
- Expected vs actual: expected UTF-16 code-unit columns (the LSP default), or an advertised `positionEncoding: utf-8`; actual raw byte indexing with nothing advertised. Slicing a multi-byte rune is also possible in completion (`line[:pos.Character]`).
- Impact: hover, completion and definition misbehave after any non-ASCII character on the line (comments, descriptions). Possible index slicing into the middle of a rune.
- Root cause: string indexing by `Position.Character` without conversion.
- Recommendation: fix — convert UTF-16 columns to rune/byte offsets in one helper, or negotiate utf-8.
- Regression check: unit test with emoji and accented characters before the cursor for hover, completion and definition.
- Effort estimate: 3h–6h

### F-LSP-04: Diagnostics are first-error-only, whole-line, and often on the wrong line
- Category: lsp-diagnostics
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/handler.go:329-395
- Trigger/reproducer: `output: Unknwn` is reported on the `action A {` line (0-based 2 or 7), not on the token; range is `character 0..1000` always.
- Expected vs actual: expected a range over `Unknwn`; actual the whole line, 1000 characters wide, and only the first lex/parse error.
- Impact: squiggles cover entire lines (and line 1 on a message with no line), and a file with several errors shows one at a time.
- Root cause: errors carry only a line number, which the server regex-extracts from `err.Error()`; the lexer tracks no columns.
- Recommendation: refactor — return structured errors (line, column, length) from the internal/errors package instead of parsing message text.
- Regression check: golden test asserting range start/end for a type error and a syntax error.
- Effort estimate: 8h–16h

### F-LSP-05: Lexer silently accepts unterminated string literals (CLI too)
- Category: lsp-diagnostics
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:258-270
- Trigger/reproducer: `model M {\n  description: "oops\n  id: string\n}` yields no diagnostic from the LSP and `veld validate` prints "Contract is valid".
- Expected vs actual: expected an "unterminated string" error; actual the string ends at the newline silently.
- Impact: typos in descriptions or import paths pass validation; with a missing closing quote on an import the wrong path is used.
- Root cause: the closing quote is optional (`if ... == '"' { pos++ }`) with no else branch.
- Recommendation: fix — return a lexer error when a string ends at newline/EOF.
- Regression check: lexer unit test for an unterminated string at newline and at EOF.
- Effort estimate: 1h–2h

### F-LSP-06: Last good AST is retained when the buffer stops parsing
- Category: lsp-sync
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/lsp/handler.go:261-273
- Trigger/reproducer: open a doc with model Local, then change to a buffer with a syntax error; hover, completion and definition still use the old AST.
- Expected vs actual: either empty results or an explicit "stale" behaviour; actual silently returns old model data (hover shows deleted models; positions point to old lines).
- Impact: wrong go-to-definition lines while editing.
- Root cause: early return on error without updating `asts`.
- Recommendation: fix — keep a separate stale AST only for completion names, and drop it for definition and hover.
- Regression check: test that after a parse error definition returns null, not the old location.
- Effort estimate: 1h–3h

### F-LSP-07: JSON-RPC conformance defects (shutdown result, null diagnostics, exit code)
- Category: lsp-transport
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/protocol.go:410-415 (`Result ... omitempty`), :511 (`Diagnostics`); internal/lsp/server.go:113-117; handler.go:284 (nil slice)
- Trigger/reproducer: shutdown returns `{"jsonrpc":"2.0","id":19}` (no `result`); empty diagnostics serialize as `"diagnostics": null`; `exit` without `shutdown` returns 0.
- Expected vs actual: expected `"result": null`, `"diagnostics": []`, and exit code 1 when exit arrives without shutdown.
- Impact: strict clients (and some Neovim/Emacs/Helix configs) can reject the response or treat `null` diagnostics as an error; shell supervisors see a clean exit after an unclean shutdown.
- Root cause: `omitempty` on `Result` and a nil slice for no diagnostics; `os.Exit(0)`.
- Recommendation: fix — always emit `result` for successful replies, use empty slices, track the shutdown flag.
- Regression check: transport test that checks the raw JSON for `"result":null` and `"diagnostics":[]`.
- Effort estimate: 2h–3h

### F-LSP-08: Single-threaded, un-debounced, no recover — slow parses block everything and a panic kills the server
- Category: lsp-robustness
- Evidence status: suspected risk
- Severity (user impact): medium
- Security severity: low
- Location: internal/lsp/server.go:34-49; internal/lsp/handler.go (no recover anywhere in internal/lsp)
- Trigger/reproducer: 2.4 MB doc: each didChange costs about 744 ms, and a hover sent after it waited 684 ms. No panic was reproduced (garbage, deep nesting and empty inputs were all handled).
- Expected vs actual: expected cancellable/debounced work with panic isolation; actual serial full re-analysis per message.
- Impact: typing in a very large contract lags; any latent parser/validator panic terminates the editor's language service (the CLI binary exits) with no log.
- Root cause: synchronous dispatch, no coalescing of changes, no `defer recover()`.
- Recommendation: fix — wrap dispatch in recover, debounce diagnostics per URI, and run requests off the read loop.
- Regression check: test feeding an input known to panic (fuzz target) and asserting the server answers the next request.
- Effort estimate: 6h–12h

### F-LSP-09: Feature surface far below the documented editor support
- Category: lsp-features
- Evidence status: documentation mismatch
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/handler.go:234-243; README.md:429
- Trigger/reproducer: capability list shows only sync, completion, hover, definition. No didClose, references, rename, formatting, documentSymbol, semanticTokens, codeAction, workspace support.
- Expected vs actual: README advertises "Any LSP editor ... Diagnostics, completions, hover, go-to-definition"; that is technically listed, but without import resolution (F-LSP-01) and with context-blind completion (F-LSP-10) it is much weaker than the VS Code and JetBrains plugins.
- Impact: LSP-only editors (Neovim, Helix, Zed) get a weaker experience; no memory/diagnostic cleanup on close.
- Root cause: the LSP is a first-pass server and test coverage is 33.6%.
- Recommendation: refactor — decide on the LSP as the single engine and add didClose, documentSymbol, references and formatting (reusing `veld fmt`) first.
- Regression check: capability-vs-handler test asserting every advertised provider answers.
- Effort estimate: 24h–40h

### F-LSP-10: Completion is context-blind and uses hard-coded lists
- Category: lsp-features
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lsp/completion.go:24-75
- Trigger/reproducer: completion after `method: `, after `input: ` and at top level all return the same 40 items.
- Expected vs actual: expected HTTP methods after `method:`, model names after `input:`/`output:`, top-level keywords only at top level; actual everything everywhere, with imported models missing. Keyword/type/method lists are copied literals, not from `internal/language`, and include `from`/`default`/`WS` handling that differs from the generated spec.
- Impact: noisy completion and no real help while writing contracts.
- Root cause: `prefix` is computed but only used for the import check; there is no cursor-context classification.
- Recommendation: refactor — classify the cursor context from the line prefix and source the candidate lists from `internal/language`.
- Regression check: table test of context to expected labels.
- Effort estimate: 6h–10h

### F-LSP-11: VS Code extension re-implements the language service in TypeScript and does not use `veld lsp`
- Category: vscode
- Evidence status: source-supported defect
- Severity (user impact): high
- Security severity: none
- Location: editors/vscode/src/extension.ts:81-1675; editors/vscode/package.json
- Trigger/reproducer: grep for `languageclient`, `child_process`, `spawn`, `veld lsp` in editors/vscode finds nothing.
- Expected vs actual: expected a thin client over the Go compiler; actual a 1,676-line parallel parser/validator that can disagree with the CLI.
- Impact: two (VS Code TS, Go LSP) plus a third (JetBrains Kotlin) implementations of Veld semantics; every language change must be made three times.
- Root cause: the extension predates or ignores the Go LSP; the LSP is too weak to adopt (F-LSP-01).
- Recommendation: rewrite — once the LSP is fixed, replace the TS service with a vscode-languageclient launcher.
- Regression check: an extension test that starts `veld lsp` and checks a diagnostic for a known-bad contract.
- Effort estimate: 20h–40h (after F-LSP-01 to 10)

### F-LSP-12: VS Code contributes commands and settings that do nothing
- Category: vscode
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: editors/vscode/package.json (commands veld.validate/generate/generateDryRun, settings veld.executablePath/validateOnSave); editors/vscode/src/extension.ts (no registerCommand, no getConfiguration)
- Trigger/reproducer: run "Veld: Generate Code" from the palette — expected "command not found" because no handler is registered (not run in a VS Code host; established by grep).
- Expected vs actual: documented commands and settings exist; actual they are never wired. The README/PUBLISHING docs describe the binary being on PATH.
- Impact: the advertised "Validate on save" and "Generate" actions silently do not exist.
- Root cause: manifest and implementation diverged.
- Recommendation: fix — either wire the commands to the CLI (using `veld.executablePath`) or remove them.
- Regression check: a manifest-vs-registerCommand consistency test.
- Effort estimate: 3h–6h

### F-LSP-13: VS Code (and JetBrains) validators reject valid action directives
- Category: vscode
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: editors/vscode/src/extension.ts:~497-506 (`ACTION_KEYS`: method, path, input, output, query, stream, middleware, errors, description); editors/jetbrains/.../VeldAnnotator.kt:71-72 (adds `response`, still no `headers`/`emit`)
- Trigger/reproducer: an action with `headers: { ... }` (documented in CLAUDE.md and accepted by internal/parser/parser.go:939) gets "Unknown action directive 'headers'" as an Error; same for `emit` (parser.go:985) and, in VS Code, `response`. Established by reading, not by running a VS Code host.
- Expected vs actual: expected no error since the parser accepts them; the generated `VELD_SPEC.directives` itself includes headers, response and emit.
- Impact: red squiggles on valid contracts.
- Root cause: hand-maintained key sets that are not derived from the generated spec.
- Recommendation: fix — derive the sets from `VELD_SPEC.directives` (or drop client-side validation per F-LSP-11).
- Regression check: test that every directive accepted by internal/parser appears in each editor's action key set.
- Effort estimate: 1h–2h

### F-LSP-14: Committed `editors/vscode/out/` is stale and the extension has no tests or lockfile
- Category: vscode
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Location: editors/vscode/out/*.js (tracked); no package-lock.json tracked
- Trigger/reproducer: `npm install && npm run compile` in a copy succeeds, but `cmp` shows the fresh `out/extension.js` and `out/veld-language-spec.js` differ from the committed ones (the committed spec file is 1,142 bytes). `npm audit`: 3 vulnerabilities (2 high) in dev dependencies.
- Expected vs actual: build artifacts should not be committed, and builds should be reproducible.
- Impact: confusion about what ships; unpinned dev dependencies; no CI job would catch a broken extension.
- Root cause: `out/` committed once and never refreshed; no lockfile; no editor CI.
- Recommendation: fix — untrack `out/`, commit a lockfile and add a compile job to CI.
- Regression check: CI job running `npm ci && npm run compile` on editors/vscode.
- Effort estimate: 2h–4h

### F-LSP-15: JetBrains plugin is a second hand-written language implementation with PATH-based CLI validation
- Category: jetbrains
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: low
- Location: editors/jetbrains/src/main/kotlin/dev/veld/jetbrains/VeldLexer.kt:272, VeldParser.kt, VeldAnnotator.kt:71, VeldExternalAnnotator.kt:25-27
- Trigger/reproducer: reading; no Gradle build was possible (Gradle not installed, no offline distribution).
- Expected vs actual: only `VeldLanguageSpec.kt` is generated; the lexer keywords, annotator key sets and completion lists are duplicated by hand. The external annotator runs `veld` from PATH (a PATH-planted binary in an untrusted project would run) in `project.basePath`, validating the saved on-disk files rather than the buffer. Not verified in an IDE.
- Impact: JetBrains drifts from the compiler and from VS Code; about 4,090 lines of Kotlin to maintain.
- Root cause: no shared engine; the plugin predates the LSP.
- Recommendation: defer — keep the PSI plugin for highlighting and move diagnostics/completion to LSP4IJ once the LSP is solid.
- Regression check: a unit test comparing the lexer's keyword set to `VeldLanguageSpec.KEYWORDS`.
- Effort estimate: 20h–40h (for LSP4IJ migration)

### F-LSP-16: `internal/language` is not consumed by the compiler, so "single source of truth" is false and CI cannot detect parser/spec drift
- Category: language-metadata
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/language/*.go (imported only by cmd/generate-language/main.go); .github/workflows/ci.yml:38-67
- Trigger/reproducer: `grep -rl "internal/language\"" --include=*.go` returns only `cmd/generate-language/main.go`.
- Expected vs actual: CLAUDE.md calls it the single source of truth; actually lexer, parser, validator, LSP completion and format each hard-code their own keyword/directive lists. The CI drift job only proves the generated editor files equal `constants.go`. `go run ./cmd/generate-language` produced no diff at HEAD, but a parser change (e.g. a new directive) would pass CI without updating editors.
- Impact: the editors stay silently wrong when the language changes (F-LSP-13 is a live example).
- Root cause: spec is documentation-grade data, not wired into the parser.
- Recommendation: refactor — add a Go test asserting parser-accepted directives, keywords and types equal the language spec, and have the LSP completion read it.
- Regression check: that test, run in `go test ./...`.
- Effort estimate: 4h–8h

### F-LSP-17: CI drift check cannot see new untracked files and the JSON output is untracked
- Category: schema-sync
- Evidence status: suspected risk
- Severity (user impact): low
- Security severity: none
- Location: .github/workflows/ci.yml:58-66; .gitignore:69
- Trigger/reproducer: the job uses `git diff --quiet`, which ignores untracked files; `veld-language.json` is gitignored yet CLAUDE.md lists it as a generated artifact.
- Expected vs actual: schema sync itself is fine (both plugin copies are byte-identical to editors/veld-config.schema.json, and re-running sync-schema.sh changed nothing). The risk is only that a newly added generated file would not fail CI.
- Impact: low; minor drift blind spot and an inconsistent claim about veld-language.json being a tracked artifact.
- Root cause: `git diff` vs `git status --porcelain`.
- Recommendation: fix — use `git status --porcelain` in the drift job and document that veld-language.json is a local build artifact.
- Regression check: touch a new generated file in a throwaway CI branch and confirm failure.
- Effort estimate: 0.5h–1h

## Recommendation

Primary editor: VS Code. It is the larger audience, already has the better-maintained client, and can become a thin vscode-languageclient wrapper over `veld lsp`. The one-off cost is fixing the Go LSP (F-LSP-01 to 10, roughly 60-100 h), after which VS Code drops to about 2-3 h/month. JetBrains costs more to keep alive (about 12-20 h/month for a Kotlin PSI plugin plus platform bumps) and should be deferred, then moved to LSP4IJ against the same server.
