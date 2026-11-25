package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Coupon;
import gen.b.bookings.bookings.Payment;
import gen.b.bookings.bookings.Room;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CreateBookingFacts;
import gen.b.bookings.server.CreateBookingRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import java.math.BigDecimal;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The production handler of {@code tests/rules/endpoints/bookings.veld} over HTTP on the JDK server:
 * facts, {@code require} with and without {@code else}, the order of the checks, {@code ensure} in
 * {@code strict} and {@code audit}, {@code old}, the clock and the rules digest header.
 */
public final class EndpointHttp {
  private EndpointHttp() {}

  private static final UUID ROOM = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID BOOKING = UUID.fromString("00000000-0000-4000-8000-0000000000b1");
  private static final Instant NOW = Instant.parse("2026-06-01T00:00:00Z");
  private static final AtomicInteger ROOM_CALLS = new AtomicInteger();
  private static final AtomicInteger COUPON_CALLS = new AtomicInteger();
  private static final AtomicInteger HANDLER_CALLS = new AtomicInteger();

  private static Room room = new Room(ROOM, 4, false, new BigDecimal("10"));
  private static Coupon coupon = new Coupon("HALF", 50);
  private static Status answerStatus = Status.PENDING;
  private static BigDecimal answerTotal = new BigDecimal("120.00");
  private static BigDecimal storedTotal = new BigDecimal("120.00");
  private static boolean roomMissing;

  private static final class Loaders implements BookingsFactLoaders {
    @Override
    public Optional<Room> room(UUID key, RequestContext ctx) {
      ROOM_CALLS.incrementAndGet();
      return roomMissing ? Optional.empty() : Optional.of(room);
    }

    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.of(EndpointHttp.booking(storedTotal));
    }

    @Override
    public Optional<Coupon> coupon(String key, RequestContext ctx) {
      COUPON_CALLS.incrementAndGet();
      return key.equals("GONE") ? Optional.empty() : Optional.of(coupon);
    }
  }

  private static Booking booking(BigDecimal total) {
    return new Booking(
        ROOM, 1, NOW.plusSeconds(60), NOW.plusSeconds(3600), null, BOOKING, Status.PENDING, total);
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking createBooking(CreateBookingRequest request, CreateBookingFacts facts, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      var b = request.body();
      return new Booking(b.roomId(), b.guests(), b.start(), b.end(), b.couponCode(), BOOKING, answerStatus, BigDecimal.ZERO);
    }

    @Override
    public Booking payBooking(PayBookingRequest request, PayBookingFacts facts, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      return new Booking(ROOM, 1, NOW.plusSeconds(60), NOW.plusSeconds(3600), null, BOOKING, Status.PAID, answerTotal);
    }
  }

  private record Answer(int status, HttpResponse<String> raw, Json.Obj body) {
    String header(String name) {
      return raw.headers().firstValue(name).orElse(null);
    }
  }

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static Answer post(HttpServer server, String path, String body) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path))
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(body))
        .build();
    HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
    Json.Obj json = response.body().isEmpty() ? null : Json.parse(response.body()).asObject();
    return new Answer(response.statusCode(), response, json);
  }

  private static String booking(int guests, String start, String end, String coupon) {
    String code = coupon == null ? "" : ",\"couponCode\":\"" + coupon + "\"";
    return "{\"roomId\":\"" + ROOM + "\",\"guests\":" + guests + ",\"start\":\"" + start + "\",\"end\":\"" + end + "\"" + code + "}";
  }

  private static final String LATER = "2026-07-01T00:00:00Z";
  private static final String LATEST = "2026-07-02T00:00:00Z";

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static HttpServer serve(RuleWalk.OutputMode mode, List<RuleWalk.Event> events) throws Exception {
    BookingsHooks hooks = new BookingsHooks()
        .clock(Clock.fixed(NOW, ZoneOffset.UTC))
        .rulesOutput(mode)
        .onRuleViolation(events::add);
    return JdkHttpServerAdapter.listen(
        BookingsHandler.create(new Impl(), new Loaders(), hooks), new InetSocketAddress("127.0.0.1", 0));
  }

  private static String type(Json.Obj problem) {
    return problem.get("title").asString();
  }

  private static Json.Obj violation(Answer a) {
    var violations = a.body().get("violations").asArray().items();
    check(violations.size() == 1, "exactly one violation: " + violations.size());
    return violations.get(0).asObject();
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    List<RuleWalk.Event> audited = Collections.synchronizedList(new ArrayList<>());
    HttpServer strict = serve(RuleWalk.OutputMode.STRICT, events);
    HttpServer audit = serve(RuleWalk.OutputMode.AUDIT, audited);
    try {
      missingFact(strict);
      requireOrder(strict, events);
      requireValidation(strict);
      optionalFact(strict);
      ensureModes(strict, events, audit, audited);
      oldValues(strict);
    } finally {
      strict.stop(0);
      audit.stop(0);
    }
    System.out.println("OK");
  }

  /** A required fact the loader does not find is {@code 404 NotFound} naming the fact. */
  private static void missingFact(HttpServer server) throws Exception {
    roomMissing = true;
    Answer a = post(server, "/bookings", booking(2, LATER, LATEST, null));
    roomMissing = false;
    check(a.status() == 404, "404 for a missing room: " + a.status());
    check(type(a.body()).equals("NotFound"), "NotFound: " + a.body().write());
    check(a.body().get("data").asObject().get("resource").asString().equals("room"), "resource is the fact's name");
    check(a.body().get("violations") == null, "a missing fact carries no violations");
    check("true".equals(String.valueOf(a.header("Veld-Rules") != null)), "the digest is on a 404");
    check(HANDLER_CALLS.get() == 0, "the handler did not run");
  }

  /** The first failing {@code require} wins; step 5 runs before any loader. */
  private static void requireOrder(HttpServer server, List<RuleWalk.Event> events) throws Exception {
    int loads = ROOM_CALLS.get();
    Answer invalid = post(server, "/bookings", booking(2, LATEST, LATER, null));
    check(invalid.status() == 422 && type(invalid.body()).equals("ValidationError"), "a bad body is 422");
    check(ROOM_CALLS.get() == loads, "no fact is loaded for an invalid body");
    check(invalid.body().get("violations").asArray().items().size() == 1, "the invariant violation");

    room = new Room(ROOM, 1, true, new BigDecimal("10"));
    Answer closed = post(server, "/bookings", booking(5, LATER, LATEST, null));
    check(closed.status() == 409 && type(closed.body()).equals("RoomClosed"), "closed wins over full: " + closed.body().write());
    Json.Obj v = violation(closed);
    check(v.get("path").asString().equals("request"), "the path of an endpoint rule is request");
    check(v.get("rule").asString().startsWith("veld:bookings/Bookings.createBooking.require@"), "rule id " + v.get("rule"));
    check(v.get("fields").asArray().items().isEmpty(), "no parameter is read by `not room.closed`");

    room = new Room(ROOM, 2, false, new BigDecimal("10"));
    Answer full = post(server, "/bookings", booking(3, LATER, LATEST, null));
    check(full.status() == 409 && type(full.body()).equals("RoomFull"), "RoomFull");
    check(full.body().get("data").asObject().get("capacity").asDouble() == 2, "the payload is evaluated: capacity 2");
    check(violation(full).get("fields").write().equals("[\"body\"]"), "the parameters the rule reads");
    room = new Room(ROOM, 4, false, new BigDecimal("10"));
    check(HANDLER_CALLS.get() == 0, "the handler did not run");
    check(!events.isEmpty() && events.get(events.size() - 1).position() == RuleWalk.Position.INPUT, "the hook saw the failed require");
  }

  /** {@code require} without {@code else} is {@code 422 ValidationError} with one issue; the clock is the option. */
  private static void requireValidation(HttpServer server) throws Exception {
    Answer past = post(server, "/bookings", booking(2, "2026-05-01T00:00:00Z", "2026-05-02T00:00:00Z", null));
    check(past.status() == 422 && type(past.body()).equals("ValidationError"), "start before the clock: 422");
    var issues = past.body().get("data").asObject().get("issues").asArray().items();
    check(issues.size() == 1, "one issue");
    check(issues.get(0).asObject().get("code").asString().equals("rule"), "code rule");
    check(issues.get(0).asObject().get("path").asString().equals("request"), "path request");
    check(issues.get(0).asObject().get("message").asString().equals("A booking cannot start in the past"), "message");
    Json.Obj v = violation(past);
    check(v.get("rule").asString().startsWith("veld:bookings/Bookings.createBooking.notInPast@"), "rule id");
    Answer exactly = post(server, "/bookings", booking(2, "2026-06-01T00:00:00Z", "2026-06-02T00:00:00Z", null));
    check(exactly.status() == 201, "a booking may start exactly now: " + exactly.status());
  }

  /** An optional fact whose key is absent is not loaded; a loader that finds nothing is not an error. */
  private static void optionalFact(HttpServer server) throws Exception {
    int calls = COUPON_CALLS.get();
    check(post(server, "/bookings", booking(2, LATER, LATEST, null)).status() == 201, "no coupon named");
    check(COUPON_CALLS.get() == calls, "the coupon loader was not called");
    check(post(server, "/bookings", booking(2, LATER, LATEST, "GONE")).status() == 201, "a coupon that is not found is absent");
    check(COUPON_CALLS.get() == calls + 1, "the coupon loader was called once");
    coupon = new Coupon("BIG", 60);
    Answer bad = post(server, "/bookings", booking(2, LATER, LATEST, "BIG"));
    check(bad.status() == 422, "the coupon rule refuses 60 percent");
    coupon = new Coupon("HALF", 50);
  }

  /** {@code ensure}: {@code strict} answers 500 with no detail, {@code audit} sends and reports. */
  private static void ensureModes(
      HttpServer strict, List<RuleWalk.Event> events, HttpServer audit, List<RuleWalk.Event> audited) throws Exception {
    answerStatus = Status.PAID;
    int before = events.size();
    Answer a = post(strict, "/bookings", booking(2, LATER, LATEST, null));
    check(a.status() == 500 && type(a.body()).equals("Internal"), "strict: 500 Internal");
    check(a.body().get("violations") == null, "no rule detail for the caller");
    check(events.size() == before + 1, "one event");
    RuleWalk.Event e = events.get(before);
    check(e.position() == RuleWalk.Position.OUTPUT && e.outcome() == RuleWalk.Outcome.REJECTED, "output, rejected");
    check(e.rule().startsWith("veld:bookings/Bookings.createBooking.startsPending@"), "rule " + e.rule());
    Answer sent = post(audit, "/bookings", booking(2, LATER, LATEST, null));
    check(sent.status() == 201, "audit sends the response: " + sent.status());
    check(audited.size() == 1 && audited.get(0).outcome() == RuleWalk.Outcome.AUDITED, "audit reports it");
    answerStatus = Status.PENDING;
    check(post(strict, "/bookings", booking(2, LATER, LATEST, null)).status() == 201, "a result that keeps its promises passes");
  }

  /** {@code old(...)} is the value before the handler ran. */
  private static void oldValues(HttpServer server) throws Exception {
    String pay = "{\"amount\":\"120.00\"}";
    String path = "/bookings/" + BOOKING + "/pay";
    answerTotal = new BigDecimal("120.00");
    Answer ok = post(server, path, pay);
    check(ok.status() == 201, "paying the total passes: " + ok.status() + " " + (ok.body() == null ? "" : ok.body().write()));
    Answer wrong = post(server, path, "{\"amount\":\"100.00\"}");
    check(wrong.status() == 422 && type(wrong.body()).equals("AmountMismatch"), "the declared error");
    check(wrong.body().get("data").asObject().get("expected").asString().equals("120.00"), "its payload: " + wrong.body().write());
    answerTotal = new BigDecimal("99.00");
    Answer changed = post(server, path, pay);
    check(changed.status() == 500, "the handler changed the total: 500, " + changed.status());
    answerTotal = new BigDecimal("120.00");
  }
}
