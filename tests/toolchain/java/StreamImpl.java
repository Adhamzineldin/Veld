package veldtest;

import gen.s.live.live.NoSuchRoom;
import gen.s.live.live.QuotaExceeded;
import gen.s.live.live.Tick;
import gen.s.live.server.ForeverRequest;
import gen.s.live.server.FeedRequest;
import gen.s.live.server.LiveHandler;
import gen.s.live.server.LiveHooks;
import gen.s.live.server.LiveService;
import gen.s.live.server.RoomEventsRequest;
import gen.s.live.server.TicksRequest;
import gen.s.live.server.VaultHandler;
import gen.s.live.server.VaultHooks;
import gen.s.live.server.VaultService;
import gen.s.runtime.RequestContext;
import gen.s.runtime.SseEvent;
import gen.s.runtime.VeldHandler;
import gen.s.runtime.VeldRequest;
import gen.s.runtime.VeldResponse;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.IntStream;
import java.util.stream.Stream;

/**
 * The Java implementations of the streaming contract's SSE endpoints (the JDK server cannot serve
 * the {@code ws} ones), and what the tests observe about them.
 */
final class StreamImpl {
  private StreamImpl() {}

  static final AtomicInteger FOREVER_CLOSED = new AtomicInteger();
  static final AtomicInteger ROOM_EVENTS_CLOSED = new AtomicInteger();
  static final List<Object> PRINCIPALS = new CopyOnWriteArrayList<>();

  static Tick tick(int n) {
    return new Tick(n, "tick " + n, Instant.parse("2024-05-01T12:00:00Z"));
  }

  static final class Live implements LiveService {
    @Override
    public Stream<SseEvent<Tick>> ticks(TicksRequest req, RequestContext ctx) {
      if (req.count() < 0) {
        throw new QuotaExceeded(new QuotaExceeded.Data(0));
      }
      return IntStream.range(0, req.count()).mapToObj(n -> {
        if (req.failAt() != null && req.failAt() == n) {
          throw new QuotaExceeded(new QuotaExceeded.Data(n));
        }
        return SseEvent.of(tick(n));
      });
    }

    @Override
    public Stream<SseEvent<Tick>> forever(ForeverRequest req, RequestContext ctx) {
      return Stream.iterate(0, n -> n + 1)
          .map(n -> {
            if (n > 0) {
              sleep(20);
            }
            return SseEvent.of(tick(n));
          })
          .onClose(FOREVER_CLOSED::incrementAndGet);
    }

    @Override
    public Stream<SseEvent<Tick>> roomEvents(RoomEventsRequest req, RequestContext ctx) {
      if (req.room().equals("nope")) {
        throw new NoSuchRoom(new NoSuchRoom.Data(req.room()));
      }
      String last = ctx.header("last-event-id");
      int from = last == null ? 0 : Integer.parseInt(last) + 1;
      return IntStream.range(from, from + 3)
          .mapToObj(n -> SseEvent.of(tick(n), "tick", String.valueOf(n)))
          .onClose(ROOM_EVENTS_CLOSED::incrementAndGet);
    }
  }

  static final class Vault implements VaultService {
    @Override
    public Stream<SseEvent<Tick>> feed(FeedRequest req, RequestContext ctx) {
      PRINCIPALS.add(ctx.principal());
      return Stream.of(SseEvent.of(tick(0)), SseEvent.of(tick(1)));
    }
  }

  static void sleep(long millis) {
    try {
      Thread.sleep(millis);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
    }
  }

  /** Both services on one handler, like the Node test server (SSE only: the JDK server has no ws). */
  static VeldHandler handler() {
    VeldHandler live = LiveHandler.create(new Live(), new LiveHooks());
    VeldHandler vault = VaultHandler.create(
        new Vault(), new VaultHooks((token, ctx) -> "good".equals(token) ? Map.of("user", "u1") : null));
    return new VeldHandler() {
      @Override
      public Optional<VeldResponse> handle(VeldRequest request) {
        Optional<VeldResponse> response = live.handle(request);
        return response.isPresent() ? response : vault.handle(request);
      }
    };
  }
}
