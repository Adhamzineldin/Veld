package veldtest;

import gen.b.runtime.Json;
import gen.b.runtime.Page;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.SseEvent;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.tenancy.server.CopyBookingFacts;
import gen.b.tenancy.server.CopyBookingRequest;
import gen.b.tenancy.server.CreateBookingRequest;
import gen.b.tenancy.server.GetBookingFacts;
import gen.b.tenancy.server.GetBookingRequest;
import gen.b.tenancy.server.ListBookingsRequest;
import gen.b.tenancy.server.LiveBookingsRequest;
import gen.b.tenancy.server.MeRequest;
import gen.b.tenancy.server.PageBookingsRequest;
import gen.b.tenancy.server.SeenFacts;
import gen.b.tenancy.server.SeenRequest;
import gen.b.tenancy.server.TenantsFactLoaders;
import gen.b.tenancy.server.TenantsHandler;
import gen.b.tenancy.server.TenantsHooks;
import gen.b.tenancy.server.TenantsService;
import gen.b.tenancy.tenancy.Booking;
import gen.b.tenancy.tenancy.Summary;
import gen.b.tenancy.tenancy.User;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * Tenancy over the real entry point (spec/platform.md section 3), against {@code
 * tests/toolchain/contract/platform_p1/tenancy.veld}: a fact of another tenant is {@code 404} and
 * never {@code 403}, an optional one is absent, a value of another tenant in a result, a list, a
 * page, a declared error and a stream item is never sent ({@code 500}, one event for each value
 * with its rule, path and field), a caller without a tenant owns nothing, and a request may lack
 * the tenant member or carry it. Prints {@code OK}.
 */
public final class TenancyHttp {
  private TenancyHttp() {}

  private static final UUID MINE = UUID.fromString("00000000-0000-4000-8000-0000000000aa");
  private static final UUID THEIRS = UUID.fromString("00000000-0000-4000-8000-0000000000bb");
  private static final UUID CALLER = UUID.fromString("00000000-0000-4000-8000-0000000000c1");
  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");

  /** The tenant of the booking the loaders find, and whether it is open. */
  private static volatile UUID loadedTenant = MINE;
  private static volatile boolean open = false;
  /** The tenant the body of the last {@code createBooking} carried. */
  private static volatile UUID carried;

  private static Booking made(UUID tenant, String title) {
    return new Booking(ID, tenant, title, open);
  }

  private static final class Impl implements TenantsService {
    @Override
    public Booking getBooking(GetBookingRequest request, GetBookingFacts facts, RequestContext ctx) {
      return facts.booking();
    }

    @Override
    public Summary seen(SeenRequest request, SeenFacts facts, RequestContext ctx) {
      return new Summary(facts.held().title());
    }

    @Override
    public List<Booking> listBookings(ListBookingsRequest request, RequestContext ctx) {
      return List.of(made(MINE, "a"), made(THEIRS, "b"));
    }

    @Override
    public Page<Booking> pageBookings(PageBookingsRequest request, RequestContext ctx) {
      return new Page<>(List.of(made(MINE, "a"), made(THEIRS, "b")), 2L, 1, 20);
    }

    @Override
    public User me(MeRequest request, RequestContext ctx) {
      return (User) ctx.principal();
    }

    @Override
    public Booking copyBooking(CopyBookingRequest request, CopyBookingFacts facts, RequestContext ctx) {
      return facts.booking();
    }

    @Override
    public Booking createBooking(CreateBookingRequest request, RequestContext ctx) {
      carried = request.body().tenantId();
      // The application sets the tenant from the caller.
      return new Booking(request.body().id(), ((User) ctx.principal()).tenantId(), request.body().title(), true);
    }

    @Override
    public Stream<SseEvent<Booking>> liveBookings(LiveBookingsRequest request, RequestContext ctx) {
      return Stream.of(SseEvent.of(made(MINE, "first")), SseEvent.of(made(THEIRS, "second")));
    }
  }

  private static final TenantsFactLoaders LOADERS = new TenantsFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.of(made(loadedTenant, "loaded"));
    }

    @Override
    public Optional<Booking> held(UUID key, RequestContext ctx) {
      return Optional.of(made(loadedTenant, "held"));
    }
  };

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  /** The caller of a request: the token is the tenant, or {@code none} for a caller with no tenant. */
  private static User caller(String token) {
    return new User(CALLER, token.equals("none") ? null : UUID.fromString(token));
  }

  private static VeldHandler handler(List<RuleWalk.Event> events, List<Throwable> errors) {
    TenantsHooks hooks = new TenantsHooks((token, ctx) -> caller(token))
        .onRuleViolation(events::add)
        .onError((e, ctx) -> errors.add(e));
    return TenantsHandler.create(new Impl(), LOADERS, hooks);
  }

  private static VeldResponse send(VeldHandler handler, String method, String path, String token, String body) {
    Map<String, List<String>> headers = new java.util.LinkedHashMap<>();
    headers.put("authorization", List.of("Bearer " + token));
    return handler
        .handle(new VeldRequest(method, path, null, headers, body, null))
        .orElseThrow(() -> new AssertionError("no route " + method + " " + path));
  }

  private static Json.Obj body(VeldResponse response) {
    return Json.parse(response.body()).asObject();
  }

  private static void internal(VeldResponse response, String what) {
    check(response.status() == 500, what + ": 500, got " + response.status() + " " + response.body());
    Json.Obj problem = body(response);
    check("Internal".equals(problem.get("title").asString()), what + ": the title is Internal");
    check(problem.get("data") instanceof Json.Obj data && data.fields().isEmpty(), what + ": no detail");
  }

  public static void main(String[] args) {
    // `unvalidated`: the contract was generated with `validate = false`, so the `require` of an
    // endpoint does not run; what tenancy does is the same.
    boolean rules = args.length == 0;
    List<RuleWalk.Event> events = new CopyOnWriteArrayList<>();
    List<Throwable> errors = new ArrayList<>();
    VeldHandler handler = handler(events, errors);
    String mine = MINE.toString();
    String none = "none";

    // A fact of the caller's tenant is loaded; another tenant's is not found, never forbidden.
    loadedTenant = MINE;
    VeldResponse found = send(handler, "GET", "/tenants/" + ID, mine, null);
    check(found.status() == 200, "a fact of the caller's tenant is found: " + found.body());
    loadedTenant = THEIRS;
    VeldResponse missing = send(handler, "GET", "/tenants/" + ID, mine, null);
    check(missing.status() == 404, "another tenant's fact is 404, got " + missing.status());
    check("booking".equals(body(missing).get("data").asObject().get("resource").asString()), "NotFound names the fact");
    check(events.isEmpty(), "a fact that is not found is not a violation");

    // An optional fact of another tenant is absent: the `require` has its say.
    if (rules) {
      VeldResponse held = send(handler, "GET", "/tenants/" + ID + "/seen", mine, null);
      check(held.status() == 404, "an optional fact of another tenant is absent: " + held.status());
      check("held".equals(body(held).get("data").asObject().get("resource").asString()), "the require answers NotFound");
    }
    loadedTenant = MINE;
    check(send(handler, "GET", "/tenants/" + ID + "/seen", mine, null).status() == 200, "an optional fact of the tenant");

    // A caller whose uuid is written in capitals is the same tenant.
    check(send(handler, "GET", "/tenants/" + ID, mine.toUpperCase(), null).status() == 200, "a uuid in any case");

    // A value of another tenant in a list or a page: 500, nothing sent, one event for the value.
    VeldResponse list = send(handler, "GET", "/tenants", mine, null);
    internal(list, "a list with a value of another tenant");
    check(!list.body().contains("\"b\""), "nothing of the other tenant is sent");
    check(events.size() == 1, "one event for the one value: " + events);
    RuleWalk.Event event = events.get(0);
    check(event.position() == RuleWalk.Position.OUTPUT && event.outcome() == RuleWalk.Outcome.REJECTED, "output, rejected");
    check(event.violation().path().equals("result[1]"), "the path of the value: " + event.violation().path());
    check(event.violation().fields().equals(List.of("tenantId")), "the wire name of the field");
    check(event.rule().startsWith("veld:tenancy/Booking.tenant@"), "the tenant id of the model: " + event.rule());
    check(event.violation().message().equals("violates rule tenant"), "the message");
    events.clear();
    VeldResponse page = send(handler, "GET", "/tenants/page", mine, null);
    internal(page, "a page with an item of another tenant");
    check(events.size() == 1 && events.get(0).violation().path().equals("result.items[1]"), "the path in a page: " + events);
    events.clear();

    // The principal is a tenant-scoped value too; the caller itself may be returned.
    VeldResponse me = send(handler, "GET", "/tenants/me", mine, null);
    check(me.status() == 200, "the caller may be returned: " + me.body());

    // A declared error that holds a value of another tenant is never sent.
    open = false;
    if (rules) {
    String own = "{\"copy\":{\"id\":\"" + ID + "\",\"tenantId\":\"" + mine + "\",\"title\":\"c\",\"open\":false}}";
    String foreign = "{\"copy\":{\"id\":\"" + ID + "\",\"tenantId\":\"" + THEIRS + "\",\"title\":\"c\",\"open\":false}}";
    VeldResponse dup = send(handler, "POST", "/tenants/" + ID + "/copy", mine, own);
    check(dup.status() == 409, "a declared error of the caller's own tenant is sent: " + dup.status() + " " + dup.body());
    check(events.isEmpty(), "no event for it");
    VeldResponse refused = send(handler, "POST", "/tenants/" + ID + "/copy", mine, foreign);
    internal(refused, "a declared error that holds another tenant's value");
    check(events.size() == 1 && events.get(0).violation().path().equals("error.copy"), "the path in an error: " + events);
    events.clear();
    }

    // A caller without a tenant owns nothing.
    loadedTenant = MINE;
    check(send(handler, "GET", "/tenants/" + ID, none, null).status() == 404, "a caller without a tenant finds no fact");
    internal(send(handler, "GET", "/tenants/me", none, null), "a caller without a tenant is a tenant-scoped value of nobody");
    events.clear();

    // A request may lack the tenant member, or carry it: the application sets the real one.
    String lacking = "{\"id\":\"" + ID + "\",\"title\":\"t\",\"open\":false}";
    VeldResponse created = send(handler, "POST", "/tenants", mine, lacking);
    check(created.status() == 200, "a request without the tenant member is accepted: " + created.status() + " " + created.body());
    check(mine.equals(body(created).get("tenantId").asString()), "the answer has the caller's tenant");
    String carrying = "{\"id\":\"" + ID + "\",\"tenantId\":\"" + THEIRS + "\",\"title\":\"t\",\"open\":false}";
    VeldResponse kept = send(handler, "POST", "/tenants", mine, carrying);
    check(kept.status() == 200, "a request that carries the member is accepted: " + kept.status() + " " + kept.body());
    check(THEIRS.equals(carried), "the member is kept as received: " + carried);
    check(mine.equals(body(kept).get("tenantId").asString()), "and the application still sets the tenant");

    // A stream item of another tenant is not sent: the stream ends as on any server failure.
    VeldResponse live = send(handler, "GET", "/tenants/live", mine, null);
    check(live.status() == 200 && live.frames() != null, "the stream is opened");
    List<String> frames = live.frames().collect(Collectors.toList());
    check(frames.get(0).contains("first"), "the item of the caller's tenant is sent: " + frames);
    check(frames.stream().noneMatch(f -> f.contains("second")), "the item of another tenant is not: " + frames);
    check(frames.size() == 2 && frames.get(1).contains("event: error"), "the stream ends with an error event: " + frames);
    check(events.size() == 1 && events.get(0).violation().path().equals("item"), "the path of an item: " + events);

    check(errors.isEmpty(), "nothing went to onError: " + errors);
    System.out.println("OK");
  }
}
