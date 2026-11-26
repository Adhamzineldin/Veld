package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.extra.extra.Caller;
import gen.b.extra.extra.Echo;
import gen.b.extra.extra.Tick;
import gen.b.extra.server.EchoRequest;
import gen.b.extra.server.ProbeHandler;
import gen.b.extra.server.ProbeHooks;
import gen.b.extra.server.ProbeService;
import gen.b.extra.server.TicksRequest;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.SseEvent;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;
import java.util.stream.IntStream;
import java.util.stream.Stream;

/**
 * Derived fields where the pipeline of {@code tests/rules/endpoints} does not reach them (spec/rules.md
 * section 17.2): the principal is given its derived field before a {@code require} reads it (the value
 * the authentication hook put there is replaced), the value of a result is computed from the handler's
 * and is checked by an invariant under the output mode ({@code strict} is 500, {@code off} still
 * computes), and every item of a stream is computed before it is sent.
 */
public final class DerivedExtraHttp {
  private DerivedExtraHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000f1");
  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static final AtomicReference<Caller> SEEN = new AtomicReference<>();

  private static final class Impl implements ProbeService {
    @Override
    public Echo echo(EchoRequest request, RequestContext ctx) {
      SEEN.set((Caller) ctx.principal());
      return new Echo(request.n(), 12345);
    }

    @Override
    public Stream<SseEvent<Tick>> ticks(TicksRequest request, RequestContext ctx) {
      return IntStream.rangeClosed(1, request.count()).mapToObj(i -> SseEvent.of(new Tick(i, 777)));
    }
  }

  private record Answer(int status, String text) {
    Json.Obj body() {
      return Json.parse(text).asObject();
    }
  }

  private static HttpServer serve(RuleWalk.OutputMode mode) throws Exception {
    // The hook puts a derived value of its own in the caller: it is replaced.
    ProbeHooks hooks = new ProbeHooks((token, ctx) -> new Caller(ID, Integer.parseInt(token), 12345));
    return JdkHttpServerAdapter.listen(
        ProbeHandler.create(
            new Impl(), hooks, gen.b.runtimeconfig.GenBRuntime.builder().rulesOutput(mode).build()),
        new InetSocketAddress("127.0.0.1", 0));
  }

  private static Answer get(HttpServer server, String path, int level) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path))
        .header("Authorization", "Bearer " + level)
        .GET()
        .build();
    HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
    return new Answer(response.statusCode(), response.body());
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    HttpServer strict = serve(RuleWalk.OutputMode.STRICT);
    HttpServer off = serve(RuleWalk.OutputMode.OFF);
    try {
      thePrincipalIsGivenItsDerivedField(strict);
      theResultIsComputedBeforeTheOutputRules(strict, off);
      everyItemOfAStreamIsComputed(strict);
    } finally {
      strict.stop(0);
      off.stop(0);
    }
    System.out.println("OK");
  }

  /** `require user.cap >= 10` reads the cap that was computed from the level, not the hook's 12345. */
  private static void thePrincipalIsGivenItsDerivedField(HttpServer server) throws Exception {
    Answer fine = get(server, "/probe/echo?n=3", 5);
    check(fine.status() == 200, "a level of 5 gives a cap of 50: " + fine.status() + fine.text());
    check(SEEN.get().cap() == 50, "the handler sees the caller with its derived field: " + SEEN.get());
    Answer refused = get(server, "/probe/echo?n=3", 0);
    check(refused.status() == 422, "a level of 0 gives a cap of 0, whatever the hook put there: " + refused.status());
  }

  /** The handler's value of a derived field is overwritten; an invariant on it follows the output mode. */
  private static void theResultIsComputedBeforeTheOutputRules(HttpServer strict, HttpServer off) throws Exception {
    Answer ok = get(strict, "/probe/echo?n=3", 5);
    check(ok.body().get("sq").asDouble() == 9, "sq is computed, not the 12345 of the handler: " + ok.text());
    Answer rejected = get(strict, "/probe/echo?n=40", 5);
    check(rejected.status() == 500, "an invariant reads the computed sq: " + rejected.status());
    Answer computed = get(off, "/probe/echo?n=40", 5);
    check(computed.status() == 200 && computed.body().get("sq").asDouble() == 1600, "off still computes: " + computed.text());
  }

  /** The items of an SSE stream carry their derived field, and the value the handler put there is replaced. */
  private static void everyItemOfAStreamIsComputed(HttpServer server) throws Exception {
    Answer stream = get(server, "/probe/ticks?count=3", 5);
    check(stream.status() == 200, "the stream: " + stream.status() + stream.text());
    List<Json.Obj> items = new ArrayList<>();
    for (String line : stream.text().split("\n")) {
      if (line.startsWith("data: ")) {
        items.add(Json.parse(line.substring("data: ".length())).asObject());
      }
    }
    check(items.size() == 3, "three items: " + stream.text());
    for (int i = 0; i < 3; i++) {
      check(items.get(i).get("n").asDouble() == i + 1, "n of item " + i);
      check(items.get(i).get("twice").asDouble() == 2 * (i + 1), "twice of item " + i + ": " + items.get(i).write());
    }
  }
}
