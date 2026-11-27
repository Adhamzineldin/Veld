package dev.veld.runtime;

/**
 * What a server asks before it runs an endpoint that has a {@code @limit} (spec/platform.md section
 * 5). An implementation MUST allow at most {@code limit} calls of one key in any window it defines of
 * {@code windowSeconds}. {@link MemoryLimiter} is the generated one; an application with more than one
 * process gives its own (Redis, a gateway). A limiter that throws lets the call through, and
 * {@code onError} hears of it.
 */
@FunctionalInterface
public interface Limiter {

  /**
   * The answer of a limiter.
   *
   * @param allowed whether the call may go on
   * @param retryAfterSeconds for a refused call, the seconds to wait (at least 1); 0 for an allowed one
   */
  record Decision(boolean allowed, long retryAfterSeconds) {
    /** The call may go on. */
    public static Decision allow() {
      return new Decision(true, 0);
    }

    /** The call is refused: try again in {@code seconds}. */
    public static Decision refuse(long seconds) {
      return new Decision(false, Math.max(1, seconds));
    }
  }

  /**
   * Counts one call.
   *
   * @param key the operation signature, the subject ({@code user}, {@code tenant}, {@code ip}) and the
   *     identity, joined with {@code |}
   * @param limit how many calls one key may make in one window
   * @param windowSeconds the length of the window the endpoint states
   */
  Decision take(String key, long limit, long windowSeconds) throws Exception;
}
