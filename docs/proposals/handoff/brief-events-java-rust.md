# Brief: events in the Java generator and in the Rust generator (phase E1)

Two agents get this brief: one builds the **Java** target, the other the **Rust** target. The message that sent you here says which one is yours. Work only in your own emitter (`crates/emitters/java` or `crates/emitters/rust`), its tests and its toolchain test files, and your target's spec file. The other agent is working in the other emitter at the same time; a third is in `crates/veld-lsp`.

You work in the Veld repository (a Rust contract-first API code generator) in your own git worktree. First run `git log --oneline -3`. If the head is older than `6ead190`, run `git merge --ff-only feat/rules-and-events` before anything else. Read `CLAUDE.md` at the repository root first.

## Goal

Make your server generator emit events: a typed publisher and a typed subscriber for every service that `publishes` or `subscribes`, the envelope, the publish and consume pipelines, the `Transport` interface and the built-in memory transport. The **Node generator already does this** and is the reference: what it settled is written into the specs, and your target does the same thing in its own idiom. Until now your server refuses a service with events (`E0801`); when you are done it declares the capability `events` for every framework it has.

## Read first

- `spec/events.md`, all of it. Sections 6 to 9 are what you generate (envelope, 6.3 and 6.4 the settled details; publish; consume and 8.1 the hooks; transport and the memory transport); 12 the event examples; 13 the IR; 14 the generated API; 19 the phases (you build E1 only: no Kafka, no RabbitMQ, no retry policy, no dead-letter channel, no inbox, no outbox, no `emits`); 20 the owner's decisions.
- `spec/targets/typescript.md` section 5f: the Node layout, names and hooks. Mirror them: `create<Service>Publisher`, `<Service>Publisher`, `<Service>PublisherOptions`, `<Service>EventHandlers`, `subscribe<Service>Events`, `<Service>SubscriberOptions`, in your language's casing and idiom.
- Your target's spec (`spec/targets/java.md` or `spec/targets/rust.md`).
- The Node implementation, as a worked example: `crates/emitters/node/src/events.rs`, `crates/emitters/typescript/runtime/events.ts`, `crates/emitters/node/tests/events.rs` and the drivers `tests/toolchain/ts/events_*.ts`.
- `tests/wire/events/README.md`: the byte goldens and how a generator's test uses them. `crates/veld-rules/src/envelope/` is the reference encoder and decoder.
- `crates/veld-ir`: `Ir::events_of`, `Ir::event_channels`, `Ir::event_reachable_decls`, `Ir::event_only_decls`, `Ir::event_features`, `checked_features`, `http_features_used`, `Service::published_channels`, `Publication`.
- How your emitter generates and tests rules today (check functions, the hooks, `onRuleViolation`, the output mode, audited rules): an event's invariants are the same check functions.

## What the generated code must do

Exactly what `spec/events.md` and section 5f describe, in your target:

1. **Types** for every event the service's channels carry and for every declaration only events reach, with the event's signature.
2. **A publisher** per publishing service, one typed method per channel and event. The publish pipeline of section 7 in order: defaults, shape and constraint validation, invariants (an enforced violation stops the publish with the typed error; an audited one is reported and does not), the envelope in canonical bytes, the key as `subject`, `veldrules`, then the transport. The id source and the clock are injectable.
3. **A subscriber** per subscription: a handler interface with one method per `on` line, and a function that connects it to a transport under the subscription's group. The consume pipeline of section 8 in order, with every outcome the goldens name. A rejected message is acknowledged and reported; a handler that fails is not acknowledged and is delivered again. The handler gets the typed event and the metadata.
4. **`Transport`** as section 9 defines it and the **memory transport** with the behaviour section 9 gives it.
5. **The hooks** of section 8.1 with the same names and the same event shapes as Node.
6. **Zero runtime dependencies** beyond what your target already has, deterministic output, the same event files for every framework of your target. A service with events and no endpoints generates only its events.
7. A contract without events must generate the same bytes as before. No existing snapshot may change.

Canonical forms that bit the Node work, so you do not repeat them: write the canonical datetime yourself (UTC, `Z`, a `.fff` fraction only when the milliseconds are not zero; do not reuse a helper that trims trailing zeros); a `long` key is its decimal digits; compile the golden contracts as file `t.veld` in package `bookings`, because the module name is part of the signature hashes.

## Tests that must exist when you hand back

1. **The byte goldens.** For every case of `tests/wire/events/envelopes.json`: generate the code for the case's contract, publish the value through a recording transport with the id and clock fixed, and compare the bytes with `canonical`; then hand that message back to a generated consumer and compare the event and the metadata. For every case of `consume.json`: feed the message to the generated consumer and compare the outcome. Real generated code, compiled and run by your target's real toolchain, as the existing toolchain tests do.
2. **Cross-target check.** A message your publisher produced must be accepted by the Node consumer's golden path and the other way round. The goldens give you this for free if test 1 compares bytes; say so in your report rather than building a second harness.
3. **The event examples** of `tests/events/bookings.veld` (and of `tests/wire/events/edges.veld` if it has any) pass against your generated publisher and subscriber over the memory transport, compared with what the reference evaluator reports. Do not edit an expected outcome.
4. **Two services in one process** through the memory transport: delivery, redelivery when a handler fails, two groups each get the message, ordering per key.
5. **Audited event rules:** remove the report call, see a test fail, restore it, and say which test.
6. Snapshots of the generated event files, and in your emitter's conformance test replace `check_events_are_refused` by `check_events_are_generated` for your target only.
7. The committed output of every example still matches (`cargo test -p veld --test examples`).

## Write down what you establish

Your target's spec file gets a section for events (files, API, hooks, how the memory transport is used), in a commit of its own. Where `spec/events.md` is unclear for your target, choose, write it into your target's spec, and list it in your report. Do not change what `spec/events.md` and section 5f already settle; if you think one of those is wrong, say so in your report instead.

## Not part of this task

The other target. Node and TypeScript. Kafka, RabbitMQ, retry, dead letter, inbox, outbox, `emits`, AsyncAPI, `veld diff`, the website, the test handler (`testing = true`) for events.

## Rules of the house

- Library code never panics: no `unwrap`, `expect`, `panic` outside tests. Functions under about 60 lines.
- Never weaken or delete a test to make it pass. A test edit that is not mechanical is named in your report with the reason.
- Shared crates (`veld-ir`, `veld-codegen`, `veld-emit-conformance`, `veld-rules`): change them only if your target cannot be built otherwise, keep the change minimal and additive, and list it in your report, because the other agent may need the same thing.
- A snapshot or stored file that existed before your work must not change; if one wants to change, stop and report. A new snapshot of a new test: do not store it yourself (a permission check blocks agents from storing snapshots, and you must not work around it); run with `INSTA_UPDATE=no`, let those tests fail for the missing file, and list each one in your report.
- The `veld.toml` layout is the workspace layout (`spec/workspace.md`); any fixture that writes one uses it.
- Small commits, each one building. End every commit message with:

  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01FPDem67EsuxeicxHPWeo57
  ```

  Write the message to a file with the Write tool and commit with `git commit -F`. Do not push and do not open a pull request.
- Do the work yourself. Do not start other agents.

## This machine

- Windows 11. Use Git Bash for cargo: `export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_TARGET_DIR=C:/veld-agent-target-events-java` (Java agent) or `C:/veld-agent-target-events-rust` (Rust agent). Other builds run on this machine at the same time: keep to 4 jobs and run one cargo command at a time.
- **Disk:** the drive your worktree is on is short of space. Keep build output of generated crates and toolchain work folders off it: for the Rust harness tests set `VELD_RUST_WORK` and `VELD_RUST_TARGET` to folders under your `C:/veld-agent-target-…` directory (read the Rust toolchain test helper to see what they do), and delete `tests/toolchain/*/.work` in your worktree when you finish.
- Never run bare `npm` or `npx` in Git Bash: they hang. Use PowerShell with `cmd /c "..."`. There is no `python`. Tests under `apps/veld` that run `npm` can stall on this machine; if one hangs, stop it and say which.
- The toolchain tests need `tests/toolchain/ts/node_modules`: copy it from `<repo>\tests\toolchain\ts\node_modules`. Copy, never link.
- A foreground command stops after 10 minutes: run the whole test suite in the background with the commands inline (a nested `bash script.sh` can hang here) and read its log.
- Never read from, write to or build on the `S:` and `R:` drives. Do not touch `C:\dw-*` or any process that is not yours.
- **Scratch files:** only in `<scratch>/agent-events-java` (Java agent) or `…/agent-events-rust` (Rust agent); make that folder, and delete only that folder when you are done. Everything else under `scratchpad` belongs to someone else.
- A recursive `grep -r` from the repository root times out; use the search tool.

## Before you hand back

Run and report the exit code and totals of each: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo xtask codegen --check`; `VELD_REQUIRE_TOOLCHAINS=1 INSTA_UPDATE=no cargo test --workspace --no-fail-fast -- --skip _against_the_real_registry` (background, log to your scratch folder; for every failure say whether it also fails without your commits).

Then report: the commits, oldest first; `git diff --diff-filter=M --name-only <base> HEAD` with a reason for every modified snapshot, lock file or file under `tests/`, `examples/`, `website/`, `.github/`, and for every file in a shared crate; the generated API in ten lines; the choices you made; the new snapshots I must store; which test failed when you removed the audited report; anything not finished, plainly; what you created outside the worktree (leave your `C:\veld-agent-target-…` folder for me to delete; delete your scratch folder).
