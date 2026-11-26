package veldtest;

import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.vers.server.GetRoomRequest;
import gen.b.vers.server.OrdersHooks;
import gen.b.vers.server.OrdersService;
import gen.b.vers.server.OrdersTestHandler;
import gen.b.vers.server.PayOrderFacts;
import gen.b.vers.server.PayOrderRequest;
import gen.b.vers.server.RenameFacts;
import gen.b.vers.server.RenameRequest;
import gen.b.vers.server.RoomsHooks;
import gen.b.vers.server.RoomsService;
import gen.b.vers.server.RoomsTestHandler;
import gen.b.vers.server.TickFacts;
import gen.b.vers.server.TickRequest;
import gen.b.vers.server.TouchFacts;
import gen.b.vers.server.TouchRequest;
import gen.b.vers.vers.Counter;
import gen.b.vers.vers.Order;
import gen.b.vers.vers.Room;
import gen.b.vers.vers.Status;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;

/**
 * The generated test handlers of {@code tests/rules/endpoints/version.veld.pending} over stub
 * applications that are never reached (every example is a dry run or brings its result). The two
 * services share one handler: a request goes to the first handler that has a route for it, and the
 * probe names both services. Used by the JDK server ({@link VersionServe}) and by Spring.
 */
public final class VersionStubs {
  private VersionStubs() {}

  private static UnsupportedOperationException never() {
    return new UnsupportedOperationException("the application's handler is not part of the examples");
  }

  /** Both test handlers behind one handler: the probe lists the services of both. */
  static VeldHandler both(VeldHandler first, VeldHandler second) {
    return new VeldHandler() {
      @Override
      public Optional<VeldResponse> handle(VeldRequest request) {
        if (request.method().equalsIgnoreCase("GET") && request.path().equals("/__veld/test")) {
          return Optional.of(probe(first, second, request));
        }
        Optional<VeldResponse> answer = first.handle(request);
        return answer.isPresent() ? answer : second.handle(request);
      }
    };
  }

  private static VeldResponse probe(VeldHandler first, VeldHandler second, VeldRequest request) {
    Map<String, Json> digests = new LinkedHashMap<>();
    for (VeldHandler h : new VeldHandler[] {first, second}) {
      VeldResponse part = h.handle(request).orElseThrow();
      digests.putAll(Json.parse(part.body()).asObject().get("digests").asObject().fields());
    }
    Json body = Json.object()
        .put("veldTestHandler", Json.of(1L))
        .put("digests", new Json.Obj(digests))
        .build();
    return new VeldResponse(200, Map.of("content-type", "application/json"), body.write());
  }

  /** The test handlers of both services behind one handler, over stub applications. */
  static VeldHandler handler() {
    RoomsService rooms = new RoomsService() {
      @Override
      public Room rename(RenameRequest request, RenameFacts facts, Long version, RequestContext ctx) {
        throw never();
      }

      @Override
      public Room touch(TouchRequest request, TouchFacts facts, Long version, RequestContext ctx) {
        throw never();
      }

      @Override
      public Counter tick(TickRequest request, TickFacts facts, Integer version, RequestContext ctx) {
        throw never();
      }

      @Override
      public Room getRoom(GetRoomRequest request, RequestContext ctx) {
        throw never();
      }
    };
    OrdersService orders = new OrdersService() {
      @Override
      public Order payOrder(
          PayOrderRequest request, PayOrderFacts facts, Long version, Transition<Status> transition, RequestContext ctx) {
        throw never();
      }
    };
    return both(
        RoomsTestHandler.create(rooms, new RoomsHooks()),
        OrdersTestHandler.create(orders, new OrdersHooks((token, ctx) -> null)));
  }
}
