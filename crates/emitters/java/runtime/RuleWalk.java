package dev.veld.runtime;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
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
    return check(schema, prepared, "$", request);
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
   * did not carry (read-only fields) are filled with placeholders so that {@code fromWire} succeeds.
   * A rule that reads such a field is skipped for a request, so a placeholder is never read.
   */
  public static Json forDecoding(Schema schema, Json json, boolean request) {
    return request ? fill(schema, json) : json;
  }

  private static Json fill(Schema s, Json v) {
    Schema node = s.resolve();
    switch (node.kind) {
      case LIST -> {
        if (v instanceof Json.Arr arr) {
          List<Json> items = new ArrayList<>();
          for (Json item : arr.items()) {
            items.add(fill(node.of, item));
          }
          return Json.array(items);
        }
      }
      case MAP -> {
        if (v instanceof Json.Obj obj) {
          Map<String, Json> out = new LinkedHashMap<>();
          for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
            out.put(e.getKey(), fill(node.of, e.getValue()));
          }
          return new Json.Obj(out);
        }
      }
      case PAGE -> {
        if (v instanceof Json.Obj obj && obj.get("items") != null) {
          return obj.with("items", fill(Schema.list(node.of), obj.get("items")));
        }
      }
      case OBJECT -> {
        if (v instanceof Json.Obj obj) {
          return fillObject(node, obj);
        }
      }
      case UNION -> {
        Json label = v instanceof Json.Obj obj ? obj.get(node.discriminator) : null;
        Schema arm = label instanceof Json.Str str ? node.armFor(str.value()) : null;
        if (arm != null && fill(arm, v) instanceof Json.Obj filled) {
          return filled.with(node.discriminator, label);
        }
      }
      default -> {
      }
    }
    return v;
  }

  private static Json fillObject(Schema node, Json.Obj obj) {
    Map<String, Json> out = new LinkedHashMap<>();
    for (Schema.Field f : node.fields) {
      Json x = obj.get(f.wire());
      if (x != null) {
        out.put(f.wire(), fill(f.type(), x));
      } else if (!f.optional() && f.defaultValue() == null) {
        out.put(f.wire(), placeholder(f.type()));
      }
    }
    return new Json.Obj(out);
  }

  private static Json placeholder(Schema s) {
    Schema node = s.resolve();
    switch (node.kind) {
      case STRING, BYTES:
        return Json.of("");
      case INT:
        return Json.of(0L);
      case FLOAT:
        return Json.of(0.0);
      case LONG, DECIMAL:
        return Json.of("0");
      case BOOL:
        return Json.of(false);
      case DATE:
        return Json.of("1970-01-01");
      case DATETIME:
        return Json.of("1970-01-01T00:00:00Z");
      case TIME:
        return Json.of("00:00:00");
      case UUID:
        return Json.of("00000000-0000-0000-0000-000000000000");
      case ENUM:
        return Json.of(node.values.isEmpty() ? "" : node.values.get(0));
      case LIST:
        return Json.array();
      case MAP:
        return Json.object().build();
      case PAGE:
        return Json.object()
            .put("items", Json.array())
            .put("total", Json.of("0"))
            .put("page", Json.of(0L))
            .put("size", Json.of(0L))
            .build();
      case OBJECT:
        return fillObject(node, Json.object().build());
      case UNION:
        return placeholderArm(node);
      default:
        return Json.NULL;
    }
  }

  private static Json placeholderArm(Schema node) {
    if (node.arms.isEmpty()) {
      return Json.NULL;
    }
    Schema.Arm first = node.arms.get(0);
    Json body = placeholder(first.schema().get());
    return body instanceof Json.Obj obj ? obj.with(node.discriminator, Json.of(first.label())) : body;
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
    long started = System.nanoTime();
    List<Violation> found = new ArrayList<>();
    for (EndpointSpec.Param p : spec.params()) {
      Json value = p.stream() ? null : bound.get(p.name());
      if (value != null && reachesRules(p.type())) {
        found.addAll(check(p.type(), value, p.location(), true));
      }
    }
    if (found.isEmpty()) {
      return null;
    }
    report(hooks, spec, Position.INPUT, Outcome.REJECTED, found, started);
    return Router.problem(
        std.validation(), "ValidationError", 422, spec.sig(), Router.issuesToWire(issues(found)), found);
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
    OutputMode mode = hooks.rulesOutput();
    if (mode == OutputMode.OFF || schema == null || !reachesRules(schema)) {
      return null;
    }
    long started = System.nanoTime();
    List<Issue> shape = Wire.validate(schema, wire, "$", false);
    if (!shape.isEmpty()) {
      if (strictShape) {
        throw new IllegalStateException(spec.name() + ": response failed validation: " + shape);
      }
      return null;
    }
    List<Violation> found = check(schema, wire, "$", false);
    if (found.isEmpty()) {
      return null;
    }
    boolean strict = mode == OutputMode.STRICT;
    report(hooks, spec, Position.OUTPUT, strict ? Outcome.REJECTED : Outcome.AUDITED, found, started);
    return strict ? Router.problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build()) : null;
  }

  private static void report(
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
