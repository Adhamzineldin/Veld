package dev.veld.adapters;

import dev.veld.runtime.VeldHandler;
import dev.veld.runtime.VeldRequest;
import dev.veld.runtime.VeldResponse;
import jakarta.servlet.http.HttpServletRequest;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * A Spring Boot 3 (Jakarta) controller that serves a {@link VeldHandler} for every path. Declare
 * the handler as a bean and component-scan this package, or register the controller yourself:
 *
 * <pre>{@code
 * @Bean
 * VeldHandler veldHandler(MyService impl) {
 *   return MyServiceHandler.create(impl, new MyServiceHooks());
 * }
 * }</pre>
 *
 * <p>This is the only generated file that imports Spring.
 */
@RestController
public class SpringAdapter {
  private final VeldHandler handler;

  /** Creates the controller with the handler bean. */
  public SpringAdapter(VeldHandler handler) {
    this.handler = handler;
  }

  /** Serves every request through the Veld handler; unmatched requests get an empty 404. */
  @RequestMapping("/**")
  public ResponseEntity<byte[]> handle(HttpServletRequest request) throws IOException {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    for (String name : Collections.list(request.getHeaderNames())) {
      headers.put(name, new ArrayList<>(Collections.list(request.getHeaders(name))));
    }
    String body = new String(request.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
    VeldRequest veld = new VeldRequest(
        request.getMethod(), request.getRequestURI(), request.getQueryString(), headers, body, request);
    Optional<VeldResponse> response = handler.handle(veld);
    if (response.isEmpty()) {
      return ResponseEntity.notFound().build();
    }
    ResponseEntity.BodyBuilder builder = ResponseEntity.status(response.get().status());
    response.get().headers().forEach(builder::header);
    String text = response.get().body();
    return text == null ? builder.build() : builder.body(text.getBytes(StandardCharsets.UTF_8));
  }
}
