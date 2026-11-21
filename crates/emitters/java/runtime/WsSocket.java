package dev.veld.runtime;

import java.util.function.Consumer;

/**
 * The transport of one accepted WebSocket, as a framework adapter provides it to
 * {@link VeldHandler#ws}. Implementations must make {@link #send} safe to call from any thread.
 */
public interface WsSocket {
  /** Sends one text frame. */
  void send(String text);

  /** Closes the connection with a close code and reason. */
  void close(int code, String reason);

  /** Registers the listener for incoming text frames (called on the transport's thread). */
  void onMessage(Consumer<String> listener);

  /** Registers the listener for the end of the connection, from either side. */
  void onClose(Runnable listener);
}
