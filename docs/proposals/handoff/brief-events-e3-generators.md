# Brief: events phase E3 in the generators (the outbox)

Three agents get this brief, one per target (Node, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

`flow pending -> paid emits BookingPaid` says an event is part of a move. Outside the generators this is done (`emits`, `E1011`, the reference outbox and relay, `expect emitted`). Your server refuses an endpoint whose transition emits (`E0801`, feature `events_outbox`). Build the outbox in your target's server. A client generates nothing for it.

## Read first

- `spec/events.md` section 11, all of it (11.1 the generated pieces, 11.2 what the compiler work settled), sections 7 (publishing: steps 1 to 5 are what `emit` runs) and 10.6 (the channel's options, which the relay passes to the transport), and 12 (examples).
- The IR: `Decl(enum).flows[].emits`, `EndpointTransition.emits: [{ event, channel }]`, `Example.emitted`. **The events of the edge actually taken are derived, not stored**: the transition's `emits` is the union over its edges; for the check after the handler take the events of the edge from the state the field had to `transition.to`. The reference function is `veld_rules::outbox::expected_events(ir, transition, from)`; if your generator needs the same table at generation time, compute it from the enum's flows the same way and say where.
- The reference (`veld_rules::outbox`): `save_of` (the entry of an emitted event: topic, key, bytes), `relay` with its script, `MemoryOutbox`, `check_written` (`Missing`, `Twice`, `Unexpected`). Goldens: `tests/wire/events/outbox.json` (`entries`, `relay`, `written`) with the README. The contract with examples: `tests/events/outbox.veld.pending`.
- `crates/veld-emit-conformance/src/events_outbox.rs`.
- **The nearest things your emitter already does:** the generated publisher of E1 (steps 1 to 5 of publishing end in `transport.publish`; `emit` ends in `outbox.save` instead and must share that code, not copy it); the typed value a handler gets for a `transition` (the handler gets `emit` next to it); the idempotency store (a required interface in the hooks with a memory implementation).

## What to build

1. **`Outbox`** (`save(topic, key | none, bytes)`, `pending(limit) -> [entry]`, `done(entry)`; an entry is `{ id, topic, key | none, bytes }` with an opaque `id` of the application's) and **`MemoryOutbox`** for tests. The hooks of a server with an emitting endpoint require `outbox`; a server without it fails when it is created.
2. **`emit`**: the handler of an endpoint whose transition has `emits` receives `emit`, next to the facts and the transition (after them, where a handler takes separate arguments): one typed function per event of `transition.emits` (`emit.bookingPaid(event)`). It runs steps 1 to 5 of publishing with the hooks' id source and clock, then `outbox.save`. Nothing goes to a transport. A violation raises the typed error, as in a publisher.
3. **The check after the handler** (on a success only): every event of the edge taken was written exactly once and nothing else was; otherwise `500 Internal`, and the error goes to `onError`. A handler that answers an error need not have written.
4. **The relay**: `run<Service>OutboxRelay(outbox, transport, options)` takes up to `limit` (default 100) pending entries, publishes each with the options of its channel (10.6), in the order `pending` gave them, calls `done` after the transport accepted it, stops at the first entry that fails and answers how many it sent; the failure goes to the `onError` of its options. Hold it to the `relay` cases of the goldens.
5. **The test handler** gives every request a memory outbox and answers with the header `Veld-Test-Emitted` exactly as 11.2 defines it (declaration names, comma-separated, no spaces, in write order; absent when nothing was written; with `Veld-Test-Result` the handler is not called, the header lists the events the edge taken emits, and the exactly-once check is skipped).
6. The capability `events_outbox` (server role) and the conformance check.

## Tests that must exist when you hand back (written and compiled, not run)

- The `entries` of `outbox.json` through the generated `emit` with a recording outbox, the id and the clock fixed: topic, key and bytes equal.
- The `relay` and `written` cases through the generated relay and the generated check.
- Over the real entry point: a handler that writes its event (the entry is in the outbox, nothing was published); one that forgets (`500`, `onError`); one that writes twice; one that writes an event of another edge; a handler that answers a declared error and writes nothing; an invalid event (`emit` raises the typed error); a server created without an outbox.
- An end-to-end run in one process with the memory outbox and the memory transport: the relay publishes, a subscriber's handler gets the event; the relay run twice does not publish twice; an entry whose publish fails stays pending and stops the run.
- The examples of `tests/events/outbox.veld.pending` through your generated test handler with `veld_testing`'s live run (`expect emitted` against the header).
- Snapshots of the generated files of the outbox contract; the conformance check.

## Not part of this task

A crash-injection harness against a real database or broker. The saga orchestrator. Clients.

## Your report

As `brief-in-the-generators.md` says, plus the generated names in your target and where `emit` shares code with the publisher.
