# Rules for every agent on this machine (Fedora, from 26 Nov 2026)

Read this file first. It **replaces** three sections of the brief you were given, wherever they disagree: "This machine", every instruction to *run* tests or suites (in "Tests that must exist when you hand back" and "Before you hand back"), and the commit trailer.

## 1. The owner's rule: build everything first, test once at the end

The owner decided on 26 Nov 2026: all features are built first; the whole test suite is run **once, at the end**, by the main session, and then fixed until nothing fails.

So you:

- **Write the code and write its tests.** Every test your brief lists must still exist when you hand back: unit tests, conformance cases, snapshot tests, toolchain test programs, vectors. Write them carefully, as if they will be run by someone else without you there, because they will be.
- **Make everything compile.** For the crates you touched: `cargo check -p <crate> --all-targets` (this compiles the tests too), `cargo fmt --all`, `cargo clippy -p <crate> --all-targets -- -D warnings`. Before you hand back, once: `cargo check --workspace --all-targets` and `cargo fmt --all --check`.
- **Do not run** `cargo test`, `cargo insta`, `cargo nextest`, the workspace suite, or any toolchain round trip that runs a generated program's tests. Do not run a mutation check ("remove the report call and see a test fail"): write the test that would catch it and say which test it is.
- **One exception, and keep it to seconds:** code that your generator emits must itself compile. You may generate one fixture with the debug binary and compile the output with the target's compiler only (`javac -d <scratch>`, `tsc --noEmit`, `cargo check` on the generated crate). That is a compile check. Do not run the output.
- Snapshots: write the snapshot tests, do not create or store `.snap` files. The main session stores them during the final run.

Where your brief says "run and report the totals of the test suite", report instead what you compiled and what you could not check.

## 2. This machine

- Fedora Linux, 24 cores, 31 GB of memory. Other agents build at the same time.
- **Your worktree and your folders are named in the message that started you.** Work only there. Never read from, write to or build in `/data/University/Graduation/Veld` (the main checkout, on a slow Windows disk) and never in another agent's worktree.
- Every cargo command starts with the environment line from your start message (`PATH`, `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS=6`, `CARGO_PROFILE_DEV_DEBUG=0`). Never build without `CARGO_TARGET_DIR` set: the default would fill the disk. No `--release` builds.
- **Disk is short** (about 50 GB free for everyone). No test binaries, no release builds, no copies of `node_modules` you do not need. If your target folder passes 8 GB, say so in your report.
- Java: `export JAVA_HOME=$HOME/.jdks/temurin-25.0.3 PATH=$HOME/.jdks/temurin-25.0.3/bin:$PATH` (JDK 25; CI uses 21 and 17, so write Java 17 source: no features newer than 17). Maven: `$HOME/.m2/wrapper/dists/apache-maven-3.9.14/*/bin/mvn` (you should not need it).
- Node 24 and `npm` work normally. `tests/toolchain/ts/node_modules` is not in your worktree; if you need `tsc` for the compile check, run `npm ci` in `tests/toolchain/ts` of your worktree.
- `grep -r`, `python3` and ordinary shell tools work. There is no Z3 yet.
- **Scratch files** only in the scratch folder named in your start message. Delete only that folder when you are done.

## 3. House rules (unchanged)

- Library code never panics: no `unwrap`, `expect`, `panic` outside tests. `thiserror` in libraries. Functions under about 60 lines. Deterministic output (`BTreeMap`, sorted `Vec`; no timestamps, absolute paths or host names).
- Unsupported means `E0801`, never wrong code. Zero runtime dependencies in generated code.
- Never weaken or delete an existing test. A snapshot or stored file from before your work must not change; if one must, stop and say so in your report.
- Where a spec is unclear or silent: choose, write the choice into the spec in a commit of its own, list it in your report.
- Do the work yourself. Do not start other agents. Do not push. Do not open a pull request. No `git worktree`, `git stash` or branch switching: stay on the branch your worktree is on.
- **Small commits, each one compiling**, conventional messages (`feat(java): ...`, `test(...)`, `docs(spec): ...`). Commit as you go: an agent can be stopped at any time, and only committed work is safe. Write the message to a file in your scratch folder and commit with `git commit -F`. Every message ends with:

  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01FPDem5j8bL3jNyxF2utX3e
  ```

## 4. Your report

Keep it short and exact:

1. The commits, oldest first (`git log --oneline <base>..HEAD`).
2. What compiles: the exact commands and their exit codes. What you could not compile-check.
3. Every file you modified that existed before (`git diff --diff-filter=M --name-only <base> HEAD`) outside your own crate, with one reason each.
4. The tests you wrote that have never been run, and the ones you expect to need a stored snapshot.
5. The choices you made where the spec was silent.
6. Anything not finished, plainly. Do not call something done that is not.
7. What the next agent needs to know (API names, IR keys, traps).
