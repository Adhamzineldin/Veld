package dev.veld.runtime;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * What {@code emit} of one request does (spec/events.md section 11.1): writes the events of the
 * handler to the outbox through steps 1 to 5 of publishing, remembers which, and checks after the
 * handler that exactly the events of the edge the move took were written.
 */
public final class Emission {
  /**
   * What the server hands the endpoints of a service whose moves emit: the outbox, the id source and
   * the clock, and who is told the name of each event once it is saved (the test handler).
   *
   * @param outbox where {@code emit} writes
   * @param options the id source, the clock and the rule hook of the publisher
   * @param written told the declaration name of an event once it is saved, or {@code null}
   */
  public record Source(Outbox outbox, Events.PublisherOptions options, Consumer<String> written) {}

  private final Source source;
  private final String publisher;
  private final Function<List<Issue>, VeldError> validation;
  private final List<String> expected;
  private final List<String> written = new ArrayList<>();
  private boolean skipped;

  Emission(Source source, String publisher, Function<List<Issue>, VeldError> validation, List<String> expected) {
    this.source = source;
    this.publisher = publisher;
    this.validation = validation;
    this.expected = expected;
  }

  /**
   * Writes one event: steps 1 to 5 of publishing, then {@code outbox.save}. Nothing goes to a
   * transport. A violation throws the typed {@code ValidationError} and writes nothing.
   */
  public <E> Events.PublishedEvent emit(Events.ChannelDef channel, Events.EventDef<E> def, E event) throws Exception {
    Events.PublisherContext ctx = new Events.PublisherContext(null, publisher, source.options(), validation);
    Events.PublishedEvent published = Events.emit(source.outbox(), ctx, channel, def, event);
    record(def.name());
    return published;
  }

  private void record(String name) {
    synchronized (written) {
      written.add(name);
    }
    if (source.written() != null) {
      source.written().accept(name);
    }
  }

  /**
   * The check after the handler (spec/events.md section 11.2), for a success: every event of the
   * edge written once, and nothing else. A fault throws an {@link IllegalStateException}, which the
   * router answers as {@code 500 Internal} and reports to {@code onError}: the state changed without
   * its event, and events already saved stay in the outbox, so the application's transaction has to
   * end with the failure.
   */
  public void check(String endpoint) {
    if (skipped) {
      return;
    }
    List<String> copy;
    synchronized (written) {
      copy = List.copyOf(written);
    }
    String result = checkWritten(expected, copy);
    if (!result.equals("ok")) {
      throw new IllegalStateException(endpoint + ": the move wrote the wrong events to the outbox: " + result);
    }
  }

  /**
   * The test handler's way to answer a request with a result of its own: the application's handler
   * did not run, so nothing is checked and the events the edge taken emits are told to the
   * {@code written} listener, in order, as what a handler that succeeded must have written.
   */
  public void skip() {
    skipped = true;
    for (String name : expected) {
      if (source.written() != null) {
        source.written().accept(name);
      }
    }
  }

  /**
   * What the server checks after a handler (spec/events.md section 11.1): {@code "ok"}, or {@code
   * "missing E"}, {@code "twice E"} or {@code "unexpected E"}, the first fault: a missing event
   * first (in the order of {@code expected}), then a repeated one, then an unexpected one (in write
   * order).
   */
  public static String checkWritten(List<String> expected, List<String> written) {
    for (String e : expected) {
      long count = written.stream().filter(e::equals).count();
      if (count == 0) {
        return "missing " + e;
      }
      if (count > 1) {
        return "twice " + e;
      }
    }
    for (String w : written) {
      if (!expected.contains(w)) {
        return "unexpected " + w;
      }
    }
    return "ok";
  }
}
