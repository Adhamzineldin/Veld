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
    return decode(ep, send(ep, request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8)));
  }

  private void check(EndpointSpec ep, Json.Obj params) {
    if (options.validate(defaults.validate())) {
      List<Issue> issues = requestIssues(ep, params);
      if (!issues.isEmpty()) {
        VeldError err = defaults.validation().apply(issues);
        err.setOp(ep.sig());
        throw err;
      }
    }
  }
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
    InputStream first = retrying(policy, () -> openEvents(ep, params, sse.lastEventId()));
    return SseStream.open(ep, first, sse, policy, last -> openEvents(ep, params, last));
  }

  /** Runs {@code attempt}, retrying connection failures with backoff when a policy is set. */
  private static <T> T retrying(Reconnect policy, java.util.function.Supplier<T> attempt) {
    int failures = 0;
    while (true) {
      try {
        return attempt.get();
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
    HttpResponse<InputStream> res = send(ep, request, HttpResponse.BodyHandlers.ofInputStream());
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
        .timeout(options.timeout());
    EndpointSpec.Param body = bodyParam(ep);
    HttpRequest.BodyPublisher publisher = HttpRequest.BodyPublishers.noBody();
    Map<String, String> headers = headers(ep, params, accept);
    headers.putAll(extra);
    if (body != null && params.get(body.name()) != null) {
      headers.put("content-type", "application/json");
      Json wire = Wire.prepare(body.type(), params.get(body.name()), Wire.Mode.RESPONSE);
      publisher = HttpRequest.BodyPublishers.ofString(wire.write(), StandardCharsets.UTF_8);
    }
    headers.forEach(builder::header);
    builder.method(ep.method(), publisher);
    return builder.build();
  }

  private <T> HttpResponse<T> send(EndpointSpec ep, HttpRequest request, HttpResponse.BodyHandler<T> handler) {
    try {
      return http.send(request, handler);
    } catch (IOException e) {
      throw new VeldTransportError(ep.method() + " " + request.uri().getRawPath() + " failed", e);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new VeldTransportError(ep.method() + " " + request.uri().getRawPath() + " was interrupted", e);
    }
  }

  private URI uri(String pathAndQuery) {
    return uri(baseUrl, pathAndQuery);
  }

  private static URI uri(String base, String pathAndQuery) {
    try {
      return URI.create(base + pathAndQuery);
    } catch (IllegalArgumentException e) {
      throw new VeldTransportError("invalid request URL " + base + pathAndQuery, e);
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
