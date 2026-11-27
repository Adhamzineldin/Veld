# Brief: the language server understands rules

You work in the Veld repository (a Rust contract-first API code generator) in your own git worktree. First run `git log --oneline -3`. If the head is older than `c2673d9`, run `git merge --ff-only feat/rules-and-events` before anything else. Read `CLAUDE.md` at the repository root first.

## Goal

`crates/veld-lsp` (started by editors through the hidden `veld lsp` command) knows declarations, events and a few annotations. It knows almost nothing about business rules. Make hover, completion, go-to-definition and the outline work for everything `spec/rules.md` adds that is built today, **inside expressions too**. Work in `crates/veld-lsp` only (plus a read-only helper in another crate if the server truly needs one; say so in your report). Three other agents are editing the compiler, the tools and the emitters right now: do not touch those crates' behaviour.

## Read first

- `spec/rules.md` sections 3 (expressions: operators, builtins, guards, comprehensions), 4 and 5 (invariants), 9 (endpoint blocks: `fact`, `require`, `ensure`, `old`, `now`, `result`), 10 (state machines: `flow`, `final`, `transition`), 11 (policies: `principal`, `policy`, `permit`, `forbid`, `allow`), 12 (examples), 17.1 (`@rollout(audit)`).
- `crates/veld-lsp/src/`: how hover, completion, definition and the outline are built today, how the server gets a syntax tree and an IR, what it does when the file has errors, and the tests (`event_tests.rs` is the most recent pattern).
- `examples/bookings/contracts/bookings.veld`: a real contract with every rule kind. Use it as your main fixture.

## What must work

First find out what already works: write a test for each item below against the bookings contract, see which fail, and report that list. Then make them pass.

1. **Hover**
   - on a rule name, a `require`, an `ensure`: its text, its message, whether it is audited, and its rule id;
   - inside an expression: on a field (its type, optionality, constraints and where it is declared), on a parameter, a fact, `result`, `now`, `old(...)`, the caller (`user` or the name after `as`), an enum literal, a `const`, a builtin (`len`, `has`, `sum`, `all`, `any`, `count`, `matches`, `days` and the rest: signature and one line of meaning, taken from one table, not typed twice);
   - on a `policy` name where it is declared and where an `allow` uses it (its parameters, its `permit` and `forbid` lines);
   - on `principal`; on a state in a `flow` or a `transition` (which moves leave it, whether it is final); on an `example` name.
2. **Completion**
   - inside an expression: the fields in scope (after a dot, the fields of that value's type, through optional and nested values), parameters, facts, `result`, `now`, `old`, the caller, the builtins, enum literals after `.` where an enum is expected;
   - the statement words of an endpoint block at the start of a line (`fact`, `require`, `ensure`, `allow`, `transition`), `else` after a `require`, error names after `else`, policy names after `allow`, states after `transition x.f:` and in a `flow`, the example words (`given`, `as`, `at`, `returns`), endpoint names and their parameters in an example call;
   - `rule` and `flow`/`final` snippets where they may stand.
3. **Go to definition** from an expression to the field, parameter, fact, const, enum variant or policy it names; from an `allow` to its policy; from an example call to the endpoint; from `else E` to the error.
4. **The outline** lists rules under their model, the statements of an endpoint block under the endpoint, policies, the principal and examples.
5. **Semantic tokens**, if the server has them for the rest of the language: the rule words and builtins get sensible token types.
6. **A file with errors.** Completion must still work while the user is typing an unfinished expression (that is when it is needed). Say exactly what works on a file that does not compile and what does not.

## Not part of this task

`@version`, `@idempotent`, derived fields (another agent is adding their syntax now). The playground. Editor plugin code under `editors/`. Any change to diagnostics.

## Rules of the house

- Library code never panics: no `unwrap`, `expect`, `panic` outside tests. Functions under about 60 lines. The server must never crash on any input: add a test that feeds it every prefix of the bookings contract (cut at every byte that is a character boundary) for hover and completion at the end and requires an answer or an empty one.
- Never weaken or delete a test to make it pass.
- A snapshot or stored file that existed before your work must not change. A new snapshot of a new test: do not store it yourself (a permission check blocks agents from storing snapshots, and you must not work around it); run with `INSTA_UPDATE=no`, let those tests fail for the missing file, and list each one in your report. Prefer plain assertions over snapshots.
- Small commits, each one building. End every commit message with:

  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01FPDem67EsuxeicxHPWeo57
  ```

  Write the message to a file with the Write tool and commit with `git commit -F`. Do not push and do not open a pull request.
- Do the work yourself. Do not start other agents.

## This machine

- Windows 11. Use Git Bash for cargo: `export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=C:/veld-agent-target-lsp`. Other builds run on this machine at the same time: keep to 4 jobs and run one cargo command at a time. Most of the time `cargo test -p veld-lsp` is all you need.
- Never run bare `npm` or `npx` in Git Bash: they hang. There is no `python`.
- A foreground command stops after 10 minutes: run the whole test suite in the background with the commands inline (a nested `bash script.sh` can hang here) and read its log. If the whole suite needs `tests/toolchain/ts/node_modules`, copy it from `<repo>\tests\toolchain\ts\node_modules` (copy, never link).
- Never read from, write to or build on the `S:` and `R:` drives. Do not touch `C:\dw-*` or any process that is not yours.
- **Scratch files:** only in `<scratch>/agent-lsp` (make that folder). Delete only that folder when you are done. Everything else under `scratchpad` belongs to someone else.
- A recursive `grep -r` from the repository root times out; use the search tool.

## Before you hand back

Run and report the exit code and totals of each: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p veld-lsp`; and once, in the background, `VELD_REQUIRE_TOOLCHAINS=1 INSTA_UPDATE=no cargo test --workspace --no-fail-fast -- --skip _against_the_real_registry` (for every failure say whether it also fails without your commits).

Then report: the commits, oldest first; the list of what worked before and what works now, item by item from the list above; what does not work on a file with errors; anything not finished, plainly; what you created outside the worktree (leave `C:\veld-agent-target-lsp` for me to delete; delete your scratch folder).
