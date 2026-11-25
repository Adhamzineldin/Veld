package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.rc.bk.bk.Booking;
import gen.rc.bk.client.BookingsClient;
import gen.rc.bk.client.CreateRequest;
import gen.rc.runtime.ClientOptions;
import gen.rc.runtime.VeldError;
import gen.rc.runtime.VeldHttpError;
import gen.rc.runtime.Violation;
import gen.rs.bk.server.BadRequest;
import gen.rs.bk.server.BookingsHandler;
import gen.rs.bk.server.BookingsHooks;
import gen.rs.bk.server.BookingsService;
import gen.rs.runtime.RequestContext;
import gen.rs.runtime.RuleWalk;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

/** Invariants over HTTP: the JDK server, the generated client with the local check on and off. */
public final class RuleRoundtrip {
  private RuleRoundtrip() {}

  private static final Instant T0 = Instant.parse("2026-01-02T00:00:00Z");
  private static final Instant T1 = Instant.parse("2026-01-01T00:00:00Z");

  private static final AtomicInteger CREATED = new AtomicInteger();

  private static final class Impl implements BookingsService {
    @Override
    public gen.rs.bk.bk.Booking create(gen.rs.bk.server.CreateRequest request, RequestContext ctx) {
      CREATED.incrementAndGet();
      return request.body();
    }

    @Override
    public gen.rs.bk.bk.Booking bad(BadRequest request, RequestContext ctx) {
      return new gen.rs.bk.bk.Booking(T0, T1);
    }

    @Override
    public void search(gen.rs.bk.server.SearchRequest request, RequestContext ctx) {
      CREATED.incrementAndGet();
    }

    @Override
    public void overOk(gen.rs.bk.server.OverOkRequest request, RequestContext ctx) {
      throw new gen.rs.bk.bk.Overbooked(new gen.rs.bk.bk.Overbooked.Data(2, 5));
    }

    @Override
    public void overBad(gen.rs.bk.server.OverBadRequest request, RequestContext ctx) {
      throw new gen.rs.bk.bk.Overbooked(new gen.rs.bk.bk.Overbooked.Data(9, 5));
    }
  }

  private static HttpServer serve(RuleWalk.OutputMode mode, List<RuleWalk.Event> events) throws Exception {
    BookingsHooks hooks = new BookingsHooks().rulesOutput(mode).onRuleViolation(events::add);
    return gen.rs.adapters.JdkHttpServerAdapter.listen(
        BookingsHandler.create(new Impl(), hooks), new InetSocketAddress("127.0.0.1", 0));
  }

  private static String url(HttpServer server) {
    return "http://127.0.0.1:" + server.getAddress().getPort();
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    HttpServer strict = serve(RuleWalk.OutputMode.STRICT, events);
    List<RuleWalk.Event> auditEvents = Collections.synchronizedList(new ArrayList<>());
    HttpServer audit = serve(RuleWalk.OutputMode.AUDIT, auditEvents);
    try {
      rawProblem(url(strict));
      Violation remote = remoteViolation(url(strict));
      localViolation(url(strict), remote);
      validPasses(url(strict));
      outputStrict(url(strict), events);
      outputAudit(url(audit), auditEvents);
      errors(url(strict), events, url(audit), auditEvents);
      throughInvoke();
      queryObject(url(strict));
      System.out.println("OK");
    } finally {
      strict.stop(0);
      audit.stop(0);
    }
  }

  private static void rawProblem(String base) throws Exception {
    HttpRequest req = HttpRequest.newBuilder(URI.create(base + "/bookings/"))
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(
            "{\"start\":\"2026-01-02T00:00:00Z\",\"end\":\"2026-01-01T00:00:00Z\"}"))
        .build();
    HttpResponse<String> res = HttpClient.newHttpClient().send(req, HttpResponse.BodyHandlers.ofString());
    Check.equal(422, res.statusCode(), "a bad body is 422");
    gen.rs.runtime.Json doc = gen.rs.runtime.Json.parse(res.body());
    gen.rs.runtime.Json.Obj o = doc.asObject();
    Check.equal("ValidationError", o.get("title").asString(), "title");
    var issues = o.get("data").asObject().get("issues").asArray().items();
    Check.equal(1, issues.size(), "one issue per violation");
    Check.equal("rule", issues.get(0).asObject().get("code").asString(), "issue code");
    Check.equal("body", issues.get(0).asObject().get("path").asString(), "issue path");
    var violations = o.get("violations").asArray().items();
    Check.equal(1, violations.size(), "one violation");
    gen.rs.runtime.Json.Obj v = violations.get(0).asObject();
    Check.that(v.get("rule").asString().startsWith("veld:bk/Booking.endAfterStart@"), "rule id " + v.get("rule"));
    Check.equal("body", v.get("path").asString(), "violation path");
    Check.equal("[\"end\",\"start\"]", v.get("fields").write(), "fields");
    Check.equal("A booking must end after it starts", v.get("message").asString(), "message");
    Check.that(v.get("error") == null, "no error member");
  }

  private static BookingsClient client(String base, boolean local) {
    return new BookingsClient(ClientOptions.builder().baseUrl(base).localRules(local).build());
  }

  private static Violation remoteViolation(String base) {
    int before = CREATED.get();
    VeldError e = Check.thrown(VeldError.class, "the server rejects", () ->
        client(base, false).create(new CreateRequest(new Booking(T0, T1))));
    Check.equal("ValidationError", e.title(), "typed ValidationError");
    Check.that(!e.raisedLocally(), "raised by the server");
    Check.equal(1, e.violations().size(), "violations decoded");
    Check.equal(before, CREATED.get(), "the implementation did not run");
    Violation v = e.violations().get(0);
    Check.equal("body", v.path(), "path");
    Check.equal(List.of("end", "start"), v.fields(), "fields");
    return v;
  }

  /**
   * A client that has seen one response: the rules digest it carries is the client's own, so the
   * local check is on (spec/rules.md section 9.7: before the first response a request is sent anyway).
   */
  private static BookingsClient warmClient(String base) {
    BookingsClient c = client(base, true);
    Booking ok = new Booking(T1, T0);
    Check.equal(ok, c.create(new CreateRequest(ok)), "the first call goes through");
    return c;
  }

  private static void localViolation(String base, Violation remote) {
    BookingsClient warm = warmClient(base);
    int before = CREATED.get();
    VeldError e = Check.thrown(VeldError.class, "the client refuses", () ->
        warm.create(new CreateRequest(new Booking(T0, T1))));
    Check.equal("ValidationError", e.title(), "typed ValidationError");
    Check.that(e.raisedLocally(), "raised locally");
    Check.equal(List.of(remote), e.violations(), "the same violations as the server");
    Check.equal(before, CREATED.get(), "nothing was sent");
  }

  private static void validPasses(String base) {
    Booking ok = new Booking(T1, T0);
    Check.equal(ok, client(base, true).create(new CreateRequest(ok)), "a valid booking round-trips");
  }

  private static void outputStrict(String base, List<RuleWalk.Event> events) {
    Throwable t = Check.thrown(RuntimeException.class, "strict output is a 500", () ->
        client(base, true).bad(new gen.rc.bk.client.BadRequest()));
    int status = t instanceof VeldError ve ? ve.status() : t instanceof VeldHttpError he ? he.status() : -1;
    Check.equal(500, status, "500");
    Check.that(!(t instanceof VeldError ve) || ve.violations().isEmpty(), "no rule detail for the caller");
    Check.equal(3, events.size(), "two rejected inputs, then one output event");
    Check.equal(RuleWalk.Position.INPUT, events.get(0).position(), "input position");
    Check.equal(RuleWalk.Outcome.REJECTED, events.get(0).outcome(), "input outcome");
    RuleWalk.Event ev = events.get(2);
    Check.equal(RuleWalk.Position.OUTPUT, ev.position(), "position");
    Check.equal(RuleWalk.Outcome.REJECTED, ev.outcome(), "outcome");
    Check.that(ev.operation().startsWith("veld:bk/Bookings.bad@"), "operation " + ev.operation());
    Check.that(ev.rule().startsWith("veld:bk/Booking.endAfterStart@"), "rule");
    Check.that(ev.elapsedMicros() >= 0, "elapsed");
  }

  /** The rules of an error declaration are checked when the server encodes it. */
  private static void errors(String strict, List<RuleWalk.Event> events, String audit, List<RuleWalk.Event> auditEvents) {
    gen.rc.bk.bk.Overbooked ok = Check.thrown(gen.rc.bk.bk.Overbooked.class, "a valid error is sent as declared", () ->
        client(strict, true).overOk(new gen.rc.bk.client.OverOkRequest()));
    Check.equal(new gen.rc.bk.bk.Overbooked.Data(2, 5), ok.data(), "its data");
    Check.equal(409, ok.status(), "its status");
    Check.equal(3, events.size(), "a valid error reports nothing");
    Throwable t = Check.thrown(RuntimeException.class, "an error that breaks its rule is a 500 in strict", () ->
        client(strict, true).overBad(new gen.rc.bk.client.OverBadRequest()));
    int status = t instanceof VeldError ve ? ve.status() : t instanceof VeldHttpError he ? he.status() : -1;
    Check.equal(500, status, "500");
    Check.equal(4, events.size(), "the violation was reported");
    Check.that(events.get(3).rule().startsWith("veld:bk/Overbooked.within@"), "rule " + events.get(3).rule());
    Check.equal(RuleWalk.Position.OUTPUT, events.get(3).position(), "output");
    gen.rc.bk.bk.Overbooked sent = Check.thrown(gen.rc.bk.bk.Overbooked.class, "audit sends the error", () ->
        client(audit, true).overBad(new gen.rc.bk.client.OverBadRequest()));
    Check.equal(new gen.rc.bk.bk.Overbooked.Data(9, 5), sent.data(), "audited data");
    Check.equal(2, auditEvents.size(), "audit reported it");
    Check.equal(RuleWalk.Outcome.AUDITED, auditEvents.get(1).outcome(), "audited");
  }

  /** The rules of a {@code query} object: path {@code query}, locally and on the server. */
  private static void queryObject(String base) {
    BookingsClient warm = warmClient(base);
    int before = CREATED.get();
    gen.rc.bk.client.SearchRequest bad = new gen.rc.bk.client.SearchRequest(new gen.rc.bk.bk.Window(5, 1));
    VeldError local = Check.thrown(VeldError.class, "the client refuses a bad query", () -> warm.search(bad));
    Check.that(local.raisedLocally(), "local");
    Check.equal("query", local.violations().get(0).path(), "local path");
    Check.equal(List.of("lo", "hi"), local.violations().get(0).fields(), "fields");
    Check.equal(before, CREATED.get(), "nothing was sent");
    VeldError remote = Check.thrown(VeldError.class, "the server rejects a bad query", () -> client(base, false).search(bad));
    Check.that(!remote.raisedLocally(), "remote");
    Check.equal(local.violations(), remote.violations(), "the same violations");
    Check.equal(before, CREATED.get(), "the implementation did not run");
    client(base, true).search(new gen.rc.bk.client.SearchRequest(new gen.rc.bk.bk.Window(1, 5)));
    Check.equal(before + 1, CREATED.get(), "a good query is served");
  }

  /** The entry the Spring adapter uses: {@code VeldHandler.invoke} runs the same pipeline. */
  private static void throughInvoke() {
    gen.rs.runtime.VeldHandler handler = BookingsHandler.create(new Impl(), new BookingsHooks());
    var request = new gen.rs.runtime.VeldRequest(
        "POST", "/bookings/", null, java.util.Map.of("content-type", List.of("application/json")),
        "{\"start\":\"2026-01-02T00:00:00Z\",\"end\":\"2026-01-01T00:00:00Z\"}", null);
    var response = handler.invoke("create", request).orElseThrow();
    Check.equal(422, response.status(), "invoke answers 422");
    Check.that(response.body().contains("\"violations\":[{\"rule\":\"veld:bk/Booking.endAfterStart@"), response.body());
  }

  private static void outputAudit(String base, List<RuleWalk.Event> events) {
    Booking got = client(base, true).bad(new gen.rc.bk.client.BadRequest());
    Check.equal(new Booking(T0, T1), got, "audit sends the response unchanged");
    Check.equal(1, events.size(), "one hook event");
    Check.equal(RuleWalk.Outcome.AUDITED, events.get(0).outcome(), "audited");
  }
}
