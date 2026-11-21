package veldtest;

import gen.sf.runtime.Json;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.List;

/**
 * The generated Java client against a server in another process:
 * {@code StreamCrossClient <base url> <label> [probe]}. With {@code probe} the server's counters
 * are read from {@code /__streams}. Every server is asked for SSE, WebSocket and auth.
 */
public final class StreamCrossClient {
  private StreamCrossClient() {}

  public static void main(String[] args) throws Exception {
    String base = args[0];
    String label = args[1];
    StreamScenario.Probe probe = args.length > 2 && args[2].equals("probe") ? probe(base) : null;
    StreamScenario.sse(base, probe);
    StreamScenario.ws(base, probe, !label.equals("spring"));
    StreamScenario.auth(base, true);
    System.out.println("stream cross client ok (" + label + ")");
  }

  private static StreamScenario.Probe probe(String base) {
    return new StreamScenario.Probe() {
      @Override
      public int foreverClosed() {
        return (int) read().get("foreverClosed").asDouble();
      }

      @Override
      public int chatEnded() {
        return (int) read().get("chatEnded").asDouble();
      }

      @Override
      public List<String> sink() {
        List<String> out = new ArrayList<>();
        for (Json j : read().get("sink").asArray().items()) {
          out.add(j.asString());
        }
        return out;
      }

      private Json.Obj read() {
        try {
          HttpResponse<String> res = HttpClient.newBuilder().version(HttpClient.Version.HTTP_1_1).build().send(
              HttpRequest.newBuilder(URI.create(base + "/__streams")).build(),
              HttpResponse.BodyHandlers.ofString());
          return Json.parse(res.body()).asObject();
        } catch (Exception e) {
          throw new IllegalStateException(e);
        }
      }
    };
  }
}
