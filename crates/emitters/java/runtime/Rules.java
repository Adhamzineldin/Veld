package dev.veld.runtime;

import java.math.BigDecimal;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalTime;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.ToDoubleFunction;
import java.util.function.ToIntFunction;
import java.util.function.ToLongFunction;
import java.util.regex.Pattern;

/**
 * The value semantics of rule expressions (spec/rules.md sections 3.4 to 3.7), used by generated
 * check functions. Instants are epoch milliseconds, dates are epoch days, times are milliseconds
 * of the day and durations are milliseconds, all as {@code long}. Integer arithmetic is checked;
 * decimals use {@link BigDecimal} and compare numerically. The three evaluation errors are raised
 * as {@link RuleError}.
 */
public final class Rules {
  private Rules() {}

  /** The check of one declaration: its own rules on one object, in wire form. */
  @FunctionalInterface
  public interface Check {
    /**
     * Appends the violations of the object's own rules.
     *
     * @param json the object, validated and prepared
     * @param request whether the value is in request position
     * @param path the path of the object
     * @param out receives the violations
     */
    void run(Json json, boolean request, String path, List<Violation> out);
  }

  /** The test of one rule. */
  @FunctionalInterface
  public interface Test {
    /** Whether the rule holds; may raise {@link RuleError}. */
    boolean holds();
  }

  private static final long MS_PER_DAY = 86_400_000L;
  private static final Map<String, Pattern> PATTERNS = new ConcurrentHashMap<>();

  // -- running a rule -------------------------------------------------------------------------

  /**
   * Evaluates one rule and records a violation when it fails or cannot be evaluated.
   *
   * @param out receives the violation
   * @param id the rule id
   * @param name the rule name
   * @param path the path of the value that carries the rule
   * @param fields the wire names of the fields the rule reads
   * @param message the rule's message, or its default
   * @param test the rule's expression
   */
  public static void run(
      List<Violation> out, String id, String name, String path, List<String> fields, String message, Test test) {
    try {
      if (!test.holds()) {
        out.add(new Violation(id, path, fields, message, null));
      }
    } catch (RuleError e) {
      out.add(new Violation(id, path, fields, "rule " + name + " could not be evaluated", e.kind()));
    }
  }

  // -- int ------------------------------------------------------------------------------------

  /** Checked {@code a + b}. */
  public static int addInt(int a, int b) {
    try {
      return Math.addExact(a, b);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  /** Checked {@code a - b}. */
  public static int subInt(int a, int b) {
    try {
      return Math.subtractExact(a, b);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  /** Checked {@code a * b}. */
  public static int mulInt(int a, int b) {
    try {
      return Math.multiplyExact(a, b);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  /** Truncating {@code a / b}; the smallest value divided by -1 overflows. */
  public static int divInt(int a, int b) {
    if (b == 0) {
      throw RuleError.divisionByZero();
    }
    if (a == Integer.MIN_VALUE && b == -1) {
      throw RuleError.overflow();
    }
    return a / b;
  }

  /** {@code a % b} with the sign of the dividend; the smallest value {@code % -1} is 0. */
  public static int remInt(int a, int b) {
    if (b == 0) {
      throw RuleError.divisionByZero();
    }
    return b == -1 ? 0 : a % b;
  }

  /** Checked {@code -a}. */
  public static int negInt(int a) {
    try {
      return Math.negateExact(a);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  // -- long (also instants, dates, times and durations) ---------------------------------------

  /** Checked {@code a + b}. */
  public static long addLong(long a, long b) {
    try {
      return Math.addExact(a, b);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  /** Checked {@code a - b}. */
  public static long subLong(long a, long b) {
    try {
      return Math.subtractExact(a, b);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  /** Checked {@code a * b}. */
  public static long mulLong(long a, long b) {
    try {
      return Math.multiplyExact(a, b);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  /** Truncating {@code a / b}; the smallest value divided by -1 overflows. */
  public static long divLong(long a, long b) {
    if (b == 0) {
      throw RuleError.divisionByZero();
    }
    if (a == Long.MIN_VALUE && b == -1) {
      throw RuleError.overflow();
    }
    return a / b;
  }

  /** {@code a % b} with the sign of the dividend; the smallest value {@code % -1} is 0. */
  public static long remLong(long a, long b) {
    if (b == 0) {
      throw RuleError.divisionByZero();
    }
    return b == -1 ? 0 : a % b;
  }

  /** Checked {@code -a}. */
  public static long negLong(long a) {
    try {
      return Math.negateExact(a);
    } catch (ArithmeticException e) {
      throw RuleError.overflow();
    }
  }

  // -- float ----------------------------------------------------------------------------------

  /** A float result must be finite. */
  public static double finite(double x) {
    if (!Double.isFinite(x)) {
      throw RuleError.nonFinite();
    }
    return x;
  }

  /** {@code a + b}, finite. */
  public static double addFloat(double a, double b) {
    return finite(a + b);
  }

  /** {@code a - b}, finite. */
  public static double subFloat(double a, double b) {
    return finite(a - b);
  }

  /** {@code a * b}, finite. */
  public static double mulFloat(double a, double b) {
    return finite(a * b);
  }

  /** {@code a / b}; the zero divisor is checked before dividing. */
  public static double divFloat(double a, double b) {
    if (b == 0.0) {
      throw RuleError.divisionByZero();
    }
    return finite(a / b);
  }

  /** {@code a % b} with the sign of the dividend; the zero divisor is checked first. */
  public static double remFloat(double a, double b) {
    if (b == 0.0) {
      throw RuleError.divisionByZero();
    }
    return finite(a % b);
  }

  // -- decimal --------------------------------------------------------------------------------

  /** Whether {@code x} is one of {@code list}, comparing numerically ({@code 1.50 in [1.5]}). */
  public static boolean decIn(List<BigDecimal> list, BigDecimal x) {
    for (BigDecimal item : list) {
      if (item.compareTo(x) == 0) {
        return true;
      }
    }
    return false;
  }

  /** Whether {@code x} is one of {@code list}, comparing as numbers ({@code 0.0 in [-0.0]}). */
  public static boolean inDouble(List<Double> list, double x) {
    for (double item : list) {
      if (item == x) {
        return true;
      }
    }
    return false;
  }

  /**
   * Whether {@code x}, an instant, date or time in the units of {@link #toDatetime}, {@link
   * #toDate} or {@link #toTime}, is one of {@code list}. The elements are {@link Instant}s,
   * {@link LocalDate}s or {@link LocalTime}s (a list field) or already {@code long}s (a literal).
   */
  public static boolean inTemporal(List<?> list, long x) {
    for (Object item : list) {
      long v;
      if (item instanceof Instant at) {
        v = toDatetime(at);
      } else if (item instanceof LocalDate day) {
        v = toDate(day);
      } else if (item instanceof LocalTime time) {
        v = toTime(time);
      } else {
        v = ((Number) item).longValue();
      }
      if (v == x) {
        return true;
      }
    }
    return false;
  }

  /** Whether {@code x} is one of {@code list}, by {@code equals}. */
  public static boolean in(List<?> list, Object x) {
    return list.contains(x);
  }

  // -- instants, dates, times, durations ------------------------------------------------------

  /** The instant as epoch milliseconds; digits below the millisecond are dropped. */
  public static long toDatetime(Instant at) {
    return at.toEpochMilli();
  }

  /** The same as {@link #toDatetime}; the name the generated check functions of the first release call. */
  public static long instantMs(Instant at) {
    return toDatetime(at);
  }

  /** The date as days since the epoch. */
  public static long toDate(LocalDate date) {
    return date.toEpochDay();
  }

  /** The time as milliseconds since midnight; digits below the millisecond are dropped. */
  public static long toTime(LocalTime time) {
    return time.toNanoOfDay() / 1_000_000L;
  }

  /** {@code a - b} for two dates, as a duration in milliseconds. */
  public static long dateDiff(long a, long b) {
    return mulLong(subLong(a, b), MS_PER_DAY);
  }

  /** {@code days(n)} in milliseconds. */
  public static long days(long n) {
    return mulLong(n, MS_PER_DAY);
  }

  /** {@code hours(n)} in milliseconds. */
  public static long hours(long n) {
    return mulLong(n, 3_600_000L);
  }

  /** {@code minutes(n)} in milliseconds. */
  public static long minutes(long n) {
    return mulLong(n, 60_000L);
  }

  /** {@code seconds(n)} in milliseconds. */
  public static long seconds(long n) {
    return mulLong(n, 1_000L);
  }

  /** {@code ms(n)}: a count of milliseconds is already a duration. */
  public static long ms(long n) {
    return n;
  }

  // -- strings --------------------------------------------------------------------------------

  /** The number of Unicode code points (the meaning of {@code @length} and {@code len}). */
  public static int lenString(String s) {
    return s.codePointCount(0, s.length());
  }

  /** The same as {@link #lenString}; the name the generated check functions of the first release call. */
  public static int len(String s) {
    return lenString(s);
  }

  /** {@code matches(s, re)}: a match anywhere unless anchored, as {@code @pattern} does. */
  public static boolean matches(String s, String regex) {
    return PATTERNS.computeIfAbsent(regex, Pattern::compile).matcher(s).find();
  }

  // -- comprehensions -------------------------------------------------------------------------

  /** True for an empty list; stops at the first false. */
  public static <T> boolean all(List<T> list, Predicate<? super T> test) {
    for (T x : list) {
      if (!test.test(x)) {
        return false;
      }
    }
    return true;
  }

  /** False for an empty list; stops at the first true. */
  public static <T> boolean any(List<T> list, Predicate<? super T> test) {
    for (T x : list) {
      if (test.test(x)) {
        return true;
      }
    }
    return false;
  }

  /** The number of elements for which {@code test} holds, checked. */
  public static <T> int countWhere(List<T> list, Predicate<? super T> test) {
    int n = 0;
    for (T x : list) {
      if (test.test(x)) {
        n = addInt(n, 1);
      }
    }
    return n;
  }

  /** Checked sum of {@code int} values; zero for an empty list. */
  public static <T> int sumInt(List<T> list, ToIntFunction<? super T> f) {
    int acc = 0;
    for (T x : list) {
      acc = addInt(acc, f.applyAsInt(x));
    }
    return acc;
  }

  /** Checked sum of {@code long} values; zero for an empty list. */
  public static <T> long sumLong(List<T> list, ToLongFunction<? super T> f) {
    long acc = 0;
    for (T x : list) {
      acc = addLong(acc, f.applyAsLong(x));
    }
    return acc;
  }

  /** Sum of {@code float} values, finite after every step; zero for an empty list. */
  public static <T> double sumFloat(List<T> list, ToDoubleFunction<? super T> f) {
    double acc = 0.0;
    for (T x : list) {
      acc = addFloat(acc, f.applyAsDouble(x));
    }
    return acc;
  }

  /** Exact sum of decimals; zero for an empty list. */
  public static <T> BigDecimal sumDecimal(List<T> list, Function<? super T, BigDecimal> f) {
    BigDecimal acc = BigDecimal.ZERO;
    for (T x : list) {
      acc = acc.add(f.apply(x));
    }
    return acc;
  }
}
