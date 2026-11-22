# 08. Compiler, language and CLI plan

Evidence: compiler-frontend audit track (72 adversarial cases, 1.67 million Go-fuzz executions over lexer, parser, validator, lint and diff with no panic or hang; F-FE-01 to F-FE-29), CLI audit (F-CLI items), lead-reviewer reproduction of F-FE-01 (`import @models/*` makes `veld validate` report a file with zero models as valid).

## 8.1 What is retained and why

| Component | Verdict | Evidence |
|---|---|---|
| Lexer (`internal/lexer`) | retain, fix | linear, no recursion, fuzz-clean; defects are local: wildcard import lexed as a block comment (`lexer.go:240-257` before the path branch at `:315`, F-FE-01), unterminated strings and comments accepted (F-FE-04), no escapes (F-FE-24), BOM rejected (F-FE-15), first error only and line-only positions (F-FE-05, F-FE-28) |
| Parser (`internal/parser`) | retain, refactor | small grammar, non-recursive on braces (200,000-deep `{` rejected cleanly); needs error recovery, spans, a recursive `TypeRef` (today `IsArray`/`IsMap` flags, `Map<string,int>[]` sets both, F-FE-08), reserved-word handling for field and enum names (F-FE-07), header identifiers (F-MULTI-01 at `parser.go:961`), directive order and duplicates (F-FE-23) |
| Validator (`internal/validator`) | retain, extend | reports all errors with correct `file:line`, suggestions, enum-default membership, extends cycles, cross-module route conflicts; gaps are missing checks: path params against fields (F-FE-09), `@default` on arrays and maps (F-FE-10), undefined union members (F-FE-11), extends field conflicts and cubic cycle check (F-FE-12), reserved model names (F-NODE-14) |
| Loader (`internal/loader`) | retain, fix | missing imports silently ignored with directory fallback (F-FE-13), `validate` and `ast` ignore aliases (F-FE-14) |
| Typed errors (`internal/errors`) | wire in | unused today (F-FE-06); becomes `internal/diag` |
| Language tables (`internal/language`) | wire in | not consumed by the compiler (F-FE-22, F-LSP-16); dead regex parser deleted |
| Formatter (`internal/format`) | rewrite | `--write` drops comments and quotes and glues tokens; corrupts the shipped example; panics on `model }` (F-FE-03) |
| Diff and lock (`internal/diff`) | refactor | classifier misses enums, nested models, middleware, unions; status change non-breaking (F-FE-18); stack overflow on cyclic extends (F-FE-19); gate dead because the lock is deleted first (F-FE-02, F-CLI-01) |
| Lint | fix | unreachable error-level rules, false positive on nested models (F-FE-20, F-CLI-12) |
| Cache | simplify | wiped by `runClean`; keyed on `.veld` content only (F-FE-21); incremental mode is not justified for inputs that compile in under a second (5 MB file validates in under 1 s); remove `--incremental` and the cache file |

No frontend rewrite is warranted. The audit's own words: the validator "is the strongest piece".

## 8.2 Pipeline and responsibilities (target)

```
sources + overlay ─► syntax.Parse (per file; error recovery; spans)
                  ─► semantic.Load (imports, aliases, packages from lockfile; cycle detection)
                  ─► semantic.Resolve (symbols, inheritance, references)
                  ─► semantic.Validate (all rules; all diagnostics collected)
                  ─► semantic.Lower (AST -> ir.Program; normalization; source map)
                  ─► compiler.Result {Program, Diagnostics}
CLI: generate ─► tools/diff against lock ─► emit.Run (built-ins + plugins) ─► manifest ─► lock written
LSP: Compile with overlay ─► diagnostics; symbols from Program for hover/definition/completion
```

Responsibilities:

- Parsing: one file at a time, never I/O; recovers at the next `model`/`enum`/`module`/`action`/`}` boundary and continues, so one file yields many diagnostics.
- Spans: every token and AST node records byte offset, line and column (0-based UTF-8 byte column; converters for UTF-16 live in `diag`).
- Name resolution: a symbol table per program; imports resolve to files through aliases and packages; a missing import is an error, never a directory scan.
- Type checking: `TypeRef` tree; scalars from `internal/language`; `Map` keys limited to `string` in lang 1.0; nested arrays and maps allowed (`string[][]`, `Map<string, T[]>`); recursion allowed through models only.
- Normalization: inheritance flattened with provenance; path segments parsed; status codes defaulted (POST 201, DELETE without output 204, else 200) and recorded explicitly in the IR so emitters never re-derive them; header wire names kept with a derived identifier.
- Diagnostics: `diag.Diagnostic{Code, Severity, Span, Message, Hints, Stage}`; codes stable (`E0101` unknown type ...); machine-readable with `--json`; text format `file:line:col: error[E0101]: message` plus a source snippet.
- Dependency graph: file import graph and workspace `consumes` graph both checked for cycles (F-FE-27).
- Emission: chapter 09 and ADR-007.

## 8.3 IR v1 (excerpt of the JSON shape)

```jsonc
{
  "irVersion": "1.0", "langVersion": "1.0",
  "enums": [{ "name": "Role", "members": ["admin", "user"], "span": {"file": "veld/models/user.veld", "line": 3, "col": 1} }],
  "models": [{
    "name": "User", "extends": ["BaseEntity"], "description": "...",
    "fields": [
      { "name": "id", "wireName": "id", "type": { "kind": "scalar", "scalar": "uuid" }, "optional": false, "from": "BaseEntity" },
      { "name": "tags", "wireName": "tags", "type": { "kind": "array", "elem": { "kind": "scalar", "scalar": "string" } }, "optional": true },
      { "name": "role", "wireName": "role", "type": { "kind": "ref", "ref": "Role" }, "default": { "kind": "enum", "value": "user" } },
      { "name": "meta", "wireName": "meta", "type": { "kind": "map", "key": { "kind": "scalar", "scalar": "string" }, "value": { "kind": "scalar", "scalar": "string" } } }
    ]
  }],
  "services": [{
    "name": "Users", "prefix": "/api",
    "operations": [{
      "name": "GetUser", "method": "GET", "path": "/users/:id",
      "segments": [{ "kind": "static", "value": "users" }, { "kind": "param", "name": "id", "type": { "kind": "scalar", "scalar": "uuid" } }],
      "query": null, "headers": [{ "wireName": "Idempotency-Key", "name": "idempotencyKey", "type": { "kind": "scalar", "scalar": "string" }, "optional": false }],
      "input": null, "outputs": [{ "status": 200, "type": { "kind": "ref", "ref": "User" } }],
      "middleware": ["AuthGuard"], "deprecated": "", "span": { "file": "...", "line": 12, "col": 3 }
    }]
  }]
}
```

The JSON Schema for this document is generated from the Go types and published at `internal/ir/schema/ir-1.0.json`; the TypeScript SDK's types are generated from the schema.

## 8.4 Language features in the graduation release (lang 1.0)

| Construct | Semantics in 1.0 | Status |
|---|---|---|
| `model` with scalar fields `string int float decimal bool date datetime uuid` | `int` is a 64-bit integer, documented as 53-bit safe for JavaScript clients; `decimal` is a decimal string on the wire; `date` is `YYYY-MM-DD`; `datetime` is RFC 3339 with offset; `uuid` is RFC 4122 text | keep; fix date/datetime distinction in Go |
| `field?: T` | absent allowed; `null` is not a valid value for an optional field (validators reject it) | keep; enforce |
| `T[]`, `Map<string, T>`, nesting | arrays of arrays and maps of arrays allowed | extend (F-FE-08) |
| `extends` | single inheritance, no field override with a different type, no cycles | keep; add override check |
| `enum` | string members; wire value equals the member name; member names may be any identifier including keywords | keep; fix F-FE-07 |
| `@default(v)` | typed literal or enum member; applies to generated validators and documentation, not to databases | keep; complete checks |
| `@deprecated "msg"` on fields and actions | emitted as doc annotations | keep |
| `module { prefix, description }` and `action { method, path, input, output(:status), query, headers, middleware }` | one success output per action; `headers` keys are wire names with derived identifiers; `middleware` is a named hook the adapter wires; path params typed from the input or query model or default `string` | keep; define rules; fix header identifiers |
| `import @alias/path`, `import "./relative.veld"`, `import @alias/*` | all three forms parse; wildcard is a real token, not a comment | fix F-FE-01 |
| constants | typed literals | keep; tighten |
| `version` header statement (`veld 1`) | optional | add |
| Deferred from 1.0 | union types (parser accepts them today with weak validation; freeze behind a warning), WebSocket actions (`WS`, `stream:`, `emit:`), error output declarations (`errors:`), per-field constraints (min, max, pattern), nullable as distinct from optional, custom scalars, generics beyond `Map`/arrays, HEAD/OPTIONS methods (parser accepts; emitters inconsistent) | spec says "reserved for a future version"; compiler emits `W_DEFERRED_FEATURE` and emitters refuse |

## 8.5 Reusable compiler APIs, determinism and configuration

- `compiler.Compile` is the only entry point used by `veld validate`, `generate`, `lint`, `fmt --check`, `diff`, `lsp` and tests. `veld validate` therefore honours aliases (fixes F-FE-14) and workspace config exactly as `generate` does.
- Cancellation: every public function takes `context.Context`; the LSP cancels superseded compiles.
- Incremental compilation: not justified (inputs compile in well under a second); removed along with `.veld-cache.json`.
- Determinism: the IR has no map-ordered collections; emitters iterate sorted slices; a CI test generates every fixture twice and on two operating systems and byte-compares (F-NODE determinism finding reproduced in `types/index.ts`).
- Portable paths: all paths in IR and manifests are forward-slash relative; absolute paths appear only in `--verbose` output.
- Environment independence: generated output never contains absolute paths, timestamps, user names, hostnames or toolchain versions beyond the Veld version header. Local toolchain diagnostics (`veld doctor`) are printed, never embedded.
- Configuration: nested form wins over flat (fixes F-CLI-07, matching the documentation); unknown keys produce a warning (F-CLI-09); `--out` overrides every configured output directory (F-CLI-08); `$schema` validated; comments and trailing commas rejected with line numbers; config discovered in `.` then `./veld` then upward to the repository root (documented).
- Machine-readable output: `--json` on `validate`, `generate`, `lint`, `diff`, `doctor`, `--version`; `NO_COLOR` and non-TTY detection; exit codes `0` ok, `1` diagnostics, `2` usage or config error, `3` plugin failure, `4` breaking change under `--strict`.
- API compatibility checks: `veld diff` uses the IR (not the AST) so enums, nested models, headers, middleware and status codes are compared (fixes F-FE-18); classification table published in the docs.

## 8.6 Safe generation (ADR-007) and the CLI surface

| Behaviour | Target |
|---|---|
| Order | compile -> diff against lock -> gate -> emit to staging -> swap -> manifest -> lock -> hooks |
| Manifest | `.veld-manifest.json`: versions, emitter list, files with SHA-256 |
| Ownership conflicts | a generated path that exists but is not in the previous manifest stops generation (`E_UNMANAGED_FILE`), unless `--overwrite-unmanaged` |
| Interrupted generation | staging directory is removed on failure; previous output untouched |
| Dry run | full pipeline, prints planned files and a diff; writes nothing, not even cache or lock (fixes F-CLI-06) |
| Clean | deletes manifest-listed files and then empty directories; refuses unmanaged non-empty directories (fixes F-CLI-03) |
| Hand-written code | never inside the output directory; the generated README says so; `veld diff` lists hand-modified generated files |
| `init` | never overwrites existing files (`O_EXCL`), fixes F-CLI-05 |
| `watch` | debounced compile; on error prints diagnostics and keeps the previous output (fixes F-NODE-03); handles SIGTERM |
| hooks | `postGenerate` run through the platform shell (`cmd /C` on Windows, fixes F-CLI-10); failure exits non-zero; `--no-hooks` |
| `doctor` | registered (fixes F-CLI-04); reports toolchains, plugins, config, tier of each configured emitter |
| Registry commands | top-level aliases `veld login/logout/push/pull` kept thin over `veld registry ...` (fixes F-CLI-14) |
| Version | from build info when not stamped (fixes F-CLI-13) |

## 8.7 Formatter rewrite (small, self-contained)

Lossless token stream (comments and strings preserved verbatim), indentation by block depth, alignment of field types within a model, idempotency test on every conformance fixture, `fmt --check` for CI. Estimate 12 to 20 h (T1 or T5), scheduled in M4 because it is not on the critical path; until then `veld fmt --write` is disabled with a message (it corrupts files today).

## 8.8 Work breakdown (compiler and CLI)

| Item | Finding(s) | Estimate | Sprint | Owner / reviewer |
|---|---|---|---|---|
| Stop `runClean` before compile; staged writer; manifest; path safety; dry-run and clean semantics | F-NODE-01..04, 16, 17; F-CLI-01..03, 05, 06 | 20 to 30 h | S1 to S2 | T2 / T1 |
| Wildcard import token; unterminated string/comment errors; BOM; escapes | F-FE-01, 04, 15, 24 | 4 to 8 h | S1 | T1 / T5 |
| Header identifier derivation at parser/IR level | F-MULTI-01 | 6 to 12 h | S1 | T1 / T4 |
| Structured diagnostics (`diag`), spans, error recovery, all-errors reporting | F-FE-05, 06, 28; F-LSP-04 | 16 to 28 h | S3 to S4 | T1 / T5 |
| `TypeRef` tree, nested generics, reserved-word identifiers | F-FE-07, 08 | 12 to 24 h | S3 to S5 | T1 / T2 |
| IR v1, lowering, JSON Schema, `veld ir`, conformance suite | ADR-002 | 40 to 60 h | S3 to S5 | T1 / T4 |
| Validator extensions (path params, defaults, unions, extends conflicts, reserved names, workspace cycles) | F-FE-09..12, 27; F-NODE-14 | 12 to 20 h | S4 to S5 | T1 / T3 |
| Loader strictness and alias consistency | F-FE-13, 14 | 4 to 7 h | S2 | T1 / T5 |
| `internal/language` as the real source of truth plus parser conformance test | F-FE-22, F-LSP-16 | 6 to 10 h | S4 | T5 / T1 |
| Diff on IR; classifier completeness; gate ordering; lock never deleted | F-FE-02, 18, 19 | 10 to 18 h | S5 (after IR) | T1 / T2 |
| Config precedence, unknown keys, `--out`, JSON errors with line numbers | F-CLI-07, 08, 09 | 5 to 9 h | S2 | T5 / T1 |
| CLI: `--json`, exit codes, `NO_COLOR`, `doctor`, aliases, version, hooks | F-CLI-04, 10, 11, 13, 14 | 10 to 16 h | S8 to S9 | T5 / T2 |
| Formatter rewrite | F-FE-03 | 12 to 20 h | S10 to S11 | T1 / T5 |
| Remove incremental cache and `internal/mock`; freeze deferred syntax behind warnings | F-FE-21, F-MULTI-14 | 3 to 5 h | S3 | T1 / T4 |

Total: 160 to 267 h. The IR and diagnostics items are the critical path for the plugin system (chapter 10) and the LSP (chapter 11).
