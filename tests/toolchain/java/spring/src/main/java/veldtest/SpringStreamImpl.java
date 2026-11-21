package veldtest;

import gen.w.live.live.Heard;
import gen.w.live.live.NoSuchRoom;
import gen.w.live.live.QuotaExceeded;
import gen.w.live.live.Say;
import gen.w.live.live.Tick;
import gen.w.live.server.AnnounceRequest;
import gen.w.live.server.ChatRequest;
import gen.w.live.server.EchoRequest;
import gen.w.live.server.FeedRequest;
import gen.w.live.server.ForeverRequest;
import gen.w.live.server.LiveService;
import gen.w.live.server.RoomEventsRequest;
import gen.w.live.server.SinkRequest;
import gen.w.live.server.TicksRequest;
import gen.w.live.server.VaultService;
import gen.w.runtime.RequestContext;
import gen.w.runtime.SseEvent;
import gen.w.runtime.WsSession;
import java.time.Instant;
import java.util.Iterator;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.IntStream;
import java.util.stream.Stream;

/**
 * The Java implementations of the whole streaming contract behind the Spring adapter, and the
 * counters {@link ProbeController} shows the test client (what the server saw).
 */
final class SpringStreamImpl {
  private SpringStreamImpl() {}

  static final AtomicInteger FOREVER_CLOSED = new AtomicInteger();
  static final AtomicInteger CHAT_ENDED = new AtomicInteger();
  static final List<String> SINK = new CopyOnWriteArrayList<>();

  static Tick tick(int n) {
    return new Tick(n, "tick " + n, Instant.parse("2024-05-01T12:00:00Z"));
  }

  static void sleep(long millis) {
    try {
      Thread.sleep(millis);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
    }
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
          .mapToObj(n -> SseEvent.of(tick(n), "tick", String.valueOf(n)));
    }

    @Override
    public void chat(ChatRequest req, RequestContext ctx, WsSession<Say, Heard> ws) {
      if (req.room().equals("locked")) {
        throw new NoSuchRoom(new NoSuchRoom.Data(req.room()));
      }
      int seq = 0;
      try {
        Iterator<Say> messages = ws.incoming().iterator();
        while (messages.hasNext()) {
          Say m = messages.next();
          String text = Boolean.TRUE.equals(m.loud()) ? m.text().toUpperCase() : m.text();
          ws.send(new Heard(req.nick() == null ? "anon" : req.nick(), text, seq++));
        }
      } finally {
        CHAT_ENDED.incrementAndGet();
      }
    }

    @Override
    public void announce(AnnounceRequest req, RequestContext ctx, WsSession<Void, Tick> ws) {
      for (int n = 0; n < 3; n++) {
        ws.send(tick(n));
      }
    }

    @Override
    public void sink(SinkRequest req, RequestContext ctx, WsSession<Say, Void> ws) {
      Iterator<Say> messages = ws.incoming().iterator();
      while (messages.hasNext()) {
        SINK.add(messages.next().text());
        if (SINK.size() % 2 == 0) {
          return;
        }
      }
    }
  }

  static final class Vault implements VaultService {
    @Override
    public Stream<SseEvent<Tick>> feed(FeedRequest req, RequestContext ctx) {
      return Stream.of(SseEvent.of(tick(0)), SseEvent.of(tick(1)));
    }

    @Override
    public void echo(EchoRequest req, RequestContext ctx, WsSession<Say, Heard> ws) {
      int seq = 0;
      Iterator<Say> messages = ws.incoming().iterator();
      while (messages.hasNext()) {
        ws.send(new Heard("vault", messages.next().text(), seq++));
      }
    }
  }
}
