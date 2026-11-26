package veldtest;

import gen.c.flags.client.RoomsClient;
import gen.c.flags.flags.Phase;
import gen.c.flags.flags.Policies;
import gen.c.flags.flags.Room;
import gen.c.flags.flags.User;
import java.util.List;
import java.util.UUID;

/**
 * The client side of flags (spec/platform.md section 11): a client has no provider, so {@code
 * can<Endpoint>} and the policy functions decide as a failing provider does (a flagged {@code permit}
 * and a flagged {@code flow} count as off, a flagged {@code forbid} as on) unless they get a function
 * from a flag name to a boolean as their last argument; {@code nextStates} lists a flagged edge. Prints
 * {@code OK}.
 */
public final class FlagsClient {
  private FlagsClient() {}

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  public static void main(String[] args) {
    UUID id = UUID.fromString("00000000-0000-4000-8000-000000000001");
    User staff = new User(id, List.of("staff"));
    User beta = new User(id, List.of("beta"));
    User suspended = new User(id, List.of("staff", "suspended"));
    Room draft = new Room(id, true, 3, Phase.DRAFT);
    Room live = new Room(id, true, 3, Phase.LIVE);

    check(RoomsClient.canBookRoom(draft, staff), "an unflagged permit grants without a function");
    check(!RoomsClient.canBookRoom(draft, beta), "a flagged permit counts as off without a function");
    check(RoomsClient.canBookRoom(draft, beta, flag -> flag.equals("betaDesk")), "and is on with the function");
    check(!RoomsClient.canBookRoom(draft, beta, flag -> false), "and off when the function says so");
    check(!RoomsClient.canBookRoom(draft, suspended), "a flagged forbid counts as on without a function");
    check(RoomsClient.canBookRoom(draft, suspended, flag -> false), "and is off when the function says so");
    check(!RoomsClient.canBookRoom(draft, suspended, flag -> flag.equals("lockSuspended")), "and on when it says so");

    check(RoomsClient.canPublishRoom(draft, staff), "an unflagged edge needs no function");
    check(!RoomsClient.canArchiveRoom(live, staff), "a flagged flow counts as off without a function");
    check(RoomsClient.canArchiveRoom(live, staff, flag -> flag.equals("archiving")), "and is an edge with it");
    check(!RoomsClient.canArchiveRoom(draft, staff, flag -> true), "the state still decides");

    check(Policies.manage(staff, draft) && !Policies.manage(beta, draft), "the policy function decides as the can methods do");
    check(Policies.manage(beta, draft, flag -> true), "also with a function");
    check(Phase.LIVE.nextStates().equals(List.of(Phase.ARCHIVED)), "nextStates lists the flagged edge");
    System.out.println("OK");
  }
}
