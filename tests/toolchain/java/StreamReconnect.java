package veldtest;

import gen.sf.live.client.ChatRequest;
import gen.sf.live.client.LiveClient;
import gen.sf.live.client.RoomEventsRequest;
import gen.sf.live.live.Heard;
import gen.sf.live.live.NoSuchRoom;
import gen.sf.live.live.Say;
import gen.sf.live.live.Tick;
import gen.sf.runtime.ClientOptions;
import gen.sf.runtime.Json;
import gen.sf.runtime.Reconnect;
import gen.sf.runtime.SseOptions;
import gen.sf.runtime.VeldTransportError;
import gen.sf.runtime.WsChannel;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

/**
 * Reconnect with exponential backoff, from the Java client against the flaky Node server
 * (tests/toolchain/ts/flaky.ts), which drops connections on purpose: {@code StreamReconnect <base url>}.
 * Mirrors tests/toolchain/ts/stream_reconnect.tsx.
 */
public final class StreamReconnect {
  private StreamReconnect() {}

  private static final Reconnect FAST = new Reconnect(30, 100, 5);

  private static List<Json> seen(String base, String key) throws Exception {
    HttpResponse<String> res = HttpClient.newBuilder().version(HttpClient.Version.HTTP_1_1).build().send(
        HttpRequest.newBuilder(URI.create(base + "/__seen?key=" + URLEncoder.encode(key, StandardCharsets.UTF_8))).build(),
        HttpResponse.BodyHandlers.ofString());
    return Json.parse(res.body()).asArray().items();
  }

  private static void sleep(long ms) {
    try {
      Thread.sleep(ms);
    } catch (InterruptedException e) {
      throw new IllegalStateException(e);
    }
  }

  public static void main(String[] args) throws Exception {
    String base = args[0];
    LiveClient plain = new LiveClient(ClientOptions.builder().baseUrl(base).build());
    LiveClient live = new LiveClient(ClientOptions.builder().baseUrl(base).reconnect(FAST).build());

    // SSE, off by default: the broken stream is a transport error and nothing is retried.
    List<Integer> got = new ArrayList<>();
    Check.thrown(VeldTransportError.class, "a broken stream", () -> {
      try (Stream<Tick> s = plain.roomEvents(new RoomEventsRequest("resume-plain"))) {
        s.forEach(t -> got.add(t.n()));
      }
    });
    Check.equal(List.of(0, 1), got, "items before the break");
    Check.equal(1, seen(base, "resume-plain").size(), "no reconnect by default");

    // SSE with a policy: resumes with Last-Event-ID, one continuous stream.
    List<String> ids = new ArrayList<>();
    try (Stream<Tick> s = live.roomEvents(new RoomEventsRequest("resume-a"), SseOptions.none().withOnEventId(ids::add))) {
      Check.equal(List.of(0, 1, 2, 3), s.map(Tick::n).toList(), "continuous stream");
    }
    Check.equal(List.of("0", "1", "2", "3"), ids, "event ids");
    List<Json> conns = seen(base, "resume-a");
    Check.equal(2, conns.size(), "two connections");
    Check.that(conns.get(0).asObject().get("lastEventId") == null
        || conns.get(0).asObject().get("lastEventId") instanceof Json.Null, "the first had no Last-Event-ID");
    Check.equal("1", conns.get(1).asObject().get("lastEventId").asString(), "resumed after the last event");

    // A per-call policy on a client that has none.
    try (Stream<Tick> s = plain.roomEvents(new RoomEventsRequest("resume-b"), SseOptions.none().withReconnect(FAST))) {
      Check.equal(List.of(0, 1, 2, 3), s.map(Tick::n).toList(), "per-call policy");
    }

    // Backoff and exhaustion: the first attempt plus `retries`.
    LiveClient three = new LiveClient(ClientOptions.builder().baseUrl(base).reconnect(new Reconnect(30, 100, 3)).build());
    Check.thrown(VeldTransportError.class, "retries exhausted", () -> {
      try (Stream<Tick> s = three.roomEvents(new RoomEventsRequest("always-a"))) {
        s.forEach(t -> {});
      }
    });
    List<Json> always = seen(base, "always-a");
    Check.equal(4, always.size(), "first attempt and three retries");
    long t0 = (long) always.get(0).asObject().get("at").asDouble();
    long t3 = (long) always.get(3).asObject().get("at").asDouble();
    Check.that(t3 - t0 >= 30 + 60 + 100 - 15, "backoff 30, 60, 100: " + (t3 - t0));

    // A typed error is an answer: no retry.
    Check.thrown(NoSuchRoom.class, "typed error", () -> live.roomEvents(new RoomEventsRequest("missing-a")));
    Check.equal(1, seen(base, "missing-a").size(), "a typed error is never retried");

    // WebSocket, off by default.
    List<Integer> first = new ArrayList<>();
    Check.thrown(VeldTransportError.class, "abnormal close", () -> {
      try (WsChannel<Heard, Say> c = plain.chat(new ChatRequest("resume-plain", null))) {
        for (Heard h = c.receive(); h != null; h = c.receive()) {
          first.add(h.seq());
        }
      }
    });
    Check.equal(List.of(0), first, "frames before the drop");
    Check.equal(1, seen(base, "ws:resume-plain").size(), "no reconnect by default");

    // WebSocket on: reopened, the iteration continues.
    List<Integer> heard = new ArrayList<>();
    try (WsChannel<Heard, Say> c = live.chat(new ChatRequest("resume-a", null))) {
      for (Heard h = c.receive(); h != null; h = c.receive()) {
        heard.add(h.seq());
      }
    }
    Check.equal(List.of(0, 1, 2), heard, "continuous channel");
    Check.equal(2, seen(base, "ws:resume-a").size(), "two connections");

    // Sent while the connection is down: queued and delivered after the reconnect.
    LiveClient slow = new LiveClient(ClientOptions.builder().baseUrl(base).reconnect(new Reconnect(300, 300, 3)).build());
    try (WsChannel<Heard, Say> c = slow.chat(new ChatRequest("queue-a", null))) {
      while (seen(base, "ws:queue-a").isEmpty()) {
        sleep(5);
      }
      sleep(80);
      c.send(new Say("later", null));
      Heard h = c.receive();
      Check.equal("later", h.text(), "queued message echoed");
    }
    Check.equal(2, seen(base, "ws:queue-a").size(), "reconnected once");

    // Retries exhausted; a typed error frame is never retried.
    LiveClient two = new LiveClient(ClientOptions.builder().baseUrl(base).reconnect(new Reconnect(20, 40, 2)).build());
    try (WsChannel<Heard, Say> c = two.chat(new ChatRequest("always-a", null))) {
      Check.thrown(VeldTransportError.class, "exhausted", c::receive);
    }
    Check.equal(3, seen(base, "ws:always-a").size(), "first attempt and two retries");
    try (WsChannel<Heard, Say> c = live.chat(new ChatRequest("nosuch-a", null))) {
      Check.thrown(NoSuchRoom.class, "typed error frame", c::receive);
    }
    Check.equal(1, seen(base, "ws:nosuch-a").size(), "never retried");

    // close() during the backoff stops reconnecting.
    LiveClient wait = new LiveClient(ClientOptions.builder().baseUrl(base).reconnect(new Reconnect(250, 250, 9)).build());
    WsChannel<Heard, Say> closing = wait.chat(new ChatRequest("always-b", null));
    while (seen(base, "ws:always-b").isEmpty()) {
      sleep(5);
    }
    closing.close();
    sleep(500);
    Check.equal(1, seen(base, "ws:always-b").size(), "closed channels do not reconnect");

    System.out.println("java reconnect ok");
  }
}
