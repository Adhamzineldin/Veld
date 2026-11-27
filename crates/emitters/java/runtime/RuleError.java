package dev.veld.runtime;

/**
 * An evaluation error of a rule (spec/rules.md section 3.7): integer or duration overflow, division
 * by zero, or a float result that is not finite. The generated check function catches it per rule
 * and records a violation that carries {@link #kind()}.
 */
public final class RuleError extends RuntimeException {
  private static final long serialVersionUID = 1L;

  /** The kind of an overflow. */
  public static final String OVERFLOW = "overflow";
  /** The kind of a division by zero. */
  public static final String DIVISION_BY_ZERO = "division_by_zero";
  /** The kind of a float result that is not finite. */
  public static final String NON_FINITE = "non_finite";

  private final String kind;

  private RuleError(String kind) {
    super(kind, null, false, false);
    this.kind = kind;
  }

  /** An overflow. */
  static RuleError overflow() {
    return new RuleError(OVERFLOW);
  }

  /** A division by zero. */
  static RuleError divisionByZero() {
    return new RuleError(DIVISION_BY_ZERO);
  }

  /** A float result that is not finite. */
  static RuleError nonFinite() {
    return new RuleError(NON_FINITE);
  }

  /** {@code overflow}, {@code division_by_zero} or {@code non_finite}. */
  public String kind() {
    return kind;
  }
}
