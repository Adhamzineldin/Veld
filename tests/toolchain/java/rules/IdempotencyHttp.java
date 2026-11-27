package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.idemp.idemp.Booking;
import gen.b.idemp.idemp.Room;
import gen.b.idemp.idemp.Shelf;
import gen.b.idemp.server.BookFacts;
import gen.b.idemp.server.BookMineRequest;
import gen.b.idemp.server.BookRequest;
import gen.b.idemp.server.BookingsFactLoaders;
import gen.b.idemp.server.BookingsHandler;
import gen.b.idemp.server.BookingsHooks;
import gen.b.idemp.server.BookingsService;
import gen.b.idemp.server.CancelRequest;
import gen.b.idemp.server.PayRequest;
import gen.b.idemp.server.ReplaceRequest;
import gen.b.idemp.server.SaveRoomFacts;
import gen.b.idemp.server.SaveRoomRequest;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import gen.b.runtime.IdempotencyStore;
import gen.b.runtime.StoredAnswer;
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
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The production handler of {@code tests/rules/endpoints/idempotency.veld.pending} over HTTP on the
 * JDK server: a new key runs and is stored, the same key and parameters replays (status, body, ETag,
 * the replay header, the handler not called and no fact loaded), other parameters and a request that
 * is still running are 409, a declared error is stored, a 500 is not, a required or malformed key is
 * 422, two callers do not replay each other, a server without the store or the caller hook fails when
 * it is created, a store that fails is a 500 (begin) or only reported (complete, release), and a
 * caller hook that fails is a 500 like a failing begin (spec/rules.md section 11.12).
 */
public final class IdempotencyHttp {
  private IdempotencyHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-0000000000b1");
  private static final String OTHER_MESSAGE = "this idempotency key was used with other parameters";
  private static final String RUNNING_MESSAGE = "a request with this idempotency key is still running";

  private static final AtomicInteger CALLS = new AtomicInteger();
  private static final AtomicInteger LOADS = new AtomicInteger();
  private static final List<String> ERRORS = Collections.synchronizedList(new ArrayList<>());

  private static boolean failNext;
  private static volatile boolean failCaller;
  private static CountDownLatch entered;
  private static CountDownLatch release;

  /** A memory store that counts, remembers the scopes it was asked about and can be made to fail. */
  private static final class Spy implements IdempotencyStore {
    final MemoryIdempotencyStore memory = new MemoryIdempotencyStore();
    final List<String> scopes = Collections.synchronizedList(new ArrayList<>());
    final AtomicInteger completes = new AtomicInteger();
    final AtomicInteger releases = new AtomicInteger();
    volatile boolean failBegin;
    volatile boolean failComplete;
    volatile boolean failRelease;

    @Override
    public Begin begin(String scope, String key, String fingerprint) throws Exception {
      scopes.add(scope);
      if (failBegin) {
        throw new IllegalStateException("the store is down");
      }
      return memory.begin(scope, key, fingerprint);
    }

    @Override
    public void complete(String scope, String key, StoredAnswer answer) throws Exception {
      completes.incrementAndGet();
      if (failComplete) {
        throw new IllegalStateException("cannot store");
      }
      memory.complete(scope, key, answer);
    }

    @Override
    public void release(String scope, String key) throws Exception {
      releases.incrementAndGet();
      if (failRelease) {
        throw new IllegalStateException("cannot release");
      }
      memory.release(scope, key);
    }
  }

  private static final class Loaders implements BookingsFactLoaders {
    @Override
    public Optional<Room> room(RequestContext ctx) {
      LOADS.incrementAndGet();
      return Optional.of(new Room(ID, 9));
    }

    @Override
    public Optional<Shelf> shelf(UUID key, RequestContext ctx) {
      LOADS.incrementAndGet();
      return Optional.of(new Shelf(ID, "Red", 4));
    }
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking book(BookRequest request, BookFacts facts, RequestContext ctx) throws Exception {
      CALLS.incrementAndGet();
      if (entered != null) {
        CountDownLatch mine = entered;
        entered = null;
        mine.countDown();
        release.await(20, TimeUnit.SECONDS);
      }
      if (failNext) {
        failNext = false;
        throw new IllegalStateException("boom");
      }
      return new Booking(ID, request.body().guests());
    }

    @Override
    public Booking pay(PayRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(ID, request.body().guests());
    }

    @Override
    public Booking replace(ReplaceRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(request.id(), request.body().guests());
    }

    @Override
    public void cancel(CancelRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
    }

    @Override
    public Booking bookMine(BookMineRequest request, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Booking(ID, request.body().guests());
    }

    @Override
    public Shelf saveRoom(SaveRoomRequest request, SaveRoomFacts facts, Long version, RequestContext ctx) {
      CALLS.incrementAndGet();
      return new Shelf(ID, request.body().name(), facts.shelf().revision() + 1);
    }
  }

  private record Answer(int status, Map<String, String> headers, String text) {
    String header(String name) {
      return headers.get(name.toLowerCase());
    }

    Json.Obj body() {
      return text.isEmpty() ? null : Json.parse(text).asObject();
    }

    boolean replayed() {
      return "true".equals(header("Veld-Idempotent-Replay"));
    }
  }

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static HttpRequest request(HttpServer server, String method, String path, Map<String, String> headers, String body) {
    HttpRequest.Builder builder = HttpRequest.newBuilder(
        URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path));
    headers.forEach(builder::header);
    if (body != null) {
      builder.header("content-type", "application/json");
    }
    builder.method(method, body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
    return builder.build();
  }

  private static Answer answer(HttpResponse<String> response) {
    Map<String, String> seen = new LinkedHashMap<>();
    response.headers().map().forEach((k, v) -> seen.put(k.toLowerCase(), v.get(0)));
    return new Answer(response.statusCode(), seen, response.body());
  }

  private static Answer send(HttpServer server, String method, String path, Map<String, String> headers, String body)
      throws Exception {
    return answer(HTTP.send(request(server, method, path, headers, body), HttpResponse.BodyHandlers.ofString()));
  }

  private static Answer book(HttpServer server, String key, int guests) throws Exception {
    Map<String, String> headers = key == null ? Map.of() : Map.of("Idempotency-Key", key);
    return send(server, "POST", "/bookings", headers, "{\"guests\":" + guests + "}");
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static String title(Answer a) {
    return a.body().get("title").asString();
  }

  private static HttpServer serve(Spy store) throws Exception {
    BookingsHooks hooks = new BookingsHooks(
            (token, ctx) -> token,
            store,
            (principal, ctx) -> {
              if (failCaller) {
                throw new IllegalStateException("the caller hook is down");
              }
              return (String) principal;
            })
        .onError((error, ctx) -> ERRORS.add(String.valueOf(error.getMessage())));
    return JdkHttpServerAdapter.listen(
        BookingsHandler.create(new Impl(), new Loaders(), hooks), new InetSocketAddress("127.0.0.1", 0));
  }

  public static void main(String[] args) throws Exception {
    Spy store = new Spy();
    HttpServer server = serve(store);
    try {
      aNewKeyRunsAndAnAnswerIsReplayed(server, store);
      otherParametersAndARunningRequest(server);
      aDeclaredErrorIsStoredAnd500IsNot(server, store);
      theKeyItself(server);
      aReplayKeepsTheEtagAndStatusOfStep6(server);
      aRequestThatIsNotValidIsNotReplayed(server);
      twoCallersDoNotReplayEachOther(server, store);
      theHeaderAndTheEmptyAnswer(server);
      aStoreThatFails(store, server);
      aCallerHookThatFails(store, server);
      aServerThatLacksAHook();
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }

  /** A new key runs and is stored; the same key and parameters is answered again and nothing else runs. */
  private static void aNewKeyRunsAndAnAnswerIsReplayed(HttpServer server, Spy store) throws Exception {
    int calls = CALLS.get();
    Answer first = book(server, "k-new", 2);
    check(first.status() == 201, "a new key runs: " + first.status());
    check(!first.replayed() && CALLS.get() == calls + 1, "the handler ran once");
    check(store.completes.get() == 1, "the answer was stored");
    check(store.scopes.get(0).contains("Bookings.book@") && !store.scopes.get(0).contains("\n"), "scope " + store.scopes.get(0));
    int loads = LOADS.get();
    Answer again = book(server, "k-new", 2);
    check(again.status() == 201 && again.replayed(), "the same key and parameters is replayed: " + again.status());
    check(again.text().equals(first.text()), "the same body: " + again.text());
    check("application/json".equals(again.header("content-type")), "the same content type");
    check(CALLS.get() == calls + 1, "the handler did not run again");
    check(LOADS.get() == loads, "no fact was loaded for a replay");
    check(first.header("Veld-Idempotent-Replay") == null, "the first answer is not marked");
  }

  /** The same key with other parameters is 409, and so is a key whose first request is running. */
  private static void otherParametersAndARunningRequest(HttpServer server) throws Exception {
    book(server, "k-other", 2);
    int calls = CALLS.get();
    int loads = LOADS.get();
    Answer other = book(server, "k-other", 3);
    check(other.status() == 409 && title(other).equals("Conflict"), "other parameters: " + other.status());
    check(other.body().get("data").asObject().get("message").asString().equals(OTHER_MESSAGE), other.text());
    check(!other.replayed(), "a conflict is not a replay");
    check(CALLS.get() == calls && LOADS.get() == loads, "nothing ran, nothing was loaded");

    entered = new CountDownLatch(1);
    release = new CountDownLatch(1);
    Map<String, String> key = Map.of("Idempotency-Key", "k-running");
    CompletableFuture<Answer> slow = HTTP
        .sendAsync(request(server, "POST", "/bookings", key, "{\"guests\":2}"), HttpResponse.BodyHandlers.ofString())
        .thenApply(IdempotencyHttp::answer);
    check(await(entered), "the first request reached the handler");
    Answer running = book(server, "k-running", 2);
    check(running.status() == 409, "a running request is 409: " + running.status());
    check(running.body().get("data").asObject().get("message").asString().equals(RUNNING_MESSAGE), running.text());
    release.countDown();
    Answer done = slow.get(20, TimeUnit.SECONDS);
    check(done.status() == 201, "the first request finished: " + done.status());
    Answer replay = book(server, "k-running", 2);
    check(replay.status() == 201 && replay.replayed(), "after it finished the key replays: " + replay.status());
  }

  private static boolean await(CountDownLatch latch) throws InterruptedException {
    return latch.await(20, TimeUnit.SECONDS);
  }

  /** A declared error is stored and replayed; a 500 is not stored, so the next request runs. */
  private static void aDeclaredErrorIsStoredAnd500IsNot(HttpServer server, Spy store) throws Exception {
    Answer full = book(server, "k-full", 20);
    check(full.status() == 409 && title(full).equals("RoomFull"), "a declared error: " + full.status());
    int loads = LOADS.get();
    Answer again = book(server, "k-full", 20);
    check(again.status() == 409 && again.replayed() && title(again).equals("RoomFull"), "the error is replayed");
    check(again.text().equals(full.text()), "with the same body");
    check(LOADS.get() == loads, "no fact for the replay");

    failNext = true;
    int releases = store.releases.get();
    Answer boom = book(server, "k-boom", 2);
    check(boom.status() == 500 && title(boom).equals("Internal"), "a failure is 500: " + boom.status());
    check(store.releases.get() == releases + 1, "a 500 releases the key");
    int calls = CALLS.get();
    Answer retry = book(server, "k-boom", 2);
    check(retry.status() == 201 && !retry.replayed(), "the next request runs: " + retry.status());
    check(CALLS.get() == calls + 1, "the handler ran again");
  }

  /** A required key that is missing, and a key that is not 1 to 255 visible ASCII characters, are 422. */
  private static void theKeyItself(HttpServer server) throws Exception {
    int calls = CALLS.get();
    Answer missing = send(server, "POST", "/bookings/paid", Map.of(), "{\"guests\":2}");
    check(missing.status() == 422, "a required key that is missing: " + missing.status());
    Json.Obj issue = missing.body().get("data").asObject().get("issues").asArray().items().get(0).asObject();
    check(issue.get("path").asString().equals("header.Idempotency-Key"), issue.write());
    check(issue.get("code").asString().equals("required") && issue.get("message").asString().equals("is required"), issue.write());
    check(missing.body().get("violations") == null, "a header issue is not a violation");
    Answer given = send(server, "POST", "/bookings/paid", Map.of("Idempotency-Key", "k-paid"), "{\"guests\":2}");
    check(given.status() == 201, "the key is there: " + given.status());
    for (String bad : new String[] {"has space", "k".repeat(256)}) {
      Answer a = book(server, bad, 2);
      check(a.status() == 422, "a malformed key is 422: " + bad.length() + " " + a.status());
      Json.Obj one = a.body().get("data").asObject().get("issues").asArray().items().get(0).asObject();
      check(one.get("path").asString().equals("header.Idempotency-Key"), one.write());
      check(one.get("code").asString().equals("type"), one.write());
      check(one.get("message").asString().equals("must be 1 to 255 visible ASCII characters"), one.write());
    }
    check(book(server, "k".repeat(255), 2).status() == 201, "255 characters are fine");
    check(CALLS.get() == calls + 2, "only the two good ones ran");
  }

  /** The answers of step 6 are stored too, a 412 included, and an ETag is replayed with its answer. */
  private static void aReplayKeepsTheEtagAndStatusOfStep6(HttpServer server) throws Exception {
    String path = "/bookings/rooms/" + ID;
    String body = "{\"name\":\"Blue\"}";
    Answer ok = send(server, "PUT", path, Map.of("Idempotency-Key", "k-shelf"), body);
    check(ok.status() == 200 && "\"5\"".equals(ok.header("etag")), "the ETag of the first answer: " + ok.header("etag"));
    int calls = CALLS.get();
    Answer again = send(server, "PUT", path, Map.of("Idempotency-Key", "k-shelf", "If-Match", "\"1\""), body);
    check(again.status() == 200 && again.replayed(), "a replay does not check If-Match: " + again.status());
    check("\"5\"".equals(again.header("etag")), "the replay carries the stored ETag: " + again.header("etag"));
    check(again.text().equals(ok.text()), "the stored body");
    check(CALLS.get() == calls, "the handler did not run");

    Answer stale = send(server, "PUT", path, Map.of("Idempotency-Key", "k-stale", "If-Match", "\"1\""), body);
    check(stale.status() == 412, "a stale tag with a new key: " + stale.status());
    Answer stored = send(server, "PUT", path, Map.of("Idempotency-Key", "k-stale", "If-Match", "\"4\""), body);
    check(stored.status() == 412 && stored.replayed(), "the 412 was stored: " + stored.status());
    check(stored.text().equals(stale.text()), "with its body");
  }

  /** A request that is not valid is refused before the store is asked, so it is never a replay. */
  private static void aRequestThatIsNotValidIsNotReplayed(HttpServer server) throws Exception {
    book(server, "k-valid", 2);
    Answer bad = book(server, "k-valid", 0);
    check(bad.status() == 422 && !bad.replayed(), "invalid parameters are refused first: " + bad.status());
    Answer both = send(server, "POST", "/bookings/paid", Map.of(), "{\"guests\":0}");
    check(both.status() == 422, "a missing key and an invalid body: " + both.status());
    var issues = both.body().get("data").asObject().get("issues").asArray().items();
    check(issues.size() == 2, "the header issue and the rule issue: " + both.body().write());
    check(issues.get(0).asObject().get("path").asString().equals("header.Idempotency-Key"), "header issues first");
    check(both.body().get("violations").asArray().items().size() == 1, "and the rule violation");
  }

  /** The scope of a key includes the caller of an endpoint with authentication. */
  private static void twoCallersDoNotReplayEachOther(HttpServer server, Spy store) throws Exception {
    Map<String, String> alice = Map.of("Idempotency-Token", "same", "Authorization", "Bearer alice");
    Map<String, String> bob = Map.of("Idempotency-Token", "same", "Authorization", "Bearer bob");
    String body = "{\"guests\":4}";
    int calls = CALLS.get();
    check(send(server, "POST", "/bookings/mine", alice, body).status() == 201, "alice runs");
    Answer other = send(server, "POST", "/bookings/mine", bob, body);
    check(other.status() == 201 && !other.replayed(), "bob does not get alice's answer: " + other.status());
    check(CALLS.get() == calls + 2, "both ran");
    Answer again = send(server, "POST", "/bookings/mine", alice, body);
    check(again.status() == 201 && again.replayed() && CALLS.get() == calls + 2, "alice's own key replays");
    String last = store.scopes.get(store.scopes.size() - 1);
    check(last.contains("Bookings.bookMine@") && last.endsWith("\nalice"), "the scope names the caller: " + last);
    Answer none = send(server, "POST", "/bookings/mine", Map.of("Authorization", "Bearer alice"), body);
    check(none.status() == 422, "the required key of another name: " + none.status());
    check(
        none.body().get("data").asObject().get("issues").asArray().items().get(0).asObject().get("path").asString()
            .equals("header.Idempotency-Token"),
        none.text());
  }

  /** Another header name, and a success without a body, are replayed as they were. */
  private static void theHeaderAndTheEmptyAnswer(HttpServer server) throws Exception {
    Map<String, String> key = Map.of("X-Request-Id", "k-put");
    String path = "/bookings/" + ID;
    Answer first = send(server, "PUT", path, key, "{\"guests\":2}");
    Answer again = send(server, "PUT", path, key, "{\"guests\":2}");
    check(first.status() == 200 && again.replayed() && again.text().equals(first.text()), "X-Request-Id works like the default");
    Map<String, String> del = Map.of("Idempotency-Key", "k-del");
    Answer gone = send(server, "DELETE", path, del, null);
    check(gone.status() == 204 && gone.text().isEmpty(), "204: " + gone.status());
    int calls = CALLS.get();
    Answer gone2 = send(server, "DELETE", path, del, null);
    check(gone2.status() == 204 && gone2.replayed() && gone2.text().isEmpty(), "a stored answer without a body");
    check(CALLS.get() == calls, "not run again");
  }

  /** begin that fails is 500 and nothing runs; complete and release that fail are only reported. */
  private static void aStoreThatFails(Spy store, HttpServer server) throws Exception {
    store.failBegin = true;
    int calls = CALLS.get();
    int errors = ERRORS.size();
    Answer down = book(server, "k-down", 2);
    check(down.status() == 500 && title(down).equals("Internal"), "begin fails: " + down.status());
    check(CALLS.get() == calls, "running without the store could run a request twice: the handler did not run");
    check(ERRORS.size() == errors + 1 && ERRORS.get(errors).equals("the store is down"), "onError saw it: " + ERRORS);
    store.failBegin = false;

    store.failComplete = true;
    Answer ok = book(server, "k-complete", 2);
    check(ok.status() == 201, "the answer is sent although complete failed: " + ok.status());
    check(ERRORS.get(ERRORS.size() - 1).equals("cannot store"), "complete failure reported: " + ERRORS);
    store.failComplete = false;

    store.failRelease = true;
    failNext = true;
    Answer boom = book(server, "k-release", 2);
    check(boom.status() == 500, "the 500 is sent although release failed: " + boom.status());
    check(ERRORS.get(ERRORS.size() - 1).equals("cannot release"), "release failure reported: " + ERRORS);
    store.failRelease = false;
  }

  /** A caller hook that fails is treated like a failing begin: 500, the store not asked, the handler not called. */
  private static void aCallerHookThatFails(Spy store, HttpServer server) throws Exception {
    Map<String, String> headers = Map.of("Idempotency-Token", "k-caller", "Authorization", "Bearer alice");
    String body = "{\"guests\":4}";
    failCaller = true;
    int calls = CALLS.get();
    int asked = store.scopes.size();
    int errors = ERRORS.size();
    Answer down = send(server, "POST", "/bookings/mine", headers, body);
    check(down.status() == 500 && title(down).equals("Internal"), "the caller hook fails: " + down.status());
    check(CALLS.get() == calls, "the handler did not run");
    check(store.scopes.size() == asked, "the store was not asked without a scope");
    check(
        ERRORS.size() == errors + 1 && ERRORS.get(errors).equals("the caller hook is down"),
        "onError saw it: " + ERRORS);
    check(!down.text().contains("caller hook"), "the message does not leak: " + down.text());
    failCaller = false;
    Answer up = send(server, "POST", "/bookings/mine", headers, body);
    check(up.status() == 201 && !up.replayed(), "nothing was stored by the failed request: " + up.status());
    check(CALLS.get() == calls + 1, "the handler ran once");
  }

  /** A hook that is missing fails when the handler is created, naming the first endpoint that needs it. */
  private static void aServerThatLacksAHook() {
    for (int missing = 0; missing < 2; missing++) {
      IdempotencyStore store = missing == 0 ? null : new MemoryIdempotencyStore();
      BookingsHooks hooks = new BookingsHooks((token, ctx) -> token, store, missing == 1 ? null : (p, c) -> "x");
      try {
        BookingsHandler.create(new Impl(), new Loaders(), hooks);
        throw new AssertionError("a server without a hook was created (" + missing + ")");
      } catch (IllegalStateException expected) {
        String endpoint = missing == 0 ? "book" : "bookMine";
        check(expected.getMessage().startsWith(endpoint + ": "), "names the endpoint: " + expected.getMessage());
        check(expected.getMessage().contains(missing == 0 ? "store" : "caller"), expected.getMessage());
      }
    }
  }
}
