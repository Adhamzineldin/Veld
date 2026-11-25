package dev.veld.runtime;

import java.util.ArrayList;
import java.util.List;

/**
 * One violated rule on one value (spec/rules.md section 7).
 *
 * @param rule the rule id, {@code veld:<package>/<Decl>.<rule>@<hash>}
 * @param path the path of the value that carries the rule, in the syntax of validation issues
 * @param fields the wire names of the fields the rule reads, in order of first use
 * @param message the rule's message, or the default message
 * @param error {@code overflow}, {@code division_by_zero} or {@code non_finite} for an evaluation
 *     error, otherwise {@code null}
 */
public record Violation(String rule, String path, List<String> fields, String message, String error) {
  /** Creates a violation (the field list is copied). */
  public Violation {
    fields = List.copyOf(fields);
  }

  /** The wire form: the entry of the {@code violations} member of a problem document. */
  public Json toWire() {
    List<Json> names = new ArrayList<>();
    for (String f : fields) {
      names.add(Json.of(f));
    }
    return Json.object()
        .put("rule", Json.of(rule))
        .put("path", Json.of(path))
        .put("fields", Json.array(names))
        .put("message", Json.of(message))
        .put("error", error == null ? null : Json.of(error))
        .build();
  }

  /** Reads the wire form; a member that is missing or has the wrong type becomes empty. */
  public static Violation fromWire(Json json) {
    Json.Obj o = json.asObject();
    List<String> names = new ArrayList<>();
    if (o.get("fields") instanceof Json.Arr arr) {
      for (Json f : arr.items()) {
        names.add(f.asString());
      }
    }
    return new Violation(
        text(o, "rule"), text(o, "path"), names, text(o, "message"),
        o.get("error") instanceof Json.Str s ? s.value() : null);
  }

  private static String text(Json.Obj o, String key) {
    return o.get(key) instanceof Json.Str s ? s.value() : "";
  }
}
