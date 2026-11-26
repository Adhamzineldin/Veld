package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.prelude.Forbidden;
import gen.b.prelude.InvalidTransition;
import gen.b.prelude.PreconditionFailed;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.Transition;
import gen.b.vers.server.GetRoomRequest;
import gen.b.vers.server.OrdersFactLoaders;
import gen.b.vers.server.OrdersHandler;
import gen.b.vers.server.OrdersHooks;
import gen.b.vers.server.OrdersService;
import gen.b.vers.server.PayOrderFacts;
import gen.b.vers.server.PayOrderRequest;
import gen.b.vers.server.RenameFacts;
import gen.b.vers.server.RenameRequest;
import gen.b.vers.server.RoomsFactLoaders;
import gen.b.vers.server.RoomsHandler;
import gen.b.vers.server.RoomsHooks;
import gen.b.vers.server.RoomsService;
import gen.b.vers.server.TickFacts;
import gen.b.vers.server.TickRequest;
import gen.b.vers.server.TouchFacts;
import gen.b.vers.server.TouchRequest;
import gen.b.vers.vers.Counter;
import gen.b.vers.vers.Order;
import gen.b.vers.vers.Room;
import gen.b.vers.vers.Status;
import gen.b.vers.vers.User;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The production handlers of {@code tests/rules/endpoints/version.veld.pending} over HTTP on the JDK
 * server: a tag that matches, a stale tag (412 and its body), {@code *}, no header, every malformed
 * form (422), the {@code version} the handler receives, the {@code ETag} of a success, the order of
 * the checks (policy, then version, then transition, then {@code require}) and an endpoint without
 * a guard, which ignores {@code If-Match}.
 */
public final class VersionHttp {
  private VersionHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000a1");
  private static final AtomicInteger CALLS = new AtomicInteger();

  private static Room room = new Room(ID, "Red", 4);
  private static boolean roomMissing;
  private static Counter counter = new Counter(ID, 10, 2);
  private static Status orderStatus = Status.PENDING;
  private static long orderRevision = 7;

  private static Long receivedLong = -1L;
  private static Integer receivedInt = -1;
  private static Transition<Status> receivedTransition;

  private static final class RoomLoaders implements RoomsFactLoaders {
    @Override
    public Optional<Counter> counter(UUID key, RequestContext ctx) {
      return Optional.of(counter);
    }

    @Override
    public Optional<Room> maybe(UUID key, RequestContext ctx) {
      return roomMissing ? Optional.empty() : Optional.of(room);
    }

    @Override
    public Optional<Room> room(UUID key, RequestContext ctx) {
      return Optional.of(room);
    }
  }

  private static final class Rooms implements RoomsService {
    @Override
    public Room rename(RenameRequest request, RenameFacts facts, Long version, RequestContext ctx) {
      CALLS.incrementAndGet();
      receivedLong = version;
      return new Room(ID, request.body().name(), facts.room().revision() + 1);
    }

    @Override
    public Room touch(TouchRequest request, TouchFacts facts, Long version, RequestContext ctx) {
      CALLS.incrementAndGet();
      receivedLong = version;
      return room;
    }

    @Override
    public Counter tick(TickRequest request, TickFacts facts, Integer version, RequestContext ctx) {
      CALLS.incrementAndGet();
      receivedInt = version;
      return new Counter(ID, 11, facts.counter().version() + 1);
    }

    @Override
    public Room getRoom(GetRoomRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return room;
    }
  }

  private static final class OrderLoaders implements OrdersFactLoaders {
    @Override
    public Optional<Order> order(UUID key, RequestContext ctx) {
      return Optional.of(new Order(ID, orderStatus, orderRevision));
    }
  }

  private static final class Orders implements OrdersService {
    @Override
    public Order payOrder(
        PayOrderRequest request, PayOrderFacts facts, Long version, Transition<Status> transition, RequestContext ctx) {
      CALLS.incrementAndGet();
      receivedLong = version;
      receivedTransition = transition;
      return new Order(ID, Status.PAID, facts.order().revision() + 1);
    }
  }

  private record Answer(int status, Map<String, String> headers, Json.Obj body) {
    String header(String name) {
      return headers.get(name.toLowerCase());
    }
  }

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static Answer send(HttpServer server, String method, String path, Map<String, String> headers, String body)
      throws Exception {
    HttpRequest.Builder builder = HttpRequest.newBuilder(
        URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path));
    headers.forEach(builder::header);
    if (body != null) {
      builder.header("content-type", "application/json");
    }
    builder.method(method, body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
    HttpResponse<String> response = HTTP.send(builder.build(), HttpResponse.BodyHandlers.ofString());
    Map<String, String> seen = new LinkedHashMap<>();
    response.headers().map().forEach((k, v) -> seen.put(k.toLowerCase(), v.get(0)));
    Json.Obj json = response.body().isEmpty() ? null : Json.parse(response.body()).asObject();
    return new Answer(response.statusCode(), seen, json);
  }

  private static Map<String, String> ifMatch(String tag) {
    return tag == null ? Map.of() : Map.of("If-Match", tag);
  }

  private static Answer rename(HttpServer server, String tag) throws Exception {
    return send(server, "PUT", "/rooms/" + ID, ifMatch(tag), "{\"name\":\"Blue\"}");
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static String title(Answer a) {
    return a.body().get("title").asString();
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    HttpServer rooms = JdkHttpServerAdapter.listen(
        RoomsHandler.create(new Rooms(), new RoomLoaders(), new RoomsHooks().onRuleViolation(events::add)),
        new InetSocketAddress("127.0.0.1", 0));
    HttpServer orders = JdkHttpServerAdapter.listen(
        OrdersHandler.create(
            new Orders(),
            new OrderLoaders(),
            new OrdersHooks((token, ctx) -> new User(ID, token.equals("staff") ? List.of("staff") : List.of()))
                .onRuleViolation(events::add)),
        new InetSocketAddress("127.0.0.1", 0));
    try {
      aTagThatMatches(rooms);
      aStaleTag(rooms, events);
      starAndNoHeader(rooms);
      theEdgesOfTheVersion(rooms);
      malformedTags(rooms);
      anIntVersion(rooms);
      anOptionalFact(rooms);
      noGuardIgnoresIfMatch(rooms);
      theOrderOfTheChecks(orders);
    } finally {
      rooms.stop(0);
      orders.stop(0);
    }
    System.out.println("OK");
  }

  /** A tag that is the current version passes, and the handler gets the version it was checked against. */
  private static void aTagThatMatches(HttpServer server) throws Exception {
    room = new Room(ID, "Red", 4);
    int calls = CALLS.get();
    Answer a = rename(server, "\"4\"");
    check(a.status() == 200, "a matching tag passes: " + a.status());
    check(CALLS.get() == calls + 1, "the handler ran");
    check(Long.valueOf(4L).equals(receivedLong), "the handler received the version: " + receivedLong);
    check("\"5\"".equals(a.header("etag")), "ETag is the version of the returned value: " + a.header("etag"));
    check(a.body().get("revision").asString().equals("5"), "the body has the new version");
  }

  /** A stale tag is 412 with expected and actual as decimal strings and one violation. */
  private static void aStaleTag(HttpServer server, List<RuleWalk.Event> events) throws Exception {
    room = new Room(ID, "Red", 4);
    int calls = CALLS.get();
    int seen = events.size();
    Answer a = rename(server, "\"3\"");
    check(a.status() == 412, "a stale tag is 412: " + a.status());
    check(a.body().get("type").asString().equals(PreconditionFailed.VELD_SIG), "type " + a.body().get("type"));
    check(title(a).equals("PreconditionFailed"), "title " + title(a));
    Json.Obj data = a.body().get("data").asObject();
    check(data.get("expected").asString().equals("3"), "expected is the tag, as a string: " + data.write());
    check(data.get("actual").asString().equals("4"), "actual is the version, as a string: " + data.write());
    var violations = a.body().get("violations").asArray().items();
    check(violations.size() == 1, "one violation");
    Json.Obj v = violations.get(0).asObject();
    check(v.get("rule").asString().startsWith("veld:vers/Rooms.rename.version@"), "rule id " + v.get("rule"));
    check(v.get("path").asString().equals("request"), "path request");
    check(v.get("fields").asArray().items().isEmpty(), "no fields");
    check(v.get("message").asString().equals("violates rule version"), "message " + v.get("message"));
    check(a.header("etag") == null, "no ETag on an error");
    check(CALLS.get() == calls, "the handler did not run");
    check(events.size() == seen + 1, "one event");
    RuleWalk.Event e = events.get(seen);
    check(e.position() == RuleWalk.Position.INPUT && e.outcome() == RuleWalk.Outcome.REJECTED, "input, rejected");
    check(e.rule().startsWith("veld:vers/Rooms.rename.version@"), "event rule " + e.rule());
    Answer ahead = rename(server, "\"9\"");
    check(ahead.status() == 412, "a tag ahead of the row is refused too");
  }

  /** A star and no header are not compared: the handler gets no version. */
  private static void starAndNoHeader(HttpServer server) throws Exception {
    room = new Room(ID, "Red", 4);
    for (String tag : new String[] {"*", null}) {
      receivedLong = -1L;
      Answer a = rename(server, tag);
      check(a.status() == 200, "passes with " + tag + ": " + a.status());
      check(receivedLong == null, "no version to write against with " + tag + ": " + receivedLong);
      check("\"5\"".equals(a.header("etag")), "ETag with " + tag);
    }
  }

  /** Zero is a version, and so is the largest long. */
  private static void theEdgesOfTheVersion(HttpServer server) throws Exception {
    room = new Room(ID, "Red", 0);
    check(rename(server, "\"0\"").status() == 200, "version zero is a version");
    check(Long.valueOf(0L).equals(receivedLong), "received zero");
    room = new Room(ID, "Red", Long.MAX_VALUE);
    Answer top = send(server, "GET", "/rooms/" + ID, Map.of("If-Match", "\"9223372036854775807\""), null);
    check(top.status() == 200 && ("\"" + Long.MAX_VALUE + "\"").equals(top.header("etag")), "ETag of the largest long");
    Answer stale = send(server, "PATCH", "/rooms/" + ID + "/touch", ifMatch("\"9223372036854775806\""), null);
    check(stale.status() == 412, "one below the largest long is stale: " + stale.status());
    Json.Obj data = stale.body().get("data").asObject();
    check(data.get("expected").asString().equals("9223372036854775806"), "expected " + data.write());
    check(data.get("actual").asString().equals("9223372036854775807"), "actual " + data.write());
    room = new Room(ID, "Red", 4);
  }

  /** Every malformed form is 422 with the one issue of step 5, and nothing runs. */
  private static void malformedTags(HttpServer server) throws Exception {
    int calls = CALLS.get();
    for (String bad : new String[] {
      "3", "\"03\"", "W/\"3\"", "\"3\", \"4\"", "\"\"", "\"-1\"", "\"9223372036854775808\"", "\"3\" \"4\"", "tag"
    }) {
      Answer a = rename(server, bad);
      check(a.status() == 422, bad + " is 422: " + a.status());
      check(title(a).equals("ValidationError"), bad + ": " + title(a));
      var issues = a.body().get("data").asObject().get("issues").asArray().items();
      check(issues.size() == 1, bad + ": one issue " + a.body().write());
      Json.Obj issue = issues.get(0).asObject();
      check(issue.get("path").asString().equals("header.If-Match"), bad + ": path " + issue.write());
      check(issue.get("code").asString().equals("type"), bad + ": code " + issue.write());
      check(
          issue.get("message").asString().equals("must be * or one strong entity tag holding a version, such as \"3\""),
          bad + ": message " + issue.write());
      check(a.body().get("violations") == null, bad + ": a header issue is not a rule violation");
    }
    check(CALLS.get() == calls, "the handler did not run");
  }

  /** An {@code int} version: the handler gets an {@code Integer}, and the strings are decimal. */
  private static void anIntVersion(HttpServer server) throws Exception {
    counter = new Counter(ID, 10, 2);
    Answer stale = send(server, "POST", "/rooms/" + ID + "/tick", ifMatch("\"1\""), null);
    check(stale.status() == 412, "a stale int version is 412: " + stale.status());
    Json.Obj data = stale.body().get("data").asObject();
    check(data.get("expected").asString().equals("1") && data.get("actual").asString().equals("2"), data.write());
    Answer ok = send(server, "POST", "/rooms/" + ID + "/tick", ifMatch("\"2\""), null);
    check(ok.status() == 200, "the current int version passes: " + ok.status());
    check(Integer.valueOf(2).equals(receivedInt), "the handler received an Integer: " + receivedInt);
    check("\"3\"".equals(ok.header("etag")), "ETag of an int version: " + ok.header("etag"));
    check(ok.body().get("version").asDouble() == 3.0, "an int version is a number on the wire");
  }

  /** A guarded fact that is absent has nothing to compare: the request passes and no version is handed over. */
  private static void anOptionalFact(HttpServer server) throws Exception {
    roomMissing = true;
    receivedLong = -1L;
    Answer absent = send(server, "PATCH", "/rooms/" + ID + "/touch", ifMatch("\"9\""), null);
    check(absent.status() == 200, "an absent optional fact passes: " + absent.status());
    check(receivedLong == null, "nothing was compared, nothing is handed over: " + receivedLong);
    roomMissing = false;
    room = new Room(ID, "Red", 4);
    Answer present = send(server, "PATCH", "/rooms/" + ID + "/touch", ifMatch("\"4\""), null);
    check(present.status() == 200 && Long.valueOf(4L).equals(receivedLong), "a present fact is compared: " + receivedLong);
  }

  /** An endpoint without a guard ignores {@code If-Match}, whatever it holds, and still owes the ETag. */
  private static void noGuardIgnoresIfMatch(HttpServer server) throws Exception {
    room = new Room(ID, "Red", 4);
    for (String tag : new String[] {"\"1\"", "garbage", "*"}) {
      Answer a = send(server, "GET", "/rooms/" + ID, ifMatch(tag), null);
      check(a.status() == 200, "ignored: " + tag + " " + a.status());
      check("\"4\"".equals(a.header("etag")), "ETag " + a.header("etag"));
    }
  }

  private static Answer pay(HttpServer server, String token, String tag, String text) throws Exception {
    Map<String, String> headers = new LinkedHashMap<>(ifMatch(tag));
    headers.put("Authorization", "Bearer " + token);
    return send(server, "POST", "/orders/" + ID + "/pay", headers, "{\"text\":\"" + text + "\"}");
  }

  /** Policy first, then the version, then the transition, then {@code require}. */
  private static void theOrderOfTheChecks(HttpServer server) throws Exception {
    orderRevision = 7;
    orderStatus = Status.PENDING;
    int calls = CALLS.get();
    Answer denied = pay(server, "guest", "\"1\"", "ok");
    check(denied.status() == 403 && title(denied).equals("Forbidden"), "a policy comes before the version: " + denied.status());
    check(denied.body().get("type").asString().equals(Forbidden.VELD_SIG), "Forbidden");
    orderStatus = Status.PAID;
    Answer stale = pay(server, "staff", "\"1\"", "ok");
    check(stale.status() == 412, "the version comes before the transition: " + stale.status());
    Answer illegal = pay(server, "staff", "\"7\"", "ok");
    check(illegal.status() == 409 && title(illegal).equals("InvalidTransition"), "then the transition: " + illegal.status());
    check(illegal.body().get("type").asString().equals(InvalidTransition.VELD_SIG), "InvalidTransition");
    orderStatus = Status.PENDING;
    Answer tooLong = pay(server, "staff", "\"7\"", "much too long");
    check(tooLong.status() == 422 && title(tooLong).equals("TooLong"), "then require: " + tooLong.status());
    check(CALLS.get() == calls, "the handler ran for none of them");
    Answer ok = pay(server, "staff", "\"7\"", "ok");
    check(ok.status() == 201, "everything holds: " + ok.status());
    check(Long.valueOf(7L).equals(receivedLong), "the version of the guard: " + receivedLong);
    check(receivedTransition != null && receivedTransition.to() == Status.PAID, "and the transition");
    check("\"8\"".equals(ok.header("etag")), "ETag of a 201: " + ok.header("etag"));
    check(ok.body().get("rev").asString().equals("8"), "the version has its wire name: " + ok.body().write());
  }
}
