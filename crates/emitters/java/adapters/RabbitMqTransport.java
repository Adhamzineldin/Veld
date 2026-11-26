package dev.veld.adapters;

import com.rabbitmq.client.AMQP;
import com.rabbitmq.client.BuiltinExchangeType;
import com.rabbitmq.client.Channel;
import com.rabbitmq.client.Connection;
import com.rabbitmq.client.Delivery;
import dev.veld.runtime.Transport;
import java.io.IOException;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/**
 * {@link Transport} over RabbitMQ with AMQP 0-9-1 (spec/events.md section 10.7). This is the only
 * generated file written against {@code com.rabbitmq:amqp-client}; the application opens the
 * {@link Connection} (addresses, TLS, credentials, recovery) and closes it after this transport.
 *
 * <p>A topic is a durable {@code fanout} exchange, declared by the adapter. A group is the durable
 * queue {@code <topic>.<group>}, declared and bound by the adapter; a message published before the
 * group first subscribed is dropped, because no queue was bound. The key is the routing key (the
 * empty string when there is none). {@code publish} completes when the broker confirmed the message,
 * which is persistent. {@code @ordered} declares the queue with {@code x-single-active-consumer}:
 * the order of a queue holds while one consumer is active, and a second consumer of an ordered queue
 * is idle. {@code @deadLetter} declares the durable exchange and queue {@code <topic>.dlq}, bound to
 * each other. A queue that exists with other arguments is the broker's {@code PRECONDITION_FAILED},
 * which {@code subscribe} passes on.
 *
 * <p>A subscription has a channel of its own with a prefetch of 1. A message answered {@code AGAIN}
 * stays unacknowledged and the same consumer delivers it again after a delay ({@link
 * Options#redelivery}), taking nothing else meanwhile. The broker's delivery acknowledgement timeout
 * ({@code consumer_timeout}, 30 minutes by default) bounds how long that can last: after it the broker
 * closes the channel. {@code attempt} counts the deliveries by this consumer since it started.
 */
public final class RabbitMqTransport implements Transport, AutoCloseable {
  /** The media type of the structured CloudEvents JSON mode (spec/events.md section 6.2). */
  private static final String CONTENT_TYPE = "application/cloudevents+json; charset=UTF-8";

  private static final long CONFIRM_TIMEOUT_MS = 30000;

  /** The options of the adapter. */
  public static final class Options {
    private long redeliveryInitialMs = 1000;
    private long redeliveryMaxMs = 30000;

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
  }

  private final Connection connection;
  private final Options options;
  private final Object lock = new Object();
  private final Object publishLock = new Object();
  private final Set<String> declared = new HashSet<>();
  private final Set<Running> running = new LinkedHashSet<>();
  private Channel publisher;
  private boolean closed;

  /** Builds the adapter on an open connection that the application made. */
  public RabbitMqTransport(Connection connection) {
    this(connection, new Options());
  }

  /** Builds the adapter on an open connection that the application made, with its own options. */
  public RabbitMqTransport(Connection connection, Options options) {
    this.connection = connection;
    this.options = options;
  }

  private void ensureOpen() {
    synchronized (lock) {
      if (closed) {
        throw new IllegalStateException("veld rabbitmq: the transport is closed");
      }
    }
  }

  // -- publishing -----------------------------------------------------------------------------

  @Override
  public void publish(String topic, String key, byte[] bytes) {
    ensureOpen();
    synchronized (publishLock) {
      try {
        declare(topic);
        Channel channel = publishChannel();
        AMQP.BasicProperties properties =
            new AMQP.BasicProperties.Builder().contentType(CONTENT_TYPE).deliveryMode(2).build();
        channel.basicPublish(topic, key == null ? "" : key, properties, bytes);
        channel.waitForConfirmsOrDie(CONFIRM_TIMEOUT_MS);
      } catch (IOException | TimeoutException e) {
        throw new IllegalStateException("veld rabbitmq: publishing to " + topic + " failed: " + e, e);
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        throw new IllegalStateException("veld rabbitmq: interrupted while publishing to " + topic, e);
      }
    }
  }

  /**
   * Declares the exchange of a topic, once, on a channel of its own: a refused declaration closes
   * its channel, and the channel that publishes must survive it.
   */
  private void declare(String topic) throws IOException {
    if (declared.contains(topic)) {
      return;
    }
    try (Channel channel = connection.createChannel()) {
      channel.exchangeDeclare(topic, BuiltinExchangeType.FANOUT, true);
    } catch (TimeoutException e) {
      throw new IOException("closing the channel that declared " + topic, e);
    }
    declared.add(topic);
  }

  private Channel publishChannel() throws IOException {
    if (publisher == null || !publisher.isOpen()) {
      publisher = connection.createChannel();
      publisher.confirmSelect();
    }
    return publisher;
  }

  // -- consuming ------------------------------------------------------------------------------

  @Override
  public Subscription subscribe(String topic, String group, Deliver deliver) {
    return subscribe(topic, group, deliver, ChannelOptions.NONE);
  }

  @Override
  public Subscription subscribe(String topic, String group, Deliver deliver, ChannelOptions channel) {
    ensureOpen();
    Running subscription = new Running(deliver);
    try {
      subscription.open(topic, group, channel == null ? ChannelOptions.NONE : channel);
    } catch (IOException e) {
      subscription.close();
      throw new IllegalStateException("veld rabbitmq: subscribing to " + topic + " as " + group + " failed: " + e, e);
    }
    synchronized (lock) {
      running.add(subscription);
    }
    return subscription;
  }

  /**
   * Stops every subscription (a delivery that runs is waited for; one that waits for its next try
   * stops, and its message stays unacknowledged for the next consumer) and closes the channel that
   * publishes. The connection is the application's: it closes it.
   */
  @Override
  public void close() {
    List<Running> all;
    synchronized (lock) {
      closed = true;
      all = new ArrayList<>(running);
      running.clear();
    }
    for (Running r : all) {
      r.close();
    }
    synchronized (publishLock) {
      closeQuietly(publisher);
      publisher = null;
    }
  }

  private static void closeQuietly(Channel channel) {
    if (channel == null) {
      return;
    }
    try {
      if (channel.isOpen()) {
        channel.close();
      }
    } catch (IOException | TimeoutException | RuntimeException ignored) {
      // The channel is gone already.
    }
  }

  /** One subscription: a channel of its own and the delivery that runs on it. */
  private final class Running implements Subscription {
    private final Deliver deliver;
    private final CountDownLatch stop = new CountDownLatch(1);
    private Channel channel;
    private String tag;
    private boolean delivering;

    Running(Deliver deliver) {
      this.deliver = deliver;
    }

    /** Declares what the matrix of spec/events.md section 10.1 lists, then starts consuming. */
    void open(String topic, String group, ChannelOptions what) throws IOException {
      channel = connection.createChannel();
      channel.basicQos(1);
      channel.exchangeDeclare(topic, BuiltinExchangeType.FANOUT, true);
      String queue = topic + "." + group;
      Map<String, Object> arguments = what.ordered() ? Map.of("x-single-active-consumer", true) : null;
      channel.queueDeclare(queue, true, false, false, arguments);
      channel.queueBind(queue, topic, "");
      if (what.deadLetter()) {
        String dead = topic + ".dlq";
        channel.exchangeDeclare(dead, BuiltinExchangeType.FANOUT, true);
        channel.queueDeclare(dead, true, false, false, null);
        channel.queueBind(dead, dead, "");
      }
      tag = channel.basicConsume(queue, false, (consumerTag, delivery) -> handle(delivery), consumerTag -> {});
    }

    private void handle(Delivery delivery) throws IOException {
      synchronized (this) {
        if (stop.getCount() == 0) {
          return;
        }
        delivering = true;
      }
      try {
        String routingKey = delivery.getEnvelope().getRoutingKey();
        String key = routingKey == null || routingKey.isEmpty() ? null : routingKey;
        long wait = options.redeliveryInitialMs;
        for (int attempt = 1; ; attempt++) {
          Outcome outcome;
          try {
            outcome = deliver.deliver(new Message(key, delivery.getBody(), attempt));
          } catch (RuntimeException | Error e) {
            outcome = Outcome.AGAIN;
          }
          if (outcome == Outcome.DONE) {
            channel.basicAck(delivery.getEnvelope().getDeliveryTag(), false);
            return;
          }
          if (!pause(wait)) {
            return;
          }
          wait = Math.min(options.redeliveryMaxMs, Math.max(1, wait) * 2);
        }
      } finally {
        synchronized (this) {
          delivering = false;
          notifyAll();
        }
      }
    }

    /** Waits for the next try; false when the subscription was closed meanwhile. */
    private boolean pause(long millis) {
      try {
        return !stop.await(millis, TimeUnit.MILLISECONDS);
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        return false;
      }
    }

    @Override
    public void close() {
      stop.countDown();
      Channel last = channel;
      if (last == null) {
        return;
      }
      try {
        if (tag != null && last.isOpen()) {
          last.basicCancel(tag);
        }
      } catch (IOException | RuntimeException ignored) {
        // The channel is closing anyway.
      }
      synchronized (this) {
        boolean waited = false;
        while (delivering && !waited) {
          try {
            wait();
          } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            waited = true;
          }
        }
      }
      closeQuietly(last);
      synchronized (lock) {
        running.remove(this);
      }
    }
  }
}
