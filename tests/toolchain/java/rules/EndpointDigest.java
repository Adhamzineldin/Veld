package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CreateBookingFacts;
import gen.b.bookings.server.CreateBookingRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.RequestContext;
import gen.c.bookings.bookings.NewBooking;
import gen.c.bookings.client.BookingsClient;
import gen.c.runtime.ClientOptions;
import gen.c.runtime.VeldError;
import java.math.BigDecimal;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The rules digest (spec/rules.md section 9.7): the server sends {@code Veld-Rules} on every
 * response, and the client rejects locally only while the last digest equals its own.
 */
public final class EndpointDigest {
  private EndpointDigest() {}

  private static final UUID ROOM = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final Instant T0 = Instant.parse("2026-07-01T00:00:00Z");
  private static final Instant T1 = Instant.parse("2026-07-02T00:00:00Z");
  private static final AtomicInteger SERVED = new AtomicInteger();

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static NewBooking good() {
    return new NewBooking(ROOM, 2, T0, T1, null);
  }

  private static NewBooking bad() {
    return new NewBooking(ROOM, 2, T1, T0, null);
  }

  private static BookingsClient client(String base, List<ClientOptions.ContractMismatch> mismatches) {
    return new BookingsClient(ClientOptions.builder().baseUrl(base).onContractMismatch(mismatches::add).build());
  }

  private static gen.c.bookings.client.CreateBookingRequest request(NewBooking body) {
    return new gen.c.bookings.client.CreateBookingRequest(body);
  }

  public static void main(String[] args) throws Exception {
    sameDigest();
    otherDigest();
    System.out.println("OK");
  }

  /** The generated server: the digest it sends is the client's own, so the local check works. */
  private static void sameDigest() throws Exception {
    BookingsService impl = new BookingsService() {
      @Override
      public gen.b.bookings.bookings.Booking createBooking(
          CreateBookingRequest request, CreateBookingFacts facts, RequestContext ctx) {
        SERVED.incrementAndGet();
        var b = request.body();
        return new gen.b.bookings.bookings.Booking(
            b.roomId(), b.guests(), b.start(), b.end(), null, ROOM, gen.b.bookings.bookings.Status.PENDING, BigDecimal.ZERO);
      }

      @Override
      public gen.b.bookings.bookings.Booking payBooking(PayBookingRequest request, PayBookingFacts facts, RequestContext ctx) {
        throw new UnsupportedOperationException();
      }
    };
    BookingsFactLoaders loaders = new BookingsFactLoaders() {
      @Override
      public Optional<gen.b.bookings.bookings.Room> room(UUID key, RequestContext ctx) {
        return Optional.of(new gen.b.bookings.bookings.Room(ROOM, 4, false, BigDecimal.TEN));
      }

      @Override
      public Optional<gen.b.bookings.bookings.Booking> booking(UUID key, RequestContext ctx) {
        return Optional.empty();
      }

      @Override
      public Optional<gen.b.bookings.bookings.Coupon> coupon(String key, RequestContext ctx) {
        return Optional.empty();
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsHandler.create(
            impl,
            loaders,
            new BookingsHooks().clock(java.time.Clock.fixed(Instant.parse("2026-06-01T00:00:00Z"), java.time.ZoneOffset.UTC))), new InetSocketAddress("127.0.0.1", 0));
    try {
      List<ClientOptions.ContractMismatch> mismatches = new ArrayList<>();
      BookingsClient client = client("http://127.0.0.1:" + server.getAddress().getPort(), mismatches);
      // Before the first response a request that fails the local check is sent anyway.
      VeldError first = thrown(() -> client.createBooking(request(bad())));
      check(first.title().equals("ValidationError") && !first.raisedLocally(), "the server decides before the first response");
      check(first.violations().size() == 1, "with its violation");
      check(SERVED.get() == 0, "the handler did not run");
      // That response carried the digest, so the client checks locally now.
      VeldError local = thrown(() -> client.createBooking(request(bad())));
      check(local.raisedLocally(), "rejected locally while the digest matches");
      check(client.createBooking(request(good())).guests() == 2, "a valid booking goes through");
      check(SERVED.get() == 1, "and runs the handler once");
      check(mismatches.isEmpty(), "the digests match: no callback");
    } finally {
      server.stop(0);
    }
  }

  /** A server with another digest: the check stops, the callback runs once per digest. */
  private static void otherDigest() throws Exception {
    String[] digest = {"0123456789abcdef"};
    AtomicInteger requests = new AtomicInteger();
    String body = "{\"roomId\":\"" + ROOM + "\",\"guests\":2,\"start\":\"2026-07-01T00:00:00Z\",\"end\":\"2026-07-02T00:00:00Z\","
        + "\"id\":\"" + ROOM + "\",\"status\":\"pending\",\"total\":\"0\"}";
    HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    server.createContext("/", exchange -> {
      requests.incrementAndGet();
      exchange.getRequestBody().readAllBytes();
      byte[] bytes = body.getBytes(StandardCharsets.UTF_8);
      exchange.getResponseHeaders().add("content-type", "application/json");
      if (digest[0] != null) {
        exchange.getResponseHeaders().add("Veld-Rules", digest[0]);
      }
      exchange.sendResponseHeaders(201, bytes.length);
      exchange.getResponseBody().write(bytes);
      exchange.close();
    });
    server.start();
    try {
      List<ClientOptions.ContractMismatch> mismatches = new ArrayList<>();
      BookingsClient client = client("http://127.0.0.1:" + server.getAddress().getPort(), mismatches);
      client.createBooking(request(good()));
      check(mismatches.size() == 1, "one callback for the first other digest: " + mismatches.size());
      ClientOptions.ContractMismatch m = mismatches.get(0);
      check(m.service().equals("Bookings") && m.serverDigest().equals("0123456789abcdef"), "its content: " + m);
      check(!m.clientDigest().equals(m.serverDigest()), "the digests differ");
      int sent = requests.get();
      client.createBooking(request(bad()));
      check(requests.get() == sent + 1, "the local check is off: the request was sent");
      check(mismatches.size() == 1, "no second callback for a digest it has seen");
      digest[0] = "fedcba9876543210";
      client.createBooking(request(good()));
      check(mismatches.size() == 2, "a new digest calls back again");
      digest[0] = null;
      client.createBooking(request(good()));
      client.createBooking(request(bad()));
      check(requests.get() == sent + 4, "a response without the header leaves the local check off");
      digest[0] = m.clientDigest();
      client.createBooking(request(good()));
      thrown(() -> client.createBooking(request(bad())));
      check(mismatches.size() == 2, "the client's own digest is not a mismatch");
    } finally {
      server.stop(0);
    }
  }

  private static VeldError thrown(Runnable call) {
    try {
      call.run();
    } catch (VeldError e) {
      return e;
    }
    throw new AssertionError("expected a VeldError");
  }
}
