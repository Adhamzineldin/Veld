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
import gen.b.shop.shop.Line;
import gen.b.shop.shop.Receipt;
import gen.b.shop.shop.Wallet;
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

/**
 * The production handler of {@code tests/rules/endpoints/derived.veld.pending} over HTTP on the JDK
 * server (spec/rules.md section 17.2): a value the caller sent for a derived field is replaced, a
 * malformed one is dropped and is not a 422, a derived value is read by a {@code require} and by an
 * invariant, a fact is given its derived field, a derived field of a fact that fails is 500, an
 * expression that fails in a request is a 422 with the violation of the field, the value the handler
 * put in a derived field is overwritten, an {@code ensure} reads the derived field of the result,
 * the output mode {@code off} still computes and {@code audit} sends the value without the field
 * that failed.
 */
public final class DerivedShopHttp {
  private DerivedShopHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000e1");
  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static volatile Wallet wallet = new Wallet(ID, 100, 20, 12345);
  private static volatile Receipt receipt = new Receipt(ID, 100, 30, 12345);
  private static final AtomicInteger CALLS = new AtomicInteger();
  private static final AtomicReference<PayRequest> REQUEST = new AtomicReference<>();
  private static final AtomicReference<PayFacts> FACTS = new AtomicReference<>();
  private static final List<RuleWalk.Event> EVENTS = Collections.synchronizedList(new ArrayList<>());

  private static final class Loaders implements ShopFactLoaders {
    @Override
    public Optional<Wallet> wallet(UUID key, RequestContext ctx) {
      return Optional.of(wallet);
    }
  }

  private static final class Impl implements ShopService {
    @Override
    public Receipt pay(PayRequest request, PayFacts facts, RequestContext ctx) {
      CALLS.incrementAndGet();
      REQUEST.set(request);
      FACTS.set(facts);
      return receipt;
    }
  }

  private record Answer(int status, String text) {
    Json.Obj body() {
      return Json.parse(text).asObject();
    }
  }

  private static HttpServer serve(RuleWalk.OutputMode mode) throws Exception {
    ShopHooks hooks = new ShopHooks().onRuleViolation(EVENTS::add);
    return JdkHttpServerAdapter.listen(
        ShopHandler.create(
            new Impl(), new Loaders(), hooks, gen.b.runtimeconfig.GenBRuntime.builder().rulesOutput(mode).build()),
        new InetSocketAddress("127.0.0.1", 0));
  }

  private static Answer pay(HttpServer server, String body) throws Exception {
    HttpRequest request = HttpRequest.newBuilder(
            URI.create("http://127.0.0.1:" + server.getAddress().getPort() + "/shop/pay/" + ID))
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

  private static String cart(String items, String more) {
    return "{\"items\":" + items + more + "}";
  }

  private static void reset() {
    wallet = new Wallet(ID, 100, 20, 12345);
    receipt = new Receipt(ID, 100, 30, 12345);
    EVENTS.clear();
  }

  public static void main(String[] args) throws Exception {
    HttpServer strict = serve(RuleWalk.OutputMode.STRICT);
    HttpServer audit = serve(RuleWalk.OutputMode.AUDIT);
    HttpServer off = serve(RuleWalk.OutputMode.OFF);
    try {
      aValueTheCallerSentIsReplacedOrDropped(strict);
      aRequireReadsADerivedValue(strict);
      aFactIsGivenItsDerivedField(strict);
      anExpressionThatFailsInARequest(strict);
      theHandlersValueIsOverwritten(strict);
      theOutputModes(strict, audit, off);
    } finally {
      strict.stop(0);
      audit.stop(0);
      off.stop(0);
    }
    System.out.println("OK");
  }

  /** What the caller sent for a derived field is replaced, and a malformed value is dropped, not a 422. */
  private static void aValueTheCallerSentIsReplacedOrDropped(HttpServer server) throws Exception {
    reset();
    String items = "[{\"price\":30,\"qty\":2,\"total\":999}]";
    Answer sent = pay(server, cart(items, ",\"subtotal\":1,\"payable\":2"));
    check(sent.status() == 200, "the values the caller sent are ignored: " + sent.status() + sent.text());
    Line line = REQUEST.get().body().items().get(0);
    check(line.total() == 60, "the item total is computed: " + line.total());
    check(REQUEST.get().body().subtotal() == 60 && REQUEST.get().body().payable() == 60, "the handler gets the computed values");
    check(REQUEST.get().body().coupon() == 0, "the default is applied before the derived fields");

    int calls = CALLS.get();
    String junk = "[{\"price\":30,\"qty\":2,\"total\":\"x\"}]";
    Answer malformed = pay(server, cart(junk, ",\"payable\":\"y\",\"subtotal\":null"));
    check(malformed.status() == 200, "a malformed value is dropped, not a 422: " + malformed.status() + malformed.text());
    check(CALLS.get() == calls + 1 && REQUEST.get().body().payable() == 60, "and the field is computed");
  }

  /** A `require` and an invariant read the derived values, and the `else` payload computes its own. */
  private static void aRequireReadsADerivedValue(HttpServer server) throws Exception {
    reset();
    Answer short_ = pay(server, cart("[{\"price\":50,\"qty\":2}]", ""));
    check(short_.status() == 402 && short_.body().get("title").asString().equals("NotEnough"), "the require fails: " + short_.status());
    Json.Obj data = short_.body().get("data").asObject();
    check(data.get("needed").asDouble() == 100 && data.get("have").asDouble() == 80, "the computed numbers: " + short_.text());
    check(data.get("short").asDouble() == 20, "the error is given its derived field: " + short_.text());
    int calls = CALLS.get();
    Answer coupon = pay(server, cart("[{\"price\":5,\"qty\":2}]", ",\"coupon\":50"));
    check(coupon.status() == 422 && coupon.body().get("violations").asArray().items().size() == 1, "an invariant reads payable: " + coupon.text());
    check(CALLS.get() == calls, "the handler did not run");
  }

  /** A fact is given its derived field after it is loaded, whatever the loader put there, and a failure is 500. */
  private static void aFactIsGivenItsDerivedField(HttpServer server) throws Exception {
    reset();
    wallet = new Wallet(ID, 100, 100, 999);
    Answer none = pay(server, cart("[{\"price\":1,\"qty\":1}]", ""));
    check(none.status() == 402, "the derived field of the fact is computed, not the loader's 999: " + none.status());
    check(none.body().get("data").asObject().get("have").asDouble() == 0, none.text());

    reset();
    wallet = new Wallet(ID, 100, 20, null);
    int calls = CALLS.get();
    Answer ok = pay(server, cart("[]", ""));
    check(ok.status() == 200 && FACTS.get().wallet().available() == 80, "the handler's fact has it: " + ok.status());
    check(CALLS.get() == calls + 1, "the handler ran");

    wallet = new Wallet(ID, Integer.MIN_VALUE, 1, null);
    EVENTS.clear();
    calls = CALLS.get();
    Answer broken = pay(server, cart("[]", ""));
    check(broken.status() == 500 && broken.body().get("title").asString().equals("Internal"), "a fact that cannot be computed is 500: " + broken.status());
    check(CALLS.get() == calls, "the handler did not run");
    check(!broken.text().contains("overflow"), "nothing leaks: " + broken.text());
    check(EVENTS.size() == 1, "the hook heard it: " + EVENTS);
    RuleWalk.Event event = EVENTS.get(0);
    check(event.rule().equals(gen.b.shop.shop.Wallet.DERIVED_AVAILABLE), "the rule is the id of the field: " + event.rule());
    check("overflow".equals(event.violation().error()), "the error is set: " + event.violation());
    check(event.position() == RuleWalk.Position.OUTPUT && event.outcome() == RuleWalk.Outcome.REJECTED, "an output violation");
  }

  /** An expression of the request that raises an evaluation error is a 422 with the violation of its field. */
  private static void anExpressionThatFailsInARequest(HttpServer server) throws Exception {
    reset();
    int calls = CALLS.get();
    Answer boom = pay(server, cart("[{\"price\":100000,\"qty\":100000}]", ""));
    check(boom.status() == 422 && boom.body().get("title").asString().equals("ValidationError"), "422: " + boom.status() + boom.text());
    Json.Arr violations = boom.body().get("violations").asArray();
    check(violations.items().size() == 1, "one violation, the readers of the absent field are skipped: " + boom.text());
    Json.Obj v = violations.items().get(0).asObject();
    check(v.get("rule").asString().equals(Line.DERIVED_TOTAL), "the rule is the id of the field: " + v.write());
    check(v.get("path").asString().equals("body.items[0]"), v.write());
    check(v.get("error").asString().equals("overflow"), v.write());
    check(CALLS.get() == calls, "the handler did not run");
  }

  /** What the handler put in a derived field of the result is replaced. */
  private static void theHandlersValueIsOverwritten(HttpServer server) throws Exception {
    reset();
    Answer ok = pay(server, cart("[{\"price\":30,\"qty\":2}]", ""));
    check(ok.status() == 200 && ok.body().get("net").asDouble() == 70, "net is computed, not the 12345 of the handler: " + ok.text());
    receipt = new Receipt(ID, 10, 30, null);
    Answer ensure = pay(server, cart("[]", ""));
    check(ensure.status() == 500, "an ensure reads the derived field of the result: " + ensure.status());
    check(EVENTS.stream().anyMatch(e -> e.violation().message().equals("A receipt never has a negative net")), "the ensure was reported");
  }

  /** `off` still computes, `audit` sends the value without the field that failed, `strict` is 500. */
  private static void theOutputModes(HttpServer strict, HttpServer audit, HttpServer off) throws Exception {
    reset();
    receipt = new Receipt(ID, 10, 30, null);
    Answer computed = pay(off, cart("[]", ""));
    check(computed.status() == 200 && computed.body().get("net").asDouble() == -20, "off still computes: " + computed.text());
    check(EVENTS.isEmpty(), "and reports nothing: " + EVENTS);

    receipt = new Receipt(ID, Integer.MIN_VALUE, 1, null);
    EVENTS.clear();
    Answer rejected = pay(strict, cart("[]", ""));
    check(rejected.status() == 500, "a derived field of the result that fails is 500 in strict: " + rejected.status());
    check(EVENTS.size() == 1 && EVENTS.get(0).rule().equals(Receipt.DERIVED_NET), "reported with the id of the field: " + EVENTS);
    check(EVENTS.get(0).outcome() == RuleWalk.Outcome.REJECTED, "rejected");

    EVENTS.clear();
    Answer audited = pay(audit, cart("[]", ""));
    check(audited.status() == 200 && audited.body().get("net") == null, "audit sends it without the field: " + audited.text());
    check(EVENTS.size() == 1 && EVENTS.get(0).outcome() == RuleWalk.Outcome.AUDITED, "and reports it: " + EVENTS);
    check("overflow".equals(EVENTS.get(0).violation().error()), "with the error");
  }
}
