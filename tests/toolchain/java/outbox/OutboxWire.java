package veldtest;

import gen.b.runtime.Events;
import gen.b.runtime.HandlerHooks;
import gen.b.runtime.Json;
import gen.b.runtime.Outbox;
import gen.b.runtime.OutboxEntry;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Schema;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.runtime.Wire;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * The {@code entries} of {@code tests/wire/events/outbox.json} through the GENERATED {@code emit}
 * (spec/events.md section 11.1): per case the handler of a probe endpoint calls {@code
 * emit.<event>(value)} with the id source and the clock of the hooks fixed, and the outbox that
 * records what it is given must have exactly the topic, the key and the bytes of the golden. There
 * is no transport to publish to. {@code cases.json} is written by {@code tests/events_outbox.rs}: the
 * generated classes of one service and the cases of one channel. Port of {@code
 * events_outbox_wire.ts}. Prints {@code OK}.
 */
public final class OutboxWire {
  private OutboxWire() {}

  private static final String PROBE = "00000000-0000-4000-8000-0000000000aa";

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
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

  /** What the outbox of a case was given. */
  private record Saved(String topic, String key, byte[] bytes) {}

  private static void run(Json.Obj group, Json.Obj c) throws Exception {
    String name = c.get("name").asString();
    String event = c.get("event").asString();
    List<Saved> saved = new CopyOnWriteArrayList<>();
    Outbox outbox = new Outbox() {
      @Override
      public void save(String topic, String key, byte[] bytes) {
        saved.add(new Saved(topic, key, bytes));
      }

      @Override
      public List<OutboxEntry> pending(int limit) {
        return List.of();
      }

      @Override
      public void done(OutboxEntry entry) { }
    };
    Class<?> eventClass = Class.forName(c.get("eventClass").asString());
    Class<?> boxClass = Class.forName(c.get("boxClass").asString());
    Schema schema = (Schema) eventClass.getField("SCHEMA").get(null);
    Object typed = invoke(
        eventClass.getMethod("fromWire", Json.class),
        null,
        Wire.prepare(schema, c.get("value"), Wire.Mode.RESPONSE));
    Method fromWire = boxClass.getMethod("fromWire", Json.class);
    Json box = Json.object().put("id", Json.of(PROBE)).put("state", Json.of("a")).build();
    Json moved = Json.object().put("id", Json.of(PROBE)).put("state", Json.of("b")).build();

    Class<?> serviceClass = Class.forName(group.get("service").asString());
    // The probe endpoint `zz<Event>` writes the event of the case and answers its move.
    Object impl = Proxy.newProxyInstance(
        serviceClass.getClassLoader(),
        new Class<?>[] {serviceClass},
        (proxy, method, args) -> {
          if (!method.getName().equals("zz" + event)) {
            throw new UnsupportedOperationException(method.getName());
          }
          for (Object arg : args) {
            if (arg != null && arg.getClass().getSimpleName().endsWith("Emit")) {
              invoke(arg.getClass().getMethod(c.get("method").asString(), eventClass), arg, typed);
            }
          }
          return fromWire.invoke(null, moved);
        });
    Class<?> loadersClass = Class.forName(group.get("loaders").asString());
    Object loaders = Proxy.newProxyInstance(
        loadersClass.getClassLoader(),
        new Class<?>[] {loadersClass},
        (proxy, method, args) -> Optional.of(fromWire.invoke(null, box)));
    Class<?> hooksClass = Class.forName(group.get("hooks").asString());
    Object hooks = hooksClass.getConstructor(Outbox.class).newInstance(outbox);
    Instant time = Instant.parse(c.get("time").asString());
    String id = c.get("id").asString();
    hooks = invoke(
        hooksClass.getMethod("events", Events.PublisherOptions.class),
        hooks,
        new Events.PublisherOptions().newId(() -> id).clock(() -> time));
    List<Throwable> errors = new ArrayList<>();
    hooks = invoke(
        hooksClass.getMethod("onError", HandlerHooks.ErrorHook.class),
        hooks,
        (HandlerHooks.ErrorHook) (error, ctx) -> errors.add(error));
    VeldHandler handler = (VeldHandler) invoke(
        Class.forName(group.get("handler").asString()).getMethod("create", serviceClass, loadersClass, hooksClass),
        null,
        impl,
        loaders,
        hooks);
    VeldResponse res = handler
        .handle(new VeldRequest("POST", group.get("at").asString() + "/zz/" + PROBE + "/" + event, "", Map.of(), null, null))
        .orElseThrow(() -> new AssertionError(name + ": no route"));
    check(res.status() < 300 && errors.isEmpty(), name + ": " + res.status() + " " + res.body() + " " + errors);
    check(saved.size() == 1, name + ": one entry, got " + saved.size());
    Saved entry = saved.get(0);
    check(entry.topic().equals(c.get("topic").asString()), name + ": topic " + entry.topic());
    String key = c.get("key") instanceof Json.Str s ? s.value() : null;
    check(java.util.Objects.equals(entry.key(), key), name + ": key " + entry.key());
    check(new String(entry.bytes(), StandardCharsets.UTF_8).equals(c.get("canonical").asString()), name + ": bytes");
  }

  public static void main(String[] args) throws Exception {
    Json.Obj group = Json.parse(Files.readString(Path.of("cases.json"), StandardCharsets.UTF_8)).asObject();
    check(!group.get("cases").asArray().items().isEmpty(), "no cases");
    for (Json c : group.get("cases").asArray().items()) {
      run(group, c.asObject());
    }
    System.out.println("OK");
  }
}
