package veldtest;

import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.bookings.User;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CreateBookingRequest;
import gen.b.bookings.server.GetBookingFacts;
import gen.b.bookings.server.GetBookingRequest;
import gen.b.bookings.server.ListBookingsRequest;
import gen.b.bookings.server.PageBookingsRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.bookings.server.PublicBookingRequest;
import gen.b.bookings.server.PublicHandler;
import gen.b.bookings.server.PublicHooks;
import gen.b.bookings.server.PublicService;
import gen.b.bookings.server.RemoveBookingRequest;
import gen.b.bookings.server.SetTotalFacts;
import gen.b.bookings.server.SetTotalRequest;
import gen.b.bookings.server.TouchRequest;
import gen.b.runtime.AuditEvent;
import gen.b.runtime.Codec;
import gen.b.runtime.Json;
import gen.b.runtime.Limiter;
import gen.b.runtime.Page;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Schema;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.runtime.Wire;
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
 * Every case of {@code tests/wire/audit/cases.json} through the generated server with the stubs of the
 * case (spec/platform.md section 4, tests/wire/audit/README.md): the status and the error of the answer,
 * the hook called once with an event whose canonical bytes are the golden's, or not at all for an
 * endpoint that is not audited. {@code payBooking} is given {@code @limit(10, per: minute, by: user)}
 * by the test that generates the contract (an operation signature does not change with a limit), so
 * that the case of the limiter runs; the limiter of the program answers what the case says.
 *
 * <p>One difference of representation is bridged, and only for the page: the golden writes {@code
 * total} of a page as a JSON number as the reference wrote it, while a {@code long} is the text of
 * its digits on the wire of a Java server (spec/language.md section 5). The value the handler returns
 * has the number turned into text, and the same member of the event is turned back into a number
 * before the bytes are compared. Prints {@code OK}.
 */
public final class AuditGoldens {
  private AuditGoldens() {}

  /** The case being run: what the stubs answer. */
  private static volatile Json.Obj current;

  private static Json member(String name) {
    return current.get(name);
  }

  private static Json wireOf(Schema schema, Json value) {
    return Wire.prepare(schema, value, Wire.Mode.RESPONSE);
  }

  /** What the handler returns: the value of {@code returns} in the type of the endpoint. */
  private static Booking booking() {
    Json returns = member("returns");
    if (returns == null || returns instanceof Json.Null) {
      throw new IllegalStateException("the handler was reached and the case says it returns nothing");
    }
    return Booking.codec().decode(wireOf(Booking.SCHEMA, returns));
  }

  /** A {@code long} is the text of its digits on the wire of a Java server: the number of the golden becomes text. */
  private static Json textOfTotal(Json page) {
    if (page instanceof Json.Obj obj && obj.get("total") instanceof Json.Num total) {
      return obj.with("total", Json.of(Long.toString((long) total.value())));
    }
    return page;
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking payBooking(
        PayBookingRequest request, PayBookingFacts facts, Integer version, Transition<Status> transition, RequestContext ctx) {
      return booking();
    }

    @Override
    public Booking getBooking(GetBookingRequest request, GetBookingFacts facts, Integer version, RequestContext ctx) {
      return booking();
    }

    @Override
    public List<Booking> listBookings(ListBookingsRequest request, RequestContext ctx) {
      return Codec.list(Booking.codec()).decode(wireOf(Schema.list(Booking.SCHEMA), member("returns")));
    }

    @Override
    public Page<Booking> pageBookings(PageBookingsRequest request, RequestContext ctx) {
      return Codec.page(Booking.codec())
          .decode(wireOf(Schema.page(Booking.SCHEMA), textOfTotal(member("returns"))));
    }

    @Override
    public Booking createBooking(CreateBookingRequest request, RequestContext ctx) {
      return booking();
    }

    @Override
    public void removeBooking(RemoveBookingRequest request, RequestContext ctx) {}

    @Override
    public Booking setTotal(SetTotalRequest request, SetTotalFacts facts, Integer version, RequestContext ctx) {
      return booking();
    }

    @Override
    public void touch(TouchRequest request, RequestContext ctx) {}
  }

  private static final BookingsFactLoaders LOADERS = new BookingsFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      Json facts = member("facts");
      Json held = facts instanceof Json.Obj obj ? obj.get("booking") : null;
      if (held == null || held instanceof Json.Null) {
        return Optional.empty();
      }
      return Optional.of(Booking.codec().decode(wireOf(Booking.SCHEMA, held)));
    }
  };

  private static final PublicService PUBLIC = new PublicService() {
    @Override
    public Booking publicBooking(PublicBookingRequest request, RequestContext ctx) {
      return booking();
    }
  };

  private static User principal() {
    Json who = member("principal");
    return who instanceof Json.Obj obj ? User.codec().decode(wireOf(User.SCHEMA, obj)) : null;
  }

  private static Json.Obj members(Json value) {
    return value instanceof Json.Obj obj ? obj : new Json.Obj(Map.of());
  }

  public static void main(String[] args) throws Exception {
    Json.Obj file = Json.parse(Files.readString(Path.of("cases.json"))).asObject();
    List<Json> cases = file.get("cases").asArray().items();
    for (Json item : cases) {
      run(item.asObject());
    }
    System.out.println("audit goldens: " + cases.size() + " cases");
    System.out.println("OK");
  }

  private static void run(Json.Obj c) {
    String name = c.get("name").asString();
    current = c;
    List<AuditEvent> events = new CopyOnWriteArrayList<>();
    List<Throwable> errors = new ArrayList<>();
    Clock clock = Clock.fixed(Instant.parse(c.get("clock").asString()), ZoneOffset.UTC);
    // The limiter answers what the case says; a case without one is never refused.
    Limiter limiter = (key, limit, windowSeconds) -> {
      Json.Obj answer = current.get("limiter") instanceof Json.Obj o ? o : null;
      return answer == null || answer.get("allowed").asBoolean()
          ? Limiter.Decision.allow()
          : Limiter.Decision.refuse((long) answer.get("retryAfterSeconds").asDouble());
    };
    VeldHandler handler;
    if (c.get("service").asString().equals("Bookings")) {
      BookingsHooks hooks = new BookingsHooks((token, ctx) -> principal(), (who, ctx) -> "caller", events::add)
          .clock(clock)
          .limiter(limiter)
          .onError((e, ctx) -> errors.add(e));
      handler = BookingsHandler.create(new Impl(), LOADERS, hooks);
    } else {
      PublicHooks hooks = new PublicHooks(events::add).clock(clock).onError((e, ctx) -> errors.add(e));
      handler = PublicHandler.create(PUBLIC, hooks);
    }

    Json.Obj request = c.get("request").asObject();
    Map<String, List<String>> headers = new LinkedHashMap<>();
    headers.put("authorization", List.of("Bearer t"));
    members(request.get("headers")).fields().forEach((key, value) -> headers.put(key.toLowerCase(java.util.Locale.ROOT), List.of(value.asString())));
    String body = request.get("body") == null || request.get("body") instanceof Json.Null ? null : request.get("body").write();
    VeldResponse response = handler
        .handle(new VeldRequest(request.get("method").asString(), request.get("path").asString(), null, headers, body, null))
        .orElseThrow(() -> new AssertionError(name + ": no route " + request.get("path").asString()));

    int status = (int) c.get("status").asDouble();
    if (response.status() != status) {
      throw new AssertionError(name + ": the status is " + status + ", got " + response.status() + " " + response.body());
    }
    if (c.get("error") instanceof Json.Str error) {
      String title = Json.parse(response.body()).asObject().get("title").asString();
      if (!title.equals(error.value())) {
        throw new AssertionError(name + ": the error is " + error.value() + ", got " + title);
      }
    }
    if (!errors.isEmpty()) {
      throw new AssertionError(name + ": nothing goes to onError: " + errors);
    }
    if (c.get("event") instanceof Json.Str golden) {
      if (events.size() != 1) {
        throw new AssertionError(name + ": the hook is called once, it was called " + events.size() + " times");
      }
      Json.Obj event = events.get(0).toJson();
      if (c.get("endpoint").asString().equals("pageBookings") && event.get("result") instanceof Json.Obj result
          && result.get("total") instanceof Json.Str total) {
        event = event.with("result", result.with("total", Json.of(Long.parseLong(total.value()))));
      }
      if (!event.canonical().equals(golden.value())) {
        throw new AssertionError(name + ": the event\n  got      " + event.canonical() + "\n  expected " + golden.value());
      }
    } else if (!events.isEmpty()) {
      throw new AssertionError(name + ": an endpoint that is not audited does not call the hook: " + events);
    }
  }
}
