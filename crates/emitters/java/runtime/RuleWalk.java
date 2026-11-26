package dev.veld.runtime;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Runs the invariants of a value by walking its {@link Schema} (spec/rules.md sections 5 and 8),
 * and carries the rule steps of the server pipeline and of the client. The schema of a declaration
 * with rules refers to its generated check function; this walk calls it for every object it
 * visits, the object's own rules before its fields, with the path syntax of validation issues.
 */
public final class RuleWalk {
  private RuleWalk() {}

  /** What the server does with a violated rule on its own output (spec/rules.md section 5.3). */
  public enum OutputMode {
    /** A violated output rule becomes {@code 500 Internal}; the violation goes to the hook. */
    STRICT,
    /** The response is sent unchanged; the violation goes to the hook. */
    AUDIT,
    /** Output rules are not evaluated. */
    OFF
  }

  /** Where the violating value was. */
  public enum Position {
    /** A value the caller sent. */
    INPUT,
    /** A value the server produced. */
    OUTPUT
  }

  /** What happened to the request or response. */
  public enum Outcome {
    /** The request was answered with {@code 422}, or the response with {@code 500}. */
    REJECTED,
    /** The response was sent unchanged. */
    AUDITED
  }

  /**
   * One violation as the {@code onRuleViolation} hook sees it.
   *
   * @param rule the rule id
   * @param operation the operation signature
   * @param position where the value was
   * @param outcome what the server did
   * @param violation the violation
   * @param elapsedMicros how long the check took, in microseconds
   */
  public record Event(
      String rule,
      String operation,
      Position position,
      Outcome outcome,
      Violation violation,
      long elapsedMicros) {}

  /** Observes violations of rules on the server. A failing hook never changes the response. */
  @FunctionalInterface
  public interface Hook {
    /** Called once per violation. */
    void onRuleViolation(Event event);
  }

  private static final Map<Schema, Boolean> REACHES = new ConcurrentHashMap<>();

  // -- the walk -------------------------------------------------------------------------------

  /**
   * The violations of a wire value: defaults are applied (read-only fields are dropped for a
   * request) and every rule of every object in it is evaluated. The value must have been
   * validated against {@code schema}. Paths start at {@code $}.
   */
  public static List<Violation> checkWire(Schema schema, Json wire, boolean request) {
    Json prepared = Wire.prepare(schema, wire, request ? Wire.Mode.REQUEST : Wire.Mode.RESPONSE);
    // #if no_derived
    return check(schema, prepared, "$", request);
    // #endif
    // #if derived
    List<Violation> found = new ArrayList<>();
    prepared = Derive.apply(schema, prepared, "$", request, found);
    found.addAll(check(schema, prepared, "$", request));
    return found;
    // #endif
  }

  /** The violations of a value that was validated and prepared, with paths that start at {@code path}. */
  public static List<Violation> check(Schema schema, Json prepared, String path, boolean request) {
    List<Violation> out = new ArrayList<>();
    walk(schema, prepared, path, request, out);
    return out;
  }

  private static void walk(Schema s, Json v, String path, boolean request, List<Violation> out) {
    Schema node = s.resolve();
    switch (node.kind) {
      case LIST -> walkItems(node.of, v, path, request, out);
      case MAP -> {
        if (v instanceof Json.Obj obj) {
          for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
            walk(node.of, e.getValue(), path + "." + e.getKey(), request, out);
          }
        }
      }
      case PAGE -> {
        if (v instanceof Json.Obj obj && obj.get("items") != null) {
          walkItems(node.of, obj.get("items"), path + ".items", request, out);
        }
      }
      case OBJECT -> walkObject(node, v, path, request, out);
      case UNION -> {
        Json label = v instanceof Json.Obj obj ? obj.get(node.discriminator) : null;
        Schema arm = label instanceof Json.Str str ? node.armFor(str.value()) : null;
        if (arm != null) {
          walk(arm, v, path, request, out);
        }
      }
      default -> {
      }
    }
  }

  private static void walkItems(Schema of, Json v, String path, boolean request, List<Violation> out) {
    if (v instanceof Json.Arr arr) {
      for (int i = 0; i < arr.items().size(); i++) {
        walk(of, arr.items().get(i), path + "[" + i + "]", request, out);
      }
    }
  }

  private static void walkObject(Schema node, Json v, String path, boolean request, List<Violation> out) {
    if (!(v instanceof Json.Obj obj)) {
      return;
    }
    if (node.check != null) {
      node.check.run(obj, request, path, out);
    }
    for (Schema.Field f : node.fields) {
      Json child = obj.get(f.wire());
      if (child != null) {
        walk(f.type(), child, path + "." + f.wire(), request, out);
      }
    }
  }

  // -- decoding a request value that lacks its read-only fields -------------------------------

  /**
   * The value a generated check decodes: in request position the required fields that the request
   * did not carry (read-only fields) are filled with placeholders so that {@code fromWire} succeeds
   * (see {@link Wire#fillReadonly}). A rule that reads such a field is skipped for a request.
   */
  public static Json forDecoding(Schema schema, Json json, boolean request) {
    return request ? Wire.fillReadonly(schema, json) : json;
  }

  // -- whether a schema can carry rules -------------------------------------------------------

  /** Whether a value of this schema can contain an object that has rules. */
  static boolean reachesRules(Schema schema) {
    Boolean known = REACHES.get(schema);
    if (known != null) {
      return known;
    }
    boolean found = search(schema, new IdentityHashMap<>());
    REACHES.put(schema, found);
    return found;
  }

  private static boolean search(Schema s, IdentityHashMap<Schema, Boolean> seen) {
    Schema node = s.resolve();
    if (seen.put(node, Boolean.TRUE) != null) {
      return false;
    }
    switch (node.kind) {
      case LIST, MAP, PAGE:
        return search(node.of, seen);
      case OBJECT:
        if (node.check != null) {
          return true;
        }
        // #if derived
        if (node.derive != null) {
          return true;
        }
        // #endif
        for (Schema.Field f : node.fields) {
          if (search(f.type(), seen)) {
            return true;
          }
        }
        return false;
      case UNION:
        for (Schema.Arm a : node.arms) {
          if (search(a.schema().get(), seen)) {
            return true;
          }
        }
        return false;
      default:
        return false;
    }
  }

  // -- the server: pipeline steps 5 and 8 -----------------------------------------------------

  /** Reports the output mode once, when the handler is created, if it is not the default. */
  static void announce(OutputMode mode) {
    if (mode != OutputMode.STRICT) {
      System.getLogger("veld.rules")
          .log(System.Logger.Level.WARNING, "rule output mode is {0}", mode.name().toLowerCase(java.util.Locale.ROOT));
    }
  }

  /**
   * Reports once, when the handler is created, that request validation is off while a request
   * body, a query object or a WebSocket message reaches a declaration with rules: those rules are
   * not enforced (spec/rules.md section 5.3).
   */
  static void announceUnvalidated(java.util.List<ServerEndpoint> endpoints) {
    for (ServerEndpoint e : endpoints) {
      for (EndpointSpec.Param p : e.spec().params()) {
        boolean carries =
            p.stream() || p.binding() == EndpointSpec.Binding.BODY || p.binding() == EndpointSpec.Binding.QUERY_OBJECT;
        if (carries && reachesRules(p.type())) {
          System.getLogger("veld.rules")
              .log(
                  System.Logger.Level.WARNING,
                  "request validation is off, so the rules of request bodies, query objects and WebSocket messages are not enforced");
          return;
        }
      }
    }
  }

  /**
   * Pipeline step 5: the invariants of the bound parameters. Returns the {@code 422} response, or
   * {@code null} when every rule holds.
   */
  static VeldResponse inputFailure(
      EndpointSpec spec, HandlerHooks hooks, Map<String, Json> bound, Router.StdSigs std) {
    // #if conditional_rules
    return inputFailure(spec, hooks, bound, std, List.of());
  }

  /**
   * Like {@link #inputFailure(EndpointSpec, HandlerHooks, Map, Router.StdSigs)}, with the issues of
   * the headers of step 5 first in the issues of the {@code 422} (spec/rules.md section 11.10).
   */
  static VeldResponse inputFailure(
      EndpointSpec spec,
      HandlerHooks hooks,
      Map<String, Json> bound,
      Router.StdSigs std,
      List<Issue> leading) {
    // #endif
    long started = System.nanoTime();
    List<Violation> found = new ArrayList<>();
    // #if derived
    found.addAll(Derive.take());
    // #endif
    // #if rollout
    Audit.begin();
    // #endif
    for (EndpointSpec.Param p : spec.params()) {
      Json value = p.stream() ? null : bound.get(p.name());
      if (value != null && reachesRules(p.type())) {
        found.addAll(check(p.type(), value, p.location(), true));
      }
    }
    // #if rollout
    Audit.report(hooks, spec, Position.INPUT, Audit.end(), started, null);
    // #endif
    if (found.isEmpty()) {
      return null;
    }
    report(hooks, spec, Position.INPUT, Outcome.REJECTED, found, started);
    // #if conditional_rules
    List<Issue> all = new ArrayList<>(leading);
    all.addAll(issues(found));
    return Router.problem(std.validation(), "ValidationError", 422, spec.sig(), Router.issuesToWire(all), found);
    // #endif
    // #if plain_rules
    return Router.problem(
        std.validation(), "ValidationError", 422, spec.sig(), Router.issuesToWire(issues(found)), found);
    // #endif
  }

  /**
   * Pipeline step 8: the invariants of a result (or of an error about to be encoded) according to
   * the output mode. When the mode is not {@code off} the shape of the value is validated first
   * ({@code strictShape}: an invalid result is an {@link IllegalStateException}, as with response
   * validation). Returns the {@code 500} response, or {@code null} when the response is to be
   * sent.
   */
  static VeldResponse outputFailure(
      EndpointSpec spec, HandlerHooks hooks, Schema schema, Json wire, boolean strictShape, Router.StdSigs std) {
    return outputRejected(spec, hooks, schema, wire, strictShape)
        ? Router.problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build())
        : null;
  }

  /** Thrown from a stream when the output mode {@code STRICT} rejects an item. */
  static final class Rejected extends RuntimeException {
    private static final long serialVersionUID = 1L;
    // #if derived
    private final transient List<Violation> violations;
    // #endif

    Rejected() {
      super("a stream item violates a rule", null, false, false);
      // #if derived
      this.violations = List.of();
      // #endif
    }
    // #if derived

    /** A rejection that reports {@code violations} (a principal that could not be completed). */
    Rejected(List<Violation> violations) {
      super("a derived field could not be computed", null, false, false);
      this.violations = List.copyOf(violations);
    }

    /** What the router reports to the rule hook; empty for a stream item, whose check reported it. */
    List<Violation> violations() {
      return violations;
    }
    // #endif
  }
  // #if derived

  /**
   * Pipeline step 5 with request validation off: the derived fields are still computed, and a value
   * that cannot be built is a {@code 422} with that field's violation (spec/rules.md section 17.2).
   * Returns the response, or {@code null} when every field was computed.
   */
  static VeldResponse derivedFailure(EndpointSpec spec, HandlerHooks hooks, Router.StdSigs std) {
    List<Violation> found = Derive.take();
    if (found.isEmpty()) {
      return null;
    }
    report(hooks, spec, Position.INPUT, Outcome.REJECTED, found, System.nanoTime());
    return Router.problem(
        std.validation(), "ValidationError", 422, spec.sig(), Router.issuesToWire(issues(found)), found);
  }
  // #endif

  /**
   * The same check for one item of a stream the server is about to send (spec/rules.md section
   * 5.2): {@code STRICT} throws {@link Rejected} (the stream ends as it does on any server
   * failure), {@code AUDIT} reports, {@code OFF} does nothing.
   */
  static void checkItem(EndpointSpec spec, HandlerHooks hooks, Json wire) {
    if (outputRejected(spec, hooks, spec.success(), wire, true)) {
      throw new Rejected();
    }
  }

  /** Whether strict mode rejects the value; reports every violation to the hook. */
  private static boolean outputRejected(
      EndpointSpec spec, HandlerHooks hooks, Schema schema, Json wire, boolean strictShape) {
    OutputMode mode = hooks.rulesOutput();
    // #if derived
    List<Violation> derived = Derive.take();
    // #endif
    if (mode == OutputMode.OFF || schema == null || !reachesRules(schema)) {
      return false;
    }
    long started = System.nanoTime();
    List<Issue> shape = Wire.validate(schema, wire, "$", false);
    if (!shape.isEmpty()) {
      if (strictShape) {
        throw new IllegalStateException(spec.name() + ": response failed validation: " + shape);
      }
      return false;
    }
    // #if rollout
    Audit.begin();
    // #endif
    List<Violation> found = check(schema, wire, "$", false);
    // #if derived
    found.addAll(0, derived);
    // #endif
    // #if rollout
    Audit.report(
        hooks, spec, Position.OUTPUT, Audit.end(), started, schema == spec.success() ? "result" : "error");
    // #endif
    if (found.isEmpty()) {
      return false;
    }
    boolean strict = mode == OutputMode.STRICT;
    report(hooks, spec, Position.OUTPUT, strict ? Outcome.REJECTED : Outcome.AUDITED, found, started);
    return strict;
  }

  /**
   * The invariants of a message a WebSocket client sent, after its shape was validated: the
   * {@code 422} problem (path root {@code message}) to send as the final frame, or {@code null}.
   */
  static VeldResponse messageFailure(
      EndpointSpec spec, HandlerHooks hooks, Schema schema, Json prepared, Router.StdSigs std) {
    if (schema == null || !reachesRules(schema)) {
      return null;
    }
    long started = System.nanoTime();
    // #if rollout
    Audit.begin();
    // #endif
    List<Violation> found = check(schema, prepared, "message", true);
    // #if derived
    found.addAll(0, Derive.take());
    // #endif
    // #if rollout
    Audit.report(hooks, spec, Position.INPUT, Audit.end(), started, null);
    // #endif
    if (found.isEmpty()) {
      return null;
    }
    report(hooks, spec, Position.INPUT, Outcome.REJECTED, found, started);
    return Router.problem(
        std.validation(), "ValidationError", 422, spec.sig(), Router.issuesToWire(issues(found)), found);
  }

  /** The client's check of a message it is about to send on a WebSocket (path root {@code message}). */
  static void localMessageCheck(
      EndpointSpec ep, Schema schema, Json wire, java.util.function.Function<List<Issue>, VeldError> validation) {
    if (schema == null || !reachesRules(schema)) {
      return;
    }
    // #if no_derived
    List<Violation> found = check(schema, Wire.prepare(schema, wire, Wire.Mode.REQUEST), "message", true);
    // #endif
    // #if derived
    List<Violation> found = new ArrayList<>();
    Json prepared = Derive.apply(schema, Wire.prepare(schema, wire, Wire.Mode.REQUEST), "message", true, found);
    found.addAll(check(schema, prepared, "message", true));
    // #endif
    if (!found.isEmpty()) {
      VeldError err = validation.apply(issues(found));
      err.setOp(ep.sig());
      err.setViolations(found);
      err.markLocal();
      throw err;
    }
  }

  static void report(
      HandlerHooks hooks, EndpointSpec spec, Position position, Outcome outcome, List<Violation> found, long started) {
    Hook hook = hooks.ruleHook();
    if (hook == null) {
      return;
    }
    long micros = (System.nanoTime() - started) / 1_000L;
    for (Violation v : found) {
      try {
        hook.onRuleViolation(new Event(v.rule(), spec.sig(), position, outcome, v, micros));
      } catch (RuntimeException ignored) {
        // A failing hook must not change the response.
      }
    }
  }

  private static List<Issue> issues(List<Violation> found) {
    List<Issue> out = new ArrayList<>();
    for (Violation v : found) {
      out.add(new Issue(v.path(), v.error() == null ? "rule" : "rule_error", v.message()));
    }
    return out;
  }

  // -- the client -----------------------------------------------------------------------------

  /**
   * Checks the invariants of the request body and of the query object before the request is
   * sent. A violation is thrown as the typed {@code ValidationError} the server would return,
   * with its violations, marked as raised locally.
   */
  static void localCheck(
      EndpointSpec ep, Json.Obj params, java.util.function.Function<List<Issue>, VeldError> validation) {
    List<Violation> found = new ArrayList<>();
    for (EndpointSpec.Param p : ep.params()) {
      boolean carries = p.binding() == EndpointSpec.Binding.BODY || p.binding() == EndpointSpec.Binding.QUERY_OBJECT;
      Json value = carries && !p.stream() ? params.get(p.name()) : null;
      if (value != null && reachesRules(p.type())) {
        Json prepared = Wire.prepare(p.type(), value, Wire.Mode.REQUEST);
        // #if derived
        prepared = Derive.apply(p.type(), prepared, p.location(), true, found);
        // #endif
        found.addAll(check(p.type(), prepared, p.location(), true));
      }
    }
    if (!found.isEmpty()) {
      VeldError err = validation.apply(issues(found));
      err.setOp(ep.sig());
      err.setViolations(found);
      err.markLocal();
      throw err;
    }
  }

  /**
   * Decodes a problem document that carries {@code violations} into the typed error with its
   * violations attached. Returns {@code null} when the document has none or is not a declared
   * error, so that the ordinary decoding applies.
   */
  static RuntimeException decodeProblem(EndpointSpec ep, int status, Json body) {
    if (!(body instanceof Json.Obj obj) || !(obj.get("type") instanceof Json.Str type)
        || !(obj.get("violations") instanceof Json.Arr listed)) {
      return null;
    }
    for (EndpointSpec.Declared d : ep.errors()) {
      if (d.sig().equals(type.value())) {
        Json data = obj.get("data") == null ? Json.object().build() : obj.get("data");
        int declared = obj.get("status") instanceof Json.Num n && n.value() != 0 ? (int) n.value() : status;
        String op = obj.get("operation") instanceof Json.Str s ? s.value() : ep.sig();
        VeldError err = d.factory().create(Wire.prepare(d.schema(), data, Wire.Mode.RESPONSE), declared, op);
        List<Violation> violations = new ArrayList<>();
        try {
          for (Json item : listed.items()) {
            violations.add(Violation.fromWire(item));
          }
        } catch (IllegalArgumentException e) {
          return null;
        }
        err.setViolations(violations);
        return err;
      }
    }
    return null;
  }
}
