package veldtest;

import gen.b.events.no_values_events.Noted;
import gen.b.events.server.ReaderEventHandlers;
import gen.b.events.server.ReaderSubscriber;
import gen.b.events.server.WriterPublisher;
import gen.b.runtime.Events;
import gen.b.runtime.Issue;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryTransport;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.Transport;
import gen.b.runtime.VeldError;
import gen.b.runtime.Violation;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Platform phase P4, part 1 (events): the rejections of the events runtime never repeat a value.
 * The marker string goes through every path that writes a message: a publisher that refuses a value
 * of the wrong shape and one that violates an invariant, and a consumer that is given a malformed
 * envelope, a message of the wrong shape, one with another content type and one that violates a
 * rule. Every message, issue and violation text is searched for the marker. Expects the server
 * generated from tests/toolchain/contract/platform_p4/no_values_events.veld in {@code gen.b}.
 * Prints {@code OK}.
 */
public final class NoValuesEvents {
  private NoValuesEvents() {}

  private static final String MARKER = "MARKER-3f9a71c2e5";
  private static final UUID ID = UUID.fromString("5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21");
  private static final List<String[]> TEXTS = new ArrayList<>();

  private static void note(String where, String text) {
    if (text != null) {
      TEXTS.add(new String[] {where, text});
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static void collect(String where, VeldError e) {
    note(where + " message", e.getMessage());
    note(where + " title", e.title());
    if (e.dataWire() instanceof Json.Obj data && data.get("issues") instanceof Json.Arr issues) {
      for (Json i : issues.items()) {
        note(where + " issue", i.asObject().get("message").asString());
      }
    }
    for (Violation v : e.violations()) {
      note(where + " violation", v.message());
    }
  }

  private static void send(MemoryTransport transport, String text) throws InterruptedException {
    transport.publish("notes.v1", ID.toString(), text.getBytes(StandardCharsets.UTF_8));
    transport.idle();
  }

  public static void main(String[] args) throws Exception {
    MemoryTransport memory = new MemoryTransport();
    List<byte[]> wire = new CopyOnWriteArrayList<>();
    memory.subscribe("notes.v1", "recorder", message -> {
      wire.add(message.bytes());
      return Transport.Outcome.DONE;
    });
    List<Events.EventRejected> rejected = new CopyOnWriteArrayList<>();
    List<RuleWalk.Event> ruleEvents = new CopyOnWriteArrayList<>();
    ReaderSubscriber.subscribe(
        memory,
        (ReaderEventHandlers) (event, meta) -> {},
        new Events.ConsumerOptions().onEventRejected(rejected::add).onRuleViolation(ruleEvents::add));
    WriterPublisher writer = WriterPublisher.create(memory, new Events.PublisherOptions().onRuleViolation(ruleEvents::add));

    // A good message, to build the others from.
    writer.notes().noted(new Noted(ID, "abcd", 2));
    memory.idle();
    check(wire.size() == 1, "the good message was published");
    Json.Obj base = Json.parse(new String(wire.get(0), StandardCharsets.UTF_8)).asObject();

    // The publisher: a text of the wrong length, a count out of bounds and an odd count.
    String[] attempts = {"a text of the wrong length", "a count out of bounds", "an odd count"};
    Noted[] values = {new Noted(ID, MARKER, 2), new Noted(ID, "abcd", 99), new Noted(ID, "abcd", 3)};
    for (int i = 0; i < attempts.length; i++) {
      try {
        writer.notes().noted(values[i]);
        throw new AssertionError("publish: " + attempts[i] + " is refused");
      } catch (VeldError e) {
        collect("publish " + attempts[i], e);
      }
    }

    // The consumer: a malformed message, an envelope with another content type, one of the wrong
    // shape and one that violates a rule.
    send(memory, MARKER);
    send(memory, "\"" + MARKER + "\"");
    send(memory, base.with("specversion", Json.of(MARKER)).write());
    send(memory, base.with("id", Json.of(7L)).with("source", Json.of(MARKER)).write());
    send(memory, base.with("subject", Json.of(7L)).write());
    send(memory, base.with("datacontenttype", Json.of(MARKER)).write());
    send(memory, base.with("data", Json.of(MARKER)).write());
    send(memory, base.with("data", Json.object().put("id", Json.of(MARKER)).put("text", Json.of(MARKER)).put("count", Json.of(MARKER)).build()).write());
    send(memory, base.with("data", Json.object().put("id", Json.of(ID.toString())).put("text", Json.of("abcd")).put("count", Json.of(3L)).build()).write());
    send(memory, base.with("type", Json.of(MARKER)).write());
    check(rejected.size() >= 6, "the messages were rejected (" + rejected.size() + ")");
    for (Events.EventRejected r : rejected) {
      note("rejected " + r.reason() + " message", r.message());
      for (Issue i : r.issues()) {
        note("rejected issue", i.message());
      }
      for (Violation v : r.violations()) {
        note("rejected violation", v.message());
      }
    }
    for (RuleWalk.Event e : ruleEvents) {
      note("rule violation event", e.violation().message());
    }

    check(TEXTS.size() > 10, "texts were collected (" + TEXTS.size() + ")");
    for (String[] t : TEXTS) {
      check(!t[1].contains(MARKER), t[0] + " repeats the value: " + t[1]);
    }
    System.out.println("OK");
  }
}
