# Contributing to Veld

Read [`CLAUDE.md`](CLAUDE.md) first. It holds the repository map and the non-negotiable rules. The
specs in [`spec/`](spec) are normative: read the relevant one before you change behaviour, and
change the spec in the same commit as the behaviour.

## Workspace layout

| Path | What |
|---|---|
| `crates/veld-ir` | The immutable IR. Generators, tools and plugins see only this. |
| `crates/veld-compiler` | Language table, lexer, parser, semantic passes, lowering and **signatures** |
| `crates/veld-codegen` | Generator traits, capabilities, `FileSet`, `CodeWriter`, naming, canonical JSON |
| `crates/veld-project` | `veld.toml` loading and the staged, manifest-based output writer |
| `crates/veld-tools` | `fmt`, `export openapi`, `diff`, `migrate` |
| `crates/emitters/*` | Built-in generators: `typescript`, `node`, `java` |
| `crates/veld-lsp`, `veld-plugin-host`, `veld-registry-client` | Language server, plugin host, registry client |
| `apps/veld` | The CLI binary |
| `registry/` | The Go registry server (a separate HTTP service) |
| `xtask/` | `cargo xtask`: generated files and emitter scaffolding |
| `tests/` | `conformance/` (compiler snapshots), `toolchain/` (real tsc and javac round trips), `wire/` (canonical byte goldens) |
| `examples/` | Example projects with committed output (checked by `apps/veld/tests/examples.rs`) |
| `archive/v1/` | The old Go implementation. Reference only, never build on it. |

Dependencies go one way, and Cargo enforces it: `apps -> tools/emitters -> codegen -> ir` and
`compiler -> ir`. An emitter never depends on `veld-compiler`.

## Commands

```bash
cargo build -p veld --release            # target/release/veld
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo xtask codegen --check              # generated files (spec/ir.d.ts, editor grammars) are current
cargo insta test --review                # snapshot changes need a human review
(cd registry && go test ./...)           # the registry
```

The toolchain tests compile generated code with `tsc` and `javac`. Install the pinned TypeScript with
`npm ci --prefix tests/toolchain/ts` and have a JDK 17+ on `PATH`. Set `VELD_REQUIRE_TOOLCHAINS=1` so
a missing tool fails the test instead of skipping it. CI does this.

## Definition of done

A change is done when all of these are true:

1. `cargo test --workspace` passes with `VELD_REQUIRE_TOOLCHAINS=1`.
2. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean.
3. `cargo xtask codegen --check` is clean, and `go test ./...` passes in `registry/` if you touched it.
4. Tests come with the change:
   - unit tests
   - a conformance case for every language change
   - a snapshot plus a toolchain round trip for every generator change
   - regenerated `examples/` output when generated code changes (`veld generate` in each example)
5. The spec is updated when behaviour changes.
6. You never weakened or deleted a test to make it pass, and snapshot changes were reviewed by a person.
7. The rules in `CLAUDE.md` hold: non-destructive writes through `veld_project::output`, deterministic
   output, zero runtime dependencies in generated code, signatures computed only in
   `veld-compiler::signature`, unsupported features reported as `E0801`, and no `unwrap`, `expect` or
   `panic` in library code.

## Commit convention

Make small commits, one module or one concern each, and push continuously. Do not pile up a big
change. Use a conventional prefix and a scope:

```
feat(emit-java): sealed interfaces for unions
fix(compiler): reject a required self-cycle with E0407
docs(spec): clarify the response list
test(conformance): add e0407_required_self_cycle
```

## Adding an emitter

Follow [`docs/emitter-guide.md`](docs/emitter-guide.md). A built-in emitter is a Rust crate scaffolded
with `cargo xtask new-emitter <name>` (see the guide), and an external one is a plugin created with
`veld plugin new <name>`. Either way you need conformance coverage, snapshots, a toolchain round trip
and a target spec in `spec/targets/`.
