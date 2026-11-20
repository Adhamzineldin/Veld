package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.a.runtime.VeldHandler;
import gen.a.runtime.VeldRequest;
import gen.a.runtime.VeldResponse;
import gen.i.adapters.JdkHttpServerAdapter;
import gen.i.iam.server.GetUserRequest;
import gen.i.iam.server.IamHandler;
import gen.i.iam.server.IamService;
import gen.i.iam.users.User;
import gen.i.iam.users.UserBanned;
import gen.i.prelude.NotFound;
import gen.i.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/** Round trip: the generated Java server on the JDK adapter, every endpoint called through the generated Java client. */
public final class Roundtrip {
  private Roundtrip() {}

  private static final UUID BANNED_ID = UUID.fromString("22222222-2222-4222-8222-222222222222");

  public static void main(String[] args) throws Exception {
    ServerImpl impl = new ServerImpl();
    HttpServer server = gen.a.adapters.JdkHttpServerAdapter.listen(
        impl.handler(), new InetSocketAddress("127.0.0.1", 0));
    String base = "http://127.0.0.1:" + server.getAddress().getPort();
    Scenario.run("jdk", base, new Scenario.Probe() {
      @Override
      public int middlewareCalls() {
        return impl.rateLimitCalls.get();
      }

      @Override
      public int requests() {
        return impl.requests.get();
      }

      @Override
      public List<Throwable> thrown() {
        return impl.thrown;
      }

      @Override
      public List<String> seen() {
        return impl.seen;
      }
    });

    // The handler core is framework-agnostic: no match is an empty Optional, and the JDK adapter turns it into a 404.
    VeldHandler core = new ServerImpl().handler();
    Optional<VeldResponse> none = core.handle(new VeldRequest("GET", "/nothing", null, Map.of(), null, null));
    Check.that(none.isEmpty(), "unmatched route gives an empty Optional");
    Optional<VeldResponse> head = core.handle(new VeldRequest("HEAD", "/orders/health", null, Map.of(), null, null));
    Check.that(head.isEmpty(), "HEAD is not implicit");
    Check.equal(204, core.handle(new VeldRequest("GET", "/orders/health/", null, Map.of(), null, null)).orElseThrow().status(), "trailing slash");
    Check.equal(404, raw404(base + "/nothing"), "adapter 404");

    iam();
    noBaseUrl();
    server.stop(0);
    System.out.println("roundtrip ok");
  }

  private static int raw404(String url) throws Exception {
    var res = java.net.http.HttpClient.newHttpClient()
        .send(java.net.http.HttpRequest.newBuilder(java.net.URI.create(url)).build(), java.net.http.HttpResponse.BodyHandlers.ofString());
    return res.statusCode();
  }

  /** The consumed Iam client (inside the Orders server tree) against a second generated server. */
  private static void iam() throws Exception {
    User user = new User(Scenario.USER_ID, "ada@example.com", "Ada");
    IamService impl = new IamService() {
      @Override
      public User getUser(GetUserRequest request, RequestContext ctx) {
        if (request.id().equals(BANNED_ID)) {
          throw new UserBanned(new UserBanned.Data("spam"));
        }
        if (!request.id().equals(Scenario.USER_ID)) {
          throw new NotFound(new NotFound.Data("user"));
        }
        return user;
      }
    };
    HttpServer iam = JdkHttpServerAdapter.listen(IamHandler.create(impl), new InetSocketAddress("127.0.0.1", 0));
    String base = "http://127.0.0.1:" + iam.getAddress().getPort();
    var client = new gen.a.clients.iam.iam.client.IamClient(
        gen.a.runtime.ClientOptions.builder().baseUrl(base).build());
    var got = client.getUser(new gen.a.clients.iam.iam.client.GetUserRequest(Scenario.USER_ID));
    Check.equal("Ada", got.displayName(), "iam user");
    var missing = Check.thrown(gen.a.clients.iam.prelude.NotFound.class, "iam NotFound",
        () -> client.getUser(new gen.a.clients.iam.iam.client.GetUserRequest(UUID.randomUUID())));
    Check.that(gen.a.clients.iam.prelude.NotFound.matches(missing), "iam NotFound matches");
    var banned = Check.thrown(gen.a.clients.iam.iam.users.UserBanned.class, "iam UserBanned",
        () -> client.getUser(new gen.a.clients.iam.iam.client.GetUserRequest(BANNED_ID)));
    Check.equal(403, banned.status(), "UserBanned status");
    Check.equal("spam", banned.data().reason(), "UserBanned data");
    iam.stop(0);
  }

  /** The contract bakes in the Iam base URL; Orders has none, so constructing its client fails. */
  private static void noBaseUrl() {
    Check.that(new gen.a.clients.iam.iam.client.IamClient() != null, "baked-in base URL is used when nothing else is given");
    Check.thrown(gen.ca.runtime.VeldTransportError.class, "no base URL", () -> new gen.ca.orders.client.OrdersClient());
  }
}
