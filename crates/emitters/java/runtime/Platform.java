package dev.veld.runtime;

import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.ToLongFunction;

/**
 * Platform phase P1 (spec/platform.md sections 2 to 9): the state of one request (the clock, read
 * once, and the caller), field visibility, the tenant check, the audit event and the limit of an
 * endpoint. The generated {@code <Service>Hooks} register what a service says about itself; the
 * router calls the functions below at the places the specification names.
 */
public final class Platform {
  private Platform() {}

  private static final ThreadLocal<State> CURRENT = new ThreadLocal<>();

  /** Paths and keys are sorted by the bytes of their UTF-8 text, as the references sort them. */
  static final Comparator<String> BYTES =
      (a, b) -> Arrays.compareUnsigned(a.getBytes(StandardCharsets.UTF_8), b.getBytes(StandardCharsets.UTF_8));

  /** The caller model of a service, as the platform features need it. */
  public static final class Principal {
    final Class<?> type;
    final Function<Object, Json> wire;
    final String tenant;
    final Schema schema;

    Principal(Class<?> type, Function<Object, Json> wire, String tenant, Schema schema) {
      this.type = type;
      this.wire = wire;
      this.tenant = tenant;
      this.schema = schema;
    }

    /** The wire form of {@code caller} when it is a value of this model, else {@code null}. */
    Json wireOf(Object caller) {
      return type.isInstance(caller) ? wire.apply(caller) : null;
    }
    // #if tenant_support

    /** The type of the {@code @tenant} field of the model. */
    TenantKind tenantKind() {
      for (Schema.Field f : schema.resolve().fields) {
        if (f.wire().equals(tenant)) {
          return switch (f.type().resolve().kind) {
            case UUID -> TenantKind.UUID;
            case INT -> TenantKind.INT;
            case LONG -> TenantKind.LONG;
            default -> TenantKind.STRING;
          };
        }
      }
      return TenantKind.STRING;
    }
    // #endif
  }

  /**
   * The principal's model of a service.
   *
   * @param type the generated class of the model
   * @param wire the wire form of a value of it ({@code toWire})
   * @param tenant the wire name of its {@code @tenant} field, or {@code null}
   * @param schema its wire description (what an audit event leaves out of the caller)
   */
  public static <P> Principal principal(Class<P> type, Function<P, Json> wire, String tenant, Schema schema) {
    return new Principal(type, caller -> wire.apply(type.cast(caller)), tenant, schema);
  }

  /** What one request keeps for the platform features. */
  static final class State {
    private volatile RequestContext ctx;
    private volatile long now;
    private volatile boolean timed;
    // #if visibility
    private final TreeSet<String> hidden = new TreeSet<>(BYTES);
    // #endif
    // #if audit
    private volatile boolean recording;
    private final Map<String, Json> parameters = new LinkedHashMap<>();
    private final Map<String, Json> facts = new LinkedHashMap<>();
    // #endif

    /** The clock reading of the request, in epoch milliseconds. */
    long now() {
      return now;
    }
    // #if audit

    /** The reading of the clock, or the system's when the request never reached the first statement of the pipeline. */
    long nowOrSystem() {
      return timed ? now : System.currentTimeMillis();
    }

    /** Whether the request is audited: it keeps its parameters and its facts. */
    boolean recording() {
      return recording;
    }

    void record() {
      this.recording = true;
    }

    void parameters(Map<String, Json> bound) {
      synchronized (parameters) {
        parameters.putAll(bound);
      }
    }

    Map<String, Json> parametersCopy() {
      synchronized (parameters) {
        return new LinkedHashMap<>(parameters);
      }
    }

    void fact(String name, Json wire) {
      synchronized (facts) {
        facts.put(name, wire);
      }
    }

    Map<String, Json> factsCopy() {
      synchronized (facts) {
        return new LinkedHashMap<>(facts);
      }
    }
    // #endif

    RequestContext ctx() {
      return ctx;
    }
    // #if visibility

    /** The wire paths that visibility dropped, sorted, without the root. */
    List<String> hidden() {
      synchronized (hidden) {
        return new ArrayList<>(hidden);
      }
    }

    void hide(Iterable<String> paths) {
      synchronized (hidden) {
        for (String p : paths) {
          hidden.add(p);
        }
      }
    }
    // #endif
  }

  // #if platform_early

  /** Ends a request with a response that is already built (the dry run of a test handler). */
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
  // #endif

  /** The state of the request this thread is running, or {@code null}. */
  static State current() {
    return CURRENT.get();
  }

  /** Puts {@code state} in force on this thread ({@code null} removes it). */
  static void install(State state) {
    if (state == null) {
      CURRENT.remove();
    } else {
      CURRENT.set(state);
    }
  }

  /** Runs {@code body} with {@code state} in force on this thread (a stream item is made after the pipeline returned). */
  static <T> T with(State state, java.util.function.Supplier<T> body) {
    State before = CURRENT.get();
    install(state);
    try {
      return body.get();
    } finally {
      install(before);
    }
  }

  /**
   * The first statement of the pipeline: reads the clock once, with the context of the request,
   * keeps it and puts the state in force on this thread. A clock that throws is reported and the
   * system clock is used.
   */
  static State open(HandlerHooks hooks, RequestContext ctx, State inherited) {
    State state = inherited == null ? new State() : inherited;
    state.ctx = ctx;
    if (!state.timed) {
      state.now = read(hooks, ctx);
      state.timed = true;
    }
    ctx.setPlatform(state);
    CURRENT.set(state);
    return state;
  }

  private static long read(HandlerHooks hooks, RequestContext ctx) {
    ToLongFunction<RequestContext> clock = hooks.clock();
    if (clock == null) {
      return System.currentTimeMillis();
    }
    try {
      return clock.applyAsLong(ctx);
    } catch (RuntimeException | StackOverflowError err) {
      report(hooks, ctx, err);
      return System.currentTimeMillis();
    }
  }

  /** Tells {@code onError}; a failing hook never changes the response. */
  static void report(HandlerHooks hooks, RequestContext ctx, Throwable err) {
    try {
      if (hooks.onError() != null) {
        hooks.onError().onError(err, ctx);
      }
    } catch (RuntimeException ignored) {
      // A failing hook must not change the response.
    }
  }
  // #if marks

  // -- marks on an object schema ---------------------------------------------------------------

  /** The platform marks of an object schema, written by the generated declarations. */
  public static final class Marks {
    // #endif
    // #if visibility
    Show show;
    // #endif
    // #if tenancy
    String tenantWire;
    String tenantRule;
    TenantKind tenantKind;
    // #endif
    // #if audit
    java.util.Set<String> pii = java.util.Set.of();
    // #endif
    // #if marks

    private Marks() {}

    /** Starts the marks of one declaration. */
    public static Marks of() {
      return new Marks();
    }
    // #endif
    // #if visibility

    /** The fields of this record that the caller may not see (spec/platform.md section 2). */
    public Marks show(Show value) {
      this.show = value;
      return this;
    }
    // #endif
    // #if tenancy

    /**
     * This record is tenant-scoped (spec/platform.md section 3).
     *
     * @param wire the wire name of its {@code @tenant} field
     * @param rule the id of the tenant rule ({@code Field.tenant_id}), what {@code onRuleViolation} hears
     * @param kind the type of the field
     */
    public Marks tenant(String wire, String rule, TenantKind kind) {
      this.tenantWire = wire;
      this.tenantRule = rule;
      this.tenantKind = kind;
      return this;
    }
    // #endif
    // #if audit

    /** The wire names of the fields of this record marked {@code @pii} or {@code @secret} (spec/rules.md section 19.3). */
    public Marks pii(String... wires) {
      this.pii = java.util.Set.of(wires);
      return this;
    }
    // #endif
    // #if marks
  }
  // #endif
  // #if visibility

  /** The verdict of the {@code @visible} fields of one declaration over one record. */
  @FunctionalInterface
  public interface Show {
    /**
     * The wire names of the present {@code @visible} fields that {@code caller} may not see. Every
     * expression is judged against the whole record, before any field is dropped.
     */
    List<String> hidden(Json.Obj record, Object caller, long now);
  }

  /**
   * Whether {@code caller}, a value of {@code type}, passes {@code test}. It fails closed: no caller,
   * a caller of another model, and a test that cannot be evaluated all hide the field.
   */
  public static <P> boolean visibleTo(Object caller, Class<P> type, Predicate<P> test) {
    if (!type.isInstance(caller)) {
      return false;
    }
    try {
      return test.test(type.cast(caller));
    } catch (RuntimeException | StackOverflowError err) {
      return false;
    }
  }

  /**
   * A record as the generated {@code fromWire} reads it: a member that is {@code null} (a field
   * with no value, at any depth) is left out, so that a value that holds one can be judged. The
   * record itself is returned when there is none.
   */
  public static Json.Obj decodable(Json.Obj record) {
    return (Json.Obj) withoutNulls(record);
  }

  private static Json withoutNulls(Json v) {
    if (v instanceof Json.Obj obj) {
      Map<String, Json> kept = new LinkedHashMap<>();
      boolean changed = false;
      for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
        Json child = withoutNulls(e.getValue());
        changed |= child != e.getValue() || e.getValue() instanceof Json.Null;
        if (!(child instanceof Json.Null)) {
          kept.put(e.getKey(), child);
        }
      }
      return changed ? new Json.Obj(kept) : obj;
    }
    if (v instanceof Json.Arr arr) {
      List<Json> kept = new ArrayList<>();
      boolean changed = false;
      for (Json x : arr.items()) {
        Json child = withoutNulls(x);
        changed |= child != x;
        kept.add(child);
      }
      return changed ? Json.array(kept) : arr;
    }
    return v;
  }

  private static final Map<Schema, Boolean> SHOWS = Collections.synchronizedMap(new IdentityHashMap<>());

  /** Whether a value of {@code schema} can hold a field with {@code @visible}. */
  static boolean reachesShow(Schema schema) {
    Boolean known = SHOWS.get(schema);
    if (known == null) {
      known = search(schema, new IdentityHashMap<>(), s -> s.marks != null && s.marks.show != null);
      SHOWS.put(schema, known);
    }
    return known;
  }
  // #endif
  // #if marks

  /** Whether {@code schema} or a schema it reaches satisfies {@code test}. */
  private static boolean search(Schema schema, IdentityHashMap<Schema, Boolean> seen, Predicate<Schema> test) {
    Schema s = schema.resolve();
    if (seen.put(s, Boolean.TRUE) != null) {
      return false;
    }
    if (test.test(s)) {
      return true;
    }
    switch (s.kind) {
      case LIST, MAP, PAGE -> {
        return search(s.of, seen, test);
      }
      case OBJECT -> {
        for (Schema.Field f : s.fields) {
          if (search(f.type(), seen, test)) {
            return true;
          }
        }
        return false;
      }
      case UNION -> {
        for (Schema.Arm a : s.arms) {
          if (search(a.schema().get(), seen, test)) {
            return true;
          }
        }
        return false;
      }
      default -> {
        return false;
      }
    }
  }
  // #endif
  // #if visibility

  // -- visibility ------------------------------------------------------------------------------

  /**
   * Applies visibility to {@code wire}, a value of {@code schema} that is about to be sent: every
   * {@code @visible} field the caller of this request may not see is left out, at any depth, and
   * the wire paths of what was dropped are kept in the state of the request (the {@code hidden} of
   * an audit event). The value is a copy; {@code root} starts the paths ({@code ""}).
   */
  static Json shown(Schema schema, Json wire, RequestContext ctx) {
    List<String> dropped = new ArrayList<>();
    Json out = showInto(schema, wire, ctx, dropped);
    if (!dropped.isEmpty()) {
      ctx.platform().hide(dropped);
    }
    return out;
  }

  /** Like {@link #shown}, handing the dropped paths back instead of keeping them. */
  private static Json showInto(Schema schema, Json wire, RequestContext ctx, List<String> dropped) {
    State state = ctx.platform();
    if (state == null || !reachesShow(schema)) {
      return wire;
    }
    return show(schema, wire, ctx.principal(), state.now(), "", dropped);
  }

  /**
   * A value after visibility and the wire paths of what was dropped, rooted at {@code $} and sorted
   * by their bytes (the function of {@code tests/wire/visibility}).
   *
   * @param value the value that is sent
   * @param hidden the paths that were dropped
   */
  public record Shown(Json value, List<String> hidden) {}

  /**
   * Applies visibility to {@code value}, a wire value of {@code schema}, for {@code caller} (a value
   * of the principal's model, or {@code null} for a request with no caller) at the clock reading
   * {@code now} in epoch milliseconds.
   *
   * @throws IllegalStateException when the value does not have the shape of its schema
   */
  public static Shown showValue(Schema schema, Json value, Object caller, long now) {
    List<String> dropped = new ArrayList<>();
    Json out = show(schema, value, caller, now, "", dropped);
    TreeSet<String> rooted = new TreeSet<>(BYTES);
    for (String path : dropped) {
      rooted.add(path.isEmpty() ? "$" : path.startsWith("[") ? "$" + path : "$." + path);
    }
    return new Shown(out, new ArrayList<>(rooted));
  }

  private static Json show(Schema schema, Json v, Object caller, long now, String path, List<String> out) {
    if (v == null || v instanceof Json.Null) {
      return v;
    }
    Schema s = schema.resolve();
    switch (s.kind) {
      case LIST -> {
        if (v instanceof Json.Arr arr) {
          List<Json> kept = new ArrayList<>();
          for (int i = 0; i < arr.items().size(); i++) {
            kept.add(show(s.of, arr.items().get(i), caller, now, path + "[" + i + "]", out));
          }
          return Json.array(kept);
        }
        throw shape(path, "an array");
      }
      case MAP -> {
        if (v instanceof Json.Obj obj) {
          Map<String, Json> kept = new LinkedHashMap<>();
          for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
            kept.put(e.getKey(), show(s.of, e.getValue(), caller, now, join(path, e.getKey()), out));
          }
          return new Json.Obj(kept);
        }
        throw shape(path, "an object");
      }
      case PAGE -> {
        if (v instanceof Json.Obj obj) {
          Json items = obj.get("items");
          return items == null
              ? obj
              : obj.with("items", show(Schema.list(s.of), items, caller, now, join(path, "items"), out));
        }
        throw shape(path, "an object");
      }
      case UNION -> {
        Json label = v instanceof Json.Obj obj ? obj.get(s.discriminator) : null;
        Schema arm = label instanceof Json.Str str ? s.armFor(str.value()) : null;
        if (arm == null) {
          throw shape(path, "a value that names an arm of the union");
        }
        return show(arm, v, caller, now, path, out);
      }
      case OBJECT -> {
        if (v instanceof Json.Obj obj) {
          return showRecord(s, obj, caller, now, path, out);
        }
        throw shape(path, "an object");
      }
      default -> {
        return v;
      }
    }
  }

  private static Json showRecord(Schema s, Json.Obj obj, Object caller, long now, String path, List<String> out) {
    List<String> hide =
        s.marks != null && s.marks.show != null ? s.marks.show.hidden(obj, caller, now) : List.of();
    Map<String, Json> kept = new LinkedHashMap<>(obj.fields());
    for (Schema.Field f : s.fields) {
      Json child = obj.get(f.wire());
      if (child == null) {
        continue;
      }
      String here = join(path, f.wire());
      if (hide.contains(f.wire())) {
        kept.remove(f.wire());
        out.add(here);
      } else {
        kept.put(f.wire(), show(f.type(), child, caller, now, here, out));
      }
    }
    return new Json.Obj(kept);
  }

  private static String join(String path, String name) {
    return path.isEmpty() ? name : path + "." + name;
  }

  // #endif

  // #if tenant_support

  // -- the tenant of the caller ----------------------------------------------------------------

  /** The type of a {@code @tenant} field: how two values of it are compared and written. */
  public enum TenantKind {
    /** A {@code uuid}: compared in any case. */
    UUID,
    /** A {@code string}: compared as it is. */
    STRING,
    /** An {@code int}: a JSON number. */
    INT,
    /** A {@code long}: a decimal string, compared by its number. */
    LONG
  }

  /**
   * The tenant of the caller of this request: the {@code @tenant} member of its principal. {@code
   * null} when the service has no principal, the caller is not a value of it, or it has no tenant: a
   * caller without a tenant owns nothing.
   */
  static Json callerTenant(HandlerHooks hooks, RequestContext ctx) {
    Principal principal = hooks.principal();
    if (principal == null || principal.tenant == null) {
      return null;
    }
    Json wire = principal.wireOf(ctx.principal());
    Json tenant = wire instanceof Json.Obj obj ? obj.get(principal.tenant) : null;
    return tenant == null || tenant instanceof Json.Null ? null : tenant;
  }

  private static java.math.BigDecimal number(Json v) {
    if (v instanceof Json.Str s) {
      return new java.math.BigDecimal(s.value());
    }
    if (v instanceof Json.Num n) {
      return java.math.BigDecimal.valueOf(n.value());
    }
    throw new IllegalArgumentException("not a number");
  }

  /**
   * The text of a tenant (the envelope attribute {@code veldtenant}, the key of a limit): a {@code
   * string} as it is, a {@code uuid} in lower case, an {@code int} or a {@code long} in decimal
   * digits (a {@code long} is already the text of its digits on the wire).
   */
  static String tenantText(TenantKind kind, Json value) {
    return switch (kind) {
      case UUID -> ((Json.Str) value).value().toLowerCase(java.util.Locale.ROOT);
      case STRING -> ((Json.Str) value).value();
      case LONG -> value instanceof Json.Str text ? text.value() : number(value).toBigInteger().toString();
      case INT -> number(value).toBigInteger().toString();
    };
  }

  // #endif
  // #if tenancy

  private static final Map<Schema, Boolean> TENANTS = Collections.synchronizedMap(new IdentityHashMap<>());

  /** Whether a value of {@code schema} can hold a tenant-scoped value. */
  static boolean reachesTenant(Schema schema) {
    Boolean known = TENANTS.get(schema);
    if (known == null) {
      known = search(schema, new IdentityHashMap<>(), s -> s.marks != null && s.marks.tenantWire != null);
      TENANTS.put(schema, known);
    }
    return known;
  }

  /** Whether the tenant {@code have} of a value is the caller's tenant {@code caller}, as values of {@code kind}. */
  static boolean sameTenant(TenantKind kind, Json have, Json caller) {
    try {
      switch (kind) {
        case UUID:
          return have instanceof Json.Str a
              && caller instanceof Json.Str b
              && java.util.UUID.fromString(a.value()).equals(java.util.UUID.fromString(b.value()));
        case STRING:
          return have instanceof Json.Str a && caller instanceof Json.Str b && a.value().equals(b.value());
        default:
          return number(have).compareTo(number(caller)) == 0;
      }
    } catch (IllegalArgumentException | ArithmeticException err) {
      return false;
    }
  }

  /**
   * Every tenant-scoped value of {@code wire}, a value of {@code schema}, that is not the caller's
   * ({@code caller} is the caller's tenant, or {@code null}), in the order of the walk. Each is a
   * violation of the rule of its model's {@code @tenant} field; its path starts at {@code root}
   * ({@code result}, {@code error}, {@code item}) and goes on as the paths of violations do:
   * {@code result[1]}, {@code result.items[0].guest}.
   */
  static List<Violation> foreign(Schema schema, Json wire, Json caller, String root) {
    List<Violation> found = new ArrayList<>();
    if (reachesTenant(schema)) {
      foreignWalk(schema, wire, caller, root, "", found);
    }
    return found;
  }

  private static void foreignWalk(Schema schema, Json v, Json caller, String root, String path, List<Violation> out) {
    if (v == null || v instanceof Json.Null) {
      return;
    }
    Schema s = schema.resolve();
    switch (s.kind) {
      case LIST -> {
        if (!(v instanceof Json.Arr arr)) {
          throw shape(path, "an array");
        }
        for (int i = 0; i < arr.items().size(); i++) {
          foreignWalk(s.of, arr.items().get(i), caller, root, path + "[" + i + "]", out);
        }
      }
      case MAP -> {
        if (!(v instanceof Json.Obj obj)) {
          throw shape(path, "an object");
        }
        for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
          foreignWalk(s.of, e.getValue(), caller, root, path + "." + e.getKey(), out);
        }
      }
      case PAGE -> {
        if (!(v instanceof Json.Obj obj)) {
          throw shape(path, "an object");
        }
        if (obj.get("items") != null) {
          foreignWalk(Schema.list(s.of), obj.get("items"), caller, root, path + ".items", out);
        }
      }
      case UNION -> {
        Json label = v instanceof Json.Obj obj ? obj.get(s.discriminator) : null;
        Schema arm = label instanceof Json.Str str ? s.armFor(str.value()) : null;
        if (arm == null) {
          throw shape(path, "a value that names an arm of the union");
        }
        foreignWalk(arm, v, caller, root, path, out);
      }
      case OBJECT -> {
        if (!(v instanceof Json.Obj obj)) {
          throw shape(path, "an object");
        }
        if (s.marks != null && s.marks.tenantWire != null) {
          Json have = obj.get(s.marks.tenantWire);
          boolean mine = have != null
              && !(have instanceof Json.Null)
              && caller != null
              && sameTenant(s.marks.tenantKind, have, caller);
          if (!mine) {
            out.add(new Violation(
                s.marks.tenantRule, root + path, List.of(s.marks.tenantWire), "violates rule tenant", null));
          }
        }
        for (Schema.Field f : s.fields) {
          if (obj.get(f.wire()) != null) {
            foreignWalk(f.type(), obj.get(f.wire()), caller, root, path + "." + f.wire(), out);
          }
        }
      }
      default -> {
      }
    }
  }

  /**
   * The tenant check of a value that is about to be sent (a result, a declared error's data): the
   * {@code 500 Internal} with no detail when it holds a value of another tenant, after one {@code
   * onRuleViolation} event for each; {@code null} when it is all the caller's. No output mode turns
   * it off.
   */
  static VeldResponse tenantFailure(
      HandlerHooks hooks, EndpointSpec spec, Schema schema, Json wire, String root, RequestContext ctx, Router.StdSigs std) {
    if (schema == null || !reachesTenant(schema)) {
      return null;
    }
    List<Violation> found = foreign(schema, wire, callerTenant(hooks, ctx), root);
    if (found.isEmpty()) {
      return null;
    }
    RuleWalk.report(hooks, spec, RuleWalk.Position.OUTPUT, RuleWalk.Outcome.REJECTED, found, System.nanoTime());
    return Router.problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
  }

  /**
   * The tenant check of an item of a stream: a value of another tenant ends the stream as any server
   * failure does, after its events.
   */
  static void tenantItem(HandlerHooks hooks, EndpointSpec spec, Json wire, RequestContext ctx) {
    List<Violation> found = foreign(spec.success(), wire, callerTenant(hooks, ctx), "item");
    if (!found.isEmpty()) {
      RuleWalk.report(hooks, spec, RuleWalk.Position.OUTPUT, RuleWalk.Outcome.REJECTED, found, System.nanoTime());
      throw new RuleWalk.Rejected();
    }
  }
  // #endif
  // #if shapes

  private static IllegalStateException shape(String path, String expected) {
    return new IllegalStateException(
        "the value at `" + (path.isEmpty() ? "$" : path) + "` is not " + expected + " as its type says");
  }

  // -- a declared error ------------------------------------------------------------------------

  /**
   * A declared error about to be sent (a problem document): its {@code data} is shown to the
   * caller, as any value is. An error of the prelude, an error that reaches nothing and a body
   * that is not a problem document are sent as they are.
   */
  static VeldResponse errorAnswer(HandlerHooks hooks, EndpointSpec spec, VeldResponse response, Router.StdSigs std) {
    State state = CURRENT.get();
    if (state == null || response.body() == null || response.status() < 400 || !errorsReach(spec)) {
      return response;
    }
    Json.Obj body;
    try {
      body = Json.parse(response.body()) instanceof Json.Obj o ? o : null;
    } catch (Json.ParseException err) {
      return response;
    }
    EndpointSpec.Declared declared = body == null ? null : declaredOf(spec, body.get("type"));
    if (declared == null || declared.schema() == null || body.get("data") == null) {
      return response;
    }
    Json data = body.get("data");
    RequestContext ctx = state.ctx();
    boolean changed = false;
    // #endif
    // #if tenancy
    VeldResponse foreign = tenantFailure(hooks, spec, declared.schema(), data, "error", ctx, std);
    if (foreign != null) {
      return foreign;
    }
    // #endif
    // #if visibility
    List<String> dropped = new ArrayList<>();
    data = showInto(declared.schema(), data, ctx, dropped);
    if (!dropped.isEmpty()) {
      state.hide(dropped);
      changed = true;
    }
    // #endif
    // #if shapes
    return changed ? new VeldResponse(response.status(), response.headers(), body.with("data", data).write()) : response;
  }

  // #endif
  // #if declared_of
  /** The declared error of the endpoint that a problem document of this {@code type} is. */
  private static EndpointSpec.Declared declaredOf(EndpointSpec spec, Json type) {
    if (type instanceof Json.Str str) {
      for (EndpointSpec.Declared d : spec.errors()) {
        if (d.sig().equals(str.value())) {
          return d;
        }
      }
    }
    return null;
  }
  // #endif
  // #if shapes

  /** Whether a declared error of the endpoint can hold what the answer features act on. */
  private static boolean errorsReach(EndpointSpec spec) {
    for (EndpointSpec.Declared d : spec.errors()) {
      if (d.schema() != null && reaches(d.schema())) {
        return true;
      }
    }
    return false;
  }

  /** Whether a value of {@code schema} can hold a field that visibility or the tenant check acts on. */
  static boolean reaches(Schema schema) {
    // #endif
    // #if visibility
    if (reachesShow(schema)) {
      return true;
    }
    // #endif
    // #if tenancy
    if (reachesTenant(schema)) {
      return true;
    }
    // #endif
    // #if shapes
    return false;
  }
  // #endif
  // #if audit

  // -- the audit log ---------------------------------------------------------------------------

  /** What a service says about one audited endpoint (spec/platform.md section 4). */
  public static final class Audited {
    final boolean authenticated;
    final Map<String, Schema> facts;
    final java.util.Set<String> errors;

    Audited(boolean authenticated, Map<String, Schema> facts, java.util.Set<String> errors) {
      this.authenticated = authenticated;
      this.facts = facts;
      this.errors = errors;
    }
  }

  /**
   * An audited endpoint.
   *
   * @param authenticated whether the endpoint has auth: its event has the principal
   * @param facts the wire description of the facts of the endpoint, by name
   * @param errors the signatures of the errors the contract declares itself (not the prelude's): the
   *     data of such an error is the {@code result} of the event
   */
  public static Audited audited(boolean authenticated, Map<String, Schema> facts, String... errors) {
    return new Audited(authenticated, Map.copyOf(facts), java.util.Set.of(errors));
  }

  /** Marks the state of an audited request: it keeps what it binds and loads. */
  static void recording(State state) {
    state.record();
  }

  /** Step 4 has passed: the parameters, in wire form, for the audit event. */
  static void parameters(State state, Map<String, Json> bound) {
    if (state != null && state.recording()) {
      state.parameters(bound);
    }
  }

  /** A fact was loaded (and is the caller's): its wire form, for the audit event. */
  static void fact(RequestContext ctx, String name, Json wire) {
    State state = ctx.platform();
    if (state != null && state.recording()) {
      state.fact(name, wire);
    }
  }

  /**
   * The audit event of a request, given to the hook once, when the answer is ready and before it is
   * written. A hook that fails, and an event that cannot be built, do not change the answer: {@code
   * onError} hears an {@link AuditFailure}.
   */
  static void record(
      HandlerHooks hooks, EndpointSpec spec, Audited info, State state, VeldResponse response) {
    HandlerHooks.AuditHook hook = hooks.auditHook();
    if (hook == null) {
      return;
    }
    AuditEvent event;
    try {
      event = build(hooks, spec, info, state, response);
    } catch (RuntimeException | StackOverflowError err) {
      report(hooks, state.ctx(), new AuditFailure("the audit event could not be built: " + err.getMessage(), null, err));
      return;
    }
    try {
      hook.audit(event);
    } catch (InterruptedException err) {
      Thread.currentThread().interrupt();
      report(hooks, state.ctx(), new AuditFailure("the audit hook was interrupted", event, err));
    } catch (Exception | StackOverflowError err) {
      report(hooks, state.ctx(), new AuditFailure("the audit hook failed: " + err.getMessage(), event, err));
    }
  }

  /** The event of a request that ended with {@code response}. */
  private static AuditEvent build(
      HandlerHooks hooks, EndpointSpec spec, Audited info, State state, VeldResponse response) {
    List<String> redacted = new ArrayList<>();
    RequestContext ctx = state.ctx();
    Json principal = null;
    if (info.authenticated && ctx != null && ctx.principal() != null) {
      principal = principalJson(hooks, ctx.principal(), redacted);
    }
    Json body = bodyOf(response);
    Json.Obj problem = response.status() >= 400 && body instanceof Json.Obj o ? o : null;
    String error = null;
    if (response.status() >= 400) {
      error = problem != null && problem.get("title") instanceof Json.Str title ? title.value() : "HTTP " + response.status();
    }
    Json result = resultOf(spec, info, response.status(), body, problem, redacted);
    // #endif
    // #if audit_visibility
    List<String> hidden = state.hidden();
    // #endif
    // #if audit_not_visibility
    List<String> hidden = List.of();
    // #endif
    // #if audit
    TreeSet<String> sorted = new TreeSet<>(BYTES);
    sorted.addAll(redacted);
    return new AuditEvent(
        spec.sig(),
        Canon.isoMillis(Instant.ofEpochMilli(state.nowOrSystem())),
        principal,
        new AuditEvent.Outcome(response.status(), error),
        new Json.Obj(parametersOf(spec, state, redacted)),
        new Json.Obj(factsOf(info, state, redacted)),
        result,
        hidden,
        new ArrayList<>(sorted));
  }

  private static Json bodyOf(VeldResponse response) {
    if (response.body() == null || response.body().isEmpty()) {
      return null;
    }
    try {
      return Json.parse(response.body());
    } catch (Json.ParseException err) {
      return null;
    }
  }

  /** The body of a success, or the data of an error the contract declares; nothing for an error of the prelude. */
  private static Json resultOf(
      EndpointSpec spec, Audited info, int status, Json body, Json.Obj problem, List<String> redacted) {
    if (status < 400) {
      if (body == null) {
        return null;
      }
      return spec.success() == null ? body : Redact.value(spec.success(), body, "result", redacted);
    }
    EndpointSpec.Declared declared = problem == null ? null : declaredOf(spec, problem.get("type"));
    if (declared == null || !info.errors.contains(declared.sig()) || problem.get("data") == null) {
      return null;
    }
    return Redact.value(declared.schema(), problem.get("data"), "result", redacted);
  }

  private static Json principalJson(HandlerHooks hooks, Object caller, List<String> redacted) {
    Principal model = hooks.principal();
    Json wire = model == null ? null : model.wireOf(caller);
    return wire == null ? toJson(caller) : Redact.value(model.schema, wire, "principal", redacted);
  }

  private static Map<String, Json> parametersOf(EndpointSpec spec, State state, List<String> redacted) {
    Map<String, Json> out = new LinkedHashMap<>();
    Map<String, Json> bound = state.parametersCopy();
    for (EndpointSpec.Param p : spec.params()) {
      Json value = bound.get(p.name());
      if (value != null) {
        out.put(p.name(), Redact.value(p.type(), value, "parameters." + p.name(), redacted));
      }
    }
    return out;
  }

  private static Map<String, Json> factsOf(Audited info, State state, List<String> redacted) {
    Map<String, Json> out = new LinkedHashMap<>();
    for (Map.Entry<String, Json> e : state.factsCopy().entrySet()) {
      Schema schema = info.facts.get(e.getKey());
      out.put(
          e.getKey(),
          schema == null ? e.getValue() : Redact.value(schema, e.getValue(), "facts." + e.getKey(), redacted));
    }
    return out;
  }

  /**
   * What a principal without a model of the contract (a hook that returns something of its own) is
   * in an audit event: JSON values, maps, lists, texts, numbers and booleans as they are; anything
   * else as its text.
   */
  static Json toJson(Object value) {
    if (value == null) {
      return Json.NULL;
    }
    if (value instanceof Json json) {
      return json;
    }
    if (value instanceof CharSequence text) {
      return Json.of(text.toString());
    }
    if (value instanceof Boolean flag) {
      return Json.of(flag.booleanValue());
    }
    if (value instanceof Number number) {
      return number instanceof Long || number instanceof Integer || number instanceof Short || number instanceof Byte
          ? Json.of(number.longValue())
          : Json.of(number.doubleValue());
    }
    if (value instanceof Map<?, ?> map) {
      Map<String, Json> members = new LinkedHashMap<>();
      for (Map.Entry<?, ?> e : map.entrySet()) {
        members.put(String.valueOf(e.getKey()), toJson(e.getValue()));
      }
      return new Json.Obj(members);
    }
    if (value instanceof Iterable<?> items) {
      List<Json> listed = new ArrayList<>();
      for (Object item : items) {
        listed.add(toJson(item));
      }
      return Json.array(listed);
    }
    return Json.of(String.valueOf(value));
  }
  // #endif
  // #if limits_server

  // -- rate limits -----------------------------------------------------------------------------

  /** Whose calls a limit counts together (spec/platform.md section 5). */
  public enum By {
    /** One count per caller: the caller-identity hook names it. */
    USER("user"),
    /** One count per tenant of the caller. */
    TENANT("tenant"),
    /** One count per client address: the address hook names it. */
    IP("ip");

    final String word;

    By(String word) {
      this.word = word;
    }
  }

  /** The limit of one endpoint: {@code count} calls in a window of {@code windowSeconds} per {@code by}. */
  public static final class Limit {
    final long count;
    final long windowSeconds;
    final By by;

    Limit(long count, long windowSeconds, By by) {
      this.count = count;
      this.windowSeconds = windowSeconds;
      this.by = by;
    }
  }

  /** {@code @limit(count, per: window, by: by)}: the window in seconds. */
  public static Limit limit(long count, long windowSeconds, By by) {
    return new Limit(count, windowSeconds, by);
  }

  /** The limiter of a server that was given none: it says so once, because it counts per process. */
  static Limiter memoryLimiter() {
    System.getLogger("veld.limits")
        .log(
            System.Logger.Level.WARNING,
            "no limiter was given: the memory limiter counts per process, so a limit holds per process");
    return new MemoryLimiter();
  }

  /**
   * The limit of the endpoint, after authentication and before the middleware (spec/platform.md
   * section 5): counts the call and answers {@code 429 TooManyRequests} with {@code retryAfter} and
   * the {@code Retry-After} header when the limiter refuses it, else {@code null}. A limiter or a hook
   * that fails lets the call through, and {@code onError} hears of it.
   */
  static VeldResponse limited(HandlerHooks hooks, EndpointSpec spec, RequestContext ctx) {
    Limit limit = hooks.limit(spec.sig());
    if (limit == null) {
      return null;
    }
    Limiter.Decision decision;
    try {
      String key = spec.sig() + "|" + limit.by.word + "|" + identity(hooks, limit.by, ctx);
      decision = hooks.limiter().take(key, limit.count, limit.windowSeconds);
      if (decision == null) {
        throw new IllegalStateException("the limiter gave no answer");
      }
    } catch (InterruptedException err) {
      Thread.currentThread().interrupt();
      report(hooks, ctx, err);
      return null;
    } catch (Exception | StackOverflowError err) {
      report(hooks, ctx, err);
      return null;
    }
    return decision.allowed() ? null : tooMany(spec, decision.retryAfterSeconds());
  }

  /** The text that stands for the caller: what the application's hook says, or the tenant of the principal. */
  private static String identity(HandlerHooks hooks, By by, RequestContext ctx) throws Exception {
    String text;
    switch (by) {
      case USER -> text = hooks.limitCaller().caller(ctx.principal(), ctx);
      case IP -> text = hooks.address().address(ctx);
      default -> {
        Json tenant = callerTenant(hooks, ctx);
        // A caller whose principal has no tenant is counted with the others that have none.
        text = tenant == null ? "" : tenantText(hooks.principal().tenantKind(), tenant);
      }
    }
    if (text == null) {
      throw new IllegalStateException("the hook that names the " + by.word + " returned no text");
    }
    return text;
  }

  /** The prelude's {@code 429 TooManyRequests}: {@code retryAfter} in seconds, and the header with the same number. */
  private static VeldResponse tooMany(EndpointSpec spec, long seconds) {
    for (EndpointSpec.Declared d : spec.errors()) {
      if (d.status() == 429) {
        Json data = Json.object().put("retryAfter", Json.of(seconds)).build();
        VeldResponse answer = Router.problem(d.sig(), "TooManyRequests", 429, spec.sig(), data);
        Map<String, String> headers = new LinkedHashMap<>(answer.headers());
        headers.put("retry-after", Long.toString(seconds));
        return new VeldResponse(429, headers, answer.body());
      }
    }
    Json data = Json.object().put("retryAfter", Json.of(seconds)).build();
    VeldResponse answer = Router.problem("about:blank", "TooManyRequests", 429, spec.sig(), data);
    Map<String, String> headers = new LinkedHashMap<>(answer.headers());
    headers.put("retry-after", Long.toString(seconds));
    return new VeldResponse(429, headers, answer.body());
  }

  /** The servers' check at start-up: a hook an endpoint's limit needs. */
  static void assertHooks(HandlerHooks hooks, EndpointSpec spec) {
    Limit limit = hooks.limit(spec.sig());
    if (limit == null) {
      return;
    }
    if (limit.by == By.USER && hooks.limitCaller() == null) {
      throw new IllegalStateException(spec.name() + ": hooks.caller is required");
    }
    if (limit.by == By.IP && hooks.address() == null) {
      throw new IllegalStateException(spec.name() + ": hooks.address is required");
    }
  }
  // #endif
}
