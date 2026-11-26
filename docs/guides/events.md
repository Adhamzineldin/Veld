# Events

A service can publish events and subscribe to them. You declare the events and the channels in the contract, and Veld generates a typed publisher and a typed subscriber. The contract names no broker: a channel becomes a topic, and the code you write talks to a small `Transport` interface.

**What is built today.** The language, `veld check`, `veld test`, `veld fmt`, `veld diff` and the editor support know events. The **Node server** generates them, with a memory transport that needs no broker. The Java and Rust servers refuse a service with events (`E0801`) until their generators are built. Adapters for Kafka and RabbitMQ are not built yet. The normative text is [`spec/events.md`](../../spec/events.md).

## 1. Declare events, a channel and who uses it

```veld
/// Published when a booking is paid.
event BookingPaid {
  bookingId: uuid
  amount: decimal
  paidAt: datetime
  nights: int = 1

  rule positive: amount > 0
  rule shortStay: nights <= 30
    message "A stay is at most 30 nights"
}

/// Published when a booking is cancelled.
event BookingCancelled {
  bookingId: uuid
  note?: string
}

@topic("shop.bookings.v1")
channel BookingEvents {
  carries BookingPaid, BookingCancelled
  key bookingId
}

@auth(none)
service Bookings at "/bookings" {
  publishes BookingEvents
  get "/ping" ping() -> 204
}

/// A service without `at`: it has no endpoints, it only consumes.
service Notifications {
  subscribes BookingEvents as "notifications" {
    on BookingPaid      -> sendReceipt
    on BookingCancelled -> sendApology
  }
}
```

- An **`event`** is written like a model: fields, defaults, constraints and rules. It cannot be used as the type of a field or a parameter, and it has no `extends`.
- A **`channel`** says which events travel together. `key` names a field every carried event has; messages with the same key keep their order. `@topic` sets the topic name; without it the name comes from the package and the channel.
- **`publishes`** and **`subscribes`** go in a service. A subscription names a consumer group (`as "notifications"`) and one handler per event it wants. An event the channel carries and the subscription does not list is ignored.

## 2. Check it and test it without a broker

`veld check` reports the mistakes a broker would only show at run time: a key that one of the events lacks, two channels on one topic, a subscription to something that is not a channel, an event nobody carries.

An `example` states what an event reaches:

```veld
example "a paid booking reaches the receipt handler" {
  given event BookingPaid { amount: 120.00 }
  expect Notifications.sendReceipt called
  expect Notifications.sendApology not called
}

example "an event that breaks a rule reaches nobody" {
  given event BookingPaid { amount: 0 }
  expect rejected
}
```

Fields you leave out are filled in, as in every example. `veld test` runs them:

```text
$ veld test
ok   a paid booking reaches the receipt handler
ok   an event that breaks a rule reaches nobody
✓ 2 passed
```

When an event is carried by more than one channel, say which one: `given event BookingPaid { … } on BookingEvents`.

## 3. Generate

With a Node server target, `veld generate` adds these files for a contract with events (a contract without events gets exactly the files it got before):

| File | What is in it |
|---|---|
| `events/bookings.ts` | the event types, each with its signature, schema and check function |
| `server/bookings-events.ts` | `createBookingsPublisher` |
| `server/notifications-events.ts` | `NotificationsEventHandlers` and `subscribeNotificationsEvents` |
| `runtime/events.ts` | the `Transport` interface, the memory transport, the envelope |

A service with events and no endpoints gets no HTTP handler. The event files are the same for every Node framework.

## 4. Publish and subscribe

```ts
import { createBookingsPublisher, subscribeNotificationsEvents, veldEvents } from './generated';

async function main(): Promise<void> {
  // One process, no broker: the memory transport delivers to every group that subscribed.
  const transport = new veldEvents.MemoryTransport();

  await subscribeNotificationsEvents(
    transport,
    {
      sendReceipt(event, meta) {
        console.log(`receipt for ${event.bookingId}: ${event.amount} (message ${meta.id})`);
      },
      sendApology(event) {
        console.log(`apology for ${event.bookingId}`);
      },
    },
    { onEventRejected: (e) => console.error(`rejected: ${e.reason}`) },
  );

  const publisher = createBookingsPublisher(transport);
  await publisher.bookingEvents.bookingPaid({
    bookingId: '0b5c9c3e-7a51-4c1f-9d7e-2f6a1f3b8c11',
    amount: '120.00',
    paidAt: '2026-06-10T12:00:00Z',
    nights: 2,
  });

  // A value that breaks a rule never leaves the publisher.
  try {
    await publisher.bookingEvents.bookingPaid({
      bookingId: '0b5c9c3e-7a51-4c1f-9d7e-2f6a1f3b8c11',
      amount: '0',
      paidAt: '2026-06-10T12:00:00Z',
      nights: 2,
    });
  } catch (err) {
    console.log(`not published: ${(err as Error).name}`);
  }

  await transport.idle();
}

void main();
```

```text
receipt for 0b5c9c3e-7a51-4c1f-9d7e-2f6a1f3b8c11: 120.00 (message 722c25ed-7389-40bc-9585-b2702edfae5f)
not published: ValidationError
```

- The publisher has one member per channel and one method per event. It applies defaults, validates the shape, checks the rules, and only then hands the bytes to the transport.
- The subscriber is one function per service. You implement the handler interface: one method per `on` line, with the typed event and the message's metadata (`id`, `source`, `type`, `subject`, `time`, `attempt`).
- **Delivery is at least once.** A handler that throws is called again with the same message, so a handler must be safe to run twice. The memory transport tries three times and then puts the message in a list a test can read (`transport.deadLetters()`).
- `transport.idle()` completes when everything published has been delivered, so a test never sleeps.

## 5. What a message looks like

The message is a [CloudEvents 1.0](https://cloudevents.io) envelope in JSON, written in one canonical form, so that every generator produces the same bytes for the same event:

```json
{
  "data": {
    "amount": "120.00",
    "bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21",
    "nights": 1,
    "paidAt": "2026-06-10T11:59:58Z"
  },
  "datacontenttype": "application/json",
  "id": "0b5c9c3e-6a57-4d0e-9a55-1f0f6c2f7a11",
  "source": "veld:bookings/Bookings",
  "specversion": "1.0",
  "subject": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21",
  "time": "2026-06-10T12:00:00Z",
  "type": "veld:bookings/BookingPaid@88c362a416ee3a4d",
  "veldrules": "0018ccc5f17d6c76"
}
```

`type` is the event's signature, `subject` is the key, and `veldrules` is a digest of the event's rules. A system that is not generated by Veld can read and write these messages: it is plain JSON with documented members.

## 6. What a consumer does with a message

In this order:

1. **Is it an envelope?** If not, the message is rejected as `malformed`.
2. **Is it an event this subscription handles?** If not, it is ignored. That is how a channel can gain an event without breaking the consumers that exist.
3. **Does the data have the right shape?** The consumer decodes with its own version of the event and ignores fields it does not know. A missing required field rejects the message as `shape`.
4. **Do the rules hold?** They are checked only when the publisher's rules digest equals the consumer's. When the two sides were generated from different versions of the rules, the consumer skips the check and tells you through `onContractMismatch`, instead of rejecting messages the publisher considered valid.
5. **The handler runs.**

A rejected message is **acknowledged**, so one bad message cannot block everything behind it, and it is reported to `onEventRejected` with the reason, the issues and the bytes. If you need to keep such messages, store them in that hook.

The hooks of a subscriber are `onEventRejected`, `onContractMismatch`, `onRuleViolation` and `onHandlerError`. A publisher takes `onRuleViolation`.

## 7. Rules on events

An event's rules are the invariants of [the rules guide](rules.md), with the same expressions. They run in the publisher, so a value that breaks one is never published, and in the consumer, when both sides have the same rules.

`@rollout(audit)` works on an event's rule too: the rule is evaluated on both sides and reported to `onRuleViolation`, and it never stops a publish or rejects a message. Use it to see what a new rule would reject before you enforce it.

## 8. Changing events

`veld diff` judges a change to an event stream:

| Change | Result |
|---|---|
| A new event, a new channel, a channel that carries one more event, a new `publishes` line, a new subscription, a new `on` line | compatible |
| Anything else about an event, a channel or a subscription (a field or a rule of an event, the topic, the key, an event taken off a channel, a subscription's group) | possibly breaking |

"Possibly breaking" fails `veld diff --strict` and stops `veld generate` until you pass `--accept-breaking`. The tool does not yet look inside an event to tell you which side a change hurts; until it does, read these changes yourself.

## 9. Transports

The generated code depends on three things only:

```text
publish(topic, key or none, bytes)
subscribe(topic, group, deliver) -> handle      deliver answers "done" or "again"
message = { key or none, bytes, attempt }
```

- The **memory transport** is generated with every server that has events. It is for tests and for services that live in one process.
- To use a broker today, implement `Transport` for it: publish the bytes to the topic with the key, and call `deliver` for each message a group receives, acknowledging on `done` and redelivering on `again`.

## What is not built yet

- Events in the Java and Rust generators.
- Adapters for Kafka and RabbitMQ, retry policies, a dead-letter channel, and deduplication of redelivered messages.
- Publishing an event from a state change (`emits`) with an outbox, so that the event and the write cannot disagree.
- An AsyncAPI export, and a `veld diff` that looks inside events.
- A counter for ignored messages.
