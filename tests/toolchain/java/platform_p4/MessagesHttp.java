package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.bookings.Messages;
import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.bookings.User;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.IntakeHandler;
import gen.b.bookings.server.IntakeHooks;
import gen.b.bookings.server.IntakeRequest;
import gen.b.bookings.server.IntakeService;
import gen.b.bookings.server.PayFacts;
import gen.b.bookings.server.PayRequest;
import gen.b.runtime.Catalogs;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.io.IOException;
import java.math.BigDecimal;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Message catalogs (spec/platform.md section 22) over the production handler and the generated
 * client of the workspace of tests/wire/messages (an {@code ar} and a {@code pt-br} catalog), with a
 * service {@code Intake} appended whose body is a {@code Booking}, so that the invariants of a
 * {@code Booking} can be sent. Expects the server in {@code gen.b} and the client in {@code gen.c},
 * the cases of tests/wire/messages/cases.json next to the working directory as {@code cases.json},
 * and the catalogs the compiler read as {@code catalogs.json}.
 *
 * <ul>
 *   <li>the rows of {@code headers} and of {@code messages} through {@code Catalogs}, and the
 *       catalogs and the keys the generated class embeds;
 *   <li>one request per kind of rule over the production handler: the language, the text, the {@code
 *       messageKey} and {@code Content-Language} of the answer, the event the hook hears;
 *   <li>the client: its catalogs, the keys of its local check, a response that carries {@code
 *       messageKey}.
 * </ul>
 *
 * Prints {@code OK}.
 */
public final class MessagesHttp {
  private MessagesHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID USER_ID = UUID.fromString("00000000-0000-4000-8000-0000000000aa");

  // The application.
  private static volatile User user = new User(USER_ID, false, false);
  private static volatile Booking booking = booking(Status.PENDING);
  private static volatile Booking result;
  private static final List<RuleWalk.Event> EVENTS = new CopyOnWriteArrayList<>();

  private static Booking booking(Status status) {
    return new Booking(
        ID, status, 3L, Instant.parse("2026-06-10T00:00:00Z"), Instant.parse("2026-06-12T00:00:00Z"), new BigDecimal("10.00"));
  }

  private static Map<String, Map<String, String>> catalogs;
  private static Json.Obj cases;

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static void same(Object got, Object expected, String what) {
    if (!java.util.Objects.equals(got, expected)) {
      throw new AssertionError(what + "\n  got      " + got + "\n  expected " + expected);
    }
  }

  private static final BookingsService PAY = new BookingsService() {
    @Override
    public Booking pay(
        PayRequest request, PayFacts facts, Long version, Transition<Status> transition, RequestContext ctx) {
      Booking base = booking;
      return result != null ? result : new Booking(request.id(), Status.PAID, base.revision(), base.start(), base.end(), base.total());
    }
  };

  private static final VeldHandler BOOKINGS = BookingsHandler.create(
      PAY,
      new BookingsFactLoaders() {
        @Override
        public Optional<Booking> booking(UUID key, RequestContext ctx) {
          return Optional.of(booking);
        }
      },
      new BookingsHooks((token, ctx) -> token.equals("me") ? user : null).onRuleViolation(EVENTS::add));

  private static final VeldHandler INTAKE = IntakeHandler.create(
      new IntakeService() {
        @Override
        public Booking intake(IntakeRequest request, RequestContext ctx) {
          return request.body();
        }
      },
      new IntakeHooks().onRuleViolation(EVENTS::add));

  private static VeldResponse dispatch(VeldRequest request) {
    Optional<VeldResponse> res = BOOKINGS.handle(request);
    return res.isPresent() ? res.get() : INTAKE.handle(request).orElseThrow(() -> new AssertionError("no route " + request.path()));
  }

  /** One answer: the response and its body as JSON. */
  private record Answer(VeldResponse res, Json.Obj doc) {
    String language() {
      return res.headers().get("content-language");
    }

    Json.Obj violation(int at) {
      return doc.get("violations").asArray().items().get(at).asObject();
    }

    String issue(int at, String member) {
      return doc.get("data").asObject().get("issues").asArray().items().get(at).asObject().get(member).asString();
    }
  }

  private static Answer send(String method, String path, String body, String language, Map<String, String> extra) {
    Map<String, List<String>> headers = new LinkedHashMap<>();
    headers.put("authorization", List.of("Bearer me"));
    extra.forEach((k, v) -> headers.put(k, List.of(v)));
    if (language != null) {
      headers.put("Accept-Language", List.of(language));
    }
    VeldResponse res = dispatch(new VeldRequest(method, path, null, headers, body, null));
    return new Answer(res, res.body() == null || res.body().isEmpty() ? null : Json.parse(res.body()).asObject());
  }

  private static String intakeBody(String... replace) {
    Map<String, String> m = new LinkedHashMap<>();
    m.put("id", "\"" + ID + "\"");
    m.put("revision", "\"1\"");
    m.put("start", "\"2026-06-12T00:00:00Z\"");
    m.put("end", "\"2026-06-10T00:00:00Z\"");
    m.put("total", "\"1.00\"");
    for (int i = 0; i < replace.length; i += 2) {
      m.put(replace[i], replace[i + 1]);
    }
    StringBuilder out = new StringBuilder("{");
    m.forEach((k, v) -> out.append(out.length() > 1 ? "," : "").append('"').append(k).append("\":").append(v));
    return out.append('}').toString();
  }

  private static String ar(String key) {
    return catalogs.get("ar").get(key);
  }

  private static String pt(String key) {
    return catalogs.get("pt-br").get(key);
  }

  private static String nullable(Json value) {
    return value == null || value instanceof Json.Null ? null : value.asString();
  }

  private static Map<String, Map<String, String>> catalogsOf(Json.Obj file) {
    Map<String, Map<String, String>> out = new TreeMap<>();
    file.fields().forEach((tag, texts) -> {
      Map<String, String> one = new TreeMap<>();
      texts.asObject().fields().forEach((k, v) -> one.put(k, v.asString()));
      out.put(tag, one);
    });
    return out;
  }

  private static void tables() {
    // The catalogs the class holds, as the compiler read them, and the languages of the table.
    same(new TreeMap<>(Messages.catalogs()), catalogs, "the catalogs of the server's class");
    same(new TreeMap<>(gen.c.bookings.Messages.catalogs()), catalogs, "the catalogs of the client's class");
    List<String> languages = new ArrayList<>();
    cases.get("languages").asArray().items().forEach(l -> languages.add(l.asString()));
    same(new java.util.TreeSet<>(Messages.catalogs().keySet()), new java.util.TreeSet<>(languages), "the languages");
    for (Json item : cases.get("headers").asArray().items()) {
      Json.Obj row = item.asObject();
      String header = nullable(row.get("header"));
      same(Catalogs.chooseLanguage(header, catalogs.keySet()), nullable(row.get("language")), "header " + header);
      same(gen.c.runtime.Catalogs.chooseLanguage(header, catalogs.keySet()), nullable(row.get("language")), "client, header " + header);
    }
    for (Json item : cases.get("messages").asArray().items()) {
      Json.Obj row = item.asObject();
      String header = nullable(row.get("header"));
      String key = row.get("key").asString();
      String note = row.get("note") == null ? key : row.get("note").asString();
      String chosen = Catalogs.chooseLanguage(header, catalogs.keySet());
      Catalogs.Text found = Catalogs.text(catalogs, chosen, key, row.get("default").asString());
      same(found.text(), row.get("text").asString(), note + ": " + header + " " + key);
      same(found.language(), nullable(row.get("language")), note + ": the language");
      gen.c.runtime.Catalogs.Text again =
          gen.c.runtime.Catalogs.text(catalogs, gen.c.runtime.Catalogs.chooseLanguage(header, catalogs.keySet()), key, row.get("default").asString());
      same(again.text(), row.get("text").asString(), "client, " + note);
    }
    // The fixed keys are the four of the table, and every key of the cases is one of the keys of a rule or a fixed one.
    same(
        List.of(Messages.TRANSITION, Messages.TRANSITION_NOT_DUE, Messages.VERSION, Messages.NOT_PERMITTED),
        List.of("veld/transition", "veld/transition-not-due", "veld/version", "veld/not-permitted"),
        "the fixed keys");
    java.util.Set<String> known = new java.util.HashSet<>(Messages.keys().values());
    known.addAll(List.of(Messages.TRANSITION, Messages.TRANSITION_NOT_DUE, Messages.VERSION, Messages.NOT_PERMITTED));
    for (Json item : cases.get("keys").asArray().items()) {
      Json.Obj row = item.asObject();
      // A denial has the key of its forbid, which the verdict function carries, not the table.
      if (!row.get("kind").asString().equals("denial")) {
        check(known.contains(row.get("key").asString()), row.get("what").asString() + ": " + row.get("key") + " is a key of the server");
      }
    }
  }

  private static void invariants() {
    // An invariant with a message: every language of the table, and the answer's Content-Language.
    String contract = "A booking must end after it starts";
    String[][] want = {
      {"ar", ar("bookings/Booking.endAfterStart"), "ar"},
      {"pt-BR", pt("bookings/Booking.endAfterStart"), "pt-br"},
      {null, contract, null},
      {"fr", contract, null},
      {"pt", contract, null},
      {"*", contract, null},
      {"ar;q=0.2, pt-br", pt("bookings/Booking.endAfterStart"), "pt-br"},
    };
    for (String[] w : want) {
      Answer a = send("POST", "/intake", intakeBody(), w[0], Map.of());
      same(a.res().status(), 422, String.valueOf(w[0]));
      same(a.doc().get("violations").asArray().items().size(), 1, "one violation");
      same(a.violation(0).get("message").asString(), w[1], "the violation in " + w[0]);
      same(a.violation(0).get("messageKey").asString(), "bookings/Booking.endAfterStart", "the key");
      same(a.issue(0, "message"), w[1], "the issue says the same");
      same(a.issue(0, "code"), "rule", "the code");
      same(a.language(), w[2], "Content-Language for " + w[0]);
    }

    // A rule without a message has its key and the text of the language: a catalog lacks it, so none is used.
    Answer a = send("POST", "/intake", intakeBody("start", "\"2026-06-10T00:00:00Z\"", "end", "\"2026-06-12T00:00:00Z\"", "total", "\"-1.00\""), "ar", Map.of());
    same(a.violation(0).get("messageKey").asString(), "bookings/Booking.totalKnown", "the key of a rule without a message");
    same(a.violation(0).get("message").asString(), "violates rule totalKnown", "the generated text");
    same(a.language(), null, "no message of the answer came from a catalog");

    // Two violations, one from the catalog: the answer says so.
    a = send("POST", "/intake", intakeBody("total", "\"-1.00\""), "ar", Map.of());
    List<String> keys = new ArrayList<>();
    a.doc().get("violations").asArray().items().forEach(v -> keys.add(v.asObject().get("messageKey").asString()));
    java.util.Collections.sort(keys);
    same(keys, List.of("bookings/Booking.endAfterStart", "bookings/Booking.totalKnown"), "both keys");
    same(a.language(), "ar", "one message came from the catalog");

    // A shape issue has no key and is not translated.
    a = send("POST", "/intake", intakeBody("total", "\"x\""), "ar", Map.of());
    same(a.res().status(), 422, "a shape issue");
    same(a.issue(0, "code"), "type", "the code");
    check(a.doc().get("violations") == null, "a shape issue has no violation");
    same(a.language(), null, "a shape issue is not translated");

    // The hook of the application hears the key and the text of the contract.
    EVENTS.clear();
    send("POST", "/intake", intakeBody(), "ar", Map.of());
    same(EVENTS.size(), 1, "one event");
    same(EVENTS.get(0).violation().messageKey(), "bookings/Booking.endAfterStart", "the hook hears the key");
    same(EVENTS.get(0).violation().message(), contract, "and the contract's text");
  }

  private static Answer pay(String amount, String language, Map<String, String> extra) {
    Map<String, String> headers = new LinkedHashMap<>();
    headers.put("if-match", "\"3\"");
    headers.putAll(extra);
    return send("POST", "/bookings/" + ID + "/pay", "{\"amount\":\"" + amount + "\"}", language, headers);
  }

  private static void statements() {
    // A named require: pt-BR lacks it, so the contract's text and no Content-Language; ar has it.
    Answer a = pay("0.00", "pt-BR", Map.of());
    same(a.res().status(), 422, "a named require");
    same(a.violation(0).get("message").asString(), "The amount must be positive", "the contract's text");
    same(a.violation(0).get("messageKey").asString(), "bookings/Bookings.pay.notZero", "its key");
    same(a.language(), null, "pt-BR lacks it");
    a = pay("0.00", "ar", Map.of());
    same(a.violation(0).get("message").asString(), ar("bookings/Bookings.pay.notZero"), "ar has it");
    same(a.issue(0, "message"), ar("bookings/Bookings.pay.notZero"), "the issue says the same");
    same(a.language(), "ar", "Content-Language");

    // An unnamed require has the key every unnamed one of the endpoint shares.
    a = pay("2000000.00", "ar", Map.of());
    same(a.violation(0).get("messageKey").asString(), "bookings/Bookings.pay.require", "the key of an unnamed require");
    same(a.violation(0).get("message").asString(), ar("bookings/Bookings.pay.require"), "its text");

    // The version guard: a fixed text, in the languages that have it.
    a = send("POST", "/bookings/" + ID + "/pay", "{\"amount\":\"5.00\"}", "pt-br", Map.of("if-match", "\"999\""));
    same(a.res().status(), 412, "a stale version");
    same(a.violation(0).get("messageKey").asString(), "veld/version", "the fixed key");
    same(a.violation(0).get("message").asString(), pt("veld/version"), "in pt-br");
    same(a.language(), "pt-br", "Content-Language");
    a = send("POST", "/bookings/" + ID + "/pay", "{\"amount\":\"5.00\"}", "ar", Map.of("if-match", "\"999\""));
    same(a.violation(0).get("messageKey").asString(), "veld/version", "the key in ar");
    same(a.language(), null, "ar has no text for it");

    // A move that is not legal.
    Booking original = booking;
    booking = booking(Status.PAID);
    a = pay("5.00", "ar;q=0.2, pt-br", Map.of());
    same(a.res().status(), 409, "a move that is not legal");
    same(a.violation(0).get("messageKey").asString(), "veld/transition", "its key");
    same(a.violation(0).get("message").asString(), pt("veld/transition"), "in pt-br");
    same(a.language(), "pt-br", "Content-Language");

    // The denial of a policy: the key of the forbid that was true, in the data and in the violation.
    booking = original;
    user = new User(USER_ID, true, false);
    EVENTS.clear();
    a = pay("5.00", "ar", Map.of());
    same(a.res().status(), 403, "a denial");
    same(EVENTS.size(), 1, "one event");
    same(EVENTS.get(0).violation().messageKey(), "bookings/Manage.forbid1", "the hook hears the key of the forbid");
    same(EVENTS.get(0).violation().message(), "A suspended account cannot manage bookings", "and the contract's text");
    same(a.violation(0).get("messageKey").asString(), "bookings/Manage.forbid1", "the key");
    same(a.violation(0).get("message").asString(), ar("bookings/Manage.forbid1"), "the text");
    same(a.doc().get("data").asObject().get("message").asString(), ar("bookings/Manage.forbid1"), "the message of the Forbidden");
    same(a.language(), "ar", "Content-Language");
    a = pay("5.00", "pt-br", Map.of());
    same(a.violation(0).get("messageKey").asString(), "bookings/Manage.forbid1", "the key in pt-br");
    same(a.doc().get("data").asObject().get("message").asString(), "A suspended account cannot manage bookings",
        "the catalog lacks the forbid: the contract text");
    same(a.language(), null, "no catalog was used");
    // The third forbid, counting the one without a message.
    user = new User(USER_ID, false, false);
    booking = booking(Status.CANCELLED);
    a = pay("5.00", "ar", Map.of());
    same(a.violation(0).get("messageKey").asString(), "bookings/Manage.forbid3", "the third forbid");
    same(a.doc().get("data").asObject().get("message").asString(), "A cancelled booking cannot be managed", "its text");
    same(a.language(), null, "ar lacks it");
    // A forbid without a message: the fixed text.
    booking = original;
    user = new User(USER_ID, false, true);
    String[][] denials = {
      {"ar-EG", ar("veld/not-permitted"), "ar"},
      {"pt-br", pt("veld/not-permitted"), "pt-br"},
      {null, "not permitted", null},
    };
    for (String[] d : denials) {
      a = pay("5.00", d[0], Map.of());
      same(a.res().status(), 403, "a denial without a message");
      same(a.violation(0).get("messageKey").asString(), "veld/not-permitted", "the fixed key");
      same(a.violation(0).get("message").asString(), d[1], "the violation in " + d[0]);
      same(a.doc().get("data").asObject().get("message").asString(), d[1], "the Forbidden in " + d[0]);
      same(a.language(), d[2], "Content-Language for " + d[0]);
    }
    user = new User(USER_ID, false, false);

    // An ensure is an output: the answer is a 500 with no text, and the hook hears the key.
    EVENTS.clear();
    result = new Booking(ID, Status.PAID, -1L, original.start(), original.end(), original.total());
    a = pay("5.00", "ar", Map.of());
    same(a.res().status(), 500, "an output violation");
    same(a.language(), null, "a 500 has no text");
    same(EVENTS.get(0).violation().messageKey(), "bookings/Bookings.pay.ensure", "the hook hears the key of the ensure");
    same(EVENTS.get(0).violation().message(), "The revision never goes down", "and the contract's text");
    // The target check of the move has the key of the transition.
    EVENTS.clear();
    result = new Booking(ID, Status.PENDING, 3L, original.start(), original.end(), original.total());
    pay("5.00", "ar", Map.of());
    same(EVENTS.get(0).violation().messageKey(), "veld/transition", "the target check has the key of the transition");
    result = null;

    // A request with no header, and an answer that carries none of it, as before.
    a = pay("0.00", null, Map.of());
    same(a.language(), null, "no header, no Content-Language");
    same(a.violation(0).get("messageKey").asString(), "bookings/Bookings.pay.notZero", "the key without a header");
    same(a.violation(0).get("message").asString(), "The amount must be positive", "the contract's text without a header");
  }

  /** The server of the production handlers over a socket, for the client. */
  private static HttpServer serve() throws IOException {
    HttpServer http = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    http.createContext("/", exchange -> {
      String body = new String(exchange.getRequestBody().readAllBytes(), StandardCharsets.UTF_8);
      Map<String, List<String>> headers = new LinkedHashMap<>(exchange.getRequestHeaders());
      headers.put("authorization", List.of("Bearer me"));
      VeldResponse res = dispatch(new VeldRequest(
          exchange.getRequestMethod(), exchange.getRequestURI().getPath(), exchange.getRequestURI().getRawQuery(), headers, body, null));
      byte[] bytes = (res.body() == null ? "" : res.body()).getBytes(StandardCharsets.UTF_8);
      res.headers().forEach((k, v) -> exchange.getResponseHeaders().add(k, v));
      exchange.sendResponseHeaders(res.status(), bytes.length == 0 ? -1 : bytes.length);
      if (bytes.length > 0) {
        exchange.getResponseBody().write(bytes);
      }
      exchange.close();
    });
    http.start();
    return http;
  }

  private static gen.c.bookings.bookings.Booking clientBooking() {
    return new gen.c.bookings.bookings.Booking(
        ID, gen.c.bookings.bookings.Status.PENDING, 1L, Instant.parse("2026-06-12T00:00:00Z"), Instant.parse("2026-06-10T00:00:00Z"), new BigDecimal("1.00"));
  }

  private static void client() throws Exception {
    HttpServer http = serve();
    try {
      String url = "http://127.0.0.1:" + http.getAddress().getPort();
      // The violations of the local check have the key, and the text of the contract; the app picks its own language.
      gen.c.bookings.client.IntakeClient local = new gen.c.bookings.client.IntakeClient(
          gen.c.runtime.ClientOptions.builder().baseUrl(url).build());
      EVENTS.clear();
      try {
        local.intake(new gen.c.bookings.client.IntakeRequest(clientBooking()));
        throw new AssertionError("the local check refuses it");
      } catch (gen.c.runtime.VeldError e) {
        check(e.raisedLocally(), "raised locally");
        gen.c.runtime.Violation v = e.violations().get(0);
        same(v.messageKey(), "bookings/Booking.endAfterStart", "the key of the local check");
        same(v.message(), "A booking must end after it starts", "the contract's text");
        check(EVENTS.isEmpty(), "nothing was sent");
        String language = gen.c.runtime.Catalogs.chooseLanguage("ar-EG", gen.c.bookings.Messages.catalogs().keySet());
        same(language, "ar", "the app's language");
        same(
            gen.c.runtime.Catalogs.text(gen.c.bookings.Messages.catalogs(), language, v.messageKey(), v.message()).text(),
            ar("bookings/Booking.endAfterStart"),
            "the app shows its own language");
        // The keys of the rules are exported with the catalogs.
        same(gen.c.bookings.Messages.keys().get(v.rule()), "bookings/Booking.endAfterStart", "the key of the rule id");
      }

      // A response that carries messageKey (and the text of the server's language) keeps both.
      gen.c.bookings.client.IntakeClient remote = new gen.c.bookings.client.IntakeClient(
          gen.c.runtime.ClientOptions.builder()
              .baseUrl(url)
              .headers(Map.of("accept-language", "pt-br"))
              .localRules(false)
              .build());
      try {
        remote.intake(new gen.c.bookings.client.IntakeRequest(clientBooking()));
        throw new AssertionError("the server refuses it");
      } catch (gen.c.runtime.VeldError e) {
        check(!e.raisedLocally(), "raised by the server");
        gen.c.runtime.Violation v = e.violations().get(0);
        same(v.messageKey(), "bookings/Booking.endAfterStart", "the key of a response");
        same(v.message(), pt("bookings/Booking.endAfterStart"), "the text of the server's language");
      }
    } finally {
      http.stop(0);
    }
  }

  public static void main(String[] args) throws Exception {
    cases = Json.parse(Files.readString(Path.of("cases.json"))).asObject();
    catalogs = catalogsOf(Json.parse(Files.readString(Path.of("catalogs.json"))).asObject());
    tables();
    invariants();
    statements();
    client();
    System.out.println("OK");
  }
}
