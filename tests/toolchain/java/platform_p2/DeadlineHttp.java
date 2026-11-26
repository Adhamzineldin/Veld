package veldtest;

import gen.b.levels.levels.Room;
import gen.b.levels.server.BuildRequest;
import gen.b.levels.server.GetRoomRequest;
import gen.b.levels.server.ListRoomsRequest;
import gen.b.levels.server.RoomsHandler;
import gen.b.levels.server.RoomsHooks;
import gen.b.levels.server.RoomsService;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Deadlines over the real entry point (spec/platform.md section 12), against {@code
 * tests/toolchain/contract/platform_p2/service_levels.veld} ({@code getRoom} and {@code build}
 * have a deadline of 200 ms, {@code listRooms} has none): a handler that answers in time sees the
 * deadline in its context; one that does not gets {@code 504 DeadlineExceeded} with {@code
 * timeoutMs}, is interrupted once and sees the context expire, its late answer is dropped and
 * nothing goes to {@code onError}; the idempotency key of a request that hit its deadline is released
 * at the deadline, and the late request does not touch the key of its retry. Prints {@code OK}.
 */
public final class DeadlineHttp {
  private DeadlineHttp() {}

  private static final UUID FAST = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID SLOW = UUID.fromString("00000000-0000-4000-8000-000000000002");

  private static final AtomicReference<Instant> SEEN_DEADLINE = new AtomicReference<>();
  private static final AtomicReference<Boolean> SEEN_EXPIRED = new AtomicReference<>();
  private static final AtomicInteger INTERRUPTS = new AtomicInteger();
  private static final AtomicInteger BUILDS = new AtomicInteger();
  private static final CountDownLatch SLOW_STARTED = new CountDownLatch(1);
  private static final CountDownLatch SLOW_SAW_EXPIRY = new CountDownLatch(1);
  private static final CountDownLatch LATE = new CountDownLatch(1);
  private static final CountDownLatch LATE_DONE = new CountDownLatch(1);
  private static final List<Throwable> ERRORS = new CopyOnWriteArrayList<>();

  private static final class Impl implements RoomsService {
    @Override
    public Room getRoom(GetRoomRequest request, RequestContext ctx) throws Exception {
      SEEN_DEADLINE.set(ctx.deadline());
      SEEN_EXPIRED.set(ctx.expired());
      if (request.id().equals(SLOW)) {
        SLOW_STARTED.countDown();
        try {
          Thread.sleep(5_000);
        } catch (InterruptedException e) {
          INTERRUPTS.incrementAndGet();
          SEEN_EXPIRED.set(ctx.expired());
          SLOW_SAW_EXPIRY.countDown();
          throw e;
        }
      }
      return new Room(request.id(), "a");
    }

    @Override
    public List<Room> listRooms(ListRoomsRequest request, RequestContext ctx) {
      SEEN_DEADLINE.set(ctx.deadline());
      return List.of();
    }

    @Override
    public Room build(BuildRequest request, RequestContext ctx) {
      int call = BUILDS.incrementAndGet();
      if (call == 1) {
        // The first request holds the key past its deadline and ignores the interrupt.
        boolean interrupted = false;
        while (true) {
          try {
            LATE.await();
            break;
          } catch (InterruptedException e) {
            interrupted = true;
          }
        }
        if (interrupted) {
          INTERRUPTS.incrementAndGet();
        }
        LATE_DONE.countDown();
      }
      return new Room(FAST, request.body().name() + call);
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static VeldResponse call(VeldHandler handler, String method, String path, String body, String key) {
    Map<String, List<String>> headers = body == null
        ? Map.of()
        : Map.of("content-type", List.of("application/json"), "idempotency-key", List.of(key));
    return handler.handle(new VeldRequest(method, path, "", headers, body, null)).orElseThrow(() -> new AssertionError("no route " + path));
  }

  public static void main(String[] args) throws Exception {
    VeldHandler handler = RoomsHandler.create(
        new Impl(), new RoomsHooks(new MemoryIdempotencyStore()).onError((e, ctx) -> ERRORS.add(e)));

    // In time: the context has the deadline, an endpoint without one has none.
    Instant before = Instant.now();
    VeldResponse ok = call(handler, "GET", "/rooms/" + FAST, null, null);
    check(ok.status() == 200, "an answer in time: " + ok.status() + ok.body());
    check(SEEN_DEADLINE.get() != null && !SEEN_DEADLINE.get().isBefore(before.plusMillis(150)) && SEEN_DEADLINE.get().isBefore(Instant.now().plusMillis(250)),
        "ctx.deadline() is the arrival plus 200 ms: " + SEEN_DEADLINE.get());
    check(Boolean.FALSE.equals(SEEN_EXPIRED.get()), "and has not expired");
    call(handler, "GET", "/rooms", null, null);
    check(SEEN_DEADLINE.get() == null, "an endpoint without @timeout has no deadline");

    // Not in time: 504, its body, the interrupt, the context expired, nothing in onError.
    long started = System.nanoTime();
    VeldResponse late = call(handler, "GET", "/rooms/" + SLOW, null, null);
    long elapsedMs = (System.nanoTime() - started) / 1_000_000L;
    check(late.status() == 504, "the deadline answers 504: " + late.status() + late.body());
    check(elapsedMs < 2_000, "the answer does not wait for the handler: " + elapsedMs + " ms");
    check(late.headers().get("content-type").equals("application/problem+json"), "a problem document");
    Json.Obj problem = Json.parse(late.body()).asObject();
    check(problem.get("type").asString().contains("/DeadlineExceeded@"), "DeadlineExceeded: " + late.body());
    check(problem.get("title").asString().equals("DeadlineExceeded"), "its title");
    check(problem.get("data").asObject().get("timeoutMs").asString().equals("200"), "timeoutMs as text: " + late.body());
    check(SLOW_SAW_EXPIRY.await(2, TimeUnit.SECONDS), "the handler was interrupted");
    check(INTERRUPTS.get() == 1 && Boolean.TRUE.equals(SEEN_EXPIRED.get()), "once, and ctx.expired() is true: " + INTERRUPTS.get());
    Thread.sleep(100);
    check(ERRORS.isEmpty(), "a deadline is not a failure: " + ERRORS);

    // The key: released at the deadline, not touched by the late request.
    VeldResponse first = call(handler, "POST", "/rooms", "{\"name\":\"b\"}", "k1");
    check(first.status() == 504, "the first request with the key hit its deadline: " + first.status());
    VeldResponse retry = call(handler, "POST", "/rooms", "{\"name\":\"b\"}", "k1");
    check(retry.status() == 201, "the key was released at the deadline, the retry runs: " + retry.status() + retry.body());
    check(BUILDS.get() == 2, "the handler ran for the retry");
    LATE.countDown();
    check(LATE_DONE.await(2, TimeUnit.SECONDS), "the late request finished");
    Thread.sleep(150);
    VeldResponse replay = call(handler, "POST", "/rooms", "{\"name\":\"b\"}", "k1");
    check(replay.status() == 201 && BUILDS.get() == 2, "the late request did not release the retry's key: " + replay.status() + " builds " + BUILDS.get());
    check("true".equals(replay.headers().get("veld-idempotent-replay")), "the answer is the stored one: " + replay.headers());
    check(Json.parse(replay.body()).asObject().get("name").asString().equals("b2"), "of the retry, not of the late request: " + replay.body());
    check(ERRORS.isEmpty(), "still nothing in onError: " + ERRORS);
    System.out.println("OK");
  }
}
