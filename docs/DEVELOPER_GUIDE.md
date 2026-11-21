# Veld developer guide

For people who change Veld itself. Users should start with [`guides/getting-started.md`](guides/getting-started.md).
This guide is the map: how a `.veld` file becomes generated code, which crate owns each step, where
to look when something is wrong, and how to add a feature without breaking the rules.

Read [`CLAUDE.md`](../CLAUDE.md) (repository map and the non-negotiable rules) and
[`CONTRIBUTING.md`](../CONTRIBUTING.md) (commands, definition of done, commit convention) alongside
this. The specs in [`spec/`](../spec) are normative: change the spec in the same commit as the
behaviour. v1 (the Go implementation) is in [`archive/v1/`](../archive/v1) for reference; never
build on it.

## 1. Mental model

```
.veld files ──lex/parse──▶ AST ──semantic passes──▶ checked model ──lower──▶ IR  (immutable, with signatures)
veld.toml   ──veld-project─▶ Project (packages, targets, plugins)                │
                                                                                 ▼
                          ┌──────────── capability check (E0801) ────────────────┤
                          ▼                                                      │
                 Generator::generate(IR, role, options, profile) ──▶ FileSet     │
                          │                                                      │
                          ▼                                                      ▼
            veld_project::output (stage, plan, manifest, commit)        tools: fmt, diff, export, lint, ...
```

Three ideas carry the design:

1. **The IR is the only interface.** Generators, tools, the language server's queries and external
   plugins see `veld-ir` and nothing else. They never see the AST, the config or the compiler. This is
   what lets a Python plugin and the built-in Java emitter agree on a contract.
2. **Equality is by signature.** `veld-compiler::signature` computes a signature per declaration
   and operation. Generators embed it verbatim. `same()` compares signature, operation and canonical
   bytes, never class identity (rule 4 in `CLAUDE.md`).
3. **Generation is never destructive.** Every file write goes through `veld_project::output`: stage,
   plan against the manifest, commit (rule 1).

## 2. Repository layout

| Path | What |
|---|---|
| `crates/veld-ir` | The IR types, `ir_version`, the JSON schema (`schema::typescript()` writes `spec/ir.d.ts`) and the shared edit-distance helper |
| `crates/veld-compiler` | `language.rs` (the language table), `syntax/` (lexer, parser, AST with comments), `semantic/` (passes), `lower.rs`, `signature.rs`, `codes.rs` (every diagnostic code) |
| `crates/veld-codegen` | `Generator` and `DynGenerator`, `Capabilities` and `Feature`, `FileSet`, `CodeWriter`, `naming`, `Profile`, RFC 8785 canonical JSON, the lint level type |
| `crates/veld-project` | `config/` (`veld.toml`: raw shape, resolution, `E07xx`), `output/` (staging, manifest, plan), `packages.rs`, the registry lock |
| `crates/veld-tools` | `fmt`, `openapi`, `diff` (breaking changes), `migrate` (v1 to v2), `lint`, and the exporters (`graphql`, `schema`, `docs`, `agents`) |
| `crates/emitters/{typescript,node,java}` | The built-in generators, each with a `frameworks/` registry |
| `crates/veld-lsp` | `veld lsp` (diagnostics, hover, definition, completion, rename, formatting, semantic tokens) |
| `crates/veld-plugin-host` | Runs an external emitter over the plugin protocol ([`spec/plugin-protocol.md`](../spec/plugin-protocol.md)) |
| `crates/veld-registry-client` | `add`, `install`, `publish`, `login` against the registry |
| `crates/veld-emit-conformance`, `veld-test-support` | The conformance kit every emitter must pass; shared test helpers |
| `apps/veld` | The CLI: `cli.rs` (clap), `commands/*`, `pipeline.rs` (the shared steps), `setup/`, `ui/` (the terminal look) |
| `xtask/` | `cargo xtask codegen`, `cargo xtask new-emitter` |
| `tests/` | `conformance/` (compiler snapshots), `toolchain/` (real `tsc`, `javac`, Spring), `wire/` (canonical byte goldens) |
| `examples/` | Complete projects with committed output, checked by tests |
| `registry/` | The Go registry server (a separate HTTP service, own `go.mod`) |
| `editors/`, `website/`, `distribution/`, `assets/` | Editor plugins, site, install wrappers and release packaging, logos |

Dependencies go one way and Cargo enforces it: `apps -> tools/emitters -> codegen -> ir` and
`compiler -> ir`. An emitter cannot depend on `veld-compiler`.

## 3. Build, run, test

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p veld --release            # target/release/veld(.exe), one self-contained binary
cargo run -p veld -- check               # run the CLI from source in a project folder
cargo test --workspace                   # VELD_REQUIRE_TOOLCHAINS=1 makes a missing tsc/javac a failure
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo xtask codegen --check              # generated files are current; `cargo xtask codegen` rewrites them
cargo insta test --review                # a human accepts snapshot changes, never an agent
(cd registry && go test ./...)
```

Useful while debugging: `veld export ir` prints the full IR with signatures, and the hidden
`veld debug` command holds developer-only dumps. `-q` and `--no-color` give stable output for diffing.

## 4. The compiler, stage by stage

Entry point: `veld_compiler::compile(CompileInput) -> CompileOutput` (`crates/veld-compiler/src/lib.rs`).
The input is a set of packages, each with its source files; the compiler never touches the disk.

1. **Lexer** (`syntax/lexer.rs`). Tokens, comments and doc comments are kept. The reserved and
   contextual keywords come from `language.rs`, the single source of truth (see section 8).
2. **Parser** (`syntax/parser.rs`, AST in `syntax/ast.rs`). Error tolerant: it reports and
   recovers, so one mistake does not hide the rest. The AST keeps comments, so the formatter in
   `veld-tools::fmt` can print it back. v1 syntax is recognised only to say `E0001` with the hint to run `veld migrate`.
3. **Semantic passes** (`semantic/`), skipped for files with syntax errors so a bad parse does not
   cascade:
   - `collect.rs` registers every declaration in every module (so import cycles are never an error);
   - `resolve.rs` resolves `use` imports and type names (`E02xx`, with "did you mean" help);
   - `inherit.rs` handles `extends` (`E04xx`);
   - `typeck.rs` checks defaults, examples, recursion and map keys (`E03xx`);
   - `services.rs` binds parameters, checks paths and responses, adds the implicit
     `ValidationError`, `Unauthorized` and `Internal` (`E05xx`);
   - `annotations.rs` checks annotation names, targets and arguments (`E06xx`); `regex.rs` checks the portable regex subset.
4. **Lowering** (`lower.rs`) produces the immutable IR: resolved types, bindings, effective auth and
   middleware, origins (file, line, column) for tooling.
5. **Signatures** (`signature.rs`) are computed last, and only here.

Diagnostics (`diagnostics.rs`) carry a code from `codes.rs`, a primary span, labels and help text.
The table in `codes.rs` generates [`spec/diagnostics.md`](../spec/diagnostics.md), and a test fails
if any source file mentions a code that is not in the table.

## 5. The project layer

`veld-project::config` loads `veld.toml` in two steps: `raw.rs` is the on-disk shape (serde with
`deny_unknown_fields`, so an unknown key is `E0701` with the closest valid key), and `resolve.rs` turns it into a
`Project` (packages, services, `consumes`, targets with resolved roles, frameworks and paths, plugins)
and checks the rules of [`spec/config.md`](../spec/config.md) (`E0702` to `E0705`, `W0701`).

`veld-project::output` is the only code that writes generated files: `plan` compares a `FileSet` with
the manifest on disk (create, update, delete, user-modified, conflict), `write` stages, commits and
writes `.veld-manifest.json`, `clean` removes unmodified files. Paths are contained: a generated
path cannot leave `out` or use the `.veld-staging-` prefix.

## 6. Generators

A built-in generator implements `Generator` (`veld-codegen/src/generator.rs`): `id`, `roles`,
`frameworks(role)`, an optional `language()`, `capabilities(role)` and
`generate(&GenRequest, options)`. `DynGenerator` is the object-safe form that the CLI holds.

- **Capabilities and `E0801`.** Before generating, `check_capabilities` compares the `Feature`s that
  the contract uses with what the generator declares for the role. Anything missing is `E0801`. A
  generator that declares a feature must implement it, and one that does not can never emit wrong code for it.
- **Frameworks.** A server framework is a `FrameworkAdapter` for the emitter's own server model, listed
  in one registry slice under `crates/emitters/<name>/src/frameworks/`. `frameworks(role)`, the
  `E0803` message and the generated adapters all come from that slice.
- **Profile.** `resolve_profile` turns `java`, `typescript` and `framework-version` into a `Profile`
  once, centrally (`E0802`, `E0803`). Generators ask `profile.java_at_least(21)`, never compare strings.
- **Writing output.** Use `CodeWriter` for formatting and `naming` for casing and reserved words.
  Return a `FileSet` (a sorted map of relative path to bytes). No timestamps, absolute paths, host names, no
  `HashMap` iteration order in output (rule 2).
- **Plugins** speak the same contract over stdio; `veld-plugin-host` runs them, checks `sha256`
  (`E0705`) and the protocol version.

Specs for the built-in targets: [`spec/targets/typescript.md`](../spec/targets/typescript.md) (the
`node` and `typescript` emitters) and [`spec/targets/java.md`](../spec/targets/java.md). Writing a new
one: [`emitter-guide.md`](emitter-guide.md) and `cargo xtask new-emitter`.

## 7. The CLI

`apps/veld/src/cli.rs` defines every command with clap; `commands/<name>.rs` implements each as a
composition of the steps in `pipeline.rs`: find and load config, collect sources, compile, render
diagnostics, check capabilities, generate per target, plan, write. `ui/` owns the terminal look
(palette, symbols, spinner, prompts, `NO_COLOR`); commands never print raw escape codes. `setup/`
edits `package.json`, `pom.xml` and Gradle files while keeping their formatting, idempotently.

Rules for command code: libraries return typed errors (`thiserror`), only `apps/veld` uses `anyhow`;
exit code 1 for a failed check; a command that can prompt must also work with flags and
never prompt without a TTY.

## 8. Generated files and the language table

`cargo xtask codegen` writes everything derived from Rust sources, and `--check` fails CI on drift:

| Output | From |
|---|---|
| `editors/veld-language.json`, the two `veld.tmLanguage.json` grammars | `veld_compiler::language::spec()` through `xtask/src/grammar.rs` |
| `spec/diagnostics.md` | `veld_compiler::codes` |
| `spec/ir.d.ts` | `veld_ir::schema` |
| `editors/veld.toml.schema.json` and its copies in the VS Code and JetBrains plugins | `xtask/src/schema.rs`, with tests that check it against the real `veld.toml` parser |

Never edit these by hand. To add a keyword, change `language.rs` and regenerate; to add a `veld.toml`
key, add it to `raw.rs`, to `spec/config.md` and to `xtask/src/schema.rs` (the schema test fails until you do).

## 9. Tests: what proves what

| Suite | Location | Proves |
|---|---|---|
| Unit tests | next to the code | one function or module |
| Conformance | `tests/conformance/<code>_<case>/` | one valid and one invalid case per diagnostic code and language rule, as snapshots |
| Emitter snapshots | `crates/emitters/*/tests/snapshots.rs` (insta) | generated bytes for fixed contracts, determinism, signatures equal to the IR |
| Conformance kit | `veld-emit-conformance` | capability gate, determinism and embedded signatures, for every emitter and plugin |
| Toolchain round trips | `tests/toolchain/` | generated code compiles with real `tsc` (es2020 and es2022) and `javac --release 17` and `21`, and a generated client talks to a generated server, including Spring and streaming |
| Wire goldens | `tests/wire/` | canonical bytes are identical across languages |
| CLI tests | `apps/veld/tests/*.rs` | each command end to end; `examples.rs` and `examples_e2e.rs` regenerate every example, boot the servers and call them |
| Registry | `registry/` (`go test ./...`) | the Go server |
| Editors | `editors/vscode/test`, `editors/jetbrains/src/test` | command wiring, snippets, schema, actions, templates |

Set `VELD_REQUIRE_TOOLCHAINS=1` so missing tools fail instead of skipping. Snapshot changes are
reviewed by a person with `cargo insta test --review`. Never weaken or delete a test to make it pass.

## 10. Recipes

**A new language feature.** Edit `spec/language.md` first. Then `language.rs` if it adds a keyword,
parser, a semantic pass, lowering, signature if it changes identity, `codes.rs` for new diagnostics, the
formatter (`veld-tools::fmt`), the LSP (completion, hover), a conformance case, and every emitter
(or an `E0801`-style capability gap). Regenerate with `cargo xtask codegen`.

**A new diagnostic.** Add the row to `codes.rs` (codes are never reused; ranges are in
`spec/language.md` section 9), use it, add a conformance case. `spec/diagnostics.md` regenerates.

**A new `veld.toml` key.** `raw.rs`, `resolve.rs`, `spec/config.md`, `xtask/src/schema.rs`, a config
test, then `cargo xtask codegen`. If an editor feature depends on it, update the snippets in `editors/vscode/snippets`.

**A new generator or framework.** `cargo xtask new-emitter`, or a framework adapter in the
emitter's registry slice; spec in `spec/targets/`, snapshots, conformance, a toolchain round trip, an
example update. See [`emitter-guide.md`](emitter-guide.md).

**A new CLI command.** `cli.rs`, `commands/<name>.rs`, logic in `veld-tools` (testable without the
CLI), output through `ui/`, a CLI test in `apps/veld/tests`, an entry in the README and in the editors' command lists if it belongs there.

**A release.** Tag `v<version>`; `.github/workflows/release.yml` builds the five platform archives,
`SHA256SUMS`, the registry binaries, `.deb`/`.rpm`, the Chocolatey package and the Scoop manifest.
See [`distribution/README.md`](../distribution/README.md) and `distribution/PUBLISHING.md`.

## 11. Where to look for a symptom

| Symptom | Start here |
|---|---|
| A valid contract is rejected | `crates/veld-compiler/src/semantic/` (the code names the pass); `veld export ir` to see what was lowered |
| A syntax error points at the wrong place | `syntax/parser.rs` recovery |
| Wrong generated code | the emitter's snapshot test, then `crates/emitters/<name>/src`; compare with `spec/targets/` |
| Output differs between machines | something iterates a `HashMap`, or embeds a path or time; `tests/wire/` and determinism tests |
| `E0801` on a valid feature | the emitter's `capabilities(role)` |
| Files overwritten or not removed | `crates/veld-project/src/output/`; read the plan before the write |
| `veld.toml` key rejected | `config/raw.rs` constants, then `xtask/src/schema.rs` |
| Editor shows nothing | `veld lsp` run by hand; the VS Code Veld output channel; the Language Servers tool window in JetBrains |
| `veld setup` changed the wrong file | `apps/veld/src/setup/` |
| A package does not resolve | `veld-registry-client`, `veld.lock`, `spec/config.md` registry section |

## 12. The rules, repeated because they matter

1. Generation is never destructive: writes go through `veld_project::output`; user-modified files are refused without `--force`.
2. Output is deterministic: `BTreeMap` or sorted `Vec`; no timestamps, absolute paths or host names.
3. Zero runtime dependencies in generated code, except the adapter of the framework the user chose.
4. Signatures are computed only in `veld-compiler::signature`; equality is `same()`.
5. Unsupported means a loud error (`E0801`); never wrong code.
6. `consumes` cycles are legal.
7. Library code never panics (no `unwrap`, `expect`, `panic`); `thiserror` in libraries, `anyhow` only in `apps/veld`; functions under about 60 lines.
8. Tests come with the change; never weaken or delete one to make it pass.
