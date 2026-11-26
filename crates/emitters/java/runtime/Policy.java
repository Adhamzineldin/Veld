package dev.veld.runtime;

import java.util.function.BooleanSupplier;

/**
 * What the generated policy methods are built from (spec/rules.md section 11.2): a policy permits
 * when at least one {@code permit} is true and no {@code forbid} is true, and a clause that cannot
 * be evaluated fails closed: such a {@code permit} does not permit and such a {@code forbid}
 * forbids.
 */
public final class Policy {
  private Policy() {}

  /** The text of a denial that no {@code forbid} explains. */
  public static final String NOT_PERMITTED = "not permitted";

  // #if messages_server
  /**
   * What a policy decided.
   *
   * @param permitted whether the caller may go on
   * @param message the message of the first {@code forbid} that was true, when it has one; {@code
   *     null} otherwise
   * @param key the message key of that {@code forbid} (spec/platform.md section 22), when it has a
   *     message; {@code null} otherwise (the denial is {@code veld/not-permitted})
   */
  public record Verdict(boolean permitted, String message, String key) {
    /** The verdict of a policy that permits. */
    public static final Verdict PERMITTED = new Verdict(true, null, null);

    /** A policy that denies; {@code message} is the message of the {@code forbid} that was true, or {@code null}. */
    public static Verdict denied(String message) {
      return new Verdict(false, message, null);
    }

    /** A policy that denies with the message of a {@code forbid} that has the key {@code key} in a catalog. */
    public static Verdict denied(String message, String key) {
      return new Verdict(false, message, key);
    }

    /** {@link #PERMITTED}, or a denial without a message. */
    public static Verdict of(boolean permitted) {
      return permitted ? PERMITTED : new Verdict(false, null, null);
    }

    /** The text of a denial: the message of the {@code forbid}, else {@code not permitted}. */
    public String denial() {
      return message == null ? NOT_PERMITTED : message;
    }
  }
  // #endif
  // #if not_messages_server
  /**
   * What a policy decided.
   *
   * @param permitted whether the caller may go on
   * @param message the message of the first {@code forbid} that was true, when it has one; {@code
   *     null} otherwise
   */
  public record Verdict(boolean permitted, String message) {
    /** The verdict of a policy that permits. */
    public static final Verdict PERMITTED = new Verdict(true, null);

    /** A policy that denies; {@code message} is the message of the {@code forbid} that was true, or {@code null}. */
    public static Verdict denied(String message) {
      return new Verdict(false, message);
    }

    /** {@link #PERMITTED}, or a denial without a message. */
    public static Verdict of(boolean permitted) {
      return permitted ? PERMITTED : new Verdict(false, null);
    }

    /** The text of a denial: the message of the {@code forbid}, else {@code not permitted}. */
    public String denial() {
      return message == null ? NOT_PERMITTED : message;
    }
  }
  // #endif

  /** Whether a {@code permit} clause holds; a clause that cannot be evaluated does not. */
  public static boolean permits(BooleanSupplier clause) {
    try {
      return clause.getAsBoolean();
    } catch (RuleError e) {
      return false;
    }
  }

  /** Whether a {@code forbid} clause holds; a clause that cannot be evaluated does. */
  public static boolean forbids(BooleanSupplier clause) {
    try {
      return clause.getAsBoolean();
    } catch (RuleError e) {
      return true;
    }
  }
}
