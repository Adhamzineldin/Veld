package dev.veld.runtime;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** A strict RFC 8259 parser. */
final class JsonParser {
  private static final int MAX_DEPTH = 256;

  private final String text;
  private int pos;

  private JsonParser(String text) {
    this.text = text;
  }

  static Json parse(String text) {
    JsonParser p = new JsonParser(text);
    p.skipWhitespace();
    Json value = p.value(0);
    p.skipWhitespace();
    if (p.pos != text.length()) {
      throw p.error("unexpected trailing content");
    }
    return value;
  }

  private Json.ParseException error(String message) {
    return new Json.ParseException(message + " at offset " + pos);
  }

  private void skipWhitespace() {
    while (pos < text.length()) {
      char c = text.charAt(pos);
      if (c != ' ' && c != '\t' && c != '\n' && c != '\r') {
        return;
      }
      pos++;
    }
  }

  private Json value(int depth) {
    if (depth > MAX_DEPTH) {
      throw error("nesting is too deep");
    }
    if (pos >= text.length()) {
      throw error("unexpected end of input");
    }
    char c = text.charAt(pos);
    switch (c) {
      case '{':
        return object(depth);
      case '[':
        return array(depth);
      case '"':
        return new Json.Str(string());
      case 't':
        literal("true");
        return new Json.Bool(true);
      case 'f':
        literal("false");
        return new Json.Bool(false);
      case 'n':
        literal("null");
        return Json.NULL;
      default:
        return number();
    }
  }

  private void literal(String word) {
    if (!text.startsWith(word, pos)) {
      throw error("invalid literal");
    }
    pos += word.length();
  }

  private Json object(int depth) {
    pos++;
    Map<String, Json> members = new LinkedHashMap<>();
    skipWhitespace();
    if (peek() == '}') {
      pos++;
      return new Json.Obj(members);
    }
    while (true) {
      skipWhitespace();
      if (peek() != '"') {
        throw error("expected a string key");
      }
      String key = string();
      skipWhitespace();
      expect(':');
      skipWhitespace();
      members.put(key, value(depth + 1));
      skipWhitespace();
      char c = next();
      if (c == '}') {
        return new Json.Obj(members);
      }
      if (c != ',') {
        pos--;
        throw error("expected ',' or '}'");
      }
    }
  }

  private Json array(int depth) {
    pos++;
    List<Json> items = new ArrayList<>();
    skipWhitespace();
    if (peek() == ']') {
      pos++;
      return new Json.Arr(items);
    }
    while (true) {
      skipWhitespace();
      items.add(value(depth + 1));
      skipWhitespace();
      char c = next();
      if (c == ']') {
        return new Json.Arr(items);
      }
      if (c != ',') {
        pos--;
        throw error("expected ',' or ']'");
      }
    }
  }

  private char peek() {
    return pos < text.length() ? text.charAt(pos) : '\0';
  }

  private char next() {
    if (pos >= text.length()) {
      throw error("unexpected end of input");
    }
    return text.charAt(pos++);
  }

  private void expect(char c) {
    if (next() != c) {
      pos--;
      throw error("expected '" + c + "'");
    }
  }

  private String string() {
    pos++;
    StringBuilder out = new StringBuilder();
    while (true) {
      char c = next();
      if (c == '"') {
        return out.toString();
      }
      if (c < 0x20) {
        pos--;
        throw error("control character in string");
      }
      if (c == '\\') {
        escape(out);
      } else {
        out.append(c);
      }
    }
  }

  private void escape(StringBuilder out) {
    char c = next();
    switch (c) {
      case '"', '\\', '/' -> out.append(c);
      case 'b' -> out.append('\b');
      case 'f' -> out.append('\f');
      case 'n' -> out.append('\n');
      case 'r' -> out.append('\r');
      case 't' -> out.append('\t');
      case 'u' -> out.append(unicode());
      default -> {
        pos--;
        throw error("invalid escape");
      }
    }
  }

  private char unicode() {
    if (pos + 4 > text.length()) {
      throw error("truncated unicode escape");
    }
    int code = 0;
    for (int i = 0; i < 4; i++) {
      int digit = Character.digit(text.charAt(pos + i), 16);
      if (digit < 0) {
        throw error("invalid unicode escape");
      }
      code = code * 16 + digit;
    }
    pos += 4;
    return (char) code;
  }

  private Json number() {
    int start = pos;
    if (peek() == '-') {
      pos++;
    }
    if (peek() == '0') {
      pos++;
    } else if (!digits()) {
      pos = start;
      throw error("invalid value");
    }
    if (peek() == '.') {
      pos++;
      if (!digits()) {
        throw error("digits expected after '.'");
      }
    }
    if (peek() == 'e' || peek() == 'E') {
      pos++;
      if (peek() == '+' || peek() == '-') {
        pos++;
      }
      if (!digits()) {
        throw error("digits expected in exponent");
      }
    }
    double value = Double.parseDouble(text.substring(start, pos));
    if (Double.isInfinite(value)) {
      throw error("number out of range");
    }
    return new Json.Num(value);
  }

  private boolean digits() {
    int start = pos;
    while (pos < text.length() && text.charAt(pos) >= '0' && text.charAt(pos) <= '9') {
      pos++;
    }
    return pos > start;
  }
}
