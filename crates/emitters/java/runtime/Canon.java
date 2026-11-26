package dev.veld.runtime;

import java.math.BigInteger;
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
import java.util.regex.Pattern;

/**
 * The canonical scalars of spec/ir.md section 5.3 in a wire value: lower-case uuids and UTC
 * datetimes with {@code .fff} only when the milliseconds are not zero. The data of an event and the
 * fingerprint of an idempotency key (spec/rules.md section 11.10) are written with the same code.
 */
final class Canon {
  private Canon() {}

  private static final Pattern RE_DATETIME =
      Pattern.compile("^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?(Z|[+-]\\d{2}:\\d{2})$");
  private static final Pattern RE_LONG = Pattern.compile("^-?\\d{1,19}$");

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

  private static String datetime(String v) {
    if (!RE_DATETIME.matcher(v).matches()) {
      return v;
    }
    try {
      return isoMillis(OffsetDateTime.parse(v).toInstant());
    } catch (DateTimeException e) {
      return v;
    }
  }

  /** A {@code long} as the canonical decimal text of its value: {@code 007} is {@code 7}, {@code -0} is {@code 0}. */
  private static String longText(String v) {
    return RE_LONG.matcher(v).matches() ? new BigInteger(v).toString() : v;
  }

  /**
   * The canonical scalars of a wire value. {@code longs} also writes a {@code long} as the canonical
   * decimal text of its value (the fingerprint does; the data of an event keeps what it was given).
   */
  static Json scalars(Schema schema, Json v, boolean longs) {
    Schema s = schema.resolve();
    switch (s.kind) {
      case UUID:
        return v instanceof Json.Str str ? Json.of(str.value().toLowerCase(Locale.ROOT)) : v;
      case DATETIME:
        return v instanceof Json.Str str ? Json.of(datetime(str.value())) : v;
      case LONG:
        return longs && v instanceof Json.Str str ? Json.of(longText(str.value())) : v;
      case LIST:
        return v instanceof Json.Arr arr ? items(s.of, arr, longs) : v;
      case MAP:
        return v instanceof Json.Obj obj ? members(s.of, obj, longs) : v;
      case PAGE:
        return v instanceof Json.Obj obj && obj.get("items") != null
            ? obj.with("items", scalars(Schema.list(s.of), obj.get("items"), longs))
            : v;
      case OBJECT:
        return v instanceof Json.Obj obj ? object(s, obj, longs) : v;
      case UNION:
        return union(s, v, longs);
      default:
        return v;
    }
  }

  private static Json items(Schema of, Json.Arr arr, boolean longs) {
    List<Json> items = new ArrayList<>();
    for (Json item : arr.items()) {
      items.add(scalars(of, item, longs));
    }
    return Json.array(items);
  }

  private static Json members(Schema of, Json.Obj obj, boolean longs) {
    Json.Obj out = obj;
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      out = out.with(e.getKey(), scalars(of, e.getValue(), longs));
    }
    return out;
  }

  private static Json object(Schema s, Json.Obj obj, boolean longs) {
    Json.Obj out = obj;
    for (Schema.Field f : s.fields) {
      Json x = out.get(f.wire());
      if (x != null) {
        out = out.with(f.wire(), scalars(f.type(), x, longs));
      }
    }
    return out;
  }

  private static Json union(Schema s, Json v, boolean longs) {
    Json label = v instanceof Json.Obj obj ? obj.get(s.discriminator) : null;
    Schema arm = label instanceof Json.Str str ? s.armFor(str.value()) : null;
    return arm == null ? v : scalars(arm, v, longs);
  }
}
