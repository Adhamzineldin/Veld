package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.r.adapters.JdkHttpServerAdapter;
import gen.r.bk.bk.Party;
import gen.r.bk.server.AuditFactLoaders;
import gen.r.bk.server.AuditHandler;
import gen.r.bk.server.AuditHooks;
import gen.r.bk.server.AuditService;
import gen.r.bk.server.BookFacts;
import gen.r.bk.server.BookRequest;
import gen.r.bk.server.TicksRequest;
import gen.r.bk.bk.Tick;
import gen.r.runtime.RequestContext;
import gen.r.runtime.RuleWalk;
import gen.r.runtime.SseEvent;
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
 * Audited rules ({@code @rollout(audit)}, spec/rules.md section 17.1) over HTTP on the JDK server: an
 * audited invariant, {@code require} (whose {@code else} payload would divide by zero) and
 * {@code ensure} are evaluated where the enforced ones would be, go to {@code onRuleViolation} with
 * the outcome {@code AUDITED}, and never reject, never answer and never show in a response.
 */
public final class RolloutHttp {
  private RolloutHttp() {}

  private static final UUID ROOM = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static int capacity = 100;

  private static final class Loaders implements AuditFactLoaders {
    @Override
    public Optional<gen.r.bk.bk.Room> room(UUID key, RequestContext ctx) {
      return Optional.of(new gen.r.bk.bk.Room(ROOM, capacity));
    }
  }

  /** Five guests come back as six, which breaks the audited {@code ensure}. */
  private static final class Impl implements AuditService {
    @Override
    public Party book(BookRequest request, BookFacts facts, RequestContext ctx) {
      Party body = request.body();
      return new Party(body.roomId(), body.guests() == 5 ? 6 : body.guests());
    }

    @Override
    public java.util.stream.Stream<SseEvent<Tick>> ticks(TicksRequest request, RequestContext ctx) {
      return java.util.stream.IntStream.range(0, request.count()).mapToObj(n -> SseEvent.of(new Tick(n)));
    }
  }

  private static HttpServer serve(AuditHooks hooks) throws Exception {
    return serve(hooks, RuleWalk.OutputMode.STRICT);
  }

  private static HttpServer serve(AuditHooks hooks, RuleWalk.OutputMode mode) throws Exception {
    return JdkHttpServerAdapter.listen(
        AuditHandler.create(
            new Impl(), new Loaders(), hooks, gen.r.runtimeconfig.GenRRuntime.builder().rulesOutput(mode).build()),
        new InetSocketAddress("127.0.0.1", 0));
  }

  private record Answer(int status, String body) {}

  private static Answer book(HttpServer server, int guests) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + "/audit/book"))
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString("{\"roomId\":\"" + ROOM + "\",\"guests\":" + guests + "}"))
        .build();
    HttpResponse<String> response = HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString());
    return new Answer(response.statusCode(), response.body());
  }

  /** What the hook heard, as {@code outcome position rule path}, in order. */
  private static List<String> heard(List<RuleWalk.Event> events) {
    List<String> out = new ArrayList<>();
    for (RuleWalk.Event e : events) {
      String rule = e.rule().replaceFirst("^veld:[^/]+/", "").replaceFirst("@.*$", "");
      out.add(e.outcome() + " " + e.position() + " " + rule + " " + e.violation().path());
    }
    return out;
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static void same(Object expected, Object actual, String what) {
    if (!expected.equals(actual)) {
      throw new AssertionError(what + ": expected " + expected + " but was " + actual);
    }
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    HttpServer strict = serve(new AuditHooks().onRuleViolation(events::add));
    try {
      // An audited invariant of the body and of the result: reported, and the request goes on.
      capacity = 100;
      Answer a = book(strict, 7);
      same(201, a.status(), "an audited invariant does not reject");
      check(!a.body().contains("violations") && !a.body().contains("smallParty"), "never in a response");
      same(
          List.of("AUDITED INPUT Party.smallParty body", "AUDITED OUTPUT Party.smallParty result"),
          heard(events),
          "the invariant of the body and of the result");

      // A client skips audited rules: the full check, the request check and the local check before a send.
      gen.c.bk.bk.Party draft = new gen.c.bk.bk.Party(ROOM, 7);
      same(List.of(), gen.c.bk.bk.Party.check(draft), "the full check skips an audited invariant");
      same(List.of(), gen.c.bk.bk.Party.checkRequest(draft), "the request check skips it too");
      events.clear();
      gen.c.bk.bk.Party sent = new gen.c.bk.client.AuditClient(
              gen.c.runtime.ClientOptions.builder()
                  .baseUrl("http://127.0.0.1:" + strict.getAddress().getPort())
                  .localRules(true)
                  .build())
          .book(new gen.c.bk.client.BookRequest(draft));
      same(draft, sent, "the local check does not block the send");
      same(2, events.size(), "the server reported the audited invariant of the body and of the result");

      // An audited `require` that fails is not answered: it has no `else` payload to build.
      events.clear();
      events.clear();
      capacity = 2;
      Answer b = book(strict, 3);
      same(201, b.status(), "an audited require does not answer");
      check(!b.body().contains("RoomFull"), "the else error is not sent");
      same(List.of("AUDITED INPUT Audit.book.require request"), heard(events), "the require");

      // An audited `ensure` that the result breaks leaves the outcome ok.
      events.clear();
      capacity = 100;
      Answer c = book(strict, 5);
      same(201, c.status(), "an audited ensure does not reject");
      same(List.of("AUDITED OUTPUT Audit.book.ensure request"), heard(events), "the ensure");

      // A rule that holds is not reported.
      events.clear();
      same(201, book(strict, 2).status(), "ok");
      same(List.of(), heard(events), "nothing to report");
    } finally {
      strict.stop(0);
    }

    // Output mode off skips an audited output rule like every output rule; an input rule is evaluated.
    List<RuleWalk.Event> offEvents = Collections.synchronizedList(new ArrayList<>());
    HttpServer off = serve(new AuditHooks().onRuleViolation(offEvents::add), RuleWalk.OutputMode.OFF);
    try {
      capacity = 100;
      same(201, book(off, 7).status(), "off");
      same(List.of("AUDITED INPUT Party.smallParty body"), heard(offEvents), "only the input rule");
      offEvents.clear();
      same(201, book(off, 5).status(), "off");
      same(List.of(), heard(offEvents), "no output rule in off");
    } finally {
      off.stop(0);
    }

    // `audit` evaluates and reports them too.
    List<RuleWalk.Event> auditEvents = Collections.synchronizedList(new ArrayList<>());
    HttpServer audit = serve(new AuditHooks().onRuleViolation(auditEvents::add), RuleWalk.OutputMode.AUDIT);
    try {
      same(201, book(audit, 5).status(), "audit");
      same(List.of("AUDITED OUTPUT Audit.book.ensure request"), heard(auditEvents), "audit mode");
    } finally {
      audit.stop(0);
    }

    // The items of an SSE stream are outputs: the odd ones break the audited invariant, are sent all the
    // same, and are reported.
    List<RuleWalk.Event> sseEvents = Collections.synchronizedList(new ArrayList<>());
    HttpServer sse = serve(new AuditHooks().onRuleViolation(sseEvents::add));
    try {
      HttpRequest request = HttpRequest.newBuilder(
              URI.create("http://127.0.0.1:" + sse.getAddress().getPort() + "/audit/ticks?count=4"))
          .GET()
          .build();
      HttpResponse<String> response = HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString());
      same(200, response.statusCode(), "the stream");
      same(4L, response.body().lines().filter(l -> l.startsWith("data:")).count(), "four items");
      check(!response.body().contains("event: error"), "the stream is not cut");
      same(
          List.of("AUDITED OUTPUT Tick.even result", "AUDITED OUTPUT Tick.even result"),
          heard(sseEvents),
          "the odd items");
    } finally {
      sse.stop(0);
    }

    // A hook that throws changes nothing.
    HttpServer boom = serve(new AuditHooks().onRuleViolation(e -> {
      throw new IllegalStateException("boom");
    }));
    try {
      capacity = 100;
      same(201, book(boom, 7).status(), "a failing hook");
    } finally {
      boom.stop(0);
    }
    System.out.println("OK");
  }
}
