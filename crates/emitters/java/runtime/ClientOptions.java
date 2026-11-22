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

  private final String baseUrl;
  private final HttpClient httpClient;
  private final Supplier<Map<String, String>> headers;
  private final Supplier<String> bearer;
  private final Supplier<Basic> basic;
  private final Supplier<String> apiKey;
  private final Boolean validate;
  private final Duration timeout;
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
    // veld:streaming
    this.reconnect = b.reconnect;
    // veld:end
  }

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

  boolean validate(boolean fallback) {
    return validate == null ? fallback : validate;
  }

  Duration timeout() {
    return timeout;
  }

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
    // veld:streaming
    private Reconnect reconnect;
    // veld:end

    private Builder() {}

    /** The base URL; overrides the {@code VELD_<SERVICE>_URL} variable and the contract's default. */
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
      return this;
    }

    // veld:streaming
    /**
     * Reconnects Server-Sent Events and WebSocket calls after a connection failure (default: off).
     */
    public Builder reconnect(Reconnect value) {
      this.reconnect = value;
      return this;
    }
    // veld:end

    /** The finished options. */
    public ClientOptions build() {
      return new ClientOptions(this);
    }
  }
}
