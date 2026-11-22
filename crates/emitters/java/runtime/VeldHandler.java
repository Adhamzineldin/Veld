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

  /**
   * Runs one named endpoint through the same pipeline as {@link #handle}, without choosing the
   * endpoint from the method and path. Frameworks that do their own routing (Spring MVC) use it
   * so the semantics live in one place.
   *
   * @param endpoint the endpoint name, as in {@code EndpointSpec.name()}
   * @return the response, or empty when the endpoint is unknown or the path does not fit it
   */
  default Optional<VeldResponse> invoke(String endpoint, VeldRequest request) {
    return handle(request);
  }
  // veld:ws

  /**
   * Serves an upgraded WebSocket through the same pipeline as HTTP (authentication, middleware,
   * binding, validation, typed errors). Auth can also come from the query string:
   * {@code access_token} (bearer) and {@code api_key} (API key, when its header is absent).
   * The implementation runs on its own thread; frames that arrive before authentication and
   * middleware finish are held back and then processed.
   *
   * @param request the upgrade request; its path and query select the endpoint
   * @param socket the transport of the accepted socket
   * @return false when no {@code ws} endpoint matches the path (the adapter closes with 4404)
   */
  default boolean ws(VeldRequest request, WsSocket socket) {
    return false;
  }
  // veld:end
}
