package dev.veld.adapters;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpHandler;
import com.sun.net.httpserver.HttpServer;
import dev.veld.runtime.VeldHandler;
import dev.veld.runtime.VeldRequest;
import dev.veld.runtime.VeldResponse;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
// veld:sse
import java.util.Iterator;
// veld:end
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;
// veld:sse
import java.util.stream.Stream;
// veld:end

/**
 * Mounts a {@link VeldHandler} on the JDK's built-in {@code com.sun.net.httpserver} (module
 * {@code jdk.httpserver}). Nothing outside the JDK is needed.
 */
public final class JdkHttpServerAdapter {
  private JdkHttpServerAdapter() {}

  /** An {@link HttpHandler} that serves {@code handler}; unmatched requests get an empty 404. */
  public static HttpHandler toHttpHandler(VeldHandler handler) {
    return exchange -> {
      try (exchange) {
        VeldRequest request = read(exchange);
        Optional<VeldResponse> response = handler.handle(request);
        write(exchange, response.orElseGet(() -> new VeldResponse(404, Map.of(), null)));
      }
    };
  }

  /**
   * Starts a server on {@code address} (use port 0 to pick a free port) that serves {@code
   * handler} for every path. Requests run on daemon threads. Call {@link HttpServer#stop(int)} to
   * stop it.
   */
  public static HttpServer listen(VeldHandler handler, InetSocketAddress address) throws IOException {
    HttpServer server = HttpServer.create(address, 0);
    server.createContext("/", toHttpHandler(handler));
    ThreadFactory daemon = task -> {
      Thread t = new Thread(task, "veld-http");
      t.setDaemon(true);
      return t;
    };
    ExecutorService executor = Executors.newCachedThreadPool(daemon);
    server.setExecutor(executor);
    server.start();
    return server;
  }

  private static VeldRequest read(HttpExchange exchange) throws IOException {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    exchange.getRequestHeaders().forEach((k, v) -> headers.put(k, new ArrayList<>(v)));
    String body = new String(exchange.getRequestBody().readAllBytes(), StandardCharsets.UTF_8);
    return new VeldRequest(
        exchange.getRequestMethod(),
        exchange.getRequestURI().getRawPath(),
        exchange.getRequestURI().getRawQuery(),
        headers,
        body,
        exchange);
  }

  private static void write(HttpExchange exchange, VeldResponse response) throws IOException {
    response.headers().forEach((k, v) -> exchange.getResponseHeaders().set(k, v));
    // veld:sse
    if (response.frames() != null) {
      stream(exchange, response);
      return;
    }
    // veld:end
    if (response.body() == null) {
      exchange.sendResponseHeaders(response.status(), -1);
      return;
    }
    byte[] bytes = response.body().getBytes(StandardCharsets.UTF_8);
    exchange.sendResponseHeaders(response.status(), bytes.length);
    try (OutputStream out = exchange.getResponseBody()) {
      out.write(bytes);
    }
  }
  // veld:sse

  /**
   * Server-Sent Events: each frame is written and flushed as it arrives. A write that fails means
   * the client went away; the stream is closed either way, which tells the implementation.
   */
  private static void stream(HttpExchange exchange, VeldResponse response) throws IOException {
    exchange.sendResponseHeaders(response.status(), 0);
    try (Stream<String> frames = response.frames();
        OutputStream out = exchange.getResponseBody()) {
      Iterator<String> it = frames.iterator();
      while (it.hasNext()) {
        out.write(it.next().getBytes(StandardCharsets.UTF_8));
        out.flush();
      }
    } catch (IOException e) {
      // The client closed the connection.
    }
  }
  // veld:end
}
