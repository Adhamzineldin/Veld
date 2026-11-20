package dev.veld.runtime;

import java.math.BigDecimal;
import java.math.MathContext;
import java.math.RoundingMode;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** Writes JSON text; with {@code sorted} the output is RFC 8785 canonical. */
final class JsonWriter {
  private JsonWriter() {}

  static void write(Json value, StringBuilder out, boolean sorted) {
    if (value instanceof Json.Null) {
      out.append("null");
    } else if (value instanceof Json.Bool b) {
      out.append(b.value() ? "true" : "false");
    } else if (value instanceof Json.Num n) {
      if (Double.isNaN(n.value()) || Double.isInfinite(n.value())) {
        throw new IllegalArgumentException("canonical: non-finite number");
      }
      out.append(esNumber(n.value()));
    } else if (value instanceof Json.Str s) {
      writeString(s.value(), out);
    } else if (value instanceof Json.Arr a) {
      writeArray(a, out, sorted);
    } else if (value instanceof Json.Obj o) {
      writeObject(o, out, sorted);
    }
  }

  private static void writeArray(Json.Arr array, StringBuilder out, boolean sorted) {
    out.append('[');
    boolean first = true;
    for (Json item : array.items()) {
      if (!first) {
        out.append(',');
      }
      first = false;
      write(item, out, sorted);
    }
    out.append(']');
  }

  private static void writeObject(Json.Obj object, StringBuilder out, boolean sorted) {
    List<Map.Entry<String, Json>> entries = new ArrayList<>(object.fields().entrySet());
    if (sorted) {
      entries.sort(Map.Entry.comparingByKey());
    }
    out.append('{');
    boolean first = true;
    for (Map.Entry<String, Json> e : entries) {
      if (!first) {
        out.append(',');
      }
      first = false;
      writeString(e.getKey(), out);
      out.append(':');
      write(e.getValue(), out, sorted);
    }
    out.append('}');
  }

  static void writeString(String s, StringBuilder out) {
    out.append('"');
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      switch (c) {
        case '"' -> out.append("\\\"");
        case '\\' -> out.append("\\\\");
        case '\b' -> out.append("\\b");
        case '\f' -> out.append("\\f");
        case '\n' -> out.append("\\n");
        case '\r' -> out.append("\\r");
        case '\t' -> out.append("\\t");
        default -> {
          if (c < 0x20 || isLoneSurrogate(s, i)) {
            out.append(String.format("\\u%04x", (int) c));
          } else {
            out.append(c);
          }
        }
      }
    }
    out.append('"');
  }

  private static boolean isLoneSurrogate(String s, int i) {
    char c = s.charAt(i);
    if (Character.isHighSurrogate(c)) {
      return i + 1 >= s.length() || !Character.isLowSurrogate(s.charAt(i + 1));
    }
    if (Character.isLowSurrogate(c)) {
      return i == 0 || !Character.isHighSurrogate(s.charAt(i - 1));
    }
    return false;
  }

  /** ECMAScript {@code Number::toString(10)} for a finite double ({@code -0} prints as {@code 0}). */
  static String esNumber(double v) {
    if (v == 0.0) {
      return "0";
    }
    double abs = Math.abs(v);
    BigDecimal shortest = shortestDigits(abs).stripTrailingZeros();
    String digits = shortest.unscaledValue().toString();
    int k = digits.length();
    int n = k - shortest.scale();
    String body;
    if (k <= n && n <= 21) {
      body = digits + "0".repeat(n - k);
    } else if (0 < n && n <= 21) {
      body = digits.substring(0, n) + "." + digits.substring(n);
    } else if (-6 < n && n <= 0) {
      body = "0." + "0".repeat(-n) + digits;
    } else {
      int e = n - 1;
      String sign = e < 0 ? "-" : "+";
      String mantissa = k == 1 ? digits : digits.charAt(0) + "." + digits.substring(1);
      body = mantissa + "e" + sign + Math.abs(e);
    }
    return v < 0 ? "-" + body : body;
  }

  /** The shortest decimal that parses back to exactly {@code abs}. */
  private static BigDecimal shortestDigits(double abs) {
    BigDecimal exact = new BigDecimal(abs);
    for (int precision = 1; precision < 17; precision++) {
      BigDecimal r = exact.round(new MathContext(precision, RoundingMode.HALF_EVEN));
      if (r.doubleValue() == abs) {
        return r;
      }
    }
    return exact.round(new MathContext(17, RoundingMode.HALF_EVEN));
  }
}
