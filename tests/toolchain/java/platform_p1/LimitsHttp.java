package veldtest;

import gen.b.limits.limits.Room;
import gen.b.limits.limits.User;
import gen.b.limits.server.ArchiveRequest;
import gen.b.limits.server.ByTenantRequest;
import gen.b.limits.server.ByUserRequest;
import gen.b.limits.server.CreateRequest;
import gen.b.limits.server.DesksHandler;
import gen.b.limits.server.DesksHooks;
import gen.b.limits.server.DesksService;
import gen.b.limits.server.FreeRequest;
import gen.b.limits.server.ListingRequest;
import gen.b.limits.server.MineRequest;
import gen.b.limits.server.PublicHandler;
import gen.b.limits.server.PublicHooks;
import gen.b.limits.server.PublicService;
import gen.b.limits.server.RoomsHandler;
import gen.b.limits.server.RoomsHooks;
import gen.b.limits.server.RoomsService;
import gen.b.runtime.AuditEvent;
import gen.b.runtime.Json;
import gen.b.runtime.Limiter;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.MemoryLimiter;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Rate limits over the real entry point (spec/platform.md section 5), against {@code
 * tests/toolchain/contract/platform_p1/limits.veld}: the last allowed call, the first refused one with
 * its body and its {@code Retry-After} header, the next window, each of {@code user}, {@code tenant}
 * (also a caller without a tenant) and {@code ip}, a limit that a service gives its endpoints, the
 * audit of a refused call, a limiter and a hook that fail (the call is let through and {@code
 * onError} hears of it), a server created without the hook it needs, and a server without a
 * limiter. Prints {@code OK}.
 */
public final class LimitsHttp {
  private LimitsHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID CALLER = UUID.fromString("00000000-0000-4000-8000-0000000000c1");
  private static final UUID TENANT_A = UUID.fromString("00000000-0000-4000-8000-0000000000aa");
  private static final UUID TENANT_B = UUID.fromString("00000000-0000-4000-8000-0000000000bb");
  private static final Room ROOM = new Room(ID);

  private static final class Impl implements RoomsService {
    @Override
    public Room byUser(ByUserRequest request, RequestContext ctx) {
      return ROOM;
    }

    @Override
    public List<Room> byTenant(ByTenantRequest request, RequestContext ctx) {
      return List.of(ROOM);
    }

    @Override
    public void archive(ArchiveRequest request, RequestContext ctx) {}

    @Override
    public Room create(CreateRequest request, RequestContext ctx) {
      return ROOM;
    }

    @Override
    public List<Room> free(FreeRequest request, RequestContext ctx) {
      return List.of(ROOM);
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  /** The token is {@code <tenant uuid>} or {@code none}: the caller of the request (its id is its tenant). */
  private static User caller(String token) {
    return token.equals("none") ? new User(CALLER, null) : new User(UUID.fromString(token), UUID.fromString(token));
  }

  private static VeldResponse get(VeldHandler handler, String path, String token) {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    if (token != null) {
      headers.put("authorization", List.of("Bearer " + token));
    }
    return handler.handle(new VeldRequest("GET", path, null, headers, null, null)).orElseThrow();
  }

  private static VeldResponse post(VeldHandler handler, String path, String token, String body) {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    headers.put("authorization", List.of("Bearer " + token));
    return handler.handle(new VeldRequest("POST", path, null, headers, body, null)).orElseThrow();
  }

  private static RoomsHooks rooms(
      Limiter limiter, List<String> addresses, List<AuditEvent> events, List<Throwable> errors) {
    RoomsHooks hooks = new RoomsHooks(
        (token, ctx) -> caller(token),
        new MemoryIdempotencyStore(),
        (principal, ctx) -> "user-" + principal.id(),
        events::add,
        ctx -> addresses.get(0));
    hooks.onError((e, ctx) -> errors.add(e));
    return limiter == null ? hooks : hooks.limiter(limiter);
  }

  public static void main(String[] args) {
    AtomicLong now = new AtomicLong(1_000_000L);
    List<String> address = new ArrayList<>(List.of("203.0.113.9"));
    List<AuditEvent> events = new CopyOnWriteArrayList<>();
    List<Throwable> errors = new CopyOnWriteArrayList<>();
    String tenantA = TENANT_A.toString();

    // By caller: the last allowed call, the first refused one, its body and header, the next window.
    VeldHandler handler = RoomsHandler.create(new Impl(), rooms(new MemoryLimiter(now::get), address, events, errors));
    String byUser = "/rooms/" + ID;
    check(get(handler, byUser, tenantA).status() == 200, "the first call is allowed");
    now.addAndGet(1000);
    check(get(handler, byUser, tenantA).status() == 200, "the second call is the last allowed one");
    now.addAndGet(1000);
    VeldResponse refused = get(handler, byUser, tenantA);
    check(refused.status() == 429, "the third call is refused: " + refused.status() + " " + refused.body());
    Json.Obj problem = Json.parse(refused.body()).asObject();
    check("TooManyRequests".equals(problem.get("title").asString()), "the prelude's TooManyRequests: " + refused.body());
    long retry = (long) problem.get("data").asObject().get("retryAfter").asDouble();
    check(retry == 58, "the time left in the window, rounded up: " + retry);
    check(Long.toString(retry).equals(refused.headers().get("retry-after")), "the Retry-After header: " + refused.headers());
    check(refused.headers().get("content-type").startsWith("application/problem+json"), "a problem document");
    // The audit of a refused call: the caller, no parameters, no facts, no result.
    AuditEvent seen = events.get(events.size() - 1);
    check(seen.outcome().status() == 429 && "TooManyRequests".equals(seen.outcome().error()), "the audit of the 429: " + seen.outcome());
    check(seen.principal() != null && seen.parameters().fields().isEmpty() && seen.facts().fields().isEmpty()
        && seen.result() == null, "the limit runs before binding: " + seen.toJson().write());
    // Another caller has a count of their own, and a refused call does not count.
    check(get(handler, byUser, TENANT_B.toString()).status() == 200, "another caller, another key");
    now.addAndGet(58_000);
    check(get(handler, byUser, tenantA).status() == 200, "the next window");

    // By tenant: a count per tenant of the caller, and one for the callers that have none.
    List<String> keys = new ArrayList<>();
    Limiter recording = (key, limit, window) -> {
      keys.add(key + "#" + limit + "/" + window);
      return Limiter.Decision.allow();
    };
    VeldHandler recorded = RoomsHandler.create(new Impl(), rooms(recording, address, events, errors));
    check(get(recorded, "/rooms/tenant", tenantA).status() == 200, "by tenant");
    check(keys.get(0).endsWith("|tenant|" + tenantA + "#3/3600"), "the tenant text of the principal: " + keys);
    keys.clear();
    check(get(recorded, "/rooms/tenant", "none").status() == 200, "a caller without a tenant");
    check(keys.get(0).endsWith("|tenant|#3/3600"), "counted with the others that have none: " + keys);
    keys.clear();
    check(get(recorded, byUser, tenantA).status() == 200, "by user");
    check(keys.get(0).endsWith("|user|user-" + TENANT_A + "#2/60"), "the identity is what the caller hook says: " + keys);
    keys.clear();
    // The real limiter counts tenants apart.
    VeldHandler counting = RoomsHandler.create(new Impl(), rooms(new MemoryLimiter(now::get), address, events, errors));
    for (int i = 0; i < 3; i++) {
      check(get(counting, "/rooms/tenant", tenantA).status() == 200, "three calls an hour for a tenant");
    }
    check(get(counting, "/rooms/tenant", tenantA).status() == 429, "the fourth is refused");
    check(get(counting, "/rooms/tenant", TENANT_B.toString()).status() == 200, "another tenant is another count");

    // By address, a post that is not idempotent, and one that is: the same limit, one a second.
    VeldHandler byIp = RoomsHandler.create(new Impl(), rooms(new MemoryLimiter(now::get), address, events, errors));
    String archive = "/rooms/" + ID + "/archive";
    check(post(byIp, archive, tenantA, null).status() == 204, "the first archive");
    VeldResponse archived = post(byIp, archive, tenantA, null);
    check(archived.status() == 429 && "1".equals(archived.headers().get("retry-after")), "the second in a second: " + archived.status());
    address.set(0, "203.0.113.10");
    check(post(byIp, archive, tenantA, null).status() == 204, "another address, another count");
    now.addAndGet(1000);
    address.set(0, "203.0.113.9");
    check(post(byIp, archive, tenantA, null).status() == 204, "the next second");

    // No limit: counted by nobody.
    for (int i = 0; i < 5; i++) {
      check(get(byIp, "/rooms/free", tenantA).status() == 200, "an endpoint without a limit");
    }

    // A limit that the service gives its endpoints, with no auth.
    PublicService open = new PublicService() {
      @Override
      public List<Room> listing(ListingRequest request, RequestContext ctx) {
        return List.of(ROOM);
      }
    };
    VeldHandler publicHandler =
        PublicHandler.create(open, new PublicHooks(ctx -> "198.51.100.1").limiter(new MemoryLimiter(now::get)));
    check(get(publicHandler, "/public", null).status() == 200 && get(publicHandler, "/public", null).status() == 200, "two a second");
    check(get(publicHandler, "/public", null).status() == 429, "the service's limit refuses the third");

    // By caller in a service with no idempotent endpoint: the hook is a hook of its own.
    DesksService desks = new DesksService() {
      @Override
      public List<Room> mine(MineRequest request, RequestContext ctx) {
        return List.of(ROOM);
      }
    };
    VeldHandler deskHandler = DesksHandler.create(
        desks, new DesksHooks((token, ctx) -> caller(token), (principal, ctx) -> "desk-" + principal.id()).limiter(new MemoryLimiter(now::get)));
    check(get(deskHandler, "/desks", tenantA).status() == 200, "one a minute");
    check(get(deskHandler, "/desks", tenantA).status() == 429, "the second is refused");

    // A limiter that fails lets the call through, and onError hears of it.
    errors.clear();
    Limiter down = (key, limit, window) -> {
      throw new IllegalStateException("the limiter is down");
    };
    VeldHandler failing = RoomsHandler.create(new Impl(), rooms(down, address, events, errors));
    check(get(failing, byUser, tenantA).status() == 200, "a limiter that is down lets the call through");
    check(errors.size() == 1 && errors.get(0) instanceof IllegalStateException, "and onError hears: " + errors);
    errors.clear();
    // So does a hook that fails: the caller-identity hook and the address hook.
    RoomsHooks brokenCaller = new RoomsHooks(
            (token, ctx) -> caller(token),
            new MemoryIdempotencyStore(),
            (principal, ctx) -> {
              throw new IllegalStateException("no identity");
            },
            events::add,
            ctx -> "203.0.113.9")
        .onError((e, ctx) -> errors.add(e));
    check(get(RoomsHandler.create(new Impl(), brokenCaller), byUser, tenantA).status() == 200, "a failing caller hook is a failing limiter");
    check(errors.size() == 1, "and it is reported: " + errors);
    errors.clear();
    RoomsHooks brokenAddress = new RoomsHooks(
            (token, ctx) -> caller(token),
            new MemoryIdempotencyStore(),
            (principal, ctx) -> "x",
            events::add,
            ctx -> {
              throw new IllegalStateException("no address");
            })
        .onError((e, ctx) -> errors.add(e));
    check(post(RoomsHandler.create(new Impl(), brokenAddress), archive, tenantA, null).status() == 204, "a failing address hook is a failing limiter");
    check(errors.size() == 1, "and it is reported: " + errors);

    // A server created without the hook it needs does not start.
    try {
      RoomsHandler.create(new Impl(), new RoomsHooks((token, ctx) -> caller(token), new MemoryIdempotencyStore(), null, events::add, ctx -> "a"));
      throw new AssertionError("a server without the caller hook started");
    } catch (IllegalStateException e) {
      check(e.getMessage().contains("hooks.caller is required"), "names the hook: " + e.getMessage());
    }
    try {
      RoomsHandler.create(
          new Impl(), new RoomsHooks((token, ctx) -> caller(token), new MemoryIdempotencyStore(), (p, ctx) -> "x", events::add, null));
      throw new AssertionError("a server without the address hook started");
    } catch (IllegalStateException e) {
      check(e.getMessage().contains("hooks.address is required"), "names the hook: " + e.getMessage());
    }

    // A server without a limiter counts in memory.
    VeldHandler memory = RoomsHandler.create(new Impl(), rooms(null, address, events, errors));
    check(get(memory, byUser, tenantA).status() == 200 && get(memory, byUser, tenantA).status() == 200, "two calls");
    check(get(memory, byUser, tenantA).status() == 429, "the memory limiter refuses the third");
    System.out.println("OK");
  }
}
