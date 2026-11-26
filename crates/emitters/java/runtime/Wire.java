package dev.veld.runtime;

import java.math.BigInteger;
import java.time.DateTimeException;
import java.time.Instant;
import java.time.LocalDate;
import java.time.OffsetDateTime;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Validation, defaults and canonical bytes for values in wire form ({@link Json}), driven by a
 * {@link Schema}. This is the Java twin of the TypeScript runtime's {@code validate},
 * {@code fromWire} and {@code canonicalOf}.
 */
public final class Wire {
  private Wire() {}

  /** How {@link #prepare} treats a value. */
  public enum Mode {
    /** A request body or parameter: read-only fields are dropped, defaults are applied. */
    REQUEST,
    /** A response or error payload: defaults are applied. */
    RESPONSE,
    /** Canonical bytes (spec/ir.md section 5.3): defaults applied, UUIDs and datetimes normalised. */
    CANONICAL
  }

  private static final Pattern RE_DATE = Pattern.compile("^(\\d{4})-(\\d{2})-(\\d{2})$");
  private static final Pattern RE_DATETIME =
      Pattern.compile("^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?(Z|[+-]\\d{2}:\\d{2})$");
  private static final Pattern RE_TIME = Pattern.compile("^([01]\\d|2[0-3]):[0-5]\\d:[0-5]\\d(\\.\\d+)?$");
  private static final Pattern RE_UUID =
      Pattern.compile("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$");
  private static final Pattern RE_DECIMAL = Pattern.compile("^-?\\d+(\\.\\d+)?$");
  private static final Pattern RE_LONG = Pattern.compile("^-?\\d{1,19}$");
  private static final Pattern RE_EMAIL = Pattern.compile("^[^\\s@]+@[^\\s@.]+(\\.[^\\s@.]+)+$");
  private static final Pattern RE_URI = Pattern.compile("^[A-Za-z][A-Za-z0-9+.-]*:\\S*$");
  private static final Pattern RE_HOST = Pattern.compile(
      "^(?=.{1,253}$)[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$");
  private static final Pattern RE_IPV4_PART = Pattern.compile("^(0|[1-9]\\d{0,2})$");
  private static final Pattern RE_IPV6_CHARS = Pattern.compile("^[0-9a-fA-F:.]+$");
  private static final Pattern RE_HEX_GROUP = Pattern.compile("^[0-9a-fA-F]{1,4}$");
  private static final BigInteger LONG_MIN = BigInteger.valueOf(Long.MIN_VALUE);
  private static final BigInteger LONG_MAX = BigInteger.valueOf(Long.MAX_VALUE);
  private static final Map<String, Pattern> USER_PATTERNS = new ConcurrentHashMap<>();

  // -- validation ----------------------------------------------------------------------------

  /**
   * Validates a wire value against {@code schema}.
   *
   * @param request when true, read-only fields may be absent
   * @return the problems found; empty when the value is valid
   */
  public static List<Issue> validate(Schema schema, Json value, String path, boolean request) {
    List<Issue> out = new ArrayList<>();
    check(schema, value, path, out, request);
    return out;
  }

  private static void push(List<Issue> out, String path, String code, String message) {
    out.add(new Issue(path, code, message));
  }

  private static void check(Schema s, Json v, String path, List<Issue> out, boolean request) {
    Schema node = s.resolve();
    switch (node.kind) {
      case ENUM -> checkEnum(node, v, path, out);
      case LIST -> checkList(node, v, path, out, request);
      case MAP -> checkMap(node, v, path, out, request);
      case PAGE -> checkPage(node, v, path, out, request);
      case OBJECT -> checkObject(node, v, path, out, request);
      case UNION -> checkUnion(node, v, path, out, request);
      default -> {
        if (!scalarOk(node.kind, v)) {
          push(out, path, "type", "must be a valid " + node.kind.name().toLowerCase(Locale.ROOT));
          return;
        }
        checkBounds(node, v, path, out);
      }
    }
  }

  private static void checkEnum(Schema s, Json v, String path, List<Issue> out) {
    if (!(v instanceof Json.Str str) || !s.values.contains(str.value())) {
      push(out, path, "enum", "must be one of " + String.join(", ", s.values));
    }
  }

  private static void checkList(Schema s, Json v, String path, List<Issue> out, boolean request) {
    if (!(v instanceof Json.Arr arr)) {
      push(out, path, "type", "must be an array");
      return;
    }
    checkBounds(s, v, path, out);
    for (int i = 0; i < arr.items().size(); i++) {
      check(s.of, arr.items().get(i), path + "[" + i + "]", out, request);
    }
  }

  private static void checkMap(Schema s, Json v, String path, List<Issue> out, boolean request) {
    if (!(v instanceof Json.Obj obj)) {
      push(out, path, "type", "must be an object");
      return;
    }
    checkBounds(s, v, path, out);
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      check(s.of, e.getValue(), path + "." + e.getKey(), out, request);
    }
  }

  private static void checkPage(Schema s, Json v, String path, List<Issue> out, boolean request) {
    if (!(v instanceof Json.Obj obj)) {
      push(out, path, "type", "must be an object");
      return;
    }
    check(Schema.list(s.of), orAbsent(obj.get("items")), path + ".items", out, request);
    check(Schema.int64(), orAbsent(obj.get("total")), path + ".total", out, request);
    check(Schema.int32(), orAbsent(obj.get("page")), path + ".page", out, request);
    check(Schema.int32(), orAbsent(obj.get("size")), path + ".size", out, request);
  }

  /** A missing member is checked as JSON {@code null}, which no schema but {@code json} accepts. */
  private static Json orAbsent(Json v) {
    return v == null ? Json.NULL : v;
  }

  private static void checkObject(Schema s, Json v, String path, List<Issue> out, boolean request) {
    if (!(v instanceof Json.Obj obj)) {
      push(out, path, "type", "must be an object");
      return;
    }
    for (Schema.Field f : s.fields) {
      Json x = obj.get(f.wire());
      String at = path + "." + f.wire();
      if (x == null) {
        if (!f.optional() && f.defaultValue() == null && !(request && f.readonly())) {
          push(out, at, "required", "is required");
        }
        continue;
      }
      check(f.type(), x, at, out, request);
    }
  }

  private static void checkUnion(Schema s, Json v, String path, List<Issue> out, boolean request) {
    Json label = v instanceof Json.Obj obj ? obj.get(s.discriminator) : null;
    Schema arm = label instanceof Json.Str str ? s.armFor(str.value()) : null;
    if (arm == null) {
      List<String> labels = new ArrayList<>();
      for (Schema.Arm a : s.arms) {
        labels.add(a.label());
      }
      push(out, path + "." + s.discriminator, "discriminator", "must be one of " + String.join(", ", labels));
      return;
    }
    check(arm, v, path, out, request);
  }

  private static void checkBounds(Schema s, Json v, String path, List<Issue> out) {
    Schema.Bounds b = s.bounds;
    if (v instanceof Json.Num n) {
      double x = n.value();
      if (b.min() != null && x < b.min()) {
        push(out, path, "min", "must be at least " + JsonWriter.esNumber(b.min()));
      }
      if (b.max() != null && x > b.max()) {
        push(out, path, "max", "must be at most " + JsonWriter.esNumber(b.max()));
      }
      return;
    }
    long len = lengthOf(v);
    if (len >= 0) {
      if (b.minLen() != null && len < b.minLen()) {
        push(out, path, "min_length", "length must be at least " + b.minLen());
      }
      if (b.maxLen() != null && len > b.maxLen()) {
        push(out, path, "max_length", "length must be at most " + b.maxLen());
      }
    }
    if (v instanceof Json.Str str) {
      checkStringRules(b, str.value(), path, out);
    }
  }

  private static long lengthOf(Json v) {
    if (v instanceof Json.Str str) {
      return str.value().codePointCount(0, str.value().length());
    }
    if (v instanceof Json.Arr arr) {
      return arr.items().size();
    }
    if (v instanceof Json.Obj obj) {
      return obj.fields().size();
    }
    return -1;
  }

  private static void checkStringRules(Schema.Bounds b, String value, String path, List<Issue> out) {
    if (b.pattern() != null) {
      Pattern p = USER_PATTERNS.computeIfAbsent(b.pattern(), Pattern::compile);
      if (!p.matcher(value).find()) {
        push(out, path, "pattern", "must match " + b.pattern());
      }
    }
    if (b.format() != null && !formatOk(b.format(), value)) {
      push(out, path, "format", "must be a valid " + b.format());
    }
  }

  private static boolean formatOk(String format, String s) {
    return switch (format) {
      case "email" -> RE_EMAIL.matcher(s).matches();
      case "uri" -> RE_URI.matcher(s).matches();
      case "hostname" -> RE_HOST.matcher(s).matches();
      case "ipv4" -> isIpv4(s);
      case "ipv6" -> isIpv6(s);
      default -> true;
    };
  }

  private static boolean isIpv4(String s) {
    String[] parts = s.split("\\.", -1);
    if (parts.length != 4) {
      return false;
    }
    for (String p : parts) {
      if (!RE_IPV4_PART.matcher(p).matches() || Integer.parseInt(p) > 255) {
        return false;
      }
    }
    return true;
  }

  private static boolean isIpv6(String s) {
    if (!RE_IPV6_CHARS.matcher(s).matches()) {
      return false;
    }
    String[] halves = s.split("::", -1);
    if (halves.length > 2) {
      return false;
    }
    List<String> all = new ArrayList<>();
    for (String half : halves) {
      if (!half.isEmpty()) {
        all.addAll(List.of(half.split(":", -1)));
      }
    }
    int count = 0;
    for (int i = 0; i < all.size(); i++) {
      String g = all.get(i);
      if (i == all.size() - 1 && g.contains(".")) {
        if (!isIpv4(g)) {
          return false;
        }
        count += 2;
      } else if (RE_HEX_GROUP.matcher(g).matches()) {
        count += 1;
      } else {
        return false;
      }
    }
    return halves.length == 2 ? count <= 7 : count == 8;
  }

  private static boolean scalarOk(Schema.Kind kind, Json v) {
    return switch (kind) {
      case STRING -> v instanceof Json.Str;
      case INT -> v instanceof Json.Num n && isInt32(n.value());
      case LONG -> v instanceof Json.Str s && isInt64(s.value());
      case FLOAT -> v instanceof Json.Num;
      case DECIMAL -> v instanceof Json.Str s && RE_DECIMAL.matcher(s.value()).matches();
      case BOOL -> v instanceof Json.Bool;
      case DATE -> v instanceof Json.Str s && validDate(s.value());
      case DATETIME -> v instanceof Json.Str s && validDatetime(s.value());
      case TIME -> v instanceof Json.Str s && RE_TIME.matcher(s.value()).matches();
      case UUID -> v instanceof Json.Str s && RE_UUID.matcher(s.value()).matches();
      case BYTES -> v instanceof Json.Str s && validBase64(s.value());
      case JSON -> true;
      default -> false;
    };
  }

  private static boolean isInt32(double d) {
    return d == Math.rint(d) && d >= Integer.MIN_VALUE && d <= Integer.MAX_VALUE;
  }

  private static boolean isInt64(String s) {
    if (!RE_LONG.matcher(s).matches()) {
      return false;
    }
    BigInteger b = new BigInteger(s);
    return b.compareTo(LONG_MIN) >= 0 && b.compareTo(LONG_MAX) <= 0;
  }

  private static boolean validDate(String s) {
    Matcher m = RE_DATE.matcher(s);
    if (!m.matches()) {
      return false;
    }
    try {
      LocalDate.of(Integer.parseInt(m.group(1)), Integer.parseInt(m.group(2)), Integer.parseInt(m.group(3)));
      return true;
    } catch (DateTimeException e) {
      return false;
    }
  }

  private static boolean validDatetime(String s) {
    if (!RE_DATETIME.matcher(s).matches()) {
      return false;
    }
    try {
      OffsetDateTime.parse(s);
      return true;
    } catch (DateTimeException e) {
      return false;
    }
  }

  /** Standard base64 with padding, checked without regular expressions so long inputs are safe. */
  private static boolean validBase64(String s) {
    int n = s.length();
    if (n % 4 != 0) {
      return false;
    }
    int pad = n >= 2 && s.charAt(n - 2) == '=' ? 2 : n >= 1 && s.charAt(n - 1) == '=' ? 1 : 0;
    for (int i = 0; i < n - pad; i++) {
      char c = s.charAt(i);
      boolean ok = c >= 'A' && c <= 'Z' || c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '+' || c == '/';
      if (!ok) {
        return false;
      }
    }
    return pad == 0 || s.charAt(n - 1) == '=';
  }

  // -- defaults, normalisation, canonical bytes ----------------------------------------------

  /** Applies defaults (and, per {@code mode}, drops read-only fields or normalises values). */
  public static Json prepare(Schema schema, Json value, Mode mode) {
    Schema s = schema.resolve();
    return switch (s.kind) {
      case LIST -> value instanceof Json.Arr arr ? mapItems(s.of, arr, mode) : value;
      case MAP -> value instanceof Json.Obj obj ? mapMembers(s.of, obj, mode) : value;
      case PAGE -> value instanceof Json.Obj obj ? preparePage(s, obj, mode) : value;
      case OBJECT -> value instanceof Json.Obj obj ? prepareObject(s, obj, mode) : value;
      case UNION -> prepareUnion(s, value, mode);
      case UUID -> mode == Mode.CANONICAL && value instanceof Json.Str str ? Json.of(str.value().toLowerCase(Locale.ROOT)) : value;
      case DATETIME -> mode == Mode.CANONICAL && value instanceof Json.Str str && RE_DATETIME.matcher(str.value()).matches()
          ? Json.of(normalizeDatetime(str.value()))
          : value;
      default -> value;
    };
  }

  private static Json mapItems(Schema of, Json.Arr arr, Mode mode) {
    List<Json> items = new ArrayList<>();
    for (Json item : arr.items()) {
      items.add(prepare(of, item, mode));
    }
    return Json.array(items);
  }

  private static Json mapMembers(Schema of, Json.Obj obj, Mode mode) {
    Map<String, Json> out = new LinkedHashMap<>();
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      out.put(e.getKey(), prepare(of, e.getValue(), mode));
    }
    return new Json.Obj(out);
  }

  private static Json preparePage(Schema s, Json.Obj obj, Mode mode) {
    Map<String, Json> out = new LinkedHashMap<>();
    Json items = obj.get("items");
    if (items != null) {
      out.put("items", prepare(Schema.list(s.of), items, mode));
    }
    for (String key : List.of("total", "page", "size")) {
      if (obj.get(key) != null) {
        out.put(key, obj.get(key));
      }
    }
    return new Json.Obj(out);
  }

  private static Json prepareObject(Schema s, Json.Obj obj, Mode mode) {
    Map<String, Json> out = new LinkedHashMap<>();
    for (Schema.Field f : s.fields) {
      if (mode == Mode.REQUEST && f.readonly()) {
        continue;
      }
      Json x = obj.get(f.wire());
      if (x == null && f.defaultValue() != null) {
        x = f.defaultValue();
      }
      if (x != null) {
        out.put(f.wire(), prepare(f.type(), x, mode));
      }
    }
    return new Json.Obj(out);
  }

  private static Json prepareUnion(Schema s, Json value, Mode mode) {
    Json label = value instanceof Json.Obj obj ? obj.get(s.discriminator) : null;
    Schema arm = label instanceof Json.Str str ? s.armFor(str.value()) : null;
    if (arm == null) {
      return value;
    }
    Json mapped = prepare(arm, value, mode);
    return mapped instanceof Json.Obj m ? m.with(s.discriminator, label) : mapped;
  }

  /** UTC with a {@code Z}, no trailing zeros in the fraction (spec/ir.md section 5.3). */
  static String normalizeDatetime(String v) {
    String s = v;
    if (!s.endsWith("Z")) {
      Instant at = OffsetDateTime.parse(s).toInstant().truncatedTo(ChronoUnit.MILLIS);
      s = at.toString();
    }
    return s.replaceAll("(\\.\\d*?)0+Z$", "$1Z").replaceAll("\\.Z$", "Z");
  }

  /** Canonical bytes (RFC 8785 JSON text) of a wire value. */
  public static String canonicalOf(Schema schema, Json wire) {
    return prepare(schema, wire, Mode.CANONICAL).canonical();
  }

  // -- a request value that lacks its read-only fields ---------------------------------------

  /**
   * The value a generated type decodes in request position: a request body has no read-only
   * fields ({@code spec/language.md} section 7), but the generated record needs them, so each
   * required field that the request did not carry is filled with a placeholder of its shape. A
   * rule that reads such a field is skipped for a request, so a placeholder is never read.
   */
  public static Json fillReadonly(Schema schema, Json json) {
    return fill(schema, json);
  }

  private static Json fill(Schema s, Json v) {
    Schema node = s.resolve();
    switch (node.kind) {
      case LIST -> {
        if (v instanceof Json.Arr arr) {
          List<Json> items = new ArrayList<>();
          for (Json item : arr.items()) {
            items.add(fill(node.of, item));
          }
          return Json.array(items);
        }
      }
      case MAP -> {
        if (v instanceof Json.Obj obj) {
          Map<String, Json> out = new LinkedHashMap<>();
          for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
            out.put(e.getKey(), fill(node.of, e.getValue()));
          }
          return new Json.Obj(out);
        }
      }
      case PAGE -> {
        if (v instanceof Json.Obj obj && obj.get("items") != null) {
          return obj.with("items", fill(Schema.list(node.of), obj.get("items")));
        }
      }
      case OBJECT -> {
        if (v instanceof Json.Obj obj) {
          return fillObject(node, obj);
        }
      }
      case UNION -> {
        Json label = v instanceof Json.Obj obj ? obj.get(node.discriminator) : null;
        Schema arm = label instanceof Json.Str str ? node.armFor(str.value()) : null;
        if (arm != null && fill(arm, v) instanceof Json.Obj filled) {
          return filled.with(node.discriminator, label);
        }
      }
      default -> {
      }
    }
    return v;
  }

  private static Json fillObject(Schema node, Json.Obj obj) {
    Map<String, Json> out = new LinkedHashMap<>();
    for (Schema.Field f : node.fields) {
      Json x = obj.get(f.wire());
      if (x != null) {
        out.put(f.wire(), fill(f.type(), x));
      } else if (!f.optional() && f.defaultValue() == null) {
        out.put(f.wire(), placeholder(f.type()));
      }
    }
    return new Json.Obj(out);
  }

  private static Json placeholder(Schema s) {
    Schema node = s.resolve();
    switch (node.kind) {
      case STRING, BYTES:
        return Json.of("");
      case INT:
        return Json.of(0L);
      case FLOAT:
        return Json.of(0.0);
      case LONG, DECIMAL:
        return Json.of("0");
      case BOOL:
        return Json.of(false);
      case DATE:
        return Json.of("1970-01-01");
      case DATETIME:
        return Json.of("1970-01-01T00:00:00Z");
      case TIME:
        return Json.of("00:00:00");
      case UUID:
        return Json.of("00000000-0000-0000-0000-000000000000");
      case ENUM:
        return Json.of(node.values.isEmpty() ? "" : node.values.get(0));
      case LIST:
        return Json.array();
      case MAP:
        return Json.object().build();
      case PAGE:
        return Json.object()
            .put("items", Json.array())
            .put("total", Json.of("0"))
            .put("page", Json.of(0L))
            .put("size", Json.of(0L))
            .build();
      case OBJECT:
        return fillObject(node, Json.object().build());
      case UNION:
        return placeholderArm(node);
      default:
        return Json.NULL;
    }
  }

  private static Json placeholderArm(Schema node) {
    if (node.arms.isEmpty()) {
      return Json.NULL;
    }
    Schema.Arm first = node.arms.get(0);
    Json body = placeholder(first.schema().get());
    return body instanceof Json.Obj obj ? obj.with(node.discriminator, Json.of(first.label())) : body;
  }
}
