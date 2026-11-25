package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.rc.bookings.bookings.Booking;
import gen.rc.bookings.client.BookingsClient;
import gen.rc.bookings.client.CreateRequest;
import gen.rc.runtime.ClientOptions;
import gen.rs.adapters.JdkHttpServerAdapter;
import gen.rs.bookings.server.BookingsHandler;
import gen.rs.bookings.server.BookingsHooks;
import gen.rs.bookings.server.BookingsService;
import gen.rs.bookings.server.ReportRequest;
import gen.rs.bookings.server.SearchRequest;
import gen.rs.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.time.Instant;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.logging.Handler;
import java.util.logging.Level;
import java.util.logging.LogRecord;
import java.util.logging.Logger;

/**
 * A server and a client generated with {@code validate = false}: a body that violates its rules
 * is accepted and sent, and the server says so exactly once, when its handler is created.
 */
public final class RuleNoValidate {
  private RuleNoValidate() {}

  private static final AtomicInteger WARNINGS = new AtomicInteger();
  private static final AtomicInteger CREATED = new AtomicInteger();

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    Logger log = Logger.getLogger("veld.rules");
    log.setUseParentHandlers(false);
    log.addHandler(new Handler() {
      @Override
      public void publish(LogRecord record) {
        if (record.getLevel() == Level.WARNING) {
          WARNINGS.incrementAndGet();
        }
      }

      @Override
      public void flush() {}

      @Override
      public void close() {}
    });
    BookingsService impl = new BookingsService() {
      @Override
      public gen.rs.bookings.bookings.Booking create(
          gen.rs.bookings.server.CreateRequest request, RequestContext ctx) {
        CREATED.incrementAndGet();
        return new gen.rs.bookings.bookings.Booking(
            Instant.parse("2026-01-01T00:00:00Z"), Instant.parse("2026-01-02T00:00:00Z"), 1);
      }

      @Override
      public Integer search(SearchRequest request, RequestContext ctx) {
        return 0;
      }

      @Override
      public gen.rs.bookings.bookings.Report report(ReportRequest request, RequestContext ctx) {
        throw new UnsupportedOperationException();
      }
    };
    check(WARNINGS.get() == 0, "nothing is reported before the handler exists");
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsHandler.create(impl, new BookingsHooks()), new InetSocketAddress("127.0.0.1", 0));
    try {
      check(WARNINGS.get() == 1, "reported once when the handler is created: " + WARNINGS.get());
      BookingsClient client = new BookingsClient(
          ClientOptions.builder().baseUrl("http://127.0.0.1:" + server.getAddress().getPort()).build());
      Booking bad = new Booking(Instant.parse("2026-01-02T00:00:00Z"), Instant.parse("2026-01-01T00:00:00Z"), 11);
      Booking back = client.create(new CreateRequest(bad));
      check(back.guests() == 1, "the violating booking was accepted and answered");
      check(CREATED.get() == 1, "the handler ran");
      client.create(new CreateRequest(bad));
      check(WARNINGS.get() == 1, "still reported once: " + WARNINGS.get());
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }
}
