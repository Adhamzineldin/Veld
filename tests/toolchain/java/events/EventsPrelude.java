package veldtest;

import gen.e.app.compiled_events.Greeted;
import gen.e.app.server.HelloPublisher;
import gen.e.prelude.ValidationError;
import gen.e.runtime.MemoryTransport;

/**
 * A service with an endpoint that has parameters has the prelude {@code ValidationError}: the
 * publisher of the same service throws that class, with the violations, marked as raised locally.
 * Prints {@code events prelude ok}.
 */
public final class EventsPrelude {
  private EventsPrelude() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  public static void main(String[] args) {
    HelloPublisher hello = HelloPublisher.create(new MemoryTransport());
    try {
      hello.greetingEvents().greeted(new Greeted(""));
      throw new IllegalStateException("FAILED: an event that breaks its rule was published");
    } catch (ValidationError e) {
      check(e.raisedLocally(), "raised locally");
      check(e.violations().size() == 1, "one violation");
      check(e.status() == 422, "status 422");
      check(ValidationError.matches(e), "matches the declared error");
    }
    System.out.println("events prelude ok");
  }
}
