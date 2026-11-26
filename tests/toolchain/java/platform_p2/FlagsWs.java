package veldtest;

import gen.b.flagsws.flags_ws.Heard;
import gen.b.flagsws.flags_ws.Say;
import gen.b.flagsws.server.ChatHandler;
import gen.b.flagsws.server.ChatHooks;
import gen.b.flagsws.server.ChatService;
import gen.b.flagsws.server.TalkRequest;
import gen.b.runtime.Flags;
import gen.b.runtime.RequestContext;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import gen.b.runtime.WsSession;
import gen.b.runtime.WsSocket;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;

/**
 * A flagged invariant on a message the client sends over a WebSocket (spec/platform.md section 11,
 * {@code spec/targets/java.md} 16.2): the message is checked on the socket's own thread, which is not
 * the thread that read the flags of the upgrade request, so the connection carries that reading
 * there. The router is driven directly through {@code VeldHandler.ws} with an in-memory socket, so no
 * Spring is needed. Against {@code flags_ws.veld}. Prints {@code OK}.
 */
public final class FlagsWs {
  private FlagsWs() {}

  /** The socket: what the server sent, the code it closed with, and a way to deliver a frame. */
  private static final class Socket implements WsSocket {
    final List<String> sent = new CopyOnWriteArrayList<>();
    volatile int closed = -1;
    private volatile Consumer<String> listener;

    @Override
    public void send(String text) {
      sent.add(text);
    }

    @Override
    public void close(int code, String reason) {
      closed = code;
    }

    @Override
    public void onMessage(Consumer<String> l) {
      listener = l;
    }

    @Override
    public void onClose(Runnable l) {}

    /** A frame that arrives on a thread of the socket's own, not the one that serves the endpoint. */
    void deliver(String text) throws InterruptedException {
      Thread socketThread = new Thread(() -> listener.accept(text), "socket-thread");
      socketThread.start();
      socketThread.join();
    }
  }

  private static final class Provider implements Flags.Provider {
    final Set<String> on = new HashSet<>();
    volatile boolean failing;

    @Override
    public boolean isOn(String name, Flags.Context context) {
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

  private static void waitFor(Socket socket, int frames) throws InterruptedException {
    for (int i = 0; i < 200 && socket.sent.size() < frames; i++) {
      Thread.sleep(10);
    }
  }

  /** Opens a conversation that echoes what it hears, and answers the socket once the endpoint runs. */
  private static Socket open(VeldHandler handler, CountDownLatch running) throws InterruptedException {
    Socket socket = new Socket();
    check(
        handler.ws(new VeldRequest("GET", "/chat/talk", "", Map.of(), null, null), socket),
        "the endpoint is a ws endpoint");
    check(running.await(5, TimeUnit.SECONDS), "the endpoint started");
    return socket;
  }

  public static void main(String[] args) throws Exception {
    Provider provider = new Provider();
    CountDownLatch[] running = {new CountDownLatch(1)};
    ChatService impl = new ChatService() {
      @Override
      public void talk(TalkRequest request, RequestContext ctx, WsSession<Say, Heard> ws) {
        running[0].countDown();
        ws.incoming().forEach(say -> ws.send(new Heard(say.text())));
      }
    };
    VeldHandler handler = ChatHandler.create(impl, new ChatHooks(provider));

    // The flag is off: the invariant is not evaluated, a long message is accepted and echoed.
    Socket off = open(handler, running[0]);
    off.deliver("{\"text\":\"much too long\"}");
    waitFor(off, 1);
    check(off.sent.size() == 1 && off.sent.get(0).contains("much too long"), "flag off: echoed: " + off.sent);
    check(off.closed == -1, "and the conversation goes on");
    off.deliver("{\"text\":\"short\"}");
    waitFor(off, 2);
    check(off.sent.size() == 2, "a second message: " + off.sent);

    // The flag is on: the same message is refused on the socket's thread, with the violation.
    provider.on.add("shortOnly");
    running[0] = new CountDownLatch(1);
    Socket on = open(handler, running[0]);
    on.deliver("{\"text\":\"short\"}");
    waitFor(on, 1);
    check(on.sent.size() == 1 && on.sent.get(0).contains("short") && on.closed == -1, "flag on, a short message passes: " + on.sent);
    on.deliver("{\"text\":\"much too long\"}");
    check(on.sent.size() == 2, "the refusal is a frame: " + on.sent);
    check(on.sent.get(1).startsWith("{\"error\":") && on.sent.get(1).contains("ValidationError"), "a ValidationError: " + on.sent.get(1));
    check(on.sent.get(1).contains("too long"), "carrying the violation: " + on.sent.get(1));
    check(on.closed == 1008, "closed with 1008: " + on.closed);

    // A provider that fails counts an invariant as on (a restriction fails closed).
    provider.on.clear();
    provider.failing = true;
    running[0] = new CountDownLatch(1);
    Socket failing = open(handler, running[0]);
    failing.deliver("{\"text\":\"much too long\"}");
    check(failing.closed == 1008 && failing.sent.get(0).contains("too long"), "a failing provider: the invariant counts as on: " + failing.sent);

    // The socket's thread keeps no view of its own afterwards.
    check(!Flags.invariantOn("shortOnly"), "the main thread has no view");
    System.out.println("OK");
  }
}
