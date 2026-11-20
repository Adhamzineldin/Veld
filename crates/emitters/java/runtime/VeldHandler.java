package dev.veld.runtime;

import java.util.Optional;

/** The framework-agnostic server core: turns a request into a response. */
@FunctionalInterface
public interface VeldHandler {
  /**
   * Handles a request.
   *
   * @return the response, or empty when no route matched
   */
  Optional<VeldResponse> handle(VeldRequest request);
}
