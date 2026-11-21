package dev.veld.runtime;

import java.util.function.Consumer;

/**
 * Options of one Server-Sent Events call. There is no automatic reconnect: to resume, call again
 * with the id of the last event received.
 *
 * @param lastEventId sent as {@code Last-Event-ID}, or {@code null}
 * @param onEventId called with the {@code id} of every event that has one, or {@code null}
 */
public record SseOptions(String lastEventId, Consumer<String> onEventId) {

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
    return new SseOptions(lastEventId, callback);
  }
}
