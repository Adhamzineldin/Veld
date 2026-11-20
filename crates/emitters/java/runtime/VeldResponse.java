package dev.veld.runtime;

import java.util.Map;

/**
 * An HTTP response produced by a handler.
 *
 * @param status the HTTP status
 * @param headers the response headers
 * @param body the body as UTF-8 text, or {@code null} for none
 */
public record VeldResponse(int status, Map<String, String> headers, String body) {
  /** Creates a response (the headers are copied). */
  public VeldResponse {
    headers = Map.copyOf(headers);
  }
}
