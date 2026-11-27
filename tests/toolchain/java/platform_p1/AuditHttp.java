package veldtest;

import gen.b.audit.audit.Note;
import gen.b.audit.audit.User;
import gen.b.audit.server.AddNoteRequest;
import gen.b.audit.server.BoomRequest;
import gen.b.audit.server.GetNoteRequest;
import gen.b.audit.server.NotesFactLoaders;
import gen.b.audit.server.NotesHandler;
import gen.b.audit.server.NotesHooks;
import gen.b.audit.server.NotesService;
import gen.b.audit.server.QuietRequest;
import gen.b.audit.server.SlowRequest;
import gen.b.audit.server.ViaFactFacts;
import gen.b.audit.server.ViaFactRequest;
import gen.b.runtime.AuditEvent;
import gen.b.runtime.AuditFailure;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.time.Clock;
import java.time.Instant;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The audit log over the real entry point (spec/platform.md section 4), against {@code
 * tests/toolchain/contract/platform_p1/audit.veld}: the event of a success, of a request with a body,
 * of a fact (the clock is read once for the whole request), of a handler that throws and of a
 * deadline, of an early answer ({@code 401}, {@code 422}), an endpoint that is not audited, a hook
 * that throws, and a hook that is not given. Prints {@code OK}.
 */
public final class AuditHttp {
  private AuditHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID CALLER = UUID.fromString("00000000-0000-4000-8000-0000000000c1");
  private static final Instant AT = Instant.parse("2026-06-10T12:00:00.250Z");

  /** A clock that is always {@link #AT} and counts how often it is read. */
  private static final class CountingClock extends Clock {
    final AtomicInteger reads = new AtomicInteger();

    @Override
    public ZoneId getZone() {
      return ZoneOffset.UTC;
    }

    @Override
    public Clock withZone(ZoneId zone) {
      return this;
    }

    @Override
    public Instant instant() {
      return AT;
    }

    @Override
    public long millis() {
      reads.incrementAndGet();
      return AT.toEpochMilli();
    }
  }

  private static Note sample() {
    return new Note(ID, "text", "s3cret");
  }

  private static final class Impl implements NotesService {
    @Override
    public Note getNote(GetNoteRequest request, RequestContext ctx) {
      return sample();
    }

    @Override
    public Note addNote(AddNoteRequest request, RequestContext ctx) {
      return request.body();
    }

    @Override
    public Note viaFact(ViaFactRequest request, ViaFactFacts facts, RequestContext ctx) {
      return facts.note();
    }

    @Override
    public Note boom(BoomRequest request, RequestContext ctx) {
      throw new IllegalStateException("the handler fails");
    }

    @Override
    public Note slow(SlowRequest request, RequestContext ctx) throws Exception {
      Thread.sleep(300);
      return sample();
    }

    @Override
    public Note quiet(QuietRequest request, RequestContext ctx) {
      return sample();
    }
  }

  private static final NotesFactLoaders LOADERS = new NotesFactLoaders() {
    @Override
    public Optional<Note> note(UUID key, RequestContext ctx) {
      return Optional.of(sample());
    }
  };

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static VeldHandler handler(
      List<AuditEvent> events, List<Throwable> errors, CountingClock clock, boolean failing) {
    NotesHooks hooks = new NotesHooks(
            (token, ctx) -> token.equals("nobody") ? null : new User(CALLER, "me@example.test"),
            event -> {
              events.add(event);
              if (failing) {
                throw new IllegalStateException("the audit store is down");
              }
            })
        .clock(clock)
        .onError((e, ctx) -> errors.add(e));
    return NotesHandler.create(new Impl(), LOADERS, hooks);
  }

  private static VeldResponse send(VeldHandler handler, String method, String path, String token, String body) {
    Map<String, List<String>> headers = new java.util.LinkedHashMap<>();
    if (token != null) {
      headers.put("authorization", List.of("Bearer " + token));
    }
    return handler
        .handle(new VeldRequest(method, path, null, headers, body, null))
        .orElseThrow(() -> new AssertionError("no route " + method + " " + path));
  }

  private static List<String> strings(Json list) {
    List<String> out = new ArrayList<>();
    for (Json item : list.asArray().items()) {
      out.add(item.asString());
    }
    return out;
  }

  public static void main(String[] args) {
    // `unvalidated`: the contract was generated with `validate = false`: a parameter of the wrong
    // shape is no 422, so that case is left out; what the audit log does is the same.
    boolean validated = args.length == 0;
    List<AuditEvent> events = new CopyOnWriteArrayList<>();
    List<Throwable> errors = new CopyOnWriteArrayList<>();
    CountingClock clock = new CountingClock();
    VeldHandler handler = handler(events, errors, clock, false);

    // A success: the result is in the event, without the secret; so is the caller, without the email.
    VeldResponse ok = send(handler, "GET", "/notes/" + ID, "me", null);
    check(ok.status() == 200, "the note is answered");
    check(events.size() == 1, "the hook is called once: " + events);
    AuditEvent event = events.get(0);
    check(event.operation().startsWith("veld:audit/Notes.getNote@"), "the operation signature: " + event.operation());
    check(event.time().equals("2026-06-10T12:00:00.250Z"), "the one reading of the clock: " + event.time());
    check(event.outcome().status() == 200 && event.outcome().error() == null, "the outcome of a success");
    check(event.principal() instanceof Json.Obj who && who.get("id") != null && who.get("email") == null,
        "the caller without its @pii: " + event.principal());
    check(event.parameters().get("id") != null, "the parameters are in the event");
    check(event.facts().fields().isEmpty(), "no fact: an object, not absent");
    check(event.result() instanceof Json.Obj result && result.get("text") != null && result.get("secret") == null,
        "the result without its @secret: " + event.result());
    check(event.hidden().isEmpty(), "nothing is hidden in a contract without @visible");
    check(event.redacted().equals(List.of("principal.email", "result.secret")), "what was left out: " + event.redacted());
    check(clock.reads.get() == 1, "the clock is read once: " + clock.reads.get());
    events.clear();

    // A body: the parameter without its secret, and the result.
    String body = "{\"id\":\"" + ID + "\",\"text\":\"t\",\"secret\":\"s\"}";
    VeldResponse created = send(handler, "POST", "/notes", "me", body);
    check(created.status() == 201, "the note is created: " + created.body());
    AuditEvent made = events.get(0);
    check(made.outcome().status() == 201, "the status of the success");
    check(made.parameters().get("body") instanceof Json.Obj p && p.get("secret") == null && p.get("text") != null,
        "the body without its secret: " + made.parameters());
    check(made.redacted().equals(List.of("parameters.body.secret", "principal.email", "result.secret")), "redacted: " + made.redacted());
    events.clear();

    // A fact: in wire form, without its secret; and the clock is read once for the whole request.
    int before = clock.reads.get();
    VeldResponse viaFact = send(handler, "GET", "/notes/fact/" + ID, "me", null);
    check(viaFact.status() == 200, "the fact is answered");
    check(clock.reads.get() - before == 1, "one reading for the request that loads a fact: " + (clock.reads.get() - before));
    AuditEvent facts = events.get(0);
    check(facts.facts().get("note") instanceof Json.Obj f && f.get("text") != null && f.get("secret") == null,
        "the fact as loaded: " + facts.facts());
    check(facts.redacted().contains("facts.note.secret"), "the secret of the fact is listed: " + facts.redacted());
    events.clear();

    // A handler that throws: the caller gets a 500, the event records it and has no result.
    VeldResponse boom = send(handler, "GET", "/notes/boom/" + ID, "me", null);
    check(boom.status() == 500, "the caller gets a 500");
    check(events.size() == 1 && events.get(0).outcome().status() == 500, "the event records the 500: " + events);
    check("Internal".equals(events.get(0).outcome().error()) && events.get(0).result() == null, "an error of the prelude has no result");
    check(errors.size() == 1, "and onError hears the failure once: " + errors);
    errors.clear();
    events.clear();

    // A deadline: the 504 is recorded, once, even though the handler is still running.
    VeldResponse slow = send(handler, "GET", "/notes/slow/" + ID, "me", null);
    check(slow.status() == 504, "the deadline answers 504: " + slow.status());
    check(events.size() == 1 && events.get(0).outcome().status() == 504, "the 504 is recorded once: " + events);
    check("DeadlineExceeded".equals(events.get(0).outcome().error()), "the name of the error");
    events.clear();

    // Early answers are audited too: no caller after a 401, no parameters after a 422.
    VeldResponse unauthorized = send(handler, "GET", "/notes/" + ID, "nobody", null);
    check(unauthorized.status() == 401, "a refused caller is a 401");
    check(events.size() == 1, "the 401 is recorded");
    check(events.get(0).principal() == null, "no principal after a failed authentication");
    check(events.get(0).parameters().fields().isEmpty() && events.get(0).facts().fields().isEmpty(), "{} and {}");
    check(events.get(0).outcome().status() == 401 && "Unauthorized".equals(events.get(0).outcome().error()), "its outcome");
    events.clear();
    if (validated) {
      VeldResponse invalid = send(handler, "GET", "/notes/not-a-uuid", "me", null);
      check(invalid.status() == 422, "a parameter of the wrong shape is a 422: " + invalid.status());
      check(events.size() == 1 && events.get(0).parameters().fields().isEmpty(), "no parameters were bound: " + events);
      check(events.get(0).principal() != null, "but the caller is known");
      events.clear();
    }

    // An endpoint that is not audited does not call the hook, and a route that does not exist neither.
    check(send(handler, "GET", "/notes/quiet/" + ID, "me", null).status() == 200, "the quiet endpoint answers");
    check(handler.handle(new VeldRequest("GET", "/nothing", null, Map.of(), null, null)).isEmpty(), "no route");
    check(events.isEmpty(), "no event for them: " + events);

    // A hook that throws does not change the answer: onError hears an AuditFailure with the event.
    List<AuditEvent> kept = new CopyOnWriteArrayList<>();
    List<Throwable> failures = new CopyOnWriteArrayList<>();
    VeldHandler failing = handler(kept, failures, new CountingClock(), true);
    VeldResponse unchanged = send(failing, "GET", "/notes/" + ID, "me", null);
    check(unchanged.status() == 200 && unchanged.body().contains("\"text\""), "the response is sent unchanged");
    check(failures.size() == 1 && failures.get(0) instanceof AuditFailure, "onError hears an AuditFailure: " + failures);
    AuditFailure failure = (AuditFailure) failures.get(0);
    check(failure.event() != null && failure.event().equals(kept.get(0)) && failure.reason().contains("failed"),
        "with the event and why: " + failure.getMessage());

    // A server whose audit hook is null records nothing and still answers.
    NotesHooks none = new NotesHooks((token, ctx) -> new User(CALLER, "me@example.test"), null);
    check(send(NotesHandler.create(new Impl(), LOADERS, none), "GET", "/notes/" + ID, "me", null).status() == 200,
        "a server without the hook answers");
    System.out.println("OK");
  }
}
