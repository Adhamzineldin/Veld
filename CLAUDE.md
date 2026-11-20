# CLAUDE.md — Veld 2.0

Veld is a contract-first API code generator. A developer writes `.veld` contracts, and Veld generates typed servers and clients. Version 2.0 is a Rust rewrite with a new language. The specs in `spec/` are **normative**. Read the relevant one before you change behaviour.

## Repository map

| Path | What |
|---|---|
| `spec/` | language.md, ir.md, config.md, plugin-protocol.md, targets/*.md |
| `crates/veld-ir` | Immutable IR. This is the only thing generators, tools and plugins see. |
| `crates/veld-compiler` | language table, lexer, parser (AST and comments), semantic passes, lowering, **signatures** |
| `crates/veld-codegen` | Generator traits, Capabilities, `FileSet`, CodeWriter, naming, RFC 8785 canonical JSON |
| `crates/veld-project` | `veld.toml` loading and the staged, manifest-based output writer (the only code that writes files) |
| `crates/veld-tools` | fmt, openapi, diff (breaking changes), migrate (v1 to v2) |
| `crates/emitters/*` | Built-in generators: `typescript` (client), `node` (server), `java` |
| `apps/veld` | The CLI binary (`veld`). `veld lsp` and `veld debug` are hidden. |
| `tests/` | `conformance/` (compiler snapshots), `toolchain/` (real tsc/javac round trips), `wire/` (canonical byte goldens) |
| `registry/` | The Go registry server. It is a separate HTTP service. |
| `editors/`, `website/`, `distribution/`, `assets/` | Editor plugins, site, install wrappers, logos |
| `archive/v1/` | The old Go implementation, docs, ADRs and scratch files. **Reference only. Never build on it.** |
| `cmd/`, `internal/`, `go.mod` | The v1 Go compiler. It moves to `archive/v1` at cutover, except `internal/server`, which moves to `registry/`. |

Dependency direction, which Cargo enforces: `apps → tools/emitters → codegen → ir` and `compiler → ir`. An emitter never depends on `veld-compiler`.

## Commands

```bash
export PATH="$HOME/.cargo/bin:$PATH"     # bash on this machine
cargo build -p veld --release            # -> target/release/veld(.exe), one self-contained binary
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo insta test --review                # snapshot changes need a human; agents never self-accept
```

## Non-negotiable rules
1. **Generation is never destructive.** All writes go through `veld_project::output`, which uses staging, a manifest and path containment.
2. **Output is deterministic.** Use `BTreeMap` or sorted `Vec`. No timestamps, absolute paths or host names in output.
3. **Zero runtime dependencies in generated code.** The only exception is a framework adapter for the framework the user chose.
4. **Signatures are computed only in `veld-compiler::signature`.** Generators embed them verbatim. Equality is decided by `same()`, which compares signature, operation and canonical bytes, never class identity.
5. **Unsupported means a loud error.** If a generator cannot represent a feature it must report `E0801`. It never emits wrong code.
6. **`consumes` cycles are legal.** Services may call each other.
7. **Library code never panics:** no `unwrap`, `expect` or `panic`. Use `thiserror` in libraries and `anyhow` only in `apps/veld`. Keep functions under about 60 lines.
8. **Tests come with the change:** unit tests, a conformance case for language changes, and snapshot plus toolchain round trip for generator changes. Never weaken or delete a test to make it pass.
