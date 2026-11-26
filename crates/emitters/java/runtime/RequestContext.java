package dev.veld.runtime;

import java.util.List;
import java.util.Locale;
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
    List<String> values = headers.get(name.toLowerCase(Locale.ROOT));
    return values == null || values.isEmpty() ? null : values.get(0);
  }

  /** What the auth hook returned for this request, or {@code null} for an endpoint without auth. */
  public Object principal() {
    return principal;
  }

  void setPrincipal(Object value) {
    this.principal = value;
  }
  // #if conditional

  private volatile Long ifMatchVersion;

  /** Records the version of the {@code If-Match} tag the request carried (step 5). */
  void setIfMatchVersion(Long value) {
    this.ifMatchVersion = value;
  }

  /** The version of the tag the request carried, or {@code null}: no header, or {@code *}. */
  Long ifMatchVersion() {
    return ifMatchVersion;
  }
  // #endif
  // #if flags

  private volatile Flags.View flagView;

  void setFlags(Flags.View view) {
    this.flagView = view;
  }

  /** The flags this request read, for the checks that run outside its thread (the items of a stream). */
  Flags.View flags() {
    return flagView;
  }
  // #endif
  // #if timeouts

  private volatile Deadline bound;

  void setDeadline(Deadline value) {
    this.bound = value;
  }

  /**
   * The instant by which the endpoint must have answered (the arrival of the request plus its
   * {@code @timeout}), or {@code null} for an endpoint without a deadline. The handler runs on a
   * thread that cannot be stopped: when the deadline passes the server answers {@code 504
   * DeadlineExceeded}, drops the answer the handler gives later and interrupts the handler's thread
   * once. A handler that writes must watch {@link #expired()} or the interrupt.
   */
  public java.time.Instant deadline() {
    Deadline d = bound;
    return d == null ? null : d.at();
  }

  /** Whether the deadline has passed (or the server has answered {@code 504}); {@code false} without one. */
  public boolean expired() {
    Deadline d = bound;
    return d != null && d.expired();
  }
  // #endif

  /** The framework's request object (for example an {@code HttpExchange}). */
  public Object raw() {
    return raw;
  }
}
