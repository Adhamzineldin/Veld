package dev.veld.runtime;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/**
 * The server side of one accepted WebSocket: validates incoming frames, holds them back until
 * authentication and middleware are done, and writes the {@code {"data":...}} and
 * {@code {"error":...}} envelopes.
 */
final class WsConnection implements WsSession<Json, Json> {
  private static final Object END = new Object();

  private final EndpointSpec spec;
  private final Schema incomingSchema;
  private final WsSocket socket;
  private final boolean validateResponses;
  private final boolean validateRequests;
  private final Router.StdSigs std;
  private final BlockingQueue<Object> inbox = new LinkedBlockingQueue<>();
  private final List<String> early = new ArrayList<>();
  private boolean ready;
  private volatile boolean closed;
  // #if rules
  private HandlerHooks rulesHooks;
  // #endif
  // #if flags
  private volatile Flags.View flagView;
  // #endif

  WsConnection(
      EndpointSpec spec,
      WsSocket socket,
      boolean validateResponses,
      boolean validateRequests,
      Router.StdSigs std) {
    this.validateRequests = validateRequests;
    this.spec = spec;
    this.socket = socket;
    this.validateResponses = validateResponses;
    this.std = std;
    Schema in = null;
    for (EndpointSpec.Param p : spec.params()) {
      if (p.stream()) {
        in = p.type();
      }
    }
    this.incomingSchema = in;
  }

  // #if rules
  /** Gives the connection the hooks that carry the rule output mode and the violation hook. */
  void rules(HandlerHooks hooks) {
    this.rulesHooks = hooks;
  }

  // #endif
  // #if flags
  /** Gives the connection the flags the request read, for the messages that arrive on the socket's own thread. */
  void flags(Flags.View view) {
    this.flagView = view;
  }

  // #endif
  /** Starts listening; frames are held back until {@link #ready()}. */
  void attach() {
    socket.onMessage(this::onText);
    socket.onClose(this::remoteClosed);
  }

  /** Processes the frames that arrived while auth and middleware ran, then every later one. */
  void ready() {
    synchronized (this) {
      ready = true;
      for (String text : early) {
        accept(text);
      }
      early.clear();
    }
  }

  private void onText(String text) {
    synchronized (this) {
      if (ready) {
        accept(text);
      } else {
        early.add(text);
      }
    }
  }

  private void remoteClosed() {
    closed = true;
    inbox.add(END);
  }

  private void accept(String text) {
    // #if flags
    Flags.View enclosing = Flags.install(flagView);
    try {
    // #endif
    if (closed) {
      return;
    }
    if (incomingSchema == null) {
      invalid(new Issue("message", "unexpected", "this endpoint does not accept messages"));
      return;
    }
    Json raw;
    try {
      raw = Json.parse(text);
    } catch (Json.ParseException e) {
      invalid(new Issue("message", "json", "message is not valid JSON"));
      return;
    }
    List<Issue> issues =
        validateRequests ? Wire.validate(incomingSchema, raw, "message", true) : List.of();
    if (!issues.isEmpty()) {
      fail(validation(issues));
      return;
    }
    // #if rules_no_derived
    if (rulesHooks != null && validateRequests) {
      VeldResponse broken = RuleWalk.messageFailure(
          spec, rulesHooks, incomingSchema, Wire.prepare(incomingSchema, raw, Wire.Mode.REQUEST), std);
      if (broken != null) {
        fail(broken);
        return;
      }
    }
    // #endif
    // #if no_derived
    inbox.add(Wire.fillReadonly(incomingSchema, Wire.prepare(incomingSchema, raw, Wire.Mode.REQUEST)));
    // #endif
    // #if derived
    Json prepared = Derive.input(incomingSchema, Wire.prepare(incomingSchema, raw, Wire.Mode.REQUEST), "message");
    if (rulesHooks != null && validateRequests) {
      VeldResponse broken = RuleWalk.messageFailure(spec, rulesHooks, incomingSchema, prepared, std);
      if (broken != null) {
        fail(broken);
        return;
      }
    } else {
      VeldResponse broken = rulesHooks == null ? null : RuleWalk.derivedFailure(spec, rulesHooks, std);
      Derive.take();
      if (broken != null) {
        fail(broken);
        return;
      }
    }
    inbox.add(Wire.fillReadonly(incomingSchema, prepared));
    // #endif
    // #if flags
    } finally {
      Flags.restore(enclosing);
    }
    // #endif
  }

  private void invalid(Issue issue) {
    fail(validation(List.of(issue)));
  }

  private VeldResponse validation(List<Issue> issues) {
    return Router.problem(
        std.validation(), "ValidationError", 422, spec.sig(), Router.issuesToWire(issues));
  }

  /** Sends the problem as the final {@code error} frame and closes (1008, or 1011 for a 500). */
  void fail(VeldResponse response) {
    if (closed) {
      return;
    }
    send("{\"error\":" + (response.body() == null ? "null" : response.body()) + "}");
    finish(response.status() >= 500 ? 1011 : 1008, "problem " + response.status());
  }

  /** Closes with {@code code}; the second and later calls do nothing. */
  void finish(int code, String reason) {
    synchronized (this) {
      if (closed) {
        return;
      }
      closed = true;
    }
    inbox.add(END);
    socket.close(code, reason);
  }

  private void send(String text) {
    try {
      socket.send(text);
    } catch (RuntimeException e) {
      closed = true;
      inbox.add(END);
    }
  }

  @Override
  public Stream<Json> incoming() {
    Iterator<Json> it = new Iterator<>() {
      private Json nextItem;
      private boolean over;

      @Override
      public boolean hasNext() {
        if (nextItem != null) {
          return true;
        }
        if (over) {
          return false;
        }
        try {
          Object x = inbox.take();
          if (x == END) {
            inbox.add(END);
            over = true;
            return false;
          }
          nextItem = (Json) x;
          return true;
        } catch (InterruptedException e) {
          Thread.currentThread().interrupt();
          over = true;
          return false;
        }
      }

      @Override
      public Json next() {
        if (!hasNext()) {
          throw new NoSuchElementException();
        }
        Json out = nextItem;
        nextItem = null;
        return out;
      }
    };
    return StreamSupport.stream(
        Spliterators.spliteratorUnknownSize(it, Spliterator.ORDERED | Spliterator.NONNULL), false);
  }

  @Override
  public void send(Json message) {
    if (closed) {
      throw new IllegalStateException(spec.name() + ": the WebSocket is closed");
    }
    if (spec.success() == null) {
      throw new IllegalStateException(spec.name() + ": this endpoint sends no messages");
    }
    Json wire = Wire.prepare(spec.success(), message, Wire.Mode.RESPONSE);
    // #if derived
    wire = Derive.output(spec.success(), wire);
    // #endif
    if (validateResponses) {
      List<Issue> issues = Wire.validate(spec.success(), wire, "$", false);
      if (!issues.isEmpty()) {
        throw new IllegalStateException(spec.name() + ": message failed validation: " + issues);
      }
    }
    // #if rules
    if (rulesHooks != null) {
      try {
        RuleWalk.checkItem(spec, rulesHooks, wire);
      } catch (RuleWalk.Rejected rejected) {
        fail(Router.problem(std.internal(), "Internal", 500, spec.sig(), Json.object().build()));
        throw rejected;
      }
    }
    // #endif
    socket.send("{\"data\":" + Wire.canonicalOf(spec.success(), wire) + "}");
  }

  @Override
  public void close() {
    finish(1000, "closed");
  }

  @Override
  public boolean isClosed() {
    return closed;
  }
}
