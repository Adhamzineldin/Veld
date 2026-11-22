package dev.veld.runtime;

import java.util.function.Consumer;

/**
 * Options of one Server-Sent Events call. Without a {@link Reconnect} policy (here or on the client
 * options) there is no automatic reconnect: to resume, call again with the id of the last event
 * received.
 *
 * @param lastEventId sent as {@code Last-Event-ID}, or {@code null}
 * @param onEventId called with the {@code id} of every event that has one, or {@code null}
 * @param reconnect reconnect policy overriding the client's, or {@code null}
 */
public record SseOptions(String lastEventId, Consumer<String> onEventId, Reconnect reconnect) {

  /** Without a reconnect policy of its own. */
  public SseOptions(String lastEventId, Consumer<String> onEventId) {
    this(lastEventId, onEventId, null);
  }

  /** No options. */
  public static SseOptions none() {
    return new SseOptions(null, null);
  }

  /** Resume after {@code id}. */
  public static SseOptions resumeAfter(String id) {
    return new SseOptions(id, null);
  }

  /** The same options with an event id callback. */
  public SseOptions withOnEventId(Consumer<String> callback) {
    return new SseOptions(lastEventId, callback, reconnect);
  }

  /** The same options with a reconnect policy: connection failures resume with {@code Last-Event-ID}. */
  public SseOptions withReconnect(Reconnect policy) {
    return new SseOptions(lastEventId, onEventId, policy);
  }
}
