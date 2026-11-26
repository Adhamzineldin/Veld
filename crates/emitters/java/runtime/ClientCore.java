package dev.veld.runtime;

import java.io.IOException;
// veld:sse
import java.io.InputStream;
// veld:end
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
// veld:ws
import java.net.http.WebSocket;
// veld:end
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
// veld:ws
import java.util.concurrent.CompletionException;
// veld:end
// veld:sse
import java.util.stream.Stream;
// veld:end

/**
 * The client core: validates a request, sends it with {@link java.net.http.HttpClient}, and turns
 * the response into a value or a typed error. Calls are blocking.
 */
public final class ClientCore {
  private final ClientOptions options;
  private final ClientDefaults defaults;
  private final String baseUrl;
  private final HttpClient http;
  // #if rules
  private volatile String ownDigest;
  private volatile String serverDigest;
  private volatile String serviceName;
  private final java.util.Set<String> mismatched = java.util.concurrent.ConcurrentHashMap.newKeySet();
  // #endif

  // #if legacy
  /**
   * Creates the core. The base URL is the first of: the option, the environment variable, the
   * contract's {@code base-url}.
   *
   * @throws VeldTransportError when none of them is set
   */
  public ClientCore(ClientOptions options, ClientDefaults defaults) {
    this.options = options;
    this.defaults = defaults;
    String base = options.baseUrl();
    if (base == null) {
      base = System.getenv(defaults.envVar());
    }
    if (base == null) {
      base = defaults.baseUrl();
    }
    if (base == null) {
      throw new VeldTransportError("no base URL: pass baseUrl or set " + defaults.envVar());
    }
    this.baseUrl = base.replaceAll("/+$", "");
  // #endif
  // #if runtimecfg
  /**
   * Creates the core. The base URL is the {@code baseUrl} option, which a client requires.
   *
   * @throws VeldTransportError when the option is not set
   */
  public ClientCore(ClientOptions options, ClientDefaults defaults) {
    this.options = options;
    this.defaults = defaults;
    String base = options.baseUrl();
    if (base == null) {
      throw new VeldTransportError("no base URL: pass the baseUrl option");
    }
    this.baseUrl = base.replaceAll("/+$", "");
  // #endif
    this.http = options.httpClient() != null ? options.httpClient() : defaultClient();
  }

  /**
   * HTTP/1.1: over cleartext the JDK would otherwise offer an {@code Upgrade: h2c} on every request,
   * which servers that also host WebSocket upgrades answer with a 400. Pass your own client for
   * HTTP/2.
   */
  private static HttpClient defaultClient() {
    return HttpClient.newBuilder().version(HttpClient.Version.HTTP_1_1).build();
  }

  /**
   * Performs a call.
   *
   * @param ep the endpoint
   * @param params the request parameters in wire form, by parameter name
   * @return the response in wire form, or {@code null} when the endpoint has no response body
   */
  public Json call(EndpointSpec ep, Json.Obj params) {
    check(ep, params);
    HttpRequest request = buildRequest(ep, params, "application/json", Map.of());
    // #if limits
    return decode(ep, sendLimited(ep, request));
    // #endif
    // #if not_limits
    return decode(ep, send(ep, request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8)));
    // #endif
  }
  // #if preconditions

  /**
   * Performs a call that also sends the headers of {@link #conditional}: {@code If-Match} and the
   * idempotency key. They are not parameters, so they are neither validated nor part of the
   * request's wire form.
   */
  public Json call(EndpointSpec ep, Json.Obj params, Map<String, String> extra) {
    check(ep, params);
    HttpRequest request = buildRequest(ep, params, "application/json", extra);
    // #endif
    // #if preconditions_limits
    return decode(ep, sendLimited(ep, request));
    // #endif
    // #if preconditions_not_limits
    return decode(ep, send(ep, request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8)));
    // #endif
    // #if preconditions
  }

  /**
   * The headers of {@code ifMatch} and of the idempotency key, for the ones that are not
   * {@code null}. The client sends what it is given and checks neither.
   */
  public static Map<String, String> conditional(IfMatch ifMatch, String keyHeader, String key) {
    Map<String, String> out = new java.util.LinkedHashMap<>();
    if (ifMatch != null) {
      out.put("If-Match", ifMatch.header());
    }
    if (key != null) {
      out.put(keyHeader, key);
    }
    return out;
  }
  // #endif

  private void check(EndpointSpec ep, Json.Obj params) {
    if (options.validate(defaults.validate())) {
      List<Issue> issues = requestIssues(ep, params);
      if (!issues.isEmpty()) {
        VeldError err = defaults.validation().apply(issues);
        err.setOp(ep.sig());
        throw err;
      }
      // #if rules_not_messages
      if (options.localRules() && localRulesOn()) {
        RuleWalk.localCheck(ep, params, defaults.validation());
      }
      // #endif
      // #if messages_rules
      if (options.localRules() && localRulesOn()) {
        try {
          RuleWalk.localCheck(ep, params, defaults.validation());
        } catch (VeldError err) {
          Catalogs.keyLocal(messageKeys, err);
          throw err;
        }
      }
      // #endif
    }
  }
  // #if messages

  private volatile java.util.Map<String, String> messageKeys = java.util.Map.of();

  /**
   * Tells the core the {@code messageKey} of each rule of the service (spec/platform.md section 22),
   * by rule id: the violations of its local check have them.
   */
  public void messageKeys(java.util.Map<String, String> keys) {
    this.messageKeys = java.util.Map.copyOf(keys);
  }
  // #endif
  // #if rules

  /**
   * Tells the core the rules digest the client was generated with (spec/rules.md section 9.7).
   * The local check then runs only while the last response carried the same digest.
   */
  public void digest(String service, String digest) {
    this.serviceName = service;
    this.ownDigest = digest;
  }

  private boolean localRulesOn() {
    return ownDigest == null || ownDigest.equals(serverDigest);
  }

  private <T> HttpResponse<T> observe(HttpResponse<T> response) {
    String seen = response.headers().firstValue("Veld-Rules").orElse(null);
    serverDigest = seen;
    if (seen != null && !seen.equals(ownDigest) && mismatched.add(seen)) {
      java.util.function.Consumer<ClientOptions.ContractMismatch> callback = options.onContractMismatch();
      if (callback != null) {
        try {
          callback.accept(new ClientOptions.ContractMismatch(serviceName, ownDigest, seen));
        } catch (RuntimeException ignored) {
          // A failing callback must not change the call.
        }
      }
    }
    return response;
  }
  // #endif
  // veld:sse

  /**
   * Opens a Server-Sent Events call. The HTTP call is eager: a non-2xx response throws its typed
   * error from this method. The returned stream reads events as they arrive; closing it cancels
   * the request. A final {@code error} event throws the typed error when it is consumed, and a
   * broken connection throws {@link VeldTransportError}. With a {@link Reconnect} policy (client
   * options or {@link SseOptions#withReconnect}) a refused or broken connection is retried with
   * backoff, resuming with {@code Last-Event-ID}; without one call again with
   * {@link SseOptions#resumeAfter} to resume.
   *
   * @return the events in wire form
   */
  public Stream<Json> stream(EndpointSpec ep, Json.Obj params, SseOptions sse) {
    check(ep, params);
    Reconnect policy = sse.reconnect() != null ? sse.reconnect() : options.reconnect();
    // Failed first attempts count against the same budget as later breaks: the counter starts
    // again only after an event is delivered.
    int failures = 0;
    while (true) {
      try {
        InputStream first = openEvents(ep, params, sse.lastEventId());
        return SseStream.open(ep, first, sse, policy, failures, last -> openEvents(ep, params, last));
      } catch (VeldTransportError e) {
        if (policy == null || failures >= policy.retries()) {
          throw e;
        }
        Reconnect.sleep(policy.backoffMs(failures++));
      }
    }
  }

  private InputStream openEvents(EndpointSpec ep, Json.Obj params, String lastEventId) {
    Map<String, String> extra = new LinkedHashMap<>();
    if (lastEventId != null) {
      extra.put("last-event-id", lastEventId);
    }
    HttpRequest request = buildRequest(ep, params, "text/event-stream", extra);
    HttpResponse<InputStream> res = send(ep, request, SseStream.bodyHandler());
    int status = res.statusCode();
    if (status < 200 || status >= 300) {
      decodeText(ep, status, readAll(ep, res.body()));
      throw new VeldTransportError(ep.method() + " " + ep.path() + ": unexpected status " + status);
    }
    return res.body();
  }

  private static String readAll(EndpointSpec ep, InputStream in) {
    try (in) {
      return new String(in.readAllBytes(), StandardCharsets.UTF_8);
    } catch (IOException e) {
      throw new VeldTransportError(ep.method() + " " + ep.path() + ": could not read the response", e);
    }
  }
  // veld:end
  // veld:ws

  /**
   * Opens a WebSocket. The channel is connected when this returns. Auth goes in the query string
   * ({@code access_token} for bearer, {@code api_key} for an API key); {@code @header} parameters
   * travel as query parameters too. Basic auth cannot be sent on a WebSocket.
   *
   * @throws VeldTransportError when the connection cannot be opened (after the retries of the
   *     {@link Reconnect} policy, if one is set)
   */
  public <In, Out> WsChannel<In, Out> connect(
      EndpointSpec ep, Json.Obj params, Codec<In> in, Codec<Out> out) {
    check(ep, params);
    WsChannel<In, Out> channel = new WsChannel<>(ep, in, out, options.validate(defaults.validate()), defaults.validation());
    // #if rules
    channel.localRules(options.localRules());
    // #endif
    URI uri = wsUri(ep, params);
    java.util.function.Function<WebSocket.Listener, WebSocket> opener =
        listener -> {
          WebSocket.Builder builder = http.newWebSocketBuilder().connectTimeout(options.timeout());
          options.headers().get().forEach(builder::header);
          try {
            return builder.buildAsync(uri, listener).join();
          } catch (CompletionException e) {
            throw new VeldTransportError(
                ep.method() + " " + ep.path() + ": could not connect", e.getCause());
          }
        };
    channel.open(opener, options.reconnect());
    return channel;
  }

  private URI wsUri(EndpointSpec ep, Json.Obj params) {
    List<String[]> pairs = queryPairs(ep, params);
    for (EndpointSpec.Param p : ep.params()) {
      Json v = params.get(p.name());
      if (p.binding() == EndpointSpec.Binding.HEADER && v != null) {
        pairs.add(new String[] {p.header() == null ? p.name() : p.header(), scalarString(v)});
      }
    }
    if (ep.auth() != null) {
      switch (ep.auth().kind()) {
        case BEARER -> {
          if (options.bearer() != null) {
            pairs.add(new String[] {"access_token", options.bearer().get()});
          }
        }
        case API_KEY -> {
          if (options.apiKey() != null) {
            pairs.add(new String[] {"api_key", options.apiKey().get()});
          }
        }
        case BASIC -> throw new VeldTransportError(
            ep.name() + ": basic auth cannot be sent on a WebSocket; use bearer or an API key");
      }
    }
    String qs = joinQuery(pairs);
    String ws = baseUrl.replaceFirst("^http", "ws");
    return uri(ws, expandPath(ep, params) + (qs.isEmpty() ? "" : "?" + qs));
  }
  // veld:end


  private static List<Issue> requestIssues(EndpointSpec ep, Json.Obj params) {
    List<Issue> out = new ArrayList<>();
    for (EndpointSpec.Param p : ep.params()) {
      if (p.stream()) {
        continue;
      }
      Json v = params.get(p.name());
      if (v == null) {
        if (!p.optional()) {
          out.add(new Issue(p.location(), "required", "is required"));
        }
      } else {
        out.addAll(Wire.validate(p.type(), v, p.location(), true));
      }
    }
    return out;
  }

  // -- request -------------------------------------------------------------------------------

  private HttpRequest buildRequest(EndpointSpec ep, Json.Obj params, String accept, Map<String, String> extra) {
    String path = expandPath(ep, params);
    String qs = queryString(ep, params);
    HttpRequest.Builder builder = HttpRequest.newBuilder(uri(path + (qs.isEmpty() ? "" : "?" + qs)))
        // #if timeouts
        .timeout(timeoutOf(ep));
        // #endif
        // #if no_timeouts
        .timeout(options.timeout());
        // #endif
    EndpointSpec.Param body = bodyParam(ep);
    HttpRequest.BodyPublisher publisher = HttpRequest.BodyPublishers.noBody();
    Map<String, String> headers = headers(ep, params, accept);
    headers.putAll(extra);
    if (body != null && params.get(body.name()) != null) {
      headers.put("content-type", "application/json");
      Json wire = Wire.prepare(body.type(), params.get(body.name()), Wire.Mode.RESPONSE);
      // #if derived
      wire = Derive.strip(body.type(), wire);
      // #endif
      publisher = HttpRequest.BodyPublishers.ofString(wire.write(), StandardCharsets.UTF_8);
    }
    headers.forEach(builder::header);
    builder.method(ep.method(), publisher);
    return builder.build();
  }

  private <T> HttpResponse<T> send(EndpointSpec ep, HttpRequest request, HttpResponse.BodyHandler<T> handler) {
    try {
      // #if rules
      if (ownDigest != null) {
        return observe(http.send(request, handler));
      }
      // #endif
      return http.send(request, handler);
    } catch (IOException e) {
      // #if timeouts
      if (e instanceof java.net.http.HttpTimeoutException) {
        throw new VeldTransportError(
            ep.method() + " " + ep.path() + " timed out after " + timeoutOf(ep).toMillis() + " ms", e);
      }
      // #endif
      throw new VeldTransportError(ep.method() + " " + ep.path() + " failed", e);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new VeldTransportError(ep.method() + " " + ep.path() + " was interrupted", e);
    }
  }
  // #if limits

  private final java.util.Set<String> retrying = java.util.concurrent.ConcurrentHashMap.newKeySet();

  /**
   * Tells the core that the endpoint with this operation signature has a limit and is a {@code get} or
   * {@code @idempotent} (spec/platform.md section 5): a call that gets {@code 429} with {@code
   * retryAfter} waits that long and sends again, once.
   */
  public void retryOnLimit(String sig) {
    retrying.add(sig);
  }

  /**
   * Sends the request; a {@code 429} with {@code retryAfter} of an endpoint that may be repeated is
   * waited out and sent again, once. The two requests and the wait are inside the one timeout of
   * the call (spec/platform.md section 9.2): the second request gets what is left of it, and a wait
   * that does not fit ends the call as timed out, without sleeping.
   */
  private HttpResponse<String> sendLimited(EndpointSpec ep, HttpRequest request) {
    long start = System.nanoTime();
    HttpResponse<String> first = send(ep, request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
    if (first.statusCode() != 429 || !options.retryOnLimit() || !retrying.contains(ep.sig())) {
      return first;
    }
    long seconds = retryAfter(first.body());
    if (seconds < 0) {
      return first;
    }
    long budget = request.timeout().map(java.time.Duration::toMillis).orElse(Long.MAX_VALUE);
    long wait = seconds >= Long.MAX_VALUE / 1000L ? Long.MAX_VALUE : seconds * 1000L;
    if (budget - elapsedMillis(start) <= wait) {
      throw timedOut(ep, budget);
    }
    try {
      options.sleeper().sleep(wait);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new VeldTransportError(ep.method() + " " + ep.path() + " was interrupted", e);
    }
    long left = budget - elapsedMillis(start);
    if (left <= 0) {
      throw timedOut(ep, budget);
    }
    // The same request, so the same idempotency key, with the time that is left.
    HttpRequest again = HttpRequest.newBuilder(request, (name, value) -> true)
        .timeout(java.time.Duration.ofMillis(left))
        .build();
    return send(ep, again, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
  }

  private static long elapsedMillis(long startNanos) {
    return (System.nanoTime() - startNanos) / 1_000_000L;
  }

  private static VeldTransportError timedOut(EndpointSpec ep, long budget) {
    return new VeldTransportError(
        ep.method() + " " + ep.path() + " timed out after " + budget + " ms",
        new java.net.http.HttpTimeoutException("the wait of a 429 does not fit in the timeout"));
  }

  /** The {@code retryAfter} of the {@code data} of a problem document, in seconds, or -1 when it has none. */
  private static long retryAfter(String body) {
    try {
      if (Json.parse(body) instanceof Json.Obj problem
          && problem.get("data") instanceof Json.Obj data
          && data.get("retryAfter") instanceof Json.Num seconds
          && seconds.value() >= 0) {
        return (long) seconds.value();
      }
    } catch (Json.ParseException e) {
      return -1;
    }
    return -1;
  }
  // #endif
  // #if timeouts

  private final java.util.Map<String, Long> deadlines = new java.util.concurrent.ConcurrentHashMap<>();

  /**
   * Tells the core the deadline of an endpoint (spec/platform.md section 12), by operation
   * signature: it is the default timeout of its calls when the caller did not set one.
   */
  public void deadline(String sig, long ms) {
    deadlines.put(sig, ms);
  }

  private java.time.Duration timeoutOf(EndpointSpec ep) {
    Long ms = deadlines.get(ep.sig());
    return ms == null || options.timeoutSet() ? options.timeout() : java.time.Duration.ofMillis(ms);
  }
  // #endif

  private URI uri(String pathAndQuery) {
    return uri(baseUrl, pathAndQuery);
  }

  private static URI uri(String base, String pathAndQuery) {
    try {
      return URI.create(base + pathAndQuery);
    } catch (IllegalArgumentException e) {
      throw new VeldTransportError("invalid request URL", e);
    }
  }

  private static EndpointSpec.Param bodyParam(EndpointSpec ep) {
    for (EndpointSpec.Param p : ep.params()) {
      if (p.binding() == EndpointSpec.Binding.BODY) {
        return p;
      }
    }
    return null;
  }

  private static String expandPath(EndpointSpec ep, Json.Obj params) {
    StringBuilder out = new StringBuilder();
    String path = ep.path();
    int i = 0;
    while (i < path.length()) {
      int open = path.indexOf('{', i);
      int close = open < 0 ? -1 : path.indexOf('}', open);
      if (open < 0 || close < 0) {
        out.append(path, i, path.length());
        break;
      }
      out.append(path, i, open);
      Json v = params.get(path.substring(open + 1, close));
      out.append(encodeUriComponent(v == null ? "" : scalarString(v)));
      i = close + 1;
    }
    return out.toString();
  }

  private static String queryString(EndpointSpec ep, Json.Obj params) {
    return joinQuery(queryPairs(ep, params));
  }

  private static List<String[]> queryPairs(EndpointSpec ep, Json.Obj params) {
    List<String[]> pairs = new ArrayList<>();
    for (EndpointSpec.Param p : ep.params()) {
      Json v = params.get(p.name());
      if (p.binding() == EndpointSpec.Binding.QUERY) {
        addPairs(pairs, p.name(), v);
      } else if (p.binding() == EndpointSpec.Binding.QUERY_OBJECT && v != null) {
        Json wire = Wire.prepare(p.type(), v, Wire.Mode.RESPONSE);
        // #if derived
        wire = Derive.strip(p.type(), wire);
        // #endif
        if (wire instanceof Json.Obj obj) {
          obj.fields().forEach((k, x) -> addPairs(pairs, k, x));
        }
      }
    }
    pairs.sort((a, b) -> a[0].compareTo(b[0]));
    return pairs;
  }

  private static String joinQuery(List<String[]> pairs) {
    StringBuilder out = new StringBuilder();
    for (String[] pair : pairs) {
      if (out.length() > 0) {
        out.append('&');
      }
      out.append(encodeUriComponent(pair[0])).append('=').append(encodeUriComponent(pair[1]));
    }
    return out.toString();
  }

  private static void addPairs(List<String[]> pairs, String key, Json v) {
    if (v == null) {
      return;
    }
    if (v instanceof Json.Arr arr) {
      for (Json x : arr.items()) {
        pairs.add(new String[] {key, scalarString(x)});
      }
    } else {
      pairs.add(new String[] {key, scalarString(v)});
    }
  }

  private static String scalarString(Json v) {
    if (v instanceof Json.Str s) {
      return s.value();
    }
    if (v instanceof Json.Num n) {
      return JsonWriter.esNumber(n.value());
    }
    if (v instanceof Json.Bool b) {
      return b.value() ? "true" : "false";
    }
    return v.write();
  }

  /** Percent-encoding with the same safe set as JavaScript's {@code encodeURIComponent}. */
  static String encodeUriComponent(String s) {
    StringBuilder out = new StringBuilder();
    for (byte b : s.getBytes(StandardCharsets.UTF_8)) {
      int c = b & 0xff;
      boolean safe = c >= 'A' && c <= 'Z' || c >= 'a' && c <= 'z' || c >= '0' && c <= '9'
          || "-_.!~*'()".indexOf(c) >= 0;
      if (safe) {
        out.append((char) c);
      } else {
        out.append('%').append(String.format("%02X", c));
      }
    }
    return out.toString();
  }

  private Map<String, String> headers(EndpointSpec ep, Json.Obj params, String accept) {
    Map<String, String> out = new LinkedHashMap<>();
    out.put("accept", accept);
    out.putAll(options.headers().get());
    for (EndpointSpec.Param p : ep.params()) {
      Json v = params.get(p.name());
      if (p.binding() == EndpointSpec.Binding.HEADER && v != null) {
        out.put(p.header() == null ? p.name() : p.header(), scalarString(v));
      }
    }
    addAuth(ep, out);
    return out;
  }

  private void addAuth(EndpointSpec ep, Map<String, String> out) {
    if (ep.auth() == null) {
      return;
    }
    switch (ep.auth().kind()) {
      case BEARER -> {
        if (options.bearer() != null) {
          out.put("authorization", "Bearer " + options.bearer().get());
        }
      }
      case BASIC -> {
        if (options.basic() != null) {
          ClientOptions.Basic c = options.basic().get();
          String raw = c.username() + ":" + c.password();
          out.put("authorization", "Basic " + Base64.getEncoder().encodeToString(raw.getBytes(StandardCharsets.UTF_8)));
        }
      }
      case API_KEY -> {
        if (options.apiKey() != null) {
          out.put(ep.auth().header() == null ? "X-API-Key" : ep.auth().header(), options.apiKey().get());
        }
      }
    }
  }

  // -- response ------------------------------------------------------------------------------

  private static Json decode(EndpointSpec ep, HttpResponse<String> res) {
    return decodeText(ep, res.statusCode(), res.body());
  }

  private static Json decodeText(EndpointSpec ep, int status, String text) {
    Json body = null;
    if (text != null && !text.isBlank()) {
      try {
        body = Json.parse(text);
      } catch (Json.ParseException e) {
        throw new VeldTransportError(
            ep.method() + " " + ep.path() + ": response body is not valid JSON (HTTP " + status + ")", e);
      }
    }
    if (status >= 200 && status < 300) {
      return decodeSuccess(ep, body);
    }
    throw decodeFailure(ep, status, body);
  }

  private static Json decodeSuccess(EndpointSpec ep, Json body) {
    if (ep.success() == null) {
      return null;
    }
    if (body == null) {
      throw new VeldTransportError(ep.method() + " " + ep.path() + ": empty response body");
    }
    return Wire.prepare(ep.success(), body, Wire.Mode.RESPONSE);
  }

  static RuntimeException decodeFailure(EndpointSpec ep, int status, Json body) {
    // #if rules
    RuntimeException withViolations = RuleWalk.decodeProblem(ep, status, body);
    if (withViolations != null) {
      return withViolations;
    }
    // #endif
    if (body instanceof Json.Obj obj && obj.get("type") instanceof Json.Str type) {
      for (EndpointSpec.Declared d : ep.errors()) {
        if (d.sig().equals(type.value())) {
          Json data = obj.get("data") == null ? Json.object().build() : obj.get("data");
          int declared = obj.get("status") instanceof Json.Num n && n.value() != 0 ? (int) n.value() : status;
          String op = obj.get("operation") instanceof Json.Str s ? s.value() : ep.sig();
          return d.factory().create(Wire.prepare(d.schema(), data, Wire.Mode.RESPONSE), declared, op);
        }
      }
    }
    return new VeldHttpError(status, body);
  }
}
