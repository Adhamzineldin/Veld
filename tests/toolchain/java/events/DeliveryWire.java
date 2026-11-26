package veldtest;

import gen.e.bookings.t.BookingPaid;
import gen.e.runtime.Events;
import gen.e.runtime.Issue;
import gen.e.runtime.Json;
import gen.e.runtime.Transport;
import gen.e.runtime.Violation;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * The dead-letter and retry goldens of phase E2 ({@code tests/wire/events/dead-letter.json} and
 * {@code retry.json}, spec/events.md sections 10.4 and 10.5) through the generated events runtime.
 * {@code cases.json} in the working directory holds the two goldens as they are, the digest of
 * {@code BookingPaid}, the topic and the group, and the five messages the retry cases need. Prints
 * one JSON object: the copy of every dead-letter case and the settlement of every retry case (with
 * the answer again when the copy cannot be published). The Rust test compares them with the goldens.
 */
public final class DeliveryWire {
  private DeliveryWire() {}

  private static String text(Json v) {
    return v instanceof Json.Str s ? s.value() : null;
  }

  private static byte[] fromHex(String hex) {
    byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) Integer.parseInt(hex.substring(2 * i, 2 * i + 2), 16);
    }
    return out;
  }

  private static String toHex(byte[] bytes) {
    StringBuilder out = new StringBuilder();
    for (byte b : bytes) {
      out.append(String.format("%02x", b & 0xff));
    }
    return out.toString();
  }

  /** A transport that keeps what it is given, and fails the publish of a dead-letter copy when told to. */
  private static final class Recording implements Transport {
    Deliver deliver;
    final List<String> topics = new ArrayList<>();
    final List<String> keys = new ArrayList<>();
    final List<byte[]> published = new ArrayList<>();
    boolean failCopies;

    @Override
    public void publish(String topic, String key, byte[] bytes) {
      if (failCopies) {
        throw new IllegalStateException("the broker refused the copy");
      }
      topics.add(topic);
      keys.add(key);
      published.add(bytes);
    }

    @Override
    public Subscription subscribe(String topic, String group, Deliver d) {
      this.deliver = d;
      return () -> {};
    }
  }

  private static Json.Obj settle(Json.Obj input, Json.Obj c, boolean failCopies) {
    Json.Obj messages = (Json.Obj) input.get("messages");
    String which = "valid";
    if (c.has("ignored")) {
      which = "ignored";
    } else if (c.has("rejected")) {
      which = text(c.get("rejected"));
    }
    Events.EventDef<BookingPaid> def =
        new Events.EventDef<>("BookingPaid", BookingPaid.VELD_SIG, BookingPaid.SCHEMA, text(input.get("digest")), BookingPaid.codec());
    Recording transport = new Recording();
    transport.failCopies = failCopies;
    List<Json> results = c.get("results") instanceof Json.Arr a ? a.items() : List.of();
    int[] runs = {0};
    List<Json> failures = new ArrayList<>();
    List<Json> waits = new ArrayList<>();
    Events.ConsumerOptions options =
        new Events.ConsumerOptions()
            .sleep(ms -> waits.add(Json.of(ms)))
            .onHandlerError(
                e -> failures.add(Json.object().put("attempt", Json.of((long) e.attempt())).put("last", Json.of(e.last())).build()));
    if (c.get("delayMs") instanceof Json.Num n) {
      options.retryDelayMs((long) n.value());
    }
    if (c.get("maxDelayMs") instanceof Json.Num n) {
      options.retryMaxDelayMs((long) n.value());
    }
    Events.RetryPolicy retry = null;
    if (c.get("retry") instanceof Json.Obj r) {
      retry =
          new Events.RetryPolicy(
              (int) ((Json.Num) r.get("max")).value(),
              text(r.get("backoff")).equals("fixed") ? Events.Backoff.FIXED : Events.Backoff.EXPONENTIAL);
    }
    boolean deadLetter = c.get("deadLetter") instanceof Json.Bool b && b.value();
    Events.Handler<BookingPaid> handler =
        (event, meta) -> {
          runs[0]++;
          Json result = results.isEmpty() ? null : results.get(Math.min(runs[0], results.size()) - 1);
          if (result != null && "failed".equals(text(result))) {
            throw new IllegalStateException("the handler failed");
          }
        };
    Events.subscribe(
        transport,
        List.of(
            new Events.SubscriptionDef(
                text(input.get("topic")), text(input.get("group")), List.of(Events.on(def, handler)), retry, deadLetter, false)),
        options);
    int attempt = c.get("transportAttempt") instanceof Json.Num n ? (int) n.value() : 1;
    Transport.Outcome answer =
        transport.deliver.deliver(
            new Transport.Message("k1", text(messages.get(which)).getBytes(StandardCharsets.UTF_8), attempt));
    Json copy = Json.NULL;
    if (!transport.published.isEmpty()) {
      Json.Obj parsed = Json.parse(new String(transport.published.get(0), StandardCharsets.UTF_8)).asObject();
      copy =
          Json.object()
              .put("reason", Json.of(text(parsed.get("veldreason"))))
              .put("attempts", parsed.get("veldattempts"))
              .put("key", transport.keys.get(0) == null ? Json.NULL : Json.of(transport.keys.get(0)))
              .put("topic", Json.of(transport.topics.get(0)))
              .put("hex", Json.of(toHex(transport.published.get(0))))
              .build();
    }
    if (transport.published.size() > 1) {
      throw new IllegalStateException(text(c.get("name")) + ": more than one copy");
    }
    return Json.object()
        .put("name", c.get("name"))
        .put("runs", Json.of((long) runs[0]))
        .put("failures", Json.array(failures))
        .put("waitsMs", Json.array(waits))
        .put("copy", copy)
        .put("answer", Json.of(answer == Transport.Outcome.DONE ? "done" : "again"))
        .build();
  }

  private static List<String> strings(Json v) {
    List<String> out = new ArrayList<>();
    if (v instanceof Json.Arr a) {
      for (Json i : a.items()) {
        out.add(text(i));
      }
    }
    return out;
  }

  public static void main(String[] args) throws Exception {
    Json.Obj input = Json.parse(Files.readString(Path.of("cases.json"))).asObject();
    List<Json> dead = new ArrayList<>();
    for (Json j : ((Json.Arr) input.get("deadLetter")).items()) {
      Json.Obj c = (Json.Obj) j;
      byte[] original =
          c.has("originalHex") ? fromHex(text(c.get("originalHex"))) : text(c.get("original")).getBytes(StandardCharsets.UTF_8);
      List<Issue> issues = new ArrayList<>();
      for (Json i : ((Json.Arr) c.get("issues")).items()) {
        Json.Obj o = (Json.Obj) i;
        issues.add(new Issue(text(o.get("path")), text(o.get("code")), text(o.get("message"))));
      }
      List<Violation> violations = new ArrayList<>();
      for (Json v : ((Json.Arr) c.get("violations")).items()) {
        Json.Obj o = (Json.Obj) v;
        violations.add(
            new Violation(
                text(o.get("rule")), text(o.get("path")), strings(o.get("fields")), text(o.get("message")), text(o.get("error"))));
      }
      byte[] copy =
          Events.deadLetterCopy(
              original,
              Events.DeadLetterReason.valueOf(text(c.get("reason")).toUpperCase(java.util.Locale.ROOT)),
              text(c.get("group")),
              (int) ((Json.Num) c.get("attempts")).value(),
              issues,
              violations);
      dead.add(Json.object().put("name", c.get("name")).put("hex", Json.of(toHex(copy))).build());
    }
    List<Json> retry = new ArrayList<>();
    for (Json j : ((Json.Arr) input.get("retry")).items()) {
      Json.Obj c = (Json.Obj) j;
      Json.Obj settled = settle(input, c, false);
      Json.Obj failed = settle(input, c, true);
      retry.add(
          new Json.Obj(settled.fields()).with("answerIfCopyFails", failed.get("answer")).with("copyFailsRuns", failed.get("runs")));
    }
    System.out.println(Json.object().put("deadLetter", Json.array(dead)).put("retry", Json.array(retry)).build().write());
  }
}
