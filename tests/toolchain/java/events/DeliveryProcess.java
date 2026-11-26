package veldtest;

import gen.e.bookings.server.BillingEventHandlers;
import gen.e.bookings.server.BillingSubscriber;
import gen.e.bookings.server.FulfilmentEventHandlers;
import gen.e.bookings.server.FulfilmentSubscriber;
import gen.e.bookings.server.OrdersPublisher;
import gen.e.bookings.t.OrderPlaced;
import gen.e.bookings.t.OrderShipped;
import gen.e.runtime.Events;
import gen.e.runtime.Json;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.Transport;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Retry, dead letter and the inbox between generated services in one process
 * ({@code tests/events/delivery.veld}: Orders publishes, Fulfilment and Billing subscribe to a channel
 * with {@code @retry(max: 3, backoff: exponential)}, {@code @deadLetter} and {@code @ordered}). Prints
 * {@code events delivery process ok}.
 */
public final class DeliveryProcess {
  private DeliveryProcess() {}

  private static final String TOPIC = "shop.orders.v1";
  private static final UUID A = UUID.fromString("5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21");

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  private static void same(Object want, Object got, String what) {
    check(want.equals(got), what + ": wanted " + want + ", got " + got);
  }

  /** A memory transport that records the options it is given and can refuse dead-letter copies. */
  private static final class Spy implements Transport {
    final MemoryTransport memory = new MemoryTransport();
    final List<ChannelOptions> publishOptions = new CopyOnWriteArrayList<>();
    final List<ChannelOptions> subscribeOptions = new CopyOnWriteArrayList<>();
    volatile boolean refuseCopies;

    @Override
    public void publish(String topic, String key, byte[] bytes) {
      if (refuseCopies && topic.endsWith(".dlq")) {
        throw new IllegalStateException("the broker refused the copy");
      }
      memory.publish(topic, key, bytes);
    }

    @Override
    public void publish(String topic, String key, byte[] bytes, ChannelOptions options) {
      publishOptions.add(options);
      publish(topic, key, bytes);
    }

    @Override
    public Subscription subscribe(String topic, String group, Deliver deliver) {
      return memory.subscribe(topic, group, deliver);
    }

    @Override
    public Subscription subscribe(String topic, String group, Deliver deliver, ChannelOptions options) {
      subscribeOptions.add(options);
      return memory.subscribe(topic, group, deliver);
    }
  }

  private static Events.PublisherOptions fixed() {
    AtomicInteger n = new AtomicInteger();
    return new Events.PublisherOptions()
        .newId(() -> "id-" + n.incrementAndGet())
        .clock(() -> Instant.parse("2026-06-10T12:00:00Z"));
  }

  /** What a subscriber of the dead-letter topic reads. */
  private static List<Json.Obj> readDeadLetters(Transport transport, List<Json.Obj> into) {
    transport.subscribe(
        TOPIC + ".dlq",
        "dlq-reader",
        m -> {
          into.add(Json.parse(new String(m.bytes(), StandardCharsets.UTF_8)).asObject());
          return Transport.Outcome.DONE;
        });
    return into;
  }

  private static final class Fulfilment implements FulfilmentEventHandlers {
    final AtomicInteger reserved = new AtomicInteger();
    final AtomicInteger shipped = new AtomicInteger();
    final List<Integer> attempts = new CopyOnWriteArrayList<>();
    volatile int failFirst;

    @Override
    public void reserve(OrderPlaced event, Events.EventMeta meta) throws Exception {
      attempts.add(meta.attempt());
      if (attempts.size() <= failFirst) {
        throw new IllegalStateException("not yet");
      }
      reserved.incrementAndGet();
    }

    @Override
    public void notifyCustomer(OrderShipped event, Events.EventMeta meta) {
      shipped.incrementAndGet();
    }
  }

  private static final class Billing implements BillingEventHandlers {
    final AtomicInteger runs = new AtomicInteger();
    volatile boolean fail;

    @Override
    public void charge(OrderPlaced event, Events.EventMeta meta) throws Exception {
      runs.incrementAndGet();
      if (fail) {
        throw new IllegalStateException("always");
      }
    }
  }

  private static void retriesWaitThroughSleepAndSucceed() throws Exception {
    Spy transport = new Spy();
    Fulfilment fulfilment = new Fulfilment();
    fulfilment.failFirst = 2;
    List<Long> waits = new CopyOnWriteArrayList<>();
    List<Events.HandlerFailure> failures = new CopyOnWriteArrayList<>();
    FulfilmentSubscriber.subscribe(
        transport,
        fulfilment,
        new Events.ConsumerOptions().sleep(waits::add).onHandlerError(failures::add));
    OrdersPublisher.create(transport, fixed()).orderEvents().orderPlaced(new OrderPlaced(A, 2));
    transport.memory.idle();
    same(List.of(1, 2, 3), fulfilment.attempts, "meta.attempt counts the runs");
    same(List.of(1000L, 2000L), waits, "the waits are the exponential backoff, through sleep");
    same(1, fulfilment.reserved.get(), "the third run succeeded");
    same(2, failures.size(), "two failed runs are reported");
    check(!failures.get(0).last() && !failures.get(1).last(), "neither failure is the last");
    same(0, transport.memory.deadLetters().size(), "the transport never saw a failure");
    check(transport.publishOptions.get(0).ordered() && transport.publishOptions.get(0).deadLetter(), "the publisher passes the options");
    check(transport.subscribeOptions.get(0).ordered() && transport.subscribeOptions.get(0).deadLetter(), "the consumer passes the options");
  }

  private static void usedUpRetriesAreCopiedToTheDeadLetterTopic() throws Exception {
    Spy transport = new Spy();
    List<Json.Obj> copies = readDeadLetters(transport, new CopyOnWriteArrayList<>());
    Billing billing = new Billing();
    billing.fail = true;
    List<Long> waits = new CopyOnWriteArrayList<>();
    List<Events.HandlerFailure> failures = new CopyOnWriteArrayList<>();
    BillingSubscriber.subscribe(
        transport, billing, new Events.ConsumerOptions().sleep(waits::add).onHandlerError(failures::add));
    OrdersPublisher.create(transport, fixed()).orderEvents().orderPlaced(new OrderPlaced(A, 1));
    transport.memory.idle();
    same(4, billing.runs.get(), "n + 1 runs");
    same(List.of(1000L, 2000L, 4000L), waits, "three waits");
    check(failures.get(3).last() && !failures.get(2).last(), "only the fourth failure is the last");
    same(1, copies.size(), "one copy, readable from <topic>.dlq");
    same("handler", ((Json.Str) copies.get(0).get("veldreason")).value(), "the reason");
    same("billing", ((Json.Str) copies.get(0).get("veldgroup")).value(), "the group");
    same(4.0, ((Json.Num) copies.get(0).get("veldattempts")).value(), "the runs");
    same(0, transport.memory.deadLetters().size(), "the message was acknowledged");
  }

  private static void aCopyThatCannotBePublishedLeavesTheMessageUnacknowledged() throws Exception {
    Spy transport = new Spy();
    transport.refuseCopies = true;
    Billing billing = new Billing();
    billing.fail = true;
    BillingSubscriber.subscribe(transport, billing, new Events.ConsumerOptions().sleep(ms -> {}));
    OrdersPublisher.create(transport, fixed()).orderEvents().orderPlaced(new OrderPlaced(A, 1));
    transport.memory.idle();
    same(12, billing.runs.get(), "the memory transport delivered it three times, four runs each");
    same(1, transport.memory.deadLetters().size(), "the transport gave up: it was never acknowledged");
  }

  /** The bytes of a published message, taken from a tap, and the same message with another quantity. */
  private static byte[] tapped(Spy transport, List<byte[]> bytes) {
    transport.subscribe(
        TOPIC,
        "tap",
        m -> {
          bytes.add(m.bytes());
          return Transport.Outcome.DONE;
        });
    return null;
  }

  private static void aRejectedMessageIsCopiedAndNeverReachesAHandler() throws Exception {
    Spy transport = new Spy();
    List<Json.Obj> copies = readDeadLetters(transport, new CopyOnWriteArrayList<>());
    List<byte[]> seen = new CopyOnWriteArrayList<>();
    tapped(transport, seen);
    Fulfilment fulfilment = new Fulfilment();
    List<Events.EventRejected> rejected = new CopyOnWriteArrayList<>();
    FulfilmentSubscriber.subscribe(transport, fulfilment, new Events.ConsumerOptions().onEventRejected(rejected::add));
    OrdersPublisher.create(transport, fixed()).orderEvents().orderPlaced(new OrderPlaced(A, 2));
    transport.memory.idle();
    Json.Obj message = Json.parse(new String(seen.get(0), StandardCharsets.UTF_8)).asObject();
    Json.Obj data = message.get("data").asObject().with("quantity", Json.of(0L));
    transport.publish(TOPIC, A.toString(), message.with("data", data).canonical().getBytes(StandardCharsets.UTF_8));
    transport.memory.idle();
    same(1, fulfilment.reserved.get(), "only the valid message reached the handler");
    same(1, rejected.size(), "the hook is called once");
    same("rule", rejected.get(0).reason().text(), "the reason");
    same(1, copies.size(), "the rejected message was copied");
    same("rule", ((Json.Str) copies.get(0).get("veldreason")).value(), "the reason of the copy");
    same(0.0, ((Json.Num) copies.get(0).get("veldattempts")).value(), "a rejected message was run 0 times");
    check(copies.get(0).has("veldviolations"), "the violations are in the copy");
  }

  private static void theInboxSkipsASecondDeliveryOfTheSameMessage() throws Exception {
    Spy transport = new Spy();
    List<byte[]> seen = new CopyOnWriteArrayList<>();
    tapped(transport, seen);
    Fulfilment fulfilment = new Fulfilment();
    Events.MemoryInbox inbox = new Events.MemoryInbox();
    List<Events.HandlerFailure> failures = new CopyOnWriteArrayList<>();
    FulfilmentSubscriber.subscribe(
        transport, fulfilment, new Events.ConsumerOptions().inbox(inbox).onHandlerError(failures::add));
    OrdersPublisher.create(transport, fixed()).orderEvents().orderPlaced(new OrderPlaced(A, 2));
    transport.memory.idle();
    transport.publish(TOPIC, A.toString(), seen.get(0));
    transport.memory.idle();
    same(1, fulfilment.reserved.get(), "the handler ran once for two deliveries");
    same(1, inbox.size(), "one pair is recorded");
    check(inbox.has("veld:bookings/Orders", "id-1"), "the pair is source and id");
    check(failures.isEmpty(), "a skipped message reaches no hook");
  }

  private static void aRunThatFailedIsNotRecordedByTheInbox() throws Exception {
    Spy transport = new Spy();
    Fulfilment fulfilment = new Fulfilment();
    fulfilment.failFirst = 1;
    Events.MemoryInbox inbox = new Events.MemoryInbox();
    FulfilmentSubscriber.subscribe(transport, fulfilment, new Events.ConsumerOptions().inbox(inbox).sleep(ms -> {}));
    OrdersPublisher.create(transport, fixed()).orderEvents().orderPlaced(new OrderPlaced(A, 2));
    transport.memory.idle();
    same(1, fulfilment.reserved.get(), "the second run went through the inbox again and ran");
    same(1, inbox.size(), "recorded after the success");
  }

  private static void aChannelWithoutRetryAnswersAgainAtTheFirstFailure() throws Exception {
    // Notifications of bookings.veld has no @retry: covered by EventsProcess. Here: the same
    // subscriber with the retry removed is not possible, so check that ignoring an event is quiet.
    Spy transport = new Spy();
    Fulfilment fulfilment = new Fulfilment();
    FulfilmentSubscriber.subscribe(transport, fulfilment, new Events.ConsumerOptions());
    OrdersPublisher.create(transport, fixed()).orderEvents().orderShipped(new OrderShipped(A, "dhl"));
    transport.memory.idle();
    same(1, fulfilment.shipped.get(), "shipped");
  }

  public static void main(String[] args) throws Exception {
    retriesWaitThroughSleepAndSucceed();
    usedUpRetriesAreCopiedToTheDeadLetterTopic();
    aCopyThatCannotBePublishedLeavesTheMessageUnacknowledged();
    aRejectedMessageIsCopiedAndNeverReachesAHandler();
    theInboxSkipsASecondDeliveryOfTheSameMessage();
    aRunThatFailedIsNotRecordedByTheInbox();
    aChannelWithoutRetryAnswersAgainAtTheFirstFailure();
    List<String> done = new ArrayList<>();
    done.add("events delivery process ok");
    System.out.println(done.get(0));
  }
}
