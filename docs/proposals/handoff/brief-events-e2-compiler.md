# Brief: events phase E2 outside the generators

You work in the Veld repository (a Rust contract-first API code generator) in your own git worktree. Read `common.md` first: it is the law of this machine. Then `CLAUDE.md` at the root of your worktree.

## Goal

Phase E2 of events is specified in `spec/events.md` section 10 (rewritten on 26 Nov; read all of it, it is new). Build everything of it that is **not** generator code:

- the three channel annotations `@retry(max: n, backoff: fixed | exponential)`, `@deadLetter`, `@ordered` (today each is rejected with `E0601`);
- `expect S.handler called once` in an event example;
- the IR keys and the opt-in feature `events_delivery` with its `E0801` gate;
- the reference side: the consumer outcome with retry and dead letter, the dead-letter copy bytes, the inbox;
- `veld test` offline and `veld test --live` for event examples;
- `veld fmt`, `veld diff`, lints, conformance cases, vectors and byte goldens.

Every generator must refuse a contract that uses one of the three annotations (`E0801`) until it implements them.

## Read first

- `spec/events.md`: sections 6 to 9 (the envelope, publish, consume, the transport), **10 (all)**, 12 (examples), 13 (IR), 16 (`diff`), 17 (diagnostics).
- How E1 was built outside the generators: `git log --oneline --grep=event -40` and the code it names: the parser and checks for `event`, `channel`, `publishes`, `subscribes` in `crates/veld-compiler`; `crates/veld-rules/src/envelope/` (the reference encoder, decoder and consume outcomes); `tests/wire/events/` (the byte goldens and their README); `tests/events/bookings.veld` (the event examples); how `veld test` runs event examples.
- How the last opt-in feature was added: `crates/veld-ir/src/lib.rs` (`Feature::OPT_IN`), `crates/veld-emit-conformance/src/version.rs` and `rollout.rs` (the "refused" helpers), and where the built-in emitters' conformance tests call them.
- `crates/veld-testing/src/live.rs` and `conditional.rs`: how a live run talks to a test handler today (`GET /__veld/test`, the headers), because the live event run extends it.

## What to build

1. **Language.** The three annotations before `channel`, with the checks of 10.2 (`E0602` for a wrong place, a repeated one, a missing or out-of-range `max`, an unknown `backoff`, an argument on `@deadLetter` or `@ordered`). `<topic>.dlq` of a channel with `@deadLetter` counts as a topic for `E1004`. The warning `W1003`. `once` after `called` in an example (`E1010` for `not called once` and for `once` with `rejected`); `once` stays an ordinary name elsewhere (add it to the "new words stay ordinary names" tests).
2. **IR.** The keys of 10.8, each omitted when absent, so a contract without them keeps its IR byte for byte. `spec/ir.md`, the schema, the regenerated `spec/ir.d.ts`.
3. **The feature** `events_delivery`, opt-in, used by a service that publishes or subscribes to a channel with one of the three annotations; `E0801` at the first such annotation. A helper `check_events_delivery_is_refused` in `crates/veld-emit-conformance` (new file), called from one new test function in the conformance test file of the Node, Java and Rust emitters (the TypeScript client generates nothing for events: check that it keeps its bytes). Other agents are editing those test files: add your function directly **after the existing events test function**, and touch nothing else in `crates/emitters/*`.
4. **Reference.** In `crates/veld-rules/src/envelope/` (new files where you can): the dead-letter copy of 10.5 as a pure function (bytes in, reason, group, attempts, issues, violations; bytes out); the consume outcome extended with what 10.4 decides (how many runs, whether the copy is made, the final answer to the transport) for a given channel policy and a given list of handler results; a reference `MemoryInbox`. Byte goldens for the copy under `tests/wire/events/` (a new file `dead-letter.json`: cases for each reason, a non-object message, a message that already has unknown members, empty and non-empty issues and violations) with the README extended; the generators will be held to these bytes.
5. **`veld test` offline.** `called once` holds where `called` holds (10.9). An example that expects `rejected` only because of a constraint is reported as "live only" instead of failing: say in your report what the offline run printed before and prints now, and keep the exit code rules as they are for everything else.
6. **`veld test --live` for event examples** (10.9): the probe's `subscribes` list, `POST /__veld/test/event` with its headers, the comparison, `called once` with two deliveries. No generator has the endpoint yet, so test your side against a small fake test handler written in the test (the way the existing live tests fake a server, if they do; otherwise a minimal HTTP listener in the test).
7. **Tools.** `veld fmt` (the annotations print before `channel`, one per line, in the order `@topic`, `@retry`, `@deadLetter`, `@ordered`; round trip and idempotence). `veld diff`: decide each change from what it does to a consumer generated from the old contract and one generated from the new (adding or removing `@retry`, changing `max` or `backoff`, adding or removing `@deadLetter`, adding or removing `@ordered`), write the table into `spec/events.md` section 16 in a commit of its own, and test it. The OpenAPI export is not concerned. The lint that counts used channels is not concerned.
8. **Do not touch `crates/veld-lsp`**: another agent owns it. Say in your report which completions and hovers the three annotations and `once` still need.
9. **Conformance cases** for every new error and warning and one `ok_` case; the regenerated editor grammars, `spec/diagnostics.md` and `spec/ir.d.ts` (`cargo xtask codegen` writes files and runs no tests: allowed).

## Other agents in the same crates

One agent is adding derived fields in `veld-compiler`, `veld-ir`, `veld-rules`, `veld-tools` and `veld-testing`; one is adding coverage to `veld-testing` and `veld-rules`. You will collide with them in a few shared lists: `Feature::OPT_IN` and `Feature::ALL` in `veld-ir`, the annotation table and the diagnostics table of the compiler, the regenerated files. That is expected and the main session resolves it. Make it easy: put your work in new files and new modules wherever you can, add to shared lists at the place that keeps related entries together (events with events), and do not reformat or reorder anything you do not need to change.

## Not part of this task

Any generator code, the adapters, the `messaging` option (it is an emitter option). Phase E3 (`emits`, the outbox, AsyncAPI, the event table of `veld diff` for fields). The language server.

## Your report

As `common.md` section 4 says, plus a section "for the generator agents": the IR keys, the reference functions and goldens they must reproduce, the exact JSON the test handler must answer on `POST /__veld/test/event` and in the probe, and anything in `spec/events.md` section 10 that you found unclear or wrong (do not change sections 10.1 to 10.9 yourself, except to fix a plain contradiction, in a commit of its own that says so).
