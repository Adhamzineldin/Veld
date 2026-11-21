package dev.veld.adapters;

import dev.veld.runtime.VeldHandler;
import dev.veld.runtime.VeldRequest;
import dev.veld.runtime.WsSocket;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.net.URI;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;
import org.springframework.web.socket.CloseStatus;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketHandler;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.handler.TextWebSocketHandler;

/**
 * The bridge between Spring WebSocket and the Veld core, used by the generated
 * {@code <Service>WebSocketConfig} of every service with {@code ws} endpoints. It hands each
 * accepted socket to {@link VeldHandler#ws}, which runs the endpoint through the shared pipeline.
 * A path that is not a {@code ws} endpoint is closed with code 4404.
 *
 * <p>This is the only generated file, besides the configuration classes, that imports
 * {@code spring-boot-starter-websocket}: it is the declared dependency exception for WebSocket.
 */
public final class SpringWebSocketAdapter {
  private SpringWebSocketAdapter() {}

  private static final String SOCKET = "veld.socket";

  /** A Spring {@link WebSocketHandler} that serves {@code handler}. */
  public static WebSocketHandler handler(VeldHandler handler) {
    return new Bridge(handler);
  }

  private static final class Bridge extends TextWebSocketHandler {
    private final VeldHandler handler;

    Bridge(VeldHandler handler) {
      this.handler = handler;
    }

    @Override
    public void afterConnectionEstablished(WebSocketSession session) throws IOException {
      Socket socket = new Socket(session);
      session.getAttributes().put(SOCKET, socket);
      URI uri = session.getUri();
      Map<String, List<String>> headers = new LinkedHashMap<>();
      session.getHandshakeHeaders().forEach((k, v) -> headers.put(k, new ArrayList<>(v)));
      VeldRequest request = new VeldRequest(
          "WS",
          uri == null ? "/" : uri.getRawPath(),
          uri == null ? null : uri.getRawQuery(),
          headers,
          null,
          session);
      if (!handler.ws(request, socket)) {
        session.close(new CloseStatus(4404, "no such endpoint"));
      }
    }

    @Override
    protected void handleTextMessage(WebSocketSession session, TextMessage message) {
      if (session.getAttributes().get(SOCKET) instanceof Socket socket) {
        socket.deliver(message.getPayload());
      }
    }

    @Override
    public void afterConnectionClosed(WebSocketSession session, CloseStatus status) {
      if (session.getAttributes().get(SOCKET) instanceof Socket socket) {
        socket.closed();
      }
    }
  }

  private static final class Socket implements WsSocket {
    private final WebSocketSession session;
    private volatile Consumer<String> onMessage = text -> { };
    private volatile Runnable onClose = () -> { };

    Socket(WebSocketSession session) {
      this.session = session;
    }

    @Override
    public void send(String text) {
      try {
        synchronized (session) {
          session.sendMessage(new TextMessage(text));
        }
      } catch (IOException e) {
        throw new UncheckedIOException(e);
      }
    }

    @Override
    public void close(int code, String reason) {
      try {
        session.close(new CloseStatus(code, reason));
      } catch (IOException e) {
        // Already closed.
      }
    }

    @Override
    public void onMessage(Consumer<String> listener) {
      this.onMessage = listener;
    }

    @Override
    public void onClose(Runnable listener) {
      this.onClose = listener;
    }

    void deliver(String text) {
      onMessage.accept(text);
    }

    void closed() {
      onClose.run();
    }
  }
}
