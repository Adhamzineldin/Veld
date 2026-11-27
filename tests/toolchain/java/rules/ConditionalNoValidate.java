package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.idemp.idemp.Booking;
import gen.b.idemp.idemp.Room;
import gen.b.idemp.idemp.Shelf;
import gen.b.idemp.server.BookFacts;
import gen.b.idemp.server.BookMineRequest;
import gen.b.idemp.server.BookRequest;
import gen.b.idemp.server.BookingsFactLoaders;
import gen.b.idemp.server.BookingsHandler;
import gen.b.idemp.server.BookingsHooks;
import gen.b.idemp.server.BookingsService;
import gen.b.idemp.server.CancelRequest;
import gen.b.idemp.server.PayRequest;
import gen.b.idemp.server.ReplaceRequest;
import gen.b.idemp.server.SaveRoomFacts;
import gen.b.idemp.server.SaveRoomRequest;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/**
 * The production handler of {@code tests/rules/endpoints/idempotency.veld.pending} generated with
 * {@code validate = false} (spec/rules.md section 11.10): the rules of the request are off, so a body
 * that breaks {@code atLeastOne} and a {@code require} that would fail go through, but the
 * idempotency key (its syntax, a required one that is missing, the store) and the {@code @version}
 * guard (the syntax of {@code If-Match}, a stale tag, {@code *}) are part of what the endpoint does
 * and stay on, and the facts are still loaded.
 */
public final class ConditionalNoValidate {
  private ConditionalNoValidate() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000d1");
  private static final AtomicInteger CALLS = new AtomicInteger();
  private static final AtomicInteger LOADS = new AtomicInteger();
  private static final AtomicReference<Long> VERSION = new AtomicReference<>();
  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static final class Loaders implements BookingsFactLoaders {
    @Override
    public Optional<Room> room(RequestContext ctx) {
      LOADS.incrementAndGet();
      return Optional.of(new Room(ID, 9));
    }

    @Override
    public Optional<Shelf> shelf(UUID key, RequestContext ctx) {
      LOADS.incrementAndGet();
      return Optional.of(new Shelf(ID, "Red", 4));
    }
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking book(BookRequest request, BookFacts facts, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(ID, request.body().guests());
    }

    @Override
    public Booking pay(PayRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(ID, request.body().guests());
    }

    @Override
    public Booking replace(ReplaceRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(request.id(), request.body().guests());
    }

    @Override
    public void cancel(CancelRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
    }

    @Override
    public Booking bookMine(BookMineRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(ID, request.body().guests());
    }

    @Override
    public Shelf saveRoom(SaveRoomRequest request, SaveRoomFacts facts, Long version, RequestContext ctx) {
      CALLS.incrementAndGet();
      VERSION.set(version);
      return new Shelf(ID, request.body().name(), facts.shelf().revision() + 1);
    }
  }

  private record Answer(int status, Map<String, String> headers, String text) {
    String header(String name) {
      return headers.get(name.toLowerCase());
    }

    Json.Obj body() {
      return text.isEmpty() ? null : Json.parse(text).asObject();
    }

    boolean replayed() {
      return "true".equals(header("Veld-Idempotent-Replay"));
    }

    Json.Obj firstIssue() {
      return body().get("data").asObject().get("issues").asArray().items().get(0).asObject();
    }
  }

  private static Answer send(HttpServer server, String method, String path, Map<String, String> headers, String body)
      throws Exception {
    HttpRequest.Builder builder = HttpRequest.newBuilder(
        URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path));
    headers.forEach(builder::header);
    if (body != null) {
      builder.header("content-type", "application/json");
    }
    builder.method(method, body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
    HttpResponse<String> response = HTTP.send(builder.build(), HttpResponse.BodyHandlers.ofString());
    Map<String, String> seen = new LinkedHashMap<>();
    response.headers().map().forEach((k, v) -> seen.put(k.toLowerCase(), v.get(0)));
    return new Answer(response.statusCode(), seen, response.body());
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    BookingsHooks hooks = new BookingsHooks(
        (token, ctx) -> token, new MemoryIdempotencyStore(), (principal, ctx) -> (String) principal);
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsHandler.create(new Impl(), new Loaders(), hooks), new InetSocketAddress("127.0.0.1", 0));
    try {
      theRulesOfTheRequestAreOff(server);
      theKeyIsStillChecked(server);
      theGuardIsStillChecked(server);
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  /** A body that breaks `atLeastOne` and a `require` that fails go through; the fact is loaded. */
  private static void theRulesOfTheRequestAreOff(HttpServer server) throws Exception {
    int loads = LOADS.get();
    Answer free = send(server, "POST", "/bookings/paid", Map.of("Idempotency-Key", "k-free"), "{\"guests\":0}");
    check(free.status() == 201, "the invariant is off: " + free.status());
    Answer full = send(server, "POST", "/bookings", Map.of("Idempotency-Key", "k-full"), "{\"guests\":20}");
    check(full.status() == 201, "the require is off: " + full.status());
    check(LOADS.get() == loads + 1, "the fact is still loaded, the handler's type holds it");
  }

  /** The syntax of the key, a required key that is missing and the store are not request validation. */
  private static void theKeyIsStillChecked(HttpServer server) throws Exception {
    int calls = CALLS.get();
    Answer missing = send(server, "POST", "/bookings/paid", Map.of(), "{\"guests\":2}");
    check(missing.status() == 422, "a required key that is missing: " + missing.status());
    check(missing.firstIssue().get("path").asString().equals("header.Idempotency-Key"), missing.text());
    check(missing.firstIssue().get("code").asString().equals("required"), missing.text());
    for (String bad : new String[] {"has space", "k".repeat(256)}) {
      Answer a = send(server, "POST", "/bookings", Map.of("Idempotency-Key", bad), "{\"guests\":2}");
      check(a.status() == 422, "a malformed key is 422: " + bad.length() + " " + a.status());
      check(a.firstIssue().get("path").asString().equals("header.Idempotency-Key"), a.text());
      check(a.firstIssue().get("code").asString().equals("type"), a.text());
    }
    check(CALLS.get() == calls, "none of them ran");
    Map<String, String> key = Map.of("Idempotency-Key", "k-same");
    Answer first = send(server, "POST", "/bookings", key, "{\"guests\":2}");
    Answer again = send(server, "POST", "/bookings", key, "{\"guests\":2}");
    check(first.status() == 201 && again.status() == 201 && again.replayed(), "the store is asked: " + again.status());
    Answer other = send(server, "POST", "/bookings", key, "{\"guests\":3}");
    check(other.status() == 409, "another fingerprint is a conflict: " + other.status());
  }

  /** The syntax of `If-Match`, a stale tag, `*` and the version the handler gets. */
  private static void theGuardIsStillChecked(HttpServer server) throws Exception {
    String path = "/bookings/rooms/" + ID;
    String body = "{\"name\":\"Blue\"}";
    int calls = CALLS.get();
    Answer weak = send(server, "PUT", path, Map.of("If-Match", "W/\"4\""), body);
    check(weak.status() == 422, "a weak tag is malformed: " + weak.status());
    check(weak.firstIssue().get("path").asString().equals("header.If-Match"), weak.text());
    check(weak.body().get("violations") == null, "a malformed tag is not a violation");
    Answer stale = send(server, "PUT", path, Map.of("If-Match", "\"1\""), body);
    check(stale.status() == 412 && stale.body().get("title").asString().equals("PreconditionFailed"), "stale: " + stale.status());
    check(stale.body().get("violations").asArray().items().size() == 1, "one violation: " + stale.text());
    check(CALLS.get() == calls, "the handler did not run");
    Answer any = send(server, "PUT", path, Map.of("If-Match", "*"), body);
    check(any.status() == 200 && VERSION.get() == null, "* passes and hands over no version: " + any.status());
    Answer fresh = send(server, "PUT", path, Map.of("If-Match", "\"4\""), body);
    check(fresh.status() == 200 && Long.valueOf(4).equals(VERSION.get()), "the version that was checked: " + VERSION.get());
    check("\"5\"".equals(fresh.header("etag")), "the ETag: " + fresh.header("etag"));
  }
}
