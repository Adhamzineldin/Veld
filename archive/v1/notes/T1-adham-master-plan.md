# Adham (T1) · Technical Lead / Compiler / IR / Rust — Master Plan

> **Role:** T1 · Technical Lead, Compiler, IR, Rust  
> **Current Milestone:** M1 (2 Nov – 29 Nov 2026) — sprint S1–S2  
> **Today:** 2026-11-19 (S2, 11 days into M1)  
> **Source-of-truth docs:** `docs/review-2026-10/00-INDEX.md`, `docs/review-2026-10/08-compiler-language-cli-plan.md`, `docs/review-2026-10/19-expected-state-at-submission.md`, `csv/backlog.csv`

---

## Audit Summary (snapshot at 2026-11-19)

### What's already done (M1 so far)
| Item | Status |
|---|---|
| Staged writer + manifest + output safety (B-006, B-008, B-009) | ✅ Done |
| Breaking-change gate fix — lock no longer deleted first (F-NODE-04) | ✅ Done |
| Determinism — `types/index.ts` stable order (B-012) | ✅ Done |
| Node routes static-before-param (B-015, F-NODE-05) | ✅ Done |
| `tsc --strict` clean — node backend + TS client + React hooks (B-015) | ✅ Done |
| `veld init` no longer overwrites existing files (F-CLI-05) | ✅ Done |
| `veld generate --dry-run` writes nothing | ✅ Done |

### Critical gaps that are T1's direct responsibility
| Finding / Backlog ID | Gap | Risk |
|---|---|---|
| F-FE-01, B-010 | `import @models/*` wildcard treated as block comment — zero models loaded silently | P0 |
| F-FE-04, B-010 | Unterminated strings/comments accepted silently | P0 |
| F-FE-15, B-010 | BOM at file start rejected | P0 |
| F-FE-24, B-010 | No string escapes in lexer | P0 |
| F-MULTI-01, B-011 | Header wire names not derived into language identifiers (`Idempotency-Key` → `idempotencyKey`) | P0 |
| F-FE-05/06/28, B-023 | Lexer/parser reports only the **first** error; no byte/column offsets | P1 |
| F-FE-07/08, B-024 | Reserved words unusable as field names; `Map<string,int>[]` sets both `IsArray` and `IsMap` (broken TypeRef) | P1 |
| F-FE-22, B-029 | `internal/language` **not consumed** by lexer/parser/LSP — drift risk | P1 |
| F-FE-13/14, B-010 | Missing imports silently ignored; `validate`/`ast` ignore aliases | P1 |
| ADR-002, B-025 | **No IR exists** — every emitter re-derives semantics from raw AST | P0 |
| F-FE-09–12, B-028 | Validator missing: path-param vs field check, `@default` on arrays/maps, undefined unions, extends conflicts | P2 |
| F-FE-19, B-047 | Stack overflow on cyclic `extends` in diff | P2 |
| F-FE-03, B-050 | Formatter corrupts files and panics on `model }` | P1 |

### Architecture problems (not yet fixed)
- `internal/language` exists but is **not wired** into the lexer, parser, or LSP — it's a dead parallel. Its regex-based parser duplicates the real one. The lexer hard-codes the same keyword lists as `language/constants.go`.  
- `internal/errors` (structured `VeldError`) exists but is **unused** by the pipeline — the pipeline returns `fmt.Errorf` strings throughout.  
- **No `internal/ir`, no `internal/diag`, no `internal/semantic`, no `compiler.Compile`** — these are core M2 deliverables but T1 must start the design now.  
- 225 `os.WriteFile` calls scattered across emitters (Mohamed/T2 is migrating TS ones; T1 must review).  
- `internal/language/veld.go` has a **second regex-based parser** that is completely separate from the real parser — dead code drift risk.

---

## Goals by Phase

### M0 ✅ (21 Oct – 1 Nov 2026) — retrospective
- Architecture confirmed (ADR-001 through ADR-010 reviewed).
- Compiler conformance examples seeded in `cmd/veld/testdata/node/app.veld`.
- Compiler/emitter API surface identified.

---

### M1 (2 – 29 Nov 2026) · **IN PROGRESS** — sprints S1–S2

> **Exit criteria:** wildcard import works; headers derived correctly; lexer stable; B-010, B-011 green.

#### B-010 · Lexer fixes (P0 — must ship before Nov 29)

**Problem:** `import @models/*` hits the block-comment branch at `lexer.go:240-257` before the path branch at `:315`. The `*` after `/` triggers the `/*` comment parser.

**Fix plan:**
1. In `Tokenize()`, re-order the `/` handling: check for `/*` comment ONLY if the character after `/` is `*` **and** we are NOT immediately following `import`/`from`. Better: simply move the path-token branch (`ch == '/'`) before the comment branches, and only treat `/*` as a comment when `l.pos+1 < len(l.source) && l.source[l.pos+1] == '*'` and the preceding non-whitespace token is NOT `TAt`, `TImport`, `TFrom`, `TIdent`.
2. Add `TStar` as a valid token after `TAt <ident>` in the import path context.
3. Add unterminated-string error (currently the lexer silently accepts it — the closing-`"` check at `lexer.go:266` only skips the `"` if present).
4. Add BOM stripping at `New()`: `if len(source) >= 3 && source[0] == '\xEF' && ...` strip it.
5. Add basic string escape handling (`\"`, `\\`, `\n`, `\t`) in the string-literal branch.
6. Add fixture: `import @models/*` → loads the directory.

**Files:** [`internal/lexer/lexer.go`](file:///G:/University/Graduation/Veld/internal/lexer/lexer.go), [`internal/lexer/lexer_test.go`](file:///G:/University/Graduation/Veld/internal/lexer/lexer_test.go)

**Commit message:** `B-010: fix wildcard import, unterminated string, BOM, string escapes (F-FE-01, F-FE-04, F-FE-15, F-FE-24)`

#### B-011 · Header identifier derivation (P0)

**Problem:** `headers:` block field names are wire names (e.g., `Idempotency-Key`) but emitters need the derived identifier (`idempotencyKey`). Today emitters receive the wire name and generate invalid identifiers.

**Fix plan:**
1. In `parser.go`, when parsing `headers: { ... }` inside an action, store the raw field name as `wireName` and derive the identifier with `toCamelCase(wireName)`.
2. Add `WireName string` to `ast.Field`.
3. Each emitter uses `Field.WireName` for HTTP headers and `Field.Name` (camelCase) for the Go/TS/Python identifier.
4. Add a conformance fixture with `Idempotency-Key` header; test that Node, Python, Go all compile.

**Files:** [`internal/ast/ast.go`](file:///G:/University/Graduation/Veld/internal/ast/ast.go), [`internal/parser/parser.go`](file:///G:/University/Graduation/Veld/internal/parser/parser.go)

**Commit message:** `B-011: header wire name + derived identifier; fixture with Idempotency-Key (F-MULTI-01)`

#### Loader strictness · Missing imports as errors (P1)

**Problem:** `loader.go` silently falls back to directory scan when an import path isn't found. `validate` and `ast` commands ignore aliases.

**Fix plan:**
1. In `internal/loader/loader.go`, return an error when an explicitly named import file does not exist (no silent directory fallback).
2. Ensure `veld validate` runs through the same loader as `veld generate` (single code path through `compiler.Compile` eventually — for now share the existing `Loader` struct).
3. Add fixture: missing import → `validate` exits 1 with a file:line error.

#### Review coordination (ongoing M1)

- Review Mohamed's (T2) staged-writer migration PRs for the TS emitters — specifically check path safety and that `Output.WriteFile` rejects `../` paths.
- Unblock Python/Go compiler-side issues (F-MULTI-03–06) where invalid generated code traces back to AST misinterpretation — coordinate with Menna (T4).

---

### M2 (30 Nov – 10 Jan 2027) · **CRITICAL PATH** — sprints S3–S5

> **Exit criteria:** `internal/ir` v1 schema published; `veld ir` command works; structured diagnostics (`diag`) with spans; TypeRef recursive; conformance suite with 30 fixtures and expected IR; tier-1 TS emitters consume IR (via T2).

#### B-023 · Structured diagnostics `internal/diag` (P1, S3–S4, ~16–28 h)

**Goal:** Replace `fmt.Errorf` strings throughout the pipeline with machine-readable diagnostics that have stable codes, byte offsets, and snippets.

**New package `internal/diag`:**
```go
type Severity int
const (SeverityError Severity = iota; SeverityWarning; SeverityInfo; SeverityHint)

type Span struct {
    File   string // relative path, forward slashes
    Offset int    // byte offset from start of file
    Line   int    // 1-based
    Col    int    // 0-based UTF-8 byte column (UTF-16 unit converter in diag)
    Len    int    // byte length of the highlighted range
}

type Diagnostic struct {
    Code     string   // stable e.g. "E0101"
    Severity Severity
    Span     Span
    Message  string
    Hints    []string
    Stage    string   // "parse" | "validate" | "emit"
}
```

**Code table (stable, never renumbered):**
| Code | Stage | Meaning |
|---|---|---|
| E0101 | parse | Unknown token |
| E0102 | parse | Unterminated string |
| E0103 | parse | Expected token X, got Y |
| E0201 | validate | Unknown type reference |
| E0202 | validate | Duplicate name |
| E0203 | validate | Cyclic extends |
| E0204 | validate | Path param not in input |
| E0205 | validate | @default on array/map |
| E0206 | validate | Undefined union member |
| E0207 | validate | Missing import |
| E0301 | emit | Unsupported feature |
| W0101 | parse | Deferred syntax (union, WS, etc.) |

**Parser error recovery:** after a syntax error, skip to the next top-level keyword (`model`, `enum`, `module`, `constants`, `import`, `from`) and continue parsing. Return all collected diagnostics, not just the first.

**Lexer spans:** add `Col int` and `Offset int` to `lexer.Token`. Update `Lexer` to track byte offset (track in addition to rune position).

**Files to create/modify:**
- Create [`internal/diag/diag.go`](file:///G:/University/Graduation/Veld/internal/diag/diag.go)
- Create `internal/diag/text.go` (text formatter: `file:line:col: error[E0101]: message\n  snippet\n  ^^^`)
- Create `internal/diag/json.go` (JSON formatter)
- Modify `internal/lexer/lexer.go` — add `Offset`, `Col` to `Token`
- Modify `internal/parser/parser.go` — return `[]diag.Diagnostic` alongside AST

**Commit:** `B-023: internal/diag with codes, spans, text+json formatters; parser error recovery`

#### B-024 · TypeRef tree + nested generics (P1, S3–S4, ~12–24 h)

**Problem:** Today `ast.Field` uses flat flags (`IsArray bool`, `IsMap bool`, `MapValueType string`). `string[][]` or `Map<string, T[]>` is broken — both flags set simultaneously.

**New `TypeRef` in `internal/ast`:**
```go
type TypeKind int
const (TypeKindScalar TypeKind = iota; TypeKindRef; TypeKindArray; TypeKindMap; TypeKindUnion)

type TypeRef struct {
    Kind     TypeKind
    Scalar   string    // when Kind == TypeKindScalar: "string", "int", etc.
    Ref      string    // when Kind == TypeKindRef: model/enum name
    Elem     *TypeRef  // when Kind == TypeKindArray
    Key      *TypeRef  // when Kind == TypeKindMap (always string scalar for now)
    Value    *TypeRef  // when Kind == TypeKindMap
    Members  []*TypeRef // when Kind == TypeKindUnion
}
```

**Parser changes:** replace the flat-flag logic in `parseField()` with recursive `parseTypeRef()` that handles:
- `string`, `int`, etc. → scalar
- `User` → ref  
- `T[]` → array of T
- `T[][]` → array of array of T
- `List<T>` → array of T (alias)
- `Map<string, T>` → map
- `Map<string, T[]>` → map of array
- `A | B` → union (warning: W0101 deferred feature)

**Fixtures:**
- `string[][]` — nested array
- `Map<string, User[]>` — map of array of model ref
- `date` usable as a field name (reserved-word as identifier — `expectFieldName()` already handles this, add fixture)
- `enum Status { active, inactive }` with keyword values

**Commit:** `B-024: recursive TypeRef, nested generics, reserved-word field/enum names (F-FE-07, F-FE-08, F-FE-23)`

#### B-025 · IR v1 — the most important M2 deliverable (P0, S3–S5, ~40–60 h)

> This is the critical path for the plugin system (M3), the LSP (M2), and all emitters (M2).

**New package `internal/ir`:**

```go
// ir.go — the only cross-process contract (ADR-002)
package ir

type Program struct {
    IRVersion   string    `json:"irVersion"`   // "1.0"
    LangVersion string    `json:"langVersion"` // "1.0"
    Enums       []Enum    `json:"enums"`
    Models      []Model   `json:"models"`
    Services    []Service `json:"services"`
    // Reserved extension fields (B-074) — always present, may be null
    Constraints interface{} `json:"constraints,omitempty"`
}

type Enum struct {
    Name        string   `json:"name"`
    Description string   `json:"description,omitempty"`
    Members     []string `json:"members"` // sorted
    Span        Span     `json:"span"`
}

type Model struct {
    Name        string   `json:"name"`
    Description string   `json:"description,omitempty"`
    Extends     []string `json:"extends,omitempty"` // resolved; flattened with provenance
    Fields      []Field  `json:"fields"`             // sorted by name
    Span        Span     `json:"span"`
}

type Field struct {
    Name       string   `json:"name"`       // derived identifier (camelCase)
    WireName   string   `json:"wireName"`   // as written in the contract
    Type       TypeRef  `json:"type"`
    Optional   bool     `json:"optional,omitempty"`
    Default    *Default `json:"default,omitempty"`
    Deprecated string   `json:"deprecated,omitempty"`
    Example    string   `json:"example,omitempty"`
    From       string   `json:"from,omitempty"` // inherited from which parent
    ServerSet  bool     `json:"serverSet,omitempty"`
    Span       Span     `json:"span"`
}

type TypeRef struct {
    Kind   string   `json:"kind"`            // "scalar"|"ref"|"array"|"map"|"union"
    Scalar string   `json:"scalar,omitempty"` // "string","int","float","decimal","bool","date","datetime","uuid"
    Ref    string   `json:"ref,omitempty"`    // model or enum name
    Elem   *TypeRef `json:"elem,omitempty"`   // array element
    Key    *TypeRef `json:"key,omitempty"`    // map key (always string scalar)
    Value  *TypeRef `json:"value,omitempty"`  // map value
    // union members reserved for Q4
}

type Default struct {
    Kind  string `json:"kind"`  // "literal"|"enum"
    Value string `json:"value"`
}

type Service struct {
    Name        string      `json:"name"`
    Description string      `json:"description,omitempty"`
    Prefix      string      `json:"prefix,omitempty"`
    Operations  []Operation `json:"operations"` // ordered: static segments before :param
    Span        Span        `json:"span"`
}

type Operation struct {
    Name          string    `json:"name"`
    Description   string    `json:"description,omitempty"`
    Method        string    `json:"method"` // GET POST PUT DELETE PATCH
    Path          string    `json:"path"`   // /:param form
    Segments      []Segment `json:"segments"`
    Query         []Field   `json:"query,omitempty"`
    Headers       []Field   `json:"headers,omitempty"` // wireName + derived name
    Input         *TypeRef  `json:"input,omitempty"`
    Outputs       []Output  `json:"outputs"` // always at least one; first is success
    Middleware    []string  `json:"middleware"`
    Deprecated    string    `json:"deprecated,omitempty"`
    Span          Span      `json:"span"`
}

type Segment struct {
    Kind  string `json:"kind"`  // "static"|"param"
    Value string `json:"value"` // static text or param name
    Type  *TypeRef `json:"type,omitempty"` // for param: type from input/query or string
}

type Output struct {
    Status int      `json:"status"` // 200, 201, 204, etc. — always explicit in IR
    Type   *TypeRef `json:"type,omitempty"` // nil for 204
}

type Span struct {
    File   string `json:"file"`   // relative, forward slashes
    Line   int    `json:"line"`
    Col    int    `json:"col"`
    Offset int    `json:"offset,omitempty"`
    Len    int    `json:"len,omitempty"`
}
```

**Lowering rules (move semantics OUT of emitters into `internal/ir/lower`):**
1. Status codes — POST default 201, DELETE with no output 204, else 200. Stored explicitly.
2. Path segments — parse `:param` from path string into `[]Segment` with types.
3. Route ordering — sort operations so static segments precede `:param` at the same path position.
4. Inheritance — flatten `extends` chain, copy parent fields with `From` annotation, check for type conflicts.
5. Header derivation — `Idempotency-Key` → `WireName: "Idempotency-Key"`, `Name: "idempotencyKey"`.
6. Deferred features — union types, WS actions, `errors:` block: emit `diag.Diagnostic{Code: "W0101", ...}` and exclude from IR (emitters that try to use them get `E0301`).

**`veld ir` command:**
- Compile the contract, lower to IR, print JSON to stdout.
- `--json` flag (default): machine-readable.
- `--pretty`: indented JSON.
- Exit 1 if any `Error` severity diagnostics.

**JSON Schema:**
- Generate Go types → JSON Schema using `github.com/invopop/jsonschema` or write it by hand.
- Publish to `internal/ir/schema/ir-1.0.json`.
- CI validates every fixture's IR output against this schema.

**Conformance suite (30 fixtures minimum):**

Create `internal/spec/testdata/` with:
```
fixtures/
  001-empty-model/         contract.veld + expected-ir.json + expected-diag.json
  002-scalar-fields/
  003-optional-fields/
  004-array-fields/
  005-map-fields/
  006-nested-generics/     Map<string, T[]>, T[][]
  007-enum/
  008-enum-default/
  009-extends-single/
  010-extends-chain/
  011-extends-conflict/    expected diagnostic E0203
  012-module-get/
  013-module-post/
  014-module-put-delete/
  015-inline-input/
  016-inline-output/
  017-query-params/
  018-headers/             Idempotency-Key
  019-middleware/
  020-wildcard-import/
  021-named-import/
  022-from-import-list/
  023-missing-import/      expected diagnostic E0207
  024-path-params/
  025-path-param-types/
  026-status-codes/
  027-constants/
  028-deprecated-field/
  029-deprecated-action/
  030-reserved-words/      date as field name
```

**Files to create:**
- `internal/ir/ir.go`
- `internal/ir/lower/lower.go` (FromAST)
- `internal/ir/schema/ir-1.0.json`
- `internal/spec/spec_test.go` (fixture runner)
- `cmd/veld/veld.go` — add `veld ir` command

**Commits (ordered):**
1. `B-074: IR v1 reserved extension fields in schema` (4–8 h, must do first so schema is never broken)
2. `B-025a: internal/ir types + JSON Schema` (8–12 h)
3. `B-025b: internal/ir/lower — FromAST lowering` (16–20 h)
4. `B-025c: veld ir command + 30 conformance fixtures` (12–18 h)

#### B-028 · Validator extensions (P2, S4–S5, ~16–27 h)

After IR v1 lands, extend `internal/validator` or move validation into `semantic.Validate`:

| Check | Finding | Description |
|---|---|---|
| Path param vs fields | F-FE-09 | Every `:param` in `path:` must appear as a field in `input:` or `query:` |
| `@default` on arrays/maps | F-FE-10 | Should be an error |
| Undefined union members | F-FE-11 | Each member of a union type must be a known type |
| Extends field conflicts | F-FE-12 | Child field cannot change the type of an inherited field |
| Reserved model names | F-NODE-14 | `Object`, `Error`, `Promise`, etc. should warn |
| Workspace cycles | F-FE-27 | Detect cycles in multi-file `import` graphs |
| Loader strictness | F-FE-13/14 | Missing import is an error |

**Commit:** `B-028: validator extensions — path params, defaults, unions, extends, reserved names (F-FE-09..12,14,27,F-NODE-14)`

#### B-029 · Wire `internal/language` into lexer/parser/LSP (P1, S4, ~6–10 h)

**Problem:** `internal/language/constants.go` and `internal/language/veld.go` define the language spec but the lexer and parser hard-code the same lists independently. Any drift causes inconsistencies.

**Plan:**
1. Delete the regex-based parser in `internal/language/veld.go` (dead code, replaced by `internal/lexer` + `internal/parser`).
2. Move keyword/HTTP method/scalar type constants out of `lexer/lexer.go` and into `internal/language/constants.go`.
3. Have `lexer.classifyWord()` call `language.IsKeyword()`, `language.IsHTTPMethod()`, `language.IsScalar()`.
4. Add a drift test: `TestLanguageLexerParity` — any keyword in `language` must map to a token in `lexer` and vice versa.
5. LSP uses `language.Keywords()` for completion, not its own list.

**Commit:** `B-029: internal/language as single source of truth; drift test (F-FE-22, F-LSP-16, F-LSP-17)`

#### M2 Coordination duties
- Freeze `internal/ir/ir.go` types with Mohamed (T2) by **2026-12-02** (decision D2).
- Review T2's `internal/ir/lower/lower.go` v0 draft (T2 writes it; T1 is reviewer).
- Review T4's (Menna) Python and Go emitter migration onto the IR.
- Provide stable IR for the plugin handshake design (M3 prep).

---

### M3 (11 Jan – 7 Feb 2027) · **Plugin contract + LSP stability** — sprints S5–S6

> **Exit criteria:** VEP v1 schema frozen; plugin handshake fixtures; language semantics defined once in compiler.

#### B-036 · VEP v1 specification (P0, S6, ~10–16 h)

Design the external emitter protocol (ADR-003 accepted). T1's role:
- Confirm IR capabilities included in `--describe` manifest.
- Review the JSON schema for `--describe` → `--emit` exchange.
- Freeze the schema at M3 exit (no breaking changes after this).
- Write compatibility tests for unknown IR fields (future IR minor versions must not break existing plugins).

**Specification items (T1 owns the IR side):**
```json
{
  "describe": {
    "name": "my-emitter",
    "version": "1.0.0",
    "irVersions": ["1.0"],
    "capabilities": ["models", "services", "enums"]
  },
  "emit_request": { "ir": {/* ir.Program */}, "config": {} },
  "emit_response": {
    "files": [{"path": "relative/path.ts", "content": "base64..."}],
    "diagnostics": []
  }
}
```

#### B-025 completion — plugin-facing fixtures (S6)
- Provide 5 conformance fixtures specifically for plugin handshake testing.
- Verify that `semantic.Lower` (T1's implementation) produces the same IR as T2's lowering v0 adapter, so the transition is seamless.

#### Compiler semantics in one place
- Confirm that the LSP (T5's work) uses `compiler.Compile` as its sole entry point.
- Delete any duplicated semantic logic found in LSP, CLI, or emitters during this review.
- Write a test: `TestCompilerIsTheSingleEntryPoint` — no other package imports `internal/lexer` or `internal/parser` directly (import-boundary enforcement).

---

### M4 (8 Feb – 21 Mar 2027) · **Alpha integration** — sprints S7–S9

#### Compiler/IR integration with first external emitter
- Help resolve cross-file import issues found during Alpha integration testing.
- Review the first external emitter's plugin handshake test.
- If B-037 (plugin host) is green, run the stub plugin test.

#### Q13 — Rust emitter proof of concept (P3, with Menna and Ahmed, only if core is safe)
> This is **conditional** on core work being complete and stable. Do NOT start if any P0/P1 backlog items are open.

If approved:
- Design the Rust emitter as an **external plugin** (not built-in), speaking the VEP v1 protocol.
- Use Axum (not Actix — per AGENTS.md §8).
- Target: `cargo check` green for a single-module contract.
- Output layout: `src/models/`, `src/routes/`, `src/middleware/`.

#### B-059 — Q1 per-field constraints (P1, S8–S9, ~30–50 h)
Compiler side of `@min/@max/@minLength/@maxLength/@pattern/@format`:
- Add `Constraints` to `ir.Field` (reserved in schema from B-074 — just populate it).
- Parser: add `@min(n)`, `@max(n)`, `@minLength(n)`, `@maxLength(n)`, `@pattern("re2")`, `@format("email")` annotations to `parseField()`.
- Validator: check that `@min <= @max`, `@minLength <= @maxLength`, pattern is valid RE2.
- Lint rule: unsatisfiable combinations (e.g., `@min(5) @max(3)`) are errors.

**Commit:** `B-059: @min/@max/@minLength/@maxLength/@pattern/@format in parser, IR and validator (ADR-012 Q1)`

#### B-061 — Q3 auth schemes (P1, S9–S10, compiler side only)
Add `bearer`, `apiKey`, `basic`, `oauth2` syntax to the contract language and IR.

#### B-062 — Q4 discriminated unions (P1, S10–S11)
Full `TypeRef` union support with `@discriminator(kind)` annotation.

---

### M5 (22 Mar – 18 Apr 2027) · **Compiler correctness freeze** — sprints S10–S12

#### Determinism work (P0 — own hard optimizations only where measurements show benefit)
- Run the full benchmark suite (`evaluation/perf/`).
- Identify the slowest phase with profiling (`go test -cpuprofile`).
- Only optimize if a measurement proves it and the change doesn't affect semantics.
- Performance target: compile + validate + lower a 2,000-line, 10-file project in under 100 ms.

#### B-050 — Formatter rewrite (P1, S10–S11, ~12–20 h)
Lossless token stream, idempotent, `fmt --check` for CI. This is T1 or T5 work (chapter 08 §8.7).

**Key rules:**
- Preserve comments verbatim.
- Indent by block depth (2 spaces).
- Align field type annotations within a model.
- Idempotency test: `format(format(x)) == format(x)` on every fixture.
- `fmt --check`: exit 1 if file would change.
- Disable `--write` today with a message; enable once the new formatter passes all idempotency tests.

#### B-047 — Diff on IR (P1, S5 or S8–S9, ~10–18 h)
Make `veld diff` use the IR instead of the AST, so the classifier covers:
- Enum member additions/removals
- Nested model field changes
- Header name changes
- Middleware list changes
- Union member changes
- Status code changes (breaking: 200→204 is breaking; 200→201 is not)

Fix the stack overflow on cyclic `extends` (F-FE-19) by using the cycle detection added in the validator.

---

### M6 (19 Apr – 16 May 2027) · **RC freeze** — sprint S13

#### Freeze compiler-facing interfaces
- No breaking changes to `internal/ir` after this point without T1 approval and an ADR.
- Review all PR descriptions for `BREAKING CHANGE:` markers.
- Confirm IR minor-version bump rules are documented (ADR-009).

#### Release-blocking compiler issues
- Own all P0 and P1 bugs filed against the compiler.
- Block the RC if `go test -race ./...` fails.
- Final sign-off on `compiler.Compile` API stability.

#### Stop unaccepted Rust/extension work (by 2 May)
Per AGENTS.md §5: "after the extended freeze (2027-05-02) only P0/P1 fixes". Any Q13 Rust work not accepted must be on a branch off `frozen/rust-poc` and not merged.

---

### M7 (17 – 31 May 2027) · **Defense prep**

#### Final review and technical walkthrough prep

Prepare a walkthrough document covering:
1. **Parsing pipeline** — lexer → parser → AST; error recovery; spans.
2. **Semantic pipeline** — loader → resolver → validator → lowering → IR.
3. **IR design decisions** — why sorted slices (determinism), why source spans on every node, why no maps in the IR, versioning strategy.
4. **Cross-language outputs** — how the same IR produces TS, Python, and Go with identical semantics.
5. **Performance numbers** — measured at RC, with methodology.
6. **Plugin protocol** — VEP v1 handshake, trust model, Wasm transport (if Q11 landed).

#### Defense demo fallback
- Provide a `veld ir --pretty examples/microservices/contract.veld` command that always works (pre-compiled if needed).
- Keep a snapshot of the IR output frozen at the RC so the demo doesn't depend on live compilation.

---

### JUNE 2027 · Defense

**Explain:**
- Architecture decisions (ADR-001: retain Go, ADR-002: AST/IR separation).
- Why we didn't rewrite (fuzz evidence, audit report).
- IR versioning and backward compatibility story.
- Tradeoffs: single-pass lowering vs multi-pass, `compiler.Compile` as the only entry point.

**Demonstrate:**
- Live `veld validate` with structured JSON diagnostics (`--json`).
- `veld ir` output for a real contract.
- `veld generate` with IR-based emitters → all three backends compile in CI.
- Plugin protocol: external emitter receiving the IR and producing a file.

---

## Code Quality & Refactoring Plan

### Immediate (M1, before Nov 29)

1. **Delete `internal/language/veld.go` regex parser** — it's dead code, replaced by the real parser. Keep only the constants and the `LanguageSpec` struct.
2. **Wire `internal/errors.VeldError`** — the structured type exists but is unused. For now, the parser can wrap errors with it; the full wiring happens in B-023.
3. **Fix map iteration in `classifyWord`** — `classifyWord` is a switch, good. But check all emitters for `for k, v := range someMap` producing output — sort keys first.
4. **Remove `internal/cache`** (chapter 08 §8.5 says to remove incremental cache; B-012 decision D1) — but coordinate timing with T2.

### M2 refactoring targets

| Package | Problem | Fix |
|---|---|---|
| `internal/language/veld.go` | Duplicate regex-based parser | Delete the parse methods; keep constants |
| `internal/language/imports.go` | Unknown — audit | Read and decide |
| `internal/emitter/emitter.go` | 90 files import `ast.AST` directly | Migrate to IR after B-025 (T2 leads, T1 reviews) |
| `internal/parser/parser.go` (1154 lines) | Single large file | Split into `parser_model.go`, `parser_module.go`, `parser_import.go` after B-024 |
| `internal/validator/validator.go` (577 lines) | Missing checks | Add B-028 checks; keep table-driven |
| All emitters | `os.WriteFile` calls | T2 migrating; T1 reviews import boundary |

### Performance targets (measure at M5–M6)

| Metric | Target |
|---|---|
| `veld compile` on 2,000-line, 10-file project | < 100 ms |
| `veld generate` for 50 models + 100 operations, 3 tier-1 backends | < 1 s (excl. plugins) |
| Peak memory for above | < 100 MB |
| Lexer throughput | > 100 MB/s (maintain fuzz baseline) |

---

## Import Boundary Rules (enforced by CI test)

```
cmd → compiler → semantic → syntax → language
emitters → ir
internal/diag ← (used by) parser, semantic, lsp, cli
internal/language ← (used by) lexer, parser, lsp
```

**Never:** `emitters → lexer`, `emitters → parser`, `emitters → ast` (after IR migration)  
**Never:** `semantic → emitters`, `cmd → internal/server`

Add `internal/boundary_test.go` checking the import graph.

---

## Language Syntax Improvements

### Veld 1.0 syntax — what to clean up now

| Syntax issue | Fix | Backlog |
|---|---|---|
| `import @models/*` hits comment branch | Lexer fix (B-010) | M1 |
| Unterminated strings silently accepted | Lexer fix (B-010) | M1 |
| `field?: type` and `field: type` both work inconsistently | Standardize in parser; both are valid (optional marker on name OR after colon) | M1 |
| Reserved words can't be field names (`date`, `default`) | `expectFieldName()` already handles keywords; add fixture | M1 |
| `List<T>` and `T[]` are both accepted but `T[][]` is broken | TypeRef fix (B-024) | M2 |
| `Map<string, T>` broken when combined with `[]` | TypeRef fix (B-024) | M2 |
| Union types `A \| B` accepted by parser but no validation | Add W0101 "deferred feature" warning; freeze for 1.0 | M2 |
| WS actions accepted but emitters silently wrong | Add W0101 + E0301 for emitters | M2 |

### Professional language features (lang 1.0 release)

The graduation release Veld 1.0 specifies:
- `model`, `module`, `action`, `enum`, `constants`, `import`/`from`, `extends`
- Scalars: `string`, `int`, `float`, `decimal`, `bool`, `date`, `datetime`, `uuid`
- Generics: `T[]`, `T[][]`, `List<T>`, `Map<string, T>`, `Map<string, T[]>`
- Optionality: `field?: T` or `field: T?` (both normalized)
- Annotations: `@default(v)`, `@deprecated "msg"`, `@example(v)`, `@unique`, `@index`, `@relation(M)`, `@serverSet`
- Module directives: `prefix: /path`, `description: "text"`, `baseUrl: https://...`
- Action directives: `method:`, `path:`, `input:`, `output:`, `query:`, `headers:`, `middleware:[]`, `description:`
- Import forms: `import @alias/*`, `import @alias/Name`, `from @alias import Name, Other`
- Optional version header: `veld 1`

**Deferred to future versions (emit W0101, refuse in emitters with E0301):**
- Union types `A | B`
- WebSocket actions (`WS`, `stream:`, `emit:`)
- Error output declarations (`errors:`)
- Per-field constraints (`@min`, `@max`, `@minLength`, `@maxLength`, `@pattern`, `@format`) — these are Q1, targeted for M4 if capacity allows
- Nullable (distinct from optional)
- Custom scalars
- HEAD/OPTIONS methods
- Generics beyond `Map` and arrays

---

## Conformance Test Strategy

### Fixture runner (`internal/spec/spec_test.go`)

```go
func TestConformance(t *testing.T) {
    fixtures, _ := filepath.Glob("testdata/*/contract.veld")
    for _, fix := range fixtures {
        t.Run(filepath.Base(filepath.Dir(fix)), func(t *testing.T) {
            // 1. Compile
            prog, diags := compiler.Compile(ctx, fix)
            // 2. Compare diagnostics to expected-diag.json
            // 3. Compare IR to expected-ir.json
            // 4. Validate IR against ir-1.0.json schema
        })
    }
}
```

### Golden output tests (after IR)

For each tier-1 emitter × each fixture, golden output stored in `cmd/veld/testdata/golden/<fixture>/<target>/`. Updated with `-update` flag; diff reviewed in PR.

---

## Sprint Checklist

### S2 (now — Nov 29) · Must ship
- [ ] B-010: Wildcard import lexer fix + tests
- [ ] B-010: Unterminated string error
- [ ] B-010: BOM handling
- [ ] B-011: Header wire name + derived identifier
- [ ] B-010: Loader missing import = error
- [ ] Review T2's `Output` API migration PR

### S3 (Nov 30 – Dec 13) · M2 starts
- [ ] B-023: `internal/diag` package (codes, spans, text+json formatters)
- [ ] B-023: Parser error recovery
- [ ] B-074: IR reserved extension fields (small, do first)
- [ ] B-025a: `internal/ir/ir.go` types + JSON Schema
- [ ] Decision D2: Freeze IR Go types with T2 by Dec 2
- [ ] B-029: Wire `internal/language` into lexer (delete regex parser)

### S4 (Dec 14 – Dec 27)
- [ ] B-024: TypeRef recursive type tree
- [ ] B-025b: IR lowering `FromAST`
- [ ] B-025c: IR JSON goldens for all fixtures
- [ ] Review T2's lowering v0

### S5 (Dec 28 – Jan 10) · M2 exit review Jan 8
- [ ] B-025c: 30 conformance fixtures complete
- [ ] B-028: Validator extensions (path params, extends conflicts)
- [ ] B-025: `veld ir` command
- [ ] IR schema validation in CI
- [ ] Import boundary test

### S6 (Jan 11 – Jan 24) · M3 starts
- [ ] B-036: VEP v1 schema (IR side)
- [ ] Plugin handshake fixtures
- [ ] Confirm compiler as sole entry point for LSP

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| IR types change after T2 starts lowering v0 | High (early design) | Major rework | Freeze `ir.go` by Dec 2 as an additive-only contract |
| B-023 (diag) blocks B-025 (IR) | Medium | M2 exit slips | Start IR types (B-074, B-025a) in parallel; diag codes can be added later |
| Rust emitter (Q13) pulls focus from core | Medium | P0 backlog slip | Do NOT start Q13 until all P0/P1 items are closed |
| Winter break (Dec 21 – Jan 3) | Certain | S5 is short | No new design in S5; only implementation of already-designed items |
| T1 estimate exceeded 1.5× | Possible | Need a decision | Write a one-page note in `docs/notes/` with two options; ask owner |
| `internal/language` wiring breaks LSP | Low | Regression | Add drift test before wiring; keep old constants as aliases during transition |

---

## Definition of Done (per AGENTS.md §2)

Every T1 commit must satisfy:
- [ ] Unit tests for new logic; conformance fixture for any language/semantic change (before the implementation)
- [ ] `go build ./...`, `go vet ./...`, `go test -race ./...`, `golangci-lint` all green
- [ ] `govulncheck` clean
- [ ] Import-boundary test passes
- [ ] Package touched does not drop below current statement coverage (new packages: 70%+)
- [ ] `CHANGELOG.md` entry under Unreleased
- [ ] ADR if any boundary, format, or guarantee changed
- [ ] PR title starts with backlog ID (`B-025:`)
- [ ] Commit messages reference finding IDs (`F-FE-01`)
- [ ] At least one reviewer who is not the author (from `docs/review-2026-10/16-team-allocation.md`)

---

*Last updated: 2026-11-19 by Adham (T1). Next review: M2 start, 2026-11-30.*
