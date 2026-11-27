package veldtest;

import gen.b.bookings.bookings.Booking;
import gen.b.bookings.bookings.BookingCancelled;
import gen.b.bookings.bookings.BookingPaid;
import gen.b.bookings.bookings.RefundDue;
import gen.b.bookings.bookings.Status;
import gen.b.bookings.server.BookingsFactLoaders;
import gen.b.bookings.server.BookingsHandler;
import gen.b.bookings.server.BookingsHooks;
import gen.b.bookings.server.BookingsOutboxRelay;
import gen.b.bookings.server.BookingsService;
import gen.b.bookings.server.CancelEmit;
import gen.b.bookings.server.CancelFacts;
import gen.b.bookings.server.CancelRequest;
import gen.b.bookings.server.NotificationsEventHandlers;
import gen.b.bookings.server.NotificationsSubscriber;
import gen.b.bookings.server.PayEmit;
import gen.b.bookings.server.PayFacts;
import gen.b.bookings.server.PayRequest;
import gen.b.bookings.server.RefundFacts;
import gen.b.bookings.server.RefundRequest;
import gen.b.prelude.InvalidTransition;
import gen.b.prelude.ValidationError;
import gen.b.runtime.Events;
import gen.b.runtime.Json;
import gen.b.runtime.MemoryOutbox;
import gen.b.runtime.MemoryTransport;
import gen.b.runtime.OutboxEntry;
import gen.b.runtime.RequestContext;
import gen.b.runtime.Transition;
import gen.b.runtime.Transport;
import gen.b.runtime.VeldHandler;
import gen.b.runtime.VeldRequest;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicReference;

/**
 * The outbox over the production handler (spec/events.md section 11, spec/targets/java.md 12.6): the
 * moves of {@code tests/events/outbox.veld.pending} (compiled as the package {@code bookings}) through
 * {@code BookingsHandler.create}, then the relay with the memory transport and the subscriber of
 * {@code Notifications}, all in one process. Port of {@code events_outbox_http.ts}. Prints {@code OK}.
 *
 * <p>What it checks: a handler that writes its event (the entry is in the outbox, nothing was
 * published), one that forgets (500 and {@code onError}), one that writes twice, one that writes an
 * event of another edge, a handler that answers a declared error and writes nothing, an invalid event
 * ({@code emit} throws the typed error and writes nothing), a move that emits nothing (no {@code
 * emit}), a server created without an outbox, and the relay: the subscriber gets the event, a second
 * run does not publish twice, an entry whose publish fails stays pending and stops the run.
 */
public final class OutboxHttp {
  private OutboxHttp() {}

  private static final UUID BOOKING = UUID.fromString("5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21");

  private enum Mode { RIGHT, FORGET, TWICE, BOTH, DECLARED, INVALID }

  private static volatile Status state = Status.PENDING;
  private static volatile Mode mode = Mode.RIGHT;
  private static final AtomicReference<Throwable> INVALID_ERROR = new AtomicReference<>();
  private static final List<Throwable> ERRORS = new CopyOnWriteArrayList<>();

  private static Booking booking(Status status) {
    return new Booking(BOOKING, status, new BigDecimal("120.00"));
  }

  private static BookingPaid paid() {
    return new BookingPaid(BOOKING, new BigDecimal("120.00"));
  }

  private static final class Impl implements BookingsService {
    @Override
    public Booking pay(PayRequest request, PayFacts facts, Transition<Status> transition, PayEmit emit, RequestContext ctx)
        throws Exception {
      switch (mode) {
        case RIGHT -> emit.bookingPaid(paid());
        case TWICE -> {
          emit.bookingPaid(paid());
          emit.bookingPaid(paid());
        }
        case DECLARED -> throw new InvalidTransition(
            new InvalidTransition.Data(transition.field(), transition.from().wire(), transition.to().wire()));
        case INVALID -> {
          try {
            emit.bookingPaid(new BookingPaid(BOOKING, new BigDecimal("-1.00")));
          } catch (ValidationError e) {
            INVALID_ERROR.set(e);
          }
        }
        case FORGET, BOTH -> { }
      }
      return booking(Status.PAID);
    }

    @Override
    public Booking cancel(CancelRequest request, CancelFacts facts, Transition<Status> transition, CancelEmit emit, RequestContext ctx)
        throws Exception {
      if (mode == Mode.BOTH) {
        // The edge from `paid` emits RefundDue; BookingCancelled belongs to the other edge.
        emit.refundDue(new RefundDue(BOOKING));
        emit.bookingCancelled(new BookingCancelled(BOOKING, null));
      } else if (transition.from() == Status.PAID) {
        emit.refundDue(new RefundDue(BOOKING));
      } else {
        emit.bookingCancelled(new BookingCancelled(BOOKING, null));
      }
      return booking(Status.CANCELLED);
    }

    @Override
    public Booking refund(RefundRequest request, RefundFacts facts, Transition<Status> transition, RequestContext ctx) {
      return booking(Status.REFUNDED);
    }
  }

  private static final BookingsFactLoaders LOADERS = new BookingsFactLoaders() {
    @Override
    public Optional<Booking> booking(UUID key, RequestContext ctx) {
      return Optional.of(OutboxHttp.booking(state));
    }
  };

  private static VeldHandler handlerWith(MemoryOutbox outbox) {
    BookingsHooks hooks = new BookingsHooks(outbox)
        .events(new Events.PublisherOptions().newId(() -> "fixed-id").clock(() -> Instant.parse("2026-06-10T12:00:00Z")))
        .onError((error, ctx) -> ERRORS.add(error));
    return BookingsHandler.create(new Impl(), LOADERS, hooks);
  }

  private static int status(VeldHandler handler, String action) {
    return handler
        .handle(new VeldRequest("POST", "/bookings/" + BOOKING + "/" + action, "", Map.of(), null, null))
        .orElseThrow(() -> new AssertionError("no route " + action))
        .status();
  }

  private static void reset() {
    ERRORS.clear();
    INVALID_ERROR.set(null);
  }

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new AssertionError(what);
    }
  }

  private static Json.Obj decode(OutboxEntry entry) {
    return Json.parse(new String(entry.bytes(), java.nio.charset.StandardCharsets.UTF_8)).asObject();
  }

  private static String message(Throwable e) {
    return String.valueOf(e.getMessage());
  }

  public static void main(String[] args) throws Exception {
    // A handler that writes its event: the entry is in the outbox, with the topic, the key and the
    // envelope; nothing was published (there is no transport to publish to).
    reset();
    state = Status.PENDING;
    mode = Mode.RIGHT;
    MemoryOutbox outbox = new MemoryOutbox();
    VeldHandler handler = handlerWith(outbox);
    check(status(handler, "pay") == 201, "pay");
    List<OutboxEntry> pending = outbox.pending(100);
    check(pending.size() == 1, "one entry: " + pending.size());
    check(pending.get(0).topic().equals("shop.bookings.v1"), "topic " + pending.get(0).topic());
    check(BOOKING.toString().equals(pending.get(0).key()), "key " + pending.get(0).key());
    Json.Obj envelope = decode(pending.get(0));
    check(envelope.get("id").asString().equals("fixed-id"), "id");
    check(envelope.get("time").asString().equals("2026-06-10T12:00:00Z"), "time");
    check(envelope.get("source").asString().equals("veld:bookings/Bookings"), "source");
    check(envelope.get("type").asString().startsWith("veld:bookings/BookingPaid@"), "type");
    check(ERRORS.isEmpty(), "no error: " + ERRORS);

    // A handler that forgets: the state changed without its event, 500, reported to `onError`.
    reset();
    mode = Mode.FORGET;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    check(status(handler, "pay") == 500, "forgot: 500");
    check(ERRORS.size() == 1 && message(ERRORS.get(0)).contains("missing BookingPaid"), "reported: " + ERRORS);
    check(outbox.size() == 0, "nothing written");

    // The event twice.
    reset();
    mode = Mode.TWICE;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    check(status(handler, "pay") == 500, "twice: 500");
    check(message(ERRORS.get(0)).contains("twice BookingPaid"), "reported: " + ERRORS);

    // An event of another edge: from `paid` the move to `cancelled` emits RefundDue, not BookingCancelled.
    reset();
    state = Status.PAID;
    mode = Mode.BOTH;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    check(status(handler, "cancel") == 500, "another edge: 500");
    check(message(ERRORS.get(0)).contains("unexpected BookingCancelled"), "reported: " + ERRORS);

    // The same move written right: the edge taken decides (RefundDue from `paid`, BookingCancelled from `pending`).
    reset();
    mode = Mode.RIGHT;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    check(status(handler, "cancel") == 201, "cancel from paid");
    check(decode(outbox.pending(1).get(0)).get("type").asString().contains("RefundDue@"), "RefundDue");
    check(outbox.pending(1).get(0).topic().equals("shop.refunds.v1"), "the topic of the other channel");
    state = Status.PENDING;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    check(status(handler, "cancel") == 201, "cancel from pending");
    check(decode(outbox.pending(1).get(0)).get("type").asString().contains("BookingCancelled@"), "BookingCancelled");
    check(ERRORS.isEmpty(), "no error: " + ERRORS);

    // A handler that answers a declared error need not have written.
    reset();
    state = Status.PENDING;
    mode = Mode.DECLARED;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    check(status(handler, "pay") == 409, "declared error: 409");
    check(ERRORS.isEmpty(), "and no onError: " + ERRORS);
    check(outbox.size() == 0, "nothing written");

    // A move refused by the guard never reaches the handler.
    state = Status.PAID;
    mode = Mode.RIGHT;
    check(status(handler, "pay") == 409, "refused by the guard");

    // An invalid event: `emit` throws the typed error, nothing is written.
    reset();
    state = Status.PENDING;
    mode = Mode.INVALID;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    status(handler, "pay");
    check(INVALID_ERROR.get() instanceof ValidationError, "the typed error: " + INVALID_ERROR.get());
    check(outbox.size() == 0, "nothing written");

    // A move that emits nothing has no `emit` and is not checked.
    reset();
    state = Status.PAID;
    mode = Mode.RIGHT;
    handler = handlerWith(new MemoryOutbox());
    check(status(handler, "refund") == 201, "refund");

    // A server created without an outbox fails when it is created.
    try {
      BookingsHandler.create(new Impl(), LOADERS, new BookingsHooks(null));
      throw new AssertionError("a server without an outbox was created");
    } catch (IllegalStateException e) {
      check(e.getMessage().contains("hooks.outbox is required"), e.getMessage());
    }

    // --- end to end: the relay publishes, a subscriber's handler gets the event -------------------
    reset();
    state = Status.PENDING;
    mode = Mode.RIGHT;
    outbox = new MemoryOutbox();
    handler = handlerWith(outbox);
    MemoryTransport transport = new MemoryTransport();
    List<String> received = new CopyOnWriteArrayList<>();
    NotificationsSubscriber.subscribe(transport, new NotificationsEventHandlers() {
      @Override
      public void sendReceipt(BookingPaid event, Events.EventMeta meta) {
        received.add(event.amount().toPlainString() + "/" + meta.id());
      }

      @Override
      public void sendApology(BookingCancelled event, Events.EventMeta meta) { }
    });
    check(status(handler, "pay") == 201, "pay again");
    check(received.isEmpty(), "nothing is published until the relay runs");
    check(BookingsOutboxRelay.run(outbox, transport) == 1, "the relay sent one");
    transport.idle();
    check(received.equals(List.of("120.00/fixed-id")), "the subscriber got the event: " + received);
    check(outbox.size() == 0, "and the entry is done");
    // Run twice: nothing is published twice.
    check(BookingsOutboxRelay.run(outbox, transport) == 0, "the second run sends nothing");
    transport.idle();
    check(received.size() == 1, "nothing twice");

    // An entry whose publish fails stays pending, and stops the run (the next entry is not tried).
    mode = Mode.RIGHT;
    state = Status.PENDING;
    status(handler, "pay");
    status(handler, "pay");
    check(outbox.size() == 2, "two entries: " + outbox.size());
    List<String> failures = new ArrayList<>();
    int[] attempts = {0};
    Transport refusing = new Transport() {
      @Override
      public void publish(String topic, String key, byte[] bytes) {
        attempts[0] += 1;
        throw new IllegalStateException("broker down");
      }

      @Override
      public Subscription subscribe(String topic, String group, Deliver deliver) {
        return transport.subscribe(topic, group, deliver);
      }
    };
    int sent = BookingsOutboxRelay.run(
        outbox, refusing, new Events.RelayOptions().onError((e, entry) -> failures.add(e.getMessage() + "/" + entry.id())));
    check(sent == 0, "nothing sent");
    check(attempts[0] == 1, "the run stops at the first failure: " + attempts[0]);
    check(outbox.size() == 2, "both still pending");
    check(failures.equals(List.of("broker down/" + outbox.pending(1).get(0).id())), "reported: " + failures);
    check(BookingsOutboxRelay.run(outbox, transport, new Events.RelayOptions().limit(1)) == 1, "the limit bounds the run");
    check(outbox.size() == 1, "one left");
    check(BookingsOutboxRelay.run(outbox, transport) == 1, "and the last one");
    transport.idle();
    check(received.size() == 3, "three events in all: " + received);
    System.out.println("OK");
  }
}
