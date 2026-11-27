package veldtest;

import gen.b.flags_rules.flags_rules.Stay;
import gen.b.flags_rules.server.BookStayRequest;
import gen.b.flags_rules.server.GetStayRequest;
import gen.b.flags_rules.server.VisitsHandler;
import gen.b.flags_rules.server.VisitsHooks;
import gen.b.flags_rules.server.VisitsService;
import gen.b.runtime.Flags;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Flagged invariants over the production handler (spec/platform.md section 11): a flagged invariant
 * of a request and of a result is off, on or, with a failing provider, on; the violations are those
 * of the invariants that are there; and the local check of a client (no provider: a flagged invariant
 * is not evaluated) {@code Stay.check} outside a request. Prints {@code OK}.
 */
public final class FlagsRulesHttp {
  private FlagsRulesHttp() {}

  private static volatile Stay result = new Stay(Instant.parse("2026-01-01T00:00:00Z"), Instant.parse("2026-03-01T00:00:00Z"), 9);

  private static final class Impl implements VisitsService {
    @Override
    public void bookStay(BookStayRequest request, RequestContext ctx) {}

    @Override
    public Stay getStay(GetStayRequest request, RequestContext ctx) {
      return result;
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static final String LONG_BIG =
      "{\"start\":\"2026-01-01T00:00:00Z\",\"end\":\"2026-03-01T00:00:00Z\",\"guests\":9}";

  private static VeldResponse book(VeldHandler handler, String body) {
    return handler
        .handle(new VeldRequest("POST", "/stays", "", Map.of("content-type", List.of("application/json")), body, null))
        .orElseThrow();
  }

  private static List<String> rules(VeldResponse response) {
    List<String> out = new java.util.ArrayList<>();
    for (Json v : Json.parse(response.body()).asObject().get("violations").asArray().items()) {
      String rule = v.asObject().get("rule").asString();
      out.add(rule.substring(rule.lastIndexOf('.') + 1, rule.indexOf('@')));
    }
    java.util.Collections.sort(out);
    return out;
  }

  public static void main(String[] args) {
    Flags.Memory provider = new Flags.Memory(List.of());
    boolean[] failing = {false};
    List<Throwable> errors = new CopyOnWriteArrayList<>();
    VisitsHooks hooks = new VisitsHooks((name, context) -> {
          if (failing[0]) {
            throw new IllegalStateException("the provider is down");
          }
          return provider.isOn(name, context);
        })
        .onError((e, ctx) -> errors.add(e));
    VeldHandler handler = VisitsHandler.create(new Impl(), hooks, gen.b.runtimeconfig.GenBRuntime.builder().rulesOutput(RuleWalk.OutputMode.STRICT).build());

    // A request: the flagged invariants that are off are not evaluated.
    VeldResponse none = book(handler, LONG_BIG);
    check(none.status() == 204, "both flags off: nothing to violate: " + none.status() + none.body());
    provider.set("shortStays", true);
    check(book(handler, LONG_BIG).status() == 422 && rules(book(handler, LONG_BIG)).equals(List.of("maxStay")), "one flag on: its invariant");
    provider.set("smallGroups", true);
    check(rules(book(handler, LONG_BIG)).equals(List.of("maxStay", "smallGroup")), "both on: both: " + rules(book(handler, LONG_BIG)));
    provider.set("shortStays", false);
    provider.set("smallGroups", false);

    // A failing provider: a flagged invariant counts as on, and the failure goes to onError.
    failing[0] = true;
    VeldResponse broken = book(handler, LONG_BIG);
    check(broken.status() == 422 && rules(broken).equals(List.of("maxStay", "smallGroup")), "a failing provider: both count as on: " + broken.body());
    check(!errors.isEmpty(), "the failure was reported");
    failing[0] = false;

    // A result: the invariant of the output follows the output mode, and the flag decides if it is there.
    VeldResponse fine = handler.handle(new VeldRequest("GET", "/stays/one", "", Map.of(), null, null)).orElseThrow();
    check(fine.status() == 200, "a flagged invariant that is off does not reject a result: " + fine.status());
    provider.set("smallGroups", true);
    VeldResponse rejected = handler.handle(new VeldRequest("GET", "/stays/one", "", Map.of(), null, null)).orElseThrow();
    check(rejected.status() == 500, "the same result is a 500 once the flag is on: " + rejected.status());
    provider.set("smallGroups", false);

    // No view, as in a client: a flagged invariant is not evaluated.
    check(Stay.check(result).isEmpty(), "outside a request a flagged invariant is not evaluated: " + Stay.check(result));
    check(Flags.withAnswers(name -> name.equals("smallGroups"), () -> Stay.check(result)).size() == 1, "with the answers of a function it is");
    check(Flags.withFailedProvider(() -> Stay.check(result)).size() == 2, "and with a failed provider both are");
    System.out.println("OK");
  }
}
