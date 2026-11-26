package dev.veld.runtime;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Derived fields (spec/rules.md section 17.2): a field whose {@code =} is followed by an
 * expression is computed by the server, never supplied. This class walks a prepared wire value by
 * its {@link Schema}, in the order of the specification: the value a caller or a handler put in a
 * derived field is dropped, the values nested in the object (lists, maps, pages, union arms, other
 * objects) are computed first, then the derived fields of the object itself, in dependency order,
 * by the function the generated declaration registered with {@link Schema#derived}.
 *
 * <p>An expression that raises an evaluation error leaves its field absent and records one
 * violation whose rule is the id of the field. An expression that reads a field that is absent
 * (a derived field that was left absent, or a field of a request that the request cannot carry) is
 * skipped without a violation: the cause is already reported or is the position of the value.
 */
public final class Derive {
  private Derive() {}

  /** The derived fields of one declaration, as the generated code computes them. */
  @FunctionalInterface
  public interface Compute {
    /**
     * Returns {@code json} with the derived members that could be computed.
     *
     * @param json the object, prepared, with every derived member dropped and the values inside it
     *     already complete
     * @param request whether the value is in request position
     * @param path the path of the object
     * @param out receives the violation of every derived field that failed
     */
    Json.Obj run(Json.Obj json, boolean request, String path, List<Violation> out);
  }

  /** The expression of one derived field, as the wire value it computes. */
  @FunctionalInterface
  public interface Value {
    /** Evaluates the expression; may raise {@link RuleError}. */
    Json get();
  }

  private static final Map<Schema, Boolean> REACHES = new ConcurrentHashMap<>();
  private static final ThreadLocal<List<Violation>> PENDING = new ThreadLocal<>();

  /**
   * Evaluates the expression of one derived field. An evaluation error is recorded as a violation
   * (rule: the id of the field) and gives {@code null}; so does a read of an absent value.
   */
  public static Json run(
      List<Violation> out, String id, String name, String path, List<String> fields, Value value) {
    try {
      return value.get();
    } catch (RuleError e) {
      out.add(new Violation(id, path, fields, "field " + name + " could not be computed", e.kind()));
      return null;
    } catch (NullPointerException e) {
      // The expression read a derived field that is absent: it is skipped, the cause is reported.
      return null;
    }
  }

  // -- the walk -------------------------------------------------------------------------------

  /**
   * Computes the derived fields of a prepared wire value (defaults applied), nested values first.
   * Paths start at {@code path}.
   */
  public static Json apply(Schema schema, Json value, String path, boolean request, List<Violation> out) {
    return walk(schema, value, path, request, out, true);
  }

  /** Drops every derived member of a wire value, in every nested object (nothing is computed). */
  public static Json strip(Schema schema, Json value) {
    return walk(schema, value, "$", false, null, false);
  }

  private static boolean composite(Schema s) {
    return switch (s.resolve().kind) {
      case OBJECT, UNION, LIST, MAP, PAGE -> true;
      default -> false;
    };
  }

  private static Json walk(Schema s, Json v, String path, boolean request, List<Violation> out, boolean compute) {
    Schema node = s.resolve();
    return switch (node.kind) {
      case LIST -> v instanceof Json.Arr arr ? items(node.of, arr, path, request, out, compute) : v;
      case MAP -> v instanceof Json.Obj obj ? members(node.of, obj, path, request, out, compute) : v;
      case PAGE -> v instanceof Json.Obj obj && obj.get("items") != null
          ? obj.with("items", walk(Schema.list(node.of), obj.get("items"), path + ".items", request, out, compute))
          : v;
      case OBJECT -> v instanceof Json.Obj obj ? object(node, obj, path, request, out, compute) : v;
      case UNION -> union(node, v, path, request, out, compute);
      default -> v;
    };
  }

  private static Json items(Schema of, Json.Arr arr, String path, boolean request, List<Violation> out, boolean compute) {
    if (!composite(of)) {
      return arr;
    }
    List<Json> done = new ArrayList<>();
    for (int i = 0; i < arr.items().size(); i++) {
      done.add(walk(of, arr.items().get(i), path + "[" + i + "]", request, out, compute));
    }
    return Json.array(done);
  }

  private static Json members(Schema of, Json.Obj obj, String path, boolean request, List<Violation> out, boolean compute) {
    if (!composite(of)) {
      return obj;
    }
    Map<String, Json> done = new LinkedHashMap<>();
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      done.put(e.getKey(), walk(of, e.getValue(), path + "." + e.getKey(), request, out, compute));
    }
    return new Json.Obj(done);
  }

  private static Json union(Schema node, Json v, String path, boolean request, List<Violation> out, boolean compute) {
    Json label = v instanceof Json.Obj obj ? obj.get(node.discriminator) : null;
    Schema arm = label instanceof Json.Str str ? node.armFor(str.value()) : null;
    if (arm == null) {
      return v;
    }
    Json done = walk(arm, v, path, request, out, compute);
    return done instanceof Json.Obj m ? m.with(node.discriminator, label) : done;
  }

  private static Json object(Schema node, Json.Obj obj, String path, boolean request, List<Violation> out, boolean compute) {
    Map<String, Json> done = new LinkedHashMap<>();
    for (Schema.Field f : node.fields) {
      Json x = obj.get(f.wire());
      if (x != null && !node.derives(f.wire())) {
        done.put(f.wire(), walk(f.type(), x, path + "." + f.wire(), request, out, compute));
      }
    }
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      if (!done.containsKey(e.getKey()) && !node.derives(e.getKey())) {
        done.put(e.getKey(), e.getValue());
      }
    }
    Json.Obj now = new Json.Obj(done);
    if (!compute || node.derive == null) {
      return now;
    }
    return ordered(node, node.derive.run(now, request, path, out));
  }

  /** The members in the order of the schema, then the members the schema does not know. */
  private static Json.Obj ordered(Schema node, Json.Obj obj) {
    Map<String, Json> done = new LinkedHashMap<>();
    for (Schema.Field f : node.fields) {
      Json x = obj.get(f.wire());
      if (x != null) {
        done.put(f.wire(), x);
      }
    }
    for (Map.Entry<String, Json> e : obj.fields().entrySet()) {
      done.putIfAbsent(e.getKey(), e.getValue());
    }
    return new Json.Obj(done);
  }

  // -- whether a schema can carry derived fields ----------------------------------------------

  /** Whether a value of this schema can contain an object that has derived fields. */
  static boolean reaches(Schema schema) {
    Boolean known = REACHES.get(schema);
    if (known != null) {
      return known;
    }
    boolean found = search(schema, new IdentityHashMap<>());
    REACHES.put(schema, found);
    return found;
  }

  private static boolean search(Schema s, IdentityHashMap<Schema, Boolean> seen) {
    Schema node = s.resolve();
    if (seen.put(node, Boolean.TRUE) != null) {
      return false;
    }
    switch (node.kind) {
      case LIST, MAP, PAGE:
        return search(node.of, seen);
      case OBJECT:
        if (node.derive != null) {
          return true;
        }
        for (Schema.Field f : node.fields) {
          if (search(f.type(), seen)) {
            return true;
          }
        }
        return false;
      case UNION:
        for (Schema.Arm a : node.arms) {
          if (search(a.schema().get(), seen)) {
            return true;
          }
        }
        return false;
      default:
        return false;
    }
  }

  // -- the server: requests and outputs -------------------------------------------------------

  /**
   * A request value after {@code Wire.prepare(..., REQUEST)}: its derived fields are computed
   * (the caller's values were already dropped), and the violations are kept for step 5, which
   * answers with them together with the violations of the invariants.
   */
  static Json input(Schema schema, Json prepared, String path) {
    if (!reaches(schema)) {
      return prepared;
    }
    List<Violation> found = new ArrayList<>();
    Json done = apply(schema, prepared, path, true, found);
    keep(found);
    return done;
  }

  /**
   * A value the server produced (a result, a declared error, a stream item), prepared: its derived
   * fields are computed, whatever the output mode, and the violations are kept for the output
   * check that follows.
   */
  static Json output(Schema schema, Json prepared) {
    return output(schema, prepared, "$");
  }

  /** Like {@link #output(Schema, Json)}, with the path the value has in violations. */
  static Json output(Schema schema, Json prepared, String path) {
    if (schema == null || !reaches(schema)) {
      return prepared;
    }
    List<Violation> found = new ArrayList<>();
    Json done = apply(schema, prepared, path, false, found);
    keep(found);
    return done;
  }

  /**
   * A prepared value with its derived fields computed and the violations left out: for an error
   * payload the application did not build, and for a value only the application reads.
   */
  static Json compute(Schema schema, Json prepared) {
    if (schema == null || !search(schema, new IdentityHashMap<>())) {
      return prepared;
    }
    return apply(schema, prepared, "$", false, new ArrayList<>());
  }

  private static void keep(List<Violation> found) {
    if (found.isEmpty()) {
      return;
    }
    List<Violation> all = PENDING.get();
    if (all == null) {
      PENDING.set(found);
    } else {
      all.addAll(found);
    }
  }

  /** The violations kept by {@link #input} and {@link #output} on this thread; forgets them. */
  static List<Violation> take() {
    List<Violation> all = PENDING.get();
    PENDING.remove();
    return all == null ? List.of() : all;
  }

  // -- values of the application --------------------------------------------------------------

  /**
   * A typed value with its defaults applied and its derived fields computed, nested values
   * included. A field that cannot be computed stays {@code null}.
   */
  public static <T> T value(Schema schema, Codec<T> codec, T value) {
    return value(schema, codec, value, new ArrayList<>());
  }

  /** Like {@link #value(Schema, Codec, Object)}; the violations of the fields that failed go to {@code out}. */
  public static <T> T value(Schema schema, Codec<T> codec, T value, List<Violation> out) {
    if (value == null || !search(schema, new IdentityHashMap<>())) {
      return value;
    }
    Json wire = Wire.prepare(schema, codec.encode(value), Wire.Mode.RESPONSE);
    return codec.decode(apply(schema, wire, "$", false, out));
  }
}
