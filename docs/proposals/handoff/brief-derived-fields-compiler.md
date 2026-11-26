# Brief: derived fields outside the generators

You work in the Veld repository (a Rust contract-first API code generator) in your own git worktree. First run `git log --oneline -3`. If the head is older than `__HEAD__`, run `git merge --ff-only feat/rules-and-events` before anything else. Read `CLAUDE.md` at the repository root first.

## Goal

Implement derived fields (`total: decimal = subtotal - discount`) everywhere **except** the generators: the parser, the semantic checks, the IR, the reference evaluator (`crates/veld-rules`), `veld test` and `veld check`, `veld fmt`, `veld diff`, the OpenAPI export, the fuzzer, the language server, the conformance cases, the shared vectors. Every generator must refuse a contract that has a derived field (`E0801`, feature `rules_derived`) until it implements them.

The owner confirmed on 25 November 2026: **an expression after `=` makes a field derived, a literal after `=` is a default; there is no keyword.** In the type a handler returns a derived field is optional, and whatever the handler puts there is overwritten.

## Read first

- `spec/rules.md` section 17.2: this is the specification. Then sections 3 (expressions), 4 and 5 (invariants, the pipeline, where a value is checked), 6 (ids), 9.7 (the rules digest), 12.2 (how an example's values are filled in), 13 (the IR), 15 (vectors).
- How the last feature of this kind was added outside the generators: `git log --oneline --grep=rollout` and read those commits. Follow the same shape: an IR key that is omitted when absent, an opt-in feature (`Feature::OPT_IN`) with its `E0801` gate and a conformance helper that every built-in emitter's conformance test calls, vectors with a `.pending` suffix.

## What to build

1. **Parser and checks.** A field's `=` followed by a literal stays a default exactly as today; anything else is a derived expression. A negative literal (`= -1`) is a literal. The scope and the errors of 17.2 (`E0902`, `E0960` for a cycle, `E0961`), each with a conformance case; an optional field read without a guard is the existing guard error.
2. **IR.** `Field` gains `"derived": Expr`, omitted when absent; the field id of 17.2; the digest rule of 17.2 (a contract with no derived field keeps the digest it has today, and keeps its whole IR byte for byte). `spec/ir.md`, the schema, the regenerated `spec/ir.d.ts`.
3. **Reference evaluator.** Derived values are computed in dependency order: on a request after defaults and before the invariants (a value the caller sent is ignored); on a response, a declared error and a stream item before the output rules; on a fact after it is loaded; on an event. An evaluation error is a violation with the field's id and `error` set. Nested declarations, lists, maps, pages and union arms are covered the way invariants are.
4. **Examples.** A derived field is never filled in and must not be written in an example (`E0952`); the result an example `returns` gets its derived fields computed before the pattern after `ok` is matched.
5. **Tools.** `veld fmt`; `veld diff` (a changed derived expression is "possibly breaking"; decide and test: a field that becomes derived, a derived field that becomes a plain field, a new derived field); the OpenAPI export (`readOnly` and `x-veld-derived`; a contract without derived fields exports the same bytes); the fuzzer (it must not send or expect wrong values for derived fields; say what you did); the language server (hover on a derived field shows its expression; completion inside the expression if the server already completes inside rule expressions, otherwise say so).
6. **Vectors.** Core vectors under `tests/rules/core/` (a derived field read by an invariant; a chain of derived fields; a derived field over a list with `sum`; an evaluation error; a value sent by a caller is replaced; request position; nested and inherited) and endpoint examples under `tests/rules/endpoints/` (a request whose derived value decides a `require`; a result whose derived field is computed; a fact). Suffix `.pending`; a test in `veld-rules` pins their counts and runs them through the reference evaluator. Describe in `spec/rules.md` section 15 any new key a vector case needs (for example the expected value after computing).

## What is not part of this task

Any generator code. `@version`, `@idempotent`, events, config.

## Snapshots and stored files

A snapshot or stored file that existed before your work must not change. If one wants to change, stop and report it. A new snapshot of a new test: do not store it yourself (a permission check blocks agents from storing snapshots, and you must not work around it). Run with `INSTA_UPDATE=no`, let those tests fail for the missing file, and list each one in your report; I store them after merging. The regenerated `spec/ir.d.ts` and `spec/diagnostics.md` change by the new names only.

## Rules of the house

- Library code never panics: no `unwrap`, `expect`, `panic` outside tests. Functions under about 60 lines. Deterministic output.
- Never weaken or delete a test to make it pass. A test edit that is not mechanical is named in your report with the reason.
- Where the spec is unclear or silent, choose, write the choice into `spec/rules.md` in a commit of its own, and list it in your report.
- Small commits, each one building. End every commit message with:

  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01FPDem67EsuxeicxHPWeo57
  ```

  Write the message to a file with the Write tool and commit with `git commit -F`. Do not push and do not open a pull request.
- Do the work yourself. Do not start other agents.

## This machine

- Windows 11. Use Git Bash for cargo: `export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=C:/veld-agent-target-derived`. Other builds run on this machine at the same time: keep to 4 jobs and run one cargo command at a time.
- Never run bare `npm` or `npx` in Git Bash: they hang. Use PowerShell with `cmd /c "..."`. There is no `python`.
- The toolchain tests need `tests/toolchain/ts/node_modules`: copy it from `<repo>\tests\toolchain\ts\node_modules`. Copy, never link.
- A foreground command stops after 10 minutes: run the whole test suite in the background with the commands inline (a nested `bash script.sh` can hang here) and read its log.
- Never read from, write to or build on the `S:` and `R:` drives. Do not touch `C:\dw-*` or any process that is not yours.
- **Scratch files:** only in `<scratch>/agent-derived` (make that folder). Delete only that folder when you are done. Everything else under `scratchpad` belongs to someone else.
- A recursive `grep -r` from the repository root times out; use the search tool.

## Before you hand back

Run and report the exit code and totals of each: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo xtask codegen --check`; `VELD_REQUIRE_TOOLCHAINS=1 INSTA_UPDATE=no cargo test --workspace --no-fail-fast -- --skip _against_the_real_registry` (background, log to your scratch folder; for every failure say whether it also fails without your commits).

Then report: the commits, oldest first; `git diff --diff-filter=M --name-only <base> HEAD` with a reason for every modified snapshot, lock file or file under `tests/`, `examples/`, `website/`, `.github/`; the choices you made where the spec was silent; a section "for the generator agent"; anything not finished, plainly; what you created outside the worktree (leave `C:\veld-agent-target-derived` for me to delete; delete your scratch folder).
