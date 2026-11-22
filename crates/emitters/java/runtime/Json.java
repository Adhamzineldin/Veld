package dev.veld.runtime;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * A JSON value. Parsing follows RFC 8259, canonical writing follows RFC 8785 (numbers are
 * formatted like ECMAScript's {@code Number::toString}). No dependencies.
 */
public sealed interface Json permits Json.Null, Json.Bool, Json.Num, Json.Str, Json.Arr, Json.Obj {

  /** The JSON {@code null}. */
  Json NULL = new Null();

  /** The JSON {@code null}. */
  record Null() implements Json {}

  /** A JSON boolean. */
  record Bool(boolean value) implements Json {}

  /** A JSON number. Integers up to 2^53 are exact. */
  record Num(double value) implements Json {}

  /** A JSON string. */
  record Str(String value) implements Json {
    /** Creates a string value. */
    public Str {
      Objects.requireNonNull(value, "value");
    }
  }

  /** A JSON array. */
  record Arr(List<Json> items) implements Json {
    /** Creates an array value (the list is copied). */
    public Arr {
      items = List.copyOf(items);
    }
  }

  /** A JSON object; keeps insertion order. */
  record Obj(Map<String, Json> fields) implements Json {
    /** Creates an object value (the map is copied). */
    public Obj {
      fields = Collections.unmodifiableMap(new LinkedHashMap<>(fields));
    }

    /** The member named {@code key}, or {@code null} when absent. */
    public Json get(String key) {
      return fields.get(key);
    }

    /** Whether the member exists. */
    public boolean has(String key) {
      return fields.containsKey(key);
    }

    /** A copy with {@code key} set to {@code value}. */
    public Obj with(String key, Json value) {
      Map<String, Json> copy = new LinkedHashMap<>(fields);
      copy.put(key, value);
      return new Obj(copy);
    }
  }

  /** Thrown when text is not valid JSON. */
  final class ParseException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    ParseException(String message) {
      super(message);
    }
  }

  /** Builds an object member by member; {@code null} values are skipped. */
  final class Builder {
    private final Map<String, Json> members = new LinkedHashMap<>();

    Builder() {}

    /** Adds {@code value} unless it is {@code null}. */
    public Builder put(String key, Json value) {
      if (value != null) {
        members.put(key, value);
      }
      return this;
    }

    /** Adds the encoded {@code value} unless it is {@code null}. */
    public <T> Builder put(String key, T value, Codec<T> codec) {
      if (value != null) {
        members.put(key, codec.encode(value));
      }
      return this;
    }

    /** The finished object. */
    public Obj build() {
      return new Obj(members);
    }
  }

  /** Starts an object builder. */
  static Builder object() {
    return new Builder();
  }

  /** An empty array. */
  static Arr array() {
    return new Arr(List.of());
  }

  /** An array of the given items. */
  static Arr array(List<? extends Json> items) {
    return new Arr(List.copyOf(items));
  }

  /** A string value. */
  static Json of(String value) {
    return new Str(value);
  }

  /** A boolean value. */
  static Json of(boolean value) {
    return new Bool(value);
  }

  /** An integer number value. */
  static Json of(long value) {
    return new Num((double) value);
  }

  /** A floating point number value. */
  static Json of(double value) {
    return new Num(value);
  }

  /** Parses RFC 8259 text. */
  static Json parse(String text) {
    return JsonParser.parse(text);
  }

  /** This value as an object, or an {@link IllegalArgumentException}. */
  default Obj asObject() {
    if (this instanceof Obj o) {
      return o;
    }
    throw new IllegalArgumentException("expected a JSON object");
  }

  /** This value as an array, or an {@link IllegalArgumentException}. */
  default Arr asArray() {
    if (this instanceof Arr a) {
      return a;
    }
    throw new IllegalArgumentException("expected a JSON array");
  }

  /** This value as a string, or an {@link IllegalArgumentException}. */
  default String asString() {
    if (this instanceof Str s) {
      return s.value();
    }
    throw new IllegalArgumentException("expected a JSON string");
  }

  /** This value as a number, or an {@link IllegalArgumentException}. */
  default double asDouble() {
    if (this instanceof Num n) {
      return n.value();
    }
    throw new IllegalArgumentException("expected a JSON number");
  }

  /** This value as a boolean, or an {@link IllegalArgumentException}. */
  default boolean asBoolean() {
    if (this instanceof Bool b) {
      return b.value();
    }
    throw new IllegalArgumentException("expected a JSON boolean");
  }

  /** Whether this is the JSON {@code null}. */
  default boolean isNull() {
    return this instanceof Null;
  }

  /** Compact JSON text, members in insertion order. */
  default String write() {
    StringBuilder out = new StringBuilder();
    JsonWriter.write(this, out, false);
    return out.toString();
  }

  /** RFC 8785 canonical JSON text. */
  default String canonical() {
    StringBuilder out = new StringBuilder();
    JsonWriter.write(this, out, true);
    return out.toString();
  }
}
