package veldtest;

import gen.v.runtime.Json;
import gen.v.runtime.RuleWalk;
import gen.v.runtime.Schema;
import gen.v.runtime.Violation;
import gen.v.runtime.Wire;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Runs the shared rule vectors (tests/rules) against generated code. Reads a file with one JSON
 * case per line ({"id", "type", "value", "request"}) and a tab-separated file of type name and
 * class name; prints one JSON line per case with the violations produced.
 */
public final class RuleVectors {
  private RuleVectors() {}

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
    Schema schema = (Schema) Class.forName(fqn).getField("SCHEMA").get(null);
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
    return Json.object().put("violations", Json.array(found)).build();
  }
}
