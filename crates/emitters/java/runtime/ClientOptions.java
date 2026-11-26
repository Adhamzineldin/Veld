package dev.veld.runtime;

import java.net.http.HttpClient;
import java.time.Duration;
import java.util.Map;
import java.util.function.Supplier;

/** Options of a generated client. Build one with {@link #builder()}. */
public final class ClientOptions {

  /**
   * Basic-auth credentials.
   *
   * @param username the user name
   * @param password the password
   */
  public record Basic(String username, String password) {}

  // #if rules
  /**
   * A server answered with another rules digest than this client was generated with.
   *
   * @param service the service name
   * @param clientDigest the digest of the client
   * @param serverDigest the digest the server sent
   */
  public record ContractMismatch(String service, String clientDigest, String serverDigest) {}

  // #endif
  private final String baseUrl;
  private final HttpClient httpClient;
  private final Supplier<Map<String, String>> headers;
  private final Supplier<String> bearer;
  private final Supplier<Basic> basic;
  private final Supplier<String> apiKey;
  private final Boolean validate;
  private final Duration timeout;
  // #if timeouts
  private final boolean timeoutSet;
  // #endif
  // #if limits
  private final boolean retryOnLimit;
  private final Sleeper sleeper;
  // #endif
  // #if rules
  private final boolean localRules;
  private final java.util.function.Consumer<ContractMismatch> onContractMismatch;
  // #endif
  // veld:streaming
  private final Reconnect reconnect;
  // veld:end

  private ClientOptions(Builder b) {
    this.baseUrl = b.baseUrl;
    this.httpClient = b.httpClient;
    this.headers = b.headers;
    this.bearer = b.bearer;
    this.basic = b.basic;
    this.apiKey = b.apiKey;
    this.validate = b.validate;
    this.timeout = b.timeout;
    // #if timeouts
    this.timeoutSet = b.timeoutSet;
    // #endif
    // #if limits
    this.retryOnLimit = b.retryOnLimit;
    this.sleeper = b.sleeper;
    // #endif
    // #if rules
    this.localRules = b.localRules;
    this.onContractMismatch = b.onContractMismatch;
    // #endif
    // veld:streaming
    this.reconnect = b.reconnect;
    // veld:end
  }

  // #if limits
  /** Waits for {@code millis} milliseconds: a client that gets {@code 429} waits through this, so a test does not. */
  @FunctionalInterface
  public interface Sleeper {
    /** Waits. */
    void sleep(long millis) throws InterruptedException;
  }

  // #endif
  /** Options with everything at its default. */
  public static ClientOptions defaults() {
    return builder().build();
  }

  /** Starts a builder. */
  public static Builder builder() {
    return new Builder();
  }

  String baseUrl() {
    return baseUrl;
  }
  // #if runtimecfg

  /** These options with another base URL (the runtime config gives each client its own). */
  public ClientOptions withBaseUrl(String value) {
    Builder b = new Builder();
    b.baseUrl = value;
    b.httpClient = httpClient;
    b.headers = headers;
    b.bearer = bearer;
    b.basic = basic;
    b.apiKey = apiKey;
    b.validate = validate;
    b.timeout = timeout;
  // #endif
  // #if timeouts_runtimecfg
    b.timeoutSet = timeoutSet;
  // #endif
  // #if limits_runtimecfg
    b.retryOnLimit = retryOnLimit;
    b.sleeper = sleeper;
  // #endif
  // #if runtimecfg_rules
    b.localRules = localRules;
    b.onContractMismatch = onContractMismatch;
  // #endif
  // #if runtimecfg
  // veld:streaming
    b.reconnect = reconnect;
  // veld:end
    return b.build();
  }
  // #endif

  HttpClient httpClient() {
    return httpClient;
  }

  Supplier<Map<String, String>> headers() {
    return headers;
  }

  Supplier<String> bearer() {
    return bearer;
  }

  Supplier<Basic> basic() {
    return basic;
  }

  Supplier<String> apiKey() {
    return apiKey;
  }

  // #if limits
  boolean retryOnLimit() {
    return retryOnLimit;
  }

  Sleeper sleeper() {
    return sleeper;
  }

  // #endif
  boolean validate(boolean fallback) {
    return validate == null ? fallback : validate;
  }

  Duration timeout() {
    return timeout;
  }
  // #if timeouts

  /** Whether the caller chose a timeout: without one, the deadline of an endpoint is its default. */
  boolean timeoutSet() {
    return timeoutSet;
  }
  // #endif
  // #if rules

  java.util.function.Consumer<ContractMismatch> onContractMismatch() {
    return onContractMismatch;
  }

  boolean localRules() {
    return localRules;
  }
  // #endif

  // veld:streaming
  Reconnect reconnect() {
    return reconnect;
  }
  // veld:end

  /** Builds {@link ClientOptions}. */
  public static final class Builder {
    private String baseUrl;
    private HttpClient httpClient;
    private Supplier<Map<String, String>> headers = Map::of;
    private Supplier<String> bearer;
    private Supplier<Basic> basic;
    private Supplier<String> apiKey;
    private Boolean validate;
    private Duration timeout = Duration.ofSeconds(30);
    // #if timeouts
    private boolean timeoutSet;
    // #endif
    // #if limits
    private boolean retryOnLimit = true;
    private Sleeper sleeper = Thread::sleep;
    // #endif
    // #if rules
    private boolean localRules = true;
    private java.util.function.Consumer<ContractMismatch> onContractMismatch;
    // #endif
    // veld:streaming
    private Reconnect reconnect;
    // veld:end

    private Builder() {}

    // #if legacy
    /** The base URL; overrides the {@code VELD_<SERVICE>_URL} variable and the contract's default. */
    // #endif
    // #if runtimecfg
    /** The base URL of the service. A client requires it. */
    // #endif
    public Builder baseUrl(String value) {
      this.baseUrl = value;
      return this;
    }

    /** The HTTP client to use; defaults to {@link HttpClient#newHttpClient()}. */
    public Builder httpClient(HttpClient value) {
      this.httpClient = value;
      return this;
    }

    /** Headers added to every request, evaluated per request. */
    public Builder headers(Supplier<Map<String, String>> value) {
      this.headers = value;
      return this;
    }

    /** Headers added to every request. */
    public Builder headers(Map<String, String> value) {
      Map<String, String> copy = Map.copyOf(value);
      this.headers = () -> copy;
      return this;
    }

    /** Supplies the bearer token for endpoints that need one. */
    public Builder bearer(Supplier<String> value) {
      this.bearer = value;
      return this;
    }

    /** Supplies basic credentials for endpoints that need them. */
    public Builder basic(Supplier<Basic> value) {
      this.basic = value;
      return this;
    }

    /** Supplies the API key for endpoints that need one. */
    public Builder apiKey(Supplier<String> value) {
      this.apiKey = value;
      return this;
    }

    /** Validates requests before sending them (default true, unless the target was generated with {@code validate = false}). */
    public Builder validate(boolean value) {
      this.validate = value;
      return this;
    }

    /** The per-request timeout (default 30 seconds). */
    public Builder timeout(Duration value) {
      this.timeout = value;
      // #if timeouts
      this.timeoutSet = true;
      // #endif
      return this;
    }

    // #if rules
    /**
     * Checks the rules of a request body and of a query object before sending (default true). A
     * violation is thrown as the typed {@code ValidationError} without sending anything. Needs
     * {@link #validate} to be on.
     */
    public Builder localRules(boolean value) {
      this.localRules = value;
      return this;
    }

    /**
     * Called once for every rules digest a server answers with that differs from the one this
     * client was generated with: the server's rules changed, so the local check is switched off
     * (spec/rules.md section 9.7).
     */
    public Builder onContractMismatch(java.util.function.Consumer<ContractMismatch> callback) {
      this.onContractMismatch = callback;
      return this;
    }

    // #endif
    // veld:streaming
    /**
     * Reconnects Server-Sent Events and WebSocket calls after a connection failure (default: off).
     */
    public Builder reconnect(Reconnect value) {
      this.reconnect = value;
      return this;
    }
    // veld:end

    // #if limits
    /**
     * Whether a call that gets {@code 429} with {@code retryAfter} waits that long and sends again,
     * once, when the endpoint is a {@code get} or {@code @idempotent} (default true). With {@code
     * false} the typed error is raised at once.
     */
    public Builder retryOnLimit(boolean value) {
      this.retryOnLimit = value;
      return this;
    }

    /** What the client waits through before it sends again after a {@code 429} (default {@code Thread.sleep}). */
    public Builder sleeper(Sleeper value) {
      this.sleeper = value;
      return this;
    }

    // #endif
    /** The finished options. */
    public ClientOptions build() {
      return new ClientOptions(this);
    }
  }
}
