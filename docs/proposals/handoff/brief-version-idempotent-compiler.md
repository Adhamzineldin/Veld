# Brief: `@version` and `@idempotent` outside the generators

You work in the Veld repository (a Rust contract-first API code generator) in your own git worktree. First run `git log --oneline -3`. If the head is older than `c70df09`, run `git merge --ff-only feat/rules-and-events` before anything else. Read `CLAUDE.md` at the repository root first.

## Goal

Implement optimistic concurrency (`@version`) and idempotent endpoints (`@idempotent`) everywhere **except** the generators: the language table, the parser, the semantic checks, the IR, the reference evaluator (`crates/veld-rules`), `veld test` and `veld check`, `veld fmt`, `veld diff`, the OpenAPI export, the fuzzer, the language server, the conformance cases, the shared vectors. Every generator must refuse a contract that uses either (`E0801`) until it implements it.

## Read first

- `spec/rules.md` sections 11.7, 11.8 and 11.9 (the exact behaviour: this is the specification), 5.3 (the pipeline), 12 (examples, `veld test`, the test handler), 13 (the IR), 15 (vectors).
- How the last feature of this kind was added outside the generators: `git log --oneline --grep=rollout` and read those commits (`@rollout(audit)`: language table, parser, IR with a key that is omitted when absent, the feature and its `E0801` gate, reference evaluator, `veld test`, fmt, diff, OpenAPI, LSP, conformance cases, vectors with a `.pending` suffix, the conformance helper `check_rollout_is_refused`). Follow the same shape.
- Decisions of the owner (25 November 2026): a stale `If-Match` is `412 PreconditionFailed`; the caller of an idempotency key is a text the application returns from a server hook, and a server that needs it and has no hook does not start.

## What to build

1. **Language:** `@version` on one `int` or `long` field of a model; `@idempotent` and `@idempotent(header: "Name", required: true)` on an endpoint; in an example, `ifMatch 3`, `ifMatch any` and the `key` line with `stored`, `other`, `running`, all contextual words that stay ordinary names elsewhere (add them to the "new words stay ordinary names" tests). Every error the spec names, each with a conformance case.
2. **Prelude:** `error PreconditionFailed 412 { expected: long, actual: long }`. It appears in an IR only when a contract reaches it, as `InvalidTransition` does. `409 Conflict` for idempotency: use the prelude's existing error if it has one, and say what you found.
3. **IR:** the keys the generators will need (which field is the version; which fact of an endpoint is checked; the idempotency header, whether it is required, whether the scope includes the caller; the implicit errors of an endpoint; the version rule id of 11.9), every one omitted from the JSON when absent, so that a contract without the annotations keeps its IR byte for byte. `spec/ir.md`, the IR schema and the regenerated `spec/ir.d.ts`.
4. **Features:** `rules_version` and `idempotency`. Both are opt-in like `rules_rollout` (`Feature::OPT_IN`): no generator supports them unless it says so. The gate reports `E0801` at the first use. A helper in `crates/veld-emit-conformance` like `check_rollout_is_refused`, called from the conformance tests of the Node, TypeScript, Java and Rust emitters (one new test function per emitter file; another agent is editing those files, so add your function at the end of each file and touch nothing else in them).
5. **Reference evaluator:** the version check in its place in step 6; the idempotency lookup after step 5 with the three store states an example can state; replay without loading facts or calling the handler; what is stored and what is not. `veld test` and `veld check` run such examples offline.
6. **Tools:** `veld fmt` (round trip and idempotence); `veld diff` (adding `@version` to a field, adding or removing `@idempotent`, changing `required`: decide each from what it does to an existing caller, write the table into `spec/rules.md` and test it); the OpenAPI export (the `If-Match` and idempotency-key headers, `ETag`, the `412` and `409` responses; a contract without the annotations exports the same bytes as before); the fuzzer leaves both alone unless that is cheap to do well, and says so; the language server (completion and hover for the two annotations).
7. **Vectors:** a contract under `tests/rules/endpoints/` with examples for every case of 11.9 (a version that matches, a stale one, `any`, no header, a malformed header as far as an example can state it; a key that is new, stored with a success, stored with a declared error, `other`, `running`, a required key that is missing, the caller scope). Suffix `.pending` until a generator supports them; a test in `veld-rules` pins their count and runs them through the reference evaluator, as for `rollout.veld.pending`.

## What is not part of this task

Any generator code. The `Veld-Test-Idempotency` header and `ETag` in generated servers belong to the generators' work; the spec for them exists. Derived fields, events, config.

## Snapshots and stored files

A snapshot or stored file that existed before your work must not change. If one wants to change, stop and report it. A new snapshot of a new test: do not store it yourself (a permission check blocks agents from storing snapshots, and you must not work around it). Run with `INSTA_UPDATE=no`, let those tests fail for the missing file, and list each one in your report; I store them after merging. The regenerated editor grammars, `spec/ir.d.ts` and `spec/diagnostics.md` are expected to change by the new names only.

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

- Windows 11. Use Git Bash for cargo: `export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=C:/veld-agent-target-version`. Other builds run on this machine at the same time: keep to 4 jobs and run one cargo command at a time.
- Never run bare `npm` or `npx` in Git Bash: they hang. Use PowerShell with `cmd /c "..."`. There is no `python`.
- The toolchain tests need `tests/toolchain/ts/node_modules`: copy it from `<repo>\tests\toolchain\ts\node_modules`. Copy, never link.
- A foreground command stops after 10 minutes: run the whole test suite in the background with the commands inline (a nested `bash script.sh` can hang here) and read its log.
- Never read from, write to or build on the `S:` and `R:` drives. Do not touch `C:\dw-*` or any process that is not yours.
- **Scratch files:** only in `<scratch>/agent-version` (make that folder). Delete only that folder when you are done. Everything else under `scratchpad` belongs to someone else.
- A recursive `grep -r` from the repository root times out; use the search tool.

## Before you hand back

Run and report the exit code and totals of each: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo xtask codegen --check`; `VELD_REQUIRE_TOOLCHAINS=1 INSTA_UPDATE=no cargo test --workspace --no-fail-fast -- --skip _against_the_real_registry` (background, log to your scratch folder; for every failure say whether it also fails without your commits).

Then report: the commits, oldest first; `git diff --diff-filter=M --name-only <base> HEAD` with a reason for every modified snapshot, lock file or file under `tests/`, `examples/`, `website/`, `.github/`; the choices you made where the spec was silent; a section "for the generator agent" (IR keys to read, the reference API that gives expected results, what the vectors cannot state); anything not finished, plainly; what you created outside the worktree (leave `C:\veld-agent-target-version` for me to delete; delete your scratch folder).

## Three things about your base

- `@rollout(audit)` is now implemented in all four built-in generators, so their conformance tests no longer call a "refused" helper for it; for your two new features write the "refused" helpers and calls as this brief says (your features are implemented in no generator yet). Two other agents are working in the emitters right now (events in the Node emitter; fixes in the Java and Rust emitters): in `crates/emitters/*` touch only the conformance test files, and only by adding your function at the end.
- The `veld.toml` layout changed just before your base (`spec/workspace.md`, `docs/guides/workspaces.md`): a workspace root plus one file per member, or the single-file form; the earlier format is the error `E0709`. Any test or fixture you add that writes a `veld.toml` uses the new layout; `apps/veld/tests/common/mod.rs` has the helpers the other CLI tests use.
- Two tests fail on your base for a known reason and are not yours to fix: the `init` snapshot tests `summary_snapshot` and `full_flow_transcript_snapshot` in `apps/veld`. Report any other failure.
