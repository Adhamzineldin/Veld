package dev.veld.runtime;

/**
 * Reconnect with exponential backoff after a connection failure: a refused or broken connection,
 * an abnormal WebSocket close. A typed error, a normal end of the stream and a close by the caller
 * never reconnect. Off unless set on {@link ClientOptions.Builder#reconnect(Reconnect)} or
 * {@link SseOptions#withReconnect(Reconnect)}.
 *
 * @param initialMs the delay before the first retry
 * @param maxMs the delay doubles after every failed attempt up to this
 * @param retries consecutive failed attempts before the failure is thrown
 */
public record Reconnect(long initialMs, long maxMs, int retries) {

  /** One second doubling to thirty seconds, ten attempts. */
  public static Reconnect defaults() {
    return new Reconnect(1000, 30000, 10);
  }

  /** The delay before retry number {@code attempt} (0-based). */
  public long backoffMs(int attempt) {
    long delay = initialMs;
    for (int i = 0; i < attempt && delay < maxMs; i++) {
      delay = Math.min(maxMs, delay * 2);
    }
    return Math.min(delay, maxMs);
  }

  static void sleep(long ms) {
    try {
      Thread.sleep(ms);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new VeldTransportError("interrupted while waiting to reconnect", e);
    }
  }
}
