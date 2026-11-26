package veldtest;

import gen.e.runtime.Events;
import gen.e.runtime.Json;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.Schema;
import gen.e.runtime.Transport;
import gen.e.runtime.VeldError;
import gen.e.runtime.Wire;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The event examples of a contract (given event ... expect S.handler called) against the GENERATED
 * publisher and subscribers of the Java server, over the memory transport.
 * crates/emitters/java/tests/events.rs writes cases.json: the subscribers, the filled-in events as
 * wire values and the same events as envelope bytes. Per example this prints:
 *
 * <ul>
 *   <li>raw: the handlers called when the message is put on the transport as it is (a rejected event
 *       reaches the consumer, which is what expect rejected means);
 *   <li>viaPublisher: the handlers called when the typed event goes through the generated
 *       publisher, or the title of the error the publisher threw (nothing is sent then).
 * </ul>
 */
public final class EventsExamples {
  private EventsExamples() {}

  public static void main(String[] args) throws Exception {
    System.setOut(new java.io.PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), true, StandardCharsets.UTF_8));
    Json.Obj cases = Json.parse(Files.readString(Path.of("cases.json"), StandardCharsets.UTF_8)).asObject();
    List<Json> services = cases.get("services").asArray().items();
    List<Json> out = new ArrayList<>();
    for (Json e : cases.get("examples").asArray().items()) {
      Json.Obj ex = e.asObject();
      out.add(
          Json.object()
              .put("name", ex.get("name"))
              .put("raw", names(raw(services, ex)))
              .put("viaPublisher", via(services, ex))
              .build());
    }
    System.out.println(Json.array(out).write());
  }

  private static Json names(List<String> called) {
    List<Json> out = new ArrayList<>();
    for (String s : called) {
      out.add(Json.of(s));
    }
    return Json.array(out);
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

  private static List<Transport.Subscription> subscribeAll(
      Transport transport, List<Json> services, List<String> called) throws Exception {
    List<Transport.Subscription> handles = new ArrayList<>();
    for (Json s : services) {
      Json.Obj service = s.asObject();
      String name = service.get("name").asString();
      Class<?> handlersType = Class.forName(service.get("handlers_class").asString());
      Object handlers =
          Proxy.newProxyInstance(
              handlersType.getClassLoader(),
              new Class<?>[] {handlersType},
              (proxy, method, args) -> {
                synchronized (called) {
                  called.add(name + "." + method.getName());
                }
                return null;
              });
      Class<?> subscriber = Class.forName(service.get("subscriber").asString());
      Method subscribe = subscriber.getMethod("subscribe", Transport.class, handlersType);
      handles.add((Transport.Subscription) invoke(subscribe, null, transport, handlers));
    }
    return handles;
  }

  private static List<String> raw(List<Json> services, Json.Obj ex) throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> called = new ArrayList<>();
    List<Transport.Subscription> handles = subscribeAll(transport, services, called);
    String key = ex.get("key") instanceof Json.Str s ? s.value() : null;
    transport.publish(
        ex.get("topic").asString(), key, ex.get("message").asString().getBytes(StandardCharsets.UTF_8));
    transport.idle();
    for (Transport.Subscription h : handles) {
      h.close();
    }
    Collections.sort(called);
    return called;
  }

  private static Json via(List<Json> services, Json.Obj ex) throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> called = new ArrayList<>();
    List<Transport.Subscription> handles = subscribeAll(transport, services, called);
    Json.Obj p = ex.get("publisher").asObject();
    Class<?> publisherClass = Class.forName(p.get("class").asString());
    Object publisher = invoke(publisherClass.getMethod("create", Transport.class), null, transport);
    Class<?> eventClass = Class.forName(p.get("event_class").asString());
    Schema schema = (Schema) eventClass.getField("SCHEMA").get(null);
    Json prepared = Wire.prepare(schema, ex.get("value"), Wire.Mode.RESPONSE);
    Object typed = invoke(eventClass.getMethod("fromWire", Json.class), null, prepared);
    Object channel = invoke(publisherClass.getMethod(p.get("channel").asString()), publisher);
    Method method = channel.getClass().getMethod(p.get("method").asString(), eventClass);
    Json refused = Json.NULL;
    try {
      Object receipt = invoke(method, channel, typed);
      if (!(receipt instanceof Events.PublishedEvent)) {
        throw new IllegalStateException("no receipt");
      }
    } catch (VeldError e) {
      refused = Json.of(e.title());
    }
    transport.idle();
    for (Transport.Subscription h : handles) {
      h.close();
    }
    Collections.sort(called);
    return Json.object().put("refused", refused).put("called", names(called)).build();
  }
}
