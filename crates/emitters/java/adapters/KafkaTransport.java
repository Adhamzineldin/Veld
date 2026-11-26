package dev.veld.adapters;

import dev.veld.runtime.Transport;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.ConsumerRebalanceListener;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.ConsumerRecords;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.clients.consumer.OffsetAndMetadata;
import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerConfig;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.common.KafkaException;
import org.apache.kafka.common.TopicPartition;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.ByteArraySerializer;
import org.apache.kafka.common.serialization.StringDeserializer;
import org.apache.kafka.common.serialization.StringSerializer;

/**
 * {@link Transport} over Kafka (spec/events.md section 10.7). This is the only generated file written
 * against {@code org.apache.kafka:kafka-clients}; the application owns the client properties
 * (addresses, TLS, credentials) and closes nothing but this transport.
 *
 * <p>What the adapter adds to the application's properties, and overrides when they contradict it:
 * for the producer {@code acks=all}, an idempotent producer and one request in flight; for every
 * consumer the group id (the group of the subscription), manual commits after the handler,
 * {@code auto.offset.reset=earliest} (a group that has no offset reads the topic from its start),
 * {@code allow.auto.create.topics=false} and one record per poll.
 *
 * <p>A topic is not created by the adapter, neither a topic nor {@code <topic>.dlq}. (A Java producer
 * cannot ask the broker not to create a topic that a publish names: a cluster that has
 * {@code auto.create.topics.enable} creates it, and the application turns that off on the cluster.)
 * A message is the envelope as the record value with the header {@code content-type}, and the key
 * text as the record key. {@code @ordered} puts every message in partition 0.
 *
 * <p>A message answered {@code AGAIN} is not committed and is delivered again by the same consumer
 * after a delay ({@link Options#redelivery}), while nothing else is delivered to that consumer. The
 * Kafka consumer is not thread-safe and has to poll within {@code max.poll.interval.ms}, so each
 * subscription has one thread that owns the consumer and keeps polling while a delivery runs on a
 * worker thread or waits for its next try: the assigned partitions are paused, so a poll returns
 * nothing, and the group sees a consumer that is alive. {@code attempt} counts the deliveries by this
 * consumer since it started: it starts at 1 again after a restart or a rebalance.
 */
public final class KafkaTransport implements Transport, AutoCloseable {
  /** The media type of the structured CloudEvents JSON mode (spec/events.md section 6.2). */
  private static final String CONTENT_TYPE = "application/cloudevents+json; charset=UTF-8";

  private static final Duration POLL = Duration.ofMillis(100);
  private static final System.Logger LOG = System.getLogger("veld.kafka");

  /** The options of the adapter. */
  public static final class Options {
    private long redeliveryInitialMs = 1000;
    private long redeliveryMaxMs = 30000;
    private long joinTimeoutMs = 60000;

    /** Creates the default options. */
    public Options() {}

    /**
     * The delay before a message answered {@code AGAIN} is delivered again: it starts at {@code
     * initialMs} and doubles up to {@code maxMs} (default 1000 and 30000).
     */
    public Options redelivery(long initialMs, long maxMs) {
      this.redeliveryInitialMs = Math.max(0, initialMs);
      this.redeliveryMaxMs = Math.max(this.redeliveryInitialMs, maxMs);
      return this;
    }

    /** How long {@code subscribe} waits for the consumer to join its group (default 60000). */
    public Options joinTimeoutMs(long value) {
      this.joinTimeoutMs = Math.max(1, value);
      return this;
    }
  }

  private final Map<String, Object> properties;
  private final Options options;
  private final Object lock = new Object();
  private final Set<Running> running = new LinkedHashSet<>();
  private KafkaProducer<String, byte[]> producer;
  private boolean closed;

  /** Builds the adapter from the client properties of the application ({@code bootstrap.servers}, security, ...). */
  public KafkaTransport(Map<String, ?> properties) {
    this(properties, new Options());
  }

  /** Builds the adapter from the client properties of the application and its own options. */
  public KafkaTransport(Map<String, ?> properties, Options options) {
    this.properties = new HashMap<String, Object>(properties);
    this.options = options;
  }

  // -- publishing -----------------------------------------------------------------------------

  @Override
  public void publish(String topic, String key, byte[] bytes) {
    publish(topic, key, bytes, ChannelOptions.NONE);
  }

  @Override
  public void publish(String topic, String key, byte[] bytes, ChannelOptions channel) {
    ProducerRecord<String, byte[]> record =
        new ProducerRecord<>(topic, channel != null && channel.ordered() ? Integer.valueOf(0) : null, key, bytes);
    record.headers().add("content-type", CONTENT_TYPE.getBytes(StandardCharsets.UTF_8));
    try {
      producer().send(record).get();
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new IllegalStateException("veld kafka: interrupted while publishing to " + topic, e);
    } catch (ExecutionException e) {
      throw new IllegalStateException("veld kafka: publishing to " + topic + " failed: " + e.getCause(), e.getCause());
    }
  }

  private KafkaProducer<String, byte[]> producer() {
    synchronized (lock) {
      if (closed) {
        throw new IllegalStateException("veld kafka: the transport is closed");
      }
      if (producer == null) {
        Map<String, Object> p = new HashMap<>(properties);
        p.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, StringSerializer.class.getName());
        p.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
        p.put(ProducerConfig.ACKS_CONFIG, "all");
        p.put(ProducerConfig.ENABLE_IDEMPOTENCE_CONFIG, true);
        p.put(ProducerConfig.MAX_IN_FLIGHT_REQUESTS_PER_CONNECTION, 1);
        p.put(ProducerConfig.RETRIES_CONFIG, Integer.MAX_VALUE);
        producer = new KafkaProducer<>(p);
      }
      return producer;
    }
  }

  // -- consuming ------------------------------------------------------------------------------

  @Override
  public Subscription subscribe(String topic, String group, Deliver deliver) {
    return subscribe(topic, group, deliver, ChannelOptions.NONE);
  }

  /** {@code ordered} and {@code deadLetter} need nothing from a Kafka consumer: they are properties of the topics. */
  @Override
  public Subscription subscribe(String topic, String group, Deliver deliver, ChannelOptions channel) {
    Running subscription;
    synchronized (lock) {
      if (closed) {
        throw new IllegalStateException("veld kafka: the transport is closed");
      }
      subscription = new Running(topic, group, deliver);
      running.add(subscription);
    }
    try {
      subscription.start();
    } catch (RuntimeException e) {
      subscription.close();
      throw e;
    }
    return subscription;
  }

  /**
   * Stops every subscription (a delivery that runs is waited for; one that waits for its next try
   * stops, and its message stays uncommitted for the next consumer) and flushes and closes the
   * producer. The application closes nothing else of Kafka: it owns the properties only.
   */
  @Override
  public void close() {
    List<Running> all;
    KafkaProducer<String, byte[]> last;
    synchronized (lock) {
      closed = true;
      all = new ArrayList<>(running);
      last = producer;
      producer = null;
    }
    for (Running r : all) {
      r.close();
    }
    if (last != null) {
      last.flush();
      last.close(Duration.ofSeconds(30));
    }
  }

  /** The record a subscription is delivering, with its state. Touched by the consumer thread only. */
  private static final class Pending {
    final ConsumerRecord<String, byte[]> record;
    Future<Outcome> future;
    int attempt = 1;
    long notBeforeNanos;
    long waitMs;
    /** The group took the partition away: the message is left to its new owner. */
    boolean lost;

    Pending(ConsumerRecord<String, byte[]> record, long waitMs) {
      this.record = record;
      this.waitMs = waitMs;
    }
  }

  /** One subscription: a consumer, the thread that owns it and the thread that runs the handler. */
  private final class Running implements Subscription, Runnable {
    private final String topic;
    private final String group;
    private final Deliver deliver;
    private final KafkaConsumer<String, byte[]> consumer;
    private final ExecutorService worker;
    private final Thread thread;
    private final CountDownLatch joined = new CountDownLatch(1);
    private volatile boolean stopping;
    private volatile Throwable failure;
    private Pending pending;

    Running(String topic, String group, Deliver deliver) {
      this.topic = topic;
      this.group = group;
      this.deliver = deliver;
      Map<String, Object> p = new HashMap<>(properties);
      p.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class.getName());
      p.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
      p.put(ConsumerConfig.GROUP_ID_CONFIG, group);
      p.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, false);
      p.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "earliest");
      p.put(ConsumerConfig.ALLOW_AUTO_CREATE_TOPICS_CONFIG, false);
      p.put(ConsumerConfig.MAX_POLL_RECORDS_CONFIG, 1);
      this.consumer = new KafkaConsumer<>(p);
      this.worker =
          Executors.newSingleThreadExecutor(
              r -> {
                Thread t = new Thread(r, "veld-kafka-handler-" + topic + "-" + group);
                t.setDaemon(true);
                return t;
              });
      this.thread = new Thread(this, "veld-kafka-" + topic + "-" + group);
      this.thread.setDaemon(true);
    }

    /** Starts the thread and returns when the consumer has joined its group. */
    void start() {
      thread.start();
      try {
        if (!joined.await(options.joinTimeoutMs, TimeUnit.MILLISECONDS)) {
          throw new IllegalStateException(
              "veld kafka: the consumer of group `"
                  + group
                  + "` was not assigned a partition of `"
                  + topic
                  + "` within "
                  + options.joinTimeoutMs
                  + " ms (the adapter creates no topic: does it exist?)");
        }
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        throw new IllegalStateException("veld kafka: interrupted while joining the group `" + group + "`", e);
      }
      if (failure != null) {
        throw new IllegalStateException("veld kafka: the consumer of group `" + group + "` failed: " + failure, failure);
      }
    }

    private final ConsumerRebalanceListener listener =
        new ConsumerRebalanceListener() {
          @Override
          public void onPartitionsAssigned(Collection<TopicPartition> partitions) {
            if (pending != null) {
              consumer.pause(partitions);
            }
            joined.countDown();
          }

          @Override
          public void onPartitionsRevoked(Collection<TopicPartition> partitions) {
            loseIfRevoked(partitions);
          }

          @Override
          public void onPartitionsLost(Collection<TopicPartition> partitions) {
            loseIfRevoked(partitions);
          }
        };

    private void loseIfRevoked(Collection<TopicPartition> partitions) {
      if (pending != null
          && partitions.contains(new TopicPartition(pending.record.topic(), pending.record.partition()))) {
        pending.lost = true;
      }
    }

    @Override
    public void run() {
      try {
        consumer.subscribe(List.of(topic), listener);
        while (!stopping) {
          ConsumerRecords<String, byte[]> records = consumer.poll(POLL);
          if (pending == null) {
            for (ConsumerRecord<String, byte[]> record : records) {
              begin(record);
              break;
            }
          }
          settle();
        }
      } catch (KafkaException | IllegalStateException e) {
        failure = e;
        LOG.log(System.Logger.Level.ERROR, "veld kafka: the consumer of group {0} on {1} stopped: {2}", group, topic, e);
      } finally {
        finish();
        joined.countDown();
      }
    }

    /** A record arrived: everything is paused, and the delivery runs on the worker thread. */
    private void begin(ConsumerRecord<String, byte[]> record) {
      pending = new Pending(record, options.redeliveryInitialMs);
      consumer.pause(consumer.assignment());
      submit();
    }

    private void submit() {
      ConsumerRecord<String, byte[]> record = pending.record;
      Message message =
          new Message(record.key(), record.value() == null ? new byte[0] : record.value(), pending.attempt);
      pending.future =
          worker.submit(
              () -> {
                try {
                  return deliver.deliver(message);
                } catch (RuntimeException | Error e) {
                  return Outcome.AGAIN;
                }
              });
    }

    /** What happens to the delivery in progress: nothing yet, the commit, or the wait for the next try. */
    private void settle() {
      if (pending == null) {
        return;
      }
      if (pending.future == null) {
        if (pending.lost) {
          done();
        } else if (System.nanoTime() - pending.notBeforeNanos >= 0) {
          submit();
        }
        return;
      }
      if (!pending.future.isDone()) {
        return;
      }
      Outcome outcome = outcomeOf(pending.future);
      pending.future = null;
      if (pending.lost) {
        done();
      } else if (outcome == Outcome.DONE) {
        commit(pending.record);
        done();
      } else {
        pending.attempt++;
        pending.notBeforeNanos = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(pending.waitMs);
        pending.waitMs = Math.min(options.redeliveryMaxMs, Math.max(1, pending.waitMs) * 2);
      }
    }

    private Outcome outcomeOf(Future<Outcome> future) {
      try {
        return future.get();
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        return Outcome.AGAIN;
      } catch (ExecutionException e) {
        return Outcome.AGAIN;
      }
    }

    /** The delivery is over: the partitions are read again. */
    private void done() {
      pending = null;
      consumer.resume(consumer.paused());
    }

    private void commit(ConsumerRecord<String, byte[]> record) {
      try {
        consumer.commitSync(
            Map.of(
                new TopicPartition(record.topic(), record.partition()), new OffsetAndMetadata(record.offset() + 1)));
      } catch (KafkaException e) {
        // The group took the partition away meanwhile: its new owner reads the message again.
        LOG.log(
            System.Logger.Level.WARNING,
            "veld kafka: the commit of {0}-{1}@{2} failed: {3}",
            record.topic(),
            record.partition(),
            record.offset(),
            e);
      }
    }

    /**
     * The loop ended. A delivery that runs is waited for and, when it was done, committed; one that
     * waits for its next try is left uncommitted. Then the consumer leaves the group.
     */
    private void finish() {
      try {
        if (pending != null && pending.future != null) {
          Outcome outcome = outcomeOf(pending.future);
          if (outcome == Outcome.DONE && !pending.lost) {
            commit(pending.record);
          }
        }
      } finally {
        pending = null;
        worker.shutdown();
        try {
          consumer.close();
        } catch (KafkaException e) {
          LOG.log(System.Logger.Level.WARNING, "veld kafka: closing the consumer of group {0}: {1}", group, e);
        }
        synchronized (lock) {
          running.remove(this);
        }
      }
    }

    @Override
    public void close() {
      stopping = true;
      if (Thread.currentThread() == thread) {
        return;
      }
      try {
        thread.join();
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
      }
    }
  }
}
