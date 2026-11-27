# Brief: events phase E2 in the generators

Three agents get this brief, one per target (Node, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

Phase E1 gave your server a typed publisher, a typed subscriber, the envelope, the `Transport` interface and a memory transport. Phase E2 is implemented outside the generators (`@retry`, `@deadLetter`, `@ordered`, `called once`, the reference retry and dead-letter copy, live event examples). Your server refuses a channel with one of the three annotations (`E0801`, feature `events_delivery`). Build E2 in your target.

## Read first

- `spec/events.md` section **10, all of it** (10.1 to 10.9 are the design, 10.10 what the compiler side chose), and sections 7 to 9 and 12 again.
- Your target's events section (`spec/targets/typescript.md` 5f, `java.md` 12, `rust.md` 13) and the code it describes in your emitter: the runtime events file, the generator of the publisher and the subscriber, the memory transport, and the tests and test programs of E1.
- The reference: `crates/veld-rules/src/envelope/` (`dead_letter_copy`, `Policy`, `handled`, `rejected`, `Settlement`, `MemoryInbox`, `encode_unchecked`), the goldens `tests/wire/events/dead-letter.json` and `retry.json` with the README, the event examples under `tests/events/` (`delivery.veld` is new), and `crates/veld-testing` (the live run of event examples: what it sends to the test handler and what it expects back; its tests use a fake test handler that shows the exact JSON).
- `crates/veld-emit-conformance/src/events_delivery.rs`: `check_events_delivery_is_refused` and its contracts.

## What to build

1. **The transport's options** (10.6): `publish` and `subscribe` take the channel's options last and optionally (`ordered`, `deadLetter`); the generated publisher and consumer pass them; the memory transport ignores them.
2. **Retry in the generated consumer** (10.4): the runs, the waits through the `sleep` option, `retryDelayMs` and `retryMaxDelayMs`, `meta.attempt`, `onHandlerError` with `attempt` and `final`, and what happens when the last run failed, with and without `@deadLetter`. A channel without `@retry` behaves exactly as in E1. The generated code must reproduce every case of `retry.json`.
3. **The dead-letter copy** (10.5): for a rejected message and for used-up retries, published before the acknowledgement, `again` when the publish fails. The bytes must equal `dead-letter.json` case by case.
4. **The inbox** (10.3): `Inbox`, `MemoryInbox`, the consumer option, step 5 of consuming; a skipped message is acknowledged and reaches no hook; with `@retry` every run goes through the inbox again.
5. **The test handler's event endpoint** (10.9): `POST /__veld/test/event` and the `subscribes` member of the probe, exactly the JSON the live runner expects, for a server built with `testing = true` whose services subscribe. The application's handlers are not called.
6. **The two broker adapters** (10.7), generated only with the target option `messaging` (`"kafka"`, `"rabbitmq"` or a list; an unknown value and the option on a target without events are `E0802`): `KafkaTransport` and `RabbitMqTransport`, each one generated file, the only one that imports the broker's library, built from a client the application configured; the semantics of the matrix in 10.1, row by row; `redelivery`; `close()`; what each declares on RabbitMQ and that nothing is created on Kafka; the session kept alive during a long delivery. The generated manifest of your target names the library only when the adapter is chosen. **This is the one allowed new runtime dependency**, and only in those files.
7. **The capability** `events_delivery` and the conformance check, as `brief-in-the-generators.md` says.

## Gates, as they apply here

- A contract **without events** generates the bytes it generated before.
- A contract with events and without the three annotations may change in its **events runtime file** (the transport's signature, the inbox option, the consumer core) and nowhere else. The snapshots of event files were all created on this branch by E1; list every one that changes and why. Gate the retry and dead-letter code the way your emitter gates optional runtime code if it can be done cleanly; say what you did.
- Without `messaging`, no adapter file, no dependency, no manifest change.

## Tests that must exist when you hand back (written and compiled, not run)

- `dead-letter.json` and `retry.json` through your generated consumer over a recording transport, with the id, the clock and `sleep` fixed.
- Over the memory transport, two services in one process: retries with waits recorded by the fake `sleep`; the dead-letter topic receives the copy and a subscriber of `<topic>.dlq` can read it; a copy that cannot be published leaves the message unacknowledged; the inbox skips a second delivery; `called once`.
- Every event example of `tests/events/` (including `delivery.veld`) through your generated test handler with `veld_testing`'s live run, as your target does for call examples. No expected outcome edited.
- **Broker tests** against real brokers, as toolchain tests that run only when `VELD_KAFKA` (bootstrap servers) and `VELD_RABBITMQ` (an AMQP URL) are set, and fail instead of skipping with `VELD_REQUIRE_BROKERS=1`: one test program per broker that covers the rows of 10.1 for your target (publish and consume, the key, order per key, a failed handler delivered again, `@ordered`, the dead-letter topic, a message published before the group subscribed, two consumers of one group, `close()`). For Kafka the test creates its topics with the library's admin client and uses fresh topic and group names per run. Write a `tests/toolchain/brokers/README.md` section for your target (how to start the two containers: `apache/kafka-native:3.9.0` and `rabbitmq:4-management-alpine`, the ports, the two variables); if the file exists, add to it. You cannot run these: compile them, and read your adapter against the library's documentation with care, because nothing else will have checked it before the final run.
- Snapshots of the adapter files and of the event files of a contract with all three annotations.

## Not part of this task

The outbox, `emits`, the relay (phase E3). The other targets. The cross-target run (a Node producer and a Java consumer over a broker): the main session wires it when two targets are in.

## Your report

As `brief-in-the-generators.md` says, plus: the adapter's constructor and options in your target, the exact library versions you wrote against, every row of 10.1 you could not make true as written (with what you did instead), and the list of changed E1 snapshots.
