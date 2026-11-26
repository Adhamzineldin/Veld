package dev.veld.runtime;

import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * What a generated test handler needs (spec/rules.md section 12.4): the facts, the principal, the
 * clock and the result come from the {@code Veld-Test-*} request headers, a dry run stops after
 * step 6, and {@code GET /__veld/test} tells a test handler from a production server.
 *
 * <p>This is for tests only. It must never be mounted in production.
 */
public final class TestSupport {
  private TestSupport() {}

  /** The path that answers as a test handler. */
  public static final String PROBE_PATH = "/__veld/test";

  /** Reports once that a test handler was created. */
  public static void notice(String service) {
    System.getLogger("veld.test")
        .log(
            System.Logger.Level.WARNING,
            "{0}: a TEST handler was created: facts, principal, clock and result come from request headers. Never mount it in production.",
            service);
  }

  private static Json json(RequestContext ctx, String header) {
    String text = ctx.header(header);
    if (text == null || text.isEmpty()) {
      return null;
    }
    byte[] bytes = Base64.getUrlDecoder().decode(text);
    return Json.parse(new String(bytes, StandardCharsets.UTF_8));
  }

  /**
   * The wire value of a fact the request carries, or empty when the fact is absent (missing from
   * the object, or {@code null}).
   */
  public static Optional<Json> fact(RequestContext ctx, String name) {
    Json facts = json(ctx, "Veld-Test-Facts");
    if (facts instanceof Json.Obj obj && obj.get(name) != null && !(obj.get(name) instanceof Json.Null)) {
      return Optional.of(obj.get(name));
    }
    return Optional.empty();
  }

  /** The clock of this request: {@code Veld-Test-Now}, or the system clock when it is absent. */
  public static long now(RequestContext ctx) {
    String text = ctx.header("Veld-Test-Now");
    return text == null ? System.currentTimeMillis() : Instant.parse(text).toEpochMilli();
  }

  /** The wire value the handler is to answer with, or {@code null} for the application's handler. */
  public static Json result(RequestContext ctx) {
    return json(ctx, "Veld-Test-Result");
  }

  /** Whether the request asks to stop after step 6. */
  public static boolean dryRun(RequestContext ctx) {
    return "1".equals(ctx.header("Veld-Test-Dry-Run"));
  }

  /** The response of a dry run that passed: {@code 204} and {@code Veld-Test-Passed: 1}. */
  public static EndpointRules.Early passed() {
    return new EndpointRules.Early(new VeldResponse(204, Map.of("Veld-Test-Passed", "1"), null));
  }

  /**
   * The endpoint without authentication and middleware: the principal comes from
   * {@code Veld-Test-Principal} (the wire JSON, or {@code true} when the header is absent).
   * Streaming endpoints are returned as they are.
   */
  public static ServerEndpoint endpoint(ServerEndpoint e) {
    if (e.call() == null) {
      return e;
    }
    EndpointSpec s = e.spec();
    EndpointSpec open = new EndpointSpec(
        s.name(), s.method(), s.path(), s.sig(), s.status(), null, s.params(), s.success(), s.errors(), s.stream());
    return new ServerEndpoint(open, List.of(), (params, ctx) -> {
      Json principal = json(ctx, "Veld-Test-Principal");
      ctx.setPrincipal(principal == null ? Boolean.TRUE : principal);
      return e.call().call(params, ctx);
    });
  }

  // #if policies
  /**
   * Like {@link #endpoint(ServerEndpoint)}, for a service with a principal: the principal is what
   * {@code decode} makes of the wire JSON in {@code Veld-Test-Principal}, the generated type of the
   * principal's model, or {@code null} when the header is absent.
   */
  public static ServerEndpoint endpoint(ServerEndpoint e, java.util.function.Function<Json, Object> decode) {
    if (e.call() == null) {
      return e;
    }
    EndpointSpec s = e.spec();
    EndpointSpec open = new EndpointSpec(
        s.name(), s.method(), s.path(), s.sig(), s.status(), null, s.params(), s.success(), s.errors(), s.stream());
    return new ServerEndpoint(open, List.of(), (params, ctx) -> {
      Json principal = json(ctx, "Veld-Test-Principal");
      ctx.setPrincipal(principal == null ? null : decode.apply(principal));
      return e.call().call(params, ctx);
    });
  }

  // #endif
  /**
   * Adds {@code GET /__veld/test}, which answers {@code 200} with
   * {@code {"veldTestHandler":1,"digests":{"<Service>":"<digest>"}}}.
   *
   * @param digests the rules digest by service name, or {@code null} for a service without one
   */
  public static VeldHandler handler(VeldHandler inner, Map<String, String> digests) {
    return new VeldHandler() {
      @Override
      public Optional<VeldResponse> handle(VeldRequest request) {
        if (request.method().equalsIgnoreCase("GET") && request.path().equals(PROBE_PATH)) {
          Map<String, Json> members = new LinkedHashMap<>();
          digests.forEach((name, digest) -> members.put(name, digest == null ? Json.NULL : Json.of(digest)));
          Json body = Json.object()
              .put("veldTestHandler", Json.of(1L))
              .put("digests", new Json.Obj(members))
              .build();
          return Optional.of(new VeldResponse(200, Map.of("content-type", "application/json"), body.write()));
        }
        // #if testing_conditional
        return withStore(request, () -> inner.handle(request));
        // #endif
        // #if testing_plain
        return inner.handle(request);
        // #endif
      }

      @Override
      public Optional<VeldResponse> invoke(String endpoint, VeldRequest request) {
        // #if testing_conditional
        return withStore(request, () -> inner.invoke(endpoint, request));
        // #endif
        // #if testing_plain
        return inner.invoke(endpoint, request);
        // #endif
      }
    };
  }
  // #if testing_conditional

  /** The request header that tells the test handler what the idempotency store holds for the key. */
  public static final String IDEMPOTENCY_HEADER = "Veld-Test-Idempotency";

  /** What the request said about the store, and the store built from it once it is asked. */
  private static final class RequestState {
    final String header;
    IdempotencyStore store;

    RequestState(String header) {
      this.header = header;
    }
  }

  private static final ThreadLocal<RequestState> REQUEST_STATE = new ThreadLocal<>();

  /**
   * The store of a test handler: every request gets a fresh memory store, and what
   * {@code Veld-Test-Idempotency} says (spec/rules.md section 11.9) is in it under the
   * fingerprint of the request, or under another one for {@code other}. Pass it to
   * {@link HandlerHooks#withIdempotency}.
   *
   * @param conditions the conditional headers of the service by operation signature, for the
   *     {@code ETag} of a stored success
   */
  public static IdempotencyStore store(Map<String, Conditions> conditions) {
    return new IdempotencyStore() {
      @Override
      public Begin begin(String scope, String key, String fingerprint) throws Exception {
        return current().begin(scope, key, fingerprint);
      }

      @Override
      public void complete(String scope, String key, StoredAnswer answer) throws Exception {
        current().complete(scope, key, answer);
      }

      @Override
      public void release(String scope, String key) throws Exception {
        current().release(scope, key);
      }

      private IdempotencyStore current() {
        RequestState state = REQUEST_STATE.get();
        if (state == null) {
          throw new IllegalStateException("the test handler's store is used outside a request");
        }
        if (state.store == null) {
          state.store = new SeededStore(state.header, conditions);
        }
        return state.store;
      }
    };
  }

  /**
   * The caller of a test request: the canonical JSON of {@code Veld-Test-Principal}, or
   * {@code caller} when the request has none (spec/rules.md section 11.10). The application's hook
   * is not called.
   */
  public static HandlerHooks.IdempotencyCaller caller() {
    return (principal, ctx) -> {
      Json wire = json(ctx, "Veld-Test-Principal");
      return wire == null ? "caller" : wire.canonical();
    };
  }

  private static Optional<VeldResponse> withStore(
      VeldRequest request, java.util.function.Supplier<Optional<VeldResponse>> call) {
    REQUEST_STATE.set(new RequestState(header(request, IDEMPOTENCY_HEADER)));
    try {
      return call.get();
    } finally {
      REQUEST_STATE.remove();
    }
  }

  private static String header(VeldRequest request, String name) {
    for (Map.Entry<String, List<String>> e : request.headers().entrySet()) {
      if (e.getKey() != null && e.getKey().equalsIgnoreCase(name) && !e.getValue().isEmpty()) {
        return e.getValue().get(0);
      }
    }
    return null;
  }

  /** A memory store that starts with what the request says it holds for the key it is about to ask for. */
  private static final class SeededStore implements IdempotencyStore {
    private final MemoryIdempotencyStore memory = new MemoryIdempotencyStore();
    private final Map<String, Conditions> conditions;
    private Json.Obj state;

    SeededStore(String header, Map<String, Conditions> conditions) {
      this.conditions = conditions;
      if (header != null && !header.isEmpty()) {
        byte[] bytes = Base64.getUrlDecoder().decode(header);
        this.state = Json.parse(new String(bytes, StandardCharsets.UTF_8)).asObject();
      }
    }

    @Override
    public Begin begin(String scope, String key, String fingerprint) throws Exception {
      Json.Obj seeded = state;
      state = null;
      if (seeded == null) {
        return memory.begin(scope, key, fingerprint);
      }
      String kind = seeded.get("state").asString();
      if (kind.equals("other")) {
        return new Begin.Other();
      }
      if (kind.equals("running")) {
        return new Begin.Running();
      }
      return new Begin.Replay(answer(scope, seeded, conditions));
    }

    @Override
    public void complete(String scope, String key, StoredAnswer answer) throws Exception {
      memory.complete(scope, key, answer);
    }

    @Override
    public void release(String scope, String key) throws Exception {
      memory.release(scope, key);
    }
  }

  /**
   * The answer a stored state stands for: the problem document of an error (its title is the name
   * in its signature, its operation the one the scope starts with) or the body of a success.
   */
  private static StoredAnswer answer(String scope, Json.Obj seeded, Map<String, Conditions> conditions) {
    int status = (int) ((Json.Num) seeded.get("status")).value();
    Json data = seeded.get("data");
    Json type = seeded.get("type");
    if (type instanceof Json.Str sig) {
      String name = sig.value().substring(sig.value().lastIndexOf('/') + 1, sig.value().indexOf('@'));
      int cut = scope.indexOf('\n');
      Json body = Json.object()
          .put("type", Json.of(sig.value()))
          .put("title", Json.of(name))
          .put("status", Json.of((long) status))
          .put("operation", Json.of(cut < 0 ? scope : scope.substring(0, cut)))
          .put("data", data == null ? Json.object().build() : data)
          .build();
      return new StoredAnswer(
          status, body.write().getBytes(StandardCharsets.UTF_8), "application/problem+json", null, null);
    }
    if (data == null || data instanceof Json.Null) {
      return new StoredAnswer(status, new byte[0], null, null, null);
    }
    int cut = scope.indexOf('\n');
    Conditions owed = conditions.get(cut < 0 ? scope : scope.substring(0, cut));
    String etag = owed == null ? null : owed.etagOf(data);
    return new StoredAnswer(
        status, data.write().getBytes(StandardCharsets.UTF_8), "application/json", etag, null);
  }
  // #endif
  // #if rollout

  /** The most audited reports a test handler sends in {@code Veld-Test-Audited}. */
  private static final int AUDITED_LIMIT = 100;

  /**
   * Like {@link #handler}, and every answer carries the response header {@code Veld-Test-Audited}
   * when the request made the server report audited rules ({@code @rollout(audit)}, spec/rules.md
   * section 12.4): the unpadded base64url of a compact JSON array with one entry per report, the
   * first 100 at most.
   */
  public static VeldHandler auditedHandler(VeldHandler inner, Map<String, String> digests) {
    VeldHandler probe = handler(inner, digests);
    return new VeldHandler() {
      @Override
      public Optional<VeldResponse> handle(VeldRequest request) {
        return audited(() -> probe.handle(request));
      }

      @Override
      public Optional<VeldResponse> invoke(String endpoint, VeldRequest request) {
        return audited(() -> probe.invoke(endpoint, request));
      }
    };
  }

  private static Optional<VeldResponse> audited(java.util.function.Supplier<Optional<VeldResponse>> call) {
    Audit.startLog();
    Optional<VeldResponse> answer;
    try {
      answer = call.get();
    } catch (RuntimeException | Error e) {
      Audit.endLog();
      throw e;
    }
    List<Violation> reported = Audit.endLog();
    return reported.isEmpty() ? answer : answer.map(r -> withAudited(r, reported));
  }

  private static VeldResponse withAudited(VeldResponse response, List<Violation> reported) {
    List<Json> entries = new java.util.ArrayList<>();
    for (Violation v : reported.subList(0, Math.min(reported.size(), AUDITED_LIMIT))) {
      entries.add(v.toWire());
    }
    String text = Base64.getUrlEncoder().withoutPadding()
        .encodeToString(Json.array(entries).write().getBytes(StandardCharsets.UTF_8));
    Map<String, String> headers = new LinkedHashMap<>(response.headers());
    headers.put("veld-test-audited", text);
    return new VeldResponse(response.status(), headers, response.body());
  }
  // #endif
}
