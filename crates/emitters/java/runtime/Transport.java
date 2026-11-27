package dev.veld.runtime;

/**
 * What the generated publishers and subscribers depend on (spec/events.md section 9). A broker
 * adapter implements it; {@link MemoryTransport} is the one that ships with the generated code.
 *
 * <p>The methods are blocking and throw no checked exception: an adapter wraps what its client
 * throws in an unchecked exception.
 */
public interface Transport {

  /** What a subscriber answers for a message: {@code AGAIN} only when its handler failed. */
  enum Outcome {
    /** The message is acknowledged: handled, ignored or rejected. */
    DONE,
    /** The handler failed: the message is not acknowledged and is delivered again. */
    AGAIN
  }

  /**
   * One message as a transport hands it to a subscriber.
   *
   * @param key the key text of the message, or {@code null} when the channel has no key
   * @param bytes the bytes the publisher sent
   * @param attempt the delivery attempt, 1 for the first
   */
  record Message(String key, byte[] bytes, int attempt) {}

  /**
   * What a channel asks of a transport (spec/events.md section 10.6): {@code @ordered} and
   * {@code @deadLetter}. A transport that cannot give {@code ordered} must fail instead of ignoring it.
   *
   * @param ordered one total order across keys
   * @param deadLetter the channel has a dead-letter topic, {@code <topic>.dlq}
   */
  record ChannelOptions(boolean ordered, boolean deadLetter) {
    /** A channel with neither annotation. */
    public static final ChannelOptions NONE = new ChannelOptions(false, false);
  }

  /** The function a transport calls for every message of a subscription. */
  @FunctionalInterface
  interface Deliver {
    /** Handles one message and says whether it is done with it. */
    Outcome deliver(Message message);
  }

  /** The handle {@link #subscribe} returns: {@code close()} stops the subscription. */
  @FunctionalInterface
  interface Subscription extends AutoCloseable {
    /** Stops the subscription. A message that is delivered right now still completes. */
    @Override
    void close();
  }

  /** Sends one message to {@code topic}; returns when the transport has accepted it. */
  void publish(String topic, String key, byte[] bytes);

  /**
   * Like {@link #publish(String, String, byte[])}, with the options of the channel. A transport
   * written before phase E2 does not override it: it gives no total order, so {@code ordered} fails
   * with an {@link UnsupportedOperationException} instead of being ignored.
   */
  default void publish(String topic, String key, byte[] bytes, ChannelOptions options) {
    if (options != null && options.ordered()) {
      throw new UnsupportedOperationException("this transport cannot give @ordered");
    }
    publish(topic, key, bytes);
  }

  /** Subscribes {@code deliver} to {@code topic} as a member of the consumer group {@code group}. */
  Subscription subscribe(String topic, String group, Deliver deliver);

  /** Like {@link #subscribe(String, String, Deliver)}, with the options of the channel (see {@link #publish(String, String, byte[], ChannelOptions)}). */
  default Subscription subscribe(String topic, String group, Deliver deliver, ChannelOptions options) {
    if (options != null && options.ordered()) {
      throw new UnsupportedOperationException("this transport cannot give @ordered");
    }
    return subscribe(topic, group, deliver);
  }
}
