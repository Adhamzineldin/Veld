package veldtest;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.WebSocket;
import java.util.Map;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/** A bare WebSocket for looking at the frames and close codes exactly as they are on the wire. */
final class RawWs implements WebSocket.Listener {
  private final BlockingQueue<Object> events = new LinkedBlockingQueue<>();
  private final StringBuilder partial = new StringBuilder();
  private WebSocket socket;

  static RawWs open(String url, Map<String, String> headers) {
    RawWs raw = new RawWs();
    WebSocket.Builder builder = HttpClient.newHttpClient().newWebSocketBuilder();
    headers.forEach(builder::header);
    raw.socket = builder.buildAsync(URI.create(url), raw).join();
    return raw;
  }

  void send(String text) {
    socket.sendText(text, true).join();
  }

  /** The next text frame. */
  String frame() {
    Object next = next();
    if (next instanceof String s) {
      return s;
    }
    throw new AssertionError("expected a text frame but the socket closed with " + next);
  }

  /** The close code of the connection (frames that are still queued must have been read). */
  int closeCode() {
    Object next = next();
    if (next instanceof Integer code) {
      return code;
    }
    throw new AssertionError("expected a close but got the frame " + next);
  }

  void close() {
    socket.sendClose(WebSocket.NORMAL_CLOSURE, "done");
  }

  private Object next() {
    try {
      Object next = events.poll(15, TimeUnit.SECONDS);
      if (next == null) {
        throw new AssertionError("timed out waiting for a WebSocket event");
      }
      return next;
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new AssertionError(e);
    }
  }

  @Override
  public void onOpen(WebSocket webSocket) {
    webSocket.request(1);
  }

  @Override
  public CompletionStage<?> onText(WebSocket webSocket, CharSequence data, boolean last) {
    partial.append(data);
    if (last) {
      events.add(partial.toString());
      partial.setLength(0);
    }
    webSocket.request(1);
    return null;
  }

  @Override
  public CompletionStage<?> onClose(WebSocket webSocket, int statusCode, String reason) {
    events.add(statusCode);
    return null;
  }

  @Override
  public void onError(WebSocket webSocket, Throwable error) {
    events.add(-1);
  }
}
