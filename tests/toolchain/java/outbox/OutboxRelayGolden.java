package veldtest;

import gen.b.bookings.server.BookingsOutboxRelay;
import gen.b.runtime.Emission;
import gen.b.runtime.Events;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryOutbox;
import gen.b.runtime.Outbox;
import gen.b.runtime.OutboxEntry;
import gen.b.runtime.Transport;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * The {@code relay} and {@code written} goldens of {@code tests/wire/events/outbox.json} (copied next
 * to the program as {@code outbox.json}) through the GENERATED relay {@code
 * BookingsOutboxRelay.run} and the check {@code Emission.checkWritten} (spec/events.md section 11,
 * tests/wire/events/README.md). Expects the server generated from {@code
 * tests/events/outbox.veld.pending} as the package {@code bookings}. Port of {@code
 * events_outbox_relay.ts}. Prints {@code OK}.
 */
public final class OutboxRelayGolden {
  private OutboxRelayGolden() {}

  private static final String TOPIC = "shop.bookings.v1";

  /** What the process that "ends" between the publish and {@code done} throws: the relay reports it, the run does not. */
  private static final Exception CRASH = new Exception("the process ended");

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static byte[] bytes(String text) {
    return text.getBytes(StandardCharsets.UTF_8);
  }

  private static List<String> strings(Json array) {
    List<String> out = new ArrayList<>();
    for (Json item : array.asArray().items()) {
      out.add(item.asString());
    }
    return out;
  }

  /** The step of a script: {@code "accepted"}, {@code "crashAfterPublish"}, or an object with one message. */
  private record Step(String kind, String text) {
    static Step of(Json step) {
      if (step instanceof Json.Str s) {
        return new Step(s.value(), null);
      }
      Json.Obj o = step.asObject();
      String kind = o.get("publishFails") != null ? "publishFails" : "doneFails";
      return new Step(kind, o.get(kind).asString());
    }
  }

  private static void runCase(Json.Obj c) throws Exception {
    MemoryOutbox memory = new MemoryOutbox();
    // Entry `n` is saved n-th: the memory outbox names it "n", and its bytes say so.
    long saves = (long) c.get("saves").asDouble();
    for (long i = 1; i <= saves; i++) {
      memory.save(TOPIC, null, bytes(Long.toString(i)));
    }
    int n = 0;
    for (Json r : c.get("runs").asArray().items()) {
      Json.Obj run = r.asObject();
      String what = c.get("name").asString() + " / run " + (++n);
      List<Step> script = new ArrayList<>();
      for (Json s : run.get("script").asArray().items()) {
        script.add(Step.of(s));
      }
      List<String> published = new ArrayList<>();
      List<String> done = new ArrayList<>();
      int[] seen = {-1};
      java.util.function.Supplier<Step> step =
          () -> seen[0] < script.size() ? script.get(seen[0]) : new Step("accepted", null);
      Transport transport = new Transport() {
        @Override
        public void publish(String topic, String key, byte[] bytes) {
          seen[0] += 1;
          Step s = step.get();
          if (s.kind().equals("publishFails")) {
            throw new IllegalStateException(s.text());
          }
          published.add(new String(bytes, StandardCharsets.UTF_8));
        }

        @Override
        public Subscription subscribe(String topic, String group, Deliver deliver) {
          return () -> { };
        }
      };
      Outbox outbox = new Outbox() {
        @Override
        public void save(String topic, String key, byte[] bytes) {
          memory.save(topic, key, bytes);
        }

        @Override
        public List<OutboxEntry> pending(int limit) {
          return memory.pending(limit);
        }

        @Override
        public void done(OutboxEntry entry) throws Exception {
          Step s = step.get();
          if (s.kind().equals("crashAfterPublish")) {
            throw CRASH;
          }
          if (s.kind().equals("doneFails")) {
            throw new IllegalStateException(s.text());
          }
          done.add(entry.id());
          memory.done(entry);
        }
      };
      String[] failure = {null};
      int sent = BookingsOutboxRelay.run(
          outbox,
          transport,
          new Events.RelayOptions()
              .limit((int) run.get("limit").asDouble())
              .onError((error, entry) -> {
                // A crash reports nothing: the process is gone.
                if (error != CRASH) {
                  failure[0] = entry.id() + "/" + error.getMessage();
                }
              }));
      Json.Obj expect = run.get("expect").asObject();
      check(published.equals(strings(expect.get("published"))), what + ": published " + published);
      check(done.equals(strings(expect.get("done"))), what + ": done " + done);
      check(sent == (int) expect.get("sent").asDouble(), what + ": sent " + sent);
      String wanted = null;
      if (expect.get("failure") instanceof Json.Obj f) {
        wanted = f.get("entry").asString() + "/" + f.get("error").asString();
      }
      check(java.util.Objects.equals(failure[0], wanted), what + ": failure " + failure[0] + " wanted " + wanted);
      List<String> pending = new ArrayList<>();
      for (OutboxEntry e : memory.pending(1000)) {
        pending.add(e.id());
      }
      check(pending.equals(strings(run.get("pendingAfter"))), what + ": pending after " + pending);
    }
  }

  public static void main(String[] args) throws Exception {
    Json.Obj golden = Json.parse(Files.readString(Path.of("outbox.json"), StandardCharsets.UTF_8)).asObject();
    check(!golden.get("relay").asArray().items().isEmpty() && !golden.get("written").asArray().items().isEmpty(), "cases");
    for (Json c : golden.get("relay").asArray().items()) {
      runCase(c.asObject());
    }
    // `written`: what the server checks after the handler of a move that emits.
    for (Json c : golden.get("written").asArray().items()) {
      Json.Obj w = c.asObject();
      String got = Emission.checkWritten(strings(w.get("expected")), strings(w.get("written")));
      check(got.equals(w.get("result").asString()), w.get("name").asString() + ": " + got);
    }
    // The relay hands the transport the options of the channel of the topic (an unknown topic: none set).
    List<Transport.ChannelOptions> seen = new ArrayList<>();
    MemoryOutbox outbox = new MemoryOutbox();
    outbox.save(TOPIC, "k", bytes("x"));
    outbox.save("somewhere.else", null, bytes("y"));
    BookingsOutboxRelay.run(outbox, new Transport() {
      @Override
      public void publish(String topic, String key, byte[] bytes) {
        throw new AssertionError("the relay publishes with the options of the channel");
      }

      @Override
      public void publish(String topic, String key, byte[] bytes, ChannelOptions options) {
        seen.add(options);
      }

      @Override
      public Subscription subscribe(String topic, String group, Deliver deliver) {
        return () -> { };
      }
    });
    check(seen.equals(List.of(new Transport.ChannelOptions(false, false), Transport.ChannelOptions.NONE)), "options: " + seen);
    System.out.println("OK");
  }
}
