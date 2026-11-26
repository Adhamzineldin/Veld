package veldtest;

import gen.e.bookings.server.BillingPublisher;
import gen.e.bookings.server.Probe0EventHandlers;
import gen.e.bookings.server.Probe0Subscriber;
import gen.e.bookings.t.InvoiceNudged;
import gen.e.runtime.Events;
import gen.e.runtime.Json;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.RuleWalk;
import gen.e.runtime.Transport;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * An audited invariant of an event (@rollout(audit), tests/wire/events/edges.veld, event
 * InvoiceNudged): it is evaluated and reported to onRuleViolation and never rejects, on the way
 * out and on the way in. Prints one JSON document: what the hooks heard and what was delivered, for
 * a value that breaks the rule and one that does not.
 */
public final class EventsAudit {
  private EventsAudit() {}

  private static final UUID INVOICE = UUID.fromString("7e6f0a52-3d84-4a4b-9c27-0f8e2b1d6a90");

  private static Json heard(List<RuleWalk.Event> events) {
    List<Json> out = new ArrayList<>();
    for (RuleWalk.Event e : events) {
      out.add(
          Json.object()
              .put("rule", Json.of(e.rule()))
              .put("operation", Json.of(e.operation()))
              .put("position", Json.of(e.position().name().toLowerCase(Locale.ROOT)))
              .put("outcome", Json.of(e.outcome().name().toLowerCase(Locale.ROOT)))
              .build());
    }
    return Json.array(out);
  }

  private static Json run(int days) throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<RuleWalk.Event> published = new CopyOnWriteArrayList<>();
    List<RuleWalk.Event> consumed = new CopyOnWriteArrayList<>();
    List<Json> delivered = new CopyOnWriteArrayList<>();
    List<Json> rejected = new CopyOnWriteArrayList<>();
    Probe0Subscriber.subscribe(
        transport,
        new Probe0EventHandlers() {
          @Override
          public void onInvoiceNudged(InvoiceNudged event, Events.EventMeta meta) {
            delivered.add(Json.of((long) event.days()));
          }
        },
        new Events.ConsumerOptions()
            .onRuleViolation(consumed::add)
            .onEventRejected(e -> rejected.add(Json.of(e.reason().text()))));
    List<Json> sent = new CopyOnWriteArrayList<>();
    Transport spy =
        new Transport() {
          @Override
          public void publish(String topic, String key, byte[] bytes) {
            sent.add(Json.of(new String(bytes, StandardCharsets.UTF_8)));
            transport.publish(topic, key, bytes);
          }

          @Override
          public Subscription subscribe(String topic, String group, Deliver deliver) {
            return transport.subscribe(topic, group, deliver);
          }
        };
    BillingPublisher billing =
        BillingPublisher.create(spy, new Events.PublisherOptions().onRuleViolation(published::add));
    billing.invoiceEvents().invoiceNudged(new InvoiceNudged(INVOICE, days));
    transport.idle();
    return Json.object()
        .put("days", Json.of((long) days))
        .put("published", heard(published))
        .put("consumed", heard(consumed))
        .put("delivered", Json.array(delivered))
        .put("rejected", Json.array(rejected))
        .put("sent", Json.array(sent))
        .build();
  }

  public static void main(String[] args) throws Exception {
    System.setOut(new java.io.PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), true, StandardCharsets.UTF_8));
    System.out.println(Json.object().put("breaking", run(45)).put("fine", run(10)).build().write());
  }
}
