# Veld events — v1

Status: **DRAFT. Phase E1 is implemented in the IR, the compiler, the reference evaluator, the tools and the three server generators. Phase E2 is implemented outside the generators (the annotations of §10.2, `called once`, the IR and the feature, the reference of §10.3 to §10.5, `veld test` offline and live, `fmt` and `diff`); a generator that has not built it refuses the three annotations with `E0801`.** It is written first so that the compiler, the reference evaluator and the three generators are built from one text. Section 18 lists what each phase builds; section 20 lists what the owner decided and what is still open.

This file extends `spec/language.md`, `spec/ir.md` and `spec/rules.md`. Where it is silent, those files apply.

## 1. What events are, and what Veld promises

A service can **publish** events to a channel and **subscribe** to a channel. Events are declared like models, channels like services, and the same signatures, invariants and breaking-change checks apply to them. The contract does not name a broker: a channel maps to a Kafka topic or a RabbitMQ exchange by the adapter the service chose.

The promises, and their limits:

- **Delivery is at least once.** A handler MAY be called more than once for the same event: a publisher can retry, a consumer can crash between its handler returning and the acknowledgement. Veld never promises exactly once. A handler MUST be idempotent, or use the inbox (§10.3).
- **An invalid event never leaves a generated publisher**, and never reaches a generated handler. This is a promise about the generated boundary. Anything else that writes to the topic is outside it.
- **Order** is promised per key only, and only as far as the transport's row in the matrix of §10.1 says.
- **Veld does not touch a database.** The outbox and the inbox are interfaces the application implements (§10.3, §11). Veld generates no tables, no SQL and no migrations.

**Hard gate.** A contract without events generates exactly the bytes it generates today, in every target.

## 2. Words and grammar

Every new word is **contextual** (`spec/rules.md` §2.2): a keyword only where the grammar expects it, an ordinary identifier everywhere else. The reserved list does not grow.

```
event channel carries key publishes subscribes on called rejected
```

`emits`, `given` and `expect` are already contextual words of `spec/rules.md`.

```ebnf
decl          = { doc } { annotation } ( … | event | channel ) ;
event         = "event" IDENT "{" { field | rule } "}" ;
channel       = "channel" IDENT "{" { carries | key } "}" ;
carries       = "carries" typeref { "," typeref } ;
key           = "key" IDENT ;

service       = "service" IDENT [ "at" STRING ] "{" { endpoint | publishes | subscribes } "}" ;
publishes     = "publishes" typeref { "," typeref } ;
subscribes    = "subscribes" typeref "as" STRING "{" { on } "}" ;
on            = { doc } "on" typeref "->" IDENT ;

flow          = "flow" IDENT "->" IDENT { "->" IDENT } [ "emits" typeref ] ;          (* §11 *)

example       = "example" STRING "{" ( call_example | event_example ) "}" ;           (* §12 *)
event_example = "given" "event" typeref record [ "on" typeref ] expectation { expectation } ;
expectation   = "expect" ( typeref "." IDENT [ "not" ] "called" | "rejected" ) ;
```

Disambiguation needs one token of lookahead. At declaration level `event` and `channel` start a declaration when an identifier follows. In a service body an endpoint starts with a method word followed by a string; `publishes` and `subscribes` are followed by an identifier. The same lookahead ends a response: after a response that is only a status (`-> 204`), `publishes` or `subscribes` followed by an identifier starts a new line of the service and is not the type of the response. `rule` in an event body is the `rule` of `spec/rules.md` §4.

## 3. Events

```veld
/// Published when a booking is paid.
event BookingPaid {
  bookingId: uuid
  amount: decimal
  paidAt: datetime
  rule positive: amount > 0
}
```

- An event is a record with event identity. Its fields follow the rules of model fields (`language.md` §5): any type a model field may have, optional fields, defaults, constraints, `@wire`, `@example`, `@deprecated`. `@readonly` is `E0602` on an event field. An event has no `extends`.
- Its `rule` lines are invariants, exactly as on a model (`spec/rules.md` §4 and §5): the same expression language, the same rule ids (`veld:<package>/<Event>.<rule>@<hex16>`), the same check functions in generated code.
- An event's name is a declaration name: unique in its package (`E0201`), imported with `use` like any other.
- An event MAY be named only in `carries`, in `on`, in `emits` and in `given event`. Using it as the type of a field, a parameter, a response, a fact or a union arm is `E1001`. An event's fields MAY use models, enums and unions, never another event (`E1001`).
- The recursion rule of `language.md` §5.4 applies (`E0407`).

## 4. Channels

```veld
@topic("shop.bookings.v1")
channel BookingEvents {
  carries BookingPaid, BookingCancelled
  key bookingId
}
```

- A channel is one logical stream of events. It is a top-level declaration, named and imported like any other.
- **`carries`** lists the events of the channel. There MUST be at least one, each MUST be an event, and none may appear twice (`E1002`). An event MAY be carried by several channels.
- **`@topic("name")`** is the name producer and consumer agree on, so it lives in the contract. Without it the topic is `<package>.<Channel>`; for a registry package the leading `@` is dropped and `/` becomes `.` (`@acme/billing` gives `acme.billing.<Channel>`). A topic MUST match `[A-Za-z0-9._-]{1,200}`, MUST NOT be `.` or `..`, and MUST NOT end in `.dlq`, which is kept for dead letters (`E1003`). Two channels of one workspace MUST NOT have the same topic (`E1004`).
- **`key f`** names the field that orders the stream. Every carried event MUST have a required field named `f`, with the same type in all of them, and that type MUST be `string`, `uuid`, `int` or `long` (`E1005`). The **key text** of an event is that field's value: a `string` as it is, a `uuid` in lowercase, an `int` or `long` in decimal digits. An empty `string` key is a violation at publish time (§7). A channel has at most one `key` (`E1005`).
- A channel without `key` makes no promise about order.
- `@retry`, `@deadLetter` and `@ordered` on a channel belong to §10.2.

## 5. Services

```veld
service Bookings at "/bookings" {
  publishes BookingEvents
  // endpoints …
}

service Notifications {
  subscribes BookingEvents as "notifications" {
    on BookingPaid      -> sendReceipt
    on BookingCancelled -> sendApology
  }
}
```

- **`publishes C, D`** says the service publishes to those channels. Each name MUST be a channel, and none may appear twice in one service (`E1006`).
- **`subscribes C as "group" { … }`** says the service consumes the channel `C` under the consumer group `group`. All running copies of the service share that group, so each event is handled by one of them. The group MUST match `[A-Za-z0-9._-]{1,200}` (`E1007`). A service subscribes to a channel at most once (`E1006`).
- **`on E -> handler`** binds an event of the channel to a handler the application implements. `E` MUST be carried by the channel; an event has at most one `on` line in a subscription; a subscription has at least one `on` line (`E1008`). A subscription need not handle every event of its channel: the others are ignored (§8).
- Handler names share the namespace of the service's endpoint names: they MUST be unique within the service (`E0509`).
- A name that does not exist is `E0205`, as everywhere. The codes above are for a name that exists and is the wrong kind of thing.
- A service MAY publish and subscribe to the same channel. The graph of `publishes` and `subscribes` MAY contain cycles, like `consumes`.
- **A service without `at`.** `at "/prefix"` is required only when the service has endpoints (`E0501` otherwise). A service that only publishes or subscribes has no HTTP surface: no paths in the OpenAPI export, nothing for the `auth-required` lint to report, and the `empty-service` lint counts a `publishes` or `subscribes` line as content.

### 5.1 What `veld check` says about the graph

`veld check` builds one graph of HTTP calls (`consumes`) and events.

- `E1009`: a service subscribes to a channel that is declared in a **service package of the workspace** and that no service of the workspace publishes. A channel from a library or a registry package is exempt, because its publisher may live in another repository.
- `W1001`: a channel of a workspace service package is published and nobody in the workspace subscribes to it.
- `W1002`: an event of a published channel has no `on` line in any subscription of the workspace. It is reported only when the channel has at least one subscription; a channel with none is `W1001` and nothing more.

The three checks run after every other check, and only when none of those reported an error: a service that another error removed would make a channel look unpublished.

## 6. Identity and the envelope

### 6.1 Signatures

- An event has a declaration signature, computed like every other (`spec/ir.md` §5.1), with the local shape `{ "k": "event", "f": [ … ] }`. An event and a model with the same fields therefore have different signatures.
- A rule is not part of the signature, as on a model (`spec/rules.md` §6): adding or changing a rule of an event changes its rules digest and leaves its signature alone.
- The **rules digest of an event** is `hex16( sha256( RFC 8785 JSON array of the sorted, distinct rule ids of every invariant the event reaches ) )`, the formula of `spec/rules.md` §9.7 applied to one event. An event that reaches no invariant has none.
- An event **reaches** its own rules and those of every declaration its fields refer to, through optional types, lists, maps and the arms of a union, and through the parents of a model, to any depth.
- The rules digest of a **service** (`spec/rules.md` §9.7) does not change: it does not include the rules of events, because a client of the service never checks an event.
- Both are computed only in `veld-compiler::signature` and embedded verbatim by generators.

### 6.2 The envelope

A message is one CloudEvents 1.0 event in the JSON format, structured mode:

```json
{ "specversion": "1.0",
  "id": "0b5c9c3e-6a57-4d0e-9a55-1f0f6c2f7a11",
  "source": "veld:bookings/Bookings",
  "type": "veld:bookings/BookingPaid@7ab31c90d2e4f611",
  "subject": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21",
  "time": "2026-06-10T12:00:00Z",
  "datacontenttype": "application/json",
  "veldrules": "3fa9c2e1b7d04a55",
  "data": { "amount": "120.00", "bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21", "paidAt": "2026-06-10T11:59:58Z" } }
```

| Attribute | Value |
|---|---|
| `specversion` | `"1.0"` |
| `id` | A non-empty string, unique among the events of one `source`. It comes from the publisher's id source (§7). It is not a secret. |
| `source` | `"veld:" package "/" Service` of the publishing service |
| `type` | The event's declaration signature |
| `subject` | The key text (§4). Omitted when the channel has no key. |
| `time` | The publisher's clock at `publish`, as a canonical `datetime` (`language.md` §5.1), at millisecond precision |
| `datacontenttype` | `"application/json"` |
| `veldrules` | The rules digest of the event as the publisher knows it (§6.1). Omitted when it has none. A CloudEvents extension attribute. |
| `data` | The event in wire form, stored as a JSON value: defaults applied, absent optional fields omitted, scalars in their canonical formats (`spec/ir.md` §5.3) |

- **The bytes of a message are the RFC 8785 canonical JSON of that object, in UTF-8.** Given the same event, `id`, `time` and service, every target produces the same bytes. `tests/wire/events` holds the goldens.
- The media type is `application/cloudevents+json; charset=UTF-8`.
- `type` does not start with a reverse-DNS name, which CloudEvents recommends and does not require. It is the signature, so that identity survives every hop.
- A consumer ignores attributes it does not know. A message with `data_base64` is malformed for Veld (§8).

### 6.3 Where the reference chose

`veld_rules::envelope` is the reference, and `tests/wire/events` holds its goldens. Where sections 6 and 8 are silent it does this, and every generator does the same:

- **`time`** is written like every `datetime`: the fraction `.fff` only when the milliseconds are not zero (`2026-06-10T12:00:00Z`, `2026-06-10T12:00:02.007Z`). "Millisecond precision" means no finer than that.
- **Step 1 is complete before step 2.** A message whose `data` is not an object is `malformed` even when its `type` names no event of the subscription. The same holds for an attribute this section names (`subject`, `time`, `datacontenttype`, `veldrules`) that is present and not a string, and for a `datacontenttype` other than `application/json`. A missing `datacontenttype` is fine.
- **The key text of a `long`** is its wire form, the decimal digits as a string. An `int` key is its decimal digits, also when negative.
- **Canonical scalars.** `data` is written in the canonical formats of `spec/ir.md` §5.3: a `uuid` in lowercase and a `datetime` in UTC with `Z`, whatever form the caller gave (`2026-06-10T13:59:58.250+02:00` is written `2026-06-10T11:59:58.250Z`). The other scalars are written as given: a decimal and a long are strings, a `bytes` value is padded base64, which the caller already holds. The key text is taken from the canonical value.
- **A violation of an audited rule** (`spec/rules.md` §17.1) never stops a publish or rejects a message, and an event whose invariants are all audited has no `veldrules` (§6.1).
- **Unknown fields** of `data` are left in the decoded value; a typed consumer ignores them.
- **A limit of the reference, not a rule for generators.** As for the examples of §12, the reference checks that required fields are there and that objects, lists and maps have the right shape, and it applies defaults and invariants; it does not check the text of a scalar. A `string` where a `uuid` is expected is not a shape failure in the reference, and is one in a generated consumer (§8, step 3). No golden depends on the difference.

### 6.4 What the first generator settled

The Node generator is the first target; Java and Rust implement the same, in their own naming (`spec/targets/typescript.md` section 5f is the reference list).

- **Canonical scalars are the generator's own.** The `time` and every `datetime` of `data` are written by one function: UTC, the fraction `.fff` only when the milliseconds are not zero. A generator must not reuse a helper that trims zeros (`.250` stays `.250`) or that always writes a fraction.
- **An empty `string` key** (§7) is one issue with the code `key` and the path `$.<wire name of the key field>`. It has no entry in `violations`: it is not a rule.
- **`onRuleViolation` for an event** (the hook of `spec/rules.md` §7 with the same event shape): `operation` is the signature of the event, `position` is `output` in a publisher and `input` in a consumer, `outcome` is `rejected` for an enforced rule and `audited` for an audited one. It is called once per violation, enforced ones first, and an exception in the hook never changes what happens. A publisher calls it before it throws the `ValidationError`.
- **Order of the consumer's hooks.** A message that is rejected is reported to `onEventRejected` and to nothing else. `onContractMismatch` is called for a message that passed steps 1 to 4, before its handler. When the digest differs, step 4 does not run at all, audited rules included, so nothing is reported to `onRuleViolation`.
- **A handler that fails** is reported to `onHandlerError` with the event, the subscription, the metadata and the error, and then the message is not acknowledged. A failing hook never changes the outcome.
- **An ignored message** (§8, step 2) is acknowledged and nothing is called: E1 has no counter and no hook for it.
- **The shape of step 3** is checked on the wire value of `data`, so the paths of its issues use wire names (`$.invoice_id`), before the typed value is built.

## 7. Publishing

The generated publisher of a service has one operation per carried event of each channel the service publishes to. It runs, in order:

1. **Shape.** The value is validated against the event's type, as a response is.
2. **Invariants**, in full mode. A violation, an evaluation error (`spec/rules.md` §3.7, which fails closed) or an empty `string` key ends the call with the target's typed `ValidationError`, raised locally, with `violations` filled. **Nothing is sent.** There is no option to turn this off.
3. The key text, when the channel has a key.
4. The envelope: `id` from the **id source**, `time` from the **clock**.
5. The bytes (§6.2).
6. `transport.publish(topic, key, bytes)`. The call completes when the transport has accepted the message.

- The **clock** and the **id source** are options of the publisher, so a test can fix both and compare bytes. The default clock is the system clock. The default id source gives a random UUID where the target's standard library has one, and a value of the same shape and the same uniqueness otherwise; `spec/targets/*.md` say which.
- `publish` is not part of the application's database transaction. A state change that must not be lost when the publish fails uses the outbox (§11).

## 8. Consuming

For every message of a subscription the generated consumer runs, in order:

1. **Envelope.** The bytes MUST be a JSON object with `specversion` `"1.0"`, non-empty strings `id`, `source` and `type`, and an object `data`. Otherwise the message is **rejected** as `malformed`.
2. **Which event.** `type` is split at its last `@` into a name and a hash. The name is looked up among the events of the channel that this subscription has an `on` line for. A name that is not among them is **ignored**: the message is acknowledged and counted, and nothing else happens. This is what makes adding an event to a channel safe for consumers that were generated before it.
3. **Decode**, with the consumer's own type for that event. Unknown fields are ignored, defaults are applied, then the shape is validated (types and constraints). A failure rejects the message as `shape`.
4. **Invariants**, in full mode, **only when the message's `veldrules` equals the consumer's own digest for that event** (both absent counts as equal). A violation or an evaluation error rejects the message as `rule`.
5. **Inbox** (§10.3), when the application supplied one: an event whose `source` and `id` were already handled is acknowledged and skipped.
6. **The handler**, with the typed event and its metadata (`id`, `source`, `type`, `subject`, `time`, and the delivery attempt when the transport knows it).
7. **Acknowledge**, only after the handler has returned normally.

- **The key is not checked.** A consumer does not compare `subject` with the event's key field, and does not reject an empty key: the key is the publisher's promise (§7).
- **A handler that fails** (throws, or returns an error) leaves the message unacknowledged. The transport delivers it again by its own policy (§9, §10.2).
- **A rejected message can never succeed for this consumer.** It is acknowledged, so that it does not block the stream, and reported to the hook `onEventRejected` with the reason (`malformed`, `shape` or `rule`), the violations and the raw bytes. With `@deadLetter` (§10.2) it is also copied to the dead-letter topic. Without a dead letter and without a hook that stores it, a rejected message is gone: the documentation says so.
- **Contract skew.** A consumer and a publisher are often generated from different versions of a contract. The consumer does not require `type` to equal its own signature: it decodes with its own type (step 3), so a message from a publisher that added an optional field is handled, and one that lacks a field the consumer requires is rejected as `shape`. When the hash of `type` or the value of `veldrules` differs from the consumer's own, the consumer calls `onContractMismatch` once per distinct pair it had not seen, with the event's name, its own signature and digest and the message's. A differing digest also switches off step 4 for that message, as a stale client stops its local checks (`spec/rules.md` §9.7): the publisher has already checked the event against the rules it was generated with.

### 8.1 The hooks

Every target names them in its own style and gives them the same content. All of them are optional, and a failing hook is ignored.

| Hook | Called | Content |
|---|---|---|
| `onEventRejected` | once per rejected message (§8), after its acknowledgement is decided | the reason (`malformed`, `shape`, `rule`), a text, the topic and the group, the name of the event or none, the issues, the violations, the raw bytes |
| `onContractMismatch` | once per distinct pair of `type` and `veldrules` of an event that this consumer had not seen | the event's name, its own signature and digest, the message's signature and digest (a digest may be absent) |
| `onRuleViolation` | once per violation, in a publisher and in a consumer (§6.4) | the rule id, the operation (the event's signature), the position, the outcome, the violation, the elapsed microseconds |
| `onHandlerError` | when a handler fails | the event's name, the topic and the group, the metadata, the error |

The metadata a handler gets next to the typed event is `id`, `source`, `type` (as the publisher wrote it), `subject` and `time` (absent when the message has none), and `attempt`.

## 9. The transport

The generated publisher and consumer depend only on a small interface, named by each target's conventions:

```
publish(topic, key | none, bytes)            completes when the message is accepted
subscribe(topic, group, deliver) -> handle   deliver(message) answers "done" or "again"; handle.close() stops it
message = { key | none, bytes, attempt }     attempt starts at 1
```

`deliver` answers "again" only when the handler failed (§8). Everything else, a rejection included, is "done".

**The memory transport** is generated with every target that has events. It has no dependency and lives in the process:

- A message is delivered to **one** subscriber of **each** group that was subscribed to the topic when it was published. A topic nobody subscribed to drops its messages: there is no retention and no replay.
- Messages of a topic are delivered to a group in publish order, one at a time.
- "again" delivers the message again at once, with `attempt` one higher, up to `maxAttempts` (default 3). The next delivery goes to the next subscriber of the group in rotation, which may be the same one, and nothing else of the group is delivered meanwhile. After the last attempt the message goes to the transport's dead-letter list, which a test can read: the topic, the group, the key, the bytes and the number of attempts.
- The subscribers of a group take the messages in turn (round robin), so two copies of a service share its load. A subscriber that was closed gets nothing more; a group without subscribers is removed, and what was queued for it is dropped.
- `publish` completes when the message is queued; handlers run afterwards, never inside the call. The bytes are copied.
- `idle()` completes when every published message has been delivered or dead-lettered, so a test can wait without sleeping. It also waits for the messages that handlers publish.

It is for tests and for development in one process. It is what `veld test` uses (§12).

## 10. Brokers (phase E2)

Phase E2 adds three annotations of a channel, the inbox, two broker adapters and live event examples. Sections 10.1 to 10.3 say what they mean; 10.4 to 10.9 fix the details the generators share. The names are in TypeScript spelling, as in `spec/targets/typescript.md`; Java and Rust use the same words in their casing.

### 10.1 The capability matrix

A broker adapter is a framework adapter: the one allowed dependency of generated code (`CLAUDE.md` rule 3), using the standard client of the language. Brokers do not share one semantics, so the portable contract is deliberately small, and a contract that asks for more than the chosen broker gives is refused with `E0801` at the line that asks.

The memory column is normative now. **The Kafka and RabbitMQ columns state the design; they are corrected to what the tests against the real brokers show.**

| | memory | Kafka | RabbitMQ |
|---|---|---|---|
| A topic is | a name in the process | a topic, which the adapter does not create | a durable `fanout` exchange, declared by the adapter |
| A group is | the subscribers that share the name | a consumer group | the consumers of the durable queue `<topic>.<group>`, declared and bound by the adapter |
| The key becomes | a field of the message | the record key | the routing key (the empty string when there is none) |
| Order per key | publish order, per topic | within a partition, so per key | only while the queue has one active consumer |
| `publish` completes | when the message is queued | when all in-sync replicas have it (`acks=all`, idempotent producer) | when the broker confirmed it (publisher confirms, persistent message) |
| A message published before a group first subscribed | is dropped | is read: a new group starts at the earliest offset | is dropped: no queue was bound |
| `again` (§9) | delivered again at once, `maxAttempts` | not committed; the same consumer delivers it again after a delay, and takes nothing else of that partition meanwhile | not acknowledged; the same consumer delivers it again after a delay, and takes nothing else meanwhile (prefetch 1) |
| `@ordered` (§10.2) | already so | every message goes to partition 0 | the queue is declared with `x-single-active-consumer` |
| Dead letter (§10.5) | the topic `<topic>.dlq`, like any topic | the topic `<topic>.dlq`, which the adapter does not create | the exchange `<topic>.dlq` and a durable queue of the same name, declared by the adapter |
| Replay of old messages | no | with the broker's own tools | no |

The CloudEvents bindings cover Kafka and AMQP 1.0. On Kafka a message is the envelope bytes as the record value, with the header `content-type` set to the media type of §6.2, and the key text as the record key. RabbitMQ's usual protocol is AMQP 0-9-1, which no CloudEvents binding covers; Veld uses the same structured mode there: the envelope bytes as the body and the same media type as the content type.

### 10.2 Retry, dead letter, order

- `@retry(max: n, backoff: fixed | exponential)` on a channel bounds how often a message whose handler fails is handled again by a consumer of that channel. `max` is 0 to 100 and required; `backoff` defaults to `exponential`.
- `@deadLetter` on a channel gives it a dead-letter topic, `<topic>.dlq`. A rejected message (§8), and a message whose retries are used up, is copied there (§10.5). Reading and replaying a dead-letter topic is the broker's job, with the broker's tools.
- `@ordered` on a channel asks for one total order across keys. A broker that can only give it by giving up parallel consumers does so; a generator whose adapter cannot give it reports `E0801`.
- Each stands before `channel`, once, and takes no other argument (`E0602`). `<topic>.dlq` counts as a topic of the workspace for `E1004`.
- `W1003`: a channel has `@retry` and no `@deadLetter`. A message whose retries are used up is then acknowledged and gone, reported only to `onHandlerError`.

### 10.3 The inbox

Delivery is at least once on every broker. The generated consumer accepts an optional **inbox**, an interface the application implements over its own store:

```
handle(source, id, run) -> runs `run` and records (source, id), or does nothing when the pair is already recorded
```

An implementation that records the pair in the same transaction as the handler's own writes prevents a duplicated business effect. One that cannot share a transaction narrows the window and does not close it. The guides say this in these words. Veld generates the interface and a memory implementation for tests, nothing else.

- The names are `Inbox`, `MemoryInbox` and the consumer option `inbox`. `handle` answers whether it ran `run`. When `run` fails the pair MUST NOT be recorded, and the failure is the handler's failure.
- An inbox belongs to one service: a service subscribes to a channel once (`E1006`), so `source` and `id` name one run of one handler.
- A message the inbox skipped is acknowledged like a handled one. It is not an error and reaches no hook.

### 10.4 Retry in the generated consumer

`@retry` is carried out by the generated consumer, not by the transport, so it behaves the same on every transport, the memory transport included.

- When the handler of a message fails on a channel with `@retry(max: n)`, the consumer waits and runs steps 5 and 6 of §8 again, up to `n` more times: `n + 1` runs in all. `meta.attempt` is the transport's `attempt` plus the runs already made.
- The wait before run `k + 1` is `retryDelayMs` for `fixed` and `retryDelayMs * 2^(k-1)` for `exponential`, never more than `retryMaxDelayMs`. Both are options of the consumer (1000 and 60000 by default). The consumer waits through its `sleep` option, a function from milliseconds to a completion, so a test passes one that does not wait.
- Every failed run is reported to `onHandlerError`, whose content gains `attempt` and `final`: `final` is true when the message will not be handled again by this consumer.
- When the last run failed: with `@deadLetter` the message is copied to the dead-letter topic with the reason `handler`, and the consumer answers `done`. Without it the consumer answers `done` and the message is gone (`W1003`).
- On a channel without `@retry` nothing changes from §8: the consumer answers `again`, and the transport delivers the message again by its own policy (§10.1), without bound on a broker.

### 10.5 The dead-letter copy

- The copy is published with `transport.publish("<topic>.dlq", key, bytes)`, with the key of the original message.
- When the original bytes are a JSON object, the copy is that object with these members added, written as canonical JSON (§6.2): `veldreason` (`malformed`, `shape`, `rule` or `handler`), `veldgroup` (the group of the consumer), `veldattempts` (the number of runs, an integer, 0 for a rejected message), and, when not empty, `veldissues` and `veldviolations`: each a string that holds the canonical JSON of the list. Every other member is kept, so a copy published again to the topic is a valid message: a consumer ignores envelope members it does not know.
- When the original bytes are not a JSON object, they are copied as they are. Nothing can be added to what is not an envelope; the reason is in `onEventRejected`.
- Nothing of a handler's error is copied: an error text is the application's and may hold what must not be stored.
- **The copy comes before the acknowledgement.** When publishing the copy fails, the consumer answers `again`.

### 10.6 The transport gets the channel's options

`publish` and `subscribe` (§9) take one more argument, last and optional, so a transport written for E1 still fits:

```
publish(topic, key | none, bytes, options?)
subscribe(topic, group, deliver, options?) -> handle
options = { ordered: bool, deadLetter: bool }     both false when absent
```

The generated publisher and consumer pass the options of the channel. The memory transport ignores them. A broker adapter uses them as §10.1 says. A transport that cannot give `ordered` MUST fail the call instead of ignoring it.

### 10.7 The adapters

`[server.options] messaging = "kafka"`, `"rabbitmq"` or a list of both chooses the adapters a server target gets. Without the option a server gets the memory transport only, and every generated byte is what it was. An unknown value is `E0802`, and so is the option on a target none of whose services publishes or subscribes.

- An adapter is one generated file that implements `Transport` (`KafkaTransport`, `RabbitMqTransport`). It is the only generated file that imports the broker's client library, and the target's generated manifest names that library only when the adapter is chosen.

  | | Kafka | RabbitMQ |
  |---|---|---|
  | Node | `kafkajs` | `amqplib` |
  | Java (JDK server and Spring) | `org.apache.kafka:kafka-clients` | `com.rabbitmq:amqp-client` |
  | Rust | `rdkafka` | `lapin` |

- **The application configures the connection.** Addresses, TLS and credentials are not in the contract and not in `veld.toml`. The adapter is built from what the library uses for that: a `Kafka` instance (Node), a map of client properties (Java), a `ClientConfig` (Rust); an open AMQP connection for RabbitMQ in all three. The adapter adds only what the semantics of §10.1 need (the group id, manual commits, `earliest`, idempotence, confirms, prefetch) and overrides a setting of the application that contradicts them.
- Options of both adapters: `redelivery: { initialMs, maxMs }`, the delay before a message answered `again` is delivered again (1000, doubling up to 30000 by default). `attempt` counts the deliveries by this consumer since it started; after a restart or a rebalance it starts at 1 again.
- `close()` stops the consumers, waits for a delivery that is running, and flushes the producer. The application closes the client it made.
- An adapter creates nothing on a Kafka cluster. On RabbitMQ it declares what §10.1 lists, with the same arguments every time; a queue that exists with other arguments (`x-single-active-consumer` added later) is the broker's `PRECONDITION_FAILED`, which the adapter passes on.
- A handler that runs for long must not cost the consumer its place in the group: the Kafka adapter keeps the session alive while a delivery runs or waits.

### 10.8 In the IR, and the feature

```jsonc
Channel      += { "retry": { "max": 5, "backoff": "exponential" },   // each omitted when absent
                  "dead_letter": true, "ordered": true }
Example.expect[] += { "once": true }                                  // `called once`; omitted when false
```

A service that publishes or subscribes to a channel with one of the three annotations uses the feature `events_delivery`. It is opt-in (`spec/ir.md` §4): a generator that built E1 does not get it by accident, and reports `E0801` at the name of the first such channel until it declares it (the IR keeps no position for an annotation). The inbox and the adapters are options of a target and need no feature.

### 10.9 `called once`, and live event examples

- `expect S.handler called once` says: the event is delivered twice, with the same `source` and `id`, and the handler runs once. The reference evaluator gives every subscription an inbox, so offline `called once` holds where `called` holds. `once` is a contextual word.
- **The test handler** (`testing = true`) of a server whose services subscribe answers `POST /__veld/test/event`. The body is the message bytes; the headers are `Veld-Test-Topic` and, optionally, `Veld-Test-Key` and `Veld-Test-Deliveries` (1 or 2). It delivers the message that many times to the generated consumer over a fresh memory transport, with a fresh memory inbox, a `sleep` that does not wait and handlers that only count, and answers `200` with `{ "outcome": "handled" | "ignored" | "rejected", "reason": "malformed" | "shape" | "rule" | null, "called": { "<Service>.<handler>": <count> }, "violations": [ … ], "deadLetters": <count> }`. The application's handlers are not called. `GET /__veld/test` (`spec/rules.md` §12.4) also lists `"subscribes": { "<Service>": ["<topic>", …] }`.
- **`veld test --live`** builds the envelope of each event example with the reference encoder (§6.3) and sends it to every test handler that subscribes to the example's channel. It compares the outcome, the handlers that were called and the violations with the reference evaluator's. `called once` sends two deliveries and expects a count of 1; `called` expects at least 1 from one delivery; `not called` expects 0. A generated consumer checks constraints, which the reference evaluator does not (§12): an example that expects `rejected` and is not rejected offline for that reason is run live and reported as "live only" offline.
- **The broker tests** are toolchain tests against real brokers in containers (`apache/kafka-native`, `rabbitmq`), and run only when `VELD_KAFKA` and `VELD_RABBITMQ` name them; `VELD_REQUIRE_BROKERS=1` turns a skip into a failure. They cover every row of §10.1 for each target, and one cross-target run per broker: a Node producer and a Java consumer with only `messaging` changed.

### 10.10 What the reference settled

The text above left these open; the compiler, the reference and the tools chose as follows, and the generators follow.

- **The checks of §10.2.** `max` is an integer literal from 0 to 100; `backoff` is the word `fixed` or `exponential`; each argument is written once, in either order; `max` is required. `@deadLetter` and `@ordered` take no argument. Everything else is `E0602`, with the annotation as the span.
- **`<topic>.dlq` and `E1004`.** The dead-letter topic of a channel is entered in the table of topics that `E1004` checks. Since a topic never ends in `.dlq` (`E1003`), it can never equal the topic of another channel; the entry changes nothing today and keeps the table whole.
- **`W1003`** is reported at the `@retry` annotation, for a channel of a service or library package; a channel of a registry package is not the author's to fix and stays silent.
- **`events_delivery` and `E0801`.** The gate points at the name of the first channel, in the order of the service's `publishes` lines and then of its subscriptions, that has one of the three annotations: the annotations have no position of their own in the IR.
- **`called once`.** `once` is read after `called`, after `not called` and after `rejected`, so that the checker can say `E1010` for the last two. Offline, `called once` is `called` (§10.9), and the example prints as it was written.
- **A copy that fails again (§10.5).** A member of the original named `veldreason`, `veldgroup`, `veldattempts`, `veldissues` or `veldviolations` is replaced by the new one, or removed when the new copy has no list for it: a copy describes its own failure and never an earlier one. Every other member is kept; the whole object is written as canonical JSON.
- **The lists of the copy (§10.5).** `veldissues` is the canonical JSON of a list of `{ "path", "code", "message" }` (the `ValidationIssue` of the prelude); `veldviolations` is the canonical JSON of a list of the violations of `spec/rules.md` §7 (`rule`, `path`, `fields`, `message`, and `error` for an evaluation error). `veldattempts` is the number of runs of the handler.
- **The retry settlement (§10.4).** A message that step 2 ignores is acknowledged and nothing else. A rejected message is acknowledged and, with `@deadLetter`, copied once with `veldattempts` 0, whatever `@retry` says. The runs of the handler are counted from 1; `final` of `onHandlerError` is false for every failure of a channel without `@retry`, because the transport delivers the message again. `tests/wire/events/retry.json` holds the table.
- **Offline `rejected` and constraints (§10.9, §12).** The reference does not check constraints in the delivery it works out. An event example that expects `rejected`, is delivered offline, and has a value that breaks a `@min`, `@max`, `@length`, `@pattern` or `@format` is reported as `skip … (live only: …)` and does not fail the run. The reference decides the five itself: `@pattern` with the portable regex engine (`veld_rules::helpers::regex`, a match anywhere in the text) and `@format` with the checks of the Rust runtime; a `@pattern` that engine does not support decides nothing. Every other mismatch fails as before, so an example that expects `rejected` of a value that breaks nothing is still a failure.
- **The live run (§10.9).** The test handler's probe MAY list `"subscribes": { "<Service>": ["<topic>", …] }`; without it, or without a subscriber of the topic, the event examples of that channel are skipped. The message is the canonical bytes of the reference encoder (without its checks for a value a publisher would refuse), with `id` `veld-test-1`, the `source` of the first service that publishes the channel and the time `2026-01-01T00:00:00Z`; the headers are `Content-Type` (the media type of §6.2), `Veld-Test-Topic`, `Veld-Test-Key` (the `subject` of the message, when it has one) and `Veld-Test-Deliveries`. An example with a `called once` line is delivered twice, the others once. The answer's `violations` are objects with at least `rule` and `path`; they are compared as a set of those two. The comparison is: the outcome (`handled` when a subscription of the handler has an `on` line for the event, `ignored` when none has, and `rejected` for what the reference rejects; a value that breaks a constraint is rejected as `shape` even when it also breaks a rule, because step 3 of §8 comes before step 4), the reason, the violations of a `rule` rejection, the set of handlers that ran (restricted to the services the handler lists), and then every `expect` line of a service the handler lists (`called` at least one run, `called once` exactly one, `not called` none). A line about another service is not judged. `deadLetters` is read and not compared.

## 11. The outbox and `emits` (phase E3)

```veld
enum BookingStatus {
  pending, paid, cancelled
  flow pending -> paid emits BookingPaid
  flow pending -> cancelled after hours(24) emits BookingCancelled
}
```

```ebnf
flow = "flow" IDENT "->" IDENT { "->" IDENT } [ "after" expr ] [ "emits" IDENT ] ;
```

`flow pending -> paid emits BookingPaid` says the event is part of that move. `emits` is a contextual word and stands on a `flow` with one edge (`E1011` otherwise).

- **Who must be able to send it.** Every endpoint whose `transition` takes that edge belongs to a service that MUST publish a channel which carries the event (`E1011`). When several of its channels carry it, the first in the service's `publishes` order is the one.
- The generated server gives the handler of such an endpoint an **outbox** to write the event to. The handler MUST call it inside the transaction that changes the state. The outbox is an interface the application implements over its own store: `save(topic, key, bytes)` and, for the relay, `pending(limit)` and `done(entry)`.
- A generated **relay** reads pending entries and publishes them. A relay can publish an entry twice (it can crash between the publish and `done`), which is why delivery is at least once and consumers need §10.3.
- An outbox closes the gap between "the state changed" and "the event was sent" only when the application's implementation really shares the transaction. Veld cannot check that, and says so.

### 11.1 The generated pieces

The names are in TypeScript spelling; Java and Rust use the same words in their casing.

- **`Outbox`** is `save(topic, key | none, bytes)`, `pending(limit) -> [entry]` and `done(entry)`. An `entry` is `{ id, topic, key | none, bytes }`; `id` is the application's own and opaque. `MemoryOutbox` is generated for tests. The hooks object of a server with an emitting endpoint requires `outbox`.
- **The handler** of an endpoint whose transition has `emits` receives `emit`, next to the facts and the transition: one typed function per event the move emits (`emit.bookingPaid(event)`). Calling it runs steps 1 to 5 of publishing (§7: shape, invariants, key, envelope, bytes; the id source and the clock are the hooks') and then `outbox.save`. Nothing is sent to a transport. A violation raises the typed error, as in a publisher, and the handler's transaction should end with it.
- **The server checks the handler.** After a success of such an endpoint the event MUST have been written exactly once. A success without it, or with it twice, answers `500 Internal` and goes to `onError`: the state changed without its event, and nobody should find that out from a missing receipt. A handler that answers an error need not have written.
- **The relay** is `run<Service>OutboxRelay(outbox, transport, options)`: it takes up to `limit` (default 100) pending entries, publishes each with the options of its channel (§10.6), in the order `pending` gave them, and calls `done` after the transport accepted it. It stops at the first entry that fails and answers how many it sent, so the order of a key is kept; the failure goes to the `onError` of its options. The application runs it in a loop or from a job.
- An endpoint whose move emits needs no transport: a service may write to the outbox in one process and relay from another.
- **Examples.** A call example of such an endpoint with `returns` MAY say `expect emitted BookingPaid` or `expect not emitted`, which is checked live only: the reference evaluator does not run handlers. The test handler (`testing = true`) gives every request a memory outbox and adds the header `Veld-Test-Emitted`, the names of the events written, to its answer.
- **IR.** A flow gains `"emits": DeclId` and an `EndpointTransition` gains `"emits": [ { "event": DeclId, "channel": <index into Ir.channels> } ]`, each omitted when absent. A service with such an endpoint uses the feature `events_outbox`, opt-in.
- `E1011`: `emits` on a `flow` with several edges, an `emits` that names no event, or a transition over an emitting edge in a service that publishes no channel carrying that event.

## 12. Examples and `veld test`

```veld
example "a paid booking sends a receipt" {
  given event BookingPaid { amount: 120.00 }
  expect Notifications.sendReceipt called
}

example "an event that breaks its rule reaches no handler" {
  given event BookingPaid { amount: 0 }
  expect rejected
}
```

- An event example describes **one delivery of one event** on one channel. The record is filled in like a parameter (`spec/rules.md` §12.2).
- `on Channel` names the channel. It MAY be left out when exactly one channel visible in the module carries the event; otherwise it is required (`E1010`).
- `expect S.handler called` and `expect S.handler not called` name a handler of a subscription of service `S` to that channel; a name that is no such handler is `E1010`. `S` is looked up in the example's own package first, and then among the services of the workspace that subscribe to the channel; a name that fits more than one of those is `E1010`. `expect rejected` says no handler is called because the event is rejected. `rejected` MUST NOT be combined with a `called` line (`E1010`). `expect S.handler called once` is phase E2 (§10.9); `once` after `not called` or after `rejected` is `E1010`.
- An example constrains only the handlers it names. A handler it does not name may or may not be called.
- **`veld test`** runs event examples in the reference evaluator: it applies steps 3 and 4 of §8 to the filled-in event with matching digests and works out which handlers of which subscriptions are called. It does not run handlers.
- **The reference evaluator checks types and required fields in step 3, not constraints** (`@min`, `@length`, `@pattern`), as it does not for the parameters of a call (`spec/rules.md` §12.3). A written value that breaks a constraint is therefore not rejected offline, while a generated consumer rejects it as `shape`. Such an example belongs to the live run of E2.
- Running event examples against live services (`--live`), and `called once` with a second delivery of the same event, are phase E2 and are specified in §10.9.

## 13. In the IR

Events are part of `ir_version` 1.1, the version of the rules and events work. Every key below is **omitted from the JSON when empty or absent**, so the IR of a contract without events does not change.

```jsonc
Decl.kind += Event = { "kind": "event", "fields": [Field],
                       "rules": [Rule],                         // as on a model
                       "rules_digest": "3fa9c2e1b7d04a55" }     // §6.1
Ir      += { "channels": [ Channel ] }                          // sorted by qualified name
Channel  = { "name": "BookingEvents", "package": PackageId, "module": "…", "qualified": "…",
             "topic": "shop.bookings.v1",
             "events": [ DeclId ],                              // `carries` order
             "key": { "name": "bookingId", "wire": "bookingId", "type": Type } | null,
             "docs": .., "deprecated": .., "origin": Origin }
Service += { "publishes": [ { "channel": <index into Ir.channels>,
                              "origin": Origin } ],             // source order; the origin is the channel name in the line
             "subscriptions": [ { "channel": <index into Ir.channels>, "group": "notifications",
                                  "handlers": [ { "event": DeclId, "name": "sendReceipt", "docs": .., "origin": Origin } ],
                                  "origin": Origin } ] }        // source order
Example += { "event": { "decl": DeclId, "channel": <index>, "value": ExampleValue },
             "expect": [ { "service": ServiceId, "handler": "sendReceipt", "called": true } ] | "rejected" }
```

- `Service.prefix` is the empty string for a service without `at`.
- An event example has `event` and `expect` and omits `service`, `endpoint` and `outcome`, which a call example always has.
- `Feature::Events` (`events`) is used by a service that has `publishes` or `subscriptions`. A generator that cannot represent it reports `E0801`: at the channel name of the first `publishes` line when the service publishes, and otherwise at its first `subscribes` line. A service that publishes or subscribes to a channel which carries an event with an audited invariant (on the event, or on a declaration the event reaches) also uses `rules_rollout` (`spec/rules.md` §17.1); the `E0801` for it points at the first such rule in source order. A client does not count that.
- Only the services a target generates count. A server that calls a publishing service over HTTP (`consumes`) does not use the feature, and neither does its client for that service.
- An event declaration is reached from a service only through its channels. The declarations a service reaches through its endpoints (`spec/ir.md`) do not include events or what only events refer to.

## 14. Generated code

The same structure in every target, so the three implementations can be compared line by line:

- **Server targets** get, for a service with events:
  - the type of every event its channels carry, with its check function when it has rules;
  - one **publisher** per service that publishes, built from a transport and the options of §7, with one operation per carried event of each channel it publishes to, grouped by channel (a channel that two services publish to is published to by each, with its own `source`);
  - one **handler interface** per service, with one method per `on` line (of every subscription of the service), and one function that subscribes it: a transport, the handlers, and the options of §8.1 (the hooks, later the inbox). It subscribes every `subscribes` line of the service, each under its group, and returns one handle that closes them all;
  - a runtime file for the envelope, the transport interface and the memory transport, **emitted only when the service has events**.
- **Client targets** generate nothing for events, not their types either: a browser or a mobile app does not talk to a broker. A client generator supports the feature by generating nothing, so a client of a service that gains `publishes` keeps its bytes.
- A target is self-contained (`CLAUDE.md`): a service that subscribes to a channel of another package gets its own copy of the event types. Two copies are the same event because they carry the same signature (`spec/ir.md` §5).
- Names of the generated pieces follow each target's naming; the reference list is in `spec/targets/typescript.md`.

## 15. `fmt`, lint, the language server

- `veld fmt` prints `carries` and `key` one per line in that order, `publishes` before the endpoints of a service and `subscribes` after them, and one `on` line per event with the arrows aligned. The annotations of a channel print before `channel`, one per line, in the order `@topic`, `@retry`, `@deadLetter`, `@ordered`; another annotation (`@deprecated`) keeps the place it had. `expect S.handler called once` prints as written.
- The `unused-model` lint counts an event as used when a channel carries it, and a channel as used when a service publishes or subscribes to it. It reports an unused channel only in a service package: a library's channel is there for others to use.
- The language server completes event names after `carries`, `on` and `given event`, and channel names after `publishes` and `subscribes`; hover shows the topic, the key and the signature.

## 16. `veld diff` and AsyncAPI (phase E3)

A change to an event is judged for both sides of the stream: consumers generated from the old contract, and consumers generated from the new one that read old messages.

| Change | Result |
|---|---|
| An optional field, or a field with a default, is added | compatible |
| A required field without a default is added | breaking: a new consumer cannot read old messages |
| A required field is removed, or a field's type changes | breaking: an old consumer rejects new messages |
| An optional field is removed | possibly breaking |
| A rule is added, removed or changed | possibly breaking; consumers with another digest stop checking rules (§8) |
| An event is added to a channel | compatible (§8, step 2) |
| An event is removed from a channel, the topic or the key changes | breaking |

**Before phase E3** `veld diff` does not look inside an event. It reports an **addition** as compatible: a new event, a new channel, an event a channel newly carries, a new `publishes` line, a new subscription, and a new `on` line of a subscription that keeps its group and its other lines. Every other change to an event, a channel, a `publishes` line or a subscription is one entry, **possibly breaking**, which fails `veld diff --strict`: the tool does not know yet which side of the stream it hurts.

**The annotations of §10.2 (phase E2).** None of the three changes a byte on the wire: a consumer generated from the old contract and one generated from the new read the same messages. What differs is what each does with a message it cannot handle, and the objects the adapter declares on the broker. Each change is judged by what it does to a consumer generated from the old contract and to one generated from the new, and is one entry of its own:

| Change | Result | Why |
|---|---|---|
| `@retry` added, and the new channel has `@deadLetter` | compatible | the new consumer gives up after `max + 1` runs and keeps the message on the dead-letter topic; the old one is redelivered by the transport |
| `@retry` added, no `@deadLetter` | possibly breaking | a message the old consumer would have seen again is gone after its retries (`W1003`) |
| `@retry` removed | possibly breaking | the transport redelivers a failing message without bound, so one message can block the stream (§10.4) |
| `max` lowered, and the new channel has no `@deadLetter` | possibly breaking | a message is given up earlier and is gone |
| `max` raised, or lowered with `@deadLetter` | compatible | what is given up is kept |
| `backoff` changed | compatible | only the waits differ |
| `@deadLetter` added | compatible | nothing is read from `<topic>.dlq` yet; on Kafka the topic must exist before the consumer is deployed (§10.7) |
| `@deadLetter` removed | possibly breaking | rejected messages and exhausted retries are gone, and whoever reads `<topic>.dlq` loses its source |
| `@ordered` added or removed | possibly breaking | consumers may depend on the order, and the adapter declares the partition or the queue differently (`x-single-active-consumer`, §10.7) |

The new channel decides when two changes meet: `max` lowered while `@deadLetter` is added in the same change is compatible. A channel that is new, or that is removed, is judged as before, not by its annotations.

`veld export asyncapi` writes an AsyncAPI 3.0 document: channels, their messages with the event schemas, and the operations of each service.

## 17. Diagnostics

| Code | Meaning |
|---|---|
| `E1001` | An event is used as a type, or an event's field has an event type |
| `E1002` | `carries` names something that is not an event, names an event twice, or is missing |
| `E1003` | The topic of a channel is not a legal topic name |
| `E1004` | Two channels of the workspace have the same topic |
| `E1005` | The `key` of a channel is not a required field of the same key type in every carried event, or the channel has two keys |
| `E1006` | `publishes` or `subscribes` names something that is not a channel, or the service names the channel twice |
| `E1007` | The group of a subscription is not a legal group name |
| `E1008` | An `on` line names an event the channel does not carry or names an event twice, or the subscription has no `on` line |
| `E1009` | A subscription to a channel of this workspace that no service publishes |
| `E1010` | An event example that is not well formed: the channel cannot be chosen, an `expect` names no handler of that channel, `rejected` is combined with `called`, or `once` follows `not called` or `rejected` |
| `W1001` | A published channel that nobody in the workspace subscribes to |
| `W1002` | An event of a published channel that no subscription handles |
| `W1003` | A channel has `@retry` and no `@deadLetter`: a message whose retries are used up is gone |
| `E1011` | `emits` is wrong: on a flow with several edges, not an event, or a transition whose service publishes no channel that carries it |

`E0501` now also says: a service with endpoints needs `at`. `E0509` now also covers handler names. `E0602` now also covers the annotations of §10.2: one on something that is not a channel, one written twice, a missing or out-of-range `max`, an unknown `backoff` or another argument, and an argument on `@deadLetter` or `@ordered`.

## 18. Phases

| Phase | Builds | Gate |
|---|---|---|
| **E1** | §2 to §9, §12 (offline), §13 to §15, in the compiler, the reference evaluator and every target; the memory transport; the wire goldens | The same event has the same bytes in Node, Java and Rust, and each decodes the others' |
| **E2** | §10: the Kafka and RabbitMQ adapters, `@retry`, `@deadLetter`, `@ordered`, the inbox, live event examples | A Node producer and a Java consumer talk over both brokers with only the configuration changed, for the rows of §10.1 |
| **E3** | §11 and §16: the outbox, the relay, `emits`, event-aware `diff`, the AsyncAPI export | A crash between commit, relay publish, handler and acknowledgement loses no committed event; duplicates are tolerated |

Until its phase is built, a construct of a later phase is rejected: `emits` with `E0003`. The annotations of §10.2 are accepted by the compiler; a generator that has not built phase E2 refuses a service that uses one with `E0801` (§10.8).

E1 needs no configuration: the application passes a transport. A broker adapter is chosen with the target option `messaging` (§10.7).

## 19. Differences from the proposal

- **Annotations stand before `channel`**, like every other annotation, and not inside its braces.
- **A consumer does not require the publisher's signature** (§8). The proposal said a consumer "checks the signature"; taken literally that would reject every message after any change to an event, including the addition of an optional field, which the proposal also calls safe.
- **`veldrules`** is new: it carries the stale-client rule of `spec/rules.md` §9.7 over to consumers.
- **`expect … called once`** is phase E2 (§10.9). In E1 an example describes one delivery.
- **`messaging` is an option of a server target** (`[server.options]`, §10.7), not a key of `[service]`: a service may have a Node server and a Java server, and each chooses its adapters. Addresses and credentials are the application's, given to the adapter as a configured client of the library.
- **Java uses the plain clients** (`kafka-clients`, `amqp-client`) under Spring too, so that both Java servers share one adapter. The proposal named Spring Kafka and Spring AMQP.
- **`@retry` is carried out by the generated consumer** (§10.4), not by each broker in its own way, so one test on the memory transport covers it for every broker.
- **The example in the proposal used `"b-1"` for a `uuid`**; examples here are type-correct.

## 20. Decisions

Decided by the owner on 25 November 2026:

1. **Contract skew (§8): the tolerant consumer.** A consumer decodes with its own type and ignores unknown fields, and checks rules only when the digests match. The alternative, rejecting every message whose signature differs, would have made every change to an event a coordinated deployment.
2. **A service without `at` (§5): allowed** for a service that has no endpoints.

Confirmed by the owner on 25 November 2026, in the evening:

3. **Rejected messages are acknowledged (§8).** Otherwise one bad message blocks a Kafka partition forever. Without `@deadLetter` and without a hook that stores it, the message is gone.
4. **`@retry` lives on the channel**, one policy for every consumer of it, as in the proposal. The alternative is per subscription.
