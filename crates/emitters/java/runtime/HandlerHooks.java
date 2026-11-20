package dev.veld.runtime;

import java.util.LinkedHashMap;
import java.util.Map;

/** The hooks a handler calls. Generated {@code <Service>Hooks} classes build one. */
public final class HandlerHooks {

  /** Authenticates a bearer token; returns the principal, or {@code null} to reject. */
  @FunctionalInterface
  public interface BearerAuth {
    /** Authenticates. */
    Object authenticate(String token, RequestContext ctx) throws Exception;
  }

  /** Authenticates basic credentials; returns the principal, or {@code null} to reject. */
  @FunctionalInterface
  public interface BasicAuth {
    /** Authenticates. */
    Object authenticate(String username, String password, RequestContext ctx) throws Exception;
  }

  /** Authenticates an API key; returns the principal, or {@code null} to reject. */
  @FunctionalInterface
  public interface ApiKeyAuth {
    /** Authenticates. */
    Object authenticate(String key, RequestContext ctx) throws Exception;
  }

  /** A named middleware, run after authentication and before the request is bound. */
  @FunctionalInterface
  public interface Middleware {
    /** Runs; throwing an error ends the request. */
    void run(RequestContext ctx) throws Exception;
  }

  /** Observes errors that become {@code Internal}; a failing hook never changes the response. */
  @FunctionalInterface
  public interface ErrorHook {
    /** Called with the original, undeclared failure. */
    void onError(Throwable error, RequestContext ctx);
  }

  private final BearerAuth bearer;
  private final BasicAuth basic;
  private final ApiKeyAuth apiKey;
  private final Map<String, Middleware> middleware;
  private final ErrorHook onError;
  private final boolean validateResponses;

  private HandlerHooks(Builder b) {
    this.bearer = b.bearer;
    this.basic = b.basic;
    this.apiKey = b.apiKey;
    this.middleware = Map.copyOf(b.middleware);
    this.onError = b.onError;
    this.validateResponses = b.validateResponses;
  }

  /** Starts a builder. */
  public static Builder builder() {
    return new Builder();
  }

  BearerAuth bearer() {
    return bearer;
  }

  BasicAuth basic() {
    return basic;
  }

  ApiKeyAuth apiKey() {
    return apiKey;
  }

  Middleware middleware(String name) {
    return middleware.get(name);
  }

  ErrorHook onError() {
    return onError;
  }

  boolean validateResponses() {
    return validateResponses;
  }

  /** Builds {@link HandlerHooks}. */
  public static final class Builder {
    private BearerAuth bearer;
    private BasicAuth basic;
    private ApiKeyAuth apiKey;
    private final Map<String, Middleware> middleware = new LinkedHashMap<>();
    private ErrorHook onError;
    private boolean validateResponses;

    private Builder() {}

    /** Sets the bearer-token hook. */
    public Builder bearer(BearerAuth hook) {
      this.bearer = hook;
      return this;
    }

    /** Sets the basic-credentials hook. */
    public Builder basic(BasicAuth hook) {
      this.basic = hook;
      return this;
    }

    /** Sets the API-key hook. */
    public Builder apiKey(ApiKeyAuth hook) {
      this.apiKey = hook;
      return this;
    }

    /** Registers a middleware by the name used in the contract. */
    public Builder middleware(String name, Middleware hook) {
      this.middleware.put(name, hook);
      return this;
    }

    /** Sets the error observer. */
    public Builder onError(ErrorHook hook) {
      this.onError = hook;
      return this;
    }

    /** When true, a response that fails validation becomes a 500 (default false). */
    public Builder validateResponses(boolean value) {
      this.validateResponses = value;
      return this;
    }

    /** The finished hooks. */
    public HandlerHooks build() {
      return new HandlerHooks(this);
    }
  }
}
