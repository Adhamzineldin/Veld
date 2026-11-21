package veldtest;

import gen.sf.live.client.AnnounceRequest;
import gen.sf.live.client.ChatRequest;
import gen.sf.live.client.LiveClient;
import gen.sf.live.client.RoomEventsRequest;
import gen.sf.live.client.SinkRequest;
import gen.sf.live.client.TicksRequest;
import gen.sf.live.client.VaultClient;
import gen.sf.live.live.Heard;
import gen.sf.live.live.NoSuchRoom;
import gen.sf.live.live.QuotaExceeded;
import gen.sf.live.live.Say;
import gen.sf.live.live.Tick;
import gen.sf.runtime.ClientOptions;
import gen.sf.runtime.SseOptions;
import gen.sf.runtime.VeldError;
import gen.sf.runtime.VeldTransportError;
import gen.sf.runtime.WsChannel;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.function.BooleanSupplier;
import java.util.stream.Stream;

/**
 * The streaming client scenario, run by the Java client against every server: the Java JDK
 * server, the Node server and the Spring server. Server-side observations need a {@link Probe};
 * without one those checks are skipped.
 */
final class StreamScenario {
  private StreamScenario() {}

  /** What the server did, when the test can see it. */
  interface Probe {
    int foreverClosed();

    int chatEnded();

    List<String> sink();
  }

  static void until(String label, BooleanSupplier condition) {
    for (int i = 0; i < 500; i++) {
      if (condition.getAsBoolean()) {
        return;
      }
      StreamImpl.sleep(10);
    }
    throw new AssertionError("timed out waiting for: " + label);
  }

  static List<Integer> ns(Stream<Tick> ticks) {
    try (ticks) {
      return ticks.map(Tick::n).toList();
    }
  }

  static void sse(String base, Probe probe) throws Exception {
    LiveClient live = new LiveClient(ClientOptions.builder().baseUrl(base).build());

    // Ordered items, decoded to the contract type.
    List<Tick> ticks;
    try (Stream<Tick> s = live.ticks(new TicksRequest(5, null))) {
      ticks = s.toList();
    }
    Check.equal(List.of(0, 1, 2, 3, 4), ticks.stream().map(Tick::n).toList(), "ordered items");
    Check.equal("tick 2", ticks.get(2).label(), "label");
    Check.equal(Instant.parse("2024-05-01T12:00:00Z"), ticks.get(0).when(), "datetime decoded");
    Check.equal(List.of(), ns(live.ticks(new TicksRequest(0, null))), "an empty stream");

    // A declared error before the first event is a normal problem response, thrown from the call.
    QuotaExceeded early = Check.thrown(QuotaExceeded.class, "typed error before the first event",
        () -> live.ticks(new TicksRequest(-1, null)));
    Check.equal(0, early.data().limit(), "early data");
    Check.equal(429, early.status(), "early status");
    Check.that(early.op() != null && early.op().contains("Live.ticks"), "op carries the operation: " + early.op());

    // After streaming has started the error is the final `event: error`, thrown when consumed.
    List<Integer> got = new ArrayList<>();
    QuotaExceeded late = Check.thrown(QuotaExceeded.class, "typed error mid-stream", () -> {
      try (Stream<Tick> s = live.ticks(new TicksRequest(10, 2))) {
        s.forEach(t -> got.add(t.n()));
      }
    });
    Check.equal(List.of(0, 1), got, "items before the error");
    Check.equal(2, late.data().limit(), "late data");
    Check.that(QuotaExceeded.matches(late), "matches");
    Check.that(!gen.sf.runtime.Veld.same(early, late), "different data is not the same error");

    // Closing the stream mid-way cancels the request: the server sees the client go away.
    List<Integer> firstThree = new ArrayList<>();
    try (Stream<Tick> s = live.forever()) {
      s.limit(3).forEach(t -> firstThree.add(t.n()));
    }
    Check.equal(List.of(0, 1, 2), firstThree, "forever items");
    if (probe != null) {
      until("the server notices the closed stream", () -> probe.foreverClosed() >= 1);
    }

    // Event names, ids and resuming with Last-Event-ID.
    List<String> ids = new ArrayList<>();
    List<Integer> first;
    try (Stream<Tick> s = live.roomEvents(new RoomEventsRequest("r"), SseOptions.none().withOnEventId(ids::add))) {
      first = s.map(Tick::n).toList();
    }
    Check.equal(List.of(0, 1, 2), first, "room events");
    Check.equal(List.of("0", "1", "2"), ids, "event ids");
    Check.equal(List.of(3, 4, 5), ns(live.roomEvents(new RoomEventsRequest("r"), SseOptions.resumeAfter("2"))), "resumed");
    NoSuchRoom missing = Check.thrown(NoSuchRoom.class, "typed error", () -> live.roomEvents(new RoomEventsRequest("nope")));
    Check.equal("nope", missing.data().room(), "room");

    wire(base);
  }

  private static HttpResponse<String> get(String url) throws Exception {
    return HttpClient.newBuilder().version(HttpClient.Version.HTTP_1_1).build().send(
        HttpRequest.newBuilder(URI.create(url)).build(), HttpResponse.BodyHandlers.ofString());
  }

  /** The bytes on the wire: framing, canonical JSON, ids and event names, the final error event. */
  private static void wire(String base) throws Exception {
    HttpResponse<String> res = get(base + "/live/ticks?count=2");
    Check.equal(200, res.statusCode(), "status");
    Check.that(res.headers().firstValue("content-type").orElse("").startsWith("text/event-stream"), "content type");
    Check.equal(
        "data: {\"label\":\"tick 0\",\"n\":0,\"when\":\"2024-05-01T12:00:00Z\"}\n\n"
            + "data: {\"label\":\"tick 1\",\"n\":1,\"when\":\"2024-05-01T12:00:00Z\"}\n\n",
        res.body(), "frames");
    String named = get(base + "/live/rooms/r/events").body();
    Check.that(named.startsWith("id: 0\nevent: tick\ndata: {\"label\""), named);
    String failed = get(base + "/live/ticks?count=3&failAt=1").body();
    Check.that(failed.matches("(?s)^data: .*\\n\\nevent: error\\ndata: \\{.*\"data\":\\{\"limit\":1}.*}\\n\\n$"), failed);
    HttpResponse<String> refused = get(base + "/live/ticks?count=-1");
    Check.equal(429, refused.statusCode(), "refused status");
    Check.equal("application/problem+json", refused.headers().firstValue("content-type").orElse(""), "problem type");
    Check.equal(422, get(base + "/live/ticks?count=x").statusCode(), "bad query");
  }

  static void auth(String base, boolean ws) throws Exception {
    VaultClient anonymous = new VaultClient(ClientOptions.builder().baseUrl(base).build());
    VeldError denied = Check.thrown(VeldError.class, "no token", () -> anonymous.feed());
    Check.equal("Unauthorized", denied.title(), "title");
    Check.equal(401, denied.status(), "status");
    VaultClient wrong = new VaultClient(ClientOptions.builder().baseUrl(base).bearer(() -> "bad").build());
    Check.equal("Unauthorized", Check.thrown(VeldError.class, "bad token", () -> wrong.feed()).title(), "bad token");
    VaultClient vault = new VaultClient(ClientOptions.builder().baseUrl(base).bearer(() -> "good").build());
    Check.equal(List.of(0, 1), ns(vault.feed()), "authenticated feed");
    if (!ws) {
      return;
    }

    // WebSocket auth: the same hook, a problem frame, close 1008; the token travels in the query.
    WsChannel<Heard, Say> rejected = anonymous.echo();
    rejected.send(new Say("hi", null));
    Check.equal("Unauthorized", Check.thrown(VeldError.class, "ws no token", rejected::receive).title(), "ws title");
    WsChannel<Heard, Say> bad = wrong.echo();
    Check.equal("Unauthorized", Check.thrown(VeldError.class, "ws bad token", bad::receive).title(), "ws bad token");
    try (WsChannel<Heard, Say> echo = vault.echo()) {
      echo.send(new Say("one", null));
      echo.send(new Say("two", null));
      Check.equal(new Heard("vault", "one", 0), echo.receive(), "first echo");
      Check.equal(new Heard("vault", "two", 1), echo.receive(), "second echo");
    }
    // The same hook also accepts the Authorization header (non-browser clients).
    RawWs raw = RawWs.open(base.replaceFirst("^http", "ws") + "/vault/echo", Map.of("authorization", "Bearer good"));
    raw.send("{\"text\":\"h\"}");
    Check.equal("{\"data\":{\"from\":\"vault\",\"seq\":0,\"text\":\"h\"}}", raw.frame(), "raw echo");
    raw.close();
  }

  /**
   * @param closes4404 whether a path that is not a ws endpoint is accepted and closed with 4404 (the
   *     Node adapter), or fails the handshake with a 404 (Spring: there is no socket to close)
   */
  static void ws(String base, Probe probe, boolean closes4404) throws Exception {
    LiveClient live = new LiveClient(ClientOptions.builder().baseUrl(base).build());

    // Bidirectional chat: replies are typed.
    int before = probe == null ? 0 : probe.chatEnded();
    try (WsChannel<Heard, Say> chat = live.chat(new ChatRequest("r1", "ada"))) {
      chat.send(new Say("hi", null));
      chat.send(new Say("yo", true));
      Check.equal(new Heard("ada", "hi", 0), chat.receive(), "first reply");
      Check.equal(new Heard("ada", "YO", 1), chat.receive(), "second reply");
    }
    if (probe != null) {
      until("the server sees the chat end", () -> probe.chatEnded() > before);
    }

    // The client validates outgoing messages.
    try (WsChannel<Heard, Say> guarded = live.chat(new ChatRequest("r2", null))) {
      VeldError invalid = Check.thrown(VeldError.class, "local validation", () -> guarded.send(new Say("", null)));
      Check.equal("ValidationError", invalid.title(), "local title");
    }

    // The server validates incoming messages: a bad one comes back as a typed ValidationError.
    LiveClient lax = new LiveClient(ClientOptions.builder().baseUrl(base).validate(false).build());
    try (WsChannel<Heard, Say> bad = lax.chat(new ChatRequest("r3", null))) {
      bad.send(new Say("", null));
      VeldError failure = Check.thrown(VeldError.class, "server validation", bad::receive);
      Check.equal("ValidationError", failure.title(), "remote title");
      Check.equal(422, failure.status(), "remote status");
      Check.that(failure.dataWire().write().contains("\"path\":\"message"), failure.dataWire().write());
    }

    // A declared error closes the socket after a problem frame, typed on the client.
    try (WsChannel<Heard, Say> locked = live.chat(new ChatRequest("locked", null))) {
      NoSuchRoom err = Check.thrown(NoSuchRoom.class, "declared ws error", locked::receive);
      Check.that(err.op() != null && err.op().contains("Live.chat"), "op: " + err.op());
      Check.equal("locked", err.data().room(), "room");
    }

    // Server-only and client-only endpoints.
    List<Integer> announced = new ArrayList<>();
    try (WsChannel<Tick, Void> announce = live.announce(new AnnounceRequest())) {
      for (Tick t = announce.receive(); t != null; t = announce.receive()) {
        announced.add(t.n());
      }
    }
    Check.equal(List.of(0, 1, 2), announced, "announced");
    try (WsChannel<Void, Say> sink = live.sink(new SinkRequest())) {
      sink.send(new Say("a", null));
      sink.send(new Say("b", null));
      Check.equal(null, sink.receive(), "the server ends the sink normally");
    }
    if (probe != null) {
      Check.equal(List.of("a", "b"), probe.sink(), "the sink saw both");
    }

    raw(base, closes4404);

    // Unreachable server: a transport error, not a hang.
    LiveClient dead = new LiveClient(ClientOptions.builder().baseUrl("http://127.0.0.1:1").build());
    Check.thrown(VeldTransportError.class, "unreachable", () -> dead.announce());
  }

  /** Raw frames and close codes: envelopes of canonical JSON, 4404, 1008. */
  private static void raw(String base, boolean closes4404) {
    String ws = base.replaceFirst("^http", "ws");
    if (closes4404) {
      Check.equal(4404, RawWs.open(ws + "/nowhere", Map.of()).closeCode(), "not a ws endpoint");
    } else {
      Throwable refused = Check.thrown(java.util.concurrent.CompletionException.class, "not a ws endpoint",
          () -> RawWs.open(ws + "/nowhere", Map.of())).getCause();
      Check.that(refused instanceof java.net.http.WebSocketHandshakeException, "handshake refused: " + refused);
    }
    RawWs raw = RawWs.open(ws + "/live/chat/raw?nick=zed", Map.of());
    raw.send("{\"loud\":false,\"text\":\"a\"}");
    Check.equal("{\"data\":{\"from\":\"zed\",\"seq\":0,\"text\":\"a\"}}", raw.frame(), "envelope");
    raw.send("not json");
    String problem = raw.frame();
    Check.that(problem.matches("^\\{\"error\":\\{.*\"title\":\"ValidationError\".*\"status\":422.*"), problem);
    Check.equal(1008, raw.closeCode(), "close code of a client-side error");
  }
}
