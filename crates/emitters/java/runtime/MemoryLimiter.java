package dev.veld.runtime;

import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import java.util.function.LongSupplier;

/**
 * The limiter a server uses when the application gives none (spec/platform.md section 5): a fixed
 * window per key that starts with the first call of the key that finds no open window. A window is
 * open from its start up to, not including, its start plus its length. A refused call does not count,
 * and the time left is rounded up to whole seconds, at least 1. It counts per process. The clock is
 * the system clock unless one is given, so that a test does not wait.
 */
public final class MemoryLimiter implements Limiter {
  /** After this many windows, the ones that ended more than a day ago are dropped. */
  private static final int SWEEP_ABOVE = 10_000;

  private static final long DAY_MS = 86_400_000L;

  private record Window(long start, long length, long calls) {}

  private final Map<String, Window> windows = new HashMap<>();
  private final LongSupplier clock;

  /** A limiter on the system clock. */
  public MemoryLimiter() {
    this(System::currentTimeMillis);
  }

  /** A limiter on {@code clock}, which answers epoch milliseconds. */
  public MemoryLimiter(LongSupplier clock) {
    this.clock = clock;
  }

  @Override
  public synchronized Decision take(String key, long limit, long windowSeconds) {
    long now = clock.getAsLong();
    long length = saturatingMultiply(windowSeconds, 1000L);
    Window held = windows.get(key);
    // A window is open up to, not including, its start plus its length; a clock that went back is in it.
    Window window = held != null && now < saturatingAdd(held.start(), length)
        ? new Window(held.start(), length, held.calls())
        : new Window(now, length, 0);
    Decision decision;
    if (window.calls() < limit) {
      window = new Window(window.start(), length, window.calls() + 1);
      decision = Decision.allow();
    } else {
      long left = saturatingAdd(window.start(), length) - now;
      long clamped = Math.min(Math.max(left, 1), Math.max(length, 1));
      decision = Decision.refuse((clamped + 999) / 1000);
    }
    windows.put(key, window);
    if (windows.size() > SWEEP_ABOVE) {
      sweep(now);
    }
    return decision;
  }

  private void sweep(long now) {
    Iterator<Window> it = windows.values().iterator();
    while (it.hasNext()) {
      Window w = it.next();
      if (saturatingAdd(w.start(), w.length()) < now - DAY_MS) {
        it.remove();
      }
    }
  }

  private static long saturatingAdd(long a, long b) {
    long sum = a + b;
    return ((a ^ sum) & (b ^ sum)) < 0 ? (a < 0 ? Long.MIN_VALUE : Long.MAX_VALUE) : sum;
  }

  private static long saturatingMultiply(long a, long b) {
    long high = Math.multiplyHigh(a, b);
    long low = a * b;
    if ((high == 0 && low >= 0) || (high == -1 && low < 0)) {
      return low;
    }
    return (a < 0) == (b < 0) ? Long.MAX_VALUE : Long.MIN_VALUE;
  }
}
