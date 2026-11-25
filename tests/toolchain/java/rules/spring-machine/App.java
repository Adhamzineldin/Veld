package veldtest;

import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.BookingsTestHandler;
import gen.b.bookings.server.CancelBookingFacts;
import gen.b.bookings.server.CancelBookingRequest;
import gen.b.bookings.server.CheckInFacts;
import gen.b.bookings.server.CheckInRequest;
import gen.b.bookings.server.PayBookingFacts;
import gen.b.bookings.server.PayBookingRequest;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import jakarta.servlet.http.HttpServletRequest;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Bean;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * The generated test handler of the state machine contract inside a Spring Boot application. The
 * generated controller is not scanned (it would answer the production routes); one catch-all
 * controller hands every request to the test handler, which is what a test mounts.
 */
@SpringBootApplication(scanBasePackages = {"veldtest"})
public class App {
  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  @Bean
  VeldHandler testHandler() {
    BookingsService stub = new BookingsService() {
      @Override
      public Booking payBooking(
          PayBookingRequest request, PayBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Booking cancelBooking(
          CancelBookingRequest request, CancelBookingFacts facts, Transition<Status> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public void checkIn(
          CheckInRequest request, CheckInFacts facts, Transition<Status> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }
    };
    return BookingsTestHandler.create(stub, new BookingsHooks());
  }

  /** Hands every request to the test handler. */
  @RestController
  public static class Bridge {
    private final VeldHandler handler;

    public Bridge(VeldHandler handler) {
      this.handler = handler;
    }

    @RequestMapping("/**")
    public ResponseEntity<byte[]> any(HttpServletRequest request) throws IOException {
      Map<String, List<String>> headers = new LinkedHashMap<>();
      for (String name : Collections.list(request.getHeaderNames())) {
        headers.put(name, new ArrayList<>(Collections.list(request.getHeaders(name))));
      }
      String body = new String(request.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
      VeldRequest in = new VeldRequest(
          request.getMethod(), request.getRequestURI(), request.getQueryString(), headers, body, request);
      Optional<VeldResponse> out = handler.handle(in);
      if (out.isEmpty()) {
        return ResponseEntity.notFound().build();
      }
      ResponseEntity.BodyBuilder builder = ResponseEntity.status(out.get().status());
      out.get().headers().forEach(builder::header);
      String text = out.get().body();
      return text == null ? builder.build() : builder.body(text.getBytes(StandardCharsets.UTF_8));
    }
  }
}
