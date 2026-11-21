package dev.veld.runtime;

import java.util.stream.Stream;

/**
 * What a {@code ws} endpoint implementation gets: the validated messages of the client and a way
 * to answer. {@code Void} marks a side that has no messages.
 *
 * @param <In> the message type the client sends
 * @param <Out> the message type the server sends
 */
public interface WsSession<In, Out> {

  /**
   * The messages of the client, validated and decoded. Reading blocks until the next message and
   * the stream ends when the socket closes.
   */
  Stream<In> incoming();

  /**
   * Sends one message to the client.
   *
   * @throws IllegalStateException when the socket is closed or the endpoint sends no messages
   */
  void send(Out message);

  /** Closes the socket normally. Returning from the implementation does the same. */
  void close();

  /** Whether the socket is closed. */
  boolean isClosed();

  /** A typed view of the session the router passes to the generated code. */
  static <I, O> WsSession<I, O> adapt(WsSession<Json, Json> raw, Codec<I> in, Codec<O> out) {
    return new WsSession<>() {
      @Override
      public Stream<I> incoming() {
        return in == null ? Stream.empty() : raw.incoming().map(in::decode);
      }

      @Override
      public void send(O message) {
        if (out == null) {
          throw new IllegalStateException("this endpoint sends no messages");
        }
        raw.send(out.encode(message));
      }

      @Override
      public void close() {
        raw.close();
      }

      @Override
      public boolean isClosed() {
        return raw.isClosed();
      }
    };
  }
}
