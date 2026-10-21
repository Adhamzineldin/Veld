# Appendix: audit-track notes (B-compiler-frontend)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Veld compiler frontend audit

Commit 6a47f815f37ea4c286a6e765312df76743fa7133, reviewed 2026-10-21. No tracked file was modified.
Scratch artefacts: `fe-audit/cases/` (72 case dirs), `fe-audit/results/` (raw stdout/stderr/rc per case), `fe-audit/fz/` (copy of the packages used for a 150 s / 1.67M-exec Go fuzz run and diff scenario tests), `fe-audit/veld.exe`.

Notes on method: `veld validate` takes a positional file (`veld validate main.veld`), not `--input`. `veld lint` takes no file; it was run with a per-case `veld.config.json`. `veld fmt` is hidden and only writes with `--write`; idempotency was tested by running `fmt --write` twice on a copy.

## 1. Package-by-package summary

### internal/lexer (451 lines)
- Public API: `New(src) *Lexer`, `Tokenize() ([]Token, error)`, `Errors()`, `IsKeyword`, `TokenType.String`. Token = {Type, Value, Line}. **Line only; no column, no offset.**
- Source is converted to `[]rune`; lines are counted on `\n` only (a lone `\r` never advances the line: case 58 reports line 1 for an error on line 3).
- Error recovery: unknown characters are collected and skipped, but `Tokenize` returns only `errors[0]` and the loader discards the rest (lexer.go:355-364).
- No limits (file size, token count). 5 MB file and 100k-field model lex+validate in under 1 s. Lexer is O(n), no recursion.
- Identifiers: `unicode.IsLetter/IsDigit` + `_` (Unicode letters OK: `Usuario` with accent passes; `$`, `-`, `.` rejected; a leading digit lexes as a number, so `1abc` becomes `1` then `abc`). `unicode.IsDigit` also accepts non-ASCII digits in numbers.
- Strings: no escapes, end at `"` or newline; an unterminated string is silently accepted (F-FE-04). Unterminated `/* ...` silently swallows the rest of file.
- `/` always starts a path token except when followed by `*` or `/`, where it starts a comment. This breaks `import @models/*` (F-FE-01).
- BOM U+FEFF is rejected as "unexpected character" (F-FE-15).
- Works well: tokenisation of all documented constructs, CRLF, tabs, emoji in strings, 200k-char brace runs (no crash), contextual keywords (`type`, `default`, `delete`, `class`, `import` as field names all work).

### internal/parser (1154 lines)
- Public API: `New(tokens)`, `Parse() (ast.AST, error)`. Hand-written recursive descent, **but effectively non-recursive** (blocks do not nest, generics are one level), so there is no stack-depth risk: a 2000-deep and a 200000-deep `{` run produce a normal error (cases 41, 41b).
- **Stops at the first error** (every function returns on first failure). Messages are plain `fmt.Errorf("line N: ...")`, with ad-hoc prefix wrapping ("model name: line 1: ...").
- Positions: token line only. Error line is the line of the *offending token as consumed*, which is sometimes the wrong line (case 62 reports line 3 for a mistake on line 2; unterminated `model A {` reports the nonexistent EOF line instead of the opening brace).
- Types: `T`, `T[]`, `List<T>`, `Map<string,V>`, unions `A | B | "lit"`. **No nesting**: `Map<string,Map<..>>`, `Map<string,T[]>`, `T[][]`, `List<List<T>>` are all parse errors. `Map<string,int>[]` parses as `{isMap:true,isArray:true}` which cannot be distinguished from a hypothetical array-of-maps vs map-of-arrays (F-FE-08).
- Optional is `name?: type` only; there is no nullable concept. `= value` and `@default(v)` both accepted; both together is an error. Annotations: default, deprecated, example, unique, index, relation, serverSet. Unknown annotation = error.
- Actions: contextual keywords `method path description input output response query headers stream emit middleware errors @deprecated`; duplicates silently last-wins (F-FE-23). `output: T[]:201` parsed; non-integer/overflow status silently dropped (F-FE-16). `{id}` is rewritten to `:id` at parse time (parser.go:149). Inline `input/output/query/headers: { ... }` create synthetic models named `<Action>Input|Output|Query|<Action>Headers` (F-FE-17).
- Enums: bare identifiers only, optional commas; no explicit values (case 45 errors); keyword-like values (`GET`, `model`, `from`, `string`) rejected (F-FE-07).
- Imports: `import @a/b`, `import /a/*`, `import a/b`, `import "file.veld"`, `from @a import X, Y`.

### internal/ast (104 lines)
- Plain structs with JSON tags; `Line`/`SourceFile` are `json:"-"`. Empty slices serialise as `null` (`"models": null`) (F-FE-29). `FileImports` map is not serialised. No column data anywhere.

### internal/loader (398 lines)
- API: `Parse(path, aliases...) (ast.AST, []string, error)`. Reads file, lex, parse, then recursively resolves imports with a `seen` map keyed on absolute path. Cycles and self-imports are silently tolerated (case 19, 20 valid). Merge order is imports first, then the importing file; ReadDir/WalkDir order is lexical so it is deterministic.
- Errors are wrapped strings ("lexing <path>: ...", "parsing <path>: ..."); paths are as given, absolute for nested imports; no import-statement line.
- Missing wildcard directory is silently ignored (loader.go:248); a missing named alias file silently falls back to loading the whole directory (loader.go:285-355); a typo'd alias loads nothing (F-FE-13). Registry-package resolution via `rootDir/packages/@org/pkg`.
- Duplicate names across files are not handled in the loader; the validator reports the second one only, without naming the first.
- No limit on file size or import depth (bounded by `seen`). `strings.Contains(content, "model "+name)` fallback also matches prefixes (`model User` matches `model UserProfile`).
- Self-edge recorded in `FileImports[a] = append(..., a)` for every glob-loaded file (loader.go:255, 273, 341).
- Relative imports are not confined to the project root (`import "../../x.veld"`): by design for a local CLI, no security impact beyond reading `.veld` files the user can already read.

### internal/validator (577 + 65 lines)
- API: `Validate(ast.AST) []error` returns **all** semantic errors at once (good), as `fmt.Errorf("file.veld:LINE: ...")` with the file *basename*. Workspace: `ValidateWorkspaceConsumes` (unknown/self consume, missing baseUrl warning; no cycle detection despite CLAUDE.md claiming "circular").
- Checks: duplicate enum/model/module/action/constants names, model-vs-enum-vs-constants collisions, empty enum, duplicate enum values, extends target exists, circular inheritance (self and longer cycles caught), duplicate fields, type existence with Levenshtein "did you mean", Map value type, @default type-compat for primitives and enum members, WS stream/emit rules, success status 200-299 and 204-without-output, route-conflict across modules (`:param` normalised), per-file "type used but not directly imported" check.
- Gaps: path params not checked against any model, query model shape not checked, union members unchecked, @default not checked on arrays/models/unions/maps, parent/child field override not checked, header/error/status ranges, module/model name collisions, `any/json/long/time/bytes` accepted as types and as shadowable model names. See findings.
- Complexity: circular-inheritance check is O(n^3) in chain depth (3000-deep chain: 26.6 s; 1000-deep: 1.4 s) (F-FE-12).
- Works well: multiple errors per run (case 63 reports 3), correct file:line on semantic errors, "did you mean" suggestions, enum default membership, status-code validation, cross-file direct-import enforcement.

### internal/language (180 + 498 + 160 lines)
- `constants.go` is consumed only by `cmd/generate-language` (IDE spec output). `veld.go`/`imports.go` is a **second, regex/line-based mini-parser with its own error type and 0% test coverage** that nothing in the compiler uses. The lexer keyword table is hardcoded in `lexer.classifyWord`, not read from the spec, so the "single source of truth" claim in CLAUDE.md does not hold (F-FE-22). Spec lists `HEAD`, `OPTIONS`, and annotations (`required`, `primary`, `min`, `regex`, ...) that the parser rejects.

### internal/errors (128 lines)
- Typed `VeldError{Kind,File,Line,Message,Cause}` and `List`. **Not used by the pipeline**: `grep` finds no call to `NewParseError`/`NewValidationError` outside the package. Lexer/parser/loader/validator all return `fmt.Errorf` strings, so stage and position are only recoverable by regex (and `cmd/veld/util.go:60` does exactly that) (F-FE-06).

### internal/format (284 lines)
- Token-stream re-printer. Because the lexer discards comments and string quotes, **the output has no comments, no quotes around strings/imports, and glued tokens** (`description:Todo management`, `import@models/todo`). `fmt --write` on a valid example produces a file that no longer parses (F-FE-03). Also panics on `model }` (negative `strings.Repeat`). Idempotent only in the trivial sense that the corrupted output re-formats to itself.

### internal/lint (361 lines)
- Rules: unused-model, empty-module, empty-model, empty-constants, duplicate-route, duplicate-action, missing-description (module/action), deprecated-action/field, file-naming-convention. Warning/Error severities, errors sorted first. Deterministic; 20k issues sorts in about 2 s (insertion sort).
- `unused-model` ignores models used as field types, so nested models are falsely flagged (F-FE-20). `duplicate-route`/`duplicate-action` are unreachable because lint only runs after `Validate` passes, and the lint route key does not normalise `:param`.
- On `examples/*` the only hit is `unused-model: UpdateUserInput` (8 of 9 example contracts); node-react has no hits. microservices is workspace-only (no per-directory input).

### internal/diff (404 + 99 lines)
- API: `Diff(old,new) []Change` (Breaking | Added), `LoadLock/SaveLock/DeleteLock`. Sorted by (kind, path) after a stable sort so output is deterministic despite map iteration; lock is `json.MarshalIndent` of slices/structs (`ErrorStatuses` map keys are sorted by encoding/json) so the lock file is deterministic; temp-file + rename write.
- Detected (verified in `fz/internal/diff_test.go`): removed/added/renamed action, module removed/added, module prefix change, method change, path change, input field removed (breaking), field type change, optional->required on input, new required input field, required->optional on output, output array flip, model slot added/removed/changed.
- NOT detected: enum member removal or addition, enum/constants at all, changes inside nested model types, model removal when not referenced directly, middleware, errors list, union type changes, @default changes, WS stream/emit. Success-status change is classified non-breaking (Added).
- `allFields` recurses over `extends` with no cycle guard: a cyclic contract overflows the stack (fatal, unrecoverable) if Diff sees an unvalidated AST (F-FE-19).
- And see F-FE-02: in `veld generate` the lock is deleted before the gate runs, so none of this ever fires.

### internal/cache (103 lines)
- Content-hash (SHA-256) per absolute file path, stored in `.veld-cache.json`; legacy mtime map is discarded. Keys are only the `.veld` source files. Not keyed on: config file, CLI version, flags/targets, aliases, or whether the output directory still exists. `generate` (even with `--incremental`) calls `runClean` first, deleting the cache (F-FE-21). Tests 92% coverage; code itself is simple and correct.

## 2. Adversarial case results

Columns: validate rc; first output line; whether `file:line` (or `line N`) appears; whether a source snippet is printed (`validate` never prints one); number of `error:` lines; Go panic; `ast` rc; `lint` rc; fmt idempotent (run twice); whether `validate` accepts the fmt output (1 = formatter broke it). Raw files: `fe-audit/results/<case>.*`.

| case | validate rc | first line | file:line | snippet | #errs | panic | ast rc | lint rc | fmt idem | fmt-out validates rc |
|---|---|---|---|---|---|---|---|---|---|---|
| 01-empty | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 02-only-comments | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 03-unterminated-string | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 04-unterminated-block | 1 | Error: parsing main.veld: line 3: expected "}", got "" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 05-nested-generics | 1 | Error: parsing main.veld: line 2: expected ">", got "<" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 06-10000-fields | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 07-5mb-file | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 08-utf8-bom | 1 | Error: lexing main.veld: line 1: unexpected character '\ufeff' | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 09-crlf | 1 | error: main.veld:3: model "A", field "y": undefined type "badtype" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 10-nonascii-ident | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 11-emoji-description | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 12-tab-indented | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 13-field-type | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 14-field-default | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 15-action-delete | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 16-model-named-string | 1 | Error: parsing main.veld: model name: line 1: expected identifier, got "string" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 17-enum-same-as-model | 1 | error: main.veld:1: name collision: "X" is defined as both model and enum | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 18-duplicate-across-files | 1 | error: main.veld:2: duplicate model name: "Foo" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 19-self-import | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 20-import-cycle | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 21-wildcard-empty-dir | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 22-missing-import | 1 | Error: import "nope.veld": reading C:\Users\adham.zaineldin\AppData\Local\Temp\claude\D--University-Veld\1d57057e-5535-488c-a1fd-0b17c38b6e12\scratchp | no | no | 0 | no | 1 | 1 | idem | 1 |
| 22b-missing-alias-file | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 22c-typo-alias-dir | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 23-extends-self | 1 | error: main.veld:1: model "A": circular inheritance detected via "A" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 24-extends-chain-override | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 25-default-notmember | 1 | error: main.veld:5: model "A", field "r": @default(notAMember) is not a valid value for enum "Role" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 26-default-str-on-int | 1 | error: main.veld:2: model "A", field "n": @default for int must be a number, got "x" | yes | no | 1 | no | 0 | 1 | idem | 0 |
| 27-path-param-missing-field | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 28-same-route-two-modules | 1 | error: main.veld:12: route conflict: GET /same in B.Y overlaps with A.X | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 29-same-action-twice | 1 | error: main.veld:10: module "A": duplicate action name: "X" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 30-status-999 | 1 | error: main.veld:5: module "M", action "A": invalid success status 999 (must be 200–299) | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 31-status-abc | 1 | Error: parsing main.veld: output status after ':': expected HTTP status code, e.g. output: Foo:201 | no | no | 0 | no | 1 | 1 | idem | 1 |
| 31b-status-1p5 | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 32-method-FETCH | 1 | Error: parsing main.veld: line 6: expected HTTP method (GET, POST, PUT, DELETE, PATCH, WS), got "FETCH" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 33-path-no-slash | 1 | Error: parsing main.veld: action path: line 7: expected path (e.g. /auth/login), got "users" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 34-path-braces | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 35-query-array-nested | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 36-headers-unquoted-dash | 1 | Error: lexing main.veld: line 9: unexpected character '-' | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 36b-headers-quoted-dash | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 37-optional-default | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 38-array-of-maps | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 1 |
| 39-map-int-key | 1 | Error: parsing main.veld: line 2: Map key type must be "string", got "int" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 40-numeric-field-name | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "1" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 41-deep-braces-2000 | 1 | Error: parsing main.veld: line 1: unexpected token "{" | yes | no | 0 | no | 1 | 1 | idem | 0 |
| 41b-deep-braces-in-model | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "{" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 42-string-array-2d | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "[" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 43-field-named-date | 1 | Error: parsing main.veld: field name: line 2: expected identifier, got "date" | yes | no | 0 | no | 1 | 1 | NOTIDEM | 1 |
| 44-enum-http-values | 1 | Error: parsing main.veld: enum value: line 2: expected identifier, got "GET" | yes | no | 0 | no | 1 | 1 | NOTIDEM | 0 |
| 45-enum-explicit-values | 1 | Error: parsing main.veld: enum value: line 2: expected identifier, got "=" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 46-inline-collision-across-modules | 1 | error: duplicate model name: "CreateInput" | no | no | 1 | no | 0 | 1 | idem | 1 |
| 47-inline-headers-nonascii-action | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 48-string-escape | 1 | Error: lexing main.veld: line 2: unexpected character '\\' | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 49-union-undefined | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 50-map-undefined-value | 1 | error: main.veld:2: model "A", field "s": undefined Map value type "Nope" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 51-constants-type-mismatch | 1 | error: main.veld:4: constants "K".C: expected true or false, got "yes" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 52-comments-preserve | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 53-model-named-json | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 54-path-dup-param | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 55-get-with-input | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 56-module-order | 1 | Error: parsing main.veld: line 10: expected "action", got "prefix" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 57-error-status-weird | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 58-lone-cr | 1 | error: main.veld:1: model "A", field "y": undefined type "nope" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 59-decimal-default | 1 | error: main.veld:3: model "A", field "q": @default for float must be a number, got "x" | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 60-204-with-output | 1 | error: main.veld:5: module "M", action "A": status 204 (No Content) cannot have an output type | yes | no | 1 | no | 0 | 1 | idem | 1 |
| 61-trailing-slash-conflict | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 62-multi-error-parse | 1 | Error: parsing main.veld: line 3: expected ":", got "}" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 63-multi-error-sem | 1 | error: main.veld:2: model "A", field "x": undefined type "Nope1" | yes | no | 3 | no | 0 | 1 | idem | 1 |
| 64-keyword-as-enum-val | 1 | Error: parsing main.veld: enum value: line 2: expected identifier, got "model" | yes | no | 0 | no | 1 | 1 | idem | 1 |
| 65-model-named-any | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 66-wildcard-comment-swallow | 0 | ✓ Contract is valid | no | no | 0 | no | 0 | 0 | idem | 0 |
| 67-fmt-panic | 1 | Error: parsing main.veld: model name: line 1: expected identifier, got "}" | yes | no | 0 | YES | 1 | 1 | idem | 1 |

### Notes on specific rows
- 03, 04, 08, 22b, 22c, 31b, 66, 27, 49, 53, 54, 55, 61, 15, 24, 57 are the "silently accepted" cases; each maps to a finding below.
- 31b-status-1p5 (`output: Foo:1.5`): `validate` says valid and `ast` has no `successStatus` at all (silently dropped).
- 47: `validate` passes and the AST contains a synthetic model whose name is `\xef\xbf\xbd�ditHeaders` (invalid UTF-8, produced by byte-slicing `act.Name[:1]`).
- 66 (reproduced from `import @models/*` + a following model/module): `ast` lists only `Real`; `Foo` and module `M` are gone, `validate` says "Contract is valid".
- 67 (`model }`) is the only Go panic seen via the CLI: `veld fmt` panics ("strings: negative Repeat count"). Lexer, parser, validator, lint and diff survived 1.67M fuzz executions (no panic, no hang) on valid-by-construction pipeline input; the only crashes found are the formatter panic and the diff stack overflow on a cyclic AST (F-FE-19).
- Stack overflow with nesting: none (cases 41, 41b); the parser is not recursive.
- Position checks by eye (Part C): case 09 (CRLF) reports `main.veld:3` for `y: badtype` on line 3: **correct**. Case 25 reports `main.veld:5` for the `r: Role @default(notAMember)` field on line 5: **correct**. Case 62 (`x: bad bad` on line 2) reports `line 3: expected ":", got "}"`: **wrong line**, the second `bad` on line 2 is the culprit. Case 04 (`model A {` never closed) reports `line 3` (past EOF) instead of the unclosed `{` on line 1: **poor**. Case 58 (lone CR) reports line 1 instead of 3: **wrong**. Multiple errors: validator reports all (case 63: 3 errors); lexer/parser report exactly one. Stage distinction: only by message shape ("lexing X:", "parsing X:", "error: file:line:"), not by typed kind.

## 3. Findings

### F-FE-01: `import @alias/*` and `@alias/**` are lexed as a block comment and silently delete the rest of the file
- Category: lexer
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/lexer/lexer.go:240-251 and :253-257 (comment detection precedes the path-token branch at :315); parser consumer internal/parser/parser.go:210-215
- Trigger/reproducer: `fe-audit/cases/66-wildcard-comment-swallow/main.veld` containing `import @models/*` followed by `model Foo {...}` and `module M {...}`; run `veld.exe validate main.veld` and `veld.exe ast main.veld`. Also the shipped example `examples/microservices/veld/app.veld` (`import @services/**`): `veld ast veld/app.veld` returns `"models": null, "modules": null`.
- Expected vs actual: Expected the wildcard import to load all files in `models/` and keep the remainder of the file. Actual: the `/*` after `@models` opens a block comment that runs to the next `*/` or EOF; `Foo` and `M` vanish, validation prints "Contract is valid". The documented forms in docs/architecture/overview.md:55 and docs/DEVELOPER_GUIDE.md:189 (`import @models/*`) are the broken ones; `import /models/*` and `from @models import *` work.
- Impact: silent loss of models/modules, generated code missing endpoints with no diagnostic.
- Root cause: `/` followed by `*` is tested for comment start before the lexer knows it is inside an import path; `//` in paths has the same problem (`path: //x`, extra case e19).
- Recommendation: fix, make comment recognition context-aware (or lex `@alias/…` import paths as one token) and add an "unterminated block comment" error.
- Regression check: loader test with `import @models/*` + model afterwards asserting model count; lexer test that `@a/*` and `@a/**` yield TPath; microservices example must yield non-empty AST.
- Effort estimate: 2h-4h

### F-FE-02: `veld generate` deletes the lock file and output before the breaking-change gate, so breaking-change detection never fires (and an invalid contract wipes the previous output)
- Category: diff
- Evidence status: reproduced defect
- Severity (user impact): critical
- Security severity: none
- Location: cmd/veld/generate.go:758-760 (`runClean`), gate at :1019-1035, `computePreChanges` :1126-1140, `runClean` removing the lock at :51; same ordering in cmd/veld/watch.go:56
- Trigger/reproducer: project in `fe-audit/gate/`: `veld generate` once with a model `A{x:int,y:string}` and `POST /a`; then change to `A{x:int}` and `PUT /b` (field removed, method and path changed) and run `veld generate --strict`. Exit code 0, no breaking-change output; `.veld.lock.json` was deleted by `runClean` before `computePreChanges` read it. Second repro: put `model A extends A {}`; `veld generate` fails validation but `generated/`, `.veld.lock.json`, `.veld-cache.json` are already gone.
- Expected vs actual: CLAUDE.md says "pre-emit gate: interactive prompt by default, `--strict` exits 1 (CI), `--force` skips prompt". Actual: gate input (`LoadLock`) is always empty for a non-dry-run, non-incremental generate; `--strict` can never fail.
- Impact: the CI safety feature advertised as a differentiator is a no-op; developers lose their previous generated output when the contract has any error.
- Root cause: `runClean(rc)` unconditionally precedes the gate and also deletes the lock (generate.go:51), and runs before parse/validate.
- Recommendation: fix, validate first, run the gate against the lock, only then clean/emit; never delete the lock in generate.
- Regression check: CLI test: generate, introduce each breaking change, assert `generate --strict` exits non-zero and leaves output intact; test that a validation failure leaves `generated/` untouched.
- Effort estimate: 3h-6h

### F-FE-03: `veld fmt --write` corrupts valid files (drops comments, string quotes, spacing) and panics on `model }`
- Category: format
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/format/format.go:27-33 (formats from a token stream), :187-189 (default branch writes `tok.Value` of TString without quotes), :153 (`writeIndent(baseIndent+depth)` with negative depth), :113-131
- Trigger/reproducer: copy `examples/node-typescript/veld` to a temp dir and run `veld.exe fmt --write app.veld models/*.veld modules/*.veld`; then `veld validate app.veld` fails with `parsing ...todos.veld: line 4: expected "action", got "description"`. Output sample: `description:Todo management`, `import@models/todo`, `action   ListTodos\n {`. Comments are gone (`cases/52-comments-preserve`). Panic: `printf 'model }\n' > x.veld; veld fmt x.veld` (`cases/67-fmt-panic`).
- Expected vs actual: A formatter must preserve semantics and comments. Actual: the output no longer parses for every file that contains a quoted string or an import (6 of the 72 cases: 03, 11, 19, 20, 36b, 38 were valid before formatting; and every shipped example file).
- Impact: destructive in-place rewrite; the command is hidden but documented in `veld fmt` help and CLAUDE.md.
- Root cause: formatter is built on the lexer, which discards comments, quote characters and whitespace; no round-trip test.
- Recommendation: rewrite, build the formatter on a lossless CST/token stream (keep trivia and raw lexeme) and add parse-after-format and idempotency checks.
- Regression check: golden tests: for every `examples/**/*.veld`, `Format(Format(x)) == Format(x)`, `Parse(Format(x))` equals `Parse(x)`, and comments survive; fuzz target that `Format` never panics.
- Effort estimate: 12h-20h

### F-FE-04: Unterminated string and unterminated block comment are accepted silently
- Category: lexer
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:259-270 (string stops at newline, no error), :240-251 (comment loop ends at EOF, no error)
- Trigger/reproducer: `cases/03-unterminated-string/main.veld` (`description: "abc` newline) -> `validate` rc 0 and AST description `abc`. `cases/04-unterminated-block` shows the related unclosed-brace report. An unterminated `/*` hides all following source (also part of F-FE-01).
- Expected vs actual: Expected a lexer error at the opening quote/comment with line. Actual: valid contract.
- Impact: typos produce silently truncated or missing definitions.
- Root cause: no error branch when the closing delimiter is not found.
- Recommendation: fix, emit a positioned lexer error at the opening delimiter.
- Regression check: lexer tests for `"abc\n` and `/* x` expecting an error with the opening line.
- Effort estimate: 1h-2h

### F-FE-05: Diagnostics are first-error-only for lexer/parser, line-only, and sometimes point at the wrong line
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:363 (returns `errors[0]`), internal/loader/loader.go:94-101, internal/parser/parser.go (every `return ..., err`), internal/lexer/lexer.go:195-199 (`Token{Type,Value,Line}`)
- Trigger/reproducer: `cases/62-multi-error-parse/main.veld` (two independent syntax errors; only one reported, on line 3 while the first mistake is `x: bad bad` on line 2); `cases/58-lone-cr/main.veld` (reports line 1, true line 3); `cases/04-unterminated-block/main.veld` (reports non-existent line 3).
- Expected vs actual: Expected all syntax errors with line:column and a snippet (validate reports all semantic errors but no snippet; `validate` never prints one). Actual: one error, no column, wrong lines in three of the four tested syntax cases.
- Impact: slow edit-compile loops; IDE/LSP squiggles cannot be precise.
- Root cause: no panic-mode recovery in the parser; tokens carry no column/offset; lexer errors are collected then truncated.
- Recommendation: refactor, add column/offset to Token, return all lexer errors, add statement-level synchronisation in the parser.
- Regression check: golden diagnostic tests (line AND column) for 10 malformed files; test that two errors yield two diagnostics.
- Effort estimate: 10h-16h

### F-FE-06: Typed error package is dead code; errors are strings, stage is not machine-readable, file names are basenames
- Category: lexer
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/errors/errors.go:1-128 (unused by pipeline), internal/validator/validator.go:30-45 (`filepath.Base`), cmd/veld/util.go:45-75 (regex-like `SplitN(msg, ":", 3)` parsing, cache keyed by basename)
- Trigger/reproducer: `grep -rn NewParseError internal cmd` (no pipeline use); two files named `user.veld` in different directories each with an error -> the snippet printer indexes by basename and can show the wrong file's line.
- Expected vs actual: CLAUDE.md says every stage returns structured typed errors with file/line context. Actual: all stages return `fmt.Errorf`.
- Impact: tooling (LSP, JSON output) cannot consume diagnostics; ambiguous locations with duplicate basenames.
- Root cause: typed errors were added but never wired in.
- Recommendation: refactor, make every stage return `*VeldError` (with file, line, column, kind) and delete string parsing in util.go.
- Regression check: test that `errors.As(err, *VeldError)` holds for a lex, parse, load and validation failure and that Kind differs.
- Effort estimate: 6h-10h

### F-FE-07: Primitive-type and verb tokens cannot be used as field names, enum values, action names or model names
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/parser/parser.go:129-136 (`expectFieldName` allows only TIdent + `lexer.IsKeyword`), :328 (enum values require TIdent), :441, :747; internal/lexer/lexer.go:203-210 (`IsKeyword` omits TType* tokens)
- Trigger/reproducer: `cases/43-field-named-date/main.veld` (`date: date`, `int: int`) -> `field name: line 2: expected identifier, got "date"`. `cases/44-enum-http-values` (`enum Method { GET POST DELETE }`) and `cases/64-keyword-as-enum-val` (`model`, `action`) fail. `cases/16-model-named-string` fails.
- Expected vs actual: `type`, `class`, `default`, `import`, `delete` already work as field/action names (cases 13-15) but `date`, `datetime`, `string`, `int`, `uuid` do not, and an HTTP-method enum is impossible.
- Impact: a field called `date` is extremely common and cannot be expressed; error text does not say the name is reserved.
- Root cause: field-name rule is inconsistent with the contextual-keyword approach used elsewhere.
- Recommendation: fix, accept any word token in name positions and report a clear "reserved" error only where ambiguity exists.
- Regression check: parser tests for fields named after each primitive, enum values `GET`/`model`, action `delete`.
- Effort estimate: 2h-4h

### F-FE-08: No nested generics; `Map<string,int>[]` is represented ambiguously
- Category: parser
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: none
- Location: internal/parser/parser.go:514-556, internal/ast/ast.go:50-51
- Trigger/reproducer: `cases/05-nested-generics`, `cases/42-string-array-2d`, extra `Map<string, string[]>` and `List<List<string>>` all fail with "expected >" / "expected identifier, got [". `cases/38-array-of-maps` parses to `{"type":"Map","isArray":true,"isMap":true,"mapValueType":"int"}`.
- Expected vs actual: real APIs need `string[][]`, `Map<string,string[]>`. The Field struct (Type, IsArray, IsMap, MapValueType) cannot express nesting; flag combination for array-of-maps vs map-of-arrays is undefined for emitters.
- Impact: contracts cannot model common JSON shapes; emitters guess semantics for the IsMap+IsArray combination.
- Root cause: flat Field representation instead of a recursive type expression.
- Recommendation: refactor, introduce a recursive `TypeRef` in the AST (breaking AST version bump).
- Regression check: parser round-trip tests for `T[][]`, `Map<string,T[]>`, `Map<string,Map<string,T>>`, `Map<string,T>[]` with distinct AST assertions.
- Effort estimate: 12h-24h (touches every emitter)

### F-FE-09: Path parameters, path shape and routes are barely validated
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/validator/validator.go:298-322 (route conflicts), :525-535 (`normalizeRoutePath`); internal/parser/parser.go:149 (`{id}` rewrite)
- Trigger/reproducer: `cases/27-path-param-missing-field` (`PUT /users/:id` with input lacking `id`) valid; `cases/54-path-dup-param` (`/a/:id/:id`) valid; `cases/61-trailing-slash-conflict` (`/a` and `/a/`) valid; `cases/34-path-braces` valid; `cases/55-get-with-input` (GET with body model) valid; extra e18: `/x/:id/` vs `/x/:other/` is flagged (normalisation works for names).
- Expected vs actual: Expected an error/warning when a path parameter has no corresponding input/query field (or is declared twice), and a normalised comparison ignoring trailing slashes and the app-level prefix. Actual: none of these is checked; the conflict check also ignores the top-level `prefix:` and duplicate `:param` names.
- Impact: generated routes can reference parameters no type provides; duplicate-route detection has holes.
- Root cause: validator only compares (method, normalised path) strings.
- Recommendation: fix, add parameter-vs-input checks, duplicate-parameter check and trailing-slash normalisation.
- Regression check: validator tests for each of the five cases.
- Effort estimate: 4h-6h

### F-FE-10: `@default` validation is incomplete
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:194-196 (skipped for Map and union fields via the `continue` at :165 and :181), :416-467 (switch on `f.Type` ignores `IsArray`, model-typed fields, `long/time/bytes/any/json`)
- Trigger/reproducer: extra e01 `ids: int[] @default(5)` valid; e02 `b: B @default(5)` (B a model) valid; `cases/37-optional-default` (`x?: int @default(5)`) valid; `float` accepts any non-quoted token including identifiers (e.g. `@default(abc)` is an ident token). Works correctly: `cases/25` (enum member), `cases/26` (`"x"` on int), `cases/59` (`"x"` on float).
- Expected vs actual: default on an array/model/map should be rejected or typed; float/int defaults that are identifiers should be rejected.
- Impact: invalid defaults pass validation and break generated code.
- Root cause: switch only handles the base primitive kinds.
- Recommendation: fix, validate defaults against the full type shape.
- Regression check: table-driven validator test for default x type matrix.
- Effort estimate: 3h-5h

### F-FE-11: Undefined union member types are silently accepted
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:169-182; internal/parser/parser.go:561-573 (string literal and identifier both stored as bare text)
- Trigger/reproducer: `cases/49-union-undefined/main.veld` (`s: Foo | Bar` with neither defined) -> valid.
- Expected vs actual: error `undefined type Foo`. Actual: members that are not close to a known name are never reported (an error is only emitted when a Levenshtein suggestion exists).
- Impact: typos in unions become literal-string types in generated code.
- Root cause: the AST cannot distinguish `"DRAFT"` (literal) from `DRAFT` (type name) after parsing.
- Recommendation: fix, keep a literal/type flag per union member and validate type members.
- Regression check: union with unknown identifier errors; union with string literals passes.
- Effort estimate: 2h-3h

### F-FE-12: `extends` validation misses field-override conflicts; cycle check is cubic
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:122-150
- Trigger/reproducer: `cases/24-extends-chain-override` (child `B extends A` redeclares `x: int` as `x: string`, `C extends B`) -> valid. Performance: `fe-audit/perf/chain3000.veld` (3000-model extends chain) takes 26.6 s to validate, 1000 takes 1.4 s.
- Expected vs actual: Expected a conflict error (TS `interface B extends A { x: string }` will not compile; Go flattening picks one). Actual: accepted. Self and longer cycles ARE correctly detected (cases 23, extra).
- Impact: generated TS can fail to compile; slow validation for pathological inheritance depth (not realistic).
- Root cause: no field comparison across the chain; O(n) model scan per chain step per model.
- Recommendation: fix, build a name->model map once and compare overridden field types.
- Regression check: validator test for incompatible override; benchmark with a 2000-deep chain under 100 ms.
- Effort estimate: 2h-4h

### F-FE-13: Loader silently ignores missing import targets and falls back to loading whole directories
- Category: loader
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/loader/loader.go:246-249 (missing dir ignored), :285-355 (not-found file falls back to wildcard and `strings.Contains("model "+name)` text scan), :255/:273/:341 (self-edge in `FileImports`)
- Trigger/reproducer: `cases/22c-typo-alias-dir` (`import @modls/*`, dir does not exist) valid until a used type is missing; `cases/22b-missing-alias-file` (`import @models/nonexistent`) valid and `ast` shows `Real` loaded from the directory; `cases/21-wildcard-empty-dir` valid. A genuinely missing relative file (case 22) errors but with an absolute path and no line of the import.
- Expected vs actual: a typo'd import must be an error pointing at the import line. Actual: silent, or an unrelated directory is loaded.
- Impact: wrong files can be pulled in; errors surface later as unrelated "undefined type". Import cycles and self-imports are accepted silently (cases 19, 20); this is arguably lenient-by-design but undocumented.
- Root cause: fallback heuristics added to make `from @x import Name` work.
- Recommendation: fix, error on unresolved imports with the import line; keep the by-name lookup but make it exact (parse, not substring).
- Regression check: loader tests for missing alias dir, missing named file, and `model User` vs `model UserProfile` substring match.
- Effort estimate: 3h-5h

### F-FE-14: `veld validate` and `veld ast` ignore config `aliases`
- Category: loader
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: cmd/veld/contract.go:39 and :70 (`loader.Parse(path)` with no aliases) vs `veld lint` at :101 and `generate` (`loader.Parse(rc.Input, rc.Aliases)`)
- Trigger/reproducer: `fe-audit/alias/` with `{"aliases":{"m":"shared"}}` and `import @m/real`: `veld validate main.veld` fails with `undefined type "Real"`, `veld lint` loads it.
- Expected vs actual: same result from every command. Actual: validate/ast disagree with lint/generate.
- Impact: a contract that generates fine is reported invalid by `validate` (CI check, editor integration).
- Root cause: `validate` resolves only the input path, not the full config.
- Recommendation: fix, route all commands through one `loadContract(rc)` helper.
- Regression check: CLI test with custom alias across validate/ast/lint/generate.
- Effort estimate: 1h-2h

### F-FE-15: UTF-8 BOM (and UTF-16) files are rejected with a confusing message
- Category: lexer
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/loader/loader.go:90-94, internal/lexer/lexer.go:355-358
- Trigger/reproducer: `cases/08-utf8-bom/main.veld` -> `Error: lexing main.veld: line 1: unexpected character '﻿'`.
- Expected vs actual: BOM stripped. Actual: hard failure on a file that Windows Notepad / PowerShell 5 `Out-File` can produce.
- Impact: Windows users hit an invisible-character error.
- Root cause: no BOM handling.
- Recommendation: fix, strip a leading U+FEFF in loader and formatter; give a clear message for UTF-16.
- Regression check: loader test with BOM-prefixed file.
- Effort estimate: 0.5h-1h

### F-FE-16: Success/error status literals are silently dropped when not a plain integer
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/parser/parser.go:873-875, :898-900, :1045-1050 (`if code, convErr := strconv.Atoi(...); convErr == nil`); internal/validator/validator.go:286-294
- Trigger/reproducer: `cases/31b-status-1p5` (`output: Foo:1.5`) valid, AST has no `successStatus`; `cases/57-error-status-weird` (`errors: [bad:99999, ok:200]`) valid; `Foo:0` is indistinguishable from "unset". `cases/30` (999) and `cases/60` (204 with body) are correctly rejected.
- Expected vs actual: error on non-integer or out-of-range statuses; error statuses should be 400-599.
- Impact: wrong HTTP codes silently generated.
- Root cause: ignored `Atoi` error; error statuses are never range-checked.
- Recommendation: fix, return a parse error on conversion failure and range-check `ErrorStatuses`.
- Regression check: parser tests for `:1.5`, `:99999999999`, `:0`; validator test for error status 200.
- Effort estimate: 1h-2h

### F-FE-17: Synthetic models from inline `input/output/query/headers` collide and can contain invalid UTF-8
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/parser/parser.go:815, :851, :925, :961 (`strings.ToUpper(act.Name[:1]) + act.Name[1:]` byte slicing; the other three do not capitalise at all, so names are inconsistent)
- Trigger/reproducer: `cases/46-inline-collision-across-modules` (two modules each with `action Create { input: {..} }`) -> `error: duplicate model name: "CreateInput"` with **no file:line** (synthetic models have no Line). `cases/47-inline-headers-nonascii-action` -> valid, but AST model name is `\xef\xbf\xbd�ditHeaders`.
- Expected vs actual: inline types should be scoped per module/action, or the error should name both actions.
- Impact: common action names (`Create`, `List`) reused in two modules cannot use inline input; non-ASCII action names produce corrupted type names.
- Root cause: synthetic model names derive from action name only and are added to the global model list.
- Recommendation: fix, namespace synthetic names by module and slice by rune.
- Regression check: validator/parser tests for two modules sharing an action name with inline input and an accented action name.
- Effort estimate: 2h-4h

### F-FE-18: Breaking-change classifier misses enums, nested models, middleware, errors and unions; status change is non-breaking
- Category: diff
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: internal/diff/diff.go:97-191 (module/action only), :195-217 (diff only the model directly named by an action slot), :377-382 (`typeChanged` ignores `UnionTypes`), :182-188 (status -> `Added`); internal/diff/lock.go:36-40 (lock stores Modules and Models only, no Enums/Constants/Prefix)
- Trigger/reproducer: `fe-audit/fz/internal/diff_test.go` (`go test ./internal -run TestDiffScenarios -v` in `fe-audit/fz`). Results: enum member removed -> 0 changes; nested model `Inner{z:int}` -> `Inner{}` or `z:string` -> 0 changes; middleware added -> 0; errors list changed -> 0; union field added/changed -> only "field added"; status 201->200 -> 1 change classified **Added (non-breaking)**. Detected correctly: removed action, method change, path change, input field removed, input field type change, optional->required (input), new required input field, required->optional (output), output enum->string type change, action rename (remove + add).
- Expected vs actual: removal of an enum member, and any field change inside a nested model, are wire-breaking.
- Impact: even once F-FE-02 is fixed, the gate will pass real breaking changes (nested model and enum changes are the most common ones).
- Root cause: diff walks action slots one level deep and the lock omits enums.
- Recommendation: refactor, diff models/enums by name (full set) and recurse through type references.
- Regression check: the scenario table above as a permanent test with expected Breaking/Added classification.
- Effort estimate: 8h-14h

### F-FE-19: Diff overflows the stack on a cyclic `extends` when run on an unvalidated AST
- Category: diff
- Evidence status: suspected risk
- Severity (user impact): low
- Security severity: low
- Location: internal/diff/diff.go:355-375 (`allFields` recursion without visited set); caller cmd/veld/generate.go:1126-1139 (`computePreChanges` parses but does not validate)
- Trigger/reproducer: `fe-audit/fz/internal/cyc_test.go` (`go test ./internal -run TestCyc`) -> `fatal error: stack overflow` (not recoverable). The CLI path is currently masked by F-FE-02 (lock deleted first); once F-FE-02 is fixed, a stale lock plus a cyclic contract crashes `generate` instead of reporting the validation error.
- Expected vs actual: validation error. Actual (library level): fatal runtime error.
- Impact: crash instead of diagnostic after the F-FE-02 fix; low for a local CLI.
- Root cause: diff assumes a validated AST.
- Recommendation: fix, add a visited set in `allFields` and validate before diffing.
- Regression check: diff test with `A extends A` expecting no panic.
- Effort estimate: 0.5h-1h

### F-FE-20: Lint false positives and unreachable rules
- Category: lint
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/lint/lint.go:80-111 (`checkUnusedModels` counts only action slots and extends), :162-190 and :192-211 (duplicate-route/-action unreachable after `Validate`; route key does not normalise `:param`)
- Trigger/reproducer: `fe-audit/lintcase/m.veld` (`Out { inner: Inner }`, only `Out` used by an action) -> `unused-model: Inner`. Also the `UpdateUserInput` warning on 8 of the 9 shipped examples (the example does not use it as an input; that one is a true positive).
- Expected vs actual: models reachable through fields of used models are used.
- Impact: noise, and users are told to delete types needed by their API.
- Root cause: no transitive reachability and no `Stream`/`Emit` counting.
- Recommendation: fix, compute reachability from action slots through field types, extends, unions and maps.
- Regression check: lint test for nested model, map value, union member and stream/emit usage.
- Effort estimate: 2h-3h

### F-FE-21: Incremental cache is bypassed by `runClean`, keyed only on `.veld` content, and misses enum/constants-only changes
- Category: cache
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/cache/cache.go:65-82; cmd/veld/generate.go:758-760 (`runClean` deletes `.veld-cache.json` even with `--incremental`), :91-113 (only Models' and Modules' source files select target modules; an enum-only or constants-only file change selects none and then marks the cache up to date)
- Trigger/reproducer: `fe-audit/inc/`: `veld generate` then edit `role.veld` (enum only) and `veld generate --incremental` -> everything regenerated (cache wiped by `runClean`), so the incremental path was not exercised via `generate`; the `watch` path calls `runClean` only at start, so the enum-only miss is reachable there but was not run (source-supported).
- Expected vs actual: cache invalidation should include config, binary version, flags and output presence. Actual: only per-file SHA-256; if `generated/` is deleted manually the next incremental run reports "no changes".
- Impact: stale output in watch mode after enum/constants edits or tool upgrades.
- Root cause: cache key is file hashes only; module-selection logic ignores enums/constants.
- Recommendation: fix, add a hash of (version, config, flags) to the cache and treat enum/constants files as global invalidators.
- Regression check: watch/incremental test that editing an enum-only file regenerates dependent modules.
- Effort estimate: 3h-5h

### F-FE-22: "Single source of truth" language spec is not what the compiler uses; a dead second parser exists
- Category: language
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: internal/language/constants.go:29-110 (spec), internal/lexer/lexer.go:374-451 (hard-coded keyword table), internal/language/veld.go:82-200 (regex mini-parser, 0% coverage, unused)
- Trigger/reproducer: spec lists HTTP methods `HEAD`, `OPTIONS` and annotations `required, optional, primary, autoincrement, readonly, min, max, minLength, maxLength, regex` which the parser rejects (`unknown field annotation`, parser.go:679-681; `isHTTPMethod` parser.go:1145); parser accepts `serverSet` which the spec lacks. Validator treats `long, time, bytes, any, json` as primitives (validator.go:12-26) although CLAUDE.md's type table omits them, and a model named `json` or `any` is accepted and shadows the primitive (cases 53, 65).
- Expected vs actual: CLAUDE.md says internal/language is the single source of truth and CI checks generated plugin files against it; the plugins therefore advertise syntax the compiler rejects.
- Impact: editor completion/highlighting diverges from the compiler.
- Root cause: constants were never wired into lexer/parser/validator.
- Recommendation: refactor, make lexer/parser/validator derive keyword, type, method and annotation sets from `internal/language`, and delete `veld.go`/`imports.go`.
- Regression check: test that every spec annotation/method parses and every parser-accepted annotation appears in the spec.
- Effort estimate: 6h-10h

### F-FE-23: Directive handling is lax and order-dependent
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/parser/parser.go:700-726 (module header order: description, then prefix, then baseUrl), :763-1058 (action directives assigned unconditionally)
- Trigger/reproducer: extra e15 (`output: A:200 output: A:201`) and e16 (`method: GET method: POST path: /x path: /y`) valid with last-wins; `cases/56-module-order` and e14 (`prefix` or `description` after the first action, or `description` after `prefix`) fail with `expected "action", got "prefix"`.
- Expected vs actual: duplicates should error; header directives should be accepted in any order before actions.
- Impact: silent overrides; confusing error text for reordered headers.
- Root cause: fixed-sequence header parsing and no "seen" set.
- Recommendation: fix, loop over header directives with a duplicate check.
- Regression check: parser tests for duplicate `method`/`output` and reordered module header.
- Effort estimate: 2h-3h

### F-FE-24: Strings have no escape sequences; descriptions cannot contain a double quote
- Category: lexer
- Evidence status: design limitation
- Severity (user impact): low
- Security severity: none
- Location: internal/lexer/lexer.go:259-270
- Trigger/reproducer: `cases/48-string-escape/main.veld` (`description: "say \"hi\""`) -> `lexing main.veld: line 2: unexpected character '\\'`. Related: `cases/36-headers-unquoted-dash` (`Idempotency-Key: string`) -> `unexpected character '-'` (valid only quoted).
- Expected vs actual: `\"`, `\\`, `\n` escapes (or documented alternative); a clearer message for unquoted header names.
- Impact: documentation text and OpenAPI descriptions cannot contain quotes; emitters that embed descriptions in source must escape on their own.
- Root cause: minimal string lexer.
- Recommendation: fix, add escape handling and a hint in the header-name error.
- Regression check: lexer tests for escapes and an emitter test with quotes in a description.
- Effort estimate: 1h-2h

### F-FE-25: Query/headers/input shapes are unconstrained
- Category: validator
- Evidence status: design limitation
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/validator.go:216-247 (only name existence)
- Trigger/reproducer: `cases/35-query-array-nested` (query model with `string[]` and a nested model field) valid; `cases/55-get-with-input` (GET with `input`) valid; `cases/36b-headers-quoted-dash` header types unconstrained (`Authorization?: string` ok; a model-typed header would also pass).
- Expected vs actual: query params and headers should be scalars (or arrays of scalars with a documented encoding); GET/DELETE bodies should warn.
- Impact: emitters must decide how to serialise nested query objects; results differ per language.
- Root cause: no contract on what each slot may contain.
- Recommendation: fix, document and enforce scalar-only headers and scalar/array-of-scalar query fields; warn on GET input.
- Regression check: validator tests per slot.
- Effort estimate: 3h-5h

### F-FE-26: Constants validation is weak and loses literal quoting
- Category: validator
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/parser/parser.go:406-423 (TString and TNumber both stored as raw text), internal/validator/validator.go:537-577
- Trigger/reproducer: `cases/51-constants-type-mismatch/main.veld`: `A: string = 42` and `B: int = "5"` are accepted; only `C: bool = "yes"` is reported (and `bool = "true"` would pass). `uuid/date/datetime/decimal` values are never format-checked.
- Expected vs actual: type/literal mismatch errors for A and B.
- Impact: emitted constants can be wrongly typed (`const A: string = 42`).
- Root cause: AST stores `Value` without a literal kind.
- Recommendation: fix, store a literal kind and check it per declared type.
- Regression check: validator tests for mismatched literal kinds.
- Effort estimate: 2h-3h

### F-FE-27: Workspace `consumes` has no cycle detection and `Foo` model/module name collision is not checked
- Category: validator
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: none
- Location: internal/validator/workspace.go:10-65 (self and unknown only; CLAUDE.md says "circular"), internal/validator/validator.go:203-207 (module names only checked against modules)
- Trigger/reproducer: `grep -rn -i circular cmd internal` shows no consume-cycle logic; `A consumes B, B consumes A` passes `ValidateWorkspaceConsumes`. `extra/e06.veld` (`model Foo` and `module Foo`) valid.
- Expected vs actual: documented cycle check; model/module/enum collisions in generated type/route namespaces are plausible.
- Impact: low; cycles may only matter for generation order, name collisions for emitters (not tested here).
- Root cause: checks never implemented.
- Recommendation: fix, add DFS cycle detection and a cross-kind name collision check.
- Regression check: workspace tests for A<->B and A->B->C->A.
- Effort estimate: 1h-2h

### F-FE-28: Positions are lines only; no column/offset, so no precise range diagnostics for editors
- Category: lexer
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: none
- Location: internal/lexer/lexer.go:195-199; internal/ast/ast.go (`Line int` fields only)
- Trigger/reproducer: every diagnostic above carries `file:line` at most; there is no column in any error. Rune vs byte vs UTF-16: lexer indexes `[]rune`, line counting is `\n` only, nothing records columns, so any future LSP/VS Code range would have to re-scan the text to get UTF-16 columns.
- Expected vs actual: editors need range diagnostics.
- Impact: blocks quality LSP and IDE work.
- Root cause: Token has no offsets.
- Recommendation: refactor, add `Offset`/`Col` to Token and `Pos` on AST nodes (see F-FE-05).
- Regression check: golden test with non-ASCII text asserting columns.
- Effort estimate: included in F-FE-05 (4h-6h extra for AST nodes)

### F-FE-29: AST JSON emits `null` for empty collections and has a fixed version
- Category: parser
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: none
- Location: internal/ast/ast.go:5-13, internal/parser/parser.go:27
- Trigger/reproducer: `cases/01-empty` `veld ast main.veld` -> `{"astVersion":"1.0.0","models":null,"modules":null}`.
- Expected vs actual: `[]`. Actual: `null`; AST consumers (the stated pipeline contract is "only AST JSON passes between stages") must special-case null.
- Impact: low.
- Root cause: nil slices with no `omitempty`/initialisation.
- Recommendation: fix, initialise slices before encoding.
- Regression check: golden JSON for an empty file.
- Effort estimate: 0.5h

## 4. Things that work correctly (retain evidence)
- Empty file, comments-only file, CRLF, tabs, emoji in descriptions, 10,000-field model, 100,000-field model, 5 MB file, 10,000 models: all handled in under 1 s with no panic.
- No stack overflow for 2000/200000-deep `{`; parser is non-recursive.
- 1.67M fuzz executions of lexer+parser+validator+lint+diff (valid-AST path) produced no panic or hang; the lexer never panics on arbitrary bytes.
- Validator: reports all semantic errors, correct file:line on field/action errors, did-you-mean suggestions, enum default membership, circular and self inheritance detection, success-status range and 204 check, cross-module route conflicts, per-file direct-import visibility, duplicate names at every level.
- Lock file is deterministic (struct/slice JSON, sorted map keys, stable sort of changes).
- Package tests pass; coverage: lexer 61%, parser 54%, loader 30%, validator 54%, format 64%, lint 84%, diff 55%, cache 92%, language 0%.

## 5. Overall assessment
- Lexer: retain, with fixes F-FE-01, 04, 15, 24 and column tracking (F-FE-28). It is small, linear and robust.
- Parser: retain and refactor. The grammar is simple and non-recursive (safe), but needs error recovery, positions, a recursive type representation and the name-position fixes (F-FE-05, 07, 08, 16, 17, 23).
- Validator: retain; add the missing semantic checks (F-FE-09 to 12, 25 to 27).
- Formatter: rewrite (F-FE-03). Diff/gate: refactor (F-FE-02 and F-FE-18 are the highest-impact defects found). Typed errors and the language spec need wiring, not rewriting.
