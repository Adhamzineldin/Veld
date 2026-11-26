package veldtest;

import gen.b.guests.guests.Address;
import gen.b.guests.guests.Guest;
import gen.b.guests.guests.GuestRegistered;
import gen.b.guests.guests.Order;
import gen.b.guests.guests.Rejected;
import gen.b.guests.guests.Staff;
import gen.b.runtime.Json;
import gen.b.runtime.Redact;
import gen.b.runtime.Schema;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

/**
 * Every case of {@code tests/wire/redaction/cases.json} through the redaction of the generated
 * runtime (spec/rules.md section 19.3): the value with its {@code @pii} and {@code @secret} fields
 * left out, canonical, must equal {@code redacted} byte for byte, and the paths of what was left out
 * must equal {@code paths}. The contract has no service, and a server needs one: the test appends an
 * audited one (the redaction is part of the audit log). Expects the cases next to the working
 * directory as {@code cases.json}. Prints {@code OK}.
 */
public final class AuditRedaction {
  private AuditRedaction() {}

  private static final Map<String, Schema> TYPES = Map.of(
      "Address", Address.SCHEMA,
      "Guest", Guest.SCHEMA,
      "GuestRegistered", GuestRegistered.SCHEMA,
      "Order", Order.SCHEMA,
      "Rejected", Rejected.SCHEMA,
      "Staff", Staff.SCHEMA);

  public static void main(String[] args) throws Exception {
    Json.Obj file = Json.parse(Files.readString(Path.of("cases.json"))).asObject();
    List<Json> cases = file.get("cases").asArray().items();
    int ran = 0;
    for (Json item : cases) {
      Json.Obj c = item.asObject();
      String name = c.get("name").asString();
      Schema schema = TYPES.get(c.get("type").asString());
      if (schema == null) {
        throw new AssertionError(name + ": the contract has no declaration " + c.get("type").asString());
      }
      List<String> found = new ArrayList<>();
      Json kept = Redact.value(schema, c.get("value"), "$", found);
      String bytes = kept.canonical();
      String expected = c.get("redacted").asString();
      if (!bytes.equals(expected)) {
        throw new AssertionError(name + ": the value that is kept\n  got      " + bytes + "\n  expected " + expected);
      }
      TreeSet<String> sorted = new TreeSet<>(
          (a, b) -> Arrays.compareUnsigned(a.getBytes(StandardCharsets.UTF_8), b.getBytes(StandardCharsets.UTF_8)));
      sorted.addAll(found);
      List<String> paths = new ArrayList<>();
      for (Json path : c.get("paths").asArray().items()) {
        paths.add(path.asString());
      }
      if (!new ArrayList<>(sorted).equals(paths)) {
        throw new AssertionError(name + ": the paths that were left out\n  got      " + sorted + "\n  expected " + paths);
      }
      ran++;
    }
    if (ran != cases.size() || ran < 10) {
      throw new AssertionError("the goldens have their cases: " + ran);
    }
    System.out.println("redaction goldens: " + ran + " cases");
    System.out.println("OK");
  }
}
