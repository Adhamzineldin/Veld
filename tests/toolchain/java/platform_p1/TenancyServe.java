package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.runtime.Page;
import gen.b.runtime.RequestContext;
import gen.b.tenancy.server.BookingsHooks;
import gen.b.tenancy.server.BookingsService;
import gen.b.tenancy.server.BookingsTestHandler;
import gen.b.tenancy.server.CopyBookingFacts;
import gen.b.tenancy.server.CopyBookingRequest;
import gen.b.tenancy.server.GetBookingFacts;
import gen.b.tenancy.server.GetBookingRequest;
import gen.b.tenancy.server.ListBookingsRequest;
import gen.b.tenancy.server.MeRequest;
import gen.b.tenancy.server.PageBookingsRequest;
import gen.b.tenancy.server.SeenFacts;
import gen.b.tenancy.server.SeenRequest;
import gen.b.tenancy.tenancy.Booking;
import gen.b.tenancy.tenancy.Summary;
import gen.b.tenancy.tenancy.User;
import java.net.InetSocketAddress;
import java.util.List;

/**
 * The generated test handler of {@code tests/rules/endpoints/tenancy.veld.pending} on the JDK
 * server: the facts come from {@code Veld-Test-Facts}, the caller from {@code Veld-Test-Principal},
 * the result from {@code Veld-Test-Result}, and the application is never reached. Prints {@code
 * PORT=<n>} and serves until it is killed.
 */
public final class TenancyServe {
  private TenancyServe() {}

  private static UnsupportedOperationException never() {
    return new UnsupportedOperationException("the application's handler is not part of the examples");
  }

  public static void main(String[] args) throws Exception {
    BookingsService stub = new BookingsService() {
      @Override
      public Booking getBooking(GetBookingRequest request, GetBookingFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public Summary seen(SeenRequest request, SeenFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public List<Booking> listBookings(ListBookingsRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Page<Booking> pageBookings(PageBookingsRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public User me(MeRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking copyBooking(CopyBookingRequest request, CopyBookingFacts facts, RequestContext ctx) {
        throw never();
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsTestHandler.create(stub, new BookingsHooks(null)), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
