package dev.veld.runtime;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
// veld:sse
import java.util.stream.Stream;
// veld:end

/**
 * The router core. Routing is done once, from the endpoint paths: matching is exact on literal
 * segments, parameters are extracted, literal routes beat parameterised ones and HEAD is not
 * implicit.
 *
 * <p>The pipeline of a request is: match, authenticate (401), middleware in declared order, bind
 * and validate path, query, headers and body (422), call the implementation, optionally validate
 * the response, serialise with the declared success status. A declared error is answered at its
 * declared status; anything else, including an undeclared {@link VeldError}, becomes a 500
 * {@code Internal} without a message, and the original goes to the {@code onError} hook.
 */
public final class Router implements VeldHandler {

  /** Signatures of the prelude errors the router itself raises. */
  public record StdSigs(String unauthorized, String validation, String internal) {}

  private record Route(ServerEndpoint endpoint, Pattern pattern, List<String> names) {}

  private static final Pattern BEARER = Pattern.compile("^Bearer\\s+(\\S+)$", Pattern.CASE_INSENSITIVE);
  private static final Pattern BASIC = Pattern.compile("^Basic\\s+(\\S+)$", Pattern.CASE_INSENSITIVE);
  private static final Pattern NUMBER = Pattern.compile("^-?\\d+(\\.\\d+)?([eE][+-]?\\d+)?$");
  private static final Pattern PLACEHOLDER = Pattern.compile("\\{([^}]+)}");

  private final List<Route> routes;
  private final StdSigs std;
  private final HandlerHooks hooks;
  // #if conditional
  private Map<String, Conditions> conditions = Map.of();
  // #endif

  private Router(List<Route> routes, StdSigs std, HandlerHooks hooks) {
    this.routes = routes;
    this.std = std;
    this.hooks = hooks;
  }

  /**
   * Builds a handler from endpoint descriptions.
   *
   * @throws IllegalStateException when a hook an endpoint needs was not provided
   */
  public static VeldHandler create(List<ServerEndpoint> endpoints, StdSigs std, HandlerHooks hooks) {
    // #if conditional
    return create(endpoints, Map.of(), std, hooks);
  }

  /**
   * Builds a handler from endpoint descriptions and the conditional headers of the endpoints that
   * have them, by operation signature.
   *
   * @throws IllegalStateException when a hook an endpoint needs was not provided
   */
  public static VeldHandler create(
      List<ServerEndpoint> endpoints, Map<String, Conditions> conditions, StdSigs std, HandlerHooks hooks) {
    assertConditions(endpoints, conditions, hooks);
    // #endif
    assertHooks(endpoints, hooks);
    // #if rules
    RuleWalk.announce(hooks.rulesOutput());
    if (!hooks.validateRequests()) {
      RuleWalk.announceUnvalidated(endpoints);
    }
    // #endif
    List<Route> routes = new ArrayList<>();
    for (ServerEndpoint e : endpoints) {
      routes.add(compile(e));
    }
    routes.sort((a, b) -> Integer.compare(a.names().size(), b.names().size()));
    // #if conditional
    Router router = new Router(List.copyOf(routes), std, hooks);
    router.conditions = Map.copyOf(conditions);
    return router;
    // #endif
    // #if plain
    return new Router(List.copyOf(routes), std, hooks);
    // #endif
  }
  // #if conditional

  /** An {@code @idempotent} endpoint needs the store, and the caller hook when its scope has one. */
  private static void assertConditions(
      List<ServerEndpoint> endpoints, Map<String, Conditions> conditions, HandlerHooks hooks) {
    for (ServerEndpoint e : endpoints) {
      Conditions c = conditions.get(e.spec().sig());
      if (c == null || c.idempotency() == null) {
        continue;
      }
      if (hooks.idempotencyStore() == null) {
        throw new IllegalStateException(e.spec().name() + ": the idempotency store is required");
      }
      if (c.idempotency().caller() && hooks.idempotencyCaller() == null) {
        throw new IllegalStateException(e.spec().name() + ": the idempotency caller hook is required");
      }
    }
  }
  // #endif

  private static void assertHooks(List<ServerEndpoint> endpoints, HandlerHooks hooks) {
    for (ServerEndpoint e : endpoints) {
      EndpointSpec.Auth auth = e.spec().auth();
      if (auth != null && !hasAuthHook(auth.kind(), hooks)) {
        throw new IllegalStateException(e.spec().name() + ": the " + auth.kind().name().toLowerCase(Locale.ROOT)
            + " auth hook is required");
      }
      for (String name : e.middleware()) {
        if (hooks.middleware(name) == null) {
          throw new IllegalStateException(e.spec().name() + ": middleware " + name + " is required");
        }
      }
      // #if flags
      if (!hooks.flagsOf(e.spec().sig()).isEmpty() && hooks.flagSource() == null) {
        throw new IllegalStateException(e.spec().name() + ": hooks.flags is required");
      }
      // #endif
    }
  }

  private static boolean hasAuthHook(EndpointSpec.AuthKind kind, HandlerHooks hooks) {
    return switch (kind) {
      case BEARER -> hooks.bearer() != null;
      case BASIC -> hooks.basic() != null;
      case API_KEY -> hooks.apiKey() != null;
    };
  }

  private static Route compile(ServerEndpoint endpoint) {
    List<String> names = new ArrayList<>();
    StringBuilder re = new StringBuilder("^");
    String path = normalizePath(endpoint.spec().path());
    Matcher m = PLACEHOLDER.matcher(path);
    int last = 0;
    while (m.find()) {
      re.append(Pattern.quote(path.substring(last, m.start())));
      re.append("([^/]+)");
      names.add(m.group(1));
      last = m.end();
    }
    re.append(Pattern.quote(path.substring(last))).append('$');
    return new Route(endpoint, Pattern.compile(re.toString()), names);
  }

  private static String normalizePath(String p) {
    return p.length() > 1 && p.endsWith("/") ? p.substring(0, p.length() - 1) : p;
  }

  @Override
  public Optional<VeldResponse> handle(VeldRequest request) {
    String method = request.method().toUpperCase(Locale.ROOT);
    String path = normalizePath(request.path());
    for (Route r : routes) {
      if (!r.endpoint().spec().method().equals(method)) {
        continue;
      }
      Matcher m = r.pattern().matcher(path);
      if (m.matches()) {
        return Optional.of(run(r, m, request));
      }
    }
    return Optional.empty();
  }

  @Override
  public Optional<VeldResponse> invoke(String endpoint, VeldRequest request) {
    String path = normalizePath(request.path());
    for (Route r : routes) {
      if (r.endpoint().spec().name().equals(endpoint)) {
        Matcher m = r.pattern().matcher(path);
        return m.matches() ? Optional.of(run(r, m, request)) : Optional.empty();
      }
    }
    return Optional.empty();
  }

  // -- pipeline ------------------------------------------------------------------------------

  // #if slo
  private VeldResponse run(Route route, Matcher match, VeldRequest request) {
    long started = System.nanoTime();
    // #endif
    // #if slo_timeouts
    VeldResponse response = bounded(route, match, request);
    // #endif
    // #if slo_only
    VeldResponse response = pipeline(route, match, request);
    // #endif
    // #if slo
    measure(route.endpoint().spec(), response, started);
    return response;
  }

  /** The metrics hook, once per request that matched an endpoint, with the answer that is ready (spec/platform.md section 13). */
  private void measure(EndpointSpec spec, VeldResponse response, long started) {
    HandlerHooks.MetricsHook hook = hooks.metrics();
    if (hook == null) {
      return;
    }
    try {
      Map<String, Object> attributes = new LinkedHashMap<>();
      attributes.put("http.request.method", spec.method());
      attributes.put("http.route", spec.path());
      attributes.put("http.response.status_code", response.status());
      if (response.status() >= 500) {
        attributes.put("error.type", Integer.toString(response.status()));
      }
      String sig = spec.sig();
      attributes.put("veld.service", sig.substring("veld:".length(), sig.lastIndexOf('.')));
      attributes.put("veld.endpoint", spec.name());
      attributes.put("veld.operation", sig);
      hook.metrics(new Metric("http.server.request.duration", "s", (System.nanoTime() - started) / 1e9, attributes));
    } catch (RuntimeException | StackOverflowError ignored) {
      // A failing hook must not change the response.
    }
  }

  // #endif
  // #if timeouts
  private static final java.util.concurrent.ExecutorService WORKERS =
      java.util.concurrent.Executors.newCachedThreadPool(task -> {
        Thread worker = new Thread(task, "veld-request");
        worker.setDaemon(true);
        return worker;
      });

  /**
   * The deadline of an endpoint (spec/platform.md section 12). A request that has one runs on a
   * worker thread, which cannot be stopped: when the deadline passes first the answer is {@code 504
   * DeadlineExceeded} with {@code timeoutMs}, the answer the pipeline gives later is dropped, its
   * idempotency key is settled at the deadline and the worker's thread is interrupted once. A
   * deadline is a declared error, not a failure: nothing goes to {@code onError}.
   */
  // #endif
  // #if slo_timeouts
  private VeldResponse bounded(Route route, Matcher match, VeldRequest request) {
  // #endif
  // #if timeouts_only
  private VeldResponse run(Route route, Matcher match, VeldRequest request) {
  // #endif
  // #if timeouts
    EndpointSpec spec = route.endpoint().spec();
    long ms = hooks.timeoutMs(spec.sig());
    if (ms <= 0) {
      return pipeline(route, match, request);
    }
    Deadline bound = new Deadline(ms);
    java.util.concurrent.Future<VeldResponse> work = WORKERS.submit(() -> {
      bound.enter();
      try {
        return pipeline(route, match, request);
      } finally {
        Deadline.leave();
      }
    });
    try {
      return work.get(ms, java.util.concurrent.TimeUnit.MILLISECONDS);
    } catch (java.util.concurrent.TimeoutException e) {
      bound.fire();
      work.cancel(true);
      return deadlineExceeded(spec, ms);
    } catch (java.util.concurrent.ExecutionException e) {
      if (e.getCause() instanceof RuntimeException runtime) {
        throw runtime;
      }
      if (e.getCause() instanceof Error error) {
        throw error;
      }
      throw new IllegalStateException(e.getCause());
    } catch (InterruptedException e) {
      bound.fire();
      work.cancel(true);
      Thread.currentThread().interrupt();
      return problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
    }
  }

  private VeldResponse deadlineExceeded(EndpointSpec spec, long ms) {
    for (EndpointSpec.Declared d : spec.errors()) {
      if (d.status() == 504) {
        Json data = Json.object().put("timeoutMs", Json.of(Long.toString(ms))).build();
        return problem(d.sig(), "DeadlineExceeded", 504, spec.sig(), data);
      }
    }
    return problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
  }
  // #endif
  // #if wrapped

  private VeldResponse pipeline(Route route, Matcher match, VeldRequest request) {
  // #endif
  // #if no_wrap
  private VeldResponse run(Route route, Matcher match, VeldRequest request) {
  // #endif
    ServerEndpoint ep = route.endpoint();
    EndpointSpec spec = ep.spec();
    RequestContext ctx = new RequestContext(spec.method(), normalizePath(request.path()), lowerHeaders(request.headers()), request.raw());
    // #if timeouts
    ctx.setDeadline(Deadline.current());
    // #endif
    // #if flags
    Flags.View enclosing = Flags.current();
    // #endif
    try {
      Guarded guarded = guard(route, match, request, ctx, false);
      if (guarded.failure != null) {
        return guarded.failure;
      }
      Json.Obj params = new Json.Obj(guarded.bound.values);
      // veld:sse
      if (spec.stream()) {
        return startSse(ep, ep.sse().call(params, ctx), ctx);
      }
      // veld:end
      // #if conditional
      Conditions found = conditions.get(spec.sig());
      String key = found == null ? null : found.key(ctx);
      if (key != null) {
        return idempotent(route, found, key, guarded.bound, ctx, params);
      }
      // #endif
      return success(ep, ep.call().call(params, ctx));
    } catch (Exception | StackOverflowError err) {
      return failure(err, spec, ctx);
      // #if flags
    } finally {
      Flags.restore(enclosing);
      // #endif
    }
  }
  // #if conditional

  /**
   * Step 5b: a request with an idempotency key asks the store before anything else runs. A new key
   * runs the rest of the pipeline and keeps its answer when it is below 500; a key with an answer
   * sends it again; a key that is running or was used for other parameters is {@code 409}.
   */
  private VeldResponse idempotent(
      Route route, Conditions conditions, String key, Bound bound, RequestContext ctx, Json.Obj params)
      throws Exception {
    ServerEndpoint ep = route.endpoint();
    EndpointSpec spec = ep.spec();
    Conditions.Idempotency idem = conditions.idempotency();
    final String scope;
    if (idem.caller()) {
      String who = hooks.idempotencyCaller().caller(ctx.principal(), ctx);
      if (who == null) {
        throw new IllegalStateException(spec.name() + ": the idempotency caller hook returned no text");
      }
      scope = spec.sig() + "\n" + who;
    } else {
      scope = spec.sig();
    }
    IdempotencyStore store = hooks.idempotencyStore();
    IdempotencyStore.Begin begun = store.begin(scope, key, fingerprint(bound));
    if (begun instanceof IdempotencyStore.Begin.Replay replay) {
      return replayed(replay.answer());
    }
    if (!(begun instanceof IdempotencyStore.Begin.Started)) {
      String message = begun instanceof IdempotencyStore.Begin.Running
          ? Conditions.RUNNING_MESSAGE
          : Conditions.OTHER_MESSAGE;
      Json data = Json.object().put("message", Json.of(message)).build();
      return problem(idem.conflictSig(), "Conflict", idem.conflictStatus(), spec.sig(), data);
    }
    // #endif
    // #if timeouts_conditional
    Deadline deadline = Deadline.current();
    if (deadline != null) {
      // A request that hits its deadline settles its key at the deadline; what it settles later is a no-op.
      deadline.lease(() -> {
        try {
          store.release(scope, key);
        } catch (Exception | StackOverflowError err) {
          observe(err, ctx);
        }
      });
    }
    // #endif
    // #if conditional
    boolean settled = false;
    try {
      VeldResponse response;
      try {
        response = success(ep, ep.call().call(params, ctx));
      } catch (Exception | StackOverflowError err) {
        response = failure(err, spec, ctx);
      }
      settle(store, scope, key, response, ctx);
      settled = true;
      return response;
    } finally {
      // #endif
      // #if timeouts_conditional
      if (!settled && Deadline.current() != null && !Deadline.current().claim()) {
        settled = true;
      }
      // #endif
      // #if conditional
      if (!settled) {
        try {
          store.release(scope, key);
        } catch (Exception | StackOverflowError err) {
          observe(err, ctx);
        }
      }
    }
  }

  /** Keeps the answer when it is below 500 and forgets the key otherwise; a failing store is only reported. */
  private void settle(IdempotencyStore store, String scope, String key, VeldResponse response, RequestContext ctx) {
    // #endif
    // #if timeouts_conditional
    if (Deadline.current() != null && !Deadline.current().claim()) {
      return;
    }
    // #endif
    // #if conditional
    try {
      if (response.status() < 500) {
        store.complete(scope, key, stored(response));
      } else {
        store.release(scope, key);
      }
    } catch (Exception | StackOverflowError err) {
      observe(err, ctx);
    }
  }

  private void observe(Throwable err, RequestContext ctx) {
    try {
      if (hooks.onError() != null) {
        hooks.onError().onError(err, ctx);
      }
    } catch (RuntimeException ignored) {
      // A failing hook must not change the response.
    }
  }

  private static StoredAnswer stored(VeldResponse response) {
    byte[] body = response.body() == null ? new byte[0] : response.body().getBytes(StandardCharsets.UTF_8);
    Map<String, String> headers = response.headers();
    return new StoredAnswer(
        response.status(), body, headers.get("content-type"), headers.get("etag"), headers.get("location"));
  }

  /** A stored answer sent again, marked with {@code Veld-Idempotent-Replay: true}. */
  private static VeldResponse replayed(StoredAnswer answer) {
    Map<String, String> headers = new LinkedHashMap<>();
    if (answer.contentType() != null) {
      headers.put("content-type", answer.contentType());
    }
    if (answer.etag() != null) {
      headers.put("etag", answer.etag());
    }
    if (answer.location() != null) {
      headers.put("location", answer.location());
    }
    headers.put("veld-idempotent-replay", "true");
    String body = answer.body().length == 0 ? null : new String(answer.body(), StandardCharsets.UTF_8);
    return new VeldResponse(answer.status(), headers, body);
  }

  /** The first 16 hex characters of the SHA-256 of the canonical JSON of the bound parameters. */
  private static String fingerprint(Bound bound) {
    try {
      byte[] digest = java.security.MessageDigest.getInstance("SHA-256")
          .digest(new Json.Obj(bound.canonical).canonical().getBytes(StandardCharsets.UTF_8));
      StringBuilder out = new StringBuilder();
      for (int i = 0; i < 8; i++) {
        out.append(Character.forDigit((digest[i] >> 4) & 0xf, 16)).append(Character.forDigit(digest[i] & 0xf, 16));
      }
      return out.toString();
    } catch (java.security.NoSuchAlgorithmException e) {
      throw new IllegalStateException("SHA-256 is not available", e);
    }
  }
  // #endif

  /** The outcome of the part of the pipeline every transport shares: a response, or the bound values. */
  private static final class Guarded {
    VeldResponse failure;
    Bound bound;
  }

  /** Authentication, middleware, binding and validation. */
  private Guarded guard(Route route, Matcher match, VeldRequest request, RequestContext ctx, boolean ws)
      throws Exception {
    EndpointSpec spec = route.endpoint().spec();
    Guarded out = new Guarded();
    // #if derived
    Derive.take();
    // #endif
    // #if timed
    boolean timer = request.raw() instanceof DueMoves.Call;
    if (timer) {
      // The due-moves runner: step 2 is replaced by the caller of the timer, and no middleware runs.
      if (spec.auth() != null) {
        ctx.setPrincipal(((DueMoves.Call) request.raw()).principal());
        if (ctx.principal() == null) {
          out.failure = problem(std.unauthorized(), "Unauthorized", 401, spec.sig(), Json.object().build());
          return out;
        }
      }
    }
    // #endif
    // #if timed
    if (!timer) {
    // #endif
    if (spec.auth() != null) {
      ctx.setPrincipal(authenticate(spec.auth(), ctx));
      if (ctx.principal() == null) {
        out.failure = problem(std.unauthorized(), "Unauthorized", 401, spec.sig(), Json.object().build());
        return out;
      }
    }
    for (String name : route.endpoint().middleware()) {
      hooks.middleware(name).run(ctx);
    }
    // #if timed
    }
    // #endif
    out.bound = bind(route, match, request, ctx, ws, hooks.validateRequests());
    // #if conditional
    Conditions held = conditions.get(spec.sig());
    List<Issue> header = held == null ? List.of() : held.issues(ctx);
    out.bound.issues.addAll(0, header);
    // #endif
    if (!out.bound.issues.isEmpty()) {
      out.failure = problem(std.validation(), "ValidationError", 422, spec.sig(), issuesData(out.bound.issues));
    }
    // #if flags
    if (out.failure == null) {
      readFlags(spec, ctx);
    }
    // #endif
    // #if rules
    if (out.failure == null && hooks.validateRequests()) {
      out.failure = RuleWalk.inputFailure(spec, hooks, out.bound.values, std);
    }
    // #endif
    // #if conditional_rules
    if (!header.isEmpty() && out.bound.issues.size() == header.size() && hooks.validateRequests()) {
      VeldResponse merged = RuleWalk.inputFailure(spec, hooks, out.bound.values, std, header);
      if (merged != null) {
        out.failure = merged;
      }
    }
    // #endif
    // #if derived
    if (out.failure == null && !hooks.validateRequests()) {
      out.failure = RuleWalk.derivedFailure(spec, hooks, std);
    }
    Derive.take();
    // #endif
    return out;
  }
  // #if flags

  /**
   * Reads every flag the endpoint can reach once, after authentication, and puts the readings in
   * force for the rest of the request (spec/platform.md section 11). A provider that fails makes that
   * flag a failed reading and goes to {@code onError}.
   */
  private void readFlags(EndpointSpec spec, RequestContext ctx) {
    java.util.List<String> names = hooks.flagsOf(spec.sig());
    if (names.isEmpty()) {
      return;
    }
    Flags.View view = Flags.read(
        names,
        hooks.flagSource(),
        ctx,
        new Flags.Context(ctx.principal(), null, spec.sig()),
        error -> {
          try {
            if (hooks.onError() != null) {
              hooks.onError().onError(error, ctx);
            }
          } catch (RuntimeException ignored) {
            // A failing hook must not change the response.
          }
        });
    ctx.setFlags(view);
    Flags.install(view);
  }
  // #endif
  // veld:sse

  /**
   * Pulls the first event before answering, so that an error raised before any event is a normal
   * problem response. After that the status is final and a failure becomes the last event.
   */
  private VeldResponse startSse(ServerEndpoint ep, Stream<SseEvent<Json>> source, RequestContext ctx) {
    EndpointSpec spec = ep.spec();
    Stream<String> frames = SseFrames.open(
        source,
        // #if flags
        event -> Flags.with(ctx.flags(), () -> sseFrame(spec, event)),
        err -> Flags.with(ctx.flags(), () -> SseEvent.frame(failure(err, spec, ctx).body(), null, "error", null)));
        // #endif
        // #if no_flags
        event -> sseFrame(spec, event),
        err -> SseEvent.frame(failure(err, spec, ctx).body(), null, "error", null));
        // #endif
    Map<String, String> headers = Map.of(
        "content-type", "text/event-stream", "cache-control", "no-cache", "x-accel-buffering", "no");
    return new VeldResponse(spec.status(), headers, null, frames);
  }

  private String sseFrame(EndpointSpec spec, SseEvent<Json> event) {
    if (spec.success() == null) {
      throw new IllegalStateException(spec.name() + ": a stream needs an item type");
    }
    Json wire = Wire.prepare(spec.success(), event.data(), Wire.Mode.RESPONSE);
    // #if derived
    wire = Derive.output(spec.success(), wire);
    // #endif
    if (hooks.validateResponses()) {
      List<Issue> issues = Wire.validate(spec.success(), wire, "$", false);
      if (!issues.isEmpty()) {
        throw new IllegalStateException(spec.name() + ": stream item failed validation: " + issues);
      }
    }
    // #if rules
    RuleWalk.checkItem(spec, hooks, wire);
    // #endif
    return SseEvent.frame(Wire.canonicalOf(spec.success(), wire), event.id(), event.event(), event.retry());
  }
  // veld:end

  // veld:ws
  @Override
  public boolean ws(VeldRequest request, WsSocket socket) {
    String path = normalizePath(request.path());
    for (Route r : routes) {
      if (!r.endpoint().spec().method().equals("WS")) {
        continue;
      }
      Matcher m = r.pattern().matcher(path);
      if (m.matches()) {
        startWs(r, m, request, socket);
        return true;
      }
    }
    return false;
  }

  private void startWs(Route route, Matcher match, VeldRequest request, WsSocket socket) {
    WsConnection conn = new WsConnection(
            route.endpoint().spec(), socket, hooks.validateResponses(), hooks.validateRequests(), std);
    // #if rules
    conn.rules(hooks);
    // #endif
    conn.attach();
    Thread thread = new Thread(() -> runWs(route, match, request, conn), "veld-ws");
    thread.setDaemon(true);
    thread.start();
  }

  private void runWs(Route route, Matcher match, VeldRequest request, WsConnection conn) {
    ServerEndpoint ep = route.endpoint();
    EndpointSpec spec = ep.spec();
    RequestContext ctx = new RequestContext(
        "WS", normalizePath(request.path()), wsHeaders(spec, request), request.raw());
    // #if flags
    Flags.View enclosing = Flags.current();
    // #endif
    try {
      Guarded guarded = guard(route, match, request, ctx, true);
      if (guarded.failure != null) {
        conn.fail(guarded.failure);
        return;
      }
      // #if flags
      conn.flags(Flags.current());
      // #endif
      conn.ready();
      ep.ws().call(new Json.Obj(guarded.bound.values), ctx, conn);
      conn.finish(1000, "done");
    } catch (Exception | StackOverflowError err) {
      conn.fail(failure(err, spec, ctx));
      // #if flags
    } finally {
      Flags.restore(enclosing);
      // #endif
    }
  }

  /** The headers of a ws upgrade: `Authorization` from `access_token`, the API key from `api_key`. */
  private static Map<String, List<String>> wsHeaders(EndpointSpec spec, VeldRequest request) {
    Map<String, List<String>> headers = new LinkedHashMap<>(lowerHeaders(request.headers()));
    Map<String, List<String>> query = parseQuery(request.query());
    EndpointSpec.Auth auth = spec.auth();
    if (auth == null) {
      return Collections.unmodifiableMap(headers);
    }
    if (auth.kind() == EndpointSpec.AuthKind.BEARER && !headers.containsKey("authorization")) {
      List<String> token = query.get("access_token");
      if (token != null && !token.isEmpty()) {
        headers.put("authorization", List.of("Bearer " + token.get(0)));
      }
    }
    String keyHeader = (auth.header() == null ? "X-API-Key" : auth.header()).toLowerCase(Locale.ROOT);
    if (auth.kind() == EndpointSpec.AuthKind.API_KEY && !headers.containsKey(keyHeader)) {
      List<String> key = query.get("api_key");
      if (key != null && !key.isEmpty()) {
        headers.put(keyHeader, List.of(key.get(0)));
      }
    }
    return Collections.unmodifiableMap(headers);
  }
  // veld:end

  private static Json issuesData(List<Issue> issues) {
    List<Json> items = new ArrayList<>();
    for (Issue i : issues) {
      items.add(Json.object()
          .put("path", Json.of(i.path()))
          .put("code", Json.of(i.code()))
          .put("message", Json.of(i.message()))
          .build());
    }
    return Json.object().put("issues", Json.array(items)).build();
  }

  /**
   * The wire form of a list of issues, as the {@code data} of a {@code ValidationError}. Used by
   * clients to build their local validation error.
   */
  public static Json issuesToWire(List<Issue> issues) {
    return issuesData(issues);
  }

  private static Map<String, List<String>> lowerHeaders(Map<String, List<String>> headers) {
    Map<String, List<String>> out = new LinkedHashMap<>();
    if (headers != null) {
      for (Map.Entry<String, List<String>> e : headers.entrySet()) {
        if (e.getKey() != null) {
          out.put(e.getKey().toLowerCase(Locale.ROOT), e.getValue());
        }
      }
    }
    return Collections.unmodifiableMap(out);
  }

  private Object authenticate(EndpointSpec.Auth auth, RequestContext ctx) throws Exception {
    switch (auth.kind()) {
      case BEARER: {
        Matcher m = BEARER.matcher(orEmpty(ctx.header("authorization")));
        return m.matches() ? hooks.bearer().authenticate(m.group(1), ctx) : null;
      }
      case BASIC: {
        Matcher m = BASIC.matcher(orEmpty(ctx.header("authorization")));
        String decoded = m.matches() ? base64Text(m.group(1)) : null;
        int i = decoded == null ? -1 : decoded.indexOf(':');
        return i < 0 ? null : hooks.basic().authenticate(decoded.substring(0, i), decoded.substring(i + 1), ctx);
      }
      default: {
        String key = ctx.header(auth.header() == null ? "X-API-Key" : auth.header());
        return key == null ? null : hooks.apiKey().authenticate(key, ctx);
      }
    }
  }

  private static String orEmpty(String s) {
    return s == null ? "" : s;
  }

  private static String base64Text(String s) {
    try {
      byte[] bytes = Base64.getDecoder().decode(s);
      return StandardCharsets.UTF_8.newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(bytes))
          .toString();
    } catch (IllegalArgumentException | CharacterCodingException e) {
      return null;
    }
  }

  private VeldResponse success(ServerEndpoint ep, Json result) {
    EndpointSpec spec = ep.spec();
    if (spec.success() == null) {
      return new VeldResponse(spec.status(), Map.of(), null);
    }
    if (result == null) {
      throw new IllegalStateException(spec.name() + ": implementation returned no value");
    }
    Json wire = Wire.prepare(spec.success(), result, Wire.Mode.RESPONSE);
    // #if derived
    wire = Derive.output(spec.success(), wire);
    // #endif
    // #if rules
    VeldResponse broken = RuleWalk.outputFailure(spec, hooks, spec.success(), wire, true, std);
    if (broken != null) {
      return broken;
    }
    // #endif
    // #if endpoint
    VeldResponse unmet = EndpointRules.ensureFailure(spec, hooks, wire, std);
    if (unmet != null) {
      return unmet;
    }
    // #endif
    if (hooks.validateResponses()) {
      List<Issue> issues = Wire.validate(spec.success(), wire, "$", false);
      if (!issues.isEmpty()) {
        throw new IllegalStateException(spec.name() + ": response failed validation: " + issues);
      }
    }
    // #if conditional
    Map<String, String> headers = new LinkedHashMap<>();
    headers.put("content-type", "application/json");
    Conditions owed = conditions.get(spec.sig());
    String etag = owed == null ? null : owed.etagOf(wire);
    if (etag != null) {
      headers.put("etag", etag);
    }
    return new VeldResponse(spec.status(), headers, wire.write());
    // #endif
    // #if plain
    return new VeldResponse(spec.status(), Map.of("content-type", "application/json"), wire.write());
    // #endif
  }

  private VeldResponse failure(Throwable err, EndpointSpec spec, RequestContext ctx) {
    // #if endpoint
    EndpointRules.discard();
    if (err instanceof EndpointRules.Early early) {
      return early.response();
    }
    // #endif
    // #if early
    if (err instanceof Conditions.Early early) {
      return early.response();
    }
    // #endif
    // #if rules
    if (err instanceof RuleWalk.Rejected) {
      // #endif
      // #if derived
      RuleWalk.report(
          hooks, spec, RuleWalk.Position.OUTPUT, RuleWalk.Outcome.REJECTED, ((RuleWalk.Rejected) err).violations(), System.nanoTime());
      // #endif
      // #if rules
      return problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
    }
    // #endif
    if (err instanceof VeldError ve) {
      for (EndpointSpec.Declared d : spec.errors()) {
        if (d.sig().equals(ve.sig())) {
          ve.setOp(spec.sig());
          Json data = Wire.prepare(d.schema(), ve.dataWire(), Wire.Mode.RESPONSE);
          // #if derived
          data = Derive.output(d.schema(), data);
          // #endif
          // #if rules
          VeldResponse broken = RuleWalk.outputFailure(spec, hooks, d.schema(), data, false, std);
          if (broken != null) {
            return broken;
          }
          // #endif
          return problem(d.sig(), ve.title(), d.status(), spec.sig(), data);
        }
      }
    }
    // #if timeouts
    if (Deadline.current() != null && Deadline.current().fired()) {
      // The answer was given at the deadline: what the handler throws now is not a failure to report.
      return problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
    }
    // #endif
    try {
      if (hooks.onError() != null) {
        hooks.onError().onError(err, ctx);
      }
    } catch (RuntimeException ignored) {
      // A failing hook must not change the response.
    }
    return problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
  }

  static VeldResponse problem(String sig, String title, int status, String op, Json data) {
    Json body = Json.object()
        .put("type", Json.of(sig))
        .put("title", Json.of(title))
        .put("status", Json.of((long) status))
        .put("operation", Json.of(op))
        .put("data", data)
        .build();
    return new VeldResponse(status, Map.of("content-type", "application/problem+json"), body.write());
  }
  // #if rules

  /** A problem document that also carries the {@code violations} member (spec/rules.md section 7). */
  static VeldResponse problem(
      String sig, String title, int status, String op, Json data, List<Violation> violations) {
    List<Json> listed = new ArrayList<>();
    for (Violation v : violations) {
      listed.add(v.toWire());
    }
    Json body = Json.object()
        .put("type", Json.of(sig))
        .put("title", Json.of(title))
        .put("status", Json.of((long) status))
        .put("operation", Json.of(op))
        .put("data", data)
        .put("violations", Json.array(listed))
        .build();
    return new VeldResponse(status, Map.of("content-type", "application/problem+json"), body.write());
  }
  // #endif

  // -- binding -------------------------------------------------------------------------------

  private static final class Bound {
    boolean check = true;
    final Map<String, Json> values = new LinkedHashMap<>();
    final List<Issue> issues = new ArrayList<>();
    // #if conditional
    /** The parameters as the fingerprint of an idempotency key sees them: defaults applied, no placeholders. */
    final Map<String, Json> canonical = new LinkedHashMap<>();
    // #endif
  }

  private static Bound bind(Route r, Matcher m, VeldRequest req, RequestContext ctx, boolean ws, boolean check) {
    Bound b = new Bound();
    b.check = check;
    Map<String, List<String>> query = parseQuery(req.query());
    for (EndpointSpec.Param p : r.endpoint().spec().params()) {
      if (p.stream()) {
        continue;
      }
      switch (p.binding()) {
        case PATH -> {
          int i = r.names().indexOf(p.name());
          String decoded = i < 0 ? null : decode(m.group(i + 1).replace("+", "%2B"));
          bindSimple(p, decoded == null ? null : List.of(decoded), b);
        }
        case QUERY -> bindSimple(p, query.get(p.name()), b);
        case QUERY_OBJECT -> bindQueryObject(p, query, b);
        case HEADER -> {
          String name = p.header() == null ? p.name() : p.header();
          String h = ctx.header(name);
          if (h == null && ws && query.get(name) != null && !query.get(name).isEmpty()) {
            h = query.get(name).get(0);
          }
          bindSimple(p, h == null ? null : List.of(h), b);
        }
        case BODY -> bindBody(p, req, b);
      }
    }
    return b;
  }

  private static void bindSimple(EndpointSpec.Param p, List<String> raw, Bound b) {
    String loc = p.location();
    if (raw == null || raw.isEmpty()) {
      if (!p.optional()) {
        b.issues.add(new Issue(loc, "required", "is required"));
      }
      return;
    }
    Json v = coerceMany(p.type(), raw);
    List<Issue> issues = b.check ? Wire.validate(p.type(), v, loc, false) : List.of();
    b.issues.addAll(issues);
    if (issues.isEmpty()) {
      // #if conditional
      b.canonical.put(p.name(), Canon.scalars(p.type(), v, true));
      // #endif
      b.values.put(p.name(), v);
    }
  }

  private static void bindQueryObject(EndpointSpec.Param p, Map<String, List<String>> query, Bound b) {
    Schema s = p.type().resolve();
    if (s.kind != Schema.Kind.OBJECT) {
      return;
    }
    Map<String, Json> wire = new LinkedHashMap<>();
    for (Schema.Field f : s.fields) {
      // #if derived
      if (s.derives(f.wire())) {
        continue;
      }
      // #endif
      List<String> raw = query.get(f.wire());
      if (raw != null) {
        wire.put(f.wire(), coerceMany(f.type(), raw));
      }
    }
    Json obj = new Json.Obj(wire);
    List<Issue> issues = b.check ? Wire.validate(p.type(), obj, p.location(), true) : List.of();
    b.issues.addAll(issues);
    if (issues.isEmpty()) {
      // #if derived
      Json prepared = Derive.input(p.type(), Wire.prepare(p.type(), obj, Wire.Mode.REQUEST), p.location());
      // #endif
      // #if no_derived_conditional
      b.canonical.put(p.name(), Canon.scalars(p.type(), Wire.prepare(p.type(), obj, Wire.Mode.REQUEST), true));
      // #endif
      // #if derived_conditional
      b.canonical.put(p.name(), Canon.scalars(p.type(), prepared, true));
      // #endif
      // #if no_derived
      b.values.put(p.name(), Wire.fillReadonly(p.type(), Wire.prepare(p.type(), obj, Wire.Mode.REQUEST)));
      // #endif
      // #if derived
      b.values.put(p.name(), Wire.fillReadonly(p.type(), prepared));
      // #endif
    }
  }

  private static void bindBody(EndpointSpec.Param p, VeldRequest req, Bound b) {
    String text = req.body();
    Json raw = null;
    boolean badJson = false;
    if (text != null && !text.isBlank()) {
      try {
        raw = Json.parse(text);
      } catch (Json.ParseException e) {
        badJson = true;
        b.issues.add(new Issue("body", "json", "body is not valid JSON"));
      }
    }
    if (raw == null) {
      if (!p.optional() && !badJson) {
        b.issues.add(new Issue("body", "required", "is required"));
      }
      return;
    }
    List<Issue> issues = b.check ? Wire.validate(p.type(), raw, "body", true) : List.of();
    b.issues.addAll(issues);
    if (issues.isEmpty()) {
      // #if derived
      Json prepared = Derive.input(p.type(), Wire.prepare(p.type(), raw, Wire.Mode.REQUEST), p.location());
      // #endif
      // #if no_derived_conditional
      b.canonical.put(p.name(), Canon.scalars(p.type(), Wire.prepare(p.type(), raw, Wire.Mode.REQUEST), true));
      // #endif
      // #if derived_conditional
      b.canonical.put(p.name(), Canon.scalars(p.type(), prepared, true));
      // #endif
      // #if no_derived
      b.values.put(p.name(), Wire.fillReadonly(p.type(), Wire.prepare(p.type(), raw, Wire.Mode.REQUEST)));
      // #endif
      // #if derived
      b.values.put(p.name(), Wire.fillReadonly(p.type(), prepared));
      // #endif
    }
  }

  private static Json coerceMany(Schema s, List<String> raw) {
    Schema t = s.resolve();
    if (t.kind == Schema.Kind.LIST) {
      List<Json> items = new ArrayList<>();
      for (String x : raw) {
        items.add(coerce(t.of, x));
      }
      return Json.array(items);
    }
    return coerce(s, raw.get(0));
  }

  private static Json coerce(Schema s, String raw) {
    Schema t = s.resolve();
    if ((t.kind == Schema.Kind.INT || t.kind == Schema.Kind.FLOAT) && NUMBER.matcher(raw).matches()) {
      return Json.of(Double.parseDouble(raw));
    }
    if (t.kind == Schema.Kind.BOOL) {
      if (raw.equals("true")) {
        return Json.of(true);
      }
      if (raw.equals("false")) {
        return Json.of(false);
      }
    }
    return Json.of(raw);
  }

  /** Parses a raw query string; keys and values are percent-decoded, {@code +} is a space. */
  static Map<String, List<String>> parseQuery(String qs) {
    Map<String, List<String>> out = new LinkedHashMap<>();
    if (qs == null) {
      return out;
    }
    for (String pair : qs.split("&")) {
      if (pair.isEmpty()) {
        continue;
      }
      int eq = pair.indexOf('=');
      String k = decode((eq < 0 ? pair : pair.substring(0, eq)).replace('+', ' '));
      String v = decode((eq < 0 ? "" : pair.substring(eq + 1)).replace('+', ' '));
      if (k != null && v != null) {
        out.computeIfAbsent(k, x -> new ArrayList<>()).add(v);
      }
    }
    return out;
  }

  /** Percent-decodes UTF-8, or returns {@code null} when the text is malformed. */
  private static String decode(String s) {
    if (s.indexOf('%') < 0) {
      return s;
    }
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      if (c != '%') {
        byte[] raw = String.valueOf(c).getBytes(StandardCharsets.UTF_8);
        if (Character.isHighSurrogate(c) && i + 1 < s.length()) {
          raw = s.substring(i, i + 2).getBytes(StandardCharsets.UTF_8);
          i++;
        }
        bytes.write(raw, 0, raw.length);
        continue;
      }
      if (i + 2 >= s.length()) {
        return null;
      }
      int hi = Character.digit(s.charAt(i + 1), 16);
      int lo = Character.digit(s.charAt(i + 2), 16);
      if (hi < 0 || lo < 0) {
        return null;
      }
      bytes.write(hi * 16 + lo);
      i += 2;
    }
    try {
      return StandardCharsets.UTF_8.newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(bytes.toByteArray()))
          .toString();
    } catch (CharacterCodingException e) {
      return null;
    }
  }
}
