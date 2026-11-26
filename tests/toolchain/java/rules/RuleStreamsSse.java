package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.rc.live.client.LiveClient;
import gen.rc.live.client.ReadingsRequest;
import gen.rc.runtime.ClientOptions;
import gen.rc.runtime.VeldError;
import gen.rs.adapters.JdkHttpServerAdapter;
import gen.rs.live.live.Overbooked;
import gen.rs.live.live.Reading;
import gen.rs.live.server.LiveHandler;
import gen.rs.live.server.LiveHooks;
import gen.rs.live.server.LiveService;
import gen.rs.runtime.RequestContext;
import gen.rs.runtime.RuleWalk;
import gen.rs.runtime.SseEvent;
import java.net.InetSocketAddress;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.stream.IntStream;
import java.util.stream.Stream;

/**
 * An SSE item that violates its rule, on the JDK server: {@code STRICT} ends the stream the way a
 * server failure does (an error before the first event is a plain 500), {@code AUDIT} sends every
 * item and reports, {@code OFF} checks nothing.
 */
public final class RuleStreamsSse {
  private RuleStreamsSse() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static final class Impl implements LiveService {
    @Override
    public Stream<SseEvent<Reading>> readings(gen.rs.live.server.ReadingsRequest request, RequestContext ctx) {
      int bad = request.bad();
      return IntStream.range(0, 3)
          .mapToObj(i -> SseEvent.of(i == bad ? new Reading(5, 1) : new Reading(1, 2)));
    }

    @Override
    public Integer seats(gen.rs.live.server.SeatsRequest request, RequestContext ctx) {
      throw new Overbooked(new Overbooked.Data(2, 5));
    }
  }

  private static LiveClient serve(RuleWalk.OutputMode mode, List<RuleWalk.Event> events, List<HttpServer> servers)
      throws Exception {
    HttpServer server = JdkHttpServerAdapter.listen(
        LiveHandler.create(
            new Impl(),
            new LiveHooks().onRuleViolation(events::add),
            gen.rs.runtimeconfig.GenRsRuntime.builder().rulesOutput(mode).build()),
        new InetSocketAddress("127.0.0.1", 0));
    servers.add(server);
    return new LiveClient(
        ClientOptions.builder().baseUrl("http://127.0.0.1:" + server.getAddress().getPort()).build());
  }

  public static void main(String[] args) throws Exception {
    List<HttpServer> servers = new ArrayList<>();
    try {
      List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
      LiveClient strict = serve(RuleWalk.OutputMode.STRICT, events, servers);
      check(strict.readings(new ReadingsRequest(7)).count() == 3, "no violating item: three items");
      check(events.isEmpty(), "no event without a violation");
      // The second item violates: the first arrives, then the stream ends with Internal.
      List<Object> got = new ArrayList<>();
      try (Stream<?> items = strict.readings(new ReadingsRequest(1))) {
        items.forEach(got::add);
        check(false, "the stream must end with an error");
      } catch (VeldError e) {
        check(e.title().equals("Internal"), "ends as an Internal error: " + e.title());
      }
      check(got.size() == 1, "only the valid item before it was sent: " + got.size());
      check(events.size() == 1, "one event: " + events.size());
      check(events.get(0).position() == RuleWalk.Position.OUTPUT, "output position");
      check(events.get(0).outcome() == RuleWalk.Outcome.REJECTED, "rejected");
      // The first item violates: nothing was sent yet, so this is an ordinary 500.
      try {
        strict.readings(new ReadingsRequest(0));
        check(false, "a violating first item is a 500 response");
      } catch (VeldError e) {
        check(e.title().equals("Internal"), "500 Internal before the first event: " + e.title());
      }

      List<RuleWalk.Event> auditEvents = Collections.synchronizedList(new ArrayList<>());
      LiveClient audit = serve(RuleWalk.OutputMode.AUDIT, auditEvents, servers);
      check(audit.readings(new ReadingsRequest(1)).count() == 3, "audit sends every item");
      check(auditEvents.size() == 1, "audit reports one violation: " + auditEvents.size());
      check(auditEvents.get(0).outcome() == RuleWalk.Outcome.AUDITED, "audited");

      List<RuleWalk.Event> offEvents = Collections.synchronizedList(new ArrayList<>());
      LiveClient off = serve(RuleWalk.OutputMode.OFF, offEvents, servers);
      check(off.readings(new ReadingsRequest(1)).count() == 3, "off sends every item");
      check(offEvents.isEmpty(), "off evaluates nothing");
    } finally {
      servers.forEach(s -> s.stop(0));
    }
    System.out.println("OK");
  }
}
