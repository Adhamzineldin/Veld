package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.bookings.server.FingerprintsHandler;
import gen.b.bookings.server.FingerprintsHooks;
import gen.b.bookings.server.FingerprintsService;
import gen.b.runtime.IdempotencyStore;
import gen.b.runtime.Json;
import gen.b.runtime.StoredAnswer;
import java.lang.reflect.Proxy;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Every case of {@code tests/wire/idempotency/fingerprints.json} (115 of them, the reference's
 * values) through the generated server: the request is sent over HTTP, the store records the
 * fingerprint {@code begin} is given, and it must equal the golden's; a case with {@code rejected}
 * must be answered with that status and never reach the store; every {@code same_as} and {@code
 * differs_from} is checked between the fingerprints this server computed. The argument is the path
 * of the file. The contract is {@code tests/wire/idempotency/fingerprints.veld}, as the package
 * {@code bookings}.
 */
public final class IdempotencyGoldens {
  private IdempotencyGoldens() {}

  private static final List<String> SEEN = Collections.synchronizedList(new ArrayList<>());

  private static final class Recording implements IdempotencyStore {
    @Override
    public Begin begin(String scope, String key, String fingerprint) {
      SEEN.add(fingerprint);
      return new Begin.Started();
    }

    @Override
    public void complete(String scope, String key, StoredAnswer answer) {}

    @Override
    public void release(String scope, String key) {}
  }

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static String enc(String s) {
    return URLEncoder.encode(s, StandardCharsets.UTF_8).replace("+", "%20");
  }

  private static String path(String decoded) {
    List<String> segments = new ArrayList<>();
    for (String segment : decoded.substring(1).split("/", -1)) {
      segments.add(enc(segment));
    }
    return "/" + String.join("/", segments);
  }

  private static String query(Json.Obj query) {
    List<String> pairs = new ArrayList<>();
    for (Map.Entry<String, Json> e : query.fields().entrySet()) {
      if (e.getValue() instanceof Json.Arr arr) {
        for (Json item : arr.items()) {
          pairs.add(enc(e.getKey()) + "=" + enc(((Json.Str) item).value()));
        }
      } else {
        pairs.add(enc(e.getKey()) + "=" + enc(((Json.Str) e.getValue()).value()));
      }
    }
    return pairs.isEmpty() ? "" : "?" + String.join("&", pairs);
  }

  private static String text(Json value) {
    return value instanceof Json.Str s ? s.value() : "";
  }

  public static void main(String[] args) throws Exception {
    List<Json> cases = ((Json.Arr) Json.parse(Files.readString(Path.of(args[0])))).items();
    FingerprintsService impl = (FingerprintsService) Proxy.newProxyInstance(
        IdempotencyGoldens.class.getClassLoader(), new Class<?>[] {FingerprintsService.class}, (p, m, a) -> null);
    HttpServer server = JdkHttpServerAdapter.listen(
        FingerprintsHandler.create(impl, new FingerprintsHooks(new Recording())), new InetSocketAddress("127.0.0.1", 0));
    List<String> problems = new ArrayList<>();
    Map<String, String> got = new HashMap<>();
    try {
      for (Json c : cases) {
        Json.Obj kase = (Json.Obj) c;
        String name = text(kase.get("name"));
        Json.Obj request = (Json.Obj) kase.get("request");
        String uri = "http://127.0.0.1:" + server.getAddress().getPort()
            + path(text(request.get("path"))) + query((Json.Obj) request.get("query"));
        HttpRequest.Builder builder = HttpRequest.newBuilder(URI.create(uri));
        for (Map.Entry<String, Json> h : ((Json.Obj) request.get("headers")).fields().entrySet()) {
          builder.header(h.getKey(), text(h.getValue()));
        }
        String body = request.has("body_text") ? text(request.get("body_text"))
            : request.has("body") ? request.get("body").write() : null;
        if (body != null) {
          builder.header("content-type", "application/json");
        }
        builder.method(text(request.get("method")),
            body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
        int before = SEEN.size();
        HttpResponse<String> response = HTTP.send(builder.build(), HttpResponse.BodyHandlers.ofString());
        if (kase.has("rejected")) {
          if (response.statusCode() != Integer.parseInt(text(kase.get("rejected")))) {
            problems.add(name + ": expected " + text(kase.get("rejected")) + " but got " + response.statusCode());
          }
          if (SEEN.size() != before) {
            problems.add(name + ": a rejected request reached the store");
          }
          continue;
        }
        if (response.statusCode() != 204) {
          problems.add(name + ": answered " + response.statusCode() + " " + response.body());
          continue;
        }
        if (SEEN.size() != before + 1) {
          problems.add(name + ": the store was asked " + (SEEN.size() - before) + " times");
          continue;
        }
        String fingerprint = SEEN.get(before);
        got.put(name, fingerprint);
        String expected = text(kase.get("fingerprint"));
        if (!expected.equals(fingerprint)) {
          problems.add(name + ": expected " + expected + " (" + text(kase.get("hashed")) + ") but got " + fingerprint);
        }
      }
      for (Json c : cases) {
        Json.Obj kase = (Json.Obj) c;
        String name = text(kase.get("name"));
        for (String relation : List.of("same_as", "differs_from")) {
          if (!kase.has(relation) || !got.containsKey(name) || !got.containsKey(text(kase.get(relation)))) {
            continue;
          }
          boolean same = got.get(name).equals(got.get(text(kase.get(relation))));
          if (same != relation.equals("same_as")) {
            problems.add(name + ": " + relation + " " + text(kase.get(relation)) + " does not hold");
          }
        }
      }
    } finally {
      server.stop(0);
    }
    if (!problems.isEmpty()) {
      throw new AssertionError(problems.size() + " of " + cases.size() + " cases differ:\n" + String.join("\n", problems));
    }
    System.out.println("OK " + cases.size());
  }
}
