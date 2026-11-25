package veldtest;

import gen.rc.live.client.LiveClient;
import gen.rc.live.client.ReadingsRequest;
import gen.rc.live.live.Note;
import gen.rc.live.live.Reading;
import gen.rc.runtime.ClientOptions;
import gen.rc.runtime.VeldError;
import gen.rc.runtime.WsChannel;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

/**
 * Drives the Spring application of {@code spring-streams} through the generated client. Arguments:
 * the port and the output mode of the server ({@code strict}, {@code audit} or {@code off}).
 */
public final class RuleStreamsWs {
  private RuleStreamsWs() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    String base = "http://127.0.0.1:" + args[0];
    String mode = args[1];
    LiveClient client = new LiveClient(ClientOptions.builder().baseUrl(base).build());
    LiveClient noLocal = new LiveClient(ClientOptions.builder().baseUrl(base).localRules(false).build());

    // SSE: the second item violates.
    List<Object> got = new ArrayList<>();
    try (Stream<?> items = client.readings(new ReadingsRequest(1))) {
      items.forEach(got::add);
      check(!mode.equals("strict"), "strict must end the stream with an error");
    } catch (VeldError e) {
      check(mode.equals("strict") && e.title().equals("Internal"), "ended with Internal in strict only: " + e.title());
    }
    check(got.size() == (mode.equals("strict") ? 1 : 3), "SSE items delivered: " + got.size());

    // WebSocket, server to client: a valid reply, then a message the server breaks.
    try (WsChannel<Reading, Note> ws = client.notes()) {
      ws.send(new Note("hi", 3));
      Reading ok = ws.receive();
      check(ok.high() == 3, "a valid reply arrives");
      ws.send(new Note("bad", 3));
      try {
        Reading broken = ws.receive();
        check(!mode.equals("strict"), "strict must not deliver the violating reading");
        check(broken.low() > broken.high(), "the violating reading arrives unchanged in " + mode);
      } catch (VeldError e) {
        check(mode.equals("strict") && e.title().equals("Internal"), "Internal error frame in strict only: " + e.title());
      }
    }

    // WebSocket, client to server: the local check stops the message before it is written.
    try (WsChannel<Reading, Note> ws = client.notes()) {
      try {
        ws.send(new Note("x", 9));
        check(false, "the local check must reject stars = 9");
      } catch (VeldError e) {
        check(e.raisedLocally() && e.violations().size() == 1, "a local violation");
        check(e.title().equals("ValidationError"), "typed ValidationError: " + e.title());
        check(e.violations().get(0).path().equals("message"), "path root is message: " + e.violations().get(0).path());
      }
    }
    // With the local check off the server rejects the message the way it rejects a bad shape.
    try (WsChannel<Reading, Note> ws = noLocal.notes()) {
      ws.send(new Note("x", 9));
      try {
        ws.receive();
        check(false, "the server must reject the message");
      } catch (VeldError e) {
        check(e.title().equals("ValidationError") && !e.raisedLocally(), "the server's ValidationError");
        check(e.violations().size() == 1, "with its violations: " + e.violations().size());
        check(e.violations().get(0).path().equals("message"), "path root is message");
      }
    }
    System.out.println("OK");
  }
}
