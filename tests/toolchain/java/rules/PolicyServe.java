package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.BookingsTestHandler;
import gen.b.bookings.server.GetBookingFacts;
import gen.b.bookings.server.GetBookingRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.RequestContext;
import java.net.InetSocketAddress;

/**
 * The generated test handler of {@code tests/rules/endpoints/policies.veld.pending} on the JDK
 * server, with a stub application that is never reached (every example is a dry run or brings its
 * result). The auth hook is never called by the test handler. Prints {@code PORT=<n>} and serves
 * until it is killed.
 */
public final class PolicyServe {
  private PolicyServe() {}

  public static void main(String[] args) throws Exception {
    BookingsService stub = new BookingsService() {
      @Override
      public Booking payBooking(PayBookingRequest request, PayBookingFacts facts, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Booking getBooking(GetBookingRequest request, GetBookingFacts facts, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsTestHandler.create(stub, new BookingsHooks((token, ctx) -> null)),
        new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
