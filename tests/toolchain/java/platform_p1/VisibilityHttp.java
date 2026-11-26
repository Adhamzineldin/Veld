package veldtest;

import gen.b.runtime.Json;
import gen.b.runtime.Page;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.SseEvent;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.runtimeconfig.VisibilityRuntime;
import gen.b.visibility.server.CancelBookingFacts;
import gen.b.visibility.server.CancelBookingRequest;
import gen.b.visibility.server.CheckedBookingRequest;
import gen.b.visibility.server.DeskFactLoaders;
import gen.b.visibility.server.DeskHandler;
import gen.b.visibility.server.DeskHooks;
import gen.b.visibility.server.DeskService;
import gen.b.visibility.server.GetBookingRequest;
import gen.b.visibility.server.ListBookingsRequest;
import gen.b.visibility.server.LiveBookingsRequest;
import gen.b.visibility.server.NoticesHandler;
import gen.b.visibility.server.NoticesHooks;
import gen.b.visibility.server.NoticesService;
import gen.b.visibility.server.PageBookingsRequest;
import gen.b.visibility.server.PublicBookingRequest;
import gen.b.visibility.visibility.Booking;
import gen.b.visibility.visibility.Guest;
import gen.b.visibility.visibility.User;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * Field visibility over the real entry point (spec/platform.md section 2), against {@code
 * tests/toolchain/contract/platform_p1/visibility.veld}: a field hidden for one caller and present
 * for another, inside a nested model, a list, a page, a declared error and a stream item, an
 * expression that cannot be evaluated, the clock of the request to the millisecond, an {@code
 * ensure} that still sees the field, an endpoint with no caller, the rule output mode {@code off}
 * and {@code validateResponses}. Prints {@code OK}.
 *
 * <p>The tests that run it generate the contract twice more, with {@code validate = false} and as
 * the server of a different framework; the program is the same.
 */
public final class VisibilityHttp {
  private VisibilityHttp() {}

  private static final UUID GUEST = UUID.fromString("00000000-0000-4000-8000-0000000000aa");
  private static final UUID OTHER = UUID.fromString("00000000-0000-4000-8000-0000000000bb");
  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final Instant OPENS = Instant.parse("2026-06-10T12:00:00Z");

  private static volatile int divisor = 1;
  private static volatile int closed = 1;

  private static Booking booking(UUID guest, String phone) {
    return new Booking(ID, new Guest(guest, "Guest", phone), divisor, OPENS, "n", "r", "e");
  }

  private static final class Impl implements DeskService {
    @Override
    public Booking getBooking(GetBookingRequest request, RequestContext ctx) {
      return booking(GUEST, "+1");
    }

    @Override
    public List<Booking> listBookings(ListBookingsRequest request, RequestContext ctx) {
      return List.of(booking(GUEST, "+1"), booking(OTHER, "+2"));
    }

    @Override
    public Page<Booking> pageBookings(PageBookingsRequest request, RequestContext ctx) {
      return new Page<>(List.of(booking(GUEST, "+1"), booking(OTHER, "+2")), 2L, 1, 20);
    }

    @Override
    public Booking checkedBooking(CheckedBookingRequest request, RequestContext ctx) {
      return booking(GUEST, "+1");
    }

    @Override
    public Booking cancelBooking(CancelBookingRequest request, CancelBookingFacts facts, RequestContext ctx) {
      return facts.booking();
    }

    @Override
    public Stream<SseEvent<Booking>> liveBookings(LiveBookingsRequest request, RequestContext ctx) {
      return Stream.of(SseEvent.of(booking(GUEST, "+1")), SseEvent.of(booking(OTHER, "+2")));
    }
  }

  private static final DeskFactLoaders LOADERS = new DeskFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.of(new Booking(ID, new Guest(GUEST, "Guest", "+1"), closed, OPENS, "n", "r", "e"));
    }
  };

  private static final class Notices implements NoticesService {
    @Override
    public Booking publicBooking(PublicBookingRequest request, RequestContext ctx) {
      return booking(GUEST, "+1");
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  /** The caller of a request: the token is {@code <uuid>:<role>,<role>}. */
  private static User caller(String token) {
    String[] parts = token.split(":", 2);
    List<String> roles = parts.length < 2 || parts[1].isEmpty() ? List.of() : List.of(parts[1].split(","));
    return new User(UUID.fromString(parts[0]), roles);
  }

  private static VeldHandler desk(Clock clock, boolean off, List<Throwable> errors) {
    DeskHooks hooks = new DeskHooks((token, ctx) -> caller(token)).clock(clock).onError((e, ctx) -> errors.add(e));
    if (off) {
      return DeskHandler.create(
          new Impl(), LOADERS, hooks, VisibilityRuntime.builder().rulesOutput(RuleWalk.OutputMode.OFF).build());
    }
    return DeskHandler.create(new Impl(), LOADERS, hooks);
  }

  private static VeldHandler notices() {
    return NoticesHandler.create(new Notices(), new NoticesHooks());
  }

  private static VeldResponse send(VeldHandler handler, String method, String path, String token) {
    Map<String, List<String>> headers =
        token == null ? Map.of() : Map.of("authorization", List.of("Bearer " + token));
    return handler
        .handle(new VeldRequest(method, path, null, headers, null, null))
        .orElseThrow(() -> new AssertionError("no route " + method + " " + path));
  }

  private static Json.Obj body(VeldResponse response) {
    return Json.parse(response.body()).asObject();
  }

  private static Json at(Json json, String... path) {
    Json cur = json;
    for (String step : path) {
      if (!(cur instanceof Json.Obj obj) || obj.get(step) == null) {
        return null;
      }
      cur = obj.get(step);
    }
    return cur;
  }

  private static Json item(Json json, int index, String... path) {
    return at(json.asArray().items().get(index), path);
  }

  private static Clock at(Instant instant) {
    return Clock.fixed(instant, ZoneOffset.UTC);
  }

  public static void main(String[] args) {
    // `unvalidated`: the contract was generated with `validate = false`, so the rules of the
    // endpoints (`ensure`, `require`) do not run; what visibility does is the same.
    boolean rules = args.length == 0;
    List<Throwable> errors = new ArrayList<>();
    String guest = GUEST + ":";
    String other = OTHER + ":";
    String support = OTHER + ":support";

    // One value: the guest's own phone is there for the guest, not for another caller; `note` is
    // for support; the rest of the fields are for everyone.
    VeldHandler handler = desk(at(OPENS), false, errors);
    VeldResponse own = send(handler, "GET", "/desk/" + ID, guest);
    check(own.status() == 200, "the guest is answered");
    check(at(body(own), "guest", "phone") != null, "the guest sees their own phone");
    check(at(body(own), "note") == null, "a guest does not see the note");
    check(at(body(own), "ratio") != null, "an expression that holds shows its field");
    check(at(body(own), "early") != null, "the clock of the request is the opening time: shown from it on");
    check(at(body(own), "guest", "name") != null, "a field without a guard is there");
    VeldResponse another = send(handler, "GET", "/desk/" + ID, other);
    check(at(body(another), "guest", "phone") == null, "another caller does not see the guest's phone");
    VeldResponse staff = send(handler, "GET", "/desk/" + ID, support);
    check(at(body(staff), "guest", "phone") != null, "support sees the phone of any guest");
    check(at(body(staff), "note") != null, "support sees the note");

    // The clock: one millisecond before the opening, the field is hidden.
    VeldHandler early = desk(at(OPENS.minusMillis(1)), false, errors);
    check(at(body(send(early, "GET", "/desk/" + ID, support)), "early") == null, "before the opening time");
    check(at(body(send(handler, "GET", "/desk/" + ID, support)), "early") != null, "at the opening time");

    // An expression that cannot be evaluated hides the field, for everyone.
    divisor = 0;
    VeldResponse broken = send(handler, "GET", "/desk/" + ID, support);
    check(broken.status() == 200, "an expression that fails is not an error");
    check(at(body(broken), "ratio") == null, "a division by zero hides the field even from support");
    check(at(body(broken), "note") != null, "the other fields are judged on their own");
    divisor = 1;

    // A list: each element is judged for the caller.
    VeldResponse list = send(handler, "GET", "/desk", guest);
    check(list.status() == 200, "the list is answered");
    check(item(body(list), 0, "guest", "phone") != null, "the guest's own booking has the phone");
    check(item(body(list), 1, "guest", "phone") == null, "another guest's booking does not");

    // A page: each item of `items`.
    VeldResponse page = send(handler, "GET", "/desk/page", guest);
    check(page.status() == 200, "the page is answered");
    check(at(body(page), "items") != null && at(body(page), "total") != null, "the page keeps its own members");
    check(item(at(body(page), "items"), 0, "guest", "phone") != null, "the first item has its phone");
    check(item(at(body(page), "items"), 1, "guest", "phone") == null, "the second item does not");

    // An `ensure` sees the whole value: the note is in the result when it is judged.
    VeldResponse checked = send(handler, "GET", "/desk/checked/" + ID, guest);
    check(checked.status() == 200, "an ensure reads the field that is then dropped: " + checked.body());
    check(at(body(checked), "note") == null, "and the caller still does not get it");

    // A declared error: its `who` is judged for the caller.
    if (rules) {
      closed = 0;
      VeldResponse refused = send(handler, "POST", "/desk/" + ID + "/cancel", guest);
      check(refused.status() == 409, "the error is declared at 409: " + refused.body());
      check(at(body(refused), "data", "reason") != null, "the error keeps its reason");
      check(at(body(refused), "data", "who") == null, "a guest does not see who");
      VeldResponse refusedForStaff = send(handler, "POST", "/desk/" + ID + "/cancel", support);
      check(at(body(refusedForStaff), "data", "who") != null, "support sees who");
      closed = 1;
    }

    // A stream item is judged for the caller too.
    VeldResponse live = send(handler, "GET", "/desk/live", guest);
    check(live.status() == 200 && live.frames() != null, "the stream is opened");
    List<String> frames = live.frames().collect(Collectors.toList());
    check(frames.size() == 2, "two items: " + frames);
    check(frames.get(0).contains("\"phone\""), "the item of the guest has the phone: " + frames.get(0));
    check(!frames.get(1).contains("\"phone\""), "the item of another guest does not: " + frames.get(1));
    check(!frames.get(0).contains("\"note\""), "no item has the note for a guest");

    // No caller: every `@visible` field of the answer is dropped.
    VeldResponse notice = send(notices(), "GET", "/notices/" + ID, null);
    check(notice.status() == 200, "the notice is answered");
    check(at(body(notice), "guest", "phone") == null, "no caller, no phone");
    check(at(body(notice), "note") == null && at(body(notice), "early") == null, "no caller, no note");
    check(at(body(notice), "ratio") == null, "no caller: even a field whose expression holds is dropped");
    check(at(body(notice), "guest", "name") != null, "what has no guard is there");

    // The rule output mode `off` and response validation change nothing: visibility is not an
    // output check.
    VeldHandler off = desk(at(OPENS), true, errors);
    check(at(body(send(off, "GET", "/desk/" + ID, other)), "guest", "phone") == null, "output off does not show it");
    check(at(body(send(off, "GET", "/desk/" + ID, guest)), "guest", "phone") != null, "output off does not hide it");
    DeskHooks validating = new DeskHooks((token, ctx) -> caller(token)).validateResponses(true);
    VeldHandler checking = DeskHandler.create(new Impl(), LOADERS, validating);
    check(at(body(send(checking, "GET", "/desk/" + ID, other)), "guest", "phone") == null, "validated responses");

    check(errors.isEmpty(), "nothing went to onError: " + errors);
    System.out.println("OK");
  }
}
