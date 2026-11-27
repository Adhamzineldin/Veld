package veldtest;

import gen.e.runtime.Events;
import gen.e.runtime.Json;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.Transport;
import gen.e.runtime.VeldError;
import gen.e.sales.sales.Sold;
import gen.e.sales.server.LedgerEventHandlers;
import gen.e.sales.server.LedgerSubscriber;
import gen.e.sales.server.ShopPublisher;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Derived fields of an event (spec/rules.md section 17.2): the publisher computes them before the
 * invariants and replaces what the application put there (a derived field that fails, or an
 * invariant that reads one, stops the publish), and a subscriber computes them again from the data of
 * the message: a message that carries another value for a derived field is not believed. Prints
 * {@code derived events ok}.
 */
public final class DerivedEvents {
  private DerivedEvents() {}

  private static final String TOPIC = "test.sales";

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  public static void main(String[] args) throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<byte[]> wire = new CopyOnWriteArrayList<>();
    transport.subscribe(TOPIC, "raw", message -> {
      wire.add(message.bytes());
      return Transport.Outcome.DONE;
    });
    List<Integer> heard = new CopyOnWriteArrayList<>();
    List<Events.EventRejected> rejected = new CopyOnWriteArrayList<>();
    LedgerSubscriber.subscribe(
        transport,
        new LedgerEventHandlers() {
          @Override
          public void record_(Sold event, Events.EventMeta meta) {
            heard.add(event.total());
          }
        },
        new Events.ConsumerOptions().onEventRejected(rejected::add));
    ShopPublisher shop = ShopPublisher.create(transport);

    // The publisher computes the field and replaces the value the application put there.
    shop.sales().sold(new Sold("a", 2, 5, 12345));
    transport.idle();
    check(heard.equals(List.of(10)), "the subscriber heard the computed total: " + heard);
    Json.Obj envelope = Json.parse(new String(wire.get(0), StandardCharsets.UTF_8)).asObject();
    check(envelope.get("data").asObject().get("total").asDouble() == 10, "the message carries it: " + envelope.write());

    // The subscriber does not believe the message: it computes the field again.
    Json.Obj data = envelope.get("data").asObject().with("total", Json.of(999L));
    byte[] forged = envelope.with("data", data).canonical().getBytes(StandardCharsets.UTF_8);
    transport.publish(TOPIC, "a", forged);
    transport.idle();
    check(heard.equals(List.of(10, 10)), "the total of the message was not believed: " + heard);

    // A consumed event whose derived field cannot be computed is rejected as a rule violation with
    // that field's violation, whether or not the digests match (spec/rules.md section 17.2).
    Json.Obj overflow = envelope.get("data").asObject().with("qty", Json.of((long) Integer.MAX_VALUE)).with("price", Json.of(2L));
    byte[] same = envelope.with("data", overflow).canonical().getBytes(StandardCharsets.UTF_8);
    transport.publish(TOPIC, "a", same);
    transport.idle();
    check(heard.size() == 2 && rejected.size() == 1, "the equal digests: rejected, not handled: " + rejected.size());
    check(rejected.get(0).reason() == Events.Reason.RULE, "with the reason rule");
    check(rejected.get(0).violations().size() == 1
        && rejected.get(0).violations().get(0).rule().equals(Sold.DERIVED_TOTAL), "and the field's violation");
    byte[] other = envelope.with("veldrules", Json.of("0000000000000000")).with("data", overflow).canonical().getBytes(StandardCharsets.UTF_8);
    transport.publish(TOPIC, "a", other);
    transport.idle();
    check(heard.size() == 2 && rejected.size() == 2, "another digest: rejected as well: " + rejected.size());
    check(rejected.get(1).reason() == Events.Reason.RULE, "with the reason rule");
    check(rejected.get(1).violations().size() == 1
        && rejected.get(1).violations().get(0).rule().equals(Sold.DERIVED_TOTAL), "and the field's violation");
    check(rejected.get(1).violations().get(0).error() != null, "with the error set");

    // A derived field that fails stops the publish, like a violated invariant: nothing is sent.
    int sent = wire.size();
    try {
      shop.sales().sold(new Sold("a", Integer.MAX_VALUE, 2, null));
      throw new IllegalStateException("FAILED: an event whose derived field overflows was published");
    } catch (VeldError e) {
      check(e.raisedLocally() && e.violations().size() == 1, "one violation: " + e.violations());
      check(e.violations().get(0).rule().equals(Sold.DERIVED_TOTAL), "the id of the field: " + e.violations());
      check("overflow".equals(e.violations().get(0).error()), "with the error");
    }
    // An invariant reads the derived value.
    try {
      shop.sales().sold(new Sold("a", 0, 5, null));
      throw new IllegalStateException("FAILED: an event with a total of 0 was published");
    } catch (VeldError e) {
      check(e.violations().size() == 1 && e.violations().get(0).rule().equals(Sold.RULE_POSITIVE), "the invariant: " + e.violations());
    }
    transport.idle();
    check(wire.size() == sent && heard.size() == 2, "nothing was sent for either: " + wire.size());
    System.out.println("derived events ok");
  }
}
