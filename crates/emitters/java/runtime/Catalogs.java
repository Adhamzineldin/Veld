package dev.veld.runtime;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;
// #if messages_server
import java.util.function.Supplier;
// #endif

/**
 * Message catalogs (spec/platform.md section 22): the language of a request, chosen from {@code
 * Accept-Language} as {@code veld_rules::messages} does, and the text of a message in it. A client
 * uses {@link #chooseLanguage} and {@link #text} with the catalogs the generator wrote for it; a
 * server with catalogs also answers in the language of the request.
 */
public final class Catalogs {
  private Catalogs() {}

  /** A language range: a primary language of one to eight letters, then subtags of letters or digits. */
  private static final Pattern RANGE = Pattern.compile("[a-z]{1,8}(?:-[a-z0-9]{1,8})*");

  /** The numbers {@code q} may be written as: digits, a point, an exponent; no name, no suffix. */
  private static final Pattern QUALITY =
      Pattern.compile("[+-]?(?:[0-9]+\\.?[0-9]*|\\.[0-9]+)(?:[eE][+-]?[0-9]+)?");

  /** ASCII lower-casing only: a letter outside {@code A} to {@code Z} (the Kelvin sign, a dotted capital I) stays as it is. */
  private static String lower(String text) {
    StringBuilder out = new StringBuilder(text.length());
    for (int i = 0; i < text.length(); i++) {
      char c = text.charAt(i);
      out.append(c >= 'A' && c <= 'Z' ? (char) (c + 32) : c);
    }
    return out.toString();
  }

  /** The characters {@code char::is_whitespace} of the reference trims (the Unicode White_Space property). */
  private static boolean space(char c) {
    return (c >= 0x09 && c <= 0x0D)
        || c == 0x20
        || c == 0x85
        || c == 0xA0
        || c == 0x1680
        || (c >= 0x2000 && c <= 0x200A)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202F
        || c == 0x205F
        || c == 0x3000;
  }

  private static String trim(String text) {
    int from = 0;
    int to = text.length();
    while (from < to && space(text.charAt(from))) {
      from++;
    }
    while (to > from && space(text.charAt(to - 1))) {
      to--;
    }
    return text.substring(from, to);
  }

  /**
   * The {@code q} of the parameters of one entry (after the tag): 1 when there is none, {@code NaN}
   * when it is not a number from 0 to 1.
   */
  private static double quality(String[] parts) {
    for (int i = 1; i < parts.length; i++) {
      String p = trim(parts[i]);
      int at = p.indexOf('=');
      if (at < 0 || !lower(trim(p.substring(0, at))).equals("q")) {
        continue;
      }
      String value = trim(p.substring(at + 1));
      if (!QUALITY.matcher(value).matches()) {
        return Double.NaN;
      }
      double q = Double.parseDouble(value);
      return q >= 0.0 && q <= 1.0 ? q : Double.NaN;
    }
    return 1.0;
  }

  private record Entry(double quality, String tag) {}

  /**
   * The language tags of an {@code Accept-Language} value, lower case, best first. An entry is
   * dropped when its tag is {@code *}, when its quality is {@code 0} or is not a number from 0 to 1,
   * or when its tag is not a language range; the rest keep the header's order among entries of one
   * quality.
   */
  public static List<String> acceptedLanguages(String header) {
    List<Entry> entries = new ArrayList<>();
    for (String item : header.split(",", -1)) {
      String[] parts = item.split(";", -1);
      String tag = lower(trim(parts[0]));
      double q = quality(parts);
      if (q > 0.0 && RANGE.matcher(tag).matches()) {
        entries.add(new Entry(q, tag));
      }
    }
    entries.sort((a, b) -> Double.compare(b.quality, a.quality));
    List<String> out = new ArrayList<>();
    for (Entry e : entries) {
      out.add(e.tag);
    }
    return out;
  }

  /**
   * The first language of {@code header}, in order of preference, that {@code available} has a
   * catalog for. A tag matches the catalog of the same tag and then the one of its primary language
   * ({@code pt-br} matches {@code pt-br}, then {@code pt}); {@code pt} does not match a catalog
   * {@code pt-br}. {@code null} for no header, a header with nothing usable and a header that
   * matches nothing: the contract's text is used.
   */
  public static String chooseLanguage(String header, Collection<String> available) {
    if (header == null) {
      return null;
    }
    for (String tag : acceptedLanguages(header)) {
      int dash = tag.indexOf('-');
      String primary = dash < 0 ? tag : tag.substring(0, dash);
      if (available.contains(tag)) {
        return tag;
      }
      if (available.contains(primary)) {
        return primary;
      }
    }
    return null;
  }

  /**
   * A message as an answer carries it.
   *
   * @param text the catalog's text, or the fallback
   * @param language the language of the catalog the text came from, or {@code null} when the
   *     fallback is used
   */
  public record Text(String text, String language) {}

  /**
   * The text of {@code key} in the catalog of {@code language}, and that language; the {@code
   * fallback} and no language when there is no language or its catalog lacks the key.
   */
  public static Text text(Map<String, Map<String, String>> catalogs, String language, String key, String fallback) {
    Map<String, String> catalog = language == null ? null : catalogs.get(language);
    String found = catalog == null ? null : catalog.get(key);
    return found == null ? new Text(fallback, null) : new Text(found, language);
  }
  // #if messages_rules

  /** The violations of a client's local check with the {@code messageKey} of their rule. */
  static void keyLocal(Map<String, String> keys, VeldError err) {
    if (keys.isEmpty() || err.violations().isEmpty()) {
      return;
    }
    List<Violation> out = new ArrayList<>();
    for (Violation v : err.violations()) {
      String key = v.messageKey() == null ? keys.get(v.rule()) : null;
      out.add(key == null ? v : v.withKey(key));
    }
    err.setViolations(out);
  }
  // #endif
  // #if messages_server

  /** The key of a {@code transition} that is not a legal move ({@code veld_ir::message_keys::TRANSITION}). */
  static final String TRANSITION = "veld/transition";

  /** The key of a timed move before its time. */
  static final String TRANSITION_NOT_DUE = "veld/transition-not-due";

  /** The key of an {@code If-Match} that is not the current version. */
  static final String VERSION = "veld/version";

  /** The key of a denial that no {@code forbid} with a message explains. */
  static final String NOT_PERMITTED = "veld/not-permitted";

  /** What a service says about its messages: the catalogs and the key of every rule id. */
  public static final class Support {
    final Map<String, Map<String, String>> catalogs;
    final Map<String, String> keys;

    Support(Map<String, Map<String, String>> catalogs, Map<String, String> keys) {
      this.catalogs = catalogs;
      this.keys = keys;
    }
  }

  /**
   * The messages of a service.
   *
   * @param catalogs language tag, as the file is named, then message key, then text
   * @param keys the {@code messageKey} of each rule id
   */
  public static Support support(Map<String, Map<String, String>> catalogs, Map<String, String> keys) {
    return new Support(catalogs, keys);
  }

  /** The language of the request that is running, and the language of the catalog a message of its answer came from. */
  private static final class State {
    final Support support;
    final String language;
    String used;

    State(Support support, String language) {
      this.support = support;
      this.language = language;
    }
  }

  private static final ThreadLocal<State> CURRENT = new ThreadLocal<>();

  /** {@code Accept-Language} of the request: a header sent twice is joined with a comma. */
  private static String acceptLanguage(VeldRequest request) {
    StringBuilder out = null;
    for (Map.Entry<String, List<String>> e : request.headers().entrySet()) {
      if (e.getKey().equalsIgnoreCase("accept-language")) {
        for (String value : e.getValue()) {
          out = out == null ? new StringBuilder(value) : out.append(',').append(value);
        }
      }
    }
    return out == null ? null : out.toString();
  }

  /**
   * Runs the pipeline of one request with its language in force on this thread: {@code
   * Accept-Language} is read once, and the answer has {@code Content-Language} when a message of it
   * came from a catalog.
   */
  static VeldResponse within(HandlerHooks hooks, VeldRequest request, Supplier<VeldResponse> body) {
    Support support = hooks.messages();
    if (support == null) {
      return body.get();
    }
    State state = new State(support, chooseLanguage(acceptLanguage(request), support.catalogs.keySet()));
    State before = CURRENT.get();
    CURRENT.set(state);
    try {
      VeldResponse response = body.get();
      // veld:sse
      if (response.frames() != null) {
        return response;
      }
      // veld:end
      if (state.used == null) {
        return response;
      }
      java.util.Map<String, String> headers = new java.util.LinkedHashMap<>(response.headers());
      headers.put("content-language", state.used);
      return new VeldResponse(response.status(), headers, response.body());
    } finally {
      if (before == null) {
        CURRENT.remove();
      } else {
        CURRENT.set(before);
      }
    }
  }

  /** The violation with its {@code messageKey} and its text as the contract wrote it: what the hook of the application hears. */
  static Violation keyed(HandlerHooks hooks, Violation v) {
    Support support = hooks.messages();
    if (support == null || v.messageKey() != null) {
      return v;
    }
    String key = support.keys.get(v.rule());
    return key == null ? v : v.withKey(key);
  }

  /**
   * The violations as the answer carries them: each with its {@code messageKey} (the one its
   * producer named, else the one of its rule id), and its text in the language of the request when
   * the catalog of that language has the key. An evaluation error is not translated.
   */
  static List<Violation> answer(List<Violation> found) {
    State state = CURRENT.get();
    if (state == null) {
      return found;
    }
    List<Violation> out = new ArrayList<>();
    for (Violation v : found) {
      String key = v.messageKey() != null ? v.messageKey() : state.support.keys.get(v.rule());
      if (key == null) {
        out.add(v);
      } else if (v.error() != null || state.language == null) {
        out.add(v.withKey(key));
      } else {
        Text shown = text(state.support.catalogs, state.language, key, v.message());
        if (shown.language() != null) {
          state.used = shown.language();
        }
        out.add(new Violation(v.rule(), v.path(), v.fields(), shown.text(), v.error(), key));
      }
    }
    return out;
  }

  /** The violations with their {@code messageKey} and no translation (a frame of a WebSocket has no language). */
  static List<Violation> keyed(HandlerHooks hooks, List<Violation> found) {
    List<Violation> out = new ArrayList<>();
    for (Violation v : found) {
      out.add(keyed(hooks, v));
    }
    return out;
  }
  // #endif
}
