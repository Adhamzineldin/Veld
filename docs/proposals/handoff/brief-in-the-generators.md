# Brief: a feature in the generators (the shared recipe)

Several agents get this brief, each for one feature in one target. The message that started you names your feature, its spec sections, your target and what is special. Read `common.md` first (the law of this machine), then `CLAUDE.md` at the root of your worktree, then this file, then your spec sections.

## The three targets

- **Node**: `crates/emitters/node` (the Node server; every adapter it has: express, fastify, hono, node-http, nest) and `crates/emitters/typescript` (the TypeScript client and the shared runtime). Test programs: `tests/toolchain/ts/`. Spec: `spec/targets/typescript.md`.
- **Java**: `crates/emitters/java` (client, JDK server, Spring adapter). Test programs: `tests/toolchain/java/`. Spec: `spec/targets/java.md`. Java 17 source level.
- **Rust**: `crates/emitters/rust` (client and axum server). Harness programs: `tests/toolchain/rust/`. Spec: `spec/targets/rust.md`.

Work only in your own emitter, its tests, its toolchain test files and your target's spec file. Other agents work in the other emitters, in the compiler and in the language server at the same time. Never touch another emitter. Shared crates (`veld-ir`, `veld-codegen`, `veld-emit-conformance`, `veld-rules`, `veld-testing`): change them only if your target cannot be built otherwise, keep the change small and additive, and list it in your report, because the other targets may need the same thing.

## What "in the generators" means

The feature is already implemented outside the generators: the compiler accepts it, the IR carries it, the reference evaluator (`crates/veld-rules`) gives the expected result, shared vectors exist, and your generator **refuses** it with `E0801` because it does not declare the feature's capability. You make your target generate it, then declare the capability.

1. **Read the reference first.** Your spec sections; the IR keys (`spec/ir.md` and `crates/veld-ir`); the reference implementation in `crates/veld-rules`; the vectors and goldens under `tests/`; the helper `check_<feature>_is_refused` in `crates/veld-emit-conformance` and the contracts it compiles.
2. **Find the nearest thing your emitter already does** and follow it from the IR to the generated code, to the hooks, to the test handler and to its tests, before you write anything. The message that started you names it. New code goes where that code is and looks like it.
3. **Generate exactly what the spec says**, in your target's idiom. The spec fixes the names in TypeScript spelling; Java uses the same words in its casing, Rust in snake case. Where the spec is unclear for your target: choose, write the choice into your target's spec file, list it in your report. Do not change the shared specs (`spec/rules.md`, `spec/events.md`, `spec/platform.md`, `spec/runtime-config.md`): if you think one is wrong, say so in your report.
4. **Gates.** A contract that does not use the feature generates **the same bytes as before**: gate new runtime code the way your emitter already gates optional runtime code (regions, optional files). No existing snapshot may change (unless the message that started you says this feature changes existing output on purpose). Zero new runtime dependencies in generated code, unless the message that started you names the one allowed dependency. Deterministic output. `E0801` for what your target truly cannot represent, never wrong code.
5. **Declare the capability** with `Capabilities::with(Feature::…)` for each role (server, client) once that role does everything the spec asks, and replace the `check_<feature>_is_refused` call in your emitter's conformance test by a check that the same contracts generate. If the "generated" helper does not exist, add it next to the "refused" one in `crates/veld-emit-conformance` in the obvious shape (same contracts, no diagnostics, at least one generated file), and say so: the other targets' agents may add the same function, and identical copies merge.
6. **Vectors stay `.pending`.** Do not rename shared vector files: the other targets' runners read whole folders and are not done at the same moment. Your tests read a `.pending` file by its name. The main session renames them when every target is in.

## Tests that must exist when you hand back (written and compiled, not run)

- **The shared vectors and goldens through your generated code**, by the mechanism your target already uses for the nearest feature (the generated test handler driven by `veld_testing::Live`, a vector runner, a byte-golden comparison). No expected outcome edited.
- **Behaviour over the real entry point**: a test program per feature that exercises every case of your spec sections against the production handler, the way the nearest feature's `*_http` program does, and checks what reaches the hooks.
- **The client side**, when the feature has one.
- **Snapshots** of the generated files of a small contract that uses the feature (you write the snapshot tests; the main session stores the `.snap` files).
- **The conformance check** of point 5.
- For "prove the test bites" items: do not run a mutation; name the test that would catch the missing call.

Compile check of the generated code is allowed and expected (`common.md` section 1): generate your fixture and compile the output with your target's compiler, together with your test programs. Do not run them.

## Write down what you establish

Your target's spec file gets a section for the feature (files, types, hooks, client surface, test handler, tests), in a commit of its own.

## Your report

As `common.md` section 4 says, plus: the generated API of your target in at most ten lines; whether you added a conformance helper; every shared-crate change.
