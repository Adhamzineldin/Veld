package veldtest;

import gen.b.runtime.RequestContext;
import gen.b.runtime.RuleWalk;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.WsSession;
import gen.b.runtime.WsSocket;
import gen.b.shapesws.server.ChatHandler;
import gen.b.shapesws.server.ChatHooks;
import gen.b.shapesws.server.ChatService;
import gen.b.shapesws.server.RoomRequest;
import gen.b.shapesws.shapesws.Msg;
import gen.b.shapesws.shapesws.Ping;
import gen.b.shapesws.shapesws.User;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;

/**
 * A message the server sends over a WebSocket is shown to the caller and checked for another tenant
 * like any other answer (spec/platform.md sections 2 and 3): a field the caller may not see is not in
 * the frame, a message of another tenant is not sent (the connection ends with the {@code Internal}
 * problem and the rule hook hears of it). The router is driven directly through {@code
 * VeldHandler.ws} with an in-memory socket, so no Spring is needed. Against {@code shapes_ws.veld}.
 * Prints {@code OK}.
 */
public final class ShapesWs {
  private ShapesWs() {}

  private static final UUID ID = UUID.fromString("00000000-0000-4000-8000-000000000001");
  private static final UUID MINE = UUID.fromString("00000000-0000-4000-8000-0000000000aa");
  private static final UUID THEIRS = UUID.fromString("00000000-0000-4000-8000-0000000000bb");

  private static final class Socket implements WsSocket {
    final List<String> sent = new CopyOnWriteArrayList<>();
    volatile int closed = -1;

    @Override
    public void send(String text) {
      sent.add(text);
    }

    @Override
    public void close(int code, String reason) {
      closed = code;
    }

    @Override
    public void onMessage(Consumer<String> listener) {}

    @Override
    public void onClose(Runnable listener) {}
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  /** Opens a conversation as the caller {@code token} and lets the endpoint send {@code messages}. */
  private static Socket open(String token, List<Msg> messages, List<RuleWalk.Event> events) throws InterruptedException {
    CountDownLatch done = new CountDownLatch(1);
    ChatService impl = new ChatService() {
      @Override
      public void room(RoomRequest request, RequestContext ctx, WsSession<Ping, Msg> ws) {
        try {
          for (Msg message : messages) {
            ws.send(message);
          }
        } finally {
          done.countDown();
        }
      }
    };
    ChatHooks hooks = new ChatHooks((bearer, ctx) -> new User(ID, token.equals("support") ? List.of("support") : List.of(), MINE))
        .onRuleViolation(events::add);
    VeldHandler handler = ChatHandler.create(impl, hooks);
    Socket socket = new Socket();
    check(
        handler.ws(
            new VeldRequest("GET", "/chat/room", "", Map.of("authorization", List.of("Bearer " + token)), null, null),
            socket),
        "the endpoint is a ws endpoint");
    check(done.await(5, TimeUnit.SECONDS), "the endpoint ran");
    return socket;
  }

  public static void main(String[] args) throws Exception {
    List<RuleWalk.Event> events = new CopyOnWriteArrayList<>();
    Msg mine = new Msg(ID, MINE, "hello", "private note");

    // A guest does not see the note; support does.
    Socket guest = open("guest", List.of(mine), events);
    check(guest.sent.size() == 1 && guest.sent.get(0).contains("hello"), "the message is sent: " + guest.sent);
    check(!guest.sent.get(0).contains("private note"), "the note is not in the frame for a guest: " + guest.sent);
    Socket support = open("support", List.of(mine), events);
    check(support.sent.get(0).contains("private note"), "support sees the note: " + support.sent);
    check(events.isEmpty(), "no violation so far: " + events);

    // A message of another tenant is never sent: the connection ends with the Internal problem.
    Socket foreign = open("guest", List.of(new Msg(ID, THEIRS, "secret of the others", null)), events);
    check(foreign.sent.stream().noneMatch(f -> f.contains("secret of the others")), "nothing of the other tenant: " + foreign.sent);
    check(foreign.sent.size() == 1 && foreign.sent.get(0).startsWith("{\"error\":") && foreign.sent.get(0).contains("Internal"),
        "the problem: " + foreign.sent);
    check(events.size() == 1 && events.get(0).violation().path().equals("item"), "the rule hook hears: " + events);
    System.out.println("OK");
  }
}
