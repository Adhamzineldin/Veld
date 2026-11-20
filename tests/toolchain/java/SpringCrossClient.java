package veldtest;

import gen.a.runtime.Json;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.List;

/**
 * Runs the client scenario against the Spring Boot server (tests/toolchain/java/spring) in another
 * process: {@code SpringCrossClient <base url>}. The server's counters come from its {@code /__probe}.
 */
public final class SpringCrossClient {
  private SpringCrossClient() {}

  public static void main(String[] args) throws Exception {
    String base = args[0];
    Scenario.run("spring", base, new Scenario.Probe() {
      @Override
      public int middlewareCalls() {
        return (int) probe().get("middleware").asDouble();
      }

      @Override
      public int requests() {
        return (int) probe().get("requests").asDouble();
      }

      @Override
      public List<Throwable> thrown() {
        List<Throwable> out = new ArrayList<>();
        for (Json j : probe().get("thrown").asArray().items()) {
          out.add(new RuntimeException(j.asString()));
        }
        return out;
      }

      @Override
      public List<String> seen() {
        List<String> out = new ArrayList<>();
        for (Json j : probe().get("seen").asArray().items()) {
          out.add(j.asString());
        }
        return out;
      }

      private Json.Obj probe() {
        try {
          HttpResponse<String> res = HttpClient.newHttpClient().send(
              HttpRequest.newBuilder(URI.create(base + "/__probe")).build(),
              HttpResponse.BodyHandlers.ofString());
          return Json.parse(res.body()).asObject();
        } catch (Exception e) {
          throw new IllegalStateException(e);
        }
      }
    });
    System.out.println("spring client ok");
  }
}
