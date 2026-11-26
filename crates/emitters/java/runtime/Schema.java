package dev.veld.runtime;

import java.util.List;
import java.util.function.Supplier;

/**
 * A declarative description of the wire form of a declaration. Generated code builds one per
 * model, error, enum and union; {@link Wire} interprets it for validation, defaults and
 * canonical bytes, so every generated package behaves identically.
 */
public final class Schema {

  /** What a node describes. */
  public enum Kind {
    STRING, INT, LONG, FLOAT, DECIMAL, BOOL, DATE, DATETIME, TIME, UUID, BYTES, JSON,
    ENUM, LIST, MAP, PAGE, OBJECT, UNION, REF
  }

  /**
   * A field of an object schema.
   *
   * @param name the source name
   * @param wire the name on the wire
   * @param type the field type
   * @param optional whether the field may be absent
   * @param defaultValue the default (wire form), or {@code null}
   * @param readonly whether the field is ignored in request bodies
   */
  public record Field(String name, String wire, Schema type, boolean optional, Json defaultValue, boolean readonly) {}

  /**
   * An arm of a union schema.
   *
   * @param label the discriminator value
   * @param schema the arm's schema, resolved lazily because types may be recursive
   */
  public record Arm(String label, Supplier<Schema> schema) {}

  /** Constraints applied to the node they are attached to. */
  record Bounds(Double min, Double max, Long minLen, Long maxLen, String pattern, String format) {
    static final Bounds NONE = new Bounds(null, null, null, null, null, null);
  }

  final Kind kind;
  final Schema of;
  final List<String> values;
  final List<Field> fields;
  final String discriminator;
  final List<Arm> arms;
  final Supplier<Schema> target;
  final Bounds bounds;
  // #if rules
  Rules.Check check;
  // #endif

  private Schema(Kind kind, Schema of, List<String> values, List<Field> fields, String discriminator,
      List<Arm> arms, Supplier<Schema> target, Bounds bounds) {
    this.kind = kind;
    this.of = of;
    this.values = values;
    this.fields = fields;
    this.discriminator = discriminator;
    this.arms = arms;
    this.target = target;
    this.bounds = bounds;
  }

  private static Schema simple(Kind kind) {
    return new Schema(kind, null, List.of(), List.of(), null, List.of(), null, Bounds.NONE);
  }

  private Schema withBounds(Bounds b) {
    return new Schema(kind, of, values, fields, discriminator, arms, target, b);
  }

  /** A string. */
  public static Schema string() {
    return simple(Kind.STRING);
  }

  /** A 32-bit integer (a JSON number). */
  public static Schema int32() {
    return simple(Kind.INT);
  }

  /** A 64-bit integer (a decimal string on the wire). */
  public static Schema int64() {
    return simple(Kind.LONG);
  }

  /** A floating point number. */
  public static Schema float64() {
    return simple(Kind.FLOAT);
  }

  /** An exact decimal (a string on the wire). */
  public static Schema decimal() {
    return simple(Kind.DECIMAL);
  }

  /** A boolean. */
  public static Schema bool() {
    return simple(Kind.BOOL);
  }

  /** A calendar date, {@code YYYY-MM-DD}. */
  public static Schema date() {
    return simple(Kind.DATE);
  }

  /** An RFC 3339 date and time. */
  public static Schema datetime() {
    return simple(Kind.DATETIME);
  }

  /** A time of day, {@code HH:MM:SS}. */
  public static Schema time() {
    return simple(Kind.TIME);
  }

  /** A UUID. */
  public static Schema uuid() {
    return simple(Kind.UUID);
  }

  /** Base64 bytes. */
  public static Schema bytes() {
    return simple(Kind.BYTES);
  }

  /** Any JSON value. */
  public static Schema json() {
    return simple(Kind.JSON);
  }

  /** An enum with the given wire values. */
  public static Schema enumOf(String... wireValues) {
    return new Schema(Kind.ENUM, null, List.of(wireValues), List.of(), null, List.of(), null, Bounds.NONE);
  }

  /** A list of {@code of}. */
  public static Schema list(Schema of) {
    return new Schema(Kind.LIST, of, List.of(), List.of(), null, List.of(), null, Bounds.NONE);
  }

  /** A map from string to {@code of}. */
  public static Schema map(Schema of) {
    return new Schema(Kind.MAP, of, List.of(), List.of(), null, List.of(), null, Bounds.NONE);
  }

  /** A page of {@code of}. */
  public static Schema page(Schema of) {
    return new Schema(Kind.PAGE, of, List.of(), List.of(), null, List.of(), null, Bounds.NONE);
  }

  /** An object with the given fields. */
  public static Schema object(Field... fields) {
    return new Schema(Kind.OBJECT, null, List.of(), List.of(fields), null, List.of(), null, Bounds.NONE);
  }

  /** A union discriminated by {@code discriminator}. */
  public static Schema union(String discriminator, Arm... arms) {
    return new Schema(Kind.UNION, null, List.of(), List.of(), discriminator, List.of(arms), null, Bounds.NONE);
  }

  /** A reference resolved on first use, which makes recursive types possible. */
  public static Schema ref(Supplier<Schema> target) {
    return new Schema(Kind.REF, null, List.of(), List.of(), null, List.of(), target, Bounds.NONE);
  }

  /** A field of an object schema. */
  public static Field field(String name, String wire, Schema type, boolean optional, Json defaultValue,
      boolean readonly) {
    return new Field(name, wire, type, optional, defaultValue, readonly);
  }

  /** An arm of a union schema. */
  public static Arm arm(String label, Supplier<Schema> schema) {
    return new Arm(label, schema);
  }

  /** A copy that requires numbers to be at least {@code value}. */
  public Schema min(double value) {
    return withBounds(new Bounds(value, bounds.max(), bounds.minLen(), bounds.maxLen(), bounds.pattern(), bounds.format()));
  }

  /** A copy that requires numbers to be at most {@code value}. */
  public Schema max(double value) {
    return withBounds(new Bounds(bounds.min(), value, bounds.minLen(), bounds.maxLen(), bounds.pattern(), bounds.format()));
  }

  /** A copy that requires strings, lists and maps to have at least {@code value} elements. */
  public Schema minLen(long value) {
    return withBounds(new Bounds(bounds.min(), bounds.max(), value, bounds.maxLen(), bounds.pattern(), bounds.format()));
  }

  /** A copy that requires strings, lists and maps to have at most {@code value} elements. */
  public Schema maxLen(long value) {
    return withBounds(new Bounds(bounds.min(), bounds.max(), bounds.minLen(), value, bounds.pattern(), bounds.format()));
  }

  /** A copy that requires strings to contain a match of the regular expression. */
  public Schema pattern(String regex) {
    return withBounds(new Bounds(bounds.min(), bounds.max(), bounds.minLen(), bounds.maxLen(), regex, bounds.format()));
  }

  /** A copy that requires strings to have a format: email, uri, hostname, ipv4 or ipv6. */
  public Schema format(String name) {
    return withBounds(new Bounds(bounds.min(), bounds.max(), bounds.minLen(), bounds.maxLen(), bounds.pattern(), name));
  }

  // #if rules
  /** A copy of this object schema that runs {@code check} for every object it describes. */
  public Schema checked(Rules.Check check) {
    Schema copy = withBounds(bounds);
    copy.check = check;
    return copy;
  }

  // #endif
  // #if derived
  /** The derived fields of this object schema, or {@code null}. */
  Derive.Compute derive;

  /** The wire names of the derived fields of this object schema. */
  List<String> derivedWires = List.of();

  /**
   * A copy of this object schema whose derived fields (spec/rules.md section 17.2) are
   * {@code wires}: nobody supplies them, they are not validated, and {@code compute} computes them.
   */
  public Schema derived(Derive.Compute compute, String... wires) {
    Schema copy = withBounds(bounds);
    copy.check = check;
    copy.derive = compute;
    copy.derivedWires = List.of(wires);
    return copy;
  }

  /** Whether the member {@code wire} of this object is a derived field. */
  boolean derives(String wire) {
    return derivedWires.contains(wire);
  }

  // #endif
  // #if marks
  /** The platform marks of this object schema (spec/platform.md sections 2 and 3), or {@code null}. */
  Platform.Marks marks;

  /** A copy of this object schema with the platform marks of its declaration. */
  public Schema marked(Platform.Marks value) {
    Schema copy = withBounds(bounds);
    // #endif
    // #if marks_rules
    copy.check = check;
    // #endif
    // #if marks_derived
    copy.derive = derive;
    copy.derivedWires = derivedWires;
    // #endif
    // #if marks
    copy.marks = value;
    return copy;
  }

  // #endif
  /** Follows references until a concrete node is reached. */
  Schema resolve() {
    Schema cur = this;
    while (cur.kind == Kind.REF) {
      cur = cur.target.get();
    }
    return cur;
  }

  Schema armFor(String label) {
    for (Arm a : arms) {
      if (a.label().equals(label)) {
        return a.schema().get();
      }
    }
    return null;
  }
}
