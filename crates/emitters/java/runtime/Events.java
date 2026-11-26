package dev.veld.runtime;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Supplier;

/**
 * The envelope of an event message (CloudEvents 1.0, structured JSON mode) and the publish and
 * consume pipelines of spec/events.md sections 6 to 9. The generated publishers and subscribers
 * call {@link #publish} and {@link #subscribe}; nothing here depends on a broker.
 */
public final class Events {
  private Events() {}

  // -- what the generated code describes ------------------------------------------------------

  /** How the key of a channel is written: its type (spec/events.md section 4). */
  public enum KeyKind {
    /** A {@code string}, used as it is. */
    STRING,
    /** A {@code uuid}, in lowercase. */
    UUID,
    /** An {@code int}, in decimal digits. */
    INT,
    /** A {@code long}, in decimal digits. */
    LONG
  }

  /**
   * One channel as the generated code describes it to the publisher.
   *
   * @param topic the topic the message is published to
   * @param keyWire the wire name of the key field, or {@code null} when the channel has no key
   * @param keyKind the type of the key field, or {@code null} when the channel has no key
   * @param ordered {@code @ordered}: one total order across keys (spec/events.md section 10.2)
   * @param deadLetter {@code @deadLetter}: the channel has the topic {@code <topic>.dlq}
   */
  public record ChannelDef(String topic, String keyWire, KeyKind keyKind, boolean ordered, boolean deadLetter) {
    /** A channel with neither {@code @ordered} nor {@code @deadLetter}. */
    public ChannelDef(String topic, String keyWire, KeyKind keyKind) {
      this(topic, keyWire, keyKind, false, false);
    }

    Transport.ChannelOptions options() {
      return new Transport.ChannelOptions(ordered, deadLetter);
    }
  }

  /**
   * One event as the generated code describes it to the pipelines.
   *
   * @param name the name of the event
   * @param sig the declaration signature, the {@code type} of every message of this event
   * @param schema the wire description
   * @param digest the rules digest ({@code veldrules}), or {@code null} when the event reaches no
   *     enforced invariant
   * @param codec the codec of the generated record
   * @param <E> the generated record
   */
  public record EventDef<E>(String name, String sig, Schema schema, String digest, Codec<E> codec) {}

  /** The call of a handler of the application. */
  @FunctionalInterface
  public interface Handler<E> {
    /** Handles one event; throwing leaves the message unacknowledged. */
    void handle(E event, EventMeta meta) throws Exception;
  }

  /**
   * One {@code on} line: the event and the application's handler.
   *
   * @param def the event
   * @param run the handler
   * @param <E> the generated record
   */
  public record Binding<E>(EventDef<E> def, Handler<E> run) {}

  /** A binding of {@code handler} to the event {@code def}. */
  public static <E> Binding<E> on(EventDef<E> def, Handler<E> handler) {
    return new Binding<>(def, handler);
  }

  /** How long a consumer waits between two runs of {@code @retry} (spec/events.md section 10.2). */
  public enum Backoff {
    /** Every wait is the delay. */
    FIXED,
    /** The wait doubles with each run. */
    EXPONENTIAL
  }

  /**
   * {@code @retry(max: n, backoff: ..)} of a channel: the consumer runs the handler again by itself
   * (spec/events.md section 10.4).
   *
   * @param max how many more runs than the first, 0 to 100
   * @param backoff how the wait between two runs grows
   */
  public record RetryPolicy(int max, Backoff backoff) {}

  /**
   * One subscription of a service.
   *
   * @param topic the topic
   * @param group the consumer group
   * @param on the events the subscription handles
   * @param retry {@code @retry} of the channel, or {@code null}
   * @param deadLetter {@code @deadLetter}: rejected messages and used-up retries are copied to {@code <topic>.dlq}
   * @param ordered {@code @ordered}: passed to the transport
   */
  public record SubscriptionDef(
      String topic, String group, List<Binding<?>> on, RetryPolicy retry, boolean deadLetter, boolean ordered) {
    /** A subscription to a channel with none of the three annotations. */
    public SubscriptionDef(String topic, String group, List<Binding<?>> on) {
      this(topic, group, on, null, false, false);
    }
  }

  /**
   * The facts about a message every handler gets next to the typed event.
   *
   * @param id the id of the message
   * @param source the publishing service
   * @param type the {@code type} attribute as the publisher wrote it: the signature it knew
   * @param subject the key text, or {@code null} when the message has none
   * @param time the time the publisher wrote, as text, or {@code null}
   * @param attempt the delivery attempt, 1 for the first
   */
  public record EventMeta(String id, String source, String type, String subject, String time, int attempt) {}

  /**
   * What a publisher call returns: the message it sent, without the data.
   *
   * @param id the id of the message
   * @param source the publishing service
   * @param type the signature of the event
   * @param subject the key text, or {@code null}
   * @param time the time written into the message
   */
  public record PublishedEvent(String id, String source, String type, String subject, String time) {}

  // -- hooks and options ----------------------------------------------------------------------

  /** Why a message is rejected (spec/events.md section 8). */
  public enum Reason {
    /** Step 1: the bytes are not a valid envelope. */
    MALFORMED,
    /** Step 3: the data does not fit the consumer's own type. */
    SHAPE,
    /** Step 4: an invariant is violated. */
    RULE;

    /** The reason as text: {@code malformed}, {@code shape} or {@code rule}. */
    public String text() {
      return name().toLowerCase(Locale.ROOT);
    }
  }

  /**
   * A message that can never succeed for this consumer: it is acknowledged and reported.
   *
   * @param reason why it is rejected
   * @param message what was wrong, as text
   * @param topic the topic
   * @param group the consumer group
   * @param event the name of the event, or {@code null} when the type names none this subscription handles
   * @param issues the problems of the shape
   * @param violations the violated rules
   * @param bytes the message as the transport delivered it
   */
  public record EventRejected(
      Reason reason,
      String message,
      String topic,
      String group,
      String event,
      List<Issue> issues,
      List<Violation> violations,
      byte[] bytes) {}

  /**
   * The consumer and the publisher were generated from different versions of the event.
   *
   * @param event the name of the event
   * @param ownSignature the signature this consumer was generated with
   * @param ownDigest the rules digest this consumer was generated with, or {@code null}
   * @param signature the signature the message carries
   * @param digest the rules digest the message carries, or {@code null}
   */
  public record ContractMismatch(
      String event, String ownSignature, String ownDigest, String signature, String digest) {}

  /**
   * A run of a handler failed (spec/events.md section 8.1).
   *
   * @param event the name of the event
   * @param topic the topic
   * @param group the consumer group
   * @param meta the metadata of the message
   * @param error what the handler threw
   * @param attempt the delivery attempt of this run: {@code meta.attempt()}
   * @param last true when this consumer will not handle the message again (spec/events.md section
   *     10.4): the last run of a channel with {@code @retry}. Always false on a channel without
   *     {@code @retry}, where the transport delivers the message again.
   */
  public record HandlerFailure(
      String event, String topic, String group, EventMeta meta, Throwable error, int attempt, boolean last) {}

  /** What an {@link Inbox} runs. */
  @FunctionalInterface
  public interface Runner {
    /** Runs the handler. */
    void run() throws Exception;
  }

  /**
   * The application's record of the messages it has handled (spec/events.md section 10.3).
   * {@code handle} runs {@code run} and records {@code (source, id)}, or does nothing when the pair
   * is recorded; it answers whether it ran {@code run}. When {@code run} fails the pair must not be
   * recorded and the failure is the handler's. An implementation that records the pair in the
   * transaction of the handler's own writes prevents a duplicated business effect; one that cannot
   * share a transaction narrows the window and does not close it.
   */
  @FunctionalInterface
  public interface Inbox {
    /** Runs {@code run} unless {@code (source, id)} was handled; answers whether it ran. */
    boolean handle(String source, String id, Runner run) throws Exception;
  }

  /** An inbox that lives in the process, for tests. It does not serialise two calls with one pair. */
  public static final class MemoryInbox implements Inbox {
    private final Set<List<String>> seen = ConcurrentHashMap.newKeySet();

    /** Creates an empty inbox. */
    public MemoryInbox() {}

    @Override
    public boolean handle(String source, String id, Runner run) throws Exception {
      List<String> key = List.of(source, id);
      if (seen.contains(key)) {
        return false;
      }
      run.run();
      seen.add(key);
      return true;
    }

    /** Whether the pair was handled. */
    public boolean has(String source, String id) {
      return seen.contains(List.of(source, id));
    }

    /** How many pairs are recorded. */
    public int size() {
      return seen.size();
    }
  }

  /** Waits for the milliseconds before a run of {@code @retry}. A test passes one that does not wait. */
  @FunctionalInterface
  public interface Sleeper {
    /** Waits. */
    void sleep(long millis) throws InterruptedException;
  }

  /** The options of a generated publisher: the id source, the clock and the rule hook. */
  public static final class PublisherOptions {
    private Supplier<String> newId = () -> UUID.randomUUID().toString();
    private Supplier<Instant> clock = Instant::now;
    private RuleWalk.Hook onRuleViolation;
    // #if flags
    private Flags.Provider flags;
    private Consumer<Throwable> onFlagError;
    // #endif

    /** Creates the default options: random UUIDs, the system clock, no hook. */
    public PublisherOptions() {}

    /** The id source of the envelope: a non-empty string, unique among the events of the service. */
    public PublisherOptions newId(Supplier<String> source) {
      this.newId = Objects.requireNonNull(source, "newId");
      return this;
    }

    /** The clock: the {@code time} of the envelope, at millisecond precision. */
    public PublisherOptions clock(Supplier<Instant> source) {
      this.clock = Objects.requireNonNull(source, "clock");
      return this;
    }

    /** Hears the violations of invariants: the enforced ones of an event that is not sent, and the audited ones. */
    public PublisherOptions onRuleViolation(RuleWalk.Hook hook) {
      this.onRuleViolation = hook;
      return this;
    }
    // #if flags

    /**
     * The provider of the flags of the invariants of the events (spec/platform.md section 11): read
     * once per event, with a context that has no principal and the event's signature as the operation.
     * Without one the flagged invariants are off; a provider that fails makes them count as on.
     */
    public PublisherOptions flags(Flags.Provider provider) {
      this.flags = provider;
      return this;
    }

    /** Hears the failures of the flag provider. */
    public PublisherOptions onFlagError(Consumer<Throwable> hook) {
      this.onFlagError = hook;
      return this;
    }
    // #endif
  }

  /** The hooks of a generated subscriber. All are optional and a failing hook is ignored. */
  public static final class ConsumerOptions {
    private Consumer<EventRejected> onEventRejected;
    private Consumer<ContractMismatch> onContractMismatch;
    private RuleWalk.Hook onRuleViolation;
    private Consumer<HandlerFailure> onHandlerError;
    // #if flags
    private Flags.Provider flags;
    private Consumer<Throwable> onFlagError;
    // #endif
    private Inbox inbox;
    private Sleeper sleep = Thread::sleep;
    private long retryDelayMs = 1000;
    private long retryMaxDelayMs = 60000;

    /** Creates options without any hook. */
    public ConsumerOptions() {}

    /** A rejected message (it is acknowledged). Without this hook, and without a dead letter, it is gone. */
    public ConsumerOptions onEventRejected(Consumer<EventRejected> hook) {
      this.onEventRejected = hook;
      return this;
    }

    /** Once per distinct pair of signature and digest this consumer had not seen. */
    public ConsumerOptions onContractMismatch(Consumer<ContractMismatch> hook) {
      this.onContractMismatch = hook;
      return this;
    }

    /** The violations of invariants of a message: the enforced ones that reject it and the audited ones. */
    public ConsumerOptions onRuleViolation(RuleWalk.Hook hook) {
      this.onRuleViolation = hook;
      return this;
    }
    // #if flags

    /**
     * The provider of the flags of the invariants of the events (spec/platform.md section 11): read once
     * per message, with a context that has no principal and the event's signature as the operation.
     * Without one the flagged invariants are off; a provider that fails makes them count as on.
     */
    public ConsumerOptions flags(Flags.Provider provider) {
      this.flags = provider;
      return this;
    }

    /** Hears the failures of the flag provider. */
    public ConsumerOptions onFlagError(Consumer<Throwable> hook) {
      this.onFlagError = hook;
      return this;
    }
    // #endif

    /** A run of a handler that failed. */
    public ConsumerOptions onHandlerError(Consumer<HandlerFailure> hook) {
      this.onHandlerError = hook;
      return this;
    }

    /** Step 5 of section 8: a message whose {@code source} and {@code id} were handled is acknowledged and skipped. */
    public ConsumerOptions inbox(Inbox value) {
      this.inbox = value;
      return this;
    }

    /** Waits for the milliseconds before a run of {@code @retry} (default {@code Thread.sleep}). */
    public ConsumerOptions sleep(Sleeper value) {
      this.sleep = Objects.requireNonNull(value, "sleep");
      return this;
    }

    /** The wait before the second run of {@code @retry} (default 1000). */
    public ConsumerOptions retryDelayMs(long value) {
      this.retryDelayMs = Math.max(0, value);
      return this;
    }

    /** No wait of {@code @retry} is longer than this (default 60000). */
    public ConsumerOptions retryMaxDelayMs(long value) {
      this.retryMaxDelayMs = Math.max(0, value);
      return this;
    }
  }

  private static <T> void hook(Consumer<T> fn, T event) {
    if (fn == null) {
      return;
    }
    try {
      fn.accept(event);
    } catch (RuntimeException ignored) {
      // A failing hook must not change what happens to the message.
    }
  }

  // -- invariants -----------------------------------------------------------------------------

  private static void report(
      RuleWalk.Hook fn,
      EventDef<?> def,
      RuleWalk.Position position,
      RuleWalk.Outcome outcome,
      List<Violation> violations,
      long started) {
    if (fn == null) {
      return;
    }
    long micros = Math.max(0L, (System.nanoTime() - started) / 1_000L);
    for (Violation v : violations) {
      try {
        fn.onRuleViolation(new RuleWalk.Event(v.rule(), def.sig(), position, outcome, v, micros));
      } catch (RuntimeException ignored) {
        // A failing hook must not change what happens to the message.
      }
    }
  }

  // #if flags

  private static final Map<String, List<String>> EVENT_FLAGS = new ConcurrentHashMap<>();

  /**
   * Registers the flags the invariants of an event carry, by the event's signature (the generated
   * publisher and subscriber call it once).
   */
  public static void flags(String sig, List<String> names) {
    EVENT_FLAGS.put(sig, List.copyOf(names));
  }

  /** Reads the flags of the event once and runs {@code body} with the readings in force. */
  private static <T> T withFlags(
      EventDef<?> def, Flags.Provider provider, Consumer<Throwable> failed, Supplier<T> body) {
    List<String> names = EVENT_FLAGS.getOrDefault(def.sig(), List.of());
    if (names.isEmpty()) {
      return body.get();
    }
    Flags.View view;
    if (provider == null) {
      view = new Flags.View(null);
      for (String name : names) {
        view.put(name, Flags.State.OFF);
      }
    } else {
      view =
          Flags.read(
              names,
              (name, request, context) -> provider.isOn(name, context),
              null,
              new Flags.Context(null, null, def.sig()),
              error -> {
                try {
                  if (failed != null) {
                    failed.accept(error);
                  }
                } catch (RuntimeException ignored) {
                  // A failing hook is ignored.
                }
              });
    }
    return Flags.with(view, body);
  }
  // #endif
  /**
   * The invariants of an event in wire form, in full mode: the enforced violations are returned,
   * and both kinds are reported to the hook (the enforced ones first). An audited rule never
   * changes what happens.
   */
  private static List<Violation> runRules(
      EventDef<?> def, Json prepared, RuleWalk.Hook fn, RuleWalk.Position position) {
    long started = System.nanoTime();
    // #if derived
    List<Violation> derived = Derive.take();
    // #endif
    // #if rollout
    Audit.begin();
    // #endif
    List<Violation> found = RuleWalk.check(def.schema(), prepared, "$", false);
    // #if derived
    found.addAll(0, derived);
    // #endif
    // #if rollout
    List<Violation> audited = Audit.end();
    // #endif
    report(fn, def, position, RuleWalk.Outcome.REJECTED, found, started);
    // #if rollout
    report(fn, def, position, RuleWalk.Outcome.AUDITED, audited, started);
    // #endif
    return found;
  }

  private static List<Issue> violationIssues(List<Violation> violations) {
    List<Issue> out = new ArrayList<>();
    for (Violation v : violations) {
      out.add(new Issue(v.path(), v.error() == null ? "rule" : "rule_error", v.message()));
    }
    return out;
  }

  // -- publishing (section 7) -----------------------------------------------------------------

  /**
   * What a generated publisher hands the pipeline.
   *
   * @param transport where the message goes
   * @param source {@code veld:<package>/<Service>}
   * @param options the id source, the clock and the rule hook
   * @param validation builds the typed {@code ValidationError} the call throws
   */
  public record PublisherContext(
      Transport transport,
      String source,
      PublisherOptions options,
      Function<List<Issue>, VeldError> validation) {}

  private static VeldError invalid(PublisherContext ctx, List<Issue> issues, List<Violation> violations) {
    VeldError err = ctx.validation().apply(issues);
    err.setViolations(violations);
    err.markLocal();
    return err;
  }

  /** The key text (section 4): a string as it is, a uuid in lowercase, an int or a long in decimal digits. */
  private static String keyText(ChannelDef channel, Json wire) {
    if (channel.keyWire() == null) {
      return null;
    }
    Json v = wire instanceof Json.Obj obj ? obj.get(channel.keyWire()) : null;
    if (v instanceof Json.Str s) {
      return s.value();
    }
    if (v instanceof Json.Num n) {
      return Long.toString((long) n.value());
    }
    throw new IllegalStateException("the key `" + channel.keyWire() + "` is not a string or a number");
  }

  /**
   * Publishes one event (section 7): defaults, shape, invariants, the key, the envelope with its
   * canonical bytes, then the transport. Nothing is sent when a step fails: the call throws the
   * typed {@code ValidationError}.
   */
  public static <E> PublishedEvent publish(PublisherContext ctx, ChannelDef channel, EventDef<E> def, E event) {
    Objects.requireNonNull(event, "event");
    Json prepared = Wire.prepare(def.schema(), def.codec().encode(event), Wire.Mode.RESPONSE);
    // #if derived
    Derive.take();
    prepared = Derive.output(def.schema(), prepared);
    // #endif
    Json wire = Canon.scalars(def.schema(), prepared, false);
    List<Issue> issues = Wire.validate(def.schema(), wire, "$", false);
    if (!issues.isEmpty()) {
      throw invalid(ctx, issues, List.of());
    }
    // #if flags
    List<Violation> violations =
        withFlags(
            def,
            ctx.options().flags,
            ctx.options().onFlagError,
            () -> runRules(def, wire, ctx.options().onRuleViolation, RuleWalk.Position.OUTPUT));
    // #endif
    // #if no_flags
    List<Violation> violations = runRules(def, wire, ctx.options().onRuleViolation, RuleWalk.Position.OUTPUT);
    // #endif
    if (!violations.isEmpty()) {
      throw invalid(ctx, violationIssues(violations), violations);
    }
    String subject = keyText(channel, wire);
    if (subject != null && subject.isEmpty()) {
      Issue key = new Issue("$." + channel.keyWire(), "key", "the key of the channel must not be empty");
      throw invalid(ctx, List.of(key), List.of());
    }
    String id = ctx.options().newId.get();
    if (id == null || id.isEmpty()) {
      throw new IllegalStateException("the id source returned an empty id");
    }
    Instant at = ctx.options().clock.get();
    if (at == null) {
      throw new IllegalStateException("the clock returned no time");
    }
    String time = Canon.isoMillis(at);
    Json.Builder envelope =
        Json.object()
            .put("specversion", Json.of("1.0"))
            .put("id", Json.of(id))
            .put("source", Json.of(ctx.source()))
            .put("type", Json.of(def.sig()))
            .put("time", Json.of(time))
            .put("datacontenttype", Json.of("application/json"))
            .put("data", wire);
    if (subject != null) {
      envelope.put("subject", Json.of(subject));
    }
    if (def.digest() != null) {
      envelope.put("veldrules", Json.of(def.digest()));
    }
    ctx.transport()
        .publish(
            channel.topic(),
            subject,
            envelope.build().canonical().getBytes(StandardCharsets.UTF_8),
            channel.options());
    return new PublishedEvent(id, ctx.source(), def.sig(), subject, time);
  }

  // -- consuming (section 8) ------------------------------------------------------------------

  private record Envelope(
      String id, String source, String type, String subject, String time, String veldrules, Json.Obj data) {}

  /** Step 1: the envelope, or the reason it is malformed. */
  private record Parsed(Envelope envelope, String problem) {}

  private static Parsed malformed(String problem) {
    return new Parsed(null, problem);
  }

  private static String text(Json.Obj o, String name) {
    return o.get(name) instanceof Json.Str s ? s.value() : null;
  }

  private static Parsed parseEnvelope(byte[] bytes) {
    Json json;
    try {
      String text =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(bytes))
              .toString();
      json = Json.parse(text);
    } catch (CharacterCodingException | RuntimeException e) {
      return malformed("the message is not UTF-8 JSON");
    }
    if (!(json instanceof Json.Obj o)) {
      return malformed("the message is not a JSON object");
    }
    if (!(o.get("specversion") instanceof Json.Str v) || !v.value().equals("1.0")) {
      return malformed("`specversion` is not \"1.0\"");
    }
    for (String name : List.of("id", "source", "type")) {
      String value = text(o, name);
      if (value == null || value.isEmpty()) {
        return malformed("`" + name + "` is missing, empty or not a string");
      }
    }
    for (String name : List.of("subject", "time", "datacontenttype", "veldrules")) {
      if (o.get(name) != null && text(o, name) == null) {
        return malformed("`" + name + "` is not a string");
      }
    }
    String contentType = text(o, "datacontenttype");
    if (contentType != null && !contentType.equals("application/json")) {
      return malformed("`datacontenttype` is `" + contentType + "`");
    }
    if (o.has("data_base64")) {
      return malformed("`data_base64` is not allowed");
    }
    if (!(o.get("data") instanceof Json.Obj data)) {
      return malformed("`data` is not an object");
    }
    return new Parsed(
        new Envelope(
            text(o, "id"),
            text(o, "source"),
            text(o, "type"),
            text(o, "subject"),
            text(o, "time"),
            text(o, "veldrules"),
            data),
        null);
  }

  /** `veld:pkg/Name@hash` to `veld:pkg/Name`. */
  private static String typeName(String sig) {
    int at = sig.lastIndexOf('@');
    return at < 0 ? sig : sig.substring(0, at);
  }

  // -- the dead-letter copy (section 10.5) ----------------------------------------------------

  /** Why a message is copied to {@code <topic>.dlq}: the three rejections, or the used-up retries of a handler. */
  public enum DeadLetterReason {
    /** Step 1. */
    MALFORMED,
    /** Step 3. */
    SHAPE,
    /** Step 4. */
    RULE,
    /** The retries of a handler are used up. */
    HANDLER;

    /** The reason as text, the value of {@code veldreason}. */
    public String text() {
      return name().toLowerCase(Locale.ROOT);
    }
  }

  /** The members a copy adds. A member of the original with one of these names is replaced or removed. */
  private static final List<String> COPY_MEMBERS =
      List.of("veldreason", "veldgroup", "veldattempts", "veldissues", "veldviolations");

  /**
   * The bytes of the dead-letter copy of {@code original} (section 10.5). When the original is a
   * JSON object, the copy is that object with {@code veldreason}, {@code veldgroup}, {@code
   * veldattempts} (the runs of the handler, 0 for a rejected message) and, when not empty, {@code
   * veldissues} and {@code veldviolations}, each a string that holds the canonical JSON of its list,
   * written as canonical JSON. Anything else is returned as it is. Nothing of a handler's error is
   * in a copy.
   */
  public static byte[] deadLetterCopy(
      byte[] original,
      DeadLetterReason reason,
      String group,
      int attempts,
      List<Issue> issues,
      List<Violation> violations) {
    Json json;
    try {
      json = Json.parse(decode(original));
    } catch (CharacterCodingException | RuntimeException e) {
      return original;
    }
    if (!(json instanceof Json.Obj obj)) {
      return original;
    }
    Map<String, Json> copy = new LinkedHashMap<>(obj.fields());
    COPY_MEMBERS.forEach(copy::remove);
    copy.put("veldreason", Json.of(reason.text()));
    copy.put("veldgroup", Json.of(group));
    copy.put("veldattempts", Json.of((long) attempts));
    if (!issues.isEmpty()) {
      List<Json> listed = new ArrayList<>();
      for (Issue i : issues) {
        listed.add(
            Json.object()
                .put("path", Json.of(i.path()))
                .put("code", Json.of(i.code()))
                .put("message", Json.of(i.message()))
                .build());
      }
      copy.put("veldissues", Json.of(Json.array(listed).canonical()));
    }
    if (!violations.isEmpty()) {
      List<Json> listed = new ArrayList<>();
      for (Violation v : violations) {
        listed.add(v.toWire());
      }
      copy.put("veldviolations", Json.of(Json.array(listed).canonical()));
    }
    return new Json.Obj(copy).canonical().getBytes(StandardCharsets.UTF_8);
  }

  private static String decode(byte[] bytes) throws CharacterCodingException {
    return StandardCharsets.UTF_8
        .newDecoder()
        .onMalformedInput(CodingErrorAction.REPORT)
        .onUnmappableCharacter(CodingErrorAction.REPORT)
        .decode(ByteBuffer.wrap(bytes))
        .toString();
  }

  /** One subscription being consumed. */
  private static final class Pipeline {
    private final Transport transport;
    private final SubscriptionDef sub;
    private final ConsumerOptions options;
    private final Set<String> seen;

    Pipeline(Transport transport, SubscriptionDef sub, ConsumerOptions options, Set<String> seen) {
      this.transport = transport;
      this.sub = sub;
      this.options = options;
      this.seen = seen;
    }

    /**
     * Publishes the copy of a message to {@code <topic>.dlq} when the channel has {@code @deadLetter}:
     * {@code AGAIN} when that fails (section 10.5).
     */
    Transport.Outcome copyToDeadLetter(
        Transport.Message message,
        DeadLetterReason reason,
        int attempts,
        List<Issue> issues,
        List<Violation> violations) {
      if (!sub.deadLetter()) {
        return Transport.Outcome.DONE;
      }
      byte[] bytes = deadLetterCopy(message.bytes(), reason, sub.group(), attempts, issues, violations);
      try {
        transport.publish(sub.topic() + ".dlq", message.key(), bytes);
      } catch (RuntimeException e) {
        return Transport.Outcome.AGAIN;
      }
      return Transport.Outcome.DONE;
    }

    /** A rejected message (steps 1, 3 and 4) is copied when the channel has {@code @deadLetter}, then reported. */
    Transport.Outcome reject(
        Transport.Message message, Reason reason, String text, String event, List<Issue> issues, List<Violation> violations) {
      Transport.Outcome outcome =
          copyToDeadLetter(message, DeadLetterReason.valueOf(reason.name()), 0, issues, violations);
      hook(
          options.onEventRejected,
          new EventRejected(
              reason, text, sub.topic(), sub.group(), event, List.copyOf(issues), List.copyOf(violations), message.bytes()));
      return outcome;
    }

    /** Tells the application once per distinct pair that the publisher is on another version. */
    void noteMismatch(EventDef<?> def, Envelope env) {
      if (env.type().equals(def.sig()) && Objects.equals(env.veldrules(), def.digest())) {
        return;
      }
      String pair = def.name() + "\u0000" + env.type() + "\u0000" + (env.veldrules() == null ? "" : env.veldrules());
      if (seen.add(pair)) {
        hook(
            options.onContractMismatch,
            new ContractMismatch(def.name(), def.sig(), def.digest(), env.type(), env.veldrules()));
      }
    }

    Transport.Outcome consume(Transport.Message message) {
      Parsed parsed = parseEnvelope(message.bytes());
      if (parsed.envelope() == null) {
        return reject(message, Reason.MALFORMED, parsed.problem(), null, List.of(), List.of());
      }
      Envelope env = parsed.envelope();
      for (Binding<?> binding : sub.on()) {
        if (typeName(binding.def().sig()).equals(typeName(env.type()))) {
          return run(binding, message, env);
        }
      }
      return Transport.Outcome.DONE;
    }

    <E> Transport.Outcome run(Binding<E> binding, Transport.Message message, Envelope env) {
      EventDef<E> def = binding.def();
      List<Issue> issues = Wire.validate(def.schema(), env.data(), "$", false);
      if (!issues.isEmpty()) {
        StringBuilder text = new StringBuilder();
        for (Issue i : issues) {
          text.append(text.length() == 0 ? "" : "; ").append(i.path()).append(' ').append(i.message());
        }
        return reject(message, Reason.SHAPE, text.toString(), def.name(), issues, List.of());
      }
      Json prepared = Wire.prepare(def.schema(), env.data(), Wire.Mode.RESPONSE);
      // #if derived
      Derive.take();
      prepared = Derive.output(def.schema(), prepared);
      if (!Objects.equals(env.veldrules(), def.digest())) {
        // The consumer cannot build the value its handler is promised, whatever the digests say
        // (spec/rules.md section 17.2): a derived field that failed rejects the message.
        List<Violation> uncomputed = Derive.take();
        if (!uncomputed.isEmpty()) {
          report(options.onRuleViolation, def, RuleWalk.Position.INPUT, RuleWalk.Outcome.REJECTED, uncomputed, System.nanoTime());
          StringBuilder text = new StringBuilder();
          for (Violation v : uncomputed) {
            text.append(text.length() == 0 ? "" : "; ").append(v.message());
          }
          return reject(message, Reason.RULE, text.toString(), def.name(), violationIssues(uncomputed), uncomputed);
        }
      }
      // #endif
      if (Objects.equals(env.veldrules(), def.digest())) {
        // #if flags
        Json checked = prepared;
        List<Violation> violations =
            withFlags(
                def,
                options.flags,
                options.onFlagError,
                () -> runRules(def, checked, options.onRuleViolation, RuleWalk.Position.INPUT));
        // #endif
        // #if no_flags
        List<Violation> violations = runRules(def, prepared, options.onRuleViolation, RuleWalk.Position.INPUT);
        // #endif
        if (!violations.isEmpty()) {
          StringBuilder text = new StringBuilder();
          for (Violation v : violations) {
            text.append(text.length() == 0 ? "" : "; ").append(v.message());
          }
          return reject(message, Reason.RULE, text.toString(), def.name(), violationIssues(violations), violations);
        }
      }
      E typed;
      try {
        typed = def.codec().decode(prepared);
      } catch (RuntimeException e) {
        return reject(message, Reason.SHAPE, String.valueOf(e.getMessage()), def.name(), List.of(), List.of());
      }
      noteMismatch(def, env);
      return handle(binding, typed, message, env);
    }

    /**
     * Steps 5 and 6, with {@code @retry} (section 10.4): the handler runs until it returns or, with
     * {@code @retry(max: n)}, up to {@code n + 1} times, waiting between the runs. A channel without
     * {@code @retry} answers {@code AGAIN} at the first failure; the last run of one with it is
     * copied when the channel has {@code @deadLetter}, and acknowledged. Every run asks the inbox.
     */
    <E> Transport.Outcome handle(Binding<E> binding, E typed, Transport.Message message, Envelope env) {
      RetryPolicy retry = sub.retry();
      int allowed = retry == null ? 1 : retry.max() + 1;
      for (int run = 1; ; run++) {
        EventMeta meta =
            new EventMeta(
                env.id(), env.source(), env.type(), env.subject(), env.time(), message.attempt() + run - 1);
        try {
          runOnce(binding, typed, env, meta);
          return Transport.Outcome.DONE;
        } catch (Exception e) {
          boolean interrupted = e instanceof InterruptedException;
          if (interrupted) {
            Thread.currentThread().interrupt();
          }
          boolean last = retry != null && run == allowed;
          hook(
              options.onHandlerError,
              new HandlerFailure(binding.def().name(), sub.topic(), sub.group(), meta, e, meta.attempt(), last));
          if (retry == null || interrupted) {
            return Transport.Outcome.AGAIN;
          }
          if (last) {
            return copyToDeadLetter(message, DeadLetterReason.HANDLER, run, List.of(), List.of());
          }
          try {
            options.sleep.sleep(retryWait(retry, run));
          } catch (InterruptedException stop) {
            Thread.currentThread().interrupt();
            return Transport.Outcome.AGAIN;
          }
        }
      }
    }

    /** Steps 5 and 6 of one run: the inbox, then the handler. */
    private <E> void runOnce(Binding<E> binding, E typed, Envelope env, EventMeta meta) throws Exception {
      Runner call = () -> binding.run().handle(typed, meta);
      if (options.inbox == null) {
        call.run();
      } else {
        options.inbox.handle(env.source(), env.id(), call);
      }
    }

    /** The wait after run {@code k} failed (from 1): the delay for fixed, doubled each time for exponential, never more than the maximum. */
    private long retryWait(RetryPolicy retry, int k) {
      long max = options.retryMaxDelayMs;
      long wait = options.retryDelayMs;
      if (retry.backoff() == Backoff.EXPONENTIAL) {
        for (int i = 1; i < k && wait < max; i++) {
          wait = wait > max / 2 ? max : wait * 2;
        }
      }
      return Math.min(wait, max);
    }
  }

  /**
   * Connects the handlers of a service to a transport, one subscription per {@code subscribes}
   * line, each under its group (section 8). A message the subscription has no {@code on} line for
   * is acknowledged and ignored; a rejected one is copied to the dead-letter topic when the channel
   * has {@code @deadLetter}, acknowledged and reported to {@code onEventRejected}; a handler that
   * throws is run again by the consumer with {@code @retry} and leaves the message unacknowledged
   * without it. Closing the returned handle closes every subscription.
   */
  public static Transport.Subscription subscribe(
      Transport transport, List<SubscriptionDef> subs, ConsumerOptions options) {
    Set<String> seen = ConcurrentHashMap.newKeySet();
    List<Transport.Subscription> opened = new ArrayList<>();
    try {
      for (SubscriptionDef sub : subs) {
        Pipeline pipeline = new Pipeline(transport, sub, options, seen);
        opened.add(
            transport.subscribe(
                sub.topic(),
                sub.group(),
                pipeline::consume,
                new Transport.ChannelOptions(sub.ordered(), sub.deadLetter())));
      }
    } catch (RuntimeException e) {
      closeAll(opened);
      throw e;
    }
    return () -> closeAll(opened);
  }

  private static void closeAll(List<Transport.Subscription> opened) {
    List<Transport.Subscription> all;
    synchronized (opened) {
      all = new ArrayList<>(opened);
      opened.clear();
    }
    for (Transport.Subscription s : all) {
      s.close();
    }
  }
}
