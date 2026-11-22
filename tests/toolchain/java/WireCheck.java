package veldtest;

import gen.ca.orders.catalog.Money;
import gen.ca.orders.catalog.Payment;
import gen.ca.orders.orders.CreateOrder;
import gen.ca.orders.orders.Order;
import gen.ca.orders.orders.OrderFilter;
import gen.ca.runtime.Json;
import gen.ca.runtime.Schema;
import gen.ca.runtime.Wire;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.stream.Stream;

/**
 * Wire goldens: tests/wire/*.json, checked through the generated runtime and companions. The same
 * goldens are checked against the generated TypeScript by tests/toolchain/ts/wire.ts, so both
 * languages produce the same canonical bytes for the same values.
 */
public final class WireCheck {
  private WireCheck() {}

  /** A model companion: its schema and the typed round trip to canonical bytes. */
  private record Model(Schema schema, Function<Json, String> typed) {}

  private static final Map<String, Model> MODELS = Map.of(
      "Money", new Model(Money.SCHEMA, w -> Money.fromWire(w).veldCanonical()),
      "CreateOrder", new Model(CreateOrder.SCHEMA, w -> CreateOrder.fromWire(w).veldCanonical()),
      "Order", new Model(Order.SCHEMA, w -> Order.fromWire(w).veldCanonical()),
      "OrderFilter", new Model(OrderFilter.SCHEMA, w -> OrderFilter.fromWire(w).veldCanonical()),
      "Payment", new Model(Payment.SCHEMA, w -> Payment.veldCanonical(Payment.fromWire(w))));

  public static void main(String[] args) throws IOException {
    Path dir = Path.of(System.getenv("WIRE_DIR"));
    int checked = 0;
    List<Path> files;
    try (Stream<Path> s = Files.list(dir)) {
      files = s.filter(p -> p.getFileName().toString().endsWith(".json")).sorted().toList();
    }
    for (Path file : files) {
      for (Json item : ((Json.Arr) Json.parse(Files.readString(file))).items()) {
        Json.Obj c = item.asObject();
        String name = file.getFileName() + ": " + c.get("name").asString();
        String expected = c.get("canonical").asString();
        Json input = c.get("input");
        if (c.get("kind").asString().equals("json")) {
          Check.equal(expected, input.canonical(), name);
        } else {
          Model m = MODELS.get(c.get("decl").asString());
          Check.that(m != null, name + ": no companion " + c.get("decl").asString());
          Check.equal(List.of(), Wire.validate(m.schema(), input, "", false), name + " validates");
          Check.equal(expected, Wire.canonicalOf(m.schema(), input), name);
          Check.equal(expected, m.typed().apply(Wire.prepare(m.schema(), input, Wire.Mode.RESPONSE)), name + " (typed)");
        }
        checked++;
      }
    }
    Check.that(checked > 0, "no goldens found");
    System.out.println("wire ok (" + checked + " cases)");
  }
}
