package veldtest;

import gen.a.runtime.Json;
import java.util.ArrayList;
import java.util.List;
import org.springframework.http.MediaType;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

/** What the test client can learn about this server: the counters of {@link ServerImpl}. */
@RestController
public class ProbeController {
  private final ServerImpl impl;

  public ProbeController(ServerImpl impl) {
    this.impl = impl;
  }

  @GetMapping(path = "/__probe", produces = MediaType.APPLICATION_JSON_VALUE)
  public String probe() {
    List<Json> thrown = new ArrayList<>();
    synchronized (impl.thrown) {
      for (Throwable t : impl.thrown) {
        thrown.add(Json.of(String.valueOf(t.getMessage())));
      }
    }
    List<Json> seen = new ArrayList<>();
    synchronized (impl.seen) {
      for (String s : impl.seen) {
        seen.add(Json.of(s));
      }
    }
    return Json.object()
        .put("middleware", Json.of((long) impl.rateLimitCalls.get()))
        .put("requests", Json.of((long) impl.requests.get()))
        .put("thrown", Json.array(thrown))
        .put("seen", Json.array(seen))
        .build()
        .write();
  }
}
