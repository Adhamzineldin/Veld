package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.sl.adapters.JdkHttpServerAdapter;
import gen.sl.shop.items.Item;
import gen.sl.shop.server.CountRequest;
import gen.sl.shop.server.CreateRequest;
import gen.sl.shop.server.ItemsHandler;
import gen.sl.shop.server.ItemsHooks;
import gen.sl.shop.server.ItemsService;
import gen.sl.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

/**
 * The JDK server of {@code contract/slash/items.veld}. Arguments come in triples (method, path,
 * body); each request prints one line {@code status body}.
 */
public final class SlashJdk {
  private SlashJdk() {}

  public static void main(String[] args) throws Exception {
    ItemsService impl = new ItemsService() {
      @Override
      public Item create(CreateRequest request, RequestContext ctx) {
        return request.body();
      }

      @Override
      public Integer count(CountRequest request, RequestContext ctx) {
        return 7;
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        ItemsHandler.create(impl, new ItemsHooks()), new InetSocketAddress("127.0.0.1", 0));
    try {
      HttpClient client = HttpClient.newHttpClient();
      for (int i = 0; i + 2 < args.length; i += 3) {
        HttpRequest request = HttpRequest.newBuilder(
                URI.create("http://127.0.0.1:" + server.getAddress().getPort() + args[i + 1]))
            .header("content-type", "application/json")
            .method(args[i], HttpRequest.BodyPublishers.ofString(args[i + 2]))
            .build();
        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
        System.out.println(response.statusCode() + " " + response.body());
      }
    } finally {
      server.stop(0);
    }
  }
}
