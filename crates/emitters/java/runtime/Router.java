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
    assertHooks(endpoints, hooks);
    List<Route> routes = new ArrayList<>();
    for (ServerEndpoint e : endpoints) {
      routes.add(compile(e));
    }
    routes.sort((a, b) -> Integer.compare(a.names().size(), b.names().size()));
    return new Router(List.copyOf(routes), std, hooks);
  }

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

  private VeldResponse run(Route route, Matcher match, VeldRequest request) {
    ServerEndpoint ep = route.endpoint();
    EndpointSpec spec = ep.spec();
    RequestContext ctx = new RequestContext(spec.method(), normalizePath(request.path()), lowerHeaders(request.headers()), request.raw());
    try {
      if (spec.auth() != null) {
        ctx.setPrincipal(authenticate(spec.auth(), ctx));
        if (ctx.principal() == null) {
          return problem(std.unauthorized(), "Unauthorized", 401, spec.sig(), Json.object().build());
        }
      }
      for (String name : ep.middleware()) {
        hooks.middleware(name).run(ctx);
      }
      Bound bound = bind(route, match, request, ctx);
      if (!bound.issues.isEmpty()) {
        return problem(std.validation(), "ValidationError", 422, spec.sig(), issuesData(bound.issues));
      }
      return success(ep, ep.call().call(new Json.Obj(bound.values), ctx));
    } catch (Exception | StackOverflowError err) {
      return failure(err, spec, ctx);
    }
  }

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
    if (hooks.validateResponses()) {
      List<Issue> issues = Wire.validate(spec.success(), wire, "$", false);
      if (!issues.isEmpty()) {
        throw new IllegalStateException(spec.name() + ": response failed validation: " + issues);
      }
    }
    return new VeldResponse(spec.status(), Map.of("content-type", "application/json"), wire.write());
  }

  private VeldResponse failure(Throwable err, EndpointSpec spec, RequestContext ctx) {
    if (err instanceof VeldError ve) {
      for (EndpointSpec.Declared d : spec.errors()) {
        if (d.sig().equals(ve.sig())) {
          ve.setOp(spec.sig());
          Json data = Wire.prepare(d.schema(), ve.dataWire(), Wire.Mode.RESPONSE);
          return problem(d.sig(), ve.title(), d.status(), spec.sig(), data);
        }
      }
    }
    try {
      if (hooks.onError() != null) {
        hooks.onError().onError(err, ctx);
      }
    } catch (RuntimeException ignored) {
      // A failing hook must not change the response.
    }
    return problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build());
  }

  private static VeldResponse problem(String sig, String title, int status, String op, Json data) {
    Json body = Json.object()
        .put("type", Json.of(sig))
        .put("title", Json.of(title))
        .put("status", Json.of((long) status))
        .put("operation", Json.of(op))
        .put("data", data)
        .build();
    return new VeldResponse(status, Map.of("content-type", "application/problem+json"), body.write());
  }

  // -- binding -------------------------------------------------------------------------------

  private static final class Bound {
    final Map<String, Json> values = new LinkedHashMap<>();
    final List<Issue> issues = new ArrayList<>();
  }

  private static Bound bind(Route r, Matcher m, VeldRequest req, RequestContext ctx) {
    Bound b = new Bound();
    Map<String, List<String>> query = parseQuery(req.query());
    for (EndpointSpec.Param p : r.endpoint().spec().params()) {
      switch (p.binding()) {
        case PATH -> {
          int i = r.names().indexOf(p.name());
          String decoded = i < 0 ? null : decode(m.group(i + 1).replace("+", "%2B"));
          bindSimple(p, decoded == null ? null : List.of(decoded), b);
        }
        case QUERY -> bindSimple(p, query.get(p.name()), b);
        case QUERY_OBJECT -> bindQueryObject(p, query, b);
        case HEADER -> {
          String h = ctx.header(p.header() == null ? p.name() : p.header());
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
    List<Issue> issues = Wire.validate(p.type(), v, loc, false);
    b.issues.addAll(issues);
    if (issues.isEmpty()) {
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
      List<String> raw = query.get(f.wire());
      if (raw != null) {
        wire.put(f.wire(), coerceMany(f.type(), raw));
      }
    }
    Json obj = new Json.Obj(wire);
    List<Issue> issues = Wire.validate(p.type(), obj, p.location(), true);
    b.issues.addAll(issues);
    if (issues.isEmpty()) {
      b.values.put(p.name(), Wire.prepare(p.type(), obj, Wire.Mode.REQUEST));
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
    List<Issue> issues = Wire.validate(p.type(), raw, "body", true);
    b.issues.addAll(issues);
    if (issues.isEmpty()) {
      b.values.put(p.name(), Wire.prepare(p.type(), raw, Wire.Mode.REQUEST));
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
