package veldtest;

import gen.e.bookings.server.LedgerTestHandler;
import gen.e.runtime.Json;
import gen.e.runtime.VeldHandler;
import gen.e.runtime.VeldRequest;
import gen.e.runtime.VeldResponse;
import java.util.List;
import java.util.Map;

/**
 * The event endpoint of a generated test handler answers {@code 400} with {@code {"error": "..."}}
 * (spec/events.md 10.9) for a missing or empty topic and for a number of deliveries that is neither 1
 * nor 2. Prints {@code events bad request ok}.
 */
public final class EventsTestBadRequest {
  private EventsTestBadRequest() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  private static void rejected(VeldHandler handler, Map<String, List<String>> headers, String what) {
    VeldRequest request = new VeldRequest("POST", "/__veld/test/event", "", headers, "{}", null);
    VeldResponse response = handler.handle(request).orElseThrow(() -> new IllegalStateException("no answer"));
    check(response.status() == 400, what + ": status " + response.status());
    Json.Obj body = Json.parse(response.body()).asObject();
    check(body.get("error") instanceof Json.Str, what + ": the body has an error text: " + response.body());
    check(body.fields().size() == 1, what + ": the body has the error member only");
  }

  public static void main(String[] args) {
    VeldHandler handler = LedgerTestHandler.create();
    rejected(handler, Map.of(), "no topic");
    rejected(handler, Map.of("Veld-Test-Topic", List.of("")), "empty topic");
    rejected(handler, Map.of("Veld-Test-Topic", List.of("bookings.paid"), "Veld-Test-Deliveries", List.of("3")), "three deliveries");
    rejected(handler, Map.of("Veld-Test-Topic", List.of("bookings.paid"), "Veld-Test-Deliveries", List.of("0")), "zero deliveries");
    System.out.println("events bad request ok");
  }
}
