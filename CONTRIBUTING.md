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
| `crates/veld-tools` | `fmt`, `export openapi`, `diff` |
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
`npm ci --prefix tests/toolchain/ts` and have a JDK 21 on `PATH` (it compiles for `--release 17` and `21`). Set `VELD_REQUIRE_TOOLCHAINS=1` so
a missing tool fails the test instead of skipping it. CI does this. The Spring test builds `tests/toolchain/java/spring` with the committed Maven Wrapper (`sh mvnw`, `mvnw.cmd`; no Maven install needed, but it downloads Maven and Spring Boot 3 on first run; it skips when offline without a populated `~/.m2`).

## How the build enforces the architecture

The architecture is checked by the compiler, the linter and the test suites, not by review. Work with these mechanisms, not around them.

- **Exhaustive matches.** The IR enums (`Type`, `DeclKind`, `Feature`, `Binding`, ...) are not `#[non_exhaustive]`, and `clippy::wildcard_enum_match_arm` and `match_wildcard_for_single_variants` are warnings (errors in CI with `-D warnings`). Match every variant explicitly, so adding one breaks the build at each site that must handle it. Where a wildcard is genuinely right (scanning a token stream, or a foreign enum), allow the lint on that item with `#[allow(..., reason = "...")]`. External plugins are protected by `ir_version` and the capability check instead.
- **Traits.** A generator implements `Generator` (`veld-codegen`); a server framework implements `FrameworkAdapter<M>` for the emitter's own server model, and the emitter lists its adapters in ONE registry slice (`crates/emitters/<name>/src/frameworks/`). `frameworks(role)` and the central `E0803` check are derived from that slice. The language and framework versions are resolved once into a `Profile`; code asks `profile.java_at_least(21)` and never compares version strings.
- **Crate boundaries.** `apps -> tools/emitters -> codegen -> ir` and `compiler -> ir`. Cargo rejects any other edge, so an emitter cannot reach into the compiler and nothing outside `veld-project::output` writes files.
- **The capability check (`E0801`).** Before `generate`, the core compares the features a contract uses with the generator's `Capabilities` and reports `E0801` for the rest. A generator that declares a capability must implement it; one that does not can never emit wrong code for it.
- **The conformance kit.** `veld-emit-conformance` runs every emitter against fixtures for each `Feature` and checks the capability gate, determinism and embedded signatures. `veld plugin test` runs the same protocol checks against an external plugin, including its declared language and framework matrix.
- **Toolchain tests.** Generated code is compiled and run with the real tools: `javac --release 17` and `--release 21`, and `tsc --target es2020` and `es2022`, the matrix being the supported language versions. CI installs JDK 17 and 21 for it. A new language version or framework version joins the matrix in the same change.

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
