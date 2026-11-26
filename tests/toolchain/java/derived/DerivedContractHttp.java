package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.dshop.server.CreateCartRequest;
import gen.b.dshop.server.PayFacts;
import gen.b.dshop.server.PayRequest;
import gen.b.dshop.server.ReceiptsRequest;
import gen.b.dshop.server.RefuseRequest;
import gen.b.dshop.server.SaveDraftRequest;
import gen.b.dshop.server.ShopFactLoaders;
import gen.b.dshop.server.ShopHandler;
import gen.b.dshop.server.ShopHooks;
import gen.b.dshop.server.ShopService;
import gen.b.dshop.dshop.Cart;
import gen.b.dshop.dshop.Draft;
import gen.b.dshop.dshop.Receipt;
import gen.b.dshop.dshop.User;
import gen.b.dshop.dshop.Wallet;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.SseEvent;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.stream.Stream;

/**
 * The shared contract {@code tests/toolchain/contract/derived/shop.veld} (the one the Node driver
 * {@code derived_http.ts} runs) over HTTP on the JDK server: the cases of its two later additions.
 * {@code saveDraft} (a {@code Draft} has a {@code @readonly} member and a derived field that reads
 * it: what the caller sent for the member is dropped, {@code double} is computed, {@code next} stays
 * absent and its rule is skipped; a response is computed whole), the principal completed for an
 * endpoint no rule reads it in ({@code level} and the derived {@code share} of {@code User}), and a
 * principal whose {@code share} cannot be computed ({@code level} 0): {@code 500 Internal}, the
 * handler not called and the violation reported. Prints {@code OK}.
 */
public final class DerivedContractHttp {
  private DerivedContractHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000c1");
  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static final AtomicInteger CALLS = new AtomicInteger();
  private static final AtomicReference<Draft> DRAFT = new AtomicReference<>();
  private static final AtomicReference<User> CALLER = new AtomicReference<>();
  private static final List<RuleWalk.Event> EVENTS = Collections.synchronizedList(new ArrayList<>());

  private static final class Impl implements ShopService {
    @Override
    public Cart createCart(CreateCartRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      CALLER.set((User) ctx.principal());
      return request.body();
    }

    @Override
    public Receipt pay(PayRequest request, PayFacts facts, RequestContext ctx) {
      throw new UnsupportedOperationException("not part of this driver");
    }

    @Override
    public Draft saveDraft(SaveDraftRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      DRAFT.set(request.body());
      return new Draft(1, request.body().qty(), null, null);
    }

    @Override
    public Stream<SseEvent<Receipt>> receipts(ReceiptsRequest request, RequestContext ctx) {
      throw new UnsupportedOperationException("not part of this driver");
    }

    @Override
    public Receipt refuse(RefuseRequest request, RequestContext ctx) {
      throw new UnsupportedOperationException("not part of this driver");
    }
  }

  private record Answer(int status, String text) {
    Json.Obj body() {
      return Json.parse(text).asObject();
    }
  }

  private static Answer post(HttpServer server, String path, String token, String body) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path))
        .header("authorization", "Bearer " + token)
        .header("content-type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(body))
        .build();
    HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
    return new Answer(response.statusCode(), response.body());
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    ShopFactLoaders loaders = new ShopFactLoaders() {
      @Override
      public Optional<Wallet> wallet(UUID key, RequestContext ctx) {
        return Optional.empty();
      }
    };
    // The hook knows the caller's level and nothing derived: `tag` and `share` are computed for it.
    ShopHooks hooks = new ShopHooks((token, ctx) -> new User(ID, token, null, token.equals("broken") ? 0 : 4, null))
        .onRuleViolation(EVENTS::add);
    HttpServer server = JdkHttpServerAdapter.listen(
        ShopHandler.create(new Impl(), loaders, hooks), new InetSocketAddress("127.0.0.1", 0));
    try {
      aReadonlyMemberKeepsItsReaderAbsent(server);
      thePrincipalIsCompletedForEveryEndpoint(server);
      aPrincipalThatCannotBeCompletedIsA500(server);
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  /** `serial` is dropped from the request, so `next` (which reads it) is absent and its rule is skipped; `double` is computed. */
  private static void aReadonlyMemberKeepsItsReaderAbsent(HttpServer server) throws Exception {
    EVENTS.clear();
    Answer saved = post(server, "/shop/drafts", "alice", "{\"serial\":-5,\"qty\":2}");
    check(saved.status() == 201, "the draft is accepted, the rule that reads the absent field is skipped: " + saved.status() + saved.text());
    Draft got = DRAFT.get();
    check(got.qty() == 2 && got.double_() != null && got.double_() == 4, "the server computed `double`: " + got);
    check(got.next() == null, "`next` reads a read-only member, so it stays absent: " + got);
    check(EVENTS.isEmpty(), "nothing is reported: " + EVENTS);
    check(saved.body().get("double").asDouble() == 4, "the response is computed: " + saved.text());
    check(saved.body().get("next").asDouble() == 2, "a response is not a request, `next` is computed from its serial: " + saved.text());
  }

  /** The handler of `createCart` (no rule reads the principal) gets the completed caller. */
  private static void thePrincipalIsCompletedForEveryEndpoint(HttpServer server) throws Exception {
    Answer made = post(server, "/shop/carts", "alice", "{\"items\":[{\"price\":3,\"qty\":2}]}");
    check(made.status() == 201, "createCart: " + made.status() + made.text());
    User caller = CALLER.get();
    check("alice".equals(caller.tag()), "tag is the name: " + caller);
    check(caller.share() != null && caller.share() == 25, "share is 100 / level: " + caller);
  }

  /** A caller whose `share` divides by a level of 0 is the application's mistake: 500, reported, not handled. */
  private static void aPrincipalThatCannotBeCompletedIsA500(HttpServer server) throws Exception {
    EVENTS.clear();
    int calls = CALLS.get();
    Answer broken = post(server, "/shop/carts", "broken", "{\"items\":[{\"price\":3,\"qty\":2}]}");
    check(broken.status() == 500, "500: " + broken.status() + broken.text());
    check(CALLS.get() == calls, "the handler is not called");
    check(EVENTS.size() == 1, "reported once: " + EVENTS);
    RuleWalk.Event event = EVENTS.get(0);
    check(event.position() == RuleWalk.Position.OUTPUT && event.outcome() == RuleWalk.Outcome.REJECTED, "as an output violation: " + event);
    check(event.rule().equals(User.DERIVED_SHARE), "naming the field: " + event.rule());
    check("division_by_zero".equals(event.violation().error()), "with the error: " + event.violation());
  }
}
