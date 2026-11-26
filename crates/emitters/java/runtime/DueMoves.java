package dev.veld.runtime;

import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * The due-moves runner (spec/platform.md section 10.1). The contract keeps no timers: the
 * application's store knows which values have waited long enough, a {@link Loader} asks it, and
 * every answer runs through steps 4 to 10 of the ordinary pipeline, in process, as a call of the
 * timed endpoint. Step 2 is replaced by the caller {@code hooks.timer} gives, and no middleware
 * runs, because there is no request.
 *
 * <p>The guard of a transition is not an atomic guarantee, and neither is a run: two runs, on one
 * machine or two, are as safe as the handler's conditional write is.
 */
public final class DueMoves {
  private DueMoves() {}

  /** Marks a request made by the runner (the {@code raw} of its {@link VeldRequest}) and carries the timer's caller. */
  public record Call(Object principal) {}

  /**
   * What happened to the calls of one endpoint.
   *
   * @param ran the calls that were made
   * @param moved the calls answered below 300
   * @param refused the calls answered {@code 409 InvalidTransition}: another request moved the value first
   * @param failed the others: a failing handler, a loader or a call that threw, another status
   */
  public record Entry(int ran, int moved, int refused, int failed) {}

  /** Asks the application's store for the calls of one endpoint that are due, in wire form. */
  @FunctionalInterface
  public interface Loader {
    /**
     * Returns the parameters of up to {@code limit} calls whose value entered the source state at or
     * before {@code before}.
     *
     * @throws Exception when the store fails: it counts as one failure and goes to {@code onError}
     */
    List<Json.Obj> load(Instant before, int limit, RequestContext ctx) throws Exception;
  }

  /** The caller of the timer. */
  @FunctionalInterface
  public interface Principal {
    /** Returns the principal the policies judge, as a caller that authenticated. */
    Object principal() throws Exception;
  }

  /** The context a loader gets: the endpoint's method and path, no headers. */
  public static RequestContext context(EndpointSpec spec) {
    return new RequestContext(spec.method(), spec.path(), Map.of(), null);
  }

  /**
   * Runs the due calls of one endpoint.
   *
   * @param router the handler built with the clock of the run
   * @param now the clock of the run, epoch milliseconds
   * @param afterMs the wait of the timed edge
   * @param limit the most calls to run
   * @param timer the caller of the timer; {@code null} when the application gave none
   */
  public static Entry endpoint(
      VeldHandler router, HandlerHooks hooks, EndpointSpec spec, long now, long afterMs, int limit, Principal timer, Loader loader) {
    RequestContext context = context(spec);
    List<Json.Obj> calls;
    try {
      calls = loader.load(Instant.ofEpochMilli(now - afterMs), limit, context);
    } catch (Exception | StackOverflowError e) {
      report(hooks, e, context);
      return new Entry(0, 0, 0, 1);
    }
    int ran = 0;
    int moved = 0;
    int refused = 0;
    int failed = 0;
    for (Json.Obj wire : calls.subList(0, Math.min(limit, calls.size()))) {
      ran++;
      try {
        Object who = spec.auth() == null || timer == null ? null : timer.principal();
        Optional<VeldResponse> answer = router.invoke(spec.name(), request(spec, wire, who));
        if (answer.isEmpty()) {
          throw new IllegalStateException(spec.name() + ": a due call does not fit the path of the endpoint");
        }
        VeldResponse response = answer.get();
        if (response.status() < 300) {
          moved++;
        } else if (response.status() == 409 && invalidTransition(response)) {
          refused++;
        } else {
          failed++;
          if (response.status() < 500) {
            report(hooks, new IllegalStateException(spec.name() + ": a due move was answered with status " + response.status()), context);
          }
        }
      } catch (Exception | StackOverflowError e) {
        failed++;
        report(hooks, e, context);
      }
    }
    return new Entry(ran, moved, refused, failed);
  }

  private static boolean invalidTransition(VeldResponse response) {
    try {
      Json type = Json.parse(response.body()).asObject().get("type");
      return type instanceof Json.Str s && s.value().contains("/InvalidTransition@");
    } catch (RuntimeException e) {
      return false;
    }
  }

  private static void report(HandlerHooks hooks, Throwable error, RequestContext context) {
    try {
      if (hooks.onError() != null) {
        hooks.onError().onError(error, context);
      }
    } catch (RuntimeException ignored) {
      // A failing hook must not stop the run.
    }
  }

  // -- the request ---------------------------------------------------------------------------

  /** The HTTP request the parameters of a call stand for: the router binds it as it binds any other. */
  static VeldRequest request(EndpointSpec spec, Json.Obj wire, Object principal) {
    String path = spec.path();
    List<String> query = new ArrayList<>();
    Map<String, List<String>> headers = new LinkedHashMap<>();
    String body = null;
    for (EndpointSpec.Param p : spec.params()) {
      Json value = p.stream() ? null : wire.get(p.name());
      if (value == null || value instanceof Json.Null) {
        continue;
      }
      switch (p.binding()) {
        case PATH -> path = path.replace("{" + p.name() + "}", encode(text(value)));
        case QUERY -> addQuery(query, p.name(), value);
        case QUERY_OBJECT -> {
          if (value instanceof Json.Obj obj) {
            for (Map.Entry<String, Json> field : obj.fields().entrySet()) {
              addQuery(query, field.getKey(), field.getValue());
            }
          }
        }
        case HEADER -> headers.put(p.header() == null ? p.name() : p.header(), List.of(text(value)));
        case BODY -> {
          body = value.write();
          headers.put("content-type", List.of("application/json"));
        }
      }
    }
    return new VeldRequest(spec.method(), path, String.join("&", query), headers, body, new Call(principal));
  }

  private static void addQuery(List<String> query, String name, Json value) {
    if (value instanceof Json.Arr list) {
      for (Json item : list.items()) {
        query.add(encode(name) + "=" + encode(text(item)));
      }
    } else if (!(value instanceof Json.Null)) {
      query.add(encode(name) + "=" + encode(text(value)));
    }
  }

  private static String text(Json value) {
    return value instanceof Json.Str s ? s.value() : value.write();
  }

  private static String encode(String text) {
    return URLEncoder.encode(text, StandardCharsets.UTF_8).replace("+", "%20");
  }
}
