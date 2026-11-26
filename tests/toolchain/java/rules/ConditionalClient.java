package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.idemp.client.BookRequest;
import gen.b.idemp.client.BookingsClient;
import gen.b.idemp.client.CancelRequest;
import gen.b.idemp.client.PayRequest;
import gen.b.idemp.client.ReplaceRequest;
import gen.b.idemp.client.SaveRoomRequest;
import gen.b.idemp.idemp.NewBooking;
import gen.b.idemp.idemp.Rename;
import gen.b.runtime.ClientOptions;
import gen.b.runtime.IfMatch;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;

/**
 * The headers the generated client sends for {@code ifMatch} and {@code idempotencyKey}: a version
 * and {@code *}, a key under the default name and under the name the annotation gives, and nothing
 * at all when the caller gives none. A required key is required by the type, and the client checks
 * the syntax of neither value (the server answers 422).
 */
public final class ConditionalClient {
  private ConditionalClient() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000c1");
  private static final AtomicReference<List<String>> SEEN = new AtomicReference<>();

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static String header(String name) {
    List<String> values = SEEN.get();
    for (String v : values) {
      if (v.toLowerCase().startsWith(name.toLowerCase() + "=")) {
        return v.substring(name.length() + 1);
      }
    }
    return null;
  }

  public static void main(String[] args) throws Exception {
    HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    server.createContext("/", exchange -> {
      java.util.ArrayList<String> seen = new java.util.ArrayList<>();
      exchange.getRequestHeaders().forEach((k, v) -> seen.add(k + "=" + v.get(0)));
      SEEN.set(seen);
      String body = exchange.getRequestURI().getPath().endsWith("/paid")
          ? "{\"id\":\"" + ID + "\",\"guests\":2}"
          : "{\"id\":\"" + ID + "\",\"name\":\"Blue\",\"revision\":\"5\"}";
      byte[] bytes = body.getBytes(StandardCharsets.UTF_8);
      exchange.getResponseHeaders().set("content-type", "application/json");
      exchange.sendResponseHeaders(exchange.getRequestMethod().equals("DELETE") ? 204 : 200, exchange.getRequestMethod().equals("DELETE") ? -1 : bytes.length);
      if (!exchange.getRequestMethod().equals("DELETE")) {
        exchange.getResponseBody().write(bytes);
      }
      exchange.close();
    });
    server.start();
    try {
      BookingsClient client = new BookingsClient(
          ClientOptions.builder().baseUrl("http://127.0.0.1:" + server.getAddress().getPort()).build());
      Rename rename = new Rename("Blue");
      client.saveRoom(new SaveRoomRequest(ID, rename, IfMatch.of(3), "k-1"));
      check("\"3\"".equals(header("If-Match")), "a version is a strong tag: " + SEEN.get());
      check("k-1".equals(header("Idempotency-Key")), "the key under the default name: " + SEEN.get());

      client.saveRoom(new SaveRoomRequest(ID, rename, IfMatch.any(), null));
      check("*".equals(header("If-Match")), "any is a star: " + SEEN.get());
      check(header("Idempotency-Key") == null, "no key given, none sent: " + SEEN.get());

      client.saveRoom(new SaveRoomRequest(ID, rename, null, null));
      check(header("If-Match") == null && header("Idempotency-Key") == null, "nothing given, nothing sent: " + SEEN.get());

      IfMatch zero = IfMatch.of(0);
      client.saveRoom(new SaveRoomRequest(ID, rename, zero, "a b"));
      check("\"0\"".equals(header("If-Match")), "version zero: " + SEEN.get());
      check("a b".equals(header("Idempotency-Key")), "the client does not check the key: " + SEEN.get());

      client.replace(new ReplaceRequest(ID, new NewBooking(2), "k-2"));
      check("k-2".equals(header("X-Request-Id")), "the header the annotation names: " + SEEN.get());
      check(header("Idempotency-Key") == null, "and not the default one: " + SEEN.get());

      client.pay(new PayRequest(new NewBooking(2), "k-3"));
      check("k-3".equals(header("Idempotency-Key")), "a required key is sent: " + SEEN.get());
      client.cancel(new CancelRequest(ID, "k-4"));
      check("k-4".equals(header("Idempotency-Key")), "the key of a call without a response body: " + SEEN.get());
      client.book(new BookRequest(new NewBooking(2), null));
      check(header("Idempotency-Key") == null, "an optional key may be left out: " + SEEN.get());

      try {
        new PayRequest(new NewBooking(2), null);
        throw new AssertionError("a required key is required by the type");
      } catch (NullPointerException expected) {
        check(expected.getMessage().contains("idempotencyKey"), "names the key: " + expected.getMessage());
      }
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }
}
