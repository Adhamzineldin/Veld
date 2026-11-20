package dev.veld.runtime;

import java.math.BigDecimal;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalTime;
import java.time.OffsetDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Function;

/**
 * Converts between Java values and their wire form. Decoding expects a value that was validated
 * and prepared with {@link Wire}, so a malformed value is a programming error and raises an
 * {@link IllegalArgumentException}.
 *
 * @param <T> the Java type
 */
public interface Codec<T> {

  /** Wire form to Java. */
  T decode(Json json);

  /** Java to wire form. */
  Json encode(T value);

  /** A codec from a decoder and an encoder. */
  static <T> Codec<T> of(Function<Json, T> decoder, Function<T, Json> encoder) {
    return new Codec<>() {
      @Override
      public T decode(Json json) {
        return decoder.apply(json);
      }

      @Override
      public Json encode(T value) {
        return encoder.apply(value);
      }
    };
  }

  /** The member {@code key} of {@code obj}, which must be present. */
  static <T> T req(Json.Obj obj, String key, Codec<T> codec) {
    Json j = obj.get(key);
    if (j == null) {
      throw new IllegalArgumentException("missing field `" + key + "`");
    }
    return codec.decode(j);
  }

  /** The member {@code key} of {@code obj}, or {@code null} when absent. */
  static <T> T opt(Json.Obj obj, String key, Codec<T> codec) {
    Json j = obj.get(key);
    return j == null ? null : codec.decode(j);
  }

  /** {@code string}. */
  static Codec<String> string() {
    return of(Json::asString, Json::of);
  }

  /** {@code int}, a JSON number. */
  static Codec<Integer> int32() {
    return of(j -> (int) j.asDouble(), v -> Json.of((long) v));
  }

  /** {@code long}, a decimal string on the wire. */
  static Codec<Long> int64() {
    return of(j -> Long.parseLong(j.asString()), v -> Json.of(Long.toString(v)));
  }

  /** {@code float}, a JSON number. */
  static Codec<Double> float64() {
    return of(Json::asDouble, v -> Json.of((double) v));
  }

  /** {@code bool}. */
  static Codec<Boolean> bool() {
    return of(Json::asBoolean, v -> Json.of((boolean) v));
  }

  /** {@code decimal}, a plain decimal string on the wire. */
  static Codec<BigDecimal> decimal() {
    return of(j -> new BigDecimal(j.asString()), v -> Json.of(v.toPlainString()));
  }

  /** {@code date}. */
  static Codec<LocalDate> date() {
    return of(j -> LocalDate.parse(j.asString()), v -> Json.of(v.toString()));
  }

  /** {@code datetime}: any RFC 3339 offset is accepted, the wire form is UTC with a {@code Z}. */
  static Codec<Instant> datetime() {
    return of(j -> OffsetDateTime.parse(j.asString()).toInstant(), v -> Json.of(DateTimeFormatter.ISO_INSTANT.format(v)));
  }

  /** {@code time}, {@code HH:MM:SS} with an optional fraction. */
  static Codec<LocalTime> time() {
    return of(j -> LocalTime.parse(j.asString()), v -> Json.of(DateTimeFormatter.ISO_LOCAL_TIME.format(v)));
  }

  /** {@code uuid}, lowercase on the wire. */
  static Codec<UUID> uuid() {
    return of(j -> UUID.fromString(j.asString()), v -> Json.of(v.toString()));
  }

  /** {@code bytes}, padded standard base64 on the wire. */
  static Codec<byte[]> bytes() {
    return of(j -> Base64.getDecoder().decode(j.asString()), v -> Json.of(Base64.getEncoder().encodeToString(v)));
  }

  /** {@code json}: any JSON value, unchanged. */
  static Codec<Json> json() {
    return of(j -> j, v -> v);
  }

  /** {@code T[]}. */
  static <T> Codec<List<T>> list(Codec<T> item) {
    return of(
        j -> {
          List<T> out = new ArrayList<>();
          for (Json x : j.asArray().items()) {
            out.add(item.decode(x));
          }
          return Collections.unmodifiableList(out);
        },
        v -> {
          List<Json> out = new ArrayList<>();
          for (T x : v) {
            out.add(item.encode(x));
          }
          return Json.array(out);
        });
  }

  /** {@code Map<string, T>}. */
  static <T> Codec<Map<String, T>> map(Codec<T> value) {
    return of(
        j -> {
          Map<String, T> out = new LinkedHashMap<>();
          for (Map.Entry<String, Json> e : j.asObject().fields().entrySet()) {
            out.put(e.getKey(), value.decode(e.getValue()));
          }
          return Collections.unmodifiableMap(out);
        },
        v -> {
          Map<String, Json> out = new LinkedHashMap<>();
          for (Map.Entry<String, T> e : v.entrySet()) {
            out.put(e.getKey(), value.encode(e.getValue()));
          }
          return new Json.Obj(out);
        });
  }

  /** {@code Page<T>}. */
  static <T> Codec<Page<T>> page(Codec<T> item) {
    Codec<List<T>> items = list(item);
    return of(
        j -> {
          Json.Obj o = j.asObject();
          return new Page<>(
              req(o, "items", items), req(o, "total", int64()), req(o, "page", int32()), req(o, "size", int32()));
        },
        v -> Json.object()
            .put("items", v.items(), items)
            .put("total", v.total(), int64())
            .put("page", v.page(), int32())
            .put("size", v.size(), int32())
            .build());
  }
}
