package veldtest;

import gen.b.flags.flags.Phase;
import gen.b.flags.flags.Room;
import gen.b.flags.flags.User;
import gen.b.flags.server.ArchiveRoomFacts;
import gen.b.flags.server.ArchiveRoomRequest;
import gen.b.flags.server.BookRoomFacts;
import gen.b.flags.server.BookRoomRequest;
import gen.b.flags.server.PublishRoomFacts;
import gen.b.flags.server.PublishRoomRequest;
import gen.b.flags.server.RoomsFactLoaders;
import gen.b.flags.server.RoomsHandler;
import gen.b.flags.server.RoomsHooks;
import gen.b.flags.server.RoomsService;
import gen.b.runtime.Flags;
import gen.b.runtime.Json;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.VeldResponse;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Flags over the real entry point (spec/platform.md section 11), against {@code
 * tests/toolchain/contract/platform_p2/flags.veld}: a flagged {@code require} on, off and with a
 * failing provider (a statement that restricts counts as on), a flagged {@code permit} and {@code
 * forbid} (a grant counts as off when the provider fails, a restriction as on), a flagged flow edge
 * off, on and failing, one reading per request and flag with a counting provider and its context, a
 * server created without the provider, and the client side: {@code canBookRoom} and {@code
 * Policies.manage} with and without the function. Prints {@code OK}.
 */
public final class FlagsHttp {
  private FlagsHttp() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final AtomicInteger HANDLED = new AtomicInteger();
  private static volatile Room room = new Room(ID, false, 3, Phase.DRAFT);

  private static final class Impl implements RoomsService {
    @Override
    public Room bookRoom(BookRoomRequest request, BookRoomFacts facts, RequestContext ctx) {
      HANDLED.incrementAndGet();
      return facts.room();
    }

    @Override
    public Room publishRoom(PublishRoomRequest request, PublishRoomFacts facts, Transition<Phase> transition, RequestContext ctx) {
      HANDLED.incrementAndGet();
      return new Room(ID, facts.room().open(), facts.room().capacity(), Phase.LIVE);
    }

    @Override
    public Room archiveRoom(ArchiveRoomRequest request, ArchiveRoomFacts facts, Transition<Phase> transition, RequestContext ctx) {
      HANDLED.incrementAndGet();
      return new Room(ID, facts.room().open(), facts.room().capacity(), Phase.ARCHIVED);
    }
  }

  private static final RoomsFactLoaders LOADERS = new RoomsFactLoaders() {
    @Override
    public Optional<Room> room(UUID key, RequestContext ctx) {
      return Optional.of(room);
    }
  };

  /** A provider that counts its calls, remembers their contexts and can be told to fail. */
  private static final class Counting implements Flags.Provider {
    final List<String> asked = new CopyOnWriteArrayList<>();
    final List<Flags.Context> contexts = new CopyOnWriteArrayList<>();
    final java.util.Set<String> on = new java.util.HashSet<>();
    volatile boolean failing;

    @Override
    public boolean isOn(String name, Flags.Context context) {
      asked.add(name);
      contexts.add(context);
      if (failing) {
        throw new IllegalStateException("the provider is down");
      }
      return on.contains(name);
    }
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static VeldHandler handler(Counting provider, List<Throwable> errors) {
    RoomsHooks hooks = new RoomsHooks((token, ctx) -> new User(UUID.fromString("00000000-0000-4000-8000-0000000000aa"), List.of(token.split(","))), provider)
        .onError((e, ctx) -> errors.add(e));
    return RoomsHandler.create(new Impl(), LOADERS, hooks);
  }

  private static VeldResponse post(VeldHandler handler, String action, String roles) {
    return handler
        .handle(new VeldRequest("POST", "/rooms/" + ID + "/" + action, "", Map.of("authorization", List.of("Bearer " + roles)), null, null))
        .orElseThrow(() -> new AssertionError("no route " + action));
  }

  public static void main(String[] args) {
    List<Throwable> errors = new ArrayList<>();
    Counting provider = new Counting();
    VeldHandler handler = handler(provider, errors);

    // A flagged require: off is not evaluated, on is an ordinary statement, a failing provider counts as on.
    room = new Room(ID, false, 3, Phase.DRAFT);
    check(post(handler, "book", "staff").status() == 200, "a flagged require that is off is not evaluated");
    provider.on.add("openOnly");
    VeldResponse refused = post(handler, "book", "staff");
    check(refused.status() == 422, "the same room is refused once the flag is on: " + refused.status() + refused.body());
    provider.on.clear();
    provider.failing = true;
    check(post(handler, "book", "staff").status() == 422, "a failing provider: a require counts as on");
    check(!errors.isEmpty() && errors.get(0) instanceof IllegalStateException, "the failure goes to onError: " + errors);
    provider.failing = false;
    errors.clear();

    // A flagged permit grants only while its flag is on, and counts as off when the provider fails.
    check(post(handler, "book", "beta").status() == 403, "a flagged permit that is off does not grant");
    provider.on.add("betaDesk");
    check(post(handler, "book", "beta").status() == 200, "and grants once the flag is on");
    provider.on.clear();
    provider.failing = true;
    check(post(handler, "book", "beta").status() == 403, "a failing provider: a permit counts as off");
    provider.failing = false;

    // A flagged forbid restricts while its flag is on, and counts as on when the provider fails.
    check(post(handler, "book", "staff,suspended").status() == 200, "a flagged forbid that is off does not restrict");
    provider.on.add("lockSuspended");
    check(post(handler, "book", "staff,suspended").status() == 403, "and restricts once the flag is on");
    provider.on.clear();
    provider.failing = true;
    check(post(handler, "book", "staff,suspended").status() == 403, "a failing provider: a forbid counts as on");
    provider.failing = false;
    errors.clear();

    // A flagged flow edge: off it is not an edge for the guard, on it is, a failing provider counts it as off.
    room = new Room(ID, true, 3, Phase.LIVE);
    check(post(handler, "archive", "staff").status() == 409, "a flagged flow edge that is off is not an edge");
    provider.on.add("archiving");
    check(post(handler, "archive", "staff").status() == 200, "and is one once the flag is on");
    provider.on.clear();
    provider.failing = true;
    check(post(handler, "archive", "staff").status() == 409, "a failing provider: a flow counts as off");
    provider.failing = false;

    // One reading per request and flag, after authentication, with the caller in the context.
    provider.asked.clear();
    provider.contexts.clear();
    room = new Room(ID, true, 3, Phase.DRAFT);
    post(handler, "book", "staff");
    check(provider.asked.size() == 3 && provider.asked.stream().distinct().count() == 3,
        "every flag the endpoint can reach is read once: " + provider.asked);
    check(provider.contexts.get(0).principal() instanceof User, "the context has the principal");
    check(provider.contexts.get(0).operation().startsWith("veld:flags/Rooms.bookRoom@"), "and the operation: " + provider.contexts.get(0));
    check(provider.contexts.get(0).tenant() == null, "and no tenant");
    provider.asked.clear();
    VeldResponse unauthenticated = handler.handle(new VeldRequest("POST", "/rooms/" + ID + "/book", "", Map.of(), null, null)).get();
    check(unauthenticated.status() == 401 && provider.asked.isEmpty(), "a request refused before the flags are read reads nothing");

    // A server created without the provider does not start.
    try {
      RoomsHandler.create(new Impl(), LOADERS, new RoomsHooks((token, ctx) -> null, null));
      throw new AssertionError("a server without the provider was created");
    } catch (IllegalStateException e) {
      check(e.getMessage().contains("hooks.flags is required"), e.getMessage());
    }

    // The client side: no provider, the answers of a function, or a failing provider's.
    User staff = new User(ID, List.of("staff"));
    User beta = new User(ID, List.of("beta"));
    Room open = new Room(ID, false, 3, Phase.DRAFT);
    check(gen.b.flags.flags.Policies.manage(staff, open), "a permit without a flag");
    check(!gen.b.flags.flags.Policies.manage(beta, open), "without the function a permit that is flagged counts as off");
    check(gen.b.flags.flags.Policies.manage(beta, open, name -> name.equals("betaDesk")), "with the function it is on");
    check(!gen.b.flags.flags.Policies.manage(beta, open, name -> false), "and off when the function says so");
    check(Phase.LIVE.nextStates().contains(Phase.ARCHIVED), "nextStates lists the flagged edge too");
    System.out.println("OK");
  }
}
