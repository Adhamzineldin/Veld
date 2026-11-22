package veldtest;

import gen.ca.orders.catalog.Money;
import java.util.List;
import gen.ca.orders.orders.OutOfStock;
import gen.ca.orders.orders.Throttled;
import gen.ca.prelude.NotFound;
import gen.ca.runtime.Veld;
import java.util.Map;

/**
 * Cross-package equality: the same contract generated into packages gen.ca and gen.cb, plus a
 * changed shape (gen.cc) and a different declaring package (gen.cd). Mirrors tests/toolchain/ts/equality.ts.
 */
public final class Equality {
  private Equality() {}

  private static String sig(String set, String key) throws Exception {
    return sigSet(set).get(key).asString();
  }

  private static gen.ca.runtime.Json.Obj sigSet(String set) throws Exception {
    return Scenario.sigsFile().get(set).asObject();
  }

  private static OutOfStock a(String sku, String op) {
    return new OutOfStock(new OutOfStock.Data(sku), 409, op);
  }

  private static gen.cb.orders.orders.OutOfStock b(String sku, String op) {
    return new gen.cb.orders.orders.OutOfStock(new gen.cb.orders.orders.OutOfStock.Data(sku), 409, op);
  }

  /**
   * Model identity: `Item` and `Thing` of the shop contract have the same fields, so the same
   * canonical bytes, but they are two declarations. The contract is generated into gen.ma and gen.mb.
   */
  private static void models() {
    var item = new gen.ma.shop.shop.Item("x");
    var itemB = new gen.mb.shop.shop.Item("x");
    var thing = new gen.ma.shop.shop.Thing("x");
    var thingB = new gen.mb.shop.shop.Thing("x");
    Check.equal(item.veldCanonical(), thing.veldCanonical(), "identical canonical bytes");
    Check.that(!gen.ma.shop.shop.Item.VELD_SIG.equals(gen.ma.shop.shop.Thing.VELD_SIG), "two declarations, two signatures");
    // Never the same, whatever the data and whichever package they come from.
    Check.that(!Veld.same(item, thing) && !Veld.same(thing, item), "Item vs Thing");
    Check.that(!Veld.same(item, thingB) && !gen.mb.runtime.Veld.same(thingB, item), "Item vs Thing across packages");
    // The same declaration with equal data is the same, across separately generated packages.
    Check.that(Veld.same(item, itemB) && gen.mb.runtime.Veld.same(itemB, item), "Item across packages");
    Check.that(Veld.same(thing, thingB), "Thing across packages");
    Check.that(!Veld.same(item, new gen.mb.shop.shop.Item("y")), "different data");
    // Record equality is class based, so it never confuses models either.
    Check.that(!item.equals(thing) && item.equals(new gen.ma.shop.shop.Item("x")), "equals");
    // A decoded value against a constructed one.
    var decoded = gen.ma.shop.shop.Item.fromWire(gen.ma.runtime.Json.parse("{\"id\":\"x\"}"));
    Check.that(Veld.same(decoded, itemB), "decoded vs constructed");
    // An error against a model with identical data.
    var gone = new gen.ma.shop.shop.Gone(new gen.ma.shop.shop.Gone.Data("x"), 410, null);
    Check.equal(gone.veldCanonical(), item.veldCanonical(), "identical data");
    Check.that(!Veld.same(gone, item) && !Veld.same(item, gone), "error vs model");
    // A generated value never equals a value that carries no identity, whatever its data.
    Check.that(!Veld.same(item, "x") && !Veld.same("x", item) && !Veld.same(item, null), "no identity on one side");
    // Lists compare element by element, across packages.
    Check.that(Veld.same(List.of(item, thing), List.of(itemB, thingB)), "lists across packages");
    Check.that(!Veld.same(List.of(item), List.of(thingB)), "lists of different declarations");
  }

  public static void main(String[] args) throws Exception {
    String create = sig("a", "Orders.createOrder");
    String pay = sig("a", "Orders.payOrder");
    // Distinct generated copies are distinct classes.
    Check.that((Object) OutOfStock.class != (Object) gen.cb.orders.orders.OutOfStock.class, "distinct classes");
    Check.that((Object) gen.ca.runtime.VeldError.class != (Object) gen.cb.runtime.VeldError.class, "distinct runtimes");

    // Equal: same signature, operation and payload, across two copies.
    OutOfStock x = a("X", create);
    var y = b("X", create);
    Check.that(Veld.same(x, y) && Veld.same(y, x), "equal errors across copies");
    Check.that(x.equals(y) && y.equals(x), "equals across copies");
    Check.equal(x.hashCode(), y.hashCode(), "hashCode across copies");
    Check.that(!(((Object) y) instanceof OutOfStock), "instanceof is not reliable across packages");
    Check.that(gen.cb.runtime.Veld.same(x, y), "the other copy's Veld agrees");

    // Different payload, operation, set-vs-unset operation.
    Check.that(!Veld.same(x, b("Y", create)), "different data");
    Check.that(!x.equals(b("Y", create)), "different data (equals)");
    Check.that(!Veld.same(x, b("X", pay)), "different op");
    Check.that(!Veld.same(a("X", null), x), "op set vs unset");

    // Same prelude error name raised by two different services.
    var nfOrders = new NotFound(new NotFound.Data("x"), 404, sig("a", "Orders.getOrder"));
    var nfIam = new gen.cb.prelude.NotFound(new gen.cb.prelude.NotFound.Data("x"), 404, sig("a", "Iam.getUser"));
    Check.that(!Veld.same(nfOrders, nfIam), "same error name, different service");
    Check.that(Veld.same(nfOrders, new gen.cb.prelude.NotFound(new gen.cb.prelude.NotFound.Data("x"), 404, sig("a", "Orders.getOrder"))), "same service");

    // Same error name declared in a different package (and so a different signature).
    var d = new gen.cd.warehouse.orders.OutOfStock(new gen.cd.warehouse.orders.OutOfStock.Data("X"), 409, sig("d", "Orders.createOrder"));
    Check.that(!Veld.same(x, d), "different declaring package");

    // A changed contract shape has a different signature.
    var c = new gen.cc.orders.orders.OutOfStock(new gen.cc.orders.orders.OutOfStock.Data("X", "w"), 409, sig("c", "Orders.createOrder"));
    Check.that(!OutOfStock.VELD_SIG.equals(gen.cc.orders.orders.OutOfStock.VELD_SIG), "changed shape, changed sig");
    Check.that(!Veld.same(x, c), "changed contract shape");

    // matches: the declaration raised by its own service, from any operation.
    Check.that(OutOfStock.matches(y), "matches across copies");
    Check.that(OutOfStock.matches(b("Z", pay)), "matches from another operation of the service");
    Check.that(!OutOfStock.matches(d), "different declaration");
    Check.that(!OutOfStock.matches(nfOrders), "different error");
    Check.that(NotFound.matches(nfIam), "prelude errors match from any service");
    Check.that(!OutOfStock.matches(b("Z", sig("a", "Iam.getUser"))), "wrong declaring service");
    Check.that(OutOfStock.matches(b("Z", pay), "orders/Orders"), "service filter");
    Check.that(!OutOfStock.matches(b("Z", pay), "orders/Payments"), "service filter excludes");
    Check.that(!OutOfStock.matches(new RuntimeException("x")), "plain exceptions never match");

    // Models: canonical bytes agree across copies and differ when the value differs.
    Money m = new Money(new java.math.BigDecimal("1.50"), "USD");
    var m2 = new gen.cb.orders.catalog.Money(new java.math.BigDecimal("1.50"), "USD");
    Check.equal(m.veldCanonical(), m2.veldCanonical(), "canonical across copies");
    Check.that(Veld.same(m, m2), "models equal across copies");
    Check.that(!Veld.same(m, new gen.cb.orders.catalog.Money(new java.math.BigDecimal("1.5"), "USD")), "decimal text matters");
    Check.equal(Money.VELD_SIG, gen.cb.orders.catalog.Money.VELD_SIG, "model signature");

    // Error data: schema defaults are applied before comparing (spec/ir.md 5.3).
    String op = create;
    var t1 = new Throttled(new Throttled.Data(null), 429, op);
    var t2 = new gen.cb.orders.orders.Throttled(new gen.cb.orders.orders.Throttled.Data(30), 429, op);
    Check.that(Veld.same(t1, t2), "absent defaulted field equals the default value");
    Check.that(Veld.same(t2, t1), "...symmetric");
    Check.that(!Veld.same(t1, new gen.cb.orders.orders.Throttled(new gen.cb.orders.orders.Throttled.Data(31), 429, op)), "a different value is different");
    Check.equal(t1.veldCanonical(), t2.veldCanonical(), "canonical bytes apply defaults");

    // Companion signatures equal the IR's.
    Map<String, gen.ca.runtime.Json> irSigs = sigSet("a").fields();
    Check.equal(irSigs.get("decl:orders:catalog.Money").asString(), Money.VELD_SIG, "Money sig");
    Check.equal(irSigs.get("decl:orders:orders.OutOfStock").asString(), OutOfStock.VELD_SIG, "OutOfStock sig");

    models();
    System.out.println("equality ok");
  }
}
