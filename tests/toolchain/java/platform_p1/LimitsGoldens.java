package veldtest;

import gen.b.bookings.bookings.Room;
import gen.b.bookings.bookings.User;
import gen.b.bookings.server.ArchiveRequest;
import gen.b.bookings.server.ByIpRequest;
import gen.b.bookings.server.ByTenantRequest;
import gen.b.bookings.server.ByUserRequest;
import gen.b.bookings.server.HotRequest;
import gen.b.bookings.server.ListingRequest;
import gen.b.bookings.server.PublicHandler;
import gen.b.bookings.server.PublicHooks;
import gen.b.bookings.server.PublicService;
import gen.b.bookings.server.RoomsHandler;
import gen.b.bookings.server.RoomsHooks;
import gen.b.bookings.server.RoomsService;
import gen.b.runtime.Json;
import gen.b.runtime.Limiter;
import gen.b.runtime.MemoryLimiter;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Every case of {@code tests/wire/limits/cases.json} through the generated runtime (spec/platform.md
 * section 5, tests/wire/limits/README.md): {@code windows} through {@link MemoryLimiter} with a clock
 * the test controls, {@code keys} through the production handler of each service, whose limiter
 * records the key it is asked about. Expects the contract of {@code tests/wire/limits/contract.veld}
 * generated as the package {@code bookings} and the cases next to the working directory as {@code
 * cases.json}. Prints {@code OK}.
 */
public final class LimitsGoldens {
  private LimitsGoldens() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final Room ROOM = new Room(ID);

  /** The case the handlers are answering. */
  private static volatile Json.Obj current;

  private static final List<String> ASKED = new ArrayList<>();

  private static String text(String member) {
    Json value = current.get(member);
    return value == null ? "" : value.asString();
  }

  public static void main(String[] args) throws Exception {
    Json.Obj file = Json.parse(Files.readString(Path.of("cases.json"))).asObject();
    windows(file.get("windows").asArray().items());
    keys(file.get("keys").asArray().items());
    System.out.println("limit goldens: " + file.get("windows").asArray().items().size() + " windows, "
        + file.get("keys").asArray().items().size() + " keys");
    System.out.println("OK");
  }

  private static void windows(List<Json> cases) {
    for (Json item : cases) {
      Json.Obj c = item.asObject();
      String name = c.get("name").asString();
      AtomicLong now = new AtomicLong();
      MemoryLimiter limiter = new MemoryLimiter(now::get);
      long limit = (long) c.get("limit").asDouble();
      long window = (long) c.get("windowSeconds").asDouble();
      List<Json> calls = c.get("calls").asArray().items();
      List<Json> decisions = c.get("decisions").asArray().items();
      for (int i = 0; i < calls.size(); i++) {
        Json.Obj call = calls.get(i).asObject();
        now.set((long) call.get("t").asDouble());
        Limiter.Decision got = limiter.take(call.get("key").asString(), limit, window);
        Json.Obj want = decisions.get(i).asObject();
        boolean allowed = want.get("allowed").asBoolean();
        long retry = (long) want.get("retryAfterSeconds").asDouble();
        if (got.allowed() != allowed || got.retryAfterSeconds() != retry) {
          throw new AssertionError(name + ": call " + i + " at " + call.get("t").asDouble() + " ms: got " + got
              + ", the reference says allowed=" + allowed + " retryAfterSeconds=" + retry);
        }
      }
    }
  }

  private static void keys(List<Json> cases) {
    Limiter recording = (key, limit, windowSeconds) -> {
      ASKED.add(key);
      return Limiter.Decision.allow();
    };
    RoomsService rooms = new RoomsService() {
      @Override
      public Room byUser(ByUserRequest request, RequestContext ctx) {
        return ROOM;
      }

      @Override
      public List<Room> byTenant(ByTenantRequest request, RequestContext ctx) {
        return List.of(ROOM);
      }

      @Override
      public List<Room> byIp(ByIpRequest request, RequestContext ctx) {
        return List.of(ROOM);
      }

      @Override
      public void archive(ArchiveRequest request, RequestContext ctx) {}
    };
    PublicService open = new PublicService() {
      @Override
      public List<Room> listing(ListingRequest request, RequestContext ctx) {
        return List.of(ROOM);
      }

      @Override
      public List<Room> hot(HotRequest request, RequestContext ctx) {
        return List.of(ROOM);
      }
    };
    VeldHandler roomsHandler = RoomsHandler.create(
        rooms,
        new RoomsHooks(
                (token, ctx) -> current.get("principal") instanceof Json.Obj who
                    ? User.fromWire(who)
                    : new User(ID, null),
                (principal, ctx) -> text("caller"),
                ctx -> text("address"))
            .limiter(recording));
    VeldHandler publicHandler = PublicHandler.create(open, new PublicHooks(ctx -> text("address")).limiter(recording));
    Map<String, Object[]> routes = Map.of(
        "byUser", new Object[] {roomsHandler, "GET", "/rooms/" + ID},
        "byTenant", new Object[] {roomsHandler, "GET", "/rooms/tenant"},
        "byIp", new Object[] {roomsHandler, "GET", "/rooms/ip"},
        "archive", new Object[] {roomsHandler, "POST", "/rooms/" + ID + "/archive"},
        "listing", new Object[] {publicHandler, "GET", "/public"},
        "hot", new Object[] {publicHandler, "GET", "/public/hot"});
    for (Json item : cases) {
      Json.Obj c = item.asObject();
      String endpoint = c.get("endpoint").asString();
      Object[] route = routes.get(endpoint);
      if (route == null) {
        throw new AssertionError("the contract has no endpoint " + endpoint);
      }
      current = c;
      ASKED.clear();
      VeldResponse response = ((VeldHandler) route[0])
          .handle(new VeldRequest((String) route[1], (String) route[2], null, Map.of("authorization", List.of("Bearer t")), null, null))
          .orElseThrow();
      if (response.status() >= 300) {
        throw new AssertionError(endpoint + ": the call is allowed, got " + response.status() + " " + response.body());
      }
      if (!ASKED.equals(List.of(c.get("key").asString()))) {
        throw new AssertionError(endpoint + ": the key\n  got      " + ASKED + "\n  expected " + c.get("key").asString());
      }
    }
  }
}
