package dev.veld.runtime;

import java.util.Map;
// veld:sse
import java.util.stream.Stream;
// veld:end

/**
 * An HTTP response produced by a handler.
 *
 * @param status the HTTP status
 * @param headers the response headers
 * @param body the body as UTF-8 text, or {@code null} for none
 * @param frames Server-Sent Events: the ready-made frames, pulled as the adapter writes them; the
 *     status and headers are final once this is set, and the adapter closes the stream when the
 *     client goes away. {@code null} for an ordinary response.
 */
public record VeldResponse(
    int status,
    Map<String, String> headers,
    String body
    // veld:sse
    , Stream<String> frames
    // veld:end
    ) {
  /** Creates a response (the headers are copied). */
  public VeldResponse {
    headers = Map.copyOf(headers);
  }

  // veld:sse
  /** Creates an ordinary response without frames. */
  public VeldResponse(int status, Map<String, String> headers, String body) {
    this(status, headers, body, null);
  }
  // veld:end
}
