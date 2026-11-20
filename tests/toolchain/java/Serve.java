package veldtest;

import com.sun.net.httpserver.HttpServer;

/** Starts the generated Java server, prints {@code PORT=<n>}, and runs until stdin closes. */
public final class Serve {
  private Serve() {}

  public static void main(String[] args) throws Exception {
    HttpServer server = new ServerImpl().listen();
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    while (System.in.read() >= 0) {
      // keep serving
    }
    server.stop(0);
  }
}
