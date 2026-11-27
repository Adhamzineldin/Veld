package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.runtime.Json;
import gen.b.runtime.Page;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.visibility.server.BookingsHooks;
import gen.b.visibility.server.BookingsService;
import gen.b.visibility.server.BookingsTestHandler;
import gen.b.visibility.server.CancelBookingFacts;
import gen.b.visibility.server.CancelBookingRequest;
import gen.b.visibility.server.CheckedBookingRequest;
import gen.b.visibility.server.GetBookingRequest;
import gen.b.visibility.server.ListBookingsRequest;
import gen.b.visibility.server.NoticesHooks;
import gen.b.visibility.server.NoticesService;
import gen.b.visibility.server.NoticesTestHandler;
import gen.b.visibility.server.PageBookingsRequest;
import gen.b.visibility.server.PublicBookingRequest;
import gen.b.visibility.visibility.Booking;
import java.net.InetSocketAddress;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * The generated test handlers of {@code tests/rules/endpoints/visibility.veld.pending} on the JDK
 * server: the caller comes from {@code Veld-Test-Principal}, the clock from {@code Veld-Test-Now},
 * the result from {@code Veld-Test-Result}, and the application is never reached. The two services
 * share one port. Prints {@code PORT=<n>} and serves until it is killed.
 */
public final class VisibilityServe {
  private VisibilityServe() {}

  private static UnsupportedOperationException never() {
    return new UnsupportedOperationException("the application's handler is not part of the examples");
  }

  /** The two test handlers behind one entry point: the first that has the route answers. */
  static VeldHandler handler() {
    BookingsService bookings = new BookingsService() {
      @Override
      public Booking getBooking(GetBookingRequest request, RequestContext ctx) {
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
      public Booking checkedBooking(CheckedBookingRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking cancelBooking(CancelBookingRequest request, CancelBookingFacts facts, RequestContext ctx) {
        throw never();
      }
    };
    NoticesService notices = new NoticesService() {
      @Override
      public Booking publicBooking(PublicBookingRequest request, RequestContext ctx) {
        throw never();
      }
    };
    VeldHandler first = BookingsTestHandler.create(bookings, new BookingsHooks((token, ctx) -> null));
    VeldHandler second = NoticesTestHandler.create(notices, new NoticesHooks());
    return request -> {
      if (request.method().equalsIgnoreCase("GET") && request.path().equals("/__veld/test")) {
        return Optional.of(probe(first, second, request));
      }
      Optional<VeldResponse> answer = first.handle(request);
      return answer.isPresent() ? answer : second.handle(request);
    };
  }

  /** The probe of both services: the digests of the two probes in one object. */
  private static VeldResponse probe(VeldHandler first, VeldHandler second, VeldRequest request) {
    Map<String, Json> digests = new LinkedHashMap<>();
    for (VeldHandler one : List.of(first, second)) {
      Json.Obj answer = Json.parse(one.handle(request).orElseThrow().body()).asObject();
      digests.putAll(answer.get("digests").asObject().fields());
    }
    Json body = Json.object()
        .put("veldTestHandler", Json.of(1L))
        .put("digests", new Json.Obj(digests))
        .build();
    return new VeldResponse(200, Map.of("content-type", "application/json"), body.write());
  }

  public static void main(String[] args) throws Exception {
    HttpServer server = JdkHttpServerAdapter.listen(handler(), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
