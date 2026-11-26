package veldtest;

import gen.b.bookings.no_values.Booking;
import gen.b.bookings.no_values.Shape;
import gen.b.bookings.no_values.Status;
import gen.b.bookings.no_values.User;
import gen.b.bookings.server.ByIdRequest;
import gen.b.bookings.server.ByNameRequest;
import gen.b.bookings.server.PayFacts;
import gen.b.bookings.server.PayRequest;
import gen.b.bookings.server.ProbeFactLoaders;
import gen.b.bookings.server.ProbeHandler;
import gen.b.bookings.server.ProbeHooks;
import gen.b.bookings.server.ProbeService;
import gen.b.bookings.server.ShapesRequest;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Platform phase P4, part 1 (spec/rules.md 19.3): no message repeats a value. A marker string goes
 * through every path of the generated server and client that writes a message, a {@code detail}, a
 * {@code title} or an exception text (validation issues of every code, violations of every kind of
 * rule, a refused transition, version, policy and idempotency key, a client that fails to connect,
 * to read or to be answered), and every text that comes out is searched for the marker. Paths and
 * names may hold it ({@code $.extra.<key>} names the map key it is about); texts may not. Expects
 * the server in {@code gen.b} and the client in {@code gen.c}, both generated from
 * tests/toolchain/contract/platform_p4/no_values.veld (package {@code bookings}). Prints {@code OK}.
 */
public final class NoValues {
  private NoValues() {}

  private static final String MARKER = "MARKER-3f9a71c2e5";
  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID CALLER = UUID.fromString("00000000-0000-4000-8000-0000000000aa");

  /** Every text the runtime wrote, labelled by where it came from. */
  private static final List<String[]> TEXTS = new ArrayList<>();

  private static void note(String where, String text) {
    if (text != null) {
      TEXTS.add(new String[] {where, text});
    }
  }

  private static void note(String where, Json text) {
    if (text instanceof Json.Str s) {
      note(where, s.value());
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  /** The texts of a problem document: title, detail, issues, violations and the message of a denial. */
  private static void problemTexts(String where, String body) {
    if (body == null || body.isEmpty()) {
      return;
    }
    Json parsed;
    try {
      parsed = Json.parse(body);
    } catch (Json.ParseException e) {
      note(where + " body", body);
      return;
    }
    if (!(parsed instanceof Json.Obj doc)) {
      return;
    }
    note(where + " title", doc.get("title"));
    note(where + " detail", doc.get("detail"));
    if (doc.get("data") instanceof Json.Obj data) {
      if (data.get("issues") instanceof Json.Arr issues) {
        for (Json i : issues.items()) {
          note(where + " issue", i.asObject().get("message"));
        }
      }
      note(where + " data.message", data.get("message"));
    }
    if (doc.get("violations") instanceof Json.Arr violations) {
      for (Json v : violations.items()) {
        note(where + " violation", v.asObject().get("message"));
      }
    }
  }

  // What the application does: it is the handler of a request that passed every check.
  private static volatile Booking booking = new Booking(ID, Status.PENDING, 3L, new BigDecimal("10.00"));
  private static volatile Booking override;
  private static volatile boolean suspended;
  private static final List<Throwable> ERRORS = new CopyOnWriteArrayList<>();
  private static final List<gen.b.runtime.RuleWalk.Event> EVENTS = new CopyOnWriteArrayList<>();

  private static final class Impl implements ProbeService {
    @Override
    public Shape shapes(ShapesRequest request, RequestContext ctx) {
      return request.body();
    }

    @Override
    public Shape byId(ByIdRequest request, RequestContext ctx) {
      throw new IllegalStateException("the handler is not reached");
    }

    @Override
    public Shape byName(ByNameRequest request, RequestContext ctx) {
      throw new IllegalStateException("the handler is not reached");
    }

    @Override
    public Booking pay(
        PayRequest request, PayFacts facts, Long version, Transition<Status> transition, RequestContext ctx) {
      return override != null ? override : new Booking(request.id(), Status.PAID, 3L, new BigDecimal("10.00"));
    }
  }

  private static final ProbeFactLoaders LOADERS = new ProbeFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.of(booking);
    }
  };

  private static VeldHandler probe() {
    ProbeHooks hooks = new ProbeHooks(
            (token, ctx) -> token.equals("me") ? new User(CALLER, suspended) : null,
            new MemoryIdempotencyStore(),
            (principal, ctx) -> principal.id().toString())
        .onError((e, ctx) -> ERRORS.add(e))
        .onRuleViolation(EVENTS::add);
    return ProbeHandler.create(new Impl(), LOADERS, hooks);
  }

  private static VeldResponse send(
      VeldHandler handler, String where, String method, String path, String query, String body, Map<String, String> extra, String token) {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    headers.put("authorization", List.of("Bearer " + token));
    extra.forEach((k, v) -> headers.put(k, List.of(v)));
    VeldResponse res = handler
        .handle(new VeldRequest(method, path, query, headers, body, null))
        .orElseThrow(() -> new AssertionError(where + ": " + method + " " + path + " matched no route"));
    problemTexts(where, res.body());
    res.headers().forEach((name, value) -> note(where + " header " + name, value));
    return res;
  }

  private static VeldResponse send(VeldHandler handler, String where, String method, String path, String body) {
    return send(handler, where, method, path, null, body, Map.of(), "me");
  }

  /** The body of {@code shapes}, valid unless {@code member} says otherwise ({@code null} text removes it). */
  private static String shape(Map<String, String> replace, String... without) {
    Map<String, String> m = new LinkedHashMap<>();
    m.put("id", "\"" + ID + "\"");
    m.put("name", "\"abcd\"");
    m.put("code", "\"ABC\"");
    m.put("count", "2");
    m.put("price", "\"1.00\"");
    m.put("when", "\"2026-06-10T12:00:00Z\"");
    m.put("status", "\"pending\"");
    m.put("tags", "[\"a\"]");
    m.put("extra", "{\"a\":1}");
    m.put("pay", "{\"kind\":\"card\",\"last4\":\"1234\"}");
    m.put("email", "\"a@b.example\"");
    m.put("blob", "{\"any\":\"thing\"}");
    m.putAll(replace);
    for (String name : without) {
      m.remove(name);
    }
    StringBuilder out = new StringBuilder("{");
    m.forEach((k, v) -> out.append(out.length() > 1 ? "," : "").append('"').append(k).append("\":").append(v));
    return out.append('}').toString();
  }

  private static String marker() {
    return "\"" + MARKER + "\"";
  }

  private static void serverCases() {
    VeldHandler handler = probe();

    // Every validation issue code of a body: type, required, enum, bounds, length, pattern, format,
    // discriminator, a map value, a list that is not a list.
    Map<String, String> everything = new LinkedHashMap<>();
    for (String field : List.of("id", "name", "code", "count", "price", "when", "status", "tags", "email")) {
      everything.put(field, marker());
    }
    everything.put("extra", "{\"" + MARKER + "\":" + marker() + "}");
    everything.put("pay", "{\"kind\":" + marker() + "}");
    VeldResponse res = send(handler, "shape: everything wrong", "POST", "/probe/shapes", shape(everything));
    check(res.status() == 422, "everything wrong is a 422: " + res.status());
    for (String field : List.of("name", "code", "count", "price", "when", "status", "tags", "extra", "pay", "email")) {
      String value = field.equals("extra") ? "{\"k\":" + marker() + "}" : field.equals("pay") ? "{\"kind\":" + marker() + "}" : marker();
      send(handler, "shape: " + field, "POST", "/probe/shapes", shape(Map.of(field, value)));
    }
    for (String missing : List.of("name", "code", "pay")) {
      send(handler, "shape: " + missing + " is missing", "POST", "/probe/shapes", shape(Map.of(), missing));
    }
    send(handler, "shape: bounds", "POST", "/probe/shapes", shape(Map.of("count", "99", "name", "\"a\"")));
    send(handler, "shape: not an object", "POST", "/probe/shapes", marker());
    send(handler, "shape: a body that is not JSON", "POST", "/probe/shapes", "{\"name\": \"" + MARKER);

    // Invariants: violated (an odd count), and one that cannot be evaluated (a division by zero).
    res = send(handler, "rule: violated", "POST", "/probe/shapes", shape(Map.of("count", "3", "name", "\"" + MARKER.substring(0, 8) + "\"")));
    check(res.status() == 422, "an odd count is a 422: " + res.body());
    res = send(handler, "rule: cannot be evaluated", "POST", "/probe/shapes", shape(Map.of("count", "4")));
    check(res.status() == 422, "a division by zero is a 422: " + res.body());
    for (gen.b.runtime.RuleWalk.Event e : EVENTS) {
      note("rule event", e.violation().message());
    }

    // The three parameters of another binding.
    send(handler, "path, query and header", "GET", "/probe/by/" + MARKER, null, null, Map.of("x-marker", MARKER), "me");
    send(handler, "query", "GET", "/probe/by/" + ID, "size=" + MARKER, null, Map.of("x-marker", ID.toString()), "me");
    send(handler, "an unknown name in the address", "GET", "/probe/name/" + MARKER, null);
    send(handler, "a refused caller", "GET", "/probe/name/" + MARKER, null, null, Map.of(), MARKER);

    // The endpoint with every guard: the idempotency key, If-Match, the facts, the policy, the
    // transition, the statements and the target check.
    String path = "/probe/" + ID + "/pay";
    String payBody = "{\"amount\":\"%s\",\"note\":\"" + MARKER + "\"}";
    send(handler, "pay: a body of the wrong type", "POST", path, "{\"amount\":" + marker() + ",\"note\":" + marker() + "}");
    pay(handler, "with a bad idempotency key", payBody, "5.00", Map.of("idempotency-key", MARKER + " \u0001"));
    pay(handler, "with a bad If-Match", payBody, "5.00", Map.of("if-match", MARKER));
    pay(handler, "with a stale version", payBody, "5.00", Map.of("if-match", "\"999\""));
    pay(handler, "a statement that fails (named)", payBody, "0.00", Map.of("if-match", "\"3\""));
    pay(handler, "a statement that fails (unnamed)", payBody, "5000.00", Map.of("if-match", "\"3\""));
    pay(handler, "a statement on a string (named)", payBody, "5.00", Map.of("if-match", "\"3\""));
    booking = new Booking(ID, Status.PAID, 3L, new BigDecimal("10.00"));
    pay(handler, "a transition that is not legal", payBody, "5.00", Map.of("if-match", "\"3\""));
    booking = new Booking(ID, Status.CANCELLED, 3L, new BigDecimal("10.00"));
    pay(handler, "a denial without a message", payBody, "5.00", Map.of("if-match", "\"3\""));
    booking = new Booking(ID, Status.PENDING, 3L, new BigDecimal("10.00"));
    suspended = true;
    pay(handler, "a denial with a message", payBody, "5.00", Map.of("if-match", "\"3\""));
    suspended = false;

    // A body that passes every statement; the handler answers wrongly so that the ensure and the
    // target check fail.
    String ok = "{\"amount\":\"5.00\",\"note\":\"ok-1\"}";
    override = new Booking(ID, Status.PENDING, 3L, new BigDecimal("10.00"));
    send(handler, "pay: the target check", "POST", path, null, ok, Map.of("if-match", "\"3\""), "me");
    override = new Booking(ID, Status.PAID, -1L, new BigDecimal("10.00"));
    send(handler, "pay: the ensure", "POST", path, null, ok, Map.of("if-match", "\"3\""), "me");
    override = null;
    send(handler, "pay: ok with a key", "POST", path, null, ok, Map.of("if-match", "\"3\"", "idempotency-key", "k1"), "me");

    // What the runtime tells onError is a log line.
    for (Throwable e : ERRORS) {
      note("onError", e.getMessage());
    }

  }

  private static void pay(VeldHandler handler, String where, String template, String amount, Map<String, String> headers) {
    send(handler, "pay " + where, "POST", "/probe/" + ID + "/pay", null, template.replace("%s", amount), headers, "me");
  }

  // -- the client ------------------------------------------------------------------------------

  private static void refused(String where, Runnable call) {
    try {
      call.run();
    } catch (gen.c.runtime.VeldError e) {
      note("client " + where + " message", e.getMessage());
      note("client " + where + " title", e.title());
      gen.c.runtime.Json data = e.dataWire();
      if (data instanceof gen.c.runtime.Json.Obj o && o.get("issues") instanceof gen.c.runtime.Json.Arr issues) {
        for (gen.c.runtime.Json i : issues.items()) {
          note("client " + where + " issue", i.asObject().get("message").asString());
        }
      }
      for (gen.c.runtime.Violation v : e.violations()) {
        note("client " + where + " violation", v.message());
      }
      return;
    } catch (RuntimeException e) {
      note("client " + where + " message", e.getMessage());
      return;
    }
    throw new AssertionError(where + ": the call did not fail");
  }

  private static void clientCases() throws Exception {
    // The local check: every issue of a body and the invariants.
    gen.c.runtime.ClientOptions local = gen.c.runtime.ClientOptions.builder()
        .baseUrl("http://127.0.0.1:1")
        .bearer(() -> "me")
        .timeout(java.time.Duration.ofSeconds(2))
        .build();
    gen.c.bookings.client.ProbeClient client = new gen.c.bookings.client.ProbeClient(local);
    refused("shape", () -> client.shapes(new gen.c.bookings.client.ShapesRequest(clientShape(MARKER, MARKER, MARKER, 99))));
    refused("rule", () -> client.shapes(new gen.c.bookings.client.ShapesRequest(clientShape("abcd", "ABC", "x@y.example", 3))));
    refused("rule that cannot be evaluated", () -> client.shapes(new gen.c.bookings.client.ShapesRequest(clientShape("abcd", "ABC", "x@y.example", 4))));

    // Transport failures: the address holds the marker (a path parameter), and so do the failure and the body.
    refused("a connection that fails", () -> client.byName(new gen.c.bookings.client.ByNameRequest(MARKER)));
    try (Fake fake = new Fake(200, "{\"x\": \"" + MARKER)) {
      refused("a body that is not JSON", () -> fake.client().byName(new gen.c.bookings.client.ByNameRequest(MARKER)));
    }
    try (Fake fake = new Fake(200, "")) {
      refused("an empty body", () -> fake.client().byName(new gen.c.bookings.client.ByNameRequest(MARKER)));
    }
    try (Fake fake = new Fake(418, "{\"detail\": \"" + MARKER + "\"}")) {
      refused("a status that is not declared", () -> fake.client().byName(new gen.c.bookings.client.ByNameRequest(MARKER)));
    }
    try (Fake fake = new Fake(200, "{}", 10)) {
      refused("a timeout", () -> fake.client().byName(new gen.c.bookings.client.ByNameRequest(MARKER)));
    }
    // A client built with no address says so without the value.
    refused("without an address", () -> new gen.c.bookings.client.ProbeClient(gen.c.runtime.ClientOptions.builder().build())
        .byName(new gen.c.bookings.client.ByNameRequest(MARKER)));
  }

  private static gen.c.bookings.no_values.Shape clientShape(String name, String code, String email, int count) {
    return new gen.c.bookings.no_values.Shape(
        ID, name, code, count, new BigDecimal("1.00"), java.time.Instant.parse("2026-06-10T12:00:00Z"),
        gen.c.bookings.no_values.Status.PENDING, List.of("a"), Map.of("a", 1),
        new gen.c.bookings.no_values.Card("1234"), email, gen.c.runtime.Json.object().build());
  }

  /** A server that answers every request with one status and body, after {@code delayMillis}. */
  private static final class Fake implements AutoCloseable {
    private final com.sun.net.httpserver.HttpServer http;

    Fake(int status, String body) throws java.io.IOException {
      this(status, body, 0);
    }

    Fake(int status, String body, long delayMillis) throws java.io.IOException {
      http = com.sun.net.httpserver.HttpServer.create(new java.net.InetSocketAddress("127.0.0.1", 0), 0);
      http.createContext("/", exchange -> {
        try {
          Thread.sleep(delayMillis);
        } catch (InterruptedException e) {
          Thread.currentThread().interrupt();
        }
        byte[] bytes = body.getBytes(java.nio.charset.StandardCharsets.UTF_8);
        exchange.getResponseHeaders().add("content-type", "application/json");
        exchange.sendResponseHeaders(status, bytes.length == 0 ? -1 : bytes.length);
        if (bytes.length > 0) {
          exchange.getResponseBody().write(bytes);
        }
        exchange.close();
      });
      http.start();
    }

    gen.c.bookings.client.ProbeClient client() {
      String url = "http://127.0.0.1:" + http.getAddress().getPort();
      return new gen.c.bookings.client.ProbeClient(gen.c.runtime.ClientOptions.builder()
          .baseUrl(url)
          .bearer(() -> "me")
          .timeout(java.time.Duration.ofMillis(5))
          .build());
    }

    @Override
    public void close() {
      http.stop(0);
    }
  }

  public static void main(String[] args) throws Exception {
    serverCases();
    clientCases();
    check(TEXTS.size() > 40, "texts were collected (" + TEXTS.size() + ")");
    for (String[] t : TEXTS) {
      check(!t[1].contains(MARKER), t[0] + " repeats the value: " + t[1]);
    }
    System.out.println("OK");
  }
}
