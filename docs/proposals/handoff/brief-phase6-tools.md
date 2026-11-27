# Brief: `veld test --coverage`, `veld export rules`, `veld test --from-violation` (the tool side)

You work in the Veld repository (a Rust contract-first API code generator) in your own git worktree. Read `common.md` first: it is the law of this machine. Then `CLAUDE.md` at the root of your worktree.

Several other agents work at the same time. One is in the compiler (`crates/veld-compiler`, `crates/veld-ir`, `crates/veld-rules`) adding derived fields and also edits `veld fmt`, `veld diff` and the OpenAPI export in `crates/veld-tools`; three are in the emitters; one is in `crates/veld-lsp`. **Your area:** `crates/veld-testing`, new modules in `crates/veld-tools`, and the `test` and `export` commands of `apps/veld`. Do not change the compiler, the IR, the emitters or the language server. If you need something from `veld-rules` that is not public, add a small read-only function and say so in your report.

## Goal

Three tools of `spec/rules.md`, none built yet:

1. **`veld test --coverage`** and `--fail-under N` (section 19.1).
2. **`veld export rules [-o FILE]`** (section 19.2).
3. **`veld test --from-violation <file>`** (section 17.3), the tool side only: reading a violation event with a capture and printing the `example` block. The server option that *produces* a capture belongs to the generators and is not yours; define the JSON of a capture (below) so they can be held to it.

## Read first

- `spec/rules.md` sections 19.1, 19.2, 17.3, and what they lean on: 5.3 (the event of `onRuleViolation`), 6 (rule ids), 9 to 11 (endpoint rules, state machines, policies), 12 (examples, `veld test`, 12.2 how an example's values are filled in), 17.1 (audited rules), 19.3 (`redacted` in a capture).
- How `veld test` runs today: `apps/veld/src/commands/` (the `test` command), `crates/veld-testing` (the shared runner, the report, the fuzzer's paste-ready `example` printer, which `--from-violation` should reuse), `crates/veld-rules` (`Pipeline::run_reported`, `ExampleReport`).
- How `veld export openapi` is wired (the `export` command, the user-owned output file, `-o`), and how `veld fmt` prints an expression (the rule book prints expressions "as `veld fmt` prints" them: reuse that printer, do not write a second one).
- `docs/guides/testing.md` and `docs/guides/rules.md`: each gets a short section for what you build.

## What to build

**Coverage.** Exactly 19.1: the units, held and violated, covered, the report format with its example lines, the percentage rounded down, `coverage: no rules`, `--fail-under`, with and without `--live`. The reference evaluator must tell you which units an example evaluated and how each ended; if `ExampleReport` does not carry that yet, add it in `veld-rules` as data the pipeline already has (additive, no behaviour change, the fields skipped when unused so no stored output changes), and list it in your report. JSON output: if `veld test` has a machine-readable output, coverage is in it.

**The rule book.** Exactly 19.2: deterministic Markdown, per service package in name order, the five parts in order, audited and deprecated marks, the sentence for a workspace without rules. State machines as a table. Events with invariants are declarations with invariants. Works with `-p`/`--workspace` like the other exports.

**From a violation.** Define the capture in `spec/rules.md` 17.3, in a commit of its own (the section today says what a capture contains, not its JSON): the keys of the violation event the tool reads, `capture: { service, endpoint, operation (the signature), params (wire form), facts, principal, now, redacted: [wire paths] }`. Keep it consistent with the event shape of 5.3 and with what the test handler already takes in its headers (facts, principal, now), so a generator can build one from what it has. Then the command: read the file, find the endpoint by operation signature, print the `example` block with the outcome the reference evaluator gives, fill in redacted values as 19.3 says and say which were filled in, write nothing; the three error cases of 17.3 with exit code 1.

## Tests (written, compiled, not run: see `common.md`)

- Coverage: a fixture contract of your own under `crates/veld-testing/tests/fixtures/` (do not read `examples/` in a test: examples change) with a rule that is covered, one never violated, one never evaluated, an `allow` never denied, a `transition` never illegal, an `ensure` without `returns`, an audited rule; the exact report text; the percentage rounding; `no rules`; `--fail-under` exit codes; the same numbers with `--live` if a live test is cheap to write with the existing helpers, otherwise say so.
- Rule book: the full Markdown of a fixture with every part, as a snapshot test (you do not store the snapshot) plus plain assertions for order and marks; determinism (two runs, same bytes); the no-rules sentence.
- From a violation: a capture that round-trips into an example which the compiler accepts and whose outcome matches; a redacted capture; the three error cases.
- CLI tests in `apps/veld/tests/` for the three, following the existing CLI tests (`apps/veld/tests/common/mod.rs` has the helpers; fixtures use the workspace layout of `veld.toml`).
- The help text of `veld test` and `veld export` names the new flags; if a test pins the help text or the website's CLI page is generated from it, update what is generated and say so.

## Status file

Do not edit `docs/proposals/rules-and-events-status.md`; say in your report what to tick.

## Your report

As `common.md` section 4 says, plus the capture's JSON in full, for the generator agents.
