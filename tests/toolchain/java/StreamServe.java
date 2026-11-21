package veldtest;

import com.sun.net.httpserver.HttpServer;

/**
 * Serves the streaming contract's SSE endpoints on the generated JDK adapter, prints
 * {@code PORT=<n>}, and runs until stdin closes.
 */
public final class StreamServe {
  private StreamServe() {}

  public static void main(String[] args) throws Exception {
    HttpServer server = gen.s.adapters.JdkHttpServerAdapter.listen(
        StreamImpl.handler(), new java.net.InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    while (System.in.read() >= 0) {
      // keep serving
    }
    server.stop(0);
  }
}
