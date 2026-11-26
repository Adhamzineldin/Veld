package veldtest;

import gen.e.adapters.KafkaTransport;
import gen.e.runtime.Events;
import gen.e.runtime.Transport;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BooleanSupplier;
import org.apache.kafka.clients.admin.Admin;
import org.apache.kafka.clients.admin.NewTopic;
import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.common.TopicPartition;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.StringDeserializer;

/**
 * The rows of spec/events.md section 10.1 for the Kafka adapter, against a real broker named by the
 * environment variable {@code VELD_KAFKA} (bootstrap servers). Every topic and group is new for each
 * run; the program creates its topics with the admin client, because the adapter creates nothing.
 * Prints {@code events kafka ok}. About 40 seconds: one row keeps a delivery running for nine
 * seconds, longer than the smallest session timeout of the broker.
 */
public final class EventsKafka {
  private EventsKafka() {}

  private static final String RUN = UUID.randomUUID().toString().substring(0, 8);
  private static String bootstrap;
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
    long until = System.nanoTime() + Duration.ofSeconds(30).toNanos();
    while (!condition.getAsBoolean()) {
      check(System.nanoTime() < until, "timed out waiting for " + what);
      Thread.sleep(50);
    }
  }

  private static Map<String, Object> properties() {
    Map<String, Object> p = new HashMap<>();
    p.put("bootstrap.servers", bootstrap);
    p.put("session.timeout.ms", 6000);
    p.put("max.poll.interval.ms", 6000);
    return p;
  }

  private static KafkaTransport transport() {
    return new KafkaTransport(properties(), new KafkaTransport.Options().redelivery(50, 400));
  }

  private static void create(String topic, int partitions) throws Exception {
    try (Admin admin = Admin.create(Map.<String, Object>of("bootstrap.servers", bootstrap))) {
      admin.createTopics(List.of(new NewTopic(topic, partitions, (short) 1))).all().get();
    }
  }

  private static byte[] bytes(String s) {
    return s.getBytes(StandardCharsets.UTF_8);
  }

  /** Everything in a topic, read from the start with a consumer of its own: partition, key, text. */
  private static List<String> readAll(String topic, int expected) throws Exception {
    Map<String, Object> p = new HashMap<>();
    p.put("bootstrap.servers", bootstrap);
    p.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class.getName());
    p.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
    p.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, false);
    List<String> out = new ArrayList<>();
    try (KafkaConsumer<String, byte[]> consumer = new KafkaConsumer<>(p)) {
      List<TopicPartition> all = new ArrayList<>();
      consumer.partitionsFor(topic).forEach(i -> all.add(new TopicPartition(topic, i.partition())));
      consumer.assign(all);
      consumer.seekToBeginning(all);
      long until = System.nanoTime() + Duration.ofSeconds(15).toNanos();
      while (out.size() < expected && System.nanoTime() < until) {
        for (ConsumerRecord<String, byte[]> r : consumer.poll(Duration.ofMillis(200))) {
          out.add(r.partition() + "|" + r.key() + "|" + new String(r.value(), StandardCharsets.UTF_8));
        }
      }
    }
    return out;
  }

  /** A subscription that collects what it is given and answers DONE. */
  private static Transport.Subscription collect(
      KafkaTransport t, String topic, String group, List<Transport.Message> into) {
    return t.subscribe(
        topic,
        group,
        m -> {
          into.add(m);
          return Transport.Outcome.DONE;
        });
  }

  private static void publishAndConsumeWithTheKey() throws Exception {
    String topic = fresh("basic");
    create(topic, 1);
    try (KafkaTransport t = transport()) {
      List<Transport.Message> got = new CopyOnWriteArrayList<>();
      collect(t, topic, fresh("g"), got);
      t.publish(topic, "k1", bytes("hello"));
      t.publish(topic, null, bytes("no key"));
      await("two messages", () -> got.size() == 2);
      same("k1", got.get(0).key(), "the key is the record key");
      same("hello", new String(got.get(0).bytes(), StandardCharsets.UTF_8), "the bytes are the value");
      same(1, got.get(0).attempt(), "the first attempt is 1");
      check(got.get(1).key() == null, "a message without a key has none");
    }
  }

  private static void orderPerKey() throws Exception {
    String topic = fresh("order");
    create(topic, 3);
    try (KafkaTransport t = transport()) {
      List<Transport.Message> got = new CopyOnWriteArrayList<>();
      collect(t, topic, fresh("g"), got);
      for (int i = 0; i < 30; i++) {
        t.publish(topic, "key" + (i % 3), bytes("m" + i));
      }
      await("30 messages", () -> got.size() == 30);
      for (int k = 0; k < 3; k++) {
        List<Integer> seen = new ArrayList<>();
        for (Transport.Message m : got) {
          if (("key" + k).equals(m.key())) {
            seen.add(Integer.parseInt(new String(m.bytes(), StandardCharsets.UTF_8).substring(1)));
          }
        }
        List<Integer> sorted = new ArrayList<>(seen);
        java.util.Collections.sort(sorted);
        same(sorted, seen, "the order of key" + k);
        same(10, seen.size(), "every message of key" + k);
      }
    }
  }

  private static void aFailedHandlerIsDeliveredAgainBySameConsumer() throws Exception {
    String topic = fresh("again");
    create(topic, 2);
    try (KafkaTransport t = transport()) {
      List<String> order = new CopyOnWriteArrayList<>();
      t.subscribe(
          topic,
          fresh("g"),
          m -> {
            String text = new String(m.bytes(), StandardCharsets.UTF_8);
            order.add(text + "@" + m.attempt());
            if (text.equals("first") && m.attempt() < 3) {
              return Transport.Outcome.AGAIN;
            }
            return Transport.Outcome.DONE;
          });
      t.publish(topic, "a", bytes("first"));
      t.publish(topic, "b", bytes("second"));
      await("both done", () -> order.contains("second@1") && order.contains("first@3"));
      same(List.of("first@1", "first@2", "first@3", "second@1"), order, "nothing else is delivered while a message is delivered again");
    }
  }

  private static void orderedGoesToPartitionZero() throws Exception {
    String topic = fresh("ordered");
    create(topic, 3);
    try (KafkaTransport t = transport()) {
      for (int i = 0; i < 9; i++) {
        t.publish(topic, "key" + i, bytes("m" + i), new Transport.ChannelOptions(true, false));
      }
    }
    List<String> all = readAll(topic, 9);
    same(9, all.size(), "all nine messages");
    for (String line : all) {
      check(line.startsWith("0|"), "every message is in partition 0: " + line);
    }
  }

  private static void theDeadLetterTopic() throws Exception {
    String topic = fresh("dead");
    create(topic, 1);
    create(topic + ".dlq", 1);
    try (KafkaTransport t = transport()) {
      List<Events.EventRejected> rejected = new CopyOnWriteArrayList<>();
      Events.subscribe(
          t,
          List.of(new Events.SubscriptionDef(topic, fresh("g"), List.of(), null, true, false)),
          new Events.ConsumerOptions().onEventRejected(rejected::add));
      t.publish(topic, "k9", bytes("this is not an envelope"));
      await("the rejection", () -> rejected.size() == 1);
      List<String> copies = readAll(topic + ".dlq", 1);
      same(1, copies.size(), "one copy in <topic>.dlq");
      check(copies.get(0).contains("|k9|this is not an envelope"), "the copy is the original with its key: " + copies.get(0));
    }
  }

  private static void aMessageBeforeTheGroupIsRead() throws Exception {
    String topic = fresh("early");
    create(topic, 1);
    try (KafkaTransport t = transport()) {
      t.publish(topic, "k", bytes("before"));
      List<Transport.Message> got = new CopyOnWriteArrayList<>();
      collect(t, topic, fresh("g"), got);
      await("the early message", () -> got.size() == 1);
      same("before", new String(got.get(0).bytes(), StandardCharsets.UTF_8), "a new group starts at the earliest offset");
    }
  }

  private static void twoConsumersOfOneGroupShareTheTopic() throws Exception {
    String topic = fresh("share");
    create(topic, 2);
    String group = fresh("g");
    try (KafkaTransport a = transport();
        KafkaTransport b = transport()) {
      List<Transport.Message> first = new CopyOnWriteArrayList<>();
      List<Transport.Message> second = new CopyOnWriteArrayList<>();
      collect(a, topic, group, first);
      collect(b, topic, group, second);
      for (int i = 0; i < 40; i++) {
        a.publish(topic, "k" + i, bytes("m" + i));
      }
      await("40 messages", () -> first.size() + second.size() >= 40);
      Thread.sleep(500);
      same(40, first.size() + second.size(), "every message once in the group");
      check(!first.isEmpty() && !second.isEmpty(), "both consumers got work: " + first.size() + "/" + second.size());
    }
  }

  private static void closeStopsTheConsumers() throws Exception {
    String topic = fresh("close");
    create(topic, 1);
    String group = fresh("g");
    KafkaTransport t = transport();
    List<Transport.Message> got = new CopyOnWriteArrayList<>();
    collect(t, topic, group, got);
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
    try (KafkaTransport other = transport()) {
      other.publish(topic, "k", bytes("three"));
      Thread.sleep(1500);
      same(1, got.size(), "a closed subscription gets nothing more");
      List<Transport.Message> next = new CopyOnWriteArrayList<>();
      collect(other, topic, group, next);
      await("the group goes on", () -> next.size() >= 1);
      same("three", new String(next.get(0).bytes(), StandardCharsets.UTF_8), "the group's offset is after the committed message");
    }
  }

  private static void aDeliveryThatOutlivesTheSession() throws Exception {
    String topic = fresh("long");
    create(topic, 1);
    String group = fresh("g");
    try (KafkaTransport t = transport()) {
      AtomicInteger runs = new AtomicInteger();
      List<String> done = new CopyOnWriteArrayList<>();
      t.subscribe(
          topic,
          group,
          m -> {
            runs.incrementAndGet();
            try {
              Thread.sleep(9000);
            } catch (InterruptedException e) {
              Thread.currentThread().interrupt();
              return Transport.Outcome.AGAIN;
            }
            done.add(new String(m.bytes(), StandardCharsets.UTF_8));
            return Transport.Outcome.DONE;
          });
      t.publish(topic, "k", bytes("slow"));
      await("the slow message", () -> done.size() == 1);
      Thread.sleep(2000);
      same(1, runs.get(), "the session stayed alive: no rebalance, no second delivery");
    }
    try (KafkaTransport t = transport()) {
      List<Transport.Message> got = new CopyOnWriteArrayList<>();
      collect(t, topic, group, got);
      t.publish(topic, "k", bytes("marker"));
      await("the marker", () -> got.size() >= 1);
      same("marker", new String(got.get(0).bytes(), StandardCharsets.UTF_8), "the slow message was committed");
    }
  }

  public static void main(String[] args) throws Exception {
    bootstrap = System.getenv("VELD_KAFKA");
    if (bootstrap == null || bootstrap.isEmpty()) {
      throw new IllegalStateException("VELD_KAFKA is not set");
    }
    publishAndConsumeWithTheKey();
    orderPerKey();
    aFailedHandlerIsDeliveredAgainBySameConsumer();
    orderedGoesToPartitionZero();
    theDeadLetterTopic();
    aMessageBeforeTheGroupIsRead();
    twoConsumersOfOneGroupShareTheTopic();
    closeStopsTheConsumers();
    aDeliveryThatOutlivesTheSession();
    System.out.println("events kafka ok");
  }
}
