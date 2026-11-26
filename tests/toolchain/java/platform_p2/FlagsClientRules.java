package veldtest;

import gen.c.flags_rules.flags_rules.Stay;
import gen.c.runtime.Flags;
import java.time.Instant;

/**
 * A client has no provider (spec/platform.md section 11): its check of a value skips a flagged
 * invariant, and the invariant without a flag still applies; with the answers of a function a flagged
 * invariant is evaluated. Prints {@code OK}.
 */
public final class FlagsClientRules {
  private FlagsClientRules() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) {
    Instant start = Instant.parse("2026-01-01T00:00:00Z");
    Stay tooLong = new Stay(start, Instant.parse("2026-03-01T00:00:00Z"), 9);
    check(Stay.check(tooLong).isEmpty(), "outside a request the flagged invariants are not evaluated: " + Stay.check(tooLong));
    check(Flags.withAnswers(name -> name.equals("shortStays"), () -> Stay.check(tooLong)).size() == 1, "with the answers of a function they are");
    Stay nobody = new Stay(start, start.plusSeconds(60), 0);
    check(Stay.check(nobody).size() == 1, "an invariant without a flag always applies");
    check(Stay.check(nobody).get(0).message().contains("positive"), "the one without a flag: " + Stay.check(nobody));
    System.out.println("OK");
  }
}
