package veldtest;

import gen.b.retention.Retention;
import gen.b.retention.RetentionEntry;
import gen.b.retention.RetentionJobs;
import gen.b.retention.RetentionOptions;
import gen.b.retention.RetentionResult;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;

/**
 * Retention (spec/platform.md section 23) in a generated server: {@code Retention.retentionPlan()},
 * and {@code Retention.runApiRetention} with jobs that record what they are asked. Expects the server
 * in {@code gen.b} generated from tests/toolchain/contract/platform_p4/retention.veld (compiled as
 * the package {@code bookings}, target {@code api}). Writes {@code plan.json} for the Rust test,
 * which compares it with {@code Ir::retention_plan}. Prints {@code OK}.
 */
public final class RetentionRunner {
  private RetentionRunner() {}

  private static final long DAY = 86_400_000L;
  private static final Instant NOW = Instant.parse("2026-06-10T12:00:00.000Z");

  /** One call a job heard. */
  private record Call(String job, String model, Instant before, List<String> fields) {}

  /** Jobs that record what they are asked; {@code failDelete} and {@code failAnonymize} make a job throw. */
  private static final class Recording implements RetentionJobs {
    final List<Call> calls = new ArrayList<>();
    boolean failDelete;
    boolean failAnonymize;

    @Override
    public long delete(String model, Instant before) throws Exception {
      if (failDelete) {
        throw new IllegalStateException("the delete failed");
      }
      calls.add(new Call("delete", model, before, null));
      return 3;
    }

    @Override
    public long anonymize(String model, Instant before, List<String> fields) throws Exception {
      if (failAnonymize) {
        throw new java.io.IOException("the anonymize failed");
      }
      calls.add(new Call("anonymize", model, before, fields));
      return 2;
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static void same(Object got, Object expected, String what) {
    if (!java.util.Objects.equals(got, expected)) {
      throw new AssertionError(what + "\n  got      " + got + "\n  expected " + expected);
    }
  }

  private static String last(String model) {
    return model.substring(model.lastIndexOf('.') + 1);
  }

  private static Instant at(long days) {
    return NOW.minusMillis(days * DAY);
  }

  private static String quoted(String text) {
    return "\"" + text + "\"";
  }

  /** The plan as JSON, the members the Rust test compares with {@code Ir::retention_plan}. */
  private static String planJson(List<RetentionEntry> plan) {
    List<String> entries = new ArrayList<>();
    for (RetentionEntry e : plan) {
      List<String> fields = new ArrayList<>();
      e.fields().forEach(f -> fields.add(quoted(f)));
      entries.add("{\"model\":" + quoted(e.model()) + ",\"afterMs\":" + e.afterMs() + ",\"since\":" + quoted(e.since())
          + ",\"then\":" + quoted(e.then().word()) + ",\"fields\":[" + String.join(",", fields) + "]}");
    }
    return "[" + String.join(",", entries) + "]";
  }

  public static void main(String[] args) throws Exception {
    // The plan: one entry per model with @retain, sorted by name, with what each needs.
    List<RetentionEntry> plan = Retention.retentionPlan();
    Files.writeString(Path.of("plan.json"), planJson(plan), StandardCharsets.UTF_8);
    List<String> names = new ArrayList<>();
    plan.forEach(e -> names.add(e.model()));
    List<String> sorted = new ArrayList<>(names);
    java.util.Collections.sort(sorted);
    same(names, sorted, "sorted by the qualified name");
    List<String> shape = new ArrayList<>();
    plan.forEach(e -> shape.add(last(e.model()) + " " + e.afterMs() / DAY + " " + e.since() + " " + e.then().word() + " " + e.fields()));
    same(
        shape,
        List.of(
            "Guest 30 checkedOutAt anonymize [name, phone]",
            "Invoice 365 closedOn delete []",
            "Staff 90 leftAt anonymize [name, badge]"),
        "the plan");
    check(names.stream().noneMatch(n -> n.endsWith(".Kept")), "a model without @retain is not in the plan");

    // One call per entry, in order, with before = now - afterMs, and the fields of an anonymize.
    Recording jobs = new Recording();
    List<RetentionResult> report = Retention.runApiRetention(jobs, new RetentionOptions(NOW));
    List<String> calls = new ArrayList<>();
    jobs.calls.forEach(c -> calls.add(c.job() + " " + last(c.model()) + " " + c.before() + " " + c.fields()));
    same(
        calls,
        List.of(
            "anonymize Guest " + at(30) + " [name, phone]",
            "delete Invoice " + at(365) + " null",
            "anonymize Staff " + at(90) + " [name, badge]"),
        "the calls");
    // The report: what each entry changed, in order.
    List<String> reported = new ArrayList<>();
    report.forEach(r -> reported.add(last(r.model()) + " " + r.then().word() + " " + r.before() + " " + r.changed() + " " + (r.error() != null)));
    same(
        reported,
        List.of(
            "Guest anonymize " + at(30) + " 2 false",
            "Invoice delete " + at(365) + " 3 false",
            "Staff anonymize " + at(90) + " 2 false"),
        "the report");

    // The clock is the system's when no now is given.
    jobs = new Recording();
    Instant started = Instant.now();
    Retention.runApiRetention(jobs);
    Instant finished = Instant.now();
    for (int i = 0; i < jobs.calls.size(); i++) {
      long afterMs = plan.get(i).afterMs();
      Instant before = jobs.calls.get(i).before();
      check(
          !before.isBefore(started.minusMillis(afterMs)) && !before.isAfter(finished.minusMillis(afterMs)),
          jobs.calls.get(i).model() + " is due before now - afterMs");
    }

    // An entry that fails does not stop the others, and is in the report: a delete that throws and
    // an anonymize that throws.
    jobs = new Recording();
    jobs.failDelete = true;
    report = Retention.runApiRetention(jobs, new RetentionOptions(NOW));
    same(jobs.calls.size(), 2, "the other entries ran");
    same(List.of(report.get(0).changed(), report.get(1).changed(), report.get(2).changed()), List.of(2L, 0L, 2L), "the counts");
    check(report.get(1).error() instanceof IllegalStateException e && e.getMessage().equals("the delete failed"), "the failure is in the report");
    check(report.get(0).error() == null, "the first entry did not fail");
    jobs = new Recording();
    jobs.failAnonymize = true;
    report = Retention.runApiRetention(jobs, new RetentionOptions(NOW));
    same(List.of(report.get(0).changed(), report.get(1).changed(), report.get(2).changed()), List.of(0L, 3L, 0L), "the counts");
    check(report.get(0).error() instanceof java.io.IOException && report.get(1).error() == null && report.get(2).error() != null,
        "an anonymize that throws, a delete between the failures");
    same(jobs.calls.size(), 1, "the delete entry ran between the failures");

    // A retention longer than the clock can count back from now is in the report, not an exception.
    check(Retention.runApiRetention(new Recording(), new RetentionOptions(Instant.MIN)).stream().allMatch(r -> r.error() != null),
        "a before that cannot be made fails the entry");
    System.out.println("OK");
  }
}
