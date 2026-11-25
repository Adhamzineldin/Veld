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
        return inner.handle(request);
      }

      @Override
      public Optional<VeldResponse> invoke(String endpoint, VeldRequest request) {
        return inner.invoke(endpoint, request);
      }
    };
  }
}
