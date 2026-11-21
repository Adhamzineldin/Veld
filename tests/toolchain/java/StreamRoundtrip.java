package veldtest;

import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;
import java.util.List;

/**
 * The generated Java client against the generated Java server on the JDK adapter, in one process:
 * SSE (ordered items, close mid-stream seen by the server, typed errors before and after the first
 * event, resuming with the last event id) and auth.
 */
public final class StreamRoundtrip {
  private StreamRoundtrip() {}

  public static void main(String[] args) throws Exception {
    HttpServer server = gen.s.adapters.JdkHttpServerAdapter.listen(
        StreamImpl.handler(), new InetSocketAddress("127.0.0.1", 0));
    try {
      String base = "http://127.0.0.1:" + server.getAddress().getPort();
      StreamScenario.sse(base, new StreamScenario.Probe() {
        @Override
        public int foreverClosed() {
          return StreamImpl.FOREVER_CLOSED.get();
        }

        @Override
        public int chatEnded() {
          return 0;
        }

        @Override
        public List<String> sink() {
          return List.of();
        }
      });
      Check.that(StreamImpl.ROOM_EVENTS_CLOSED.get() >= 1, "the server closes finished streams too");
      StreamScenario.auth(base, false);
      Check.that(!StreamImpl.PRINCIPALS.isEmpty(), "the auth hook's principal reached the implementation");
      System.out.println("stream roundtrip ok");
    } finally {
      server.stop(0);
    }
  }
}
