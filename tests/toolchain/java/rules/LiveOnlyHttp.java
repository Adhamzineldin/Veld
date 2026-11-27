package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.only.only.Booking;
import gen.b.only.only.Room;
import gen.b.only.server.BookRequest;
import gen.b.only.server.BookingsHooks;
import gen.b.only.server.BookingsService;
import gen.b.only.server.BookingsTestHandler;
import gen.b.only.server.GetRoomRequest;
import gen.b.only.server.RoomsService;
import gen.b.only.server.RoomsTestHandler;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * The generated test handlers of a contract whose services have no endpoint rule: {@code Bookings}
 * has one {@code @idempotent} endpoint and {@code Rooms} one that owes an {@code ETag}
 * (spec/rules.md section 11.12: both get a test handler, so that {@code veld test --live} reaches
 * them). Checks the probe, a dry run, the state of the store from {@code Veld-Test-Idempotency}
 * (a stored answer, another fingerprint, a running request), the result from
 * {@code Veld-Test-Result} with its {@code ETag}, and that the application is never reached.
 */
public final class LiveOnlyHttp {
  private LiveOnlyHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000c1");
  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private record Answer(int status, Map<String, String> headers, String text) {
    String header(String name) {
      return headers.get(name.toLowerCase());
    }

    Json.Obj body() {
      return text.isEmpty() ? null : Json.parse(text).asObject();
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static String b64(String text) {
    return Base64.getUrlEncoder().withoutPadding().encodeToString(text.getBytes(StandardCharsets.UTF_8));
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

  /** The application: never reached, because every request is a dry run or brings its result. */
  private static final class Bookings implements BookingsService {
    @Override
    public Booking book(BookRequest request, RequestContext ctx) {
      throw new UnsupportedOperationException("the application is not part of the examples");
    }
  }

  private static final class Rooms implements RoomsService {
    @Override
    public Room getRoom(GetRoomRequest request, RequestContext ctx) {
      throw new UnsupportedOperationException("the application is not part of the examples");
    }
  }

  public static void main(String[] args) throws Exception {
    HttpServer bookings = JdkHttpServerAdapter.listen(
        BookingsTestHandler.create(new Bookings(), new BookingsHooks(new MemoryIdempotencyStore())),
        new InetSocketAddress("127.0.0.1", 0));
    HttpServer rooms = JdkHttpServerAdapter.listen(
        RoomsTestHandler.create(new Rooms()), new InetSocketAddress("127.0.0.1", 0));
    try {
      theKeyOnlyService(bookings);
      theEtagOnlyService(rooms);
    } finally {
      bookings.stop(0);
      rooms.stop(0);
    }
    System.out.println("OK");
  }

  private static void theKeyOnlyService(HttpServer server) throws Exception {
    Answer probe = send(server, "GET", "/__veld/test", Map.of(), null);
    check(probe.status() == 200 && probe.body().get("veldTestHandler") != null, "the probe: " + probe.status());

    String body = "{\"guests\":2}";
    Answer dry = send(server, "POST", "/bookings", Map.of("Idempotency-Key", "k-1", "Veld-Test-Dry-Run", "1"), body);
    check(dry.status() == 204 && "1".equals(dry.header("Veld-Test-Passed")), "a dry run passes: " + dry.status());

    String booking = "{\"id\":\"" + ID + "\",\"guests\":2}";
    Answer given = send(
        server, "POST", "/bookings", Map.of("Idempotency-Key", "k-2", "Veld-Test-Result", b64(booking)), body);
    check(given.status() == 201 && given.body().get("guests") != null, "the supplied result: " + given.status());

    String stored = "{\"state\":\"stored\",\"status\":201,\"type\":null,\"data\":" + booking + "}";
    Answer replay = send(
        server, "POST", "/bookings", Map.of("Idempotency-Key", "k-3", "Veld-Test-Idempotency", b64(stored)), body);
    check(replay.status() == 201, "a stored answer: " + replay.status());
    check("true".equals(replay.header("Veld-Idempotent-Replay")), "marked as a replay");
    check(replay.body().get("id").asString().equals(ID.toString()), "the stored body: " + replay.text());

    for (String state : new String[] {"other", "running"}) {
      Answer conflict = send(
          server,
          "POST",
          "/bookings",
          Map.of("Idempotency-Key", "k-4", "Veld-Test-Idempotency", b64("{\"state\":\"" + state + "\"}")),
          body);
      check(conflict.status() == 409 && conflict.body().get("title").asString().equals("Conflict"), state + ": " + conflict.status());
    }

    Answer bad = send(server, "POST", "/bookings", Map.of("Idempotency-Key", "has space"), body);
    check(bad.status() == 422, "a malformed key is 422: " + bad.status());
  }

  private static void theEtagOnlyService(HttpServer server) throws Exception {
    Answer probe = send(server, "GET", "/__veld/test", Map.of(), null);
    check(probe.status() == 200, "the probe: " + probe.status());
    String path = "/rooms/" + ID;
    Answer dry = send(server, "GET", path, Map.of("Veld-Test-Dry-Run", "1"), null);
    check(dry.status() == 204 && "1".equals(dry.header("Veld-Test-Passed")), "a dry run passes: " + dry.status());
    String room = "{\"id\":\"" + ID + "\",\"name\":\"Blue\",\"revision\":\"7\"}";
    Answer got = send(server, "GET", path, Map.of("Veld-Test-Result", b64(room)), null);
    check(got.status() == 200, "the supplied result: " + got.status());
    check("\"7\"".equals(got.header("ETag")), "the ETag of the returned value: " + got.header("ETag"));
  }
}
