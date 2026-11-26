package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.e.adapters.JdkHttpServerAdapter;
import gen.e.runtime.Json;
import gen.e.runtime.VeldHandler;
import gen.e.runtime.VeldRequest;
import gen.e.runtime.VeldResponse;
import java.net.InetSocketAddress;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * The generated test handlers of the services that subscribe (spec/events.md section 10.9), mounted
 * in one JDK server: {@code java veldtest.EventsServe <class> ...}, for example {@code
 * gen.e.bookings.server.LedgerTestHandler}. Prints {@code PORT=<n>} and serves until it is killed.
 *
 * <p>A generated test handler answers for its own service, and two of them cannot share the path
 * {@code /__veld/test/event}, so this program is the one server of a workspace: it hands each request
 * to every handler and merges the answers the way the runner reads one handler that serves several
 * services (the probe lists every service; the outcome is {@code rejected} when a service rejected
 * the message, else {@code handled} when one handled it; the counts are summed).
 */
public final class EventsServe {
  private EventsServe() {}

  private static VeldResponse json(int status, Json body) {
    return new VeldResponse(status, Map.of("content-type", "application/json"), body.write());
  }

  private static List<VeldResponse> answers(List<VeldHandler> handlers, VeldRequest request) {
    List<VeldResponse> out = new ArrayList<>();
    for (VeldHandler h : handlers) {
      out.add(h.handle(request).orElseThrow(() -> new IllegalStateException("a test handler did not answer")));
    }
    return out;
  }

  private static VeldResponse probe(List<VeldHandler> handlers, VeldRequest request) {
    Map<String, Json> digests = new LinkedHashMap<>();
    Map<String, Json> subscribes = new LinkedHashMap<>();
    for (VeldResponse r : answers(handlers, request)) {
      Json.Obj body = Json.parse(r.body()).asObject();
      digests.putAll(body.get("digests").asObject().fields());
      if (body.get("subscribes") instanceof Json.Obj s) {
        subscribes.putAll(s.fields());
      }
    }
    return json(
        200,
        Json.object()
            .put("veldTestHandler", Json.of(1L))
            .put("digests", new Json.Obj(digests))
            .put("subscribes", new Json.Obj(subscribes))
            .build());
  }

  private static VeldResponse event(List<VeldHandler> handlers, VeldRequest request) {
    List<VeldResponse> all = answers(handlers, request);
    Json.Obj rejected = null;
    boolean handled = false;
    Map<String, Json> called = new LinkedHashMap<>();
    long deadLetters = 0;
    for (VeldResponse r : all) {
      if (r.status() != 200) {
        return r;
      }
      Json.Obj body = Json.parse(r.body()).asObject();
      String outcome = body.get("outcome").asString();
      if (outcome.equals("rejected") && rejected == null) {
        rejected = body;
      }
      handled |= outcome.equals("handled");
      called.putAll(body.get("called").asObject().fields());
      deadLetters += (long) body.get("deadLetters").asDouble();
    }
    return json(
        200,
        Json.object()
            .put("outcome", Json.of(rejected != null ? "rejected" : handled ? "handled" : "ignored"))
            .put("reason", rejected == null ? Json.NULL : rejected.get("reason"))
            .put("called", new Json.Obj(called))
            .put("violations", rejected == null ? Json.array() : rejected.get("violations"))
            .put("deadLetters", Json.of(deadLetters))
            .build());
  }

  public static void main(String[] args) throws Exception {
    if (args.length == 0) {
      throw new IllegalArgumentException("usage: EventsServe <test handler class> ...");
    }
    List<VeldHandler> handlers = new ArrayList<>();
    for (String name : args) {
      handlers.add((VeldHandler) Class.forName(name).getMethod("create").invoke(null));
    }
    VeldHandler combined =
        new VeldHandler() {
          @Override
          public Optional<VeldResponse> handle(VeldRequest request) {
            if (request.method().equalsIgnoreCase("GET") && request.path().equals("/__veld/test")) {
              return Optional.of(probe(handlers, request));
            }
            if (request.method().equalsIgnoreCase("POST") && request.path().equals("/__veld/test/event")) {
              return Optional.of(event(handlers, request));
            }
            return Optional.empty();
          }
        };
    HttpServer server = JdkHttpServerAdapter.listen(combined, new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
