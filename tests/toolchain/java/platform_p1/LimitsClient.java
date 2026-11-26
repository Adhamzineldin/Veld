package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.c.limits.client.ArchiveRequest;
import gen.c.limits.client.ByUserRequest;
import gen.c.limits.client.CreateRequest;
import gen.c.limits.client.RoomsClient;
import gen.c.limits.limits.Room;
import gen.c.prelude.TooManyRequests;
import gen.c.runtime.ClientOptions;
import gen.c.runtime.VeldTransportError;
import java.io.IOException;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The client of a contract with limits (spec/platform.md section 5): a method that gets {@code 429}
 * with {@code retryAfter} waits that long (through the sleeper of the options, so the test does not
 * wait) and sends the same request again, once, when the endpoint is a {@code get} or {@code
 * @idempotent}; any other endpoint, {@code retryOnLimit(false)}, a second {@code 429} and a {@code
 * 429} without {@code retryAfter} raise the typed error at once. Prints {@code OK}.
 */
public final class LimitsClient {
  private LimitsClient() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");

  /** What the fake server answers, in turn, and what it saw. */
  private static final class Server {
    final List<Integer> answers = new CopyOnWriteArrayList<>();
    final AtomicInteger seen = new AtomicInteger();
    final List<String> keys = new CopyOnWriteArrayList<>();
    volatile String limitedBody;
    HttpServer http;

    String limited(Integer retryAfter) {
      String data = retryAfter == null ? "{}" : "{\"retryAfter\":" + retryAfter + "}";
      return "{\"type\":\"veld:veld/TooManyRequests@6bb370c47c942eb5\",\"title\":\"TooManyRequests\",\"status\":429,\"operation\":\"x\",\"data\":"
          + data + "}";
    }

    void start() throws IOException {
      http = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
      http.createContext("/", exchange -> {
        try {
          exchange.getRequestBody().readAllBytes();
          String key = exchange.getRequestHeaders().getFirst("Idempotency-Key");
          if (key != null) {
            keys.add(key);
          }
          int n = seen.getAndIncrement();
          int status = n < answers.size() ? answers.get(n) : 200;
          byte[] body = (status == 429 ? limitedBody : "{\"id\":\"" + ID + "\"}").getBytes(StandardCharsets.UTF_8);
          exchange.getResponseHeaders().add("content-type", status == 429 ? "application/problem+json" : "application/json");
          exchange.sendResponseHeaders(status, body.length);
          exchange.getResponseBody().write(body);
        } finally {
          exchange.close();
        }
      });
      http.start();
    }

    String url() {
      return "http://127.0.0.1:" + http.getAddress().getPort();
    }

    void reset(Integer retryAfter, Integer... answered) {
      seen.set(0);
      keys.clear();
      answers.clear();
      for (Integer a : answered) {
        answers.add(a);
      }
      limitedBody = limited(retryAfter);
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    Server server = new Server();
    server.start();
    List<Long> slept = new ArrayList<>();
    try {
      RoomsClient client = new RoomsClient(
          ClientOptions.builder().baseUrl(server.url()).sleeper(slept::add).build());

      // A get: one wait of retryAfter, then the same request again.
      server.reset(2, 429);
      Room room = client.byUser(new ByUserRequest(ID));
      check(room.id().equals(ID), "the second answer is the call's");
      check(server.seen.get() == 2, "sent twice: " + server.seen.get());
      check(slept.equals(List.of(2000L)), "waited retryAfter, in milliseconds: " + slept);

      // An @idempotent post: the same key travels again.
      slept.clear();
      server.reset(1, 429);
      client.create(new CreateRequest(new Room(ID), "key-1"));
      check(server.seen.get() == 2 && server.keys.equals(List.of("key-1", "key-1")), "the same key: " + server.keys);
      check(slept.equals(List.of(1000L)), "waited: " + slept);

      // A post that is not idempotent: the typed error at once.
      slept.clear();
      server.reset(2, 429);
      try {
        client.archive(new ArchiveRequest(ID));
        throw new AssertionError("a post that may not be repeated is not sent again");
      } catch (TooManyRequests e) {
        check(e.data().retryAfter() != null && e.data().retryAfter() == 2, "the typed error has retryAfter: " + e.data());
      }
      check(server.seen.get() == 1 && slept.isEmpty(), "no wait, one request");

      // A second 429 is the typed error: the client waits once.
      slept.clear();
      server.reset(3, 429, 429);
      try {
        client.byUser(new ByUserRequest(ID));
        throw new AssertionError("a second 429 is raised");
      } catch (TooManyRequests e) {
        check(server.seen.get() == 2 && slept.equals(List.of(3000L)), "one wait, two requests: " + server.seen.get() + " " + slept);
      }

      // A 429 without retryAfter is raised at once.
      slept.clear();
      server.reset(null, 429);
      try {
        client.byUser(new ByUserRequest(ID));
        throw new AssertionError("a 429 without retryAfter is raised");
      } catch (TooManyRequests e) {
        check(server.seen.get() == 1 && slept.isEmpty(), "nothing to wait for");
      }

      // retryOnLimit(false) turns the wait off.
      RoomsClient patient = new RoomsClient(
          ClientOptions.builder().baseUrl(server.url()).sleeper(slept::add).retryOnLimit(false).build());
      server.reset(2, 429);
      try {
        patient.byUser(new ByUserRequest(ID));
        throw new AssertionError("with retryOnLimit(false) the typed error is raised at once");
      } catch (TooManyRequests e) {
        check(server.seen.get() == 1 && slept.isEmpty(), "no wait");
      }

      // The wait is inside the call's timeout (spec/platform.md 9.2): a wait that does not fit ends
      // the call as timed out, without sleeping and without a second request.
      slept.clear();
      RoomsClient hurried = new RoomsClient(ClientOptions.builder()
          .baseUrl(server.url())
          .sleeper(slept::add)
          .timeout(java.time.Duration.ofSeconds(1))
          .build());
      server.reset(5, 429);
      try {
        hurried.byUser(new ByUserRequest(ID));
        throw new AssertionError("a wait longer than the timeout is a timeout");
      } catch (VeldTransportError e) {
        check(e.getMessage().contains("timed out after 1000 ms"), "the message: " + e.getMessage());
        check(server.seen.get() == 1 && slept.isEmpty(), "one request and no wait: " + server.seen.get() + " " + slept);
      }
      // A wait that fits is waited out, as before.
      server.reset(0, 429);
      Room again = hurried.byUser(new ByUserRequest(ID));
      check(again.id().equals(ID) && server.seen.get() == 2, "a wait of 0 fits in the timeout");
    } finally {
      server.http.stop(0);
    }
    System.out.println("OK");
  }
}
