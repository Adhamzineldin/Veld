package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.p.adapters.JdkHttpServerAdapter;
import gen.p.bk.bk.Booking;
import gen.p.bk.bk.Receipt;
import gen.p.bk.server.BookingRequest;
import gen.p.bk.server.EnforcedFacts;
import gen.p.bk.server.EnforcedRequest;
import gen.p.bk.server.MixedRequest;
import gen.p.bk.server.PartiesHooks;
import gen.p.bk.server.PartiesService;
import gen.p.bk.server.PartiesTestHandler;
import gen.p.bk.server.ReceiptRequest;
import gen.p.bk.server.WatchedFacts;
import gen.p.bk.server.WatchedRequest;
import gen.p.runtime.RequestContext;
import java.net.InetSocketAddress;

/**
 * The generated test handler of {@code tests/rules/endpoints/rollout.veld} on the JDK server, with a
 * stub application that is never reached (every example is a dry run or brings its result). Prints
 * {@code PORT=<n>} and serves until it is killed.
 */
public final class RolloutServe {
  private RolloutServe() {}

  private static UnsupportedOperationException never() {
    return new UnsupportedOperationException("the application's handler is not part of the examples");
  }

  public static void main(String[] args) throws Exception {
    PartiesService stub = new PartiesService() {
      @Override
      public Booking watched(WatchedRequest request, WatchedFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public Receipt enforced(EnforcedRequest request, EnforcedFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking mixed(MixedRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking booking(BookingRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Receipt receipt(ReceiptRequest request, RequestContext ctx) {
        throw never();
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        PartiesTestHandler.create(stub, new PartiesHooks()), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
