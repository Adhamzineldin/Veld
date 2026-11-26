package veldtest;

import gen.b.runtime.DueMoves;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.timed.server.ArchiveBookingFacts;
import gen.b.timed.server.ArchiveBookingRequest;
import gen.b.timed.server.BookingsDueLoaders;
import gen.b.timed.server.BookingsDueMoves;
import gen.b.timed.server.BookingsFactLoaders;
import gen.b.timed.server.BookingsHandler;
import gen.b.timed.server.BookingsHooks;
import gen.b.timed.server.BookingsService;
import gen.b.timed.server.ExpireBookingFacts;
import gen.b.timed.server.ExpireBookingRequest;
import gen.b.timed.timed.Booking;
import gen.b.timed.timed.Status;
import java.time.Clock;
import java.time.Duration;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Timed moves over the real entry point (spec/platform.md section 10), against {@code
 * tests/toolchain/contract/platform_p2/timed.veld}: the boundary of the due check (exactly due, one
 * millisecond early, a {@code since} in the future), the message of the refusal, {@code
 * timedMoves()}, and the due-moves runner with a memory loader: a due move reaches the handler with
 * the timer's principal, a move another request made first is refused and not failed, a failing
 * handler does not stop the others, the report, {@code limit}, and a run without the timer's
 * principal on an endpoint with auth. Prints {@code OK}.
 */
public final class TimedHttp {
  private TimedHttp() {}

  private static final Instant NOW = Instant.parse("2026-06-11T12:00:00Z");
  private static final Map<UUID, Booking> STORE = new ConcurrentHashMap<>();
  private static final List<Object> PRINCIPALS = new CopyOnWriteArrayList<>();
  private static final List<Throwable> ERRORS = new CopyOnWriteArrayList<>();
  private static final AtomicInteger AUTH_CALLS = new AtomicInteger();
  private static final UUID BOOM = UUID.fromString("00000000-0000-4000-8000-0000000000bb");

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static UUID id(int n) {
    return UUID.fromString(String.format("00000000-0000-4000-8000-%012d", n));
  }

  private static Booking put(UUID id, Status status, Instant since) {
    Booking b = new Booking(id, status, since);
    STORE.put(id, b);
    return b;
  }

  private static final class Impl implements BookingsService {
    private Booking move(UUID id, Status to, RequestContext ctx) {
      PRINCIPALS.add(ctx.principal());
      if (id.equals(BOOM)) {
        throw new IllegalStateException("the store is down");
      }
      Booking old = STORE.get(id);
      Booking moved = new Booking(id, to, old.pendingSince());
      STORE.put(id, moved);
      return moved;
    }

    @Override
    public Booking expireBooking(
        ExpireBookingRequest request, ExpireBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
      return move(request.id(), transition.to(), ctx);
    }

    @Override
    public Booking archiveBooking(
        ArchiveBookingRequest request, ArchiveBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
      return move(request.id(), transition.to(), ctx);
    }
  }

  private static final BookingsFactLoaders FACTS = new BookingsFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.ofNullable(STORE.get(key));
    }
  };

  private static BookingsHooks hooks() {
    return new BookingsHooks((token, ctx) -> {
          AUTH_CALLS.incrementAndGet();
          return token;
        })
        .clock(Clock.fixed(NOW, ZoneOffset.UTC))
        .onError((error, ctx) -> ERRORS.add(error));
  }

  private static VeldResponse post(VeldHandler handler, String path) {
    VeldRequest request = new VeldRequest("POST", path, "", Map.of("authorization", List.of("Bearer caller")), null, null);
    return handler.handle(request).orElseThrow(() -> new AssertionError("no route for " + path));
  }

  public static void main(String[] args) throws Exception {
    theDueCheck();
    timedMovesOfTheStateMachine();
    theRunner();
    System.out.println("OK");
  }

  /** `now - since >= 24 h`: exactly due is due, one millisecond less is not, the future is not. */
  private static void theDueCheck() {
    VeldHandler handler = BookingsHandler.create(new Impl(), FACTS, hooks());
    Instant limit = NOW.minus(Duration.ofHours(24));
    put(id(1), Status.PENDING, limit);
    VeldResponse exact = post(handler, "/bookings/" + id(1) + "/expire");
    check(exact.status() == 200, "exactly due: " + exact.status() + exact.body());
    check(STORE.get(id(1)).status() == Status.CANCELLED, "and the handler moved it");

    put(id(2), Status.PENDING, limit.plusMillis(1));
    VeldResponse early = post(handler, "/bookings/" + id(2) + "/expire");
    check(early.status() == 409, "one millisecond early: " + early.status() + early.body());
    Json.Obj problem = Json.parse(early.body()).asObject();
    check(problem.get("type").asString().contains("InvalidTransition"), "an InvalidTransition: " + early.body());
    check(problem.get("data").asObject().get("field").asString().equals("status"), "with the field: " + early.body());
    check(problem.get("data").asObject().get("from").asString().equals("pending"), "from: " + early.body());
    check(problem.get("data").asObject().get("to").asString().equals("cancelled"), "to: " + early.body());
    check(problem.get("violations").asArray().items().get(0).asObject().get("message").asString()
        .equals("violates rule transition: not due"), "the message says why: " + early.body());
    check(STORE.get(id(2)).status() == Status.PENDING, "and nothing moved");

    put(id(3), Status.PENDING, NOW.plus(Duration.ofHours(1)));
    check(post(handler, "/bookings/" + id(3) + "/expire").status() == 409, "a since in the future is not due");

    put(id(4), Status.PAID, limit.minus(Duration.ofDays(3)));
    VeldResponse wrong = post(handler, "/bookings/" + id(4) + "/expire");
    check(wrong.status() == 409, "another state: " + wrong.status());
    check(Json.parse(wrong.body()).asObject().get("violations").asArray().items().get(0).asObject().get("message").asString()
        .equals("violates rule transition"), "the plain message for the state: " + wrong.body());
  }

  private static void timedMovesOfTheStateMachine() {
    check(Status.timedMoves().equals(List.of(new Status.TimedMove(Status.PENDING, Status.CANCELLED, 86_400_000L))),
        "timedMoves: " + Status.timedMoves());
    check(Status.PENDING.nextStates().contains(Status.CANCELLED), "the timed edge is an edge for nextStates");
  }

  /** What the runner asked the loaders and what the handler saw. */
  private static final class Memory implements BookingsDueLoaders {
    final List<Instant> before = new ArrayList<>();
    final List<Integer> limits = new ArrayList<>();
    final List<UUID> due = new ArrayList<>();
    final AtomicInteger archived = new AtomicInteger();

    @Override
    public List<ExpireBookingRequest> expireBooking(Instant at, int limit, RequestContext ctx) {
      before.add(at);
      limits.add(limit);
      List<ExpireBookingRequest> out = new ArrayList<>();
      for (UUID id : due) {
        out.add(new ExpireBookingRequest(id));
      }
      return out;
    }

    @Override
    public List<ArchiveBookingRequest> archiveBooking(Instant at, int limit, RequestContext ctx) {
      archived.incrementAndGet();
      return List.of();
    }
  }

  private static void theRunner() {
    Instant old = NOW.minus(Duration.ofDays(2));
    put(id(10), Status.PENDING, old);
    put(id(11), Status.PAID, old); // another request was first
    put(BOOM, Status.PENDING, old); // the handler fails
    put(id(12), Status.PENDING, old);
    Memory memory = new Memory();
    memory.due.addAll(List.of(id(10), id(11), BOOM, id(12)));
    PRINCIPALS.clear();
    ERRORS.clear();

    Map<String, DueMoves.Entry> report =
        BookingsDueMoves.run(new Impl(), FACTS, hooks().timer(() -> "the-timer"), new BookingsDueMoves.Options(memory).now(NOW));
    check(report.get("expireBooking").equals(new DueMoves.Entry(4, 2, 1, 1)), "the report: " + report);
    check(report.get("archiveBooking").equals(new DueMoves.Entry(0, 0, 0, 0)), "an endpoint with nothing due: " + report);
    check(memory.before.equals(List.of(NOW.minus(Duration.ofHours(24)))), "before = now - 24 h: " + memory.before);
    check(memory.limits.equals(List.of(100)), "the default limit: " + memory.limits);
    check(STORE.get(id(10)).status() == Status.CANCELLED && STORE.get(id(12)).status() == Status.CANCELLED,
        "the due moves ran, also the one after the failing one");
    check(STORE.get(id(11)).status() == Status.PAID, "the one another request moved first is untouched");
    check(PRINCIPALS.contains("the-timer") && PRINCIPALS.size() == 3, "the handler saw the timer's principal: " + PRINCIPALS);
    check(AUTH_CALLS.get() == 0, "the authentication hook did not run");
    check(ERRORS.size() == 1 && ERRORS.get(0) instanceof IllegalStateException, "the failure went to onError: " + ERRORS);
    check(memory.archived.get() == 1, "the endpoint without auth was asked as well");

    // `limit` bounds the calls of an endpoint, whatever the loader answers.
    put(id(20), Status.PENDING, old);
    put(id(21), Status.PENDING, old);
    put(id(22), Status.PENDING, old);
    Memory many = new Memory();
    many.due.addAll(List.of(id(20), id(21), id(22)));
    Map<String, DueMoves.Entry> limited =
        BookingsDueMoves.run(new Impl(), FACTS, hooks().timer(() -> "the-timer"), new BookingsDueMoves.Options(many).now(NOW).limit(2));
    check(many.limits.equals(List.of(2)), "the loader was told the limit: " + many.limits);
    check(limited.get("expireBooking").ran() == 2, "at most limit calls: " + limited);

    // Without the hook a timed endpoint that has auth does not run, and nothing is asked.
    Memory none = new Memory();
    try {
      BookingsDueMoves.run(new Impl(), FACTS, hooks(), new BookingsDueMoves.Options(none).now(NOW));
      throw new AssertionError("a run without hooks.timer was started");
    } catch (IllegalStateException e) {
      check(e.getMessage().startsWith("expireBooking: hooks.timer.principal is required"), e.getMessage());
    }
    check(none.before.isEmpty() && none.archived.get() == 0, "no loader was called");
  }
}
