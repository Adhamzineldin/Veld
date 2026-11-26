package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.align.align.Caller;
import gen.b.align.align.Crate;
import gen.b.align.align.Part;
import gen.b.align.align.Seen;
import gen.b.align.server.DepotHandler;
import gen.b.align.server.DepotHooks;
import gen.b.align.server.DepotService;
import gen.b.align.server.SeenRequest;
import gen.b.align.server.ShipRequest;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Derived fields that have no value (spec/rules.md section 17.2, "Where the three generators
 * differed"): a derived field that reaches a {@code @readonly} field through a nested value or a
 * lambda is absent in a request, with no invented value and no violation; an {@code allow}, a
 * {@code require}, an {@code old(...)} and their audited forms that read it are skipped (no
 * {@code NullPointerException}); and the principal is completed once, after authentication, for
 * every endpoint, including one without a rule block, and a principal that cannot be completed is
 * {@code 500 Internal} with the violation reported to the rule hook. Prints {@code OK}.
 */
public final class DerivedAlign {
  private DerivedAlign() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000a1");
  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static final AtomicReference<ShipRequest> SHIPPED = new AtomicReference<>();
  private static final AtomicReference<Caller> SEEN = new AtomicReference<>();
  private static final AtomicInteger CALLS = new AtomicInteger();
  private static final List<RuleWalk.Event> EVENTS = new CopyOnWriteArrayList<>();

  private static final class Impl implements DepotService {
    @Override
    public Crate ship(ShipRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      SHIPPED.set(request);
      Part one = new Part(ID, 3, "one");
      return new Crate(List.of(one), one, null, null, null);
    }

    @Override
    public Seen seen(SeenRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      SEEN.set((Caller) ctx.principal());
      return new Seen(request.n(), ((Caller) ctx.principal()).cap());
    }
  }

  private record Answer(int status, String text) {
    Json.Obj body() {
      return Json.parse(text).asObject();
    }
  }

  private static Answer send(HttpServer server, String method, String path, int level, String body) throws Exception {
    HttpRequest.Builder request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path))
        .header("Authorization", "Bearer " + level)
        .header("content-type", "application/json");
    request.method(method, body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
    HttpResponse<String> response = HTTP.send(request.build(), HttpResponse.BodyHandlers.ofString());
    return new Answer(response.statusCode(), response.body());
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static final String CRATE = "{\"parts\":[{\"name\":\"a\"}],\"lead\":{\"name\":\"b\"}}";

  public static void main(String[] args) throws Exception {
    // The hook puts a derived value of its own in the caller: it is replaced, for every endpoint.
    DepotHooks hooks = new DepotHooks((token, ctx) -> new Caller(ID, Integer.parseInt(token), 12345))
        .onRuleViolation(EVENTS::add);
    HttpServer server = JdkHttpServerAdapter.listen(
        DepotHandler.create(
            new Impl(), hooks, gen.b.runtimeconfig.GenBRuntime.builder().rulesOutput(RuleWalk.OutputMode.STRICT).build()),
        new InetSocketAddress("127.0.0.1", 0));
    try {
      aDerivedFieldThatReachesAReadonlyFieldIsAbsentInARequest(server);
      statementsThatReadAnAbsentDerivedFieldAreSkipped(server);
      thePrincipalIsCompletedForEveryEndpoint(server);
      aPrincipalThatCannotBeCompletedIsA500(server);
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  /** `count` reads no read-only field and is there; `heavy` (a lambda) and `leadWeight` (a nested path) are absent. */
  private static void aDerivedFieldThatReachesAReadonlyFieldIsAbsentInARequest(HttpServer server) throws Exception {
    Answer fine = send(server, "POST", "/depot/ship", 5, CRATE);
    check(fine.status() == 200, "the request is answered: " + fine.status() + fine.text());
    Crate seen = SHIPPED.get().body();
    check(seen.count() != null && seen.count() == 1, "a field that reads no read-only field is computed: " + seen);
    check(seen.heavy() == null, "a field that reads one through a lambda is absent, not invented: " + seen);
    check(seen.leadWeight() == null, "a field that reads one through a nested value is absent: " + seen);
    check(fine.body().get("leadWeight").asDouble() == 3, "the result's own value is computed: " + fine.text());
    check(!fine.body().get("heavy").write().equals("null"), "and so is the result's heavy: " + fine.text());
  }

  /**
   * `allow CanShip(user, body)` (the policy reads `crate.leadWeight`), `require not body.heavy` and
   * `ensure result.count == old(body.leadWeight)`, enforced and audited: every one is skipped, so the
   * answer is the handler's, and nothing is reported.
   */
  private static void statementsThatReadAnAbsentDerivedFieldAreSkipped(HttpServer server) throws Exception {
    EVENTS.clear();
    Answer fine = send(server, "POST", "/depot/ship", 5, CRATE);
    check(fine.status() == 200, "no statement fails on an absent derived field: " + fine.status() + fine.text());
    check(EVENTS.isEmpty(), "and none is reported: " + EVENTS);
  }

  /** The handler of an endpoint with no rule block sees the caller with its computed cap, not the hook's 12345. */
  private static void thePrincipalIsCompletedForEveryEndpoint(HttpServer server) throws Exception {
    Answer seen = send(server, "GET", "/depot/seen?n=3", 5, null);
    check(seen.status() == 200, "an endpoint without rules: " + seen.status() + seen.text());
    check(seen.body().get("cap").asDouble() == 50, "the handler read the computed cap: " + seen.text());
    check(SEEN.get().cap() == 50, "ctx.principal() is the completed value: " + SEEN.get());
  }

  /** A level of 2 000 000 000 overflows `level * 10`: `500 Internal`, the handler is not called, the hook is told. */
  private static void aPrincipalThatCannotBeCompletedIsA500(HttpServer server) throws Exception {
    EVENTS.clear();
    int before = CALLS.get();
    for (String[] call : new String[][] {{"GET", "/depot/seen?n=3", null}, {"POST", "/depot/ship", CRATE}}) {
      Answer broken = send(server, call[0], call[1], 2_000_000_000, call[2]);
      check(broken.status() == 500, call[1] + " is 500: " + broken.status() + broken.text());
      check(!broken.text().contains("overflow"), "without detail: " + broken.text());
    }
    check(CALLS.get() == before, "the handler is not called");
    check(EVENTS.size() == 2, "the violation is reported once per request: " + EVENTS);
    for (RuleWalk.Event event : EVENTS) {
      check(event.position() == RuleWalk.Position.OUTPUT && event.outcome() == RuleWalk.Outcome.REJECTED, "as an output violation: " + event);
      check(event.rule().equals(Caller.DERIVED_CAP), "naming the field: " + event);
    }
  }
}
