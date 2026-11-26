package dev.veld.runtime;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.function.BooleanSupplier;
import java.util.function.Function;
import java.util.function.Supplier;

/**
 * Endpoint rules (spec/rules.md section 9): step 6 of the pipeline (facts, {@code require},
 * {@code old}) and step 9 ({@code ensure}). The generated {@code <Service>Handler} wraps every
 * endpoint that has rules in a {@link Run}; the router asks {@link #ensureFailure} for step 9
 * after it has checked the invariants of the result.
 */
public final class EndpointRules {
  private EndpointRules() {}

  private static final ThreadLocal<List<Check>> PENDING = new ThreadLocal<>();
  // #if rollout
  private static final ThreadLocal<List<Check>> PENDING_AUDITED = new ThreadLocal<>();
  // #endif

  /** Thrown to end a request with a response that is already built. */
  public static final class Early extends RuntimeException {
    private static final long serialVersionUID = 1L;
    private final transient VeldResponse response;

    /** Ends the request with {@code response}. */
    public Early(VeldResponse response) {
      super("the request ends with status " + response.status(), null, false, false);
      this.response = response;
    }

    /** The response. */
    public VeldResponse response() {
      return response;
    }
  }

  /** A value saved before the handler runs: the value, or the evaluation error that stopped it. */
  public static final class Saved<T> {
    private final T value;
    private final RuleError error;
    // #if derived
    private final boolean absent;
    // #endif

    private Saved(T value, RuleError error) {
      this.value = value;
      this.error = error;
      // #if derived
      this.absent = false;
      // #endif
    }
    // #if derived

    private Saved() {
      this.value = null;
      this.error = null;
      this.absent = true;
    }
    // #endif

    /** Evaluates {@code expr} now; an evaluation error is kept and raised when the value is read. */
    public static <T> Saved<T> of(Supplier<T> expr) {
      try {
        return new Saved<>(expr.get(), null);
      } catch (RuleError e) {
        return new Saved<>(null, e);
        // #if derived
      } catch (NullPointerException e) {
        // An operand that reads a derived field with no value is saved as absent: the statement
        // that reads it later is skipped (spec/rules.md section 17.2).
        return new Saved<>();
        // #endif
      }
    }

    /** A slot that was not evaluated (the rules are off for this request). */
    public static <T> Saved<T> skipped() {
      return new Saved<>(null, null);
    }

    /** The saved value.
     *
     * @throws RuleError when evaluating it failed
     */
    public T get() {
      if (error != null) {
        throw error;
      }
      // #if derived
      if (absent) {
        throw new Derive.Absent();
      }
      // #endif
      return value;
    }
  }

  /** One {@code ensure}, waiting for the result. */
  private record Check(String id, String name, List<String> fields, String message, BooleanSupplier holds) {}

  /** Starts step 6 of one request. */
  public static Run start(
      HandlerHooks hooks,
      EndpointSpec spec,
      Function<List<Issue>, VeldError> validation,
      Function<String, VeldError> notFound,
      long now) {
    return new Run(hooks, spec, validation, notFound, now);
  }

  /** Forgets the {@code ensure} checks of a request that ended without reaching step 9. */
  static void discard() {
    PENDING.remove();
    // #if rollout
    PENDING_AUDITED.remove();
    // #endif
  }

  /** The schema-prepared wire form of an error payload, defaults applied. */
  public static Json payload(Schema schema, Json.Obj data) {
    return Wire.prepare(schema, data, Wire.Mode.RESPONSE);
  }

  /** Step 6 and the registration of step 9 for one request. */
  public static final class Run {
    private final HandlerHooks hooks;
    private final EndpointSpec spec;
    private final Function<List<Issue>, VeldError> validation;
    private final Function<String, VeldError> notFound;
    private final long now;
    private final long started = System.nanoTime();
    private final List<Check> ensures = new ArrayList<>();
    // #if rollout
    private final List<Check> auditedEnsures = new ArrayList<>();
    // #endif

    private Run(
        HandlerHooks hooks,
        EndpointSpec spec,
        Function<List<Issue>, VeldError> validation,
        Function<String, VeldError> notFound,
        long now) {
      this.hooks = hooks;
      this.spec = spec;
      this.validation = validation;
      this.notFound = notFound;
      this.now = now;
    }

    /** The one reading of the clock of this request, in epoch milliseconds. */
    public long now() {
      return now;
    }

    /**
     * Whether the rules run: they do not when request validation is off, because they need
     * well-formed values (spec/rules.md section 5.3).
     */
    public boolean active() {
      return hooks.validateRequests();
    }

    /** A required fact: what the loader found, or {@code 404 NotFound} naming the fact. */
    public <T> T required(String fact, Optional<T> found) {
      if (found.isPresent()) {
        return found.get();
      }
      VeldError error = notFound.apply(fact);
      throw new Early(Router.problem(error.sig(), error.title(), 404, spec.sig(), error.dataWire()));
    }

    // #if platform_facts
    /**
     * A fact the application loaded, with its defaults applied and its derived fields computed, before
     * any policy: a tenant-scoped value of another tenant anywhere in it makes the fact not found
     * ({@code 404 NotFound} naming it, or absent for an optional one), never {@code 403}
     * (spec/platform.md section 3); what is kept is recorded for the audit event of the request.
     */
    public <T> T kept(RequestContext ctx, String name, Schema schema, Codec<T> codec, T value, boolean optional) {
      if (value == null) {
        return null;
      }
      Json wire = codec.encode(value);
      // #endif
      // #if tenancy
      if (!Platform.foreign(schema, wire, Platform.callerTenant(hooks, ctx), name).isEmpty()) {
        if (optional) {
          return null;
        }
        VeldError error = notFound.apply(name);
        throw new Early(Router.problem(error.sig(), error.title(), 404, spec.sig(), error.dataWire()));
      }
      // #endif
      // #if audit
      Platform.fact(ctx, name, wire);
      // #endif
      // #if platform_facts
      return value;
    }

    // #endif
    /**
     * One {@code require}. A failure ends the request: with {@code otherwise}, the declared error
     * it builds (only now), else {@code 422 ValidationError}; both carry the one violation.
     */
    public void require(
        String id,
        String name,
        List<String> fields,
        String message,
        BooleanSupplier holds,
        Supplier<VeldError> otherwise) {
      if (!active()) {
        return;
      }
      String error = null;
      boolean ok;
      try {
        ok = holds.getAsBoolean();
      } catch (RuleError e) {
        ok = false;
        error = e.kind();
        // #if derived
      } catch (NullPointerException e) {
        // The statement reads a derived field that is absent: it is skipped, the cause is reported.
        ok = true;
        // #endif
      }
      if (ok) {
        return;
      }
      String text = error == null ? message : "rule " + name + " could not be evaluated";
      Violation violation = new Violation(id, "request", fields, text, error);
      List<Violation> found = List.of(violation);
      RuleWalk.report(hooks, spec, RuleWalk.Position.INPUT, RuleWalk.Outcome.REJECTED, found, started);
      if (otherwise == null) {
        List<Issue> issues = List.of(new Issue("request", error == null ? "rule" : "rule_error", text));
        throw early(validation.apply(issues), 422, found);
      }
      VeldError built;
      try {
        built = otherwise.get();
      } catch (RuleError e) {
        Violation payload = new Violation(id, "request", fields, "rule " + name + " could not be evaluated", e.kind());
        RuleWalk.report(hooks, spec, RuleWalk.Position.OUTPUT, RuleWalk.Outcome.REJECTED, List.of(payload), started);
        throw new RuleWalk.Rejected();
      }
      Json data = Wire.prepare(built.dataSchema(), built.dataWire(), Wire.Mode.RESPONSE);
      // #if derived
      List<Violation> uncomputed = new ArrayList<>();
      data = Derive.compute(built.dataSchema(), data, "error", uncomputed);
      // #endif
      // #if rollout
      Audit.checkError(hooks, spec, built.dataSchema(), data, started);
      // #endif
      List<Violation> broken = RuleWalk.check(built.dataSchema(), data, "error", false);
      // #if derived
      broken.addAll(0, uncomputed);
      // #endif
      if (!broken.isEmpty()) {
        RuleWalk.report(hooks, spec, RuleWalk.Position.OUTPUT, RuleWalk.Outcome.REJECTED, broken, started);
        throw new RuleWalk.Rejected();
      }
      throw early(built, built.status(), found);
    }

    // #if derived
    /**
     * A value the application handed over, a fact or the principal, with its defaults applied and
     * its derived fields computed (spec/rules.md section 17.2). A derived field that cannot be
     * computed is the application's mistake, not the caller's: it is reported as an output
     * violation and the request ends with {@code 500 Internal}.
     */
    public <T> T derive(Schema schema, Codec<T> codec, T value) {
      List<Violation> found = new ArrayList<>();
      T done = Derive.value(schema, codec, value, found);
      if (!found.isEmpty()) {
        RuleWalk.report(hooks, spec, RuleWalk.Position.OUTPUT, RuleWalk.Outcome.REJECTED, found, started);
        throw new RuleWalk.Rejected();
      }
      return done;
    }

    // #endif
    private Early early(VeldError error, int status, List<Violation> found) {
      Json data = Wire.prepare(error.dataSchema(), error.dataWire(), Wire.Mode.RESPONSE);
      // #if derived
      data = Derive.compute(error.dataSchema(), data, "error", new ArrayList<>());
      // #endif
      return new Early(Router.problem(error.sig(), error.title(), status, spec.sig(), data, found));
    }

    // #if machine
    /**
     * The transition guard (spec/rules.md section 10.2): {@code from} must be one of {@code
     * sources}, else the request ends with {@code 409 InvalidTransition}. It runs whether or not
     * request validation is on: it reads a fact, never a request value. The result is what the
     * handler receives; it is a guard, not an atomic guarantee.
     */
    public <S extends Enum<S>> Transition<S> transition(
        String id, String field, S from, List<S> sources, S to, Supplier<VeldError> invalid) {
      if (sources.contains(from)) {
        return new Transition<>(field, from, to);
      }
      List<Violation> found = List.of(new Violation(id, "request", List.of(), "violates rule transition", null));
      RuleWalk.report(hooks, spec, RuleWalk.Position.INPUT, RuleWalk.Outcome.REJECTED, found, started);
      VeldError error = invalid.get();
      throw early(error, error.status(), found);
    }

    // #endif
    // #if outbox
    /**
     * The {@code emit} of this request (spec/events.md section 11.1): {@code edges} holds, by wire
     * value of each source state, the declaration names of the events the edge from it to the
     * target emits (a source whose edge emits nothing is left out); {@code from} is the wire value
     * the guard found. The events written are checked after the handler ({@link Emission#check}).
     */
    public Emission emission(
        Emission.Source source, String publisher, java.util.Map<String, List<String>> edges, String from) {
      return new Emission(source, publisher, validation, edges.getOrDefault(from, List.of()));
    }

    // #endif
    // #if timed
    /**
     * The guard of a timed edge (spec/platform.md section 10): the state check of {@link #transition}
     * and, after it, {@code now - since >= afterMs}, with the clock of the request. A value that
     * has waited exactly {@code afterMs} is due, one millisecond less is not, and neither is a
     * {@code since} in the future or one that cannot be read. The refusal is the same {@code 409
     * InvalidTransition}; its violation says {@code violates rule transition: not due}.
     */
    public <S extends Enum<S>> Transition<S> transitionDue(
        String id,
        String field,
        S from,
        List<S> sources,
        S to,
        long afterMs,
        java.util.function.LongSupplier since,
        Supplier<VeldError> invalid) {
      boolean state = sources.contains(from);
      if (state && due(afterMs, since)) {
        return new Transition<>(field, from, to);
      }
      String text = state ? "violates rule transition: not due" : "violates rule transition";
      List<Violation> found = List.of(new Violation(id, "request", List.of(), text, null));
      RuleWalk.report(hooks, spec, RuleWalk.Position.INPUT, RuleWalk.Outcome.REJECTED, found, started);
      VeldError error = invalid.get();
      throw early(error, error.status(), found);
    }

    private boolean due(long afterMs, java.util.function.LongSupplier since) {
      try {
        return Math.subtractExact(now, since.getAsLong()) >= afterMs;
      } catch (RuntimeException e) {
        return false;
      }
    }

    // #endif
    // #if policies
    /**
     * One {@code allow} (spec/rules.md section 11.3). When the policy denies, or an argument
     * cannot be evaluated (it fails closed), the request ends with {@code 403 Forbidden}: the
     * message is the one of the first {@code forbid} that was true, else {@code not permitted}.
     * It runs whether or not request validation is on, because it decides who may call.
     */
    public void allow(
        String id, List<String> fields, Supplier<Policy.Verdict> verdict, Function<String, VeldError> forbidden) {
      Policy.Verdict decided;
      try {
        decided = verdict.get();
      } catch (RuleError e) {
        decided = Policy.Verdict.denied(null);
      }
      if (decided.permitted()) {
        return;
      }
      String text = decided.denial();
      List<Violation> found = List.of(new Violation(id, "request", fields, text, null));
      RuleWalk.report(hooks, spec, RuleWalk.Position.INPUT, RuleWalk.Outcome.REJECTED, found, started);
      VeldError error = forbidden.apply(text);
      throw early(error, error.status(), found);
    }

    // #endif
    // #if conditional
    /**
     * The version guard (spec/rules.md section 11.9): when the request carried an {@code If-Match}
     * tag and the guarded fact is present, the tag must be the fact's current version, else the
     * request ends with {@code 412 PreconditionFailed} (the message names {@code expected}, the
     * caller's version, and {@code actual}). {@code *} and no header pass. The result is the version
     * that was checked, for the handler, or {@code null} when nothing was compared. It runs whether
     * or not request validation is on; like a transition it is a guard, not an atomic guarantee.
     */
    public Long version(
        RequestContext ctx,
        String id,
        Long actual,
        java.util.function.BiFunction<Long, Long, VeldError> stale) {
      Long expected = ctx.ifMatchVersion();
      if (expected == null || actual == null) {
        return null;
      }
      if (expected.longValue() == actual.longValue()) {
        return expected;
      }
      List<Violation> found = List.of(new Violation(id, "request", List.of(), "violates rule version", null));
      RuleWalk.report(hooks, spec, RuleWalk.Position.INPUT, RuleWalk.Outcome.REJECTED, found, started);
      VeldError error = stale.apply(expected, actual);
      throw early(error, error.status(), found);
    }

    // #endif
    /** One {@code ensure}: checked after the handler, once the result's invariants hold. */
    public void ensure(String id, String name, List<String> fields, String message, BooleanSupplier holds) {
      if (active()) {
        ensures.add(new Check(id, name, fields, message, holds));
      }
    }

    /** Hands the {@code ensure} checks to the router; call it last, just before returning the result. */
    public void arm() {
      if (!ensures.isEmpty()) {
        PENDING.set(List.copyOf(ensures));
      }
      // #if rollout
      if (!auditedEnsures.isEmpty()) {
        PENDING_AUDITED.set(List.copyOf(auditedEnsures));
      }
      // #endif
    }
    // #if rollout

    /**
     * One audited {@code require} ({@code @rollout(audit)}, spec/rules.md section 17.1): evaluated
     * where the enforced one would be, reported with the outcome {@code AUDITED}, and it never
     * answers. It has no {@code else} payload to build.
     */
    public void requireAudited(
        String id, String name, List<String> fields, String message, BooleanSupplier holds) {
      if (!active()) {
        return;
      }
      String error = null;
      boolean ok;
      try {
        ok = holds.getAsBoolean();
      } catch (RuleError e) {
        ok = false;
        error = e.kind();
        // #endif
        // #if rollout_derived
      } catch (NullPointerException e) {
        // The statement reads a derived field that is absent: it is skipped, the cause is reported.
        ok = true;
        // #endif
        // #if rollout
      }
      if (!ok) {
        String text = error == null ? message : "rule " + name + " could not be evaluated";
        Audit.report(
            hooks, spec, RuleWalk.Position.INPUT, List.of(new Violation(id, "request", fields, text, error)), started, null);
      }
    }

    /** One audited {@code ensure}: checked after the handler, it never rejects. */
    public void ensureAudited(String id, String name, List<String> fields, String message, BooleanSupplier holds) {
      if (active()) {
        auditedEnsures.add(new Check(id, name, fields, message, holds));
      }
    }
    // #endif
  }

  /**
   * Step 9: every {@code ensure} registered by the handler of this request, in source order,
   * under the output mode. Returns the {@code 500} response, or {@code null} when the response is
   * to be sent. The result's shape is validated first, as for the invariants of step 8.
   */
  static VeldResponse ensureFailure(
      EndpointSpec spec, HandlerHooks hooks, Json wire, Router.StdSigs std) {
    // #if rollout
    auditedEnsures(spec, hooks, wire);
    // #endif
    List<Check> checks = PENDING.get();
    PENDING.remove();
    RuleWalk.OutputMode mode = hooks.rulesOutput();
    if (checks == null || mode == RuleWalk.OutputMode.OFF) {
      return null;
    }
    long started = System.nanoTime();
    List<Issue> shape = Wire.validate(spec.success(), wire, "$", false);
    if (!shape.isEmpty()) {
      if (mode == RuleWalk.OutputMode.STRICT) {
        throw new IllegalStateException(spec.name() + ": response failed validation: " + shape);
      }
      return null;
    }
    List<Violation> found = new ArrayList<>();
    for (Check c : checks) {
      String error = null;
      boolean ok;
      try {
        ok = c.holds().getAsBoolean();
      } catch (RuleError e) {
        ok = false;
        error = e.kind();
        // #if derived
      } catch (NullPointerException e) {
        // The statement reads a derived field that is absent: it is skipped, the cause is reported.
        ok = true;
        // #endif
      }
      if (!ok) {
        String text = error == null ? c.message() : "rule " + c.name() + " could not be evaluated";
        found.add(new Violation(c.id(), "request", c.fields(), text, error));
      }
    }
    if (found.isEmpty()) {
      return null;
    }
    boolean strict = mode == RuleWalk.OutputMode.STRICT;
    RuleWalk.report(
        hooks, spec, RuleWalk.Position.OUTPUT, strict ? RuleWalk.Outcome.REJECTED : RuleWalk.Outcome.AUDITED, found, started);
    return strict ? Router.problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build()) : null;
  }
  // #if rollout

  /**
   * Step 9 for the audited {@code ensure} statements of this request: evaluated and reported when
   * the output mode is not {@code off}, and never rejecting. They are evaluated even when no
   * enforced {@code ensure} is registered.
   */
  private static void auditedEnsures(EndpointSpec spec, HandlerHooks hooks, Json wire) {
    List<Check> checks = PENDING_AUDITED.get();
    PENDING_AUDITED.remove();
    if (checks == null || hooks.rulesOutput() == RuleWalk.OutputMode.OFF) {
      return;
    }
    long started = System.nanoTime();
    if (!Wire.validate(spec.success(), wire, "$", false).isEmpty()) {
      return;
    }
    List<Violation> found = new ArrayList<>();
    for (Check c : checks) {
      String error = null;
      boolean ok;
      try {
        ok = c.holds().getAsBoolean();
      } catch (RuleError e) {
        ok = false;
        error = e.kind();
        // #endif
        // #if rollout_derived
      } catch (NullPointerException e) {
        // The statement reads a derived field that is absent: it is skipped, the cause is reported.
        ok = true;
        // #endif
        // #if rollout
      }
      if (!ok) {
        String text = error == null ? c.message() : "rule " + c.name() + " could not be evaluated";
        found.add(new Violation(c.id(), "request", c.fields(), text, error));
      }
    }
    Audit.report(hooks, spec, RuleWalk.Position.OUTPUT, found, started, null);
  }
  // #endif
}
