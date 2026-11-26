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

  // #if slo
  /**
   * Observes every request that matched an endpoint of the server (spec/platform.md section 13): called
   * once, when the answer is ready to be written, with the duration and the attributes of the OpenTelemetry
   * HTTP conventions. A failing hook is ignored.
   */
  @FunctionalInterface
  public interface MetricsHook {
    /** Called with the measurement of one request. */
    void metrics(Metric metric);
  }

  // #endif
  // #if conditional
  /**
   * The caller of a request, as text, for the scope of an idempotency key. The scope of a key
   * includes the caller only for an endpoint with authentication.
   */
  @FunctionalInterface
  public interface IdempotencyCaller {
    /** Returns the caller text; the same caller must always give the same text. */
    String caller(Object principal, RequestContext ctx) throws Exception;
  }

  // #endif
  private final BearerAuth bearer;
  private final BasicAuth basic;
  private final ApiKeyAuth apiKey;
  private final Map<String, Middleware> middleware;
  private final ErrorHook onError;
  private final boolean validateResponses;
  private final boolean validateRequests;
  // #if rules
  private final RuleWalk.OutputMode rulesOutput;
  private final RuleWalk.Hook ruleHook;
  // #endif
  // #if conditional
  private final IdempotencyStore idempotencyStore;
  private final IdempotencyCaller idempotencyCaller;
  // #endif
  // #if slo
  private final MetricsHook metrics;
  // #endif

  private HandlerHooks(Builder b) {
    this.bearer = b.bearer;
    this.basic = b.basic;
    this.apiKey = b.apiKey;
    this.middleware = Map.copyOf(b.middleware);
    this.onError = b.onError;
    this.validateResponses = b.validateResponses;
    this.validateRequests = b.validateRequests;
    // #if rules
    this.rulesOutput = b.rulesOutput;
    this.ruleHook = b.ruleHook;
    // #endif
    // #if conditional
    this.idempotencyStore = b.idempotencyStore;
    this.idempotencyCaller = b.idempotencyCaller;
    // #endif
    // #if slo
    this.metrics = b.metrics;
    // #endif
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

  boolean validateRequests() {
    return validateRequests;
  }
  // #if slo

  MetricsHook metrics() {
    return metrics;
  }
  // #endif
  // #if rules

  RuleWalk.OutputMode rulesOutput() {
    return rulesOutput;
  }

  RuleWalk.Hook ruleHook() {
    return ruleHook;
  }
  // #endif
  // #if conditional

  IdempotencyStore idempotencyStore() {
    return idempotencyStore;
  }

  IdempotencyCaller idempotencyCaller() {
    return idempotencyCaller;
  }

  /** These hooks with another idempotency store and caller (the test handler brings its own). */
  public HandlerHooks withIdempotency(IdempotencyStore store, IdempotencyCaller caller) {
    return builder().copyOf(this).idempotencyStore(store).idempotencyCaller(caller).build();
  }
  // #endif
  // #if testing_rules

  /** These hooks with another output mode of the rules (the test handler takes it as an argument). */
  public HandlerHooks withRulesOutput(RuleWalk.OutputMode mode) {
    Builder b = builder();
    b.bearer = bearer;
    b.basic = basic;
    b.apiKey = apiKey;
    b.middleware.putAll(middleware);
    b.onError = onError;
    b.validateResponses = validateResponses;
    b.validateRequests = validateRequests;
    b.ruleHook = ruleHook;
    b.rulesOutput = mode == null ? RuleWalk.OutputMode.STRICT : mode;
    // #endif
    // #if testing_rules_conditional
    b.idempotencyStore = idempotencyStore;
    b.idempotencyCaller = idempotencyCaller;
    // #endif
    // #if testing_rules
    return b.build();
  }
  // #endif

  /** Builds {@link HandlerHooks}. */
  public static final class Builder {
    private BearerAuth bearer;
    private BasicAuth basic;
    private ApiKeyAuth apiKey;
    private final Map<String, Middleware> middleware = new LinkedHashMap<>();
    private ErrorHook onError;
    private boolean validateResponses;
    private boolean validateRequests = true;
    // #if rules
    private RuleWalk.OutputMode rulesOutput = RuleWalk.OutputMode.STRICT;
    private RuleWalk.Hook ruleHook;
    // #endif
    // #if conditional
    private IdempotencyStore idempotencyStore;
    private IdempotencyCaller idempotencyCaller;
    // #endif
    // #if slo
    private MetricsHook metrics;
    // #endif

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

    /** When false, request parameters and bodies are decoded but not validated (default true). */
    public Builder validateRequests(boolean value) {
      this.validateRequests = value;
      return this;
    }

    // #if rules
    /**
     * What happens when a rule is violated by the server's own output: {@code STRICT} (default)
     * answers {@code 500 Internal}, {@code AUDIT} sends the response and reports, {@code OFF}
     * does not evaluate output rules.
     */
    public Builder rulesOutput(RuleWalk.OutputMode value) {
      this.rulesOutput = value == null ? RuleWalk.OutputMode.STRICT : value;
      return this;
    }

    /** Observes every violated rule; a failing hook never changes the response. */
    public Builder onRuleViolation(RuleWalk.Hook hook) {
      this.ruleHook = hook;
      return this;
    }

    // #endif
    // #if conditional
    /** The store of the {@code @idempotent} endpoints. */
    public Builder idempotencyStore(IdempotencyStore store) {
      this.idempotencyStore = store;
      return this;
    }

    /** Tells who the caller is, for the scope of an idempotency key. */
    public Builder idempotencyCaller(IdempotencyCaller hook) {
      this.idempotencyCaller = hook;
      return this;
    }

    /** Starts from the hooks of {@code other}. */
    Builder copyOf(HandlerHooks other) {
      this.bearer = other.bearer;
      this.basic = other.basic;
      this.apiKey = other.apiKey;
      this.middleware.putAll(other.middleware);
      this.onError = other.onError;
      this.validateResponses = other.validateResponses;
      this.validateRequests = other.validateRequests;
      // #endif
      // #if conditional_rules
      this.rulesOutput = other.rulesOutput;
      this.ruleHook = other.ruleHook;
      // #endif
      // #if conditional
      return this;
    }

    // #endif
    // #if slo
    /** Observes the duration of every request that matched an endpoint of the server. */
    public Builder metrics(MetricsHook hook) {
      this.metrics = hook;
      return this;
    }

    // #endif
    /** The finished hooks. */
    public HandlerHooks build() {
      return new HandlerHooks(this);
    }
  }
}
