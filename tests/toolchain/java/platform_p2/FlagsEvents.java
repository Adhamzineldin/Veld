package veldtest;

import gen.b.flags_events.flags_events.Booked;
import gen.b.flags_events.server.DeskPublisher;
import gen.b.flags_events.server.FloorEventHandlers;
import gen.b.flags_events.server.FloorSubscriber;
import gen.b.runtime.Events;
import gen.b.runtime.Flags;
import gen.b.runtime.MemoryTransport;
import gen.b.runtime.VeldError;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * A flagged invariant of an event (spec/platform.md section 11): the publisher and the consumer
 * evaluate it with a context that has no principal, when the provider of their options says the
 * flag is on; without a provider it is off; a failing provider counts it as on and the failure goes
 * to {@code onFlagError}. Prints {@code OK}.
 */
public final class FlagsEvents {
  private FlagsEvents() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) throws Exception {
    UUID id = UUID.fromString("00000000-0000-4000-8000-000000000001");
    Booked big = new Booked(id, 9);

    // The publisher.
    MemoryTransport transport = new MemoryTransport();
    check(DeskPublisher.create(transport).bookings().booked(big) != null, "without a provider the flagged invariant is off");
    List<Flags.Context> contexts = new CopyOnWriteArrayList<>();
    Flags.Provider on = (name, context) -> {
      contexts.add(context);
      return name.equals("smallGroups");
    };
    try {
      DeskPublisher.create(transport, new Events.PublisherOptions().flags(on)).bookings().booked(big);
      throw new AssertionError("a flag that is on makes the invariant apply");
    } catch (VeldError e) {
      check(e.violations().size() == 1, "one violation: " + e.violations());
    }
    check(contexts.size() == 1 && contexts.get(0).principal() == null && contexts.get(0).operation().startsWith("veld:flags_events/Booked@"),
        "the context has no principal and the event's signature: " + contexts);
    Flags.Provider off = (name, context) -> false;
    check(DeskPublisher.create(transport, new Events.PublisherOptions().flags(off)).bookings().booked(big) != null, "off: not evaluated");
    List<Throwable> failures = new CopyOnWriteArrayList<>();
    Flags.Provider failing = (name, context) -> {
      throw new IllegalStateException("down");
    };
    try {
      DeskPublisher.create(transport, new Events.PublisherOptions().flags(failing).onFlagError(failures::add)).bookings().booked(big);
      throw new AssertionError("a failing provider counts the invariant as on");
    } catch (VeldError e) {
      check(e.violations().size() == 1, "one violation");
    }
    check(!failures.isEmpty(), "the failure went to onFlagError");
    transport.idle();

    // The consumer.
    for (int round = 0; round < 3; round++) {
      MemoryTransport wire = new MemoryTransport();
      List<Integer> seated = new CopyOnWriteArrayList<>();
      List<Events.EventRejected> rejected = new CopyOnWriteArrayList<>();
      Events.ConsumerOptions options = new Events.ConsumerOptions().onEventRejected(rejected::add);
      if (round == 1) {
        options.flags(on);
      }
      if (round == 2) {
        options.flags(failing);
      }
      FloorSubscriber.subscribe(
          wire,
          new FloorEventHandlers() {
            @Override
            public void seat(Booked event, Events.EventMeta meta) {
              seated.add(event.guests());
            }
          },
          options);
      DeskPublisher.create(wire).bookings().booked(big);
      wire.idle();
      if (round == 0) {
        check(seated.equals(List.of(9)) && rejected.isEmpty(), "no provider: the flagged invariant is off, the event is handled");
      } else {
        check(seated.isEmpty() && rejected.size() == 1 && rejected.get(0).reason() == Events.Reason.RULE,
            "round " + round + ": the invariant applies and the event is rejected: " + rejected);
      }
    }
    System.out.println("OK");
  }
}
