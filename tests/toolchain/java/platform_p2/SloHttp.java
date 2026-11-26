package veldtest;

import gen.b.levels.levels.Room;
import gen.b.levels.server.BuildRequest;
import gen.b.levels.server.GetRoomRequest;
import gen.b.levels.server.ListRoomsRequest;
import gen.b.levels.server.RoomsHandler;
import gen.b.levels.server.RoomsHooks;
import gen.b.levels.server.RoomsService;
import gen.b.prelude.NotFound;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.Metric;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Service levels (spec/platform.md section 13), against {@code
 * tests/toolchain/contract/platform_p2/service_levels.veld}: the metric of a {@code 200}, of a declared
 * error and of a {@code 500}, the names and values of the attributes, every endpoint measured (one without an
 * objective of its own too), a route that matches nothing not, a failing hook ignored, and a server whose hooks have
 * no metrics hook. Prints {@code OK}.
 */
public final class SloHttp {
  private SloHttp() {}

  private static final UUID FOUND = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID MISSING = UUID.fromString("00000000-0000-4000-8000-000000000002");
  private static final UUID BROKEN = UUID.fromString("00000000-0000-4000-8000-000000000003");

  private static final class Impl implements RoomsService {
    @Override
    public Room getRoom(GetRoomRequest request, RequestContext ctx) {
      if (request.id().equals(MISSING)) {
        throw new NotFound(new NotFound.Data("room"));
      }
      if (request.id().equals(BROKEN)) {
        throw new IllegalStateException("the store is down");
      }
      return new Room(request.id(), "a");
    }

    @Override
    public List<Room> listRooms(ListRoomsRequest request, RequestContext ctx) {
      return List.of();
    }

    @Override
    public Room build(BuildRequest request, RequestContext ctx) {
      return new Room(FOUND, request.body().name());
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static VeldResponse call(VeldHandler handler, String method, String path, String body) {
    Map<String, List<String>> headers = body == null
        ? Map.of()
        : Map.of("content-type", List.of("application/json"), "idempotency-key", List.of("k" + path.hashCode() + body.hashCode()));
    return handler.handle(new VeldRequest(method, path, "", headers, body, null)).orElseThrow(() -> new AssertionError("no route " + path));
  }

  public static void main(String[] args) {
    List<Metric> seen = new CopyOnWriteArrayList<>();
    List<Throwable> errors = new CopyOnWriteArrayList<>();
    RoomsHooks hooks = new RoomsHooks(new MemoryIdempotencyStore()).onError((e, ctx) -> errors.add(e)).metrics(seen::add);
    VeldHandler handler = RoomsHandler.create(new Impl(), hooks);

    VeldResponse ok = call(handler, "GET", "/rooms/" + FOUND, null);
    check(ok.status() == 200 && seen.size() == 1, "a 200 is measured once: " + seen.size());
    Metric m = seen.get(0);
    check(m.name().equals("http.server.request.duration") && m.unit().equals("s"), "name and unit: " + m);
    check(m.value() >= 0 && m.value() < 5, "a duration in seconds: " + m.value());
    check(m.attributes().get("http.request.method").equals("GET"), "method: " + m);
    check(m.attributes().get("http.route").equals("/rooms/{id}"), "the full path template: " + m);
    check(m.attributes().get("http.response.status_code").equals(200), "the status as a number: " + m);
    check(!m.attributes().containsKey("error.type"), "no error.type below 500: " + m);
    check(m.attributes().get("veld.service").equals("levels/Rooms"), "service: " + m);
    check(m.attributes().get("veld.endpoint").equals("getRoom"), "endpoint: " + m);
    check(((String) m.attributes().get("veld.operation")).startsWith("veld:levels/Rooms.getRoom@"), "the whole operation signature: " + m);

    VeldResponse missing = call(handler, "GET", "/rooms/" + MISSING, null);
    check(missing.status() == 404 && seen.size() == 2, "a declared error is measured");
    check(seen.get(1).attributes().get("http.response.status_code").equals(404)
        && !seen.get(1).attributes().containsKey("error.type"), "404 is no error.type: " + seen.get(1));

    VeldResponse broken = call(handler, "GET", "/rooms/" + BROKEN, null);
    check(broken.status() == 500 && seen.size() == 3, "a 500 is measured");
    check(seen.get(2).attributes().get("error.type").equals("500"), "error.type of a 5xx: " + seen.get(2));

    call(handler, "GET", "/rooms", null);
    check(seen.size() == 4 && seen.get(3).attributes().get("veld.endpoint").equals("listRooms"),
        "an endpoint without an objective of its own is measured too: " + seen.size());

    check(handler.handle(new VeldRequest("GET", "/nothing", "", Map.of(), null, null)).isEmpty() && seen.size() == 4,
        "a route that matches nothing is not measured");

    // A hook that throws is ignored: the answer is the same.
    VeldHandler throwing = RoomsHandler.create(
        new Impl(), new RoomsHooks(new MemoryIdempotencyStore()).metrics(metric -> {
          throw new IllegalStateException("the exporter is down");
        }));
    check(call(throwing, "GET", "/rooms/" + FOUND, null).status() == 200, "a throwing hook is ignored");

    // A server without the hook works as it did.
    VeldHandler plain = RoomsHandler.create(new Impl(), new RoomsHooks(new MemoryIdempotencyStore()));
    check(call(plain, "GET", "/rooms/" + FOUND, null).status() == 200, "no hook, no measuring");
    check(errors.size() == 1, "the 500 went to onError once: " + errors);
    System.out.println("OK");
  }
}
