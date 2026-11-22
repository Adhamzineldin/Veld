package veldtest;

import java.util.Objects;

/** Minimal assertions that do not need {@code -ea}. */
final class Check {
  private Check() {}

  /** Something that may throw. */
  @FunctionalInterface
  interface Call {
    void run() throws Exception;
  }

  static void that(boolean condition, String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  static void equal(Object expected, Object actual, String message) {
    if (!Objects.equals(expected, actual)) {
      throw new AssertionError(message + ": expected <" + expected + "> but was <" + actual + ">");
    }
  }

  /** Runs {@code call}, which must throw a {@code type}; returns it. */
  static <E extends Throwable> E thrown(Class<E> type, String message, Call call) {
    try {
      call.run();
    } catch (Throwable t) {
      if (type.isInstance(t)) {
        return type.cast(t);
      }
      throw new AssertionError(message + ": expected " + type.getName() + " but got " + t, t);
    }
    throw new AssertionError(message + ": expected " + type.getName() + " but nothing was thrown");
  }
}
