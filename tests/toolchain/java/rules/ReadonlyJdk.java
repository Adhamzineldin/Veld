package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.ro.adapters.JdkHttpServerAdapter;
import gen.ro.drafts.drafts.Draft;
import gen.ro.drafts.server.CreateRequest;
import gen.ro.drafts.server.DraftsHandler;
import gen.ro.drafts.server.DraftsHooks;
import gen.ro.drafts.server.DraftsService;
import gen.ro.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

/**
 * A request body whose model has a required {@code @readonly} field, over HTTP to the JDK server.
 * Arguments are bodies; each is sent as {@code POST /drafts} and prints one line {@code status
 * body}. The service answers with the body it was given.
 */
public final class ReadonlyJdk {
  private ReadonlyJdk() {}

  public static void main(String[] args) throws Exception {
    DraftsService impl = new DraftsService() {
      @Override
      public Draft create(CreateRequest request, RequestContext ctx) {
        return request.body();
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        DraftsHandler.create(impl, new DraftsHooks()), new InetSocketAddress("127.0.0.1", 0));
    try {
      HttpClient client = HttpClient.newHttpClient();
      for (String body : args) {
        HttpRequest request = HttpRequest.newBuilder(
                URI.create("http://127.0.0.1:" + server.getAddress().getPort() + "/drafts"))
            .header("content-type", "application/json")
            .POST(HttpRequest.BodyPublishers.ofString(body))
            .build();
        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
        System.out.println(response.statusCode() + " " + response.body());
      }
    } finally {
      server.stop(0);
    }
  }
}
