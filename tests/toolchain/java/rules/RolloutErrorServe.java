package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.e.adapters.JdkHttpServerAdapter;
import gen.e.bk.bk.Confirmation;
import gen.e.bk.server.BookRequest;
import gen.e.bk.server.TablesHooks;
import gen.e.bk.server.TablesService;
import gen.e.bk.server.TablesTestHandler;
import gen.e.runtime.RequestContext;
import java.net.InetSocketAddress;

/**
 * The generated test handler of {@code tests/rules/endpoints/rollout_error.veld} on the JDK server,
 * with a stub application that is never reached. Prints {@code PORT=<n>} and serves until it is
 * killed.
 */
public final class RolloutErrorServe {
  private RolloutErrorServe() {}

  public static void main(String[] args) throws Exception {
    TablesService stub = new TablesService() {
      @Override
      public Confirmation book(BookRequest request, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        TablesTestHandler.create(stub, new TablesHooks()), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
