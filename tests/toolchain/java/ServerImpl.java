package veldtest;

import gen.a.adapters.JdkHttpServerAdapter;
import gen.a.iam.users.User;
import gen.a.orders.catalog.Money;
import gen.a.orders.catalog.OrderItem;
import gen.a.orders.catalog.Status;
import gen.a.orders.orders.OutOfStock;
import gen.a.orders.orders.Order;
import gen.a.orders.orders.QuotaExceeded;
import gen.a.orders.server.BoomRequest;
import gen.a.orders.server.CancelOrderRequest;
import gen.a.orders.server.CreateOrderRequest;
import gen.a.orders.server.GetOrderRequest;
import gen.a.orders.server.HealthRequest;
import gen.a.orders.server.ListOrdersRequest;
import gen.a.orders.server.OrdersHandler;
import gen.a.orders.server.OrdersHooks;
import gen.a.orders.server.OrdersService;
import gen.a.orders.server.PayOrderRequest;
import gen.a.orders.server.SearchOrdersRequest;
import gen.a.prelude.NotFound;
import gen.a.runtime.Page;
import gen.a.runtime.RequestContext;
import gen.a.runtime.VeldHandler;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.math.BigDecimal;
import java.net.InetSocketAddress;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

/** The Orders implementation behind the generated Java server; mirrors tests/toolchain/ts/roundtrip.ts. */
final class ServerImpl implements OrdersService {
  static final UUID USER_ID = UUID.fromString("11111111-1111-4111-8111-111111111111");

  final Map<UUID, Order> store = new ConcurrentHashMap<>();
  final List<Throwable> thrown = Collections.synchronizedList(new ArrayList<>());
  final List<String> seen = Collections.synchronizedList(new ArrayList<>());
  final AtomicInteger rateLimitCalls = new AtomicInteger();
  final AtomicInteger requests = new AtomicInteger();

  private final User user = new User(USER_ID, "ada@example.com", "Ada");

  @Override
  public Order getOrder(GetOrderRequest request, RequestContext ctx) {
    Order order = store.get(request.id());
    if (order == null) {
      throw new NotFound(new NotFound.Data("order"));
    }
    return order;
  }

  @Override
  public Order createOrder(CreateOrderRequest request, RequestContext ctx) {
    Check.equal("u1", ctx.principal(), "principal reaches the implementation");
    if (request.body().items().stream().anyMatch(i -> i.sku().equals("OUT-1"))) {
      throw new OutOfStock(new OutOfStock.Data("OUT-1"));
    }
    if ("quota".equals(request.idempotencyKey())) {
      throw new QuotaExceeded(new QuotaExceeded.Data(5), 429, null);
    }
    String note = request.idempotencyKey() != null ? request.idempotencyKey() : request.body().note();
    Order order = new Order(
        Instant.parse("2024-05-01T10:00:00Z"),
        UUID.randomUUID(),
        new User(request.body().customerId(), user.email(), user.displayName()),
        request.body().items(),
        Status.PENDING,
        note,
        request.body().payment(),
        new Money(new BigDecimal("10.50"), "USD"),
        9007199254740993L,
        null,
        null);
    store.put(order.id(), order);
    return order;
  }

  @Override
  public Page<Order> listOrders(ListOrdersRequest request, RequestContext ctx) {
    var filter = request.query();
    List<Order> items = new ArrayList<>();
    for (Order o : store.values()) {
      if (filter.status() == null || o.status() == filter.status()) {
        items.add(o);
      }
    }
    int limit = filter.limit() == null ? 100 : filter.limit();
    List<Order> limited = items.subList(0, Math.min(limit, items.size()));
    if (filter.tag() != null) {
      seen.add("tags=" + String.join("|", filter.tag()));
    }
    return new Page<>(new ArrayList<>(limited), items.size(), 1, limited.size());
  }

  @Override
  public List<Order> searchOrders(SearchOrdersRequest request, RequestContext ctx) {
    seen.add("q=" + request.q() + ";limit=" + request.limit());
    List<Order> out = new ArrayList<>();
    for (Order o : store.values()) {
      if ((o.note() == null ? "" : o.note()).contains(request.q())) {
        out.add(o);
      }
    }
    return out;
  }

  @Override
  public Order payOrder(PayOrderRequest request, RequestContext ctx) {
    Order o = store.get(request.id());
    if (o == null) {
      throw new NotFound(new NotFound.Data(null));
    }
    Order paid = new Order(o.createdAt(), o.id(), o.customer(), o.items(), Status.PAID, o.note(), request.body(),
        o.total(), o.seq(), o.attributes(), o.extra());
    store.put(paid.id(), paid);
    return paid;
  }

  @Override
  public void cancelOrder(CancelOrderRequest request, RequestContext ctx) {
    if (store.remove(request.id()) == null) {
      throw new NotFound(new NotFound.Data("order"));
    }
  }

  @Override
  public void health(HealthRequest request, RequestContext ctx) {}

  @Override
  public Order boom(BoomRequest request, RequestContext ctx) {
    throw new IllegalStateException("secret internal detail");
  }

  OrdersHooks hooks() {
    return new OrdersHooks(
            (token, ctx) -> "good".equals(token) ? "u1" : null,
            ctx -> rateLimitCalls.incrementAndGet())
        .onError((err, ctx) -> thrown.add(err));
  }

  VeldHandler handler() {
    VeldHandler core = OrdersHandler.create(this, hooks());
    return request -> {
      requests.incrementAndGet();
      return core.handle(request);
    };
  }

  /** Starts the server on a free port. */
  HttpServer listen() throws IOException {
    return JdkHttpServerAdapter.listen(handler(), new InetSocketAddress("127.0.0.1", 0));
  }
}
