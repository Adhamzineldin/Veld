package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.bookings.User;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CancelBookingFacts;
import gen.b.bookings.server.CancelBookingRequest;
import gen.b.bookings.server.DividesBookingFacts;
import gen.b.bookings.server.DividesBookingRequest;
import gen.b.bookings.server.GetBookingFacts;
import gen.b.bookings.server.GetBookingRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.bookings.server.PositiveBookingFacts;
import gen.b.bookings.server.PositiveBookingRequest;
import gen.b.bookings.server.RefundBookingFacts;
import gen.b.bookings.server.RefundBookingRequest;
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
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The production handler of a contract with policies and a state machine over HTTP on the JDK
 * server: {@code 403 Forbidden} and its messages, several {@code allow} lines, the order of the
 * checks (a missing fact, the policies, the transition guard, {@code require}), the principal the
 * handler sees, and the generated client's {@code can<Endpoint>} against the server.
 */
public final class PolicyHttp {
  private PolicyHttp() {}

  private static final UUID GUEST = UUID.fromString("00000000-0000-4000-8000-0000000000aa");
  private static final UUID OTHER = UUID.fromString("00000000-0000-4000-8000-0000000000bb");
  private static final UUID BOOKING = UUID.fromString("00000000-0000-4000-8000-0000000000b1");
  private static final BigDecimal TOTAL = new BigDecimal("50.00");
  private static final AtomicInteger HANDLER_CALLS = new AtomicInteger();

  /** The callers by bearer token: id, roles, suspended. */
  private static final Map<String, Object[]> CALLERS = new LinkedHashMap<>();

  static {
    CALLERS.put("guest", new Object[] {GUEST, List.of(), false});
    CALLERS.put("member", new Object[] {GUEST, List.of("member"), false});
    CALLERS.put("other", new Object[] {OTHER, List.of(), false});
    CALLERS.put("support", new Object[] {OTHER, List.of("support"), false});
    CALLERS.put("suspended", new Object[] {GUEST, List.of("support"), true});
    CALLERS.put("otherMember", new Object[] {OTHER, List.of("member"), true});
    CALLERS.put("five", new Object[] {OTHER, List.of("a", "b", "c", "d", "e"), false});
  }

  @SuppressWarnings("unchecked")
  private static User serverUser(String token) {
    Object[] c = CALLERS.get(token);
    return c == null ? null : new User((UUID) c[0], (List<String>) c[1], (boolean) c[2]);
  }

  @SuppressWarnings("unchecked")
  private static gen.c.bookings.bookings.User clientUser(String token) {
    Object[] c = CALLERS.get(token);
    return new gen.c.bookings.bookings.User((UUID) c[0], (List<String>) c[1], (boolean) c[2]);
  }

  private static Status stored = Status.PENDING;
  private static boolean missing;
  private static User seen;
  private static Transition<Status> received;

  private static Booking booking(Status status) {
    return new Booking(BOOKING, GUEST, status, TOTAL);
  }

  private static final class Loaders implements BookingsFactLoaders {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return missing ? Optional.empty() : Optional.of(PolicyHttp.booking(stored));
    }
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking payBooking(
        PayBookingRequest request, PayBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      seen = (User) ctx.principal();
      received = transition;
      return booking(Status.PAID);
    }

    @Override
    public Booking getBooking(GetBookingRequest request, GetBookingFacts facts, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      return facts.booking();
    }

    @Override
    public Booking dividesBooking(DividesBookingRequest request, DividesBookingFacts facts, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      return facts.booking();
    }

    @Override
    public Booking refundBooking(RefundBookingRequest request, RefundBookingFacts facts, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      return facts.booking();
    }

    @Override
    public Booking positiveBooking(PositiveBookingRequest request, PositiveBookingFacts facts, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      return facts.booking();
    }

    @Override
    public Booking cancelBooking(
        CancelBookingRequest request, CancelBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
      HANDLER_CALLS.incrementAndGet();
      return booking(Status.CANCELLED);
    }
  }

  private record Answer(int status, Json.Obj body) {}

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static Answer call(HttpServer server, String method, String path, String token, String body)
      throws Exception {
    HttpRequest.Builder b = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path))
        .header("content-type", "application/json");
    if (token != null) {
      b.header("authorization", "Bearer " + token);
    }
    HttpRequest request = b.method(method, HttpRequest.BodyPublishers.ofString(body == null ? "" : body)).build();
    HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
    Json.Obj json = response.body().isEmpty() ? null : Json.parse(response.body()).asObject();
    return new Answer(response.statusCode(), json);
  }

  private static Answer pay(HttpServer server, String token, String amount) throws Exception {
    return call(server, "POST", "/bookings/" + BOOKING + "/pay", token, "{\"amount\":\"" + amount + "\"}");
  }

  private static Answer get(HttpServer server, String token) throws Exception {
    return call(server, "GET", "/bookings/" + BOOKING, token, null);
  }

  private static Answer getPath(HttpServer server, String token, String suffix) throws Exception {
    return call(server, "GET", "/bookings/" + BOOKING + "/" + suffix, token, null);
  }

  private static Answer cancel(HttpServer server, String token) throws Exception {
    return call(server, "POST", "/bookings/" + BOOKING + "/cancel", token, "{}");
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static String title(Answer a) {
    return a.body().get("title").asString();
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    BookingsHooks hooks = new BookingsHooks((token, ctx) -> serverUser(token)).onRuleViolation(events::add);
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsHandler.create(new Impl(), new Loaders(), hooks), new InetSocketAddress("127.0.0.1", 0));
    try {
      permitAndForbid(server, events);
      theOrderOfTheChecks(server);
      severalAllows(server);
      aClauseThatCannotBeEvaluatedFailsClosed(server);
      anArgumentThatReadsAParameter(server);
      theClientAgreesWithTheServer(server);
      thePolicyMethods();
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  /** A permitted caller reaches the handler, which sees the principal as the generated type. */
  private static void permitAndForbid(HttpServer server, List<RuleWalk.Event> events) throws Exception {
    check(pay(server, null, "50.00").status() == 401, "no token: 401");
    Answer ok = pay(server, "guest", "50.00");
    check(ok.status() == 201, "the guest may pay: " + ok.status());
    check(seen != null && seen.id().equals(GUEST), "the handler sees the principal: " + seen);
    check(received.from() == Status.PENDING && received.to() == Status.PAID, "the transition " + received);
    check(pay(server, "support", "50.00").status() == 201, "support may pay for anyone");

    int calls = HANDLER_CALLS.get();
    int before = events.size();
    Answer denied = pay(server, "other", "50.00");
    check(denied.status() == 403, "another caller: 403 " + denied.status());
    check(title(denied).equals("Forbidden"), "Forbidden: " + denied.body().write());
    check(denied.body().get("data").asObject().get("message").asString().equals("not permitted"), "message");
    var violations = denied.body().get("violations").asArray().items();
    check(violations.size() == 1, "one violation");
    Json.Obj v = violations.get(0).asObject();
    check(v.get("rule").asString().startsWith("veld:bookings/ManageBooking@"), "the policy id: " + v.get("rule"));
    check(v.get("path").asString().equals("request"), "path request");
    check(v.get("message").asString().equals("not permitted"), "the violation carries the same text");
    check(HANDLER_CALLS.get() == calls, "the handler did not run");
    check(events.size() == before + 1 && events.get(before).position() == RuleWalk.Position.INPUT, "the hook saw it");

    Answer suspended = pay(server, "suspended", "50.00");
    String text = "A suspended account cannot manage bookings";
    check(suspended.status() == 403, "forbid beats permit: " + suspended.status());
    check(suspended.body().get("data").asObject().get("message").asString().equals(text), "the forbid's message");
    check(suspended.body().get("violations").asArray().items().get(0).asObject().get("message").asString().equals(text),
        "and in the violation");
  }

  /** A missing fact, then the policies, then the transition guard, then {@code require}. */
  private static void theOrderOfTheChecks(HttpServer server) throws Exception {
    missing = true;
    check(pay(server, "other", "1.00").status() == 404, "a missing fact wins over a denial");
    missing = false;
    stored = Status.PAID;
    Answer denial = pay(server, "other", "50.00");
    check(denial.status() == 403, "a denial wins over the transition guard: " + denial.status());
    Answer guard = pay(server, "guest", "1.00");
    check(guard.status() == 409 && title(guard).equals("InvalidTransition"), "the guard wins over require: " + guard.status());
    stored = Status.PENDING;
    Answer wrong = pay(server, "other", "1.00");
    check(wrong.status() == 403, "a denial wins over require: " + wrong.status());
    Answer require = pay(server, "guest", "1.00");
    check(require.status() == 422, "a permitted caller, a legal move and a wrong amount: " + require.status());
  }

  /** Every {@code allow} must permit, in source order; the first denial ends the request. */
  private static void severalAllows(HttpServer server) throws Exception {
    stored = Status.PENDING;
    check(get(server, "guest").status() == 403, "the second allow denies a caller without a role");
    check(get(server, "member").status() == 200, "a guest with a role passes both");
    Answer first = get(server, "otherMember");
    check(first.status() == 403, "the first allow that denies ends the request: " + first.status());
    check(first.body().get("data").asObject().get("message").asString().equals("A suspended account cannot manage bookings"),
        "its message is the first allow's");
  }

  /**
   * A division by zero in a clause: such a {@code permit} does not permit, such a {@code forbid}
   * forbids, and an argument that cannot be evaluated denies.
   */
  private static void aClauseThatCannotBeEvaluatedFailsClosed(HttpServer server) throws Exception {
    stored = Status.PENDING;
    Answer none = getPath(server, "guest", "divides");
    check(none.status() == 403, "no roles: the permit fails and the forbid forbids: " + none.status());
    check(none.body().get("data").asObject().get("message").asString().equals("not permitted"), "no message");
    check(getPath(server, "member", "divides").status() == 403, "one role: the forbid is true");
    check(getPath(server, "five", "divides").status() == 200, "five roles: permitted");
    Answer arg = getPath(server, "guest", "positive");
    check(arg.status() == 403 && title(arg).equals("Forbidden"), "an argument that cannot be evaluated denies: " + arg.status());
    check(arg.body().get("data").asObject().get("message").asString().equals("not permitted"), "not permitted");
    check(getPath(server, "member", "positive").status() == 200, "a defined argument passes");
  }

  /** An argument that reads a request parameter: the violation names it. */
  private static void anArgumentThatReadsAParameter(HttpServer server) throws Exception {
    check(refund(server, "guest", "50.00").status() == 201, "a small payment passes");
    Answer big = refund(server, "guest", "500.00");
    check(big.status() == 403, "a big payment: " + big.status());
    check(big.body().get("violations").asArray().items().get(0).asObject().get("fields").write().equals("[\"body\"]"),
        "the violation names the parameter: " + big.body().write());
  }

  private static Answer refund(HttpServer server, String token, String amount) throws Exception {
    return call(server, "POST", "/bookings/" + BOOKING + "/refund", token, "{\"amount\":\"" + amount + "\"}");
  }

  /** {@code can<Endpoint>} answers what the server answers, for every caller and both states. */
  private static void theClientAgreesWithTheServer(HttpServer server) throws Exception {
    int compared = 0;
    for (Status state : new Status[] {Status.PENDING, Status.PAID}) {
      stored = state;
      gen.c.bookings.bookings.Booking fact = new gen.c.bookings.bookings.Booking(
          BOOKING, GUEST, gen.c.bookings.bookings.Status.valueOf(state.name()), TOTAL);
      for (String token : CALLERS.keySet()) {
        gen.c.bookings.bookings.User caller = clientUser(token);
        boolean payable = gen.c.bookings.client.BookingsClient.canPayBooking(fact, caller);
        check(payable == (pay(server, token, "50.00").status() == 201), "canPayBooking " + token + " " + state);
        boolean readable = gen.c.bookings.client.BookingsClient.canGetBooking(fact, caller);
        check(readable == (get(server, token).status() == 200), "canGetBooking " + token + " " + state);
        boolean divides = gen.c.bookings.client.BookingsClient.canDividesBooking(caller);
        check(divides == (getPath(server, token, "divides").status() == 200), "canDividesBooking " + token);
        boolean positive = gen.c.bookings.client.BookingsClient.canPositiveBooking(caller);
        check(positive == (getPath(server, token, "positive").status() == 200), "canPositiveBooking " + token);
        for (String amount : new String[] {"50.00", "500.00"}) {
          boolean small = gen.c.bookings.client.BookingsClient.canRefundBooking(
              caller, new gen.c.bookings.bookings.Payment(new BigDecimal(amount)));
          check(small == (refund(server, token, amount).status() / 100 == 2), "canRefundBooking " + token + " " + amount);
        }
        compared += 6;
      }
      boolean cancellable = gen.c.bookings.client.BookingsClient.canCancelBooking(fact);
      check(cancellable == (cancel(server, "guest").status() == 201), "canCancelBooking " + state);
      compared++;
    }
    check(compared == 2 * (6 * CALLERS.size() + 1), "every combination was compared: " + compared);
    // Spot checks of the answers themselves, so that agreeing on `false` everywhere cannot pass.
    gen.c.bookings.bookings.Booking pending = new gen.c.bookings.bookings.Booking(
        BOOKING, GUEST, gen.c.bookings.bookings.Status.PENDING, TOTAL);
    check(gen.c.bookings.client.BookingsClient.canPayBooking(pending, clientUser("guest")), "guest can pay a pending booking");
    check(!gen.c.bookings.client.BookingsClient.canPayBooking(pending, clientUser("other")), "another caller cannot");
    check(!gen.c.bookings.client.BookingsClient.canPayBooking(pending, clientUser("suspended")), "a suspended one cannot");
    check(gen.c.bookings.bookings.Status.PENDING.nextStates().size() == 2, "the client has the helpers");
  }

  /** The policy methods themselves: the boolean one and the verdict with the message. */
  private static void thePolicyMethods() {
    gen.c.bookings.bookings.Booking b = new gen.c.bookings.bookings.Booking(
        BOOKING, GUEST, gen.c.bookings.bookings.Status.PENDING, TOTAL);
    check(gen.c.bookings.bookings.Policies.manageBooking(clientUser("guest"), b), "permit");
    check(!gen.c.bookings.bookings.Policies.manageBooking(clientUser("suspended"), b), "forbid beats permit");
    var verdict = gen.c.bookings.bookings.Policies.manageBookingVerdict(clientUser("suspended"), b, 0L);
    check(!verdict.permitted() && verdict.denial().equals("A suspended account cannot manage bookings"), "message");
    var plain = gen.c.bookings.bookings.Policies.manageBookingVerdict(clientUser("other"), b, 0L);
    check(!plain.permitted() && plain.denial().equals("not permitted"), "no forbid: not permitted");
    check(gen.c.bookings.bookings.Policies.anyRole(clientUser("member")), "len(roles) > 0");
    check(!gen.c.bookings.bookings.Policies.anyRole(clientUser("guest")), "no roles");
  }
}
