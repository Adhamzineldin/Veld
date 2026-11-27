package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.c.runtime.ClientOptions;
import gen.c.tenancy.client.CreateBookingRequest;
import gen.c.tenancy.client.GetBookingRequest;
import gen.c.tenancy.client.TenantsClient;
import gen.c.tenancy.tenancy.Booking;
import java.io.IOException;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;

/**
 * The client of a contract with a tenant-scoped model (spec/platform.md section 3): the {@code
 * @tenant} field is set by the server, so a request may lack it (the client's local check does not
 * ask for it, and nothing of it is sent when the caller has none), and a value that carries it is
 * sent as it is. The response is decoded with its tenant. Prints {@code OK}.
 */
public final class TenancyClient {
  private TenancyClient() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID TENANT = UUID.fromString("00000000-0000-4000-8000-0000000000aa");

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    AtomicReference<String> received = new AtomicReference<>();
    HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    server.createContext("/", exchange -> {
      try {
        received.set(new String(exchange.getRequestBody().readAllBytes(), StandardCharsets.UTF_8));
        byte[] body = ("{\"id\":\"" + ID + "\",\"tenantId\":\"" + TENANT + "\",\"title\":\"t\",\"open\":false}")
            .getBytes(StandardCharsets.UTF_8);
        exchange.getResponseHeaders().add("content-type", "application/json");
        exchange.sendResponseHeaders(200, body.length);
        exchange.getResponseBody().write(body);
      } catch (IOException e) {
        throw e;
      } finally {
        exchange.close();
      }
    });
    server.start();
    try {
      TenantsClient client = new TenantsClient(
          ClientOptions.builder().baseUrl("http://127.0.0.1:" + server.getAddress().getPort()).build());
      // The caller has no tenant to send: the local check does not ask for one.
      Booking made = client.createBooking(new CreateBookingRequest(new Booking(ID, null, "t", false)));
      check(!received.get().contains("tenantId"), "no tenant is sent when there is none: " + received.get());
      check(TENANT.equals(made.tenantId()), "the answer has the tenant the server set");
      // A value that carries one is sent as it is: the server decides what to do with it.
      client.createBooking(new CreateBookingRequest(new Booking(ID, TENANT, "t", false)));
      check(received.get().contains(TENANT.toString()), "a tenant the caller wrote is sent as it is: " + received.get());
      check(client.getBooking(new GetBookingRequest(ID)).tenantId().equals(TENANT), "a response is decoded as it is");
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }
}
