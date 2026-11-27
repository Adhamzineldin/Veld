package veldtest;

import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import gen.b.tenantkey.server.AddNoteRequest;
import gen.b.tenantkey.server.NotesHandler;
import gen.b.tenantkey.server.NotesHooks;
import gen.b.tenantkey.server.NotesService;
import gen.b.tenantkey.tenantkey.Note;
import gen.b.tenantkey.tenantkey.User;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The idempotency key of a request that carries the {@code @tenant} member (spec/platform.md section
 * 3, 9.2): the fingerprint leaves the tenant out, as it leaves out a read-only member, so the same
 * body with another tenant is the same request (a replay), and a body that differs in another member
 * is not ({@code 409}). Prints {@code OK}.
 */
public final class TenancyFingerprint {
  private TenancyFingerprint() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID A = UUID.fromString("00000000-0000-4000-8000-0000000000aa");
  private static final UUID B = UUID.fromString("00000000-0000-4000-8000-0000000000bb");
  private static final AtomicInteger CALLS = new AtomicInteger();

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static VeldResponse post(VeldHandler handler, String key, UUID tenant, String text) {
    String body = "{\"id\":\"" + ID + "\"" + (tenant == null ? "" : ",\"tenantId\":\"" + tenant + "\"") + ",\"text\":\"" + text + "\"}";
    Map<String, List<String>> headers = Map.of("authorization", List.of("Bearer x"), "idempotency-key", List.of(key));
    return handler.handle(new VeldRequest("POST", "/notes", null, headers, body, null)).orElseThrow();
  }

  public static void main(String[] args) {
    NotesService impl = new NotesService() {
      @Override
      public Note addNote(AddNoteRequest request, RequestContext ctx) {
        CALLS.incrementAndGet();
        return new Note(ID, A, request.body().text());
      }
    };
    NotesHooks hooks = new NotesHooks((token, ctx) -> new User(A, A), new MemoryIdempotencyStore(), (principal, ctx) -> "caller");
    VeldHandler handler = NotesHandler.create(impl, hooks);

    check(post(handler, "k1", A, "x").status() == 201, "the first request runs");
    VeldResponse replay = post(handler, "k1", B, "x");
    check(replay.status() == 201 && "true".equals(replay.headers().get("veld-idempotent-replay")),
        "another tenant is the same request: " + replay.status() + " " + replay.body());
    VeldResponse without = post(handler, "k1", null, "x");
    check("true".equals(without.headers().get("veld-idempotent-replay")), "no tenant is the same request: " + without.status());
    check(CALLS.get() == 1, "the handler ran once: " + CALLS.get());
    check(post(handler, "k1", A, "y").status() == 409, "another member is another request");
    System.out.println("OK");
  }
}
