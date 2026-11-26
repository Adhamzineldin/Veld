package veldtest;

import gen.e.runtime.Events;
import gen.e.runtime.Json;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.Schema;
import gen.e.runtime.Transport;
import gen.e.runtime.Wire;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;

/**
 * The wire goldens of events (tests/wire/events) run through the GENERATED publisher and subscriber
 * of the Java server. The Rust test (crates/emitters/java/tests/events.rs) writes cases.json from
 * envelopes.json and consume.json, so a new case needs no code here.
 *
 * <ul>
 *   <li>publish: the typed value is published through a transport that keeps the bytes, with the id
 *       source and the clock fixed; the bytes are printed as hex.
 *   <li>consume: a message is handed to the generated subscriber of a service that handles the
 *       listed events; what its handlers, hooks and the transport saw is printed.
 * </ul>
 *
 * Prints one JSON document on standard output.
 */
public final class EventsWire {
  private EventsWire() {}

  public static void main(String[] args) throws Exception {
    System.setOut(new java.io.PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), true, StandardCharsets.UTF_8));
    Json.Obj cases = Json.parse(Files.readString(Path.of("cases.json"), StandardCharsets.UTF_8)).asObject();
    List<Json> publishes = new ArrayList<>();
    for (Json c : cases.get("publish").asArray().items()) {
      publishes.add(publish(c.asObject()));
    }
    List<Json> consumes = new ArrayList<>();
    for (Json c : cases.get("consume").asArray().items()) {
      consumes.add(consume(c.asObject()));
    }
    System.out.println(
        Json.object().put("publish", Json.array(publishes)).put("consume", Json.array(consumes)).build().write());
  }

  private static String text(Json.Obj o, String key) {
    return o.get(key).asString();
  }

  private static Json orNull(String s) {
    return s == null ? Json.NULL : Json.of(s);
  }

  private static Object invoke(Method m, Object target, Object... args) throws Exception {
    try {
      return m.invoke(target, args);
    } catch (InvocationTargetException e) {
      if (e.getCause() instanceof Exception cause) {
        throw cause;
      }
      throw e;
    }
  }

  private static Json publish(Json.Obj c) throws Exception {
    List<Json> sent = new ArrayList<>();
    Transport recording =
        new Transport() {
          @Override
          public void publish(String topic, String key, byte[] bytes) {
            sent.add(
                Json.object()
                    .put("topic", Json.of(topic))
                    .put("key", orNull(key))
                    .put("hex", Json.of(HexFormat.of().formatHex(bytes)))
                    .build());
          }

          @Override
          public Subscription subscribe(String topic, String group, Deliver deliver) {
            return () -> {};
          }
        };
    String id = text(c, "id");
    Instant time = Instant.parse(text(c, "time"));
    Events.PublisherOptions options = new Events.PublisherOptions().newId(() -> id).clock(() -> time);
    Class<?> publisherClass = Class.forName(text(c, "publisher"));
    Object publisher =
        invoke(publisherClass.getMethod("create", Transport.class, Events.PublisherOptions.class), null, recording, options);
    Class<?> eventClass = Class.forName(text(c, "eventClass"));
    Schema schema = (Schema) eventClass.getField("SCHEMA").get(null);
    Json prepared = Wire.prepare(schema, c.get("value"), Wire.Mode.RESPONSE);
    Object typed = invoke(eventClass.getMethod("fromWire", Json.class), null, prepared);
    Object channel = invoke(publisherClass.getMethod(text(c, "channel")), publisher);
    Object receipt = invoke(channel.getClass().getMethod(text(c, "method"), eventClass), channel, typed);
    Events.PublishedEvent p = (Events.PublishedEvent) receipt;
    Json receiptJson =
        Json.object()
            .put("id", Json.of(p.id()))
            .put("source", Json.of(p.source()))
            .put("type", Json.of(p.type()))
            .put("subject", orNull(p.subject()))
            .put("time", Json.of(p.time()))
            .build();
    return Json.object().put("name", c.get("name")).put("sent", Json.array(sent)).put("receipt", receiptJson).build();
  }

  private static Json meta(Events.EventMeta m) {
    return Json.object()
        .put("id", Json.of(m.id()))
        .put("source", Json.of(m.source()))
        .put("type", Json.of(m.type()))
        .put("subject", m.subject() == null ? null : Json.of(m.subject()))
        .put("time", m.time() == null ? null : Json.of(m.time()))
        .put("attempt", Json.of((long) m.attempt()))
        .build();
  }

  private static Json consume(Json.Obj c) throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<Json> delivered = new ArrayList<>();
    List<Json> rejected = new ArrayList<>();
    List<Json> mismatches = new ArrayList<>();
    List<Json> rules = new ArrayList<>();
    Map<String, String> events = new HashMap<>();
    for (Json h : c.get("handlers").asArray().items()) {
      events.put(text(h.asObject(), "method"), text(h.asObject(), "event"));
    }
    Class<?> handlersType = Class.forName(text(c, "handlers_class"));
    Object handlers =
        Proxy.newProxyInstance(
            handlersType.getClassLoader(),
            new Class<?>[] {handlersType},
            (proxy, method, args) -> {
              Object typed = args[0];
              Json data = (Json) typed.getClass().getMethod("toWire").invoke(typed);
              delivered.add(
                  Json.object()
                      .put("event", Json.of(events.get(method.getName())))
                      .put("data", data)
                      .put("meta", meta((Events.EventMeta) args[1]))
                      .build());
              return null;
            });
    Events.ConsumerOptions options =
        new Events.ConsumerOptions()
            .onEventRejected(
                e ->
                    rejected.add(
                        Json.object()
                            .put("reason", Json.of(e.reason().text()))
                            .put("event", orNull(e.event()))
                            .put("violations", Json.of((long) e.violations().size()))
                            .put("hex", Json.of(HexFormat.of().formatHex(e.bytes())))
                            .build()))
            .onContractMismatch(
                e ->
                    mismatches.add(
                        Json.object()
                            .put("event", Json.of(e.event()))
                            .put("ownSignature", Json.of(e.ownSignature()))
                            .put("ownDigest", orNull(e.ownDigest()))
                            .put("signature", Json.of(e.signature()))
                            .put("digest", orNull(e.digest()))
                            .build()))
            .onRuleViolation(
                e ->
                    rules.add(
                        Json.object()
                            .put("rule", Json.of(e.rule()))
                            .put("outcome", Json.of(e.outcome().name().toLowerCase(java.util.Locale.ROOT)))
                            .build()));
    Class<?> subscriber = Class.forName(text(c, "subscriber"));
    Transport.Subscription handle =
        (Transport.Subscription)
            invoke(
                subscriber.getMethod("subscribe", Transport.class, handlersType, Events.ConsumerOptions.class),
                null,
                transport,
                handlers,
                options);
    String key = c.get("key") instanceof Json.Str s ? s.value() : null;
    transport.publish(text(c, "topic"), key, text(c, "message").getBytes(StandardCharsets.UTF_8));
    transport.idle();
    handle.close();
    return Json.object()
        .put("name", c.get("name"))
        .put("delivered", Json.array(delivered))
        .put("rejected", Json.array(rejected))
        .put("mismatches", Json.array(mismatches))
        .put("rules", Json.array(rules))
        .put("dead", Json.of((long) transport.deadLetters().size()))
        .build();
  }
}
