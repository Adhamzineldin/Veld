package veldtest;

import gen.v.runtime.HandlerHooks;
import gen.v.runtime.Json;
import gen.v.runtime.RuleWalk;
import gen.v.runtime.Schema;
import gen.v.runtime.VeldHandler;
import gen.v.runtime.VeldRequest;
import gen.v.runtime.VeldResponse;
import gen.v.runtime.Violation;
import gen.v.runtime.Wire;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * The vector driver of the Java server (spec/rules.md section 15), the counterpart of
 * {@code RuleVectors} for a target that evaluates rules on the server and reports to
 * {@code onRuleViolation}.
 *
 * <p>Reads a file with one JSON case per line ({@code id}, {@code type}, {@code value},
 * {@code request}, {@code output}). Every case runs through the generated handler of the service
 * {@code Vectors}: a case in request position is posted to {@code /v/in/<Type>} (step 5), any other
 * case is the result of {@code GET /v/out/<Type>} (step 8), under the output mode the case names
 * ({@code "output": "off"}) or {@code STRICT}. Prints one JSON line per case: {@code id},
 * {@code status}, {@code violations} (the events with the outcome {@code REJECTED}) and
 * {@code audited} (those with the outcome {@code AUDITED}), each the violation object of spec/rules.md
 * section 7 with the path of the value ({@code $}, {@code $.rows[1]}).
 */
public final class RuleVectorsServer {
  private RuleVectorsServer() {}

  private static Json current;
  private static String failure = "";
  private static final java.util.Map<String, String> TYPES = new java.util.HashMap<>();

  private static VeldHandler handler(List<RuleWalk.Event> events, RuleWalk.OutputMode mode) throws Exception {
    Class<?> service = Class.forName("gen.v.t.server.VectorsService");
    Class<?> hooksClass = Class.forName("gen.v.t.server.VectorsHooks");
    Object hooks = hooksClass.getConstructor().newInstance();
    hooks = hooksClass.getMethod("rulesOutput", RuleWalk.OutputMode.class).invoke(hooks, mode);
    hooks = hooksClass.getMethod("onRuleViolation", RuleWalk.Hook.class).invoke(hooks, (RuleWalk.Hook) events::add);
    hooks = hooksClass.getMethod("onError", HandlerHooks.ErrorHook.class).invoke(hooks, (HandlerHooks.ErrorHook) (e, ctx) -> failure = String.valueOf(e));
    Object impl = Proxy.newProxyInstance(
        service.getClassLoader(),
        new Class<?>[] {service},
        (proxy, method, args) -> method.getName().startsWith("out") ? decode(method.getName().substring(3)) : null);
    Class<?> handlerClass = Class.forName("gen.v.t.server.VectorsHandler");
    return (VeldHandler) handlerClass.getMethod("create", service, hooksClass).invoke(null, impl, hooks);
  }

  /** The value of the case as the generated type {@code type}. */
  private static Object decode(String type) throws Exception {
    Class<?> cls = Class.forName(TYPES.get(type));
    Schema schema = (Schema) cls.getField("SCHEMA").get(null);
    Json prepared = Wire.prepare(schema, current, Wire.Mode.RESPONSE);
    return cls.getMethod("fromWire", Json.class).invoke(null, prepared);
  }

  /** The path of the value that carries the rule: {@code body.rows[1]} and {@code result.rows[1]} are {@code $.rows[1]}. */
  private static String valuePath(String path) {
    for (String root : List.of("body", "result")) {
      if (path.equals(root) || path.startsWith(root + ".") || path.startsWith(root + "[")) {
        return "$" + path.substring(root.length());
      }
    }
    return path;
  }

  private static Json reported(List<RuleWalk.Event> events, RuleWalk.Outcome outcome) {
    List<Json> out = new ArrayList<>();
    for (RuleWalk.Event e : events) {
      if (e.outcome() == outcome) {
        Violation v = e.violation();
        out.add(new Violation(v.rule(), valuePath(v.path()), v.fields(), v.message(), v.error()).toWire());
      }
    }
    return Json.array(out);
  }

  private static Json run(Json.Obj c) throws Exception {
    List<RuleWalk.Event> events = Collections.synchronizedList(new ArrayList<>());
    boolean off = c.get("output") instanceof Json.Str mode && mode.value().equals("off");
    VeldHandler handler = handler(events, off ? RuleWalk.OutputMode.OFF : RuleWalk.OutputMode.STRICT);
    String type = c.get("type").asString();
    boolean request = c.get("request") != null && c.get("request").asBoolean();
    current = c.get("value");
    VeldRequest in = request
        ? new VeldRequest(
            "POST", "/v/in/" + type, "", Map.of("content-type", List.of("application/json")), current.write(), null)
        : new VeldRequest("GET", "/v/out/" + type, "", Map.of(), "", null);
    Optional<VeldResponse> answer = handler.handle(in);
    if (answer.isEmpty()) {
      throw new IllegalStateException("no route");
    }
    return Json.object()
        .put("status", Json.of((long) answer.get().status()))
        .put("body", Json.of(failure + " " + (answer.get().body() == null ? "" : answer.get().body())))
        .put("violations", reported(events, RuleWalk.Outcome.REJECTED))
        .put("audited", reported(events, RuleWalk.Outcome.AUDITED))
        .build();
  }

  public static void main(String[] args) throws Exception {
    for (String row : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
      String[] parts = row.split("	");
      if (parts.length == 2) {
        TYPES.put(parts[0], parts[1]);
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
        out = run(c);
      } catch (Exception | LinkageError e) {
        out = Json.object().put("error", Json.of(String.valueOf(e))).build();
      }
      System.out.println(((Json.Obj) out).with("id", Json.of(id)).write());
    }
  }
}
