package dev.veld.runtime;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;

/**
 * Sends {@code Veld-Rules: <digest>} on every response of a service that has a rules digest
 * (spec/rules.md section 9.7), errors included. A request no route matches gets nothing.
 */
public final class DigestHandler implements VeldHandler {
  /** The name of the response header. */
  public static final String HEADER = "Veld-Rules";

  private final VeldHandler inner;
  private final String digest;

  private DigestHandler(VeldHandler inner, String digest) {
    this.inner = inner;
    this.digest = digest;
  }

  /** Wraps {@code inner}. */
  public static VeldHandler wrap(VeldHandler inner, String digest) {
    return new DigestHandler(inner, digest);
  }

  @Override
  public Optional<VeldResponse> handle(VeldRequest request) {
    return inner.handle(request).map(this::stamp);
  }

  @Override
  public Optional<VeldResponse> invoke(String endpoint, VeldRequest request) {
    return inner.invoke(endpoint, request).map(this::stamp);
  }
  // veld:ws

  @Override
  public boolean ws(VeldRequest request, WsSocket socket) {
    return inner.ws(request, socket);
  }
  // veld:end

  private VeldResponse stamp(VeldResponse response) {
    Map<String, String> headers = new LinkedHashMap<>(response.headers());
    headers.put(HEADER, digest);
    VeldResponse out = new VeldResponse(response.status(), headers, response.body());
    // veld:sse
    if (response.frames() != null) {
      out = new VeldResponse(response.status(), headers, response.body(), response.frames());
    }
    // veld:end
    return out;
  }
}
