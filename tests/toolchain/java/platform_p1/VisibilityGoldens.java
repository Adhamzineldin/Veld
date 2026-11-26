package veldtest;

import gen.b.guests.guests.Address;
import gen.b.guests.guests.Guest;
import gen.b.guests.guests.Order;
import gen.b.guests.guests.Rejected;
import gen.b.guests.guests.User;
import gen.b.guests.guests.Vip;
import gen.b.runtime.Json;
import gen.b.runtime.Platform;
import gen.b.runtime.Schema;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Every case of {@code tests/wire/visibility/cases.json} through the generated runtime
 * ({@code Platform.showValue}, spec/platform.md section 2): the value after visibility, canonical,
 * must equal {@code shown} byte for byte, and the paths of what was dropped must equal {@code
 * hidden}. Expects the contract of {@code tests/wire/visibility/contract.veld}, generated as the
 * package {@code guests} with a service appended (a server needs one), and the cases copied next to
 * the working directory as {@code cases.json}. Prints {@code OK}.
 */
public final class VisibilityGoldens {
  private VisibilityGoldens() {}

  private static final Map<String, Schema> TYPES = Map.of(
      "Address", Address.SCHEMA,
      "Guest", Guest.SCHEMA,
      "Order", Order.SCHEMA,
      "Vip", Vip.SCHEMA,
      "Rejected", Rejected.SCHEMA);

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
      // A caller of the principal's model, or none; the clock of the request in milliseconds.
      Object caller = c.get("caller") instanceof Json.Obj who ? User.fromWire(who) : null;
      long now = Instant.parse(c.get("now").asString()).toEpochMilli();
      Platform.Shown shown = Platform.showValue(schema, c.get("value"), caller, now);
      String bytes = shown.value().canonical();
      String expected = c.get("shown").asString();
      if (!bytes.equals(expected)) {
        throw new AssertionError(name + ": the value that is sent\n  got      " + bytes + "\n  expected " + expected);
      }
      List<String> hidden = new ArrayList<>();
      for (Json path : c.get("hidden").asArray().items()) {
        hidden.add(path.asString());
      }
      if (!shown.hidden().equals(hidden)) {
        throw new AssertionError(name + ": the paths that were dropped\n  got      " + shown.hidden() + "\n  expected " + hidden);
      }
      ran++;
    }
    if (ran != cases.size() || ran < 20) {
      throw new AssertionError("the goldens have their cases: " + ran);
    }
    System.out.println("visibility goldens: " + ran + " cases");
    System.out.println("OK");
  }
}
