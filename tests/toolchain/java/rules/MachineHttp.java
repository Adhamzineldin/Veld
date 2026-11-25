package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CancelBookingFacts;
import gen.b.bookings.server.CancelBookingRequest;
import gen.b.bookings.server.CheckInFacts;
import gen.b.bookings.server.CheckInRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.Transition;
import java.math.BigDecimal;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The production handler of {@code tests/rules/endpoints/transitions.veld.pending} over HTTP on the
 * JDK server: the guard (409 with {@code field}, {@code from} and {@code to}), the transition the
 * handler receives, the order of the checks and the target check in {@code strict} and {@code audit}.
 */
public final class MachineHttp {
  private MachineHttp() {}

  private static final UUID BOOKING = UUID.fromString("00000000-0000-4000-8000-0000000000b1");
  private static final AtomicInteger HANDLER_CALLS = new AtomicInteger();

  private static Status stored = Status.PENDING;
  private static boolean missing;
  private static Status answered = Status.PAID;
  private static Transition<Status> received;

  private static Booking booking(Status status) {
    return new Booking(BOOKING, status, new BigDecimal("50.00"));
  }

  private static final class Loaders implements BookingsFactLoaders {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return missing ? Optional.empty() : Optional.of(MachineHttp.booking(stored));
    }
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking payBooking(
        PayBookingRequest request, PayBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      received = transition;
      return booking(answered);
    }

    @Override
    public Booking cancelBooking(
        CancelBookingRequest request, CancelBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      received = transition;
      return booking(Status.CANCELLED);
    }

    @Override
    public void checkIn(
        CheckInRequest request, CheckInFacts facts, Transition<Status> transition, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      received = transition;
    }
  }

  private record Answer(int status, Json.Obj body) {}

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static Answer post(HttpServer server, String path, String body) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path))
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(body))
        .build();
    HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
    Json.Obj json = response.body().isEmpty() ? null : Json.parse(response.body()).asObject();
    return new Answer(response.statusCode(), json);
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static HttpServer serve(RuleWalk.OutputMode mode, List<RuleWalk.Event> events) throws Exception {
    BookingsHooks hooks = new BookingsHooks().rulesOutput(mode).onRuleViolation(events::add);
    return JdkHttpServerAdapter.listen(
        BookingsHandler.create(new Impl(), new Loaders(), hooks), new InetSocketAddress("127.0.0.1", 0));
  }

  private static String pay(String amount) {
    return "{\"amount\":\"" + amount + "\"}";
  }

  private static String path(String action) {
    return "/bookings/" + BOOKING + "/" + action;
  }

  private static String title(Answer a) {
    return a.body().get("title").asString();
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    List<RuleWalk.Event> audited = Collections.synchronizedList(new ArrayList<>());
    HttpServer strict = serve(RuleWalk.OutputMode.STRICT, events);
    HttpServer audit = serve(RuleWalk.OutputMode.AUDIT, audited);
    try {
      legalMove(strict);
      illegalMove(strict, events);
      theOrderOfTheChecks(strict);
      severalSources(strict);
      finalState(strict);
      noResult(strict);
      targetCheck(strict, events, audit, audited);
    } finally {
      strict.stop(0);
      audit.stop(0);
    }
    System.out.println("OK");
  }

  /** A legal move calls the handler, which receives the field, the state it had and the target. */
  private static void legalMove(HttpServer server) throws Exception {
    stored = Status.PENDING;
    answered = Status.PAID;
    Answer a = post(server, path("pay"), pay("50.00"));
    check(a.status() == 201, "a legal move passes: " + a.status());
    check(received != null, "the handler received a transition");
    check(received.field().equals("status"), "the field is the wire path: " + received.field());
    check(received.from() == Status.PENDING, "from is the state that was checked: " + received.from());
    check(received.to() == Status.PAID, "to is the target: " + received.to());
    check(received.from().nextStates().contains(received.to()), "the helper agrees with the edge");
    check(!Status.PENDING.isFinal() && Status.REFUNDED.isFinal(), "isFinal");
    check(Status.PENDING.nextStates().equals(List.of(Status.PAID, Status.CANCELLED)), "nextStates of pending");
  }

  /** An illegal move is 409 InvalidTransition and the handler does not run. */
  private static void illegalMove(HttpServer server, List<RuleWalk.Event> events) throws Exception {
    stored = Status.PAID;
    int calls = HANDLER_CALLS.get();
    int seen = events.size();
    Answer a = post(server, path("pay"), pay("50.00"));
    check(a.status() == 409, "409: " + a.status());
    check(title(a).equals("InvalidTransition"), "InvalidTransition: " + a.body().write());
    Json.Obj data = a.body().get("data").asObject();
    check(data.get("field").asString().equals("status"), "field");
    check(data.get("from").asString().equals("paid"), "from");
    check(data.get("to").asString().equals("paid"), "to");
    var violations = a.body().get("violations").asArray().items();
    check(violations.size() == 1, "one violation");
    Json.Obj v = violations.get(0).asObject();
    check(v.get("rule").asString().startsWith("veld:bookings/Bookings.payBooking.transition@"), "rule id " + v.get("rule"));
    check(v.get("path").asString().equals("request"), "path request");
    check(v.get("fields").asArray().items().isEmpty(), "no fields");
    check(HANDLER_CALLS.get() == calls, "the handler did not run");
    check(events.size() == seen + 1 && events.get(seen).position() == RuleWalk.Position.INPUT, "the hook saw the guard");
  }

  /** A missing fact wins over the guard; the guard wins over {@code require}. */
  private static void theOrderOfTheChecks(HttpServer server) throws Exception {
    missing = true;
    check(post(server, path("pay"), pay("1.00")).status() == 404, "a missing fact is 404");
    missing = false;
    stored = Status.CANCELLED;
    Answer guard = post(server, path("pay"), pay("1.00"));
    check(guard.status() == 409 && title(guard).equals("InvalidTransition"), "the guard before require: " + guard.status());
    stored = Status.PENDING;
    Answer require = post(server, path("pay"), pay("1.00"));
    check(require.status() == 422 && title(require).equals("AmountMismatch"), "a legal move with a wrong amount: " + require.status());
  }

  /** Several sources: both pending and paid may be cancelled. */
  private static void severalSources(HttpServer server) throws Exception {
    for (Status s : new Status[] {Status.PENDING, Status.PAID}) {
      stored = s;
      Answer a = post(server, path("cancel"), "{}");
      check(a.status() == 201, "cancel from " + s + ": " + a.status());
      check(received.from() == s && received.to() == Status.CANCELLED, "the handler got " + received);
    }
  }

  /** No way out of a final state. */
  private static void finalState(HttpServer server) throws Exception {
    stored = Status.REFUNDED;
    Answer a = post(server, path("cancel"), "{}");
    check(a.status() == 409, "409 from a final state: " + a.status());
    check(a.body().get("data").asObject().get("from").asString().equals("refunded"), "from refunded");
    check(a.body().get("data").asObject().get("to").asString().equals("cancelled"), "to cancelled");
  }

  /** An endpoint without a result gets the guard and the transition and no target check. */
  private static void noResult(HttpServer server) throws Exception {
    stored = Status.PENDING;
    check(post(server, path("check-in"), "{}").status() == 409, "pending cannot check in");
    stored = Status.PAID;
    Answer ok = post(server, path("check-in"), "{}");
    check(ok.status() == 204, "paid can check in: " + ok.status());
    check(received.from() == Status.PAID && received.to() == Status.CHECKED_IN, "transition " + received);
  }

  /** The result must hold the target: strict answers 500, audit sends and reports. */
  private static void targetCheck(
      HttpServer strict, List<RuleWalk.Event> events, HttpServer audit, List<RuleWalk.Event> audited) throws Exception {
    stored = Status.PENDING;
    answered = Status.PENDING;
    int before = events.size();
    Answer a = post(strict, path("pay"), pay("50.00"));
    check(a.status() == 500 && title(a).equals("Internal"), "strict: 500 Internal: " + a.status());
    check(a.body().get("violations") == null, "no detail for the caller");
    check(events.size() == before + 1, "one event");
    RuleWalk.Event e = events.get(before);
    check(e.position() == RuleWalk.Position.OUTPUT && e.outcome() == RuleWalk.Outcome.REJECTED, "output, rejected");
    check(e.rule().startsWith("veld:bookings/Bookings.payBooking.transition@"), "rule " + e.rule());
    check(e.violation().message().contains("`paid`") && e.violation().message().contains("found `pending`"), "message " + e.violation().message());
    Answer sent = post(audit, path("pay"), pay("50.00"));
    check(sent.status() == 201, "audit sends the response: " + sent.status());
    check(audited.size() == 1 && audited.get(0).outcome() == RuleWalk.Outcome.AUDITED, "audit reports it");
    answered = Status.PAID;
    check(post(strict, path("pay"), pay("50.00")).status() == 201, "a result in the target state passes");
  }
}
