package veldtest;

import com.rabbitmq.client.Channel;
import com.rabbitmq.client.Connection;
import com.rabbitmq.client.ConnectionFactory;
import com.rabbitmq.client.GetResponse;
import gen.e.adapters.RabbitMqTransport;
import gen.e.runtime.Events;
import gen.e.runtime.Transport;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.BooleanSupplier;

/**
 * The rows of spec/events.md section 10.1 for the RabbitMQ adapter, against a real broker named by
 * the environment variable {@code VELD_RABBITMQ} (an AMQP URL). Every topic and group is new for each
 * run. Prints {@code events rabbitmq ok}.
 */
public final class EventsRabbitmq {
  private EventsRabbitmq() {}

  private static final String RUN = UUID.randomUUID().toString().substring(0, 8);
  private static Connection connection;
  private static int counter;

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  private static void same(Object want, Object got, String what) {
    check(want.equals(got), what + ": wanted " + want + ", got " + got);
  }

  private static String fresh(String what) {
    return "veld-" + RUN + "-" + what + "-" + (++counter);
  }

  private static void await(String what, BooleanSupplier condition) throws InterruptedException {
    long until = System.nanoTime() + Duration.ofSeconds(20).toNanos();
    while (!condition.getAsBoolean()) {
      check(System.nanoTime() < until, "timed out waiting for " + what);
      Thread.sleep(50);
    }
  }

  private static RabbitMqTransport transport() {
    return new RabbitMqTransport(connection, new RabbitMqTransport.Options().redelivery(50, 400));
  }

  private static byte[] bytes(String s) {
    return s.getBytes(StandardCharsets.UTF_8);
  }

  private static String text(Transport.Message m) {
    return new String(m.bytes(), StandardCharsets.UTF_8);
  }

  private static Transport.Subscription collect(
      RabbitMqTransport t, String topic, String group, List<Transport.Message> into, Transport.ChannelOptions options) {
    return t.subscribe(
        topic,
        group,
        m -> {
          into.add(m);
          return Transport.Outcome.DONE;
        },
        options);
  }

  private static void publishAndConsumeWithTheKey() throws Exception {
    String topic = fresh("basic");
    RabbitMqTransport t = transport();
    List<Transport.Message> got = new CopyOnWriteArrayList<>();
    collect(t, topic, "g", got, Transport.ChannelOptions.NONE);
    t.publish(topic, "k1", bytes("hello"));
    t.publish(topic, null, bytes("no key"));
    t.publish(topic, "", bytes("empty key"));
    await("three messages", () -> got.size() == 3);
    same("k1", got.get(0).key(), "the key is the routing key");
    same("hello", text(got.get(0)), "the body");
    same(1, got.get(0).attempt(), "the first attempt is 1");
    check(got.get(1).key() == null && got.get(2).key() == null, "no key and the empty key are no key");
    t.close();
  }

  private static void orderPerKeyWithOneConsumer() throws Exception {
    String topic = fresh("order");
    RabbitMqTransport t = transport();
    List<Transport.Message> got = new CopyOnWriteArrayList<>();
    collect(t, topic, "g", got, Transport.ChannelOptions.NONE);
    List<String> sent = new ArrayList<>();
    for (int i = 0; i < 30; i++) {
      sent.add("m" + i);
      t.publish(topic, "key" + (i % 3), bytes("m" + i));
    }
    await("30 messages", () -> got.size() == 30);
    List<String> seen = new ArrayList<>();
    got.forEach(m -> seen.add(text(m)));
    same(sent, seen, "publish order");
    t.close();
  }

  private static void aFailedHandlerIsDeliveredAgain() throws Exception {
    String topic = fresh("again");
    RabbitMqTransport t = transport();
    List<String> order = new CopyOnWriteArrayList<>();
    t.subscribe(
        topic,
        "g",
        m -> {
          order.add(text(m) + "@" + m.attempt());
          return text(m).equals("first") && m.attempt() < 3 ? Transport.Outcome.AGAIN : Transport.Outcome.DONE;
        });
    t.publish(topic, "a", bytes("first"));
    t.publish(topic, "b", bytes("second"));
    await("both done", () -> order.contains("second@1"));
    same(List.of("first@1", "first@2", "first@3", "second@1"), order, "nothing else is delivered meanwhile");
    t.close();
  }

  private static void orderedGivesOneActiveConsumer() throws Exception {
    String topic = fresh("ordered");
    Transport.ChannelOptions ordered = new Transport.ChannelOptions(true, false);
    RabbitMqTransport a = transport();
    RabbitMqTransport b = transport();
    List<Transport.Message> first = new CopyOnWriteArrayList<>();
    List<Transport.Message> second = new CopyOnWriteArrayList<>();
    collect(a, topic, "g", first, ordered);
    collect(b, topic, "g", second, ordered);
    for (int i = 0; i < 10; i++) {
      a.publish(topic, "k", bytes("m" + i));
    }
    await("ten messages", () -> first.size() + second.size() == 10);
    check(first.size() == 10 || second.size() == 10, "one consumer is active, the other idle: " + first.size() + "/" + second.size());
    // A queue that exists with other arguments is the broker's PRECONDITION_FAILED, passed on.
    RabbitMqTransport c = transport();
    boolean refused = false;
    try {
      collect(c, topic, "g", new ArrayList<>(), Transport.ChannelOptions.NONE);
    } catch (IllegalStateException e) {
      refused = true;
    }
    check(refused, "a queue declared without x-single-active-consumer is refused");
    a.close();
    b.close();
    c.close();
  }

  private static void theDeadLetterQueue() throws Exception {
    String topic = fresh("dead");
    RabbitMqTransport t = transport();
    List<Events.EventRejected> rejected = new CopyOnWriteArrayList<>();
    Events.subscribe(
        t,
        List.of(new Events.SubscriptionDef(topic, "g", List.of(), null, true, false)),
        new Events.ConsumerOptions().onEventRejected(rejected::add));
    t.publish(topic, "k9", bytes("this is not an envelope"));
    await("the rejection", () -> rejected.size() == 1);
    try (Channel channel = connection.createChannel()) {
      GetResponse copy = null;
      long until = System.nanoTime() + Duration.ofSeconds(10).toNanos();
      while (copy == null && System.nanoTime() < until) {
        copy = channel.basicGet(topic + ".dlq", true);
        Thread.sleep(50);
      }
      check(copy != null, "the dead-letter queue holds the copy");
      same("this is not an envelope", new String(copy.getBody(), StandardCharsets.UTF_8), "the copy is the original");
      same("k9", copy.getEnvelope().getRoutingKey(), "with its key");
    }
    t.close();
  }

  private static void aMessageBeforeTheGroupIsDropped() throws Exception {
    String topic = fresh("early");
    RabbitMqTransport t = transport();
    t.publish(topic, "k", bytes("before"));
    List<Transport.Message> got = new CopyOnWriteArrayList<>();
    collect(t, topic, "g", got, Transport.ChannelOptions.NONE);
    t.publish(topic, "k", bytes("after"));
    await("the later message", () -> got.size() == 1);
    Thread.sleep(500);
    same(1, got.size(), "no queue was bound before: the first message is gone");
    same("after", text(got.get(0)), "the message after");
    t.close();
  }

  private static void twoConsumersOfOneGroupShareTheQueue() throws Exception {
    String topic = fresh("share");
    RabbitMqTransport a = transport();
    RabbitMqTransport b = transport();
    List<Transport.Message> first = new CopyOnWriteArrayList<>();
    List<Transport.Message> second = new CopyOnWriteArrayList<>();
    collect(a, topic, "g", first, Transport.ChannelOptions.NONE);
    collect(b, topic, "g", second, Transport.ChannelOptions.NONE);
    for (int i = 0; i < 40; i++) {
      a.publish(topic, "k" + i, bytes("m" + i));
    }
    await("40 messages", () -> first.size() + second.size() >= 40);
    Thread.sleep(500);
    same(40, first.size() + second.size(), "every message once in the group");
    check(!first.isEmpty() && !second.isEmpty(), "both consumers got work: " + first.size() + "/" + second.size());
    a.close();
    b.close();
  }

  private static void closeStopsTheConsumersAndLeavesTheConnection() throws Exception {
    String topic = fresh("close");
    RabbitMqTransport t = transport();
    List<Transport.Message> got = new CopyOnWriteArrayList<>();
    collect(t, topic, "g", got, Transport.ChannelOptions.NONE);
    t.publish(topic, "k", bytes("one"));
    await("one message", () -> got.size() == 1);
    t.close();
    boolean refused = false;
    try {
      t.publish(topic, "k", bytes("two"));
    } catch (IllegalStateException e) {
      refused = true;
    }
    check(refused, "publish after close fails");
    check(connection.isOpen(), "the connection is the application's: close leaves it open");
    RabbitMqTransport other = transport();
    other.publish(topic, "k", bytes("three"));
    Thread.sleep(1000);
    same(1, got.size(), "a closed subscription gets nothing more");
    List<Transport.Message> next = new CopyOnWriteArrayList<>();
    collect(other, topic, "g", next, Transport.ChannelOptions.NONE);
    await("the queue's message", () -> next.size() == 1);
    same("three", text(next.get(0)), "the message waited in the durable queue");
    other.close();
  }

  public static void main(String[] args) throws Exception {
    String url = System.getenv("VELD_RABBITMQ");
    if (url == null || url.isEmpty()) {
      throw new IllegalStateException("VELD_RABBITMQ is not set");
    }
    ConnectionFactory factory = new ConnectionFactory();
    factory.setUri(url);
    connection = factory.newConnection();
    try {
      publishAndConsumeWithTheKey();
      orderPerKeyWithOneConsumer();
      aFailedHandlerIsDeliveredAgain();
      orderedGivesOneActiveConsumer();
      theDeadLetterQueue();
      aMessageBeforeTheGroupIsDropped();
      twoConsumersOfOneGroupShareTheQueue();
      closeStopsTheConsumersAndLeavesTheConnection();
    } finally {
      connection.close();
    }
    System.out.println("events rabbitmq ok");
  }
}
