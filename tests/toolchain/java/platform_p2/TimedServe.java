package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.BookingStatus;
import gen.b.bookings.server.ArchiveBookingFacts;
import gen.b.bookings.server.ArchiveBookingRequest;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.BookingsTestHandler;
import gen.b.bookings.server.ExpireBookingFacts;
import gen.b.bookings.server.ExpireBookingRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import java.net.InetSocketAddress;

/**
 * The generated test handler of {@code tests/rules/endpoints/timed.veld.pending} on the JDK server,
 * with a stub application that is never reached (every example is a dry run or brings its result).
 * Prints {@code PORT=<n>} and serves until it is killed.
 */
public final class TimedServe {
  private TimedServe() {}

  public static void main(String[] args) throws Exception {
    BookingsService stub = new BookingsService() {
      @Override
      public Booking expireBooking(
          ExpireBookingRequest request, ExpireBookingFacts facts, Transition<BookingStatus> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Booking archiveBooking(
          ArchiveBookingRequest request, ArchiveBookingFacts facts, Transition<BookingStatus> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Booking payBooking(
          PayBookingRequest request, PayBookingFacts facts, Transition<BookingStatus> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsTestHandler.create(stub, new BookingsHooks()), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
