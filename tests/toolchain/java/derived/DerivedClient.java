package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.c.runtime.ClientOptions;
import gen.c.runtime.Json;
import gen.c.runtime.VeldError;
import gen.c.runtime.Violation;
import gen.c.shop.client.PayRequest;
import gen.c.shop.client.ShopClient;
import gen.c.shop.shop.Cart;
import gen.c.shop.shop.Line;
import gen.c.shop.shop.Receipt;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * The generated client of {@code tests/rules/endpoints/derived.veld.pending} (spec/rules.md section
 * 17.2): its local check computes the derived fields of the request body before it checks the
 * invariants (a coupon above the subtotal is a local violation, nothing is sent), it never sends a
 * derived field (not one the caller filled in), it takes a derived value of a response as it is, and
 * every declaration with derived fields has the public function that computes them.
 */
public final class DerivedClient {
  private DerivedClient() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000a1");
  private static final List<String> BODIES = new CopyOnWriteArrayList<>();

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  /**
   * The client runs its local check only once the server has shown that it has the same rules (the
   * digest of the last response, spec/rules.md section 9.7), so the server sends it and the first call
   * is made to learn it.
   */
  public static void main(String[] args) throws Exception {
    String digest = args[0];
    HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    // A server that disagrees with the client about `net`, to show that the client keeps what it is told.
    server.createContext("/", exchange -> {
      BODIES.add(new String(exchange.getRequestBody().readAllBytes(), StandardCharsets.UTF_8));
      byte[] answer = ("{\"id\":\"" + ID + "\",\"amount\":10,\"fee\":3,\"net\":999}").getBytes(StandardCharsets.UTF_8);
      exchange.getResponseHeaders().add("content-type", "application/json");
      exchange.getResponseHeaders().add("Veld-Rules", digest);
      exchange.sendResponseHeaders(200, answer.length);
      exchange.getResponseBody().write(answer);
      exchange.close();
    });
    server.start();
    try {
      ShopClient client = new ShopClient(
          ClientOptions.builder().baseUrl("http://127.0.0.1:" + server.getAddress().getPort()).build());
      client.pay(new PayRequest(ID, new Cart(List.of(new Line(1, 1, null)), 0, null, null)));
      check(BODIES.size() == 1, "the first call is sent: " + BODIES);
      theLocalCheckComputesBeforeItChecks(client);
      aDerivedFieldIsNeverSent(client);
      aDerivedValueOfAResponseIsKept(client);
      thePublicFunction();
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  /** The invariant `payableFits` reads `payable`, which the client computed from the items and the coupon. */
  private static void theLocalCheckComputesBeforeItChecks(ShopClient client) {
    Cart cart = new Cart(List.of(new Line(5, 2, null)), 50, null, null);
    try {
      client.pay(new PayRequest(ID, cart));
      throw new AssertionError("a coupon above the subtotal was sent");
    } catch (VeldError e) {
      check(e.raisedLocally(), "raised locally");
      check(e.violations().size() == 1, "one violation: " + e.violations());
      check(e.violations().get(0).rule().equals(Cart.RULE_PAYABLE_FITS), "the rule of the invariant: " + e.violations());
    }
    check(BODIES.size() == 1, "nothing was sent: " + BODIES);
    Cart overflow = new Cart(List.of(new Line(100000, 100000, null)), 0, null, null);
    try {
      client.pay(new PayRequest(ID, overflow));
      throw new AssertionError("a request whose derived field cannot be computed was sent");
    } catch (VeldError e) {
      check(e.raisedLocally() && e.violations().size() == 1, "the violation of the field: " + e.violations());
      check(e.violations().get(0).rule().equals(Line.DERIVED_TOTAL), "the rule is the id of the field: " + e.violations());
      check("overflow".equals(e.violations().get(0).error()), "with the error");
    }
    check(BODIES.size() == 1, "still nothing was sent: " + BODIES);
  }

  /** A value the caller put in a derived field is not in the body of the request. */
  private static void aDerivedFieldIsNeverSent(ShopClient client) {
    Cart cart = new Cart(List.of(new Line(30, 2, 999)), 0, 888, 777);
    client.pay(new PayRequest(ID, cart));
    check(BODIES.size() == 2, "one more request: " + BODIES);
    Json.Obj sent = Json.parse(BODIES.get(1)).asObject();
    check(sent.get("subtotal") == null && sent.get("payable") == null, "no derived field of the cart: " + BODIES.get(1));
    Json.Obj line = sent.get("items").asArray().items().get(0).asObject();
    check(line.get("total") == null, "no derived field of the item: " + BODIES.get(1));
    check(line.get("price") != null && line.get("qty") != null, "the fields the caller owns are sent: " + BODIES.get(1));
  }

  /** What the server computed is what the client returns, even when the client would compute another value. */
  private static void aDerivedValueOfAResponseIsKept(ShopClient client) {
    Receipt receipt = client.pay(new PayRequest(ID, new Cart(List.of(new Line(1, 1, null)), 0, null, null)));
    check(receipt.net() == 999, "net is the server's, not 10 - 3: " + receipt.net());
  }

  /** `Cart.derive` is the code the server runs: a form can show the payable amount while the user types. */
  private static void thePublicFunction() {
    Cart draft = new Cart(List.of(new Line(30, 2, null), new Line(5, 1, 12345)), null, null, null);
    Cart computed = Cart.derive(draft);
    check(computed.items().get(0).total() == 60 && computed.items().get(1).total() == 5, "the items: " + computed);
    check(computed.coupon() == 0, "the default is applied: " + computed);
    check(computed.subtotal() == 65 && computed.payable() == 65, "the cart: " + computed);

    List<Violation> failed = new ArrayList<>();
    Cart broken = Cart.derive(new Cart(List.of(new Line(100000, 100000, null)), 0, null, null), failed);
    check(broken.items().get(0).total() == null && broken.subtotal() == null && broken.payable() == null, "stays null: " + broken);
    check(failed.size() == 1 && failed.get(0).rule().equals(Line.DERIVED_TOTAL), "reported once: " + failed);
  }
}
