package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.BookingsTestHandler;
import gen.b.bookings.server.CancelBookingFacts;
import gen.b.bookings.server.CancelBookingRequest;
import gen.b.bookings.server.CheckInFacts;
import gen.b.bookings.server.CheckInRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import java.net.InetSocketAddress;

/**
 * The generated test handler of {@code tests/rules/endpoints/transitions.veld} on the JDK
 * server, with a stub application that is never reached (every example is a dry run or brings its
 * result). Prints {@code PORT=<n>} and serves until it is killed.
 */
public final class MachineServe {
  private MachineServe() {}

  public static void main(String[] args) throws Exception {
    BookingsService stub = new BookingsService() {
      @Override
      public Booking payBooking(
          PayBookingRequest request, PayBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Booking cancelBooking(
          CancelBookingRequest request, CancelBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public void checkIn(
          CheckInRequest request, CheckInFacts facts, Transition<Status> transition, RequestContext ctx) {
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
