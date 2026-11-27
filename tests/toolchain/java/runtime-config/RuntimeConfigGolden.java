package veldtest;

import gen.k.server.runtime.Json;
import gen.k.server.runtime.VeldConfigException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;

/**
 * The golden driver of the typed runtime config (tests/wire/runtime-config/cases.json).
 *
 * <p>Reads a file with one JSON case per line: {@code id}, {@code class} (the generated
 * {@code <Target>Runtime}), {@code from} ({@code value}, {@code map} or {@code env}), {@code input},
 * {@code names}, {@code env} and {@code getters} (the method that reads each path). Prints one JSON
 * line per case: {@code id} and either {@code values} (every path that has a value, as text) or
 * {@code error} (the text of the exception) and {@code problems}.
 */
public final class RuntimeConfigGolden {
  private RuntimeConfigGolden() {}

  /** A JSON value as the plain Java value a JSON or YAML parser would give. */
  private static Object plain(Json json) {
    if (json == null || json instanceof Json.Null) {
      return null;
    }
    if (json instanceof Json.Str s) {
      return s.value();
    }
    if (json instanceof Json.Bool b) {
      return b.value();
    }
    if (json instanceof Json.Num n) {
      return n.value();
    }
    if (json instanceof Json.Arr a) {
      List<Object> items = new ArrayList<>();
      for (Json item : a.items()) {
        items.add(plain(item));
      }
      return items;
    }
    Map<String, Object> map = new LinkedHashMap<>();
    for (Map.Entry<String, Json> e : ((Json.Obj) json).fields().entrySet()) {
      map.put(e.getKey(), plain(e.getValue()));
    }
    return map;
  }

  private static Map<String, String> strings(Json json) {
    Map<String, String> out = new TreeMap<>();
    if (json instanceof Json.Obj o) {
      for (Map.Entry<String, Json> e : o.fields().entrySet()) {
        out.put(e.getKey(), ((Json.Str) e.getValue()).value());
      }
    }
    return out;
  }

  private static Object build(Class<?> type, Json.Obj c) throws Exception {
    return switch (((Json.Str) c.get("from")).value()) {
      case "value" -> type.getMethod("fromValue", Object.class).invoke(null, plain(c.get("input")));
      case "map" -> type.getMethod("fromMap", Map.class).invoke(null, strings(c.get("input")));
      default ->
          type.getMethod("fromEnv", Map.class, Map.class)
              .invoke(null, strings(c.get("names")), strings(c.get("env")));
    };
  }

  private static Json run(Json.Obj c) throws Exception {
    Class<?> type = Class.forName(((Json.Str) c.get("class")).value());
    Object runtime;
    try {
      runtime = build(type, c);
    } catch (InvocationTargetException e) {
      if (!(e.getCause() instanceof VeldConfigException failure)) {
        throw e;
      }
      List<Json> problems = new ArrayList<>();
      for (VeldConfigException.Problem p : failure.problems()) {
        problems.add(
            Json.object().put("path", Json.of(p.path())).put("message", Json.of(p.message())).build());
      }
      return Json.object()
          .put("error", Json.of(failure.getMessage()))
          .put("problems", Json.array(problems))
          .build();
    }
    Json.Builder values = Json.object();
    for (Map.Entry<String, Json> g : ((Json.Obj) c.get("getters")).fields().entrySet()) {
      Method getter = type.getMethod(((Json.Str) g.getValue()).value());
      Object value = getter.invoke(runtime);
      if (value instanceof Enum<?> e) {
        value = e.name().toLowerCase(Locale.ROOT);
      }
      values.put(g.getKey(), value == null ? null : Json.of(String.valueOf(value)));
    }
    return Json.object().put("values", values.build()).build();
  }

  public static void main(String[] args) throws Exception {
    for (String line : Files.readAllLines(Path.of(args[0]), StandardCharsets.UTF_8)) {
      if (line.isBlank()) {
        continue;
      }
      Json.Obj c = Json.parse(line).asObject();
      Json out;
      try {
        out = run(c);
      } catch (Exception e) {
        out = Json.object().put("failure", Json.of(String.valueOf(e))).build();
      }
      System.out.println(((Json.Obj) out).with("id", c.get("id")).write());
    }
  }
}
