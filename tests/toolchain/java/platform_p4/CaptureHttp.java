package veldtest;

import gen.b.bookings.capture.Booking;
import gen.b.bookings.capture.Card;
import gen.b.bookings.capture.Guest;
import gen.b.bookings.capture.NewStay;
import gen.b.bookings.capture.User;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CreateRequest;
import gen.b.bookings.server.OpenStayRequest;
import gen.b.bookings.server.PayFacts;
import gen.b.bookings.server.PayRequest;
import gen.b.bookings.server.TotalFacts;
import gen.b.bookings.server.TotalRequest;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * The capture of {@code onRuleViolation} (spec/rules.md section 17.3) over the production handler of
 * tests/toolchain/contract/platform_p4/capture.veld, generated with {@code capture-violations =
 * true}: an input violation with the principal and {@code @pii} values at two depths, a failing
 * {@code require} with two facts, one that is found and one that is not, secrets in the body, the
 * fact and the principal, an output violation ({@code ensure}), an endpoint without auth, the clock
 * to the millisecond, the option turned off, a hook that throws and a handler without a hook. Every
 * event that carries a capture is written to {@code captures.json} for the Rust test, which gives
 * each to {@code veld_testing::from_violation}. Prints {@code OK}.
 */
public final class CaptureHttp {
  private CaptureHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID COUPON = UUID.fromString("00000000-0000-4000-8000-0000000000cc");
  private static final User USER =
      new User(UUID.fromString("00000000-0000-4000-8000-0000000000aa"), "me@example.test", "hunter2", List.of());
  private static final String NOW = "2026-06-01T00:00:00Z";

  private static final List<RuleWalk.Event> EVENTS = new CopyOnWriteArrayList<>();
  private static final List<Throwable> FAILURES = new CopyOnWriteArrayList<>();
  private static final List<Json> WRITTEN = new ArrayList<>();

  private static volatile Booking booking = booking(new BigDecimal("10.00"));
  private static volatile Optional<Booking> discount = Optional.of(booking(new BigDecimal("10.00")));
  private static volatile BigDecimal overTotal = new BigDecimal("-1.00");

  private static Booking booking(BigDecimal total) {
    return new Booking(ID, new Guest("Ada", "+20100000000"), total, new Card("4111", "Ada"));
  }

  private static final class Impl implements BookingsService {
    @Override
    public NewStay create(CreateRequest request, RequestContext ctx) {
      return request.body();
    }

    @Override
    public Booking pay(PayRequest request, PayFacts facts, RequestContext ctx) {
      return booking;
    }

    @Override
    public Booking total(TotalRequest request, TotalFacts facts, RequestContext ctx) {
      return booking(overTotal);
    }

    @Override
    public NewStay openStay(OpenStayRequest request, RequestContext ctx) {
      return request.body();
    }
  }

  private static final BookingsFactLoaders LOADERS = new BookingsFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.of(booking);
    }

    @Override
    public Optional<Booking> discount(UUID key, RequestContext ctx) {
      return discount;
    }
  };

  /** A handler with the options the case asks for. */
  private static VeldHandler bookings(Clock clock, boolean capture, boolean hook) {
    BookingsHooks hooks = new BookingsHooks((token, ctx) -> token.equals("me") ? USER : null)
        .clock(clock)
        .captureViolations(capture)
        .onError((e, ctx) -> FAILURES.add(e));
    if (hook) {
      hooks.onRuleViolation(EVENTS::add);
    }
    return BookingsHandler.create(new Impl(), LOADERS, hooks);
  }

  private static Clock clockAt(String instant) {
    return Clock.fixed(Instant.parse(instant), ZoneOffset.UTC);
  }

  private static VeldResponse send(VeldHandler handler, String method, String path, String query, String body, boolean auth) {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    if (auth) {
      headers.put("authorization", List.of("Bearer me"));
    }
    return handler
        .handle(new VeldRequest(method, path, query, headers, body, null))
        .orElseThrow(() -> new AssertionError("no route " + method + " " + path));
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static RuleWalk.Event only() {
    check(EVENTS.size() == 1, "one event, got " + EVENTS.size());
    return EVENTS.get(0);
  }

  private static String stay() {
    return "{\"guests\":2,\"start\":\"2026-06-12T00:00:00Z\",\"end\":\"2026-06-10T00:00:00Z\","
        + "\"guest\":{\"name\":\"Ada\",\"phone\":\"+20100000000\"},"
        + "\"companions\":[{\"name\":\"Bo\"},{\"name\":\"Cy\",\"phone\":\"+20100000001\"}]}";
  }

  private static String payment(String amount) {
    return "{\"amount\":\"" + amount + "\",\"card\":{\"number\":\"4242\",\"holder\":\"Ada\"}}";
  }

  /** What {@code from_violation} reads: the event, by the case it is for. */
  private static void keep(String name, String kind, RuleWalk.Event event) {
    WRITTEN.add(Json.object()
        .put("name", Json.of(name))
        .put("kind", Json.of(kind))
        .put("event", event.toJson())
        .build());
  }

  private static void same(Json.Obj got, String expected, String what) {
    String want = Json.parse(expected).canonical();
    check(got.canonical().equals(want), what + "\n  got      " + got.canonical() + "\n  expected " + want);
  }

  public static void main(String[] args) throws Exception {
    VeldHandler handler = bookings(clockAt(NOW), true, true);
    String id = ID.toString();

    // An input violation: the body with @pii values at two depths, and the principal.
    VeldResponse res = send(handler, "POST", "/bookings", null, stay(), true);
    check(res.status() == 422, "the stay is refused: " + res.status());
    RuleWalk.Event event = only();
    check(event.position() == RuleWalk.Position.INPUT && event.outcome() == RuleWalk.Outcome.REJECTED, "input, rejected");
    check(event.capture() != null, "the event has a capture");
    String sig = event.operation();
    check(sig.startsWith("veld:bookings/Bookings.create@"), "the operation: " + sig);
    same(event.capture(),
        "{\"service\":\"Bookings\",\"endpoint\":\"create\",\"operation\":\"" + sig + "\","
            + "\"params\":{\"body\":{\"guests\":2,\"start\":\"2026-06-12T00:00:00Z\",\"end\":\"2026-06-10T00:00:00Z\","
            + "\"guest\":{\"name\":\"Ada\"},\"companions\":[{\"name\":\"Bo\"},{\"name\":\"Cy\"}]}},"
            + "\"facts\":{},\"principal\":{\"id\":\"" + USER.id() + "\",\"roles\":[]},\"now\":\"" + NOW + "\","
            + "\"redacted\":[\"params.body.companions[1].phone\",\"params.body.guest.phone\",\"principal.email\",\"principal.password\"]}",
        "the capture of an input violation");
    check(event.violation().message().equals("The stay must end after it starts"), "the message: " + event.violation().message());
    keep("an input violation with the principal and pii at two depths", "input", event);
    check(Json.parse(res.body()).asObject().get("violations").asArray().items().size() == 1,
        "the caller is sent what the contract says, whatever the capture leaves out");

    // A require that fails with two facts, the principal and secrets in the body, the fact and the principal.
    EVENTS.clear();
    res = send(handler, "POST", "/bookings/" + id + "/pay", "coupon=" + COUPON, payment("99.00"), true);
    check(res.status() == 422, "the payment is refused: " + res.status());
    event = only();
    check(event.violation().message().equals("The amount is more than the booking costs"), "the message");
    String found = "{\"id\":\"" + id + "\",\"guest\":{\"name\":\"Ada\"},\"total\":\"10.00\",\"card\":{\"holder\":\"Ada\"}}";
    same(event.capture(),
        "{\"service\":\"Bookings\",\"endpoint\":\"pay\",\"operation\":\"" + event.operation() + "\","
            + "\"params\":{\"id\":\"" + id + "\",\"body\":{\"amount\":\"99.00\",\"card\":{\"holder\":\"Ada\"}},\"coupon\":\"" + COUPON + "\"},"
            + "\"facts\":{\"booking\":" + found + ",\"discount\":" + found + "},"
            + "\"principal\":{\"id\":\"" + USER.id() + "\",\"roles\":[]},\"now\":\"" + NOW + "\","
            + "\"redacted\":[\"facts.booking.card.number\",\"facts.booking.guest.phone\",\"facts.discount.card.number\","
            + "\"facts.discount.guest.phone\",\"params.body.card.number\",\"principal.email\",\"principal.password\"]}",
        "the capture of a require with facts and secrets");
    keep("a require with facts, an optional fact and secrets", "input", event);

    // The optional fact is not asked for (no coupon): null, and the parameter is absent.
    EVENTS.clear();
    send(handler, "POST", "/bookings/" + id + "/pay", null, payment("99.00"), true);
    event = only();
    check(event.capture().get("facts").asObject().get("discount") instanceof Json.Null,
        "an optional fact that was not asked for is null: " + event.capture());
    check(event.capture().get("params").asObject().get("coupon") == null, "a parameter that was not sent is absent");
    keep("an optional fact that is not loaded", "input", event);

    // The optional fact is asked for and the loader does not find it: null.
    EVENTS.clear();
    discount = Optional.empty();
    send(handler, "POST", "/bookings/" + id + "/pay", "coupon=" + COUPON, payment("99.00"), true);
    event = only();
    check(event.capture().get("facts").asObject().get("discount") instanceof Json.Null, "an optional fact the loader did not find is null");
    check(event.capture().get("params").asObject().get("coupon").asString().equals(COUPON.toString()), "the coupon is a parameter");
    keep("an optional fact the loader did not find", "input", event);
    discount = Optional.of(booking(new BigDecimal("10.00")));

    // An output violation (ensure): the capture holds what the request had.
    EVENTS.clear();
    res = send(handler, "GET", "/bookings/" + id + "/total", null, null, true);
    check(res.status() == 500, "an output violation is a 500: " + res.status());
    event = only();
    check(event.position() == RuleWalk.Position.OUTPUT, "output");
    check(event.violation().message().equals("The total is never negative"), "the message");
    check(event.capture().get("params").canonical().equals("{\"id\":\"" + id + "\"}"), "the parameters: " + event.capture());
    check(event.capture().get("facts").asObject().fields().keySet().equals(java.util.Set.of("booking")), "one fact");
    check(event.capture().get("redacted").canonical().equals(
            "[\"facts.booking.card.number\",\"facts.booking.guest.phone\",\"principal.email\",\"principal.password\"]"),
        "redacted: " + event.capture().get("redacted").canonical());
    keep("an output violation", "output", event);

    // No auth, no principal, no fact: principal is null.
    EVENTS.clear();
    send(handler, "POST", "/bookings/open", null, stay(), false);
    event = only();
    check(event.capture().get("principal") instanceof Json.Null, "no principal: " + event.capture());
    check(event.capture().get("facts").asObject().fields().isEmpty(), "no facts");
    check(event.capture().get("redacted").canonical().equals("[\"params.body.companions[1].phone\",\"params.body.guest.phone\"]"),
        "redacted: " + event.capture().get("redacted").canonical());
    keep("no auth and no principal", "input", event);
    check(event.capture().get("now").asString().equals(NOW), "the clock is the one reading of the request");

    // The clock to the millisecond.
    EVENTS.clear();
    send(bookings(clockAt("2026-06-01T10:20:30.045Z"), true, true), "POST", "/bookings", null, stay(), true);
    check(only().capture().get("now").asString().equals("2026-06-01T10:20:30.045Z"), "the millisecond is kept");

    // The option is off: the event is what it was before, a capture of null; and a handler without a
    // hook builds nothing.
    EVENTS.clear();
    send(bookings(clockAt(NOW), false, true), "POST", "/bookings", null, stay(), true);
    event = only();
    check(event.capture() == null, "the option is off: no capture");
    check(!event.toJson().fields().containsKey("capture"), "the JSON has no capture member either");
    EVENTS.clear();
    res = send(bookings(clockAt(NOW), true, false), "POST", "/bookings", null, stay(), true);
    check(res.status() == 422 && EVENTS.isEmpty(), "no hook, no event");

    // A hook that throws never changes the answer.
    FAILURES.clear();
    BookingsHooks throwing = new BookingsHooks((token, ctx) -> USER)
        .clock(clockAt(NOW))
        .onError((e, ctx) -> FAILURES.add(e))
        .onRuleViolation(e -> {
          throw new IllegalStateException("the hook failed");
        });
    res = send(BookingsHandler.create(new Impl(), LOADERS, throwing), "POST", "/bookings", null, stay(), true);
    check(res.status() == 422, "a hook that throws does not change the answer");
    check(FAILURES.isEmpty(), "a failing hook of the application is ignored, not reported: " + FAILURES);

    // What from_violation reads.
    Files.writeString(Path.of("captures.json"), Json.array(WRITTEN).write(), StandardCharsets.UTF_8);
    System.out.println("OK");
  }
}
