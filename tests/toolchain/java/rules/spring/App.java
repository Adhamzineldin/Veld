package veldtest;

import gen.rs.bk.bk.Booking;
import gen.rs.bk.bk.Overbooked;
import gen.rs.bk.server.BadRequest;
import gen.rs.bk.server.BookingsHooks;
import gen.rs.bk.server.BookingsService;
import gen.rs.bk.server.CreateRequest;
import gen.rs.bk.server.OverBadRequest;
import gen.rs.bk.server.OverOkRequest;
import gen.rs.bk.server.SearchRequest;
import gen.rs.runtime.RequestContext;
import java.time.Instant;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Bean;

/**
 * A Spring Boot application around the generated Spring adapter of the invariants contract: the
 * service and its hooks are plain beans, the generated controller is found by component scan.
 */
@SpringBootApplication(scanBasePackages = {"veldtest", "gen.rs.adapters"})
public class App {
  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  @Bean
  BookingsService bookingsService() {
    return new BookingsService() {
      @Override
      public Booking create(CreateRequest request, RequestContext ctx) {
        return request.body();
      }

      @Override
      public Booking bad(BadRequest request, RequestContext ctx) {
        return new Booking(Instant.parse("2026-01-02T00:00:00Z"), Instant.parse("2026-01-01T00:00:00Z"));
      }

      @Override
      public void overOk(OverOkRequest request, RequestContext ctx) {
        throw new Overbooked(new Overbooked.Data(2, 5));
      }

      @Override
      public void overBad(OverBadRequest request, RequestContext ctx) {
        throw new Overbooked(new Overbooked.Data(9, 5));
      }

      @Override
      public void search(SearchRequest request, RequestContext ctx) {}
    };
  }

  @Bean
  BookingsHooks bookingsHooks() {
    return new BookingsHooks();
  }
}
