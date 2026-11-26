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

  /** Subscribes {@code deliver} to {@code topic} as a member of the consumer group {@code group}. */
  Subscription subscribe(String topic, String group, Deliver deliver);
}
