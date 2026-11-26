package dev.veld.runtime;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * The audit event of one request (spec/platform.md section 4): what the {@code audit} hook of a
 * server with an audited endpoint is given, once for every request that matched the endpoint,
 * whatever the outcome. The members are the members of the specification's JSON; {@link #toJson()}
 * is that JSON, and its canonical bytes are the same in every target.
 *
 * @param operation the operation signature
 * @param time the clock reading of the request, written like every {@code datetime}
 * @param principal the caller as the authentication hook returned it, without its {@code @pii} and
 *     {@code @secret} fields; {@code null} when authentication failed and for an endpoint with no
 *     auth
 * @param outcome the status and, for an error, its name
 * @param parameters the parameters bound in step 4, by name, in wire form; {@code {}} for a request
 *     that ended before they were bound
 * @param facts the facts loaded when the request ended, by name, in wire form; {@code {}} before the
 *     first
 * @param result the body of the answer as it was sent (the success value, or the data of an error
 *     the contract declares), or {@code null}
 * @param hidden the wire paths {@code @visible} dropped from the result, sorted, without duplicates
 * @param redacted the wire paths left out because of {@code @pii} or {@code @secret}, sorted,
 *     without duplicates
 */
public record AuditEvent(
    String operation,
    String time,
    Json principal,
    Outcome outcome,
    Json.Obj parameters,
    Json.Obj facts,
    Json result,
    List<String> hidden,
    List<String> redacted) {

  /**
   * How the request ended.
   *
   * @param status the status of the answer
   * @param error the name of the error, or {@code null} for a success
   */
  public record Outcome(int status, String error) {}

  /** Creates an event (the lists are copied). */
  public AuditEvent {
    hidden = List.copyOf(hidden);
    redacted = List.copyOf(redacted);
  }

  /** The event as the JSON of the specification: absent members are left out. */
  public Json.Obj toJson() {
    Json.Builder out = Json.object().put("operation", Json.of(operation)).put("time", Json.of(time));
    out.put("principal", principal);
    Json.Builder outcomeJson = Json.object().put("status", Json.of((long) outcome.status()));
    if (outcome.error() != null) {
      outcomeJson.put("error", Json.of(outcome.error()));
    }
    out.put("outcome", outcomeJson.build());
    out.put("parameters", parameters).put("facts", facts).put("result", result);
    out.put("hidden", strings(hidden)).put("redacted", strings(redacted));
    return out.build();
  }

  /** The RFC 8785 canonical bytes of {@link #toJson()}, as text. */
  public String canonical() {
    return toJson().canonical();
  }

  private static Json strings(List<String> items) {
    List<Json> out = new ArrayList<>();
    for (String s : items) {
      out.add(Json.of(s));
    }
    return Json.array(out);
  }

  /** The members as a map, for a log that prefers one. */
  public Map<String, Json> members() {
    return toJson().fields();
  }
}
