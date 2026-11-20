package dev.veld.runtime;

import java.util.List;
import java.util.Map;

/** What a service implementation, an auth hook and a middleware hook know about a request. */
public final class RequestContext {
  private final String method;
  private final String path;
  private final Map<String, List<String>> headers;
  private final Object raw;
  private volatile Object principal;

  RequestContext(String method, String path, Map<String, List<String>> headers, Object raw) {
    this.method = method;
    this.path = path;
    this.headers = headers;
    this.raw = raw;
  }

  /** The HTTP method of the matched endpoint. */
  public String method() {
    return method;
  }

  /** The request path, without a trailing slash. */
  public String path() {
    return path;
  }

  /** The request headers, with lower-case names. */
  public Map<String, List<String>> headers() {
    return headers;
  }

  /** The first value of a header, or {@code null}. */
  public String header(String name) {
    List<String> values = headers.get(name.toLowerCase(java.util.Locale.ROOT));
    return values == null || values.isEmpty() ? null : values.get(0);
  }

  /** What the auth hook returned for this request, or {@code null} for an endpoint without auth. */
  public Object principal() {
    return principal;
  }

  void setPrincipal(Object value) {
    this.principal = value;
  }

  /** The framework's request object (for example an {@code HttpExchange}). */
  public Object raw() {
    return raw;
  }
}
