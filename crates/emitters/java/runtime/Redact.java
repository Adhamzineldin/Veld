package dev.veld.runtime;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * What a capture or an audit event may keep of a value (spec/rules.md section 19.3): a wire value
 * with every field marked {@code @pii} or {@code @secret} left out, at any depth (nested models,
 * lists, maps, the {@code items} of a page, the arm of a union), and the wire paths of what was left
 * out. A marked field is removed whole, with whatever it holds, and only its own path is listed; a
 * field the value does not have is not listed; members the declaration does not know are kept.
 *
 * <p>Paths are written from {@code root}: {@code principal.email}, {@code parameters.body.card},
 * {@code result[1].name}, {@code result.items[0].phone}: {@code .name} is a member (the wire name),
 * {@code [2]} a list item.
 */
public final class Redact {
  private Redact() {}

  /**
   * {@code value}, a wire value of {@code schema}, without the marked fields.
   *
   * @param root the start of every path ({@code principal}, {@code parameters.body}, ...)
   * @param paths receives the paths of the fields that were left out
   * @return the value that may be kept
   * @throws IllegalStateException when the value does not have the shape of its schema: a value that
   *     cannot be walked cannot be known to be clean, so nothing is returned
   */
  public static Json value(Schema schema, Json value, String root, List<String> paths) {
    return walk(schema, value, root, paths);
  }

  private static Json walk(Schema schema, Json v, String path, List<String> out) {
    if (v == null || v instanceof Json.Null) {
      return v;
    }
    Schema s = schema.resolve();
    switch (s.kind) {
      case LIST -> {
        return items(s.of, v, path, out);
      }
      case MAP -> {
        if (!(v instanceof Json.Obj obj)) {
          throw mismatch(path, "an object");
        }
        Map<String, Json> kept = new LinkedHashMap<>();
        for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
          kept.put(e.getKey(), walk(s.of, e.getValue(), path + "." + e.getKey(), out));
        }
        return new Json.Obj(kept);
      }
      case PAGE -> {
        if (!(v instanceof Json.Obj obj)) {
          throw mismatch(path, "an object");
        }
        Json listed = obj.get("items");
        return listed == null ? obj : obj.with("items", items(s.of, listed, path + ".items", out));
      }
      case UNION -> {
        if (!(v instanceof Json.Obj obj)) {
          throw mismatch(path, "an object");
        }
        Schema arm = obj.get(s.discriminator) instanceof Json.Str label ? s.armFor(label.value()) : null;
        if (arm == null) {
          throw mismatch(path, "a value that names an arm of the union");
        }
        return walk(arm, v, path, out);
      }
      case OBJECT -> {
        if (!(v instanceof Json.Obj obj)) {
          throw mismatch(path, "an object");
        }
        return record(s, obj, path, out);
      }
      default -> {
        return v;
      }
    }
  }

  private static Json items(Schema item, Json v, String path, List<String> out) {
    if (!(v instanceof Json.Arr arr)) {
      throw mismatch(path, "an array");
    }
    java.util.ArrayList<Json> kept = new java.util.ArrayList<>();
    for (int i = 0; i < arr.items().size(); i++) {
      kept.add(walk(item, arr.items().get(i), path + "[" + i + "]", out));
    }
    return Json.array(kept);
  }

  private static Json record(Schema s, Json.Obj obj, String path, List<String> out) {
    Map<String, Json> kept = new LinkedHashMap<>(obj.fields());
    for (Schema.Field f : s.fields) {
      Json child = obj.get(f.wire());
      if (child == null) {
        continue;
      }
      String here = path + "." + f.wire();
      if (s.marks != null && s.marks.pii.contains(f.wire())) {
        kept.remove(f.wire());
        out.add(here);
      } else {
        kept.put(f.wire(), walk(f.type(), child, here, out));
      }
    }
    return new Json.Obj(kept);
  }

  private static IllegalStateException mismatch(String path, String expected) {
    return new IllegalStateException("the value at `" + path + "` is not " + expected + " as its type says");
  }
}
