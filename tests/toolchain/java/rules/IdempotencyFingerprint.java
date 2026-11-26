package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.fp.fp.Body;
import gen.b.fp.fp.Guests;
import gen.b.fp.server.FHandler;
import gen.b.fp.server.FHooks;
import gen.b.fp.server.FService;
import gen.b.fp.server.GuestsRequest;
import gen.b.fp.server.MakeRequest;
import gen.b.fp.server.PingRequest;
import gen.b.runtime.IdempotencyStore;
import gen.b.runtime.RequestContext;
import gen.b.runtime.StoredAnswer;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The fingerprints of the reference (crates/veld-rules, {@code idempotency.rs} and {@code
 * version_idempotency.rs}), computed by the generated runtime: the first 16 hex characters of the
 * SHA-256 of the RFC 8785 JSON of the bound parameters in wire form. A store that only records what
 * it is asked sees them. The key itself is not part of a fingerprint, defaults are applied and an
 * optional that was not sent is left out.
 */
public final class IdempotencyFingerprint {
  private IdempotencyFingerprint() {}

  private static final String ID = "00000000-0000-4000-8000-000000000001";
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

  private static final class Impl implements FService {
    @Override
    public Body make(MakeRequest request, RequestContext ctx) {
      return request.body();
    }

    @Override
    public Guests guests(GuestsRequest request, RequestContext ctx) {
      return request.body();
    }

    @Override
    public void ping(PingRequest request, RequestContext ctx) {}
  }

  private static final HttpClient HTTP = HttpClient.newHttpClient();

  private static String fingerprintOf(HttpServer server, String method, String path, String key, String body)
      throws Exception {
    HttpRequest.Builder builder = HttpRequest.newBuilder(
        URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path));
    builder.header("Idempotency-Key", key);
    if (body != null) {
      builder.header("content-type", "application/json");
    }
    builder.method(method, body == null ? HttpRequest.BodyPublishers.noBody() : HttpRequest.BodyPublishers.ofString(body));
    int before = SEEN.size();
    HttpResponse<String> response = HTTP.send(builder.build(), HttpResponse.BodyHandlers.ofString());
    check(response.statusCode() < 300, path + " answered " + response.statusCode() + ": " + response.body());
    check(SEEN.size() == before + 1, "the store was asked once for " + path);
    return SEEN.get(before);
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static void expect(String what, String expected, String actual) {
    check(expected.equals(actual), what + ": expected " + expected + " but got " + actual);
  }

  public static void main(String[] args) throws Exception {
    HttpServer server = JdkHttpServerAdapter.listen(
        FHandler.create(new Impl(), new FHooks(new Recording())), new InetSocketAddress("127.0.0.1", 0));
    try {
      String put = "/f/" + ID;
      // `note` has a default and is added; `tag` and `dry` were not sent and are not there.
      String plain = fingerprintOf(server, "PUT", put, "k-1", "{\"n\":1}");
      expect("defaults applied, optionals left out", "bc508ee59e0a79b7", plain);
      // The same request with the default written out has the same fingerprint, whatever the key.
      expect("the default written out", plain, fingerprintOf(server, "PUT", put, "another-key", "{\"n\":1,\"note\":\"none\"}"));
      // An optional parameter that was sent counts, and so does an optional member.
      expect(
          "optionals that were sent",
          "62c08a8b20a7323e",
          fingerprintOf(server, "PUT", put + "?dry=true", "k-3", "{\"n\":1,\"tag\":\"a\"}"));
      expect("a body of guests: 2", "74ff6d8fbb076cfc", fingerprintOf(server, "POST", "/f/guests", "k-4", "{\"guests\":2}"));
      expect("a body of guests: 3", "19b1b33ecd669a6f", fingerprintOf(server, "POST", "/f/guests", "k-5", "{\"guests\":3}"));
      // No parameters at all: the fingerprint of the empty object.
      expect("no parameters", "44136fa355b3678a", fingerprintOf(server, "POST", "/f/ping", "k-6", null));
    } finally {
      server.stop(0);
    }
    System.out.println("OK");
  }
}
