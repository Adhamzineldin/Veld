package dev.veld.runtime;

import java.nio.charset.StandardCharsets;
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

    Principal(Class<?> type, Function<Object, Json> wire, String tenant) {
      this.type = type;
      this.wire = wire;
      this.tenant = tenant;
    }

    /** The wire form of {@code caller} when it is a value of this model, else {@code null}. */
    Json wireOf(Object caller) {
      return type.isInstance(caller) ? wire.apply(caller) : null;
    }
  }

  /**
   * The principal's model of a service.
   *
   * @param type the generated class of the model
   * @param wire the wire form of a value of it ({@code toWire})
   * @param tenant the wire name of its {@code @tenant} field, or {@code null}
   */
  public static <P> Principal principal(Class<P> type, Function<P, Json> wire, String tenant) {
    return new Principal(type, caller -> wire.apply(type.cast(caller)), tenant);
  }

  /** What one request keeps for the platform features. */
  static final class State {
    private volatile RequestContext ctx;
    private volatile long now;
    private volatile boolean timed;
    // #if visibility
    private final TreeSet<String> hidden = new TreeSet<>(BYTES);
    // #endif

    /** The clock reading of the request, in epoch milliseconds. */
    long now() {
      return now;
    }

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

  private static IllegalStateException shape(String path, String expected) {
    return new IllegalStateException("visibility: the value at `" + (path.isEmpty() ? "$" : path) + "` is not " + expected);
  }
  // #endif

  // #if shapes

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
    // #endif
    // #if visibility
    List<String> dropped = new ArrayList<>();
    data = showInto(declared.schema(), data, ctx, dropped);
    if (dropped.isEmpty()) {
      return response;
    }
    state.hide(dropped);
    // #endif
    // #if shapes
    return new VeldResponse(response.status(), response.headers(), body.with("data", data).write());
  }

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
    // #if shapes
    return false;
  }
  // #endif
}
