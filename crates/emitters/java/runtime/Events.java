package dev.veld.runtime;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.time.DateTimeException;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.OffsetDateTime;
import java.time.ZoneOffset;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
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
import java.util.regex.Pattern;

/**
 * The envelope of an event message (CloudEvents 1.0, structured JSON mode) and the publish and
 * consume pipelines of spec/events.md sections 6 to 9. The generated publishers and subscribers
 * call {@link #publish} and {@link #subscribe}; nothing here depends on a broker.
 */
public final class Events {
  private Events() {}

  private static final Pattern RE_DATETIME =
      Pattern.compile("^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?(Z|[+-]\\d{2}:\\d{2})$");

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
   */
  public record ChannelDef(String topic, String keyWire, KeyKind keyKind) {}

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

  /**
   * One subscription of a service.
   *
   * @param topic the topic
   * @param group the consumer group
   * @param on the events the subscription handles
   */
  public record SubscriptionDef(String topic, String group, List<Binding<?>> on) {}

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
   * A handler failed: the message is not acknowledged and the transport delivers it again.
   *
   * @param event the name of the event
   * @param topic the topic
   * @param group the consumer group
   * @param meta the metadata of the message
   * @param error what the handler threw
   */
  public record HandlerFailure(String event, String topic, String group, EventMeta meta, Throwable error) {}

  /** The options of a generated publisher: the id source, the clock and the rule hook. */
  public static final class PublisherOptions {
    private Supplier<String> newId = () -> UUID.randomUUID().toString();
    private Supplier<Instant> clock = Instant::now;
    private RuleWalk.Hook onRuleViolation;

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
  }

  /** The hooks of a generated subscriber. All are optional and a failing hook is ignored. */
  public static final class ConsumerOptions {
    private Consumer<EventRejected> onEventRejected;
    private Consumer<ContractMismatch> onContractMismatch;
    private RuleWalk.Hook onRuleViolation;
    private Consumer<HandlerFailure> onHandlerError;

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

    /** A handler that failed. */
    public ConsumerOptions onHandlerError(Consumer<HandlerFailure> hook) {
      this.onHandlerError = hook;
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

  // -- canonical scalars ----------------------------------------------------------------------

  /** A datetime in UTC with {@code Z}, the fraction {@code .fff} only when the milliseconds are not zero. */
  static String isoMillis(Instant at) {
    LocalDateTime d = LocalDateTime.ofInstant(at.truncatedTo(ChronoUnit.MILLIS), ZoneOffset.UTC);
    String base =
        String.format(
            Locale.ROOT,
            "%04d-%02d-%02dT%02d:%02d:%02d",
            d.getYear(),
            d.getMonthValue(),
            d.getDayOfMonth(),
            d.getHour(),
            d.getMinute(),
            d.getSecond());
    int millis = d.getNano() / 1_000_000;
    return millis == 0 ? base + "Z" : base + String.format(Locale.ROOT, ".%03dZ", millis);
  }

  private static String canonDatetime(String v) {
    if (!RE_DATETIME.matcher(v).matches()) {
      return v;
    }
    try {
      return isoMillis(OffsetDateTime.parse(v).toInstant());
    } catch (DateTimeException e) {
      return v;
    }
  }

  /** The canonical scalars of spec/ir.md section 5.3 in a wire value: lowercase uuids, UTC datetimes. */
  static Json canonScalars(Schema schema, Json v) {
    Schema s = schema.resolve();
    switch (s.kind) {
      case UUID:
        return v instanceof Json.Str str ? Json.of(str.value().toLowerCase(Locale.ROOT)) : v;
      case DATETIME:
        return v instanceof Json.Str str ? Json.of(canonDatetime(str.value())) : v;
      case LIST:
        return v instanceof Json.Arr arr ? canonItems(s.of, arr) : v;
      case MAP:
        return v instanceof Json.Obj obj ? canonMembers(s.of, obj) : v;
      case PAGE:
        return v instanceof Json.Obj obj && obj.get("items") != null
            ? obj.with("items", canonScalars(Schema.list(s.of), obj.get("items")))
            : v;
      case OBJECT:
        return v instanceof Json.Obj obj ? canonObject(s, obj) : v;
      case UNION:
        return canonUnion(s, v);
      default:
        return v;
    }
  }

  private static Json canonItems(Schema of, Json.Arr arr) {
    List<Json> items = new ArrayList<>();
    for (Json item : arr.items()) {
      items.add(canonScalars(of, item));
    }
    return Json.array(items);
  }

  private static Json canonMembers(Schema of, Json.Obj obj) {
    Json.Obj out = obj;
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      out = out.with(e.getKey(), canonScalars(of, e.getValue()));
    }
    return out;
  }

  private static Json canonObject(Schema s, Json.Obj obj) {
    Json.Obj out = obj;
    for (Schema.Field f : s.fields) {
      Json x = out.get(f.wire());
      if (x != null) {
        out = out.with(f.wire(), canonScalars(f.type(), x));
      }
    }
    return out;
  }

  private static Json canonUnion(Schema s, Json v) {
    Json label = v instanceof Json.Obj obj ? obj.get(s.discriminator) : null;
    Schema arm = label instanceof Json.Str str ? s.armFor(str.value()) : null;
    return arm == null ? v : canonScalars(arm, v);
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

  /**
   * The invariants of an event in wire form, in full mode: the enforced violations are returned,
   * and both kinds are reported to the hook (the enforced ones first). An audited rule never
   * changes what happens.
   */
  private static List<Violation> runRules(
      EventDef<?> def, Json prepared, RuleWalk.Hook fn, RuleWalk.Position position) {
    long started = System.nanoTime();
    // #if rollout
    Audit.begin();
    // #endif
    List<Violation> found = RuleWalk.check(def.schema(), prepared, "$", false);
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
    Json wire = canonScalars(def.schema(), prepared);
    List<Issue> issues = Wire.validate(def.schema(), wire, "$", false);
    if (!issues.isEmpty()) {
      throw invalid(ctx, issues, List.of());
    }
    List<Violation> violations = runRules(def, wire, ctx.options().onRuleViolation, RuleWalk.Position.OUTPUT);
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
    String time = isoMillis(at);
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
    ctx.transport().publish(channel.topic(), subject, envelope.build().canonical().getBytes(StandardCharsets.UTF_8));
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

  /** One subscription being consumed. */
  private static final class Pipeline {
    private final SubscriptionDef sub;
    private final ConsumerOptions options;
    private final Set<String> seen;

    Pipeline(SubscriptionDef sub, ConsumerOptions options, Set<String> seen) {
      this.sub = sub;
      this.options = options;
      this.seen = seen;
    }

    Transport.Outcome reject(
        Transport.Message message, Reason reason, String text, String event, List<Issue> issues, List<Violation> violations) {
      hook(
          options.onEventRejected,
          new EventRejected(
              reason, text, sub.topic(), sub.group(), event, List.copyOf(issues), List.copyOf(violations), message.bytes()));
      return Transport.Outcome.DONE;
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
      if (Objects.equals(env.veldrules(), def.digest())) {
        List<Violation> violations = runRules(def, prepared, options.onRuleViolation, RuleWalk.Position.INPUT);
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
      EventMeta meta = new EventMeta(env.id(), env.source(), env.type(), env.subject(), env.time(), message.attempt());
      try {
        binding.run().handle(typed, meta);
      } catch (Exception e) {
        if (e instanceof InterruptedException) {
          Thread.currentThread().interrupt();
        }
        hook(options.onHandlerError, new HandlerFailure(def.name(), sub.topic(), sub.group(), meta, e));
        return Transport.Outcome.AGAIN;
      }
      return Transport.Outcome.DONE;
    }
  }

  /**
   * Connects the handlers of a service to a transport, one subscription per {@code subscribes}
   * line, each under its group (section 8). A message the subscription has no {@code on} line for
   * is acknowledged and ignored; a rejected one is acknowledged and reported to
   * {@code onEventRejected}; a handler that throws leaves it unacknowledged. Closing the returned
   * handle closes every subscription.
   */
  public static Transport.Subscription subscribe(
      Transport transport, List<SubscriptionDef> subs, ConsumerOptions options) {
    Set<String> seen = ConcurrentHashMap.newKeySet();
    List<Transport.Subscription> opened = new ArrayList<>();
    try {
      for (SubscriptionDef sub : subs) {
        Pipeline pipeline = new Pipeline(sub, options, seen);
        opened.add(transport.subscribe(sub.topic(), sub.group(), pipeline::consume));
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
