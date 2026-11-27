package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.n.adapters.JdkHttpServerAdapter;
import gen.n.bk.bk.Party;
import gen.n.bk.bk.Room;
import gen.n.bk.bk.Tick;
import gen.n.bk.server.AuditFactLoaders;
import gen.n.bk.server.AuditHandler;
import gen.n.bk.server.AuditHooks;
import gen.n.bk.server.AuditService;
import gen.n.bk.server.BookFacts;
import gen.n.bk.server.BookRequest;
import gen.n.bk.server.TicksRequest;
import gen.n.runtime.RequestContext;
import gen.n.runtime.RuleWalk;
import gen.n.runtime.SseEvent;
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

/**
 * Audited rules ({@code @rollout(audit)}, spec/rules.md section 17.1) on a server generated with
 * {@code validate = false}: the audited rules that request validation switches off (the invariants of
 * the body, {@code require} and {@code ensure}) are off exactly where the enforced ones would be, and
 * the invariants of a result are still evaluated.
 */
public final class RolloutNoValidate {
  private RolloutNoValidate() {}

  private static final UUID ROOM = UUID.fromString("00000000-0000-4000-8000-000000000001");

  private static final class Loaders implements AuditFactLoaders {
    @Override
    public Optional<Room> room(UUID key, RequestContext ctx) {
      return Optional.of(new Room(ROOM, 2));
    }
  }

  private static final class Impl implements AuditService {
    @Override
    public Party book(BookRequest request, BookFacts facts, RequestContext ctx) {
      Party body = request.body();
      return new Party(body.roomId(), body.guests() == 5 ? 6 : body.guests());
    }

    @Override
    public java.util.stream.Stream<SseEvent<Tick>> ticks(TicksRequest request, RequestContext ctx) {
      return java.util.stream.Stream.empty();
    }
  }

  private static int book(HttpServer server, int guests) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + "/audit/book"))
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString("{\"roomId\":\"" + ROOM + "\",\"guests\":" + guests + "}"))
        .build();
    return HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString()).statusCode();
  }

  private static void same(Object expected, Object actual, String what) {
    if (!expected.equals(actual)) {
      throw new AssertionError(what + ": expected " + expected + " but was " + actual);
    }
  }

  private static List<String> heard(List<RuleWalk.Event> events) {
    List<String> out = new ArrayList<>();
    for (RuleWalk.Event e : events) {
      String rule = e.rule().replaceFirst("^veld:[^/]+/", "").replaceFirst("@.*$", "");
      out.add(e.outcome() + " " + e.position() + " " + rule + " " + e.violation().path());
    }
    return out;
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    HttpServer server = JdkHttpServerAdapter.listen(
        AuditHandler.create(new Impl(), new Loaders(), new AuditHooks().onRuleViolation(events::add)),
        new InetSocketAddress("127.0.0.1", 0));
    try {
      // Seven guests in a room for two: the audited invariant of the body and the audited require
      // would report, and with validation off they are not evaluated. The result is still checked.
      same(201, book(server, 7), "validation off");
      same(List.of("AUDITED OUTPUT Party.smallParty result"), heard(events), "the result only");
      events.clear();
      // Five guests come back as six: the audited ensure is off too.
      same(201, book(server, 5), "validation off");
      same(List.of(), heard(events), "no ensure without validation");
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }
}
