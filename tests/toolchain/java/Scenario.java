package veldtest;

import gen.ca.orders.catalog.BankTransfer;
import gen.ca.orders.catalog.Card;
import gen.ca.orders.catalog.OrderItem;
import gen.ca.orders.catalog.Payment;
import gen.ca.orders.catalog.Status;
import gen.ca.orders.client.CancelOrderRequest;
import gen.ca.orders.client.CreateOrderRequest;
import gen.ca.orders.client.GetOrderRequest;
import gen.ca.orders.client.ListOrdersRequest;
import gen.ca.orders.client.OrdersClient;
import gen.ca.orders.client.PayOrderRequest;
import gen.ca.orders.client.SearchOrdersRequest;
import gen.ca.orders.orders.CreateOrder;
import gen.ca.orders.orders.Order;
import gen.ca.orders.orders.OrderFilter;
import gen.ca.orders.orders.OutOfStock;
import gen.ca.orders.orders.QuotaExceeded;
import gen.ca.prelude.Internal;
import gen.ca.prelude.NotFound;
import gen.ca.prelude.Unauthorized;
import gen.ca.prelude.ValidationError;
import gen.ca.runtime.ClientOptions;
import gen.ca.runtime.Json;
import gen.ca.runtime.Page;
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.stream.Collectors;

/**
 * The client side of the round trip: every endpoint called through the generated Java client.
 * It only assumes the behaviour of tests/toolchain/ts/roundtrip.ts's implementation, so it runs
 * against the Java server and against the Node server alike.
 */
final class Scenario {
  private Scenario() {}

  /** What a test can learn about the server it is talking to. Null when it runs in another process. */
  interface Probe {
    int middlewareCalls();

    int requests();

    List<Throwable> thrown();

    List<String> seen();
  }

  static final UUID USER_ID = UUID.fromString("11111111-1111-4111-8111-111111111111");

  /** The signature sets of the IR variants (written by the Rust test as sigs.json): a, c and d. */
  static Map<String, Json> sigsFile() throws IOException {
    return Json.parse(Files.readString(Path.of("sigs.json"))).asObject().fields();
  }

  /** Operation and declaration signatures of the standard contract. */
  static Map<String, Json> sigs() throws IOException {
    return sigsFile().get("a").asObject().fields();
  }

  private record Raw(int status, String text) {}

  private static Raw raw(String url, String method, Map<String, String> headers, String body) throws Exception {
    HttpRequest.Builder b = HttpRequest.newBuilder(URI.create(url));
    headers.forEach(b::header);
    b.method(method, body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
    HttpResponse<String> res =
        HttpClient.newHttpClient().send(b.build(), HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
    return new Raw(res.statusCode(), res.body());
  }

  private static OrdersClient client(String base, boolean validate, String token) {
    ClientOptions.Builder o = ClientOptions.builder().baseUrl(base).validate(validate);
    if (token != null) {
      o.bearer(() -> token);
    }
    return new OrdersClient(o.build());
  }

  private static CreateOrder newOrder(String sku, int quantity, Payment payment) {
    return new CreateOrder(USER_ID, List.of(new OrderItem(sku, quantity)), null, payment);
  }

  static void run(String label, String base, Probe probe) throws Exception {
    String op = sigs().get("Orders.getOrder").asString();
    OrdersClient client = client(base, true, "good");
    OrdersClient lax = client(base, false, "good");
    int middlewareBefore = probe == null ? 0 : probe.middlewareCalls();

    // health: no auth, 204
    client.health();

    // createOrder: 201, defaults applied, header param, union body field
    Order created = client.createOrder(new CreateOrderRequest(newOrder("ABC-1", 2, new Card("4242")), "key-1"));
    Check.equal(Status.PENDING, created.status(), label + ": default status");
    Check.equal("key-1", created.note(), label + ": header param reaches the implementation");
    Check.equal("Ada", created.customer().displayName(), label + ": customer");
    Check.equal(new Card("4242"), created.payment(), label + ": union value");
    Check.equal(9007199254740993L, created.seq(), label + ": long keeps all its digits");
    Check.equal(Instant.parse("2024-05-01T10:00:00Z"), created.createdAt(), label + ": datetime");

    // wire form uses @wire names
    Raw wire = raw(base + "/orders/" + created.id(), "GET", Map.of("authorization", "Bearer good"), null);
    Check.equal(200, wire.status(), label + ": raw get");
    Json.Obj body = Json.parse(wire.text()).asObject();
    Check.that(body.has("created_at"), label + ": @wire name created_at");
    Check.equal("Ada", body.get("customer").asObject().get("display_name").asString(), label + ": display_name");
    Check.equal("9007199254740993", body.get("seq").asString(), label + ": long is a string on the wire");

    // getOrder
    Check.equal(created, client.getOrder(new GetOrderRequest(created.id())), label + ": fetched equals created");

    // declared errors arrive as the right class with op set
    NotFound notFound = Check.thrown(NotFound.class, label + ": NotFound class", () -> client.getOrder(new GetOrderRequest(UUID.randomUUID())));
    Check.equal(op, notFound.op(), label + ": NotFound op");
    Check.equal(404, notFound.status(), label + ": NotFound status");
    Check.equal("order", notFound.data().resource(), label + ": NotFound data");
    Check.that(NotFound.matches(notFound), label + ": matches");
    OutOfStock outOfStock = Check.thrown(OutOfStock.class, label + ": OutOfStock", () -> client.createOrder(new CreateOrderRequest(newOrder("OUT-1", 1, null), null)));
    Check.equal(sigs().get("Orders.createOrder").asString(), outOfStock.op(), label + ": OutOfStock op");
    Check.equal(409, outOfStock.status(), label + ": OutOfStock status");
    Check.equal("OUT-1", outOfStock.data().sku(), label + ": OutOfStock data");
    QuotaExceeded quota = Check.thrown(QuotaExceeded.class, label + ": QuotaExceeded", () -> client.createOrder(new CreateOrderRequest(newOrder("ABC-1", 1, null), "quota")));
    Check.equal(429, quota.status(), label + ": use-site status");
    Check.equal(5, quota.data().limit(), label + ": QuotaExceeded data");

    // list with a query object (enum, repeated list keys, int)
    Page<Order> page = client.listOrders(new ListOrdersRequest(new OrderFilter(Status.PENDING, List.of("a", "b c"), 10)));
    Check.equal(1L, page.total(), label + ": page total");
    Check.equal(1, page.items().size(), label + ": page items");
    if (probe != null) {
      Check.that(probe.seen().contains("tags=a|b c"), label + ": repeated query keys");
    }
    Check.equal(0, client.listOrders(new ListOrdersRequest(new OrderFilter(Status.PAID, null, null))).items().size(), label + ": filter");

    // search with single query params and awkward characters
    Check.equal(1, client.searchOrders(new SearchOrdersRequest("key-1", null)).size(), label + ": search");
    client.createOrder(new CreateOrderRequest(newOrder("ABC-1", 1, null), "a b&c=d")).id();
    List<Order> awkward = client.searchOrders(new SearchOrdersRequest("a b&c=d", null));
    Check.equal(1, awkward.size(), label + ": query encoding round trip");
    Check.equal("a b&c=d", awkward.get(0).note(), label + ": query encoding value");
    client.searchOrders(new SearchOrdersRequest("x", 3));
    if (probe != null) {
      Check.that(probe.seen().contains("q=x;limit=3"), label + ": optional query param");
      Check.that(probe.seen().contains("q=key-1;limit=null"), label + ": omitted optional query param");
    }

    // pay: union body and union in the response
    Order paid = client.payOrder(new PayOrderRequest(created.id(), new BankTransfer("DE00")));
    Check.equal(Status.PAID, paid.status(), label + ": paid");
    Check.equal(new BankTransfer("DE00"), paid.payment(), label + ": union arm round trip");
    Check.that(NotFound.matches(Check.thrown(NotFound.class, label + ": pay unknown", () -> client.payOrder(new PayOrderRequest(UUID.randomUUID(), new BankTransfer("x"))))), label + ": pay NotFound");

    // cancel: 204, then NotFound
    client.cancelOrder(new CancelOrderRequest(created.id()));
    Check.thrown(NotFound.class, label + ": cancel twice", () -> client.cancelOrder(new CancelOrderRequest(created.id())));

    undeclared(label, base, client, probe);
    validation(label, base, client, lax, probe, op);
    auth(label, base, op);
    if (probe != null) {
      Check.that(probe.middlewareCalls() > middlewareBefore, label + ": middleware called");
    }
  }

  /** An undeclared failure surfaces as the declared prelude `Internal`, without leaking its message. */
  private static void undeclared(String label, String base, OrdersClient client, Probe probe) throws Exception {
    Internal internal = Check.thrown(Internal.class, label + ": boom is a declared Internal error", client::boom);
    Check.that(Internal.matches(internal), label + ": Internal.matches");
    Check.equal(500, internal.status(), label + ": Internal status");
    Check.that(!internal.data().toWire().write().contains("secret"), label + ": no leak in data");
    Raw boom = raw(base + "/orders/boom", "GET", Map.of(), null);
    Check.equal(500, boom.status(), label + ": raw boom status");
    Check.that(!boom.text().contains("secret"), label + ": no leak on the wire");
    if (probe != null) {
      Check.that(
          probe.thrown().stream().anyMatch(t -> "secret internal detail".equals(t.getMessage())),
          label + ": onError saw the original");
    }
  }

  private static void validation(String label, String base, OrdersClient client, OrdersClient lax, Probe probe, String op) throws Exception {
    CreateOrder bad = new CreateOrder(USER_ID, List.of(), null, null);
    CreateOrder badItem = new CreateOrder(USER_ID, List.of(new OrderItem("nope", 0)), null, null);
    int sent = probe == null ? 0 : probe.requests();
    ValidationError local = Check.thrown(ValidationError.class, label + ": local ValidationError", () -> client.createOrder(new CreateOrderRequest(bad, null)));
    Check.equal(sigs().get("Orders.createOrder").asString(), local.op(), label + ": local op");
    Check.that(local.data().issues().size() >= 1, label + ": local issues");
    ValidationError localItem = Check.thrown(ValidationError.class, label + ": local item errors", () -> client.createOrder(new CreateOrderRequest(badItem, null)));
    List<String> localPaths = localItem.data().issues().stream().map(i -> i.path() + ":" + i.code()).collect(Collectors.toList());
    Check.that(localPaths.contains("body.items[0].sku:pattern") && localPaths.contains("body.items[0].quantity:min"), label + ": " + localPaths);
    if (probe != null) {
      Check.equal(sent, probe.requests(), label + ": no request for a locally invalid call");
    }
    ValidationError remote = Check.thrown(ValidationError.class, label + ": remote ValidationError", () -> lax.createOrder(new CreateOrderRequest(bad, null)));
    Check.equal(422, remote.status(), label + ": remote status");
    Check.equal(sigs().get("Orders.createOrder").asString(), remote.op(), label + ": remote op");
    List<String> paths = remote.data().issues().stream().map(i -> i.path() + ":" + i.code()).collect(Collectors.toList());
    Check.that(paths.contains("body.items:min_length"), label + ": " + paths);
    ValidationError query = Check.thrown(ValidationError.class, label + ": remote query validation", () -> lax.listOrders(new ListOrdersRequest(new OrderFilter(null, null, 1000))));
    Check.that(query.data().issues().stream().anyMatch(i -> i.path().equals("query.limit") && i.code().equals("max")), label + ": query.limit max");
    Raw badJson = raw(base + "/orders", "POST", Map.of("authorization", "Bearer good", "content-type", "application/json"), "{not json");
    Check.equal(422, badJson.status(), label + ": bad JSON");
    Check.equal("ValidationError", Json.parse(badJson.text()).asObject().get("title").asString(), label + ": bad JSON title");
  }

  private static void auth(String label, String base, String op) throws Exception {
    OrdersClient anon = client(base, true, null);
    Unauthorized missing = Check.thrown(Unauthorized.class, label + ": missing token", () -> anon.getOrder(new GetOrderRequest(UUID.randomUUID())));
    Check.equal(401, missing.status(), label + ": 401");
    Check.equal(op, missing.op(), label + ": 401 op");
    OrdersClient wrong = client(base, true, "bad");
    Check.that(Unauthorized.matches(Check.thrown(Unauthorized.class, label + ": wrong token", () -> wrong.listOrders(new ListOrdersRequest(new OrderFilter(null, null, null))))), label + ": rejected token");
    anon.health();
  }
}
