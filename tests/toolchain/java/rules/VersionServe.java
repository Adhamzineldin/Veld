package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import java.net.InetSocketAddress;

/**
 * The generated test handlers of {@code tests/rules/endpoints/version.veld.pending} on the JDK
 * server ({@link VersionStubs}). Prints {@code PORT=<n>} and serves until it is killed.
 */
public final class VersionServe {
  private VersionServe() {}

  public static void main(String[] args) throws Exception {
    HttpServer server = JdkHttpServerAdapter.listen(VersionStubs.handler(), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
