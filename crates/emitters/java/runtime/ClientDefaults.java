package dev.veld.runtime;

import java.util.List;
import java.util.function.Function;

/**
 * What a generated client knows about its service.
 *
// #if legacy
 * @param envVar the environment variable consulted for the base URL, such as {@code VELD_ORDERS_URL}
 * @param baseUrl the base URL baked in from the contract, or {@code null}
// #endif
 * @param validation builds the local {@code ValidationError} for a request that fails validation
 * @param validate whether requests are validated unless {@link ClientOptions.Builder#validate} says otherwise
 */
// #if legacy
public record ClientDefaults(
    String envVar, String baseUrl, Function<List<Issue>, VeldError> validation, boolean validate) {

  /** Defaults that validate requests unless the options say otherwise. */
  public ClientDefaults(
      String envVar, String baseUrl, Function<List<Issue>, VeldError> validation) {
    this(envVar, baseUrl, validation, true);
  }
}
// #endif
// #if runtimecfg
public record ClientDefaults(Function<List<Issue>, VeldError> validation, boolean validate) {

  /** Defaults that validate requests unless the options say otherwise. */
  public ClientDefaults(Function<List<Issue>, VeldError> validation) {
    this(validation, true);
  }
}
// #endif
