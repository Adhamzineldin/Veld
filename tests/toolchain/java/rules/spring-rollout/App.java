package veldtest;

import gen.p.bk.bk.Booking;
import gen.p.bk.bk.Receipt;
import gen.p.bk.server.BookingRequest;
import gen.p.bk.server.EnforcedFacts;
import gen.p.bk.server.EnforcedRequest;
import gen.p.bk.server.MixedRequest;
import gen.p.bk.server.ReceiptRequest;
import gen.p.bk.server.WatchedFacts;
import gen.p.bk.server.WatchedRequest;
import gen.p.bk.server.PartiesHooks;
import gen.p.bk.server.PartiesService;
import gen.p.bk.server.PartiesTestHandler;
import gen.p.runtime.RequestContext;
import gen.p.runtime.VeldHandler;
import gen.p.runtime.VeldRequest;
import gen.p.runtime.VeldResponse;
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
 * The generated test handler of the endpoint rules contract inside a Spring Boot application. The
 * generated controller is not scanned (it would answer the production routes); one catch-all
 * controller hands every request to the test handler, which is what a test mounts.
 */
@SpringBootApplication(scanBasePackages = {"veldtest"})
public class App {
  private static UnsupportedOperationException never() {
    return new UnsupportedOperationException("the application's handler is not part of the examples");
  }

  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  @Bean
  VeldHandler testHandler() {
    PartiesService stub = new PartiesService() {
      @Override
      public Booking watched(WatchedRequest request, WatchedFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public Receipt enforced(EnforcedRequest request, EnforcedFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking mixed(MixedRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking booking(BookingRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Receipt receipt(ReceiptRequest request, RequestContext ctx) {
        throw never();
      }
    };
    return PartiesTestHandler.create(stub, new PartiesHooks());
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
