package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.b.adapters.JdkHttpServerAdapter;
import gen.b.flags.flags.Phase;
import gen.b.flags.flags.Room;
import gen.b.flags.server.ArchiveRoomFacts;
import gen.b.flags.server.ArchiveRoomRequest;
import gen.b.flags.server.BookRoomFacts;
import gen.b.flags.server.BookRoomRequest;
import gen.b.flags.server.PublishRoomFacts;
import gen.b.flags.server.PublishRoomRequest;
import gen.b.flags.server.ResizeRoomFacts;
import gen.b.flags.server.ResizeRoomRequest;
import gen.b.flags.server.RoomsHooks;
import gen.b.flags.server.RoomsService;
import gen.b.flags.server.RoomsTestHandler;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import java.net.InetSocketAddress;

/**
 * The generated test handler of {@code tests/rules/endpoints/flags.veld.pending} on the JDK server:
 * the flags come from the header {@code Veld-Test-Flags}, the provider and the authentication hook
 * of the hooks are never called (they are {@code null}), and the application is never reached.
 * Prints {@code PORT=<n>} and serves until it is killed.
 */
public final class FlagsServe {
  private FlagsServe() {}

  public static void main(String[] args) throws Exception {
    RoomsService stub = new RoomsService() {
      @Override
      public Room bookRoom(BookRoomRequest request, BookRoomFacts facts, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Room publishRoom(
          PublishRoomRequest request, PublishRoomFacts facts, Transition<Phase> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Room resizeRoom(ResizeRoomRequest request, ResizeRoomFacts facts, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }

      @Override
      public Room archiveRoom(
          ArchiveRoomRequest request, ArchiveRoomFacts facts, Transition<Phase> transition, RequestContext ctx) {
        throw new UnsupportedOperationException("the application's handler is not part of the examples");
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        RoomsTestHandler.create(stub, new RoomsHooks(null, null)), new InetSocketAddress("127.0.0.1", 0));
    System.out.println("PORT=" + server.getAddress().getPort());
    System.out.flush();
    Thread.sleep(Long.MAX_VALUE);
  }
}
