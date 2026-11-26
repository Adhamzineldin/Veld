package veldtest;

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
 * The generated test handler of the idempotency contract inside a Spring Boot application, built by
 * {@code IdempotencyStubs.handler()}. The generated controllers are not scanned (they would answer the production
 * routes); one catch-all controller hands every request to the test handler, which is what a test
 * mounts.
 */
@SpringBootApplication(scanBasePackages = {"veldtest"})
public class App {
  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  @Bean
  VeldHandler testHandler() {
    return IdempotencyStubs.handler();
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
