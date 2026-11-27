package veldtest;

import gen.rg.bookings.bookings.Booking;
import gen.rg.bookings.bookings.Report;
import gen.rg.bookings.server.BookingsHooks;
import gen.rg.bookings.server.BookingsService;
import gen.rg.bookings.server.CreateRequest;
import gen.rg.bookings.server.ReportRequest;
import gen.rg.bookings.server.SearchRequest;
import gen.rg.runtime.RequestContext;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Bean;

/** The Spring application of the 422 golden: the generated adapter around the rules contract. */
@SpringBootApplication(scanBasePackages = {"veldtest", "gen.rg.adapters"})
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
      public Integer search(SearchRequest request, RequestContext ctx) {
        return 0;
      }

      @Override
      public Report report(ReportRequest request, RequestContext ctx) {
        throw new UnsupportedOperationException();
      }
    };
  }

  @Bean
  BookingsHooks bookingsHooks() {
    return new BookingsHooks();
  }
}
