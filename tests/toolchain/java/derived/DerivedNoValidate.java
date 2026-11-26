package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.shop.server.PayFacts;
import gen.b.shop.server.PayRequest;
import gen.b.shop.server.ShopFactLoaders;
import gen.b.shop.server.ShopHandler;
import gen.b.shop.server.ShopHooks;
import gen.b.shop.server.ShopService;
import gen.b.shop.shop.Receipt;
import gen.b.shop.shop.Wallet;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;

/**
 * The production handler of {@code tests/rules/endpoints/derived.veld.pending} generated with
 * {@code validate = false} (spec/rules.md section 5.3): the rules of the request are off, but the
 * derived fields are still computed, because the handler's type holds them; a derived field of the
 * request that cannot be computed is left absent and is neither a {@code 422} nor, later, a
 * violation of the output (the failure of a request is not the server's own).
 */
public final class DerivedNoValidate {
  private DerivedNoValidate() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000e2");
  private static final AtomicReference<PayRequest> REQUEST = new AtomicReference<>();

  public static void main(String[] args) throws Exception {
    ShopService impl = new ShopService() {
      @Override
      public Receipt pay(PayRequest request, PayFacts facts, RequestContext ctx) {
        REQUEST.set(request);
        return new Receipt(ID, 100, 30, null);
      }
    };
    ShopFactLoaders loaders = new ShopFactLoaders() {
      @Override
      public Optional<Wallet> wallet(UUID key, RequestContext ctx) {
        return Optional.of(new Wallet(ID, 100, 20, null));
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        ShopHandler.create(
            impl,
            loaders,
            new ShopHooks(),
            gen.b.runtimeconfig.GenBRuntime.builder().rulesOutput(RuleWalk.OutputMode.STRICT).build()),
        new InetSocketAddress("127.0.0.1", 0));
    try {
      String computed = send(server, "{\"items\":[{\"price\":30,\"qty\":2}]}");
      check(computed.startsWith("200 "), "a request is answered: " + computed);
      check(REQUEST.get().body().items().get(0).total() == 60, "the derived fields are computed: " + REQUEST.get());
      check(REQUEST.get().body().payable() == 60, "the chain too: " + REQUEST.get());
      check(Json.parse(computed.substring(4)).asObject().get("net").asDouble() == 70, "and the result's: " + computed);

      REQUEST.set(null);
      String broken = send(server, "{\"items\":[{\"price\":100000,\"qty\":100000}]}");
      check(broken.startsWith("200 "), "a derived field of the request that fails is not reported: " + broken);
      check(REQUEST.get() != null && REQUEST.get().body().items().get(0).total() == null, "it is absent: " + REQUEST.get());
      check(REQUEST.get().body().subtotal() == null, "and so is what reads it: " + REQUEST.get());
      check(Json.parse(broken.substring(4)).asObject().get("net").asDouble() == 70, "the output is not touched by it: " + broken);
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  private static String send(HttpServer server, String body) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + "/shop/pay/" + ID))
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(body))
        .build();
    HttpResponse<String> response = HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString());
    return response.statusCode() + " " + response.body();
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }
}
