# Writing a Veld emitter

An emitter turns the IR (`spec/ir.md`) into files. Veld has two kinds. Pick one before you start.

## Built-in emitter or plugin

| | Built-in (Rust crate in `crates/emitters/`) | Plugin (`veld plugin new`) |
|---|---|---|
| Language | Rust | Anything that reads and writes JSON lines (the scaffold uses TypeScript) |
| Ships | Inside the `veld` binary | As a separate executable pinned in `veld.toml` by `sha256` |
| Choose it when | The target is first-party, or it must be fast and always available | The target is yours, experimental, or best written in its own language |
| Contract | The `Generator` trait | `spec/plugin-protocol.md` (generated types in `spec/ir.d.ts`) |
| Checked by | `veld-emit-conformance`, snapshots, toolchain tests | `veld plugin test` (the same checks over the protocol) |

Both obey the same rules below. A plugin is not a lesser citizen: the host enforces the paths, limits and capabilities.

## The generator contract

- A generator declares its **roles** (`server`, `client`) and, per role, its **frameworks** and **capabilities**.
- **Options** are typed. A Rust generator has an `Options` type that implements `Deserialize + Default`, and Veld fills it from `[server.options]` or `[client.options]`. A plugin may publish a JSON Schema for them in its `hello`.
- `generate(request, options)` is a **pure function**. It receives the IR, the role and the selected services. It returns a `FileSet`, an ordered map of relative path to bytes, or a list of diagnostics. It does **no I/O**: it never reads or writes the disk, the network or the clock. Veld writes the files through `veld_project::output`, which stages them, keeps a manifest and checks that every path stays inside `out`.
- **Output is deterministic.** Use `BTreeMap` or sorted `Vec`. Put no timestamps, absolute paths or host names in the output.
- **Generated code has zero runtime dependencies**, except a framework adapter for the framework the user chose.
- An emitter never depends on `veld-compiler`. It sees only `veld-ir`.
- Library code never panics: no `unwrap`, `expect` or `panic`.

## Capabilities and E0801

The compiler computes the features a contract uses (`unions`, `constraints`, `auth`, `streaming_ws`, and so on). Before it calls your generator, it compares them with your declared `Capabilities` and reports **`E0801`** for each feature you do not support.

- Declare only what you implemented. Add features one at a time, each with a test.
- If you meet something you cannot represent, return `E0801`. Never emit wrong or partial code.

## Signature and equality obligations

(`spec/ir.md` section 5.)

- Every declaration and every endpoint carries a `sig`. **Embed them verbatim** in the generated files. Never compute or reformat one: signatures are computed only in `veld-compiler::signature`.
- The generated runtime must provide `canonical(value) -> bytes` (RFC 8785 JSON of the decoded wire form) and `same(a, b)`:
  `same` is true when the declaration signatures are equal, the operation signatures are equal for errors, and the canonical bytes are equal. It never compares class or prototype identity, so two copies of the same error generated into different packages are `same`.
- `X.matches(err)` is the looser check: same declaration and same service, from any operation.

## Required tests

Tests ship with the emitter. Never weaken or delete one to make it pass.

1. **Shared conformance.** `crates/<name>/tests/conformance.rs` calls `veld_emit_conformance::check_generator`. It checks capability coverage (`E0801`), that every `sig` is embedded verbatim, and determinism.
2. **Snapshots.** `insta` snapshots of the output for the fixture contracts. Review changes with `cargo insta test --review`. Agents never accept snapshots on their own.
3. **Toolchain compile and round trip.** Under `tests/toolchain/`, compile the generated code with the real compiler (`tsc`, `javac`, ...) and run a client against a server in a round trip. Set `VELD_REQUIRE_TOOLCHAINS=1` so a missing tool fails instead of skipping. CI sets it.
4. **Wire goldens.** `tests/wire/` holds canonical byte goldens. Your `canonical()` must reproduce them exactly.
5. **Cross-package equality.** Generate two packages that share an error and show that `same` holds across them, and fails for a different service, operation or payload.

## Step by step: a built-in emitter

1. `cargo xtask new-emitter <name> [--roles server,client]` creates `crates/emitters/<name>`, a conformance test and the registry entry.
2. Write `spec/targets/<name>.md` first: type mapping, file layout, runtime API and the capabilities you will support.
3. Implement `generate`. Start with an empty `capabilities` and grow it.
4. Add the five test suites above. Run `cargo test -p veld-emit-<name>`, then `cargo test --workspace`.
5. Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all`.

## Step by step: a plugin

1. `veld plugin new <name>` creates `./<name>` with a TypeScript project: `src/protocol.ts` (handshake and loop), `src/ir.d.ts` (generated IR types), `src/generate.ts` (your code) and a README.
2. `cd <name> && npm install && npm run build`.
3. `veld plugin test node dist/main.js` runs the protocol conformance suite: handshake, path safety, determinism and signature embedding. It exits 1 on any failure.
4. Pin it in the workspace root's `veld.toml`, and use it in the `veld.toml` of the member that gets the generated code:
   ```toml
   # veld.toml of the workspace root
   [plugins.<name>]
   command = "node <name>/dist/main.js"
   version = "0.1.0"
   sha256 = "<sha-256 of the first word of command>"
   ```
   ```toml
   # veld.toml of the member (a `[service]` or an `[app]`)
   [client]
   emitter = "plugin:<name>"
   out = "generated/<name>"   # a plugin has no default `out`
   ```
   The pin is the hash of the executable named first in `command`. Relative file arguments are resolved against the workspace root.
5. Run `veld generate`. Then add a toolchain test for your language, as above.
