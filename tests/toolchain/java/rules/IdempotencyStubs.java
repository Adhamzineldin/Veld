package veldtest;

import gen.b.idemp.idemp.Booking;
import gen.b.idemp.idemp.Shelf;
import gen.b.idemp.server.BookFacts;
import gen.b.idemp.server.BookMineRequest;
import gen.b.idemp.server.BookRequest;
import gen.b.idemp.server.BookingsHooks;
import gen.b.idemp.server.BookingsService;
import gen.b.idemp.server.BookingsTestHandler;
import gen.b.idemp.server.CancelRequest;
import gen.b.idemp.server.PayRequest;
import gen.b.idemp.server.ReplaceRequest;
import gen.b.idemp.server.SaveRoomFacts;
import gen.b.idemp.server.SaveRoomRequest;
import gen.b.runtime.MemoryIdempotencyStore;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;

/**
 * The generated test handler of {@code tests/rules/endpoints/idempotency.veld.pending} over
 * a stub application that is never reached (every example is a dry run or brings its
 * result). The store and the caller hook of the hooks are never used: the test handler brings
 * its own. Used by the JDK server ({@link IdempotencyServe}) and by Spring.
 */
public final class IdempotencyStubs {
  private IdempotencyStubs() {}

  private static UnsupportedOperationException never() {
    return new UnsupportedOperationException("the application's handler is not part of the examples");
  }

  /** The test handler over a stub application. */
  static VeldHandler handler() {
    BookingsService stub = new BookingsService() {
      @Override
      public Booking book(BookRequest request, BookFacts facts, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking pay(PayRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking replace(ReplaceRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public void cancel(CancelRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Booking bookMine(BookMineRequest request, RequestContext ctx) {
        throw never();
      }

      @Override
      public Shelf saveRoom(SaveRoomRequest request, SaveRoomFacts facts, Long version, RequestContext ctx) {
        throw never();
      }
    };
    BookingsHooks hooks = new BookingsHooks((token, ctx) -> null, new MemoryIdempotencyStore(), (principal, ctx) -> "x");
    return BookingsTestHandler.create(stub, hooks);
  }
}
