package veldtest;

import gen.e.bookings.server.ListenerEventHandlers;
import gen.e.bookings.server.ListenerSubscriber;
import gen.e.bookings.server.PingerPublisher;
import gen.e.bookings.t.Pinged;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.VeldError;
import java.time.Instant;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Events in a contract with no rule at all: it publishes, consumes and refuses a value of the wrong
 * shape, and writes the datetime in UTC. Prints {@code events plain ok}.
 */
public final class EventsPlain {
  private EventsPlain() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  public static void main(String[] args) throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> heard = new CopyOnWriteArrayList<>();
    ListenerSubscriber.subscribe(
        transport,
        new ListenerEventHandlers() {
          @Override
          public void heard(Pinged event, gen.e.runtime.Events.EventMeta meta) {
            heard.add(event.when().toString());
          }
        });
    PingerPublisher pinger = PingerPublisher.create(transport);
    pinger.pings().pinged(new Pinged(Instant.parse("2026-06-10T11:59:58.250Z")));
    transport.idle();
    check(heard.equals(List.of("2026-06-10T11:59:58.250Z")), "the datetime arrives: " + heard);
    try {
      pinger.pings().pinged(new Pinged(null));
      throw new IllegalStateException("FAILED: a missing required field was accepted");
    } catch (VeldError e) {
      check(e.title().equals("ValidationError"), "the title");
      check(e.raisedLocally(), "raised locally");
    }
    transport.idle();
    check(heard.size() == 1, "nothing was sent for the invalid value");
    System.out.println("events plain ok");
  }
}
