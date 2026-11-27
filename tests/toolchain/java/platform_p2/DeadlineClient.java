package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.c.levels.client.GetRoomRequest;
import gen.c.levels.client.ListRoomsRequest;
import gen.c.levels.client.RoomsClient;
import gen.c.runtime.ClientOptions;
import gen.c.runtime.VeldTransportError;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.UUID;

/**
 * The client of a contract with a deadline (spec/platform.md section 12), against a server that
 * answers after 600 ms: a call of an endpoint with {@code @timeout(ms(200))} fails with the client's
 * timeout error after about 200 ms when the caller set no timeout; a timeout the caller set wins; an
 * endpoint without a deadline keeps the default of the options. Prints {@code OK}.
 */
public final class DeadlineClient {
  private DeadlineClient() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    server.createContext("/", exchange -> {
      try {
        Thread.sleep(600);
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
      }
      boolean list = exchange.getRequestURI().getPath().equals("/rooms");
      byte[] body = (list ? "[]" : "{\"id\":\"" + ID + "\",\"name\":\"a\"}").getBytes(StandardCharsets.UTF_8);
      exchange.getResponseHeaders().add("content-type", "application/json");
      exchange.sendResponseHeaders(200, body.length);
      exchange.getResponseBody().write(body);
      exchange.close();
    });
    server.start();
    String base = "http://127.0.0.1:" + server.getAddress().getPort();
    try {
      RoomsClient byDeadline = new RoomsClient(ClientOptions.builder().baseUrl(base).build());
      long started = System.nanoTime();
      try {
        byDeadline.getRoom(new GetRoomRequest(ID));
        throw new AssertionError("the call outlived its deadline");
      } catch (VeldTransportError e) {
        long ms = (System.nanoTime() - started) / 1_000_000L;
        check(e.getMessage().contains("timed out after 200 ms"), "the client's timeout error: " + e.getMessage());
        check(ms < 550, "after about the deadline, not the handler: " + ms + " ms");
      }

      RoomsClient chosen = new RoomsClient(ClientOptions.builder().baseUrl(base).timeout(Duration.ofSeconds(5)).build());
      check(chosen.getRoom(new GetRoomRequest(ID)).name().equals("a"), "a timeout the caller set wins over the deadline");

      check(byDeadline.listRooms(new ListRoomsRequest()).isEmpty(), "an endpoint without a deadline keeps the default timeout");
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }
}
