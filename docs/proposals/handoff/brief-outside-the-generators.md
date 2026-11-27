# Brief: a feature "outside the generators" (the shared recipe)

Several agents get this brief, each for another feature. The message that started you names your feature, its spec sections and what is special about it. Read `common.md` first (the law of this machine), then `CLAUDE.md` at the root of your worktree, then this file, then your spec sections.

"Outside the generators" means: everything a feature needs **except** code in `crates/emitters/*`. When you are done, the compiler accepts the feature, every tool understands it, the reference evaluator gives the expected result for it, shared vectors exist for the generators to be held to, and every generator **refuses** the feature with `E0801` until it implements it.

## The recipe

It has been followed three times on this branch. Read how before you write anything: `git log --oneline --grep=rollout`, `git log --oneline --grep=version -25`, and the files those commits touch.

1. **Language.** The lexer does not change unless your spec says so: new words are **contextual** (`crates/veld-compiler/src/language.rs` and the lookahead of `spec/rules.md` section 2.2), so they stay ordinary names elsewhere; add each to the "new words stay ordinary names" tests. Parser and AST with comments kept. Every check your spec names, each with the code it names. A new diagnostic code goes into the table that `spec/diagnostics.md` is generated from (`crates/veld-compiler/src/codes.rs`), or a table test rejects it.
2. **IR** (`crates/veld-ir`). The keys your spec names, **each omitted from the JSON when absent**, so a contract without the feature keeps its IR byte for byte. `spec/ir.md`, the schema (`crates/veld-ir/src/schema.rs`) and the regenerated `spec/ir.d.ts`. Ids and signatures are computed only in `veld-compiler::signature`.
3. **The feature flag.** Your feature is a `Feature` that is **opt-in** (`Feature::OPT_IN`): no generator supports it unless it says so, and the capability check reports `E0801` at the first use. Write a helper `check_<feature>_is_refused` in a **new file** of `crates/veld-emit-conformance` (model: `src/version.rs`), with small contracts that use the feature, and call it from one new test function in the conformance test file of the Node, TypeScript, Java and Rust emitters. That one function per file is the only thing you touch in `crates/emitters/*`. Put it directly after the last existing `..._is_refused`/`..._generated` test of the file's rules section, not at the very end, and keep it to a few lines: other agents add theirs too.
4. **The reference evaluator** (`crates/veld-rules`). The exact behaviour of your spec, in the pipeline step it names. This is what the generators will be compared with, so it must be complete and it must not guess: where the spec is silent, choose, and write the choice into the spec.
5. **Examples, `veld test`, `veld check`.** Any example syntax your spec adds; examples run offline through the reference evaluator.
6. **Tools** (`crates/veld-tools`, `apps/veld`). `veld fmt` prints the new syntax (round trip and idempotence tests). `veld diff` judges each change of the feature by what it does to a caller or consumer built against the old contract; write the table into your spec and test it. The OpenAPI export gets what your spec says, and a contract without the feature exports the same bytes. A new export or flag your spec names lives under the existing commands (`veld export <x>`, flags of `veld check` or `veld test`): no new top-level command.
7. **Vectors.** Shared vectors under `tests/rules/` (or `tests/events/`, `tests/wire/` when your spec says so) with the suffix `.pending`, covering every case of your spec; a test in `veld-rules` pins their count and runs them through the reference evaluator.
8. **Conformance cases** (`tests/conformance/`): one for every new error and warning and at least one `ok_` case. You write the inputs; the snapshots are stored by the main session.
9. **Generated files.** Run `cargo xtask codegen` (it writes the editor grammars, `spec/ir.d.ts`, `spec/diagnostics.md`; it runs no tests) and commit what it changes with the commit that causes it.
10. **Not yours:** `crates/veld-lsp` (one agent owns it; say in your report what the language server still needs for your feature), the website, the guides, and any generator code.

## You are not alone in these crates

Other agents add other features to the same crates at the same time. You will collide in a few shared lists: `Feature::OPT_IN` and `Feature::ALL`, the annotation table, the code table, the schema, the regenerated files. That is expected; the main session merges. Make it easy:

- put your work in **new files and new modules** wherever you can;
- add to a shared list next to the related entries, one entry per line, without reordering or reformatting anything else;
- never run `cargo fmt` on a file you did not otherwise change, and never "tidy" shared code;
- do not rename or move anything that exists.

## Your report

As `common.md` section 4 says, plus a section **"for the generator agents"**: the IR keys to read, the reference functions that give expected results, the vectors and what they cannot state, the names your spec fixes for generated code, and anything in your spec sections you found unclear, contradictory or wrong. Do not rewrite the spec's decisions; fix a plain contradiction in a commit of its own that says so, and list it.
