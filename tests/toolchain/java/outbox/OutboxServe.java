package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.BookingsTestHandler;
import gen.b.bookings.server.CancelEmit;
import gen.b.bookings.server.CancelFacts;
import gen.b.bookings.server.CancelRequest;
import gen.b.bookings.server.NotificationsTestHandler;
import gen.b.bookings.server.PayEmit;
import gen.b.bookings.server.PayFacts;
import gen.b.bookings.server.PayRequest;
import gen.b.bookings.server.RefundFacts;
import gen.b.bookings.server.RefundRequest;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.net.InetSocketAddress;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;

/**
 * The generated test handlers of {@code tests/events/outbox.veld.pending} (spec/events.md sections
 * 10.9 and 11.2), served over the JDK server at one address: {@code Bookings} (the moves, with
 * {@code Veld-Test-Emitted}) and {@code Notifications} (the event endpoint). Prints {@code PORT=<n>}
 * and serves until it is killed. The Rust test sends every example to it through the shared live
 * runner. The application's handlers must never run: every call example hands the test handler its
 * result or is refused by the guard. Port of {@code events_outbox_serve.ts}.
 */
public final class OutboxServe {
  private OutboxServe() {}

  private static AssertionError never() {
    return new AssertionError("the application handler must not be reached");
  }

  private static final BookingsService NEVER = new BookingsService() {
    @Override
    public Booking pay(PayRequest request, PayFacts facts, Transition<Status> transition, PayEmit emit, RequestContext ctx) {
      throw never();
    }

    @Override
    public Booking cancel(CancelRequest request, CancelFacts facts, Transition<Status> transition, CancelEmit emit, RequestContext ctx) {
      throw never();
    }

    @Override
    public Booking refund(RefundRequest request, RefundFacts facts, Transition<Status> transition, RequestContext ctx) {
      throw never();
    }
  };

  private static VeldResponse probe(VeldHandler bookings, VeldHandler notifications, VeldRequest request) {
    Map<String, Json> digests = new LinkedHashMap<>();
    Map<String, Json> subscribes = new LinkedHashMap<>();
    for (VeldHandler handler : new VeldHandler[] {bookings, notifications}) {
      Json.Obj body = Json.parse(handler.handle(request).orElseThrow().body()).asObject();
      digests.putAll(body.get("digests").asObject().fields());
      if (body.get("subscribes") instanceof Json.Obj s) {
        subscribes.putAll(s.fields());
      }
    }
    Json body = Json.object()
        .put("veldTestHandler", Json.of(1L))
        .put("digests", new Json.Obj(digests))
        .put("subscribes", new Json.Obj(subscribes))
        .build();
    return new VeldResponse(200, Map.of("content-type", "application/json"), body.write());
  }

  public static void main(String[] args) throws Exception {
    VeldHandler bookings = BookingsTestHandler.create(NEVER, new BookingsHooks(null));
    VeldHandler notifications = NotificationsTestHandler.create();
    VeldHandler combined = new VeldHandler() {
      @Override
      public Optional<VeldResponse> handle(VeldRequest request) {
        if (request.method().equalsIgnoreCase("GET") && request.path().equals("/__veld/test")) {
          return Optional.of(probe(bookings, notifications, request));
        }
        if (request.method().equalsIgnoreCase("POST") && request.path().equals("/__veld/test/event")) {
          return notifications.handle(request);
        }
        return bookings.handle(request);
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(combined, new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
