package veldtest;

import gen.v.runtime.Derive;
import gen.v.runtime.Json;
import gen.v.runtime.RuleWalk;
import gen.v.runtime.Schema;
import gen.v.runtime.Violation;
import gen.v.runtime.Wire;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Runs the shared vectors of derived fields ({@code tests/rules/core/derived.json.pending}) against
 * generated code. Reads a file with one JSON case per line ({@code id}, {@code type}, {@code value},
 * {@code request}) and a tab-separated file of type name and class name; prints one JSON line per
 * case with {@code violations} (the derived fields' and the invariants', through
 * {@code RuleWalk.checkWire}), {@code computed} (the value with its defaults applied and its derived
 * fields computed, through {@code Derive.apply}) and, for a class that has the public function and a
 * value in response position, {@code viaPublic} (the same value through the generated
 * {@code derive(value)}).
 */
public final class DerivedVectors {
  private DerivedVectors() {}

  public static void main(String[] args) throws Exception {
    Map<String, String> types = new HashMap<>();
    for (String line : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
      String[] parts = line.split("\t");
      if (parts.length == 2) {
        types.put(parts[0], parts[1]);
      }
    }
    for (String line : Files.readAllLines(Path.of(args[0]), StandardCharsets.UTF_8)) {
      if (line.isBlank()) {
        continue;
      }
      Json.Obj c = Json.parse(line).asObject();
      long id = (long) c.get("id").asDouble();
      Json out;
      try {
        out = run(types, c);
      } catch (Exception | LinkageError e) {
        out = Json.object().put("error", Json.of(String.valueOf(e))).build();
      }
      System.out.println(((Json.Obj) out).with("id", Json.of(id)).write());
    }
  }

  private static Json run(Map<String, String> types, Json.Obj c) throws Exception {
    String fqn = types.get(c.get("type").asString());
    if (fqn == null) {
      throw new IllegalArgumentException("unknown type " + c.get("type").asString());
    }
    Class<?> cls = Class.forName(fqn);
    Schema schema = (Schema) cls.getField("SCHEMA").get(null);
    boolean request = c.get("request") != null && c.get("request").asBoolean();
    Json value = c.get("value");
    var issues = Wire.validate(schema, value, "$", request);
    if (!issues.isEmpty()) {
      throw new IllegalArgumentException("the value is not valid: " + issues);
    }
    List<Json> found = new ArrayList<>();
    for (Violation v : RuleWalk.checkWire(schema, value, request)) {
      found.add(v.toWire());
    }
    Json prepared = Wire.prepare(schema, value, request ? Wire.Mode.REQUEST : Wire.Mode.RESPONSE);
    Json computed = Derive.apply(schema, prepared, "$", request, new ArrayList<>());
    Json.Builder out = Json.object().put("violations", Json.array(found)).put("computed", computed);
    Json viaPublic = viaPublic(cls, schema, prepared, request);
    if (viaPublic != null) {
      out.put("viaPublic", viaPublic);
    }
    return out.build();
  }

  /**
   * The value as the public function of the generated class gives it: the typed value is read from
   * the value without its derived fields (what a form holds), then {@code derive} is called.
   */
  private static Json viaPublic(Class<?> cls, Schema schema, Json prepared, boolean request) throws Exception {
    if (request) {
      return null;
    }
    Method derive;
    try {
      derive = cls.getMethod("derive", cls);
    } catch (NoSuchMethodException e) {
      return null;
    }
    Json bare = Derive.strip(schema, prepared);
    Object typed = cls.getMethod("fromWire", Json.class).invoke(null, bare);
    Object derived = derive.invoke(null, typed);
    return (Json) cls.getMethod("toWire").invoke(derived);
  }
}
