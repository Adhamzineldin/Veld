package veldtest;

import gen.e.bookings.server.BookingsPublisher;
import gen.e.bookings.server.LedgerEventHandlers;
import gen.e.bookings.server.LedgerSubscriber;
import gen.e.bookings.server.NotificationsEventHandlers;
import gen.e.bookings.server.NotificationsSubscriber;
import gen.e.bookings.t.BookingCancelled;
import gen.e.bookings.t.BookingPaid;
import gen.e.bookings.t.Reason;
import gen.e.runtime.Events;
import gen.e.runtime.MemoryTransport;
import gen.e.runtime.RuleWalk;
import gen.e.runtime.Transport;
import gen.e.runtime.VeldError;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Two generated services in one process over the memory transport: Bookings publishes,
 * Notifications and Ledger subscribe (tests/events/bookings.veld). Redelivery when a handler
 * throws, two groups that each get the message, competing consumers of one group, ordering per key,
 * and what an invalid event does. Prints {@code events process ok}.
 */
public final class EventsProcess {
  private EventsProcess() {}

  private static final UUID A = UUID.fromString("5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21");
  private static final UUID B = UUID.fromString("7e6f0a52-3d84-4a4b-9c27-0f8e2b1d6a90");
  private static final Instant NOON = Instant.parse("2026-06-10T12:00:00Z");

  private static void check(boolean ok, String what) {
    if (!ok) {
      throw new IllegalStateException("FAILED: " + what);
    }
  }

  private static void same(Object want, Object got, String what) {
    check(want.equals(got), what + ": wanted " + want + ", got " + got);
  }

  private static BookingPaid paid(UUID id, String amount, int nights) {
    return new BookingPaid(id, new BigDecimal(amount), Instant.parse("2026-06-10T11:59:58Z"), nights);
  }

  private static Events.PublisherOptions counted() {
    AtomicInteger n = new AtomicInteger();
    return new Events.PublisherOptions().newId(() -> "id-" + n.incrementAndGet()).clock(() -> NOON);
  }

  /** Notifications with a receipt callback and a quiet apology. */
  private static NotificationsEventHandlers notifications(List<String> heard, List<Events.EventMeta> metas) {
    return new NotificationsEventHandlers() {
      @Override
      public void sendReceipt(BookingPaid event, Events.EventMeta meta) {
        heard.add("receipt " + event.bookingId());
        metas.add(meta);
      }

      @Override
      public void sendApology(BookingCancelled event, Events.EventMeta meta) {
        heard.add("apology");
      }
    };
  }

  private static LedgerEventHandlers ledger(List<String> heard) {
    return new LedgerEventHandlers() {
      @Override
      public void recordPayment(BookingPaid event, Events.EventMeta meta) {
        heard.add("ledger " + event.bookingId());
      }

      @Override
      public void auditPayment(BookingPaid event, Events.EventMeta meta) {
        heard.add("audit");
      }
    };
  }

  private static void twoGroupsEachGetTheMessage() throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> heard = new CopyOnWriteArrayList<>();
    List<Events.EventMeta> metas = new CopyOnWriteArrayList<>();
    NotificationsSubscriber.subscribe(transport, notifications(heard, metas));
    LedgerSubscriber.subscribe(transport, ledger(heard));
    BookingsPublisher bookings = BookingsPublisher.create(transport, counted());
    Events.PublishedEvent sent = bookings.bookingEvents().bookingPaid(paid(A, "120.00", 1));
    transport.idle();
    List<String> sorted = new ArrayList<>(heard);
    Collections.sort(sorted);
    same(List.of("ledger " + A, "receipt " + A), sorted, "both groups hear it");
    same("id-1", sent.id(), "the id");
    same(A.toString(), sent.subject(), "the subject");
    Events.EventMeta m = metas.get(0);
    same("id-1", m.id(), "meta.id");
    same("veld:bookings/Bookings", m.source(), "meta.source");
    same(BookingPaid.VELD_SIG, m.type(), "meta.type");
    same(A.toString(), m.subject(), "meta.subject");
    same("2026-06-10T12:00:00Z", m.time(), "meta.time");
    same(1, m.attempt(), "meta.attempt");
    // The other channel is another topic and another group.
    bookings.audit().bookingPaid(paid(B, "1.00", 1));
    transport.idle();
    same(3, heard.size(), "the audit channel reaches the ledger audit subscription only");
    check(heard.contains("audit"), "audit");
    same(0, transport.deadLetters().size(), "no dead letters");
  }

  private static void aHandlerThatThrowsGetsTheMessageAgain() throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<Integer> attempts = new CopyOnWriteArrayList<>();
    List<Events.HandlerFailure> failures = new CopyOnWriteArrayList<>();
    List<String> receipts = new CopyOnWriteArrayList<>();
    NotificationsSubscriber.subscribe(
        transport,
        new NotificationsEventHandlers() {
          @Override
          public void sendReceipt(BookingPaid event, Events.EventMeta meta) {
            receipts.add(event.bookingId().toString());
          }

          @Override
          public void sendApology(BookingCancelled event, Events.EventMeta meta) {}
        });
    LedgerSubscriber.subscribe(
        transport,
        new LedgerEventHandlers() {
          @Override
          public void recordPayment(BookingPaid event, Events.EventMeta meta) throws Exception {
            attempts.add(meta.attempt());
            if (meta.attempt() < 3) {
              throw new Exception("not yet (" + meta.attempt() + ")");
            }
          }

          @Override
          public void auditPayment(BookingPaid event, Events.EventMeta meta) {}
        },
        new Events.ConsumerOptions().onHandlerError(failures::add));
    BookingsPublisher.create(transport).bookingEvents().bookingPaid(paid(A, "120.00", 1));
    transport.idle();
    same(List.of(1, 2, 3), attempts, "at least once: again at once, with the attempt counted");
    same(2, failures.size(), "two failures reported");
    same("BookingPaid", failures.get(0).event(), "the failure names the event");
    same(List.of(A.toString()), receipts, "the other group is not delivered again");
    same(0, transport.deadLetters().size(), "no dead letters");
  }

  private static void aMessageThatNeverSucceedsIsDeadLettered() throws Exception {
    MemoryTransport transport = new MemoryTransport(2);
    AtomicInteger calls = new AtomicInteger();
    LedgerSubscriber.subscribe(
        transport,
        new LedgerEventHandlers() {
          @Override
          public void recordPayment(BookingPaid event, Events.EventMeta meta) throws Exception {
            calls.incrementAndGet();
            throw new Exception("always");
          }

          @Override
          public void auditPayment(BookingPaid event, Events.EventMeta meta) {}
        });
    BookingsPublisher.create(transport).bookingEvents().bookingPaid(paid(A, "120.00", 1));
    transport.idle();
    same(2, calls.get(), "two attempts");
    List<MemoryTransport.DeadLetter> dead = transport.deadLetters();
    same(1, dead.size(), "one dead letter");
    same("ledger", dead.get(0).group(), "its group");
    same("shop.bookings.v1", dead.get(0).topic(), "its topic");
    same(2, dead.get(0).attempts(), "its attempts");
    same(A.toString(), dead.get(0).key(), "its key");
  }

  private static void orderPerKeyHolds() throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> seen = new CopyOnWriteArrayList<>();
    LedgerSubscriber.subscribe(
        transport,
        new LedgerEventHandlers() {
          @Override
          public void recordPayment(BookingPaid event, Events.EventMeta meta) throws Exception {
            Thread.sleep(3);
            seen.add(event.bookingId().toString().substring(0, 4) + ":" + event.nights());
          }

          @Override
          public void auditPayment(BookingPaid event, Events.EventMeta meta) {}
        });
    BookingsPublisher bookings = BookingsPublisher.create(transport);
    for (int n = 1; n <= 6; n++) {
      bookings.bookingEvents().bookingPaid(paid(n % 2 == 0 ? A : B, "1.00", n));
    }
    transport.idle();
    same(List.of("5d2b:2", "5d2b:4", "5d2b:6"), of(seen, "5d2b"), "order of the first key");
    same(List.of("7e6f:1", "7e6f:3", "7e6f:5"), of(seen, "7e6f"), "order of the second key");
    same(6, seen.size(), "all six");
  }

  private static List<String> of(List<String> seen, String prefix) {
    List<String> out = new ArrayList<>();
    for (String s : seen) {
      if (s.startsWith(prefix)) {
        out.add(s);
      }
    }
    return out;
  }

  private static NotificationsEventHandlers counting(List<String> into) {
    return new NotificationsEventHandlers() {
      @Override
      public void sendReceipt(BookingPaid event, Events.EventMeta meta) {
        into.add(event.bookingId().toString());
      }

      @Override
      public void sendApology(BookingCancelled event, Events.EventMeta meta) {}
    };
  }

  private static void competingConsumersOfOneGroupShareTheMessages() throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> one = new CopyOnWriteArrayList<>();
    List<String> two = new CopyOnWriteArrayList<>();
    Transport.Subscription first = NotificationsSubscriber.subscribe(transport, counting(one));
    NotificationsSubscriber.subscribe(transport, counting(two));
    BookingsPublisher bookings = BookingsPublisher.create(transport);
    for (int n = 0; n < 4; n++) {
      bookings.bookingEvents().bookingPaid(paid(A, "1.00", n + 1));
    }
    transport.idle();
    same(4, one.size() + two.size(), "each message is handled by one copy of the service");
    check(!one.isEmpty() && !two.isEmpty(), "both copies get some");
    first.close();
    bookings.bookingEvents().bookingPaid(paid(A, "1.00", 1));
    transport.idle();
    same(5, one.size() + two.size(), "five in all");
    same(2, one.size(), "the closed copy gets nothing more");
  }

  private static void aTopicNobodySubscribedToDropsTheMessage() throws Exception {
    MemoryTransport transport = new MemoryTransport();
    BookingsPublisher bookings = BookingsPublisher.create(transport);
    bookings.bookingEvents().bookingPaid(paid(A, "1.00", 1));
    transport.idle();
    List<String> seen = new CopyOnWriteArrayList<>();
    Transport.Subscription handle = LedgerSubscriber.subscribe(transport, ledger(seen));
    transport.idle();
    check(seen.isEmpty(), "no retention and no replay");
    handle.close();
    bookings.bookingEvents().bookingPaid(paid(A, "1.00", 1));
    transport.idle();
    check(seen.isEmpty(), "a closed subscription hears nothing");
  }

  private static void anInvalidEventNeverLeavesThePublisher() throws Exception {
    List<byte[]> sent = new CopyOnWriteArrayList<>();
    Transport recording =
        new Transport() {
          @Override
          public void publish(String topic, String key, byte[] bytes) {
            sent.add(bytes);
          }

          @Override
          public Subscription subscribe(String topic, String group, Deliver deliver) {
            return () -> {};
          }
        };
    List<RuleWalk.Event> heard = new CopyOnWriteArrayList<>();
    BookingsPublisher bookings =
        BookingsPublisher.create(recording, new Events.PublisherOptions().onRuleViolation(heard::add));
    // An invariant: amount > 0.
    VeldError err = refused(() -> bookings.bookingEvents().bookingPaid(paid(A, "0.00", 1)));
    same("ValidationError", err.title(), "the title");
    check(err.raisedLocally(), "raised locally");
    same(1, err.violations().size(), "one violation");
    same(BookingPaid.RULE_POSITIVE, err.violations().get(0).rule(), "the rule");
    same("$", err.violations().get(0).path(), "the path");
    same(1, heard.size(), "heard once");
    same(RuleWalk.Outcome.REJECTED, heard.get(0).outcome(), "rejected");
    same(RuleWalk.Position.OUTPUT, heard.get(0).position(), "output");
    same(BookingPaid.VELD_SIG, heard.get(0).operation(), "the operation is the signature");
    // The shape: a required field that is missing.
    VeldError shape =
        refused(() -> bookings.bookingEvents().bookingPaid(new BookingPaid(A, null, NOON, 1)));
    same("ValidationError", shape.title(), "shape");
    // The invariant of an optional field.
    VeldError note =
        refused(
            () -> bookings.bookingEvents().bookingCancelled(new BookingCancelled(A, Reason.GUEST, "")));
    same("ValidationError", note.title(), "note");
    same(0, sent.size(), "nothing was sent");
    bookings.bookingEvents().bookingCancelled(new BookingCancelled(A, Reason.GUEST, "x"));
    same(1, sent.size(), "a valid one is sent");
  }

  private static VeldError refused(Runnable call) {
    try {
      call.run();
    } catch (VeldError e) {
      return e;
    }
    throw new IllegalStateException("the publisher accepted an invalid event");
  }

  private static byte[] message(String data, String veldrules) {
    String text =
        "{\"specversion\":\"1.0\",\"id\":\"x\",\"source\":\"s\",\"type\":\""
            + BookingPaid.VELD_SIG
            + "\",\"veldrules\":\""
            + veldrules
            + "\",\"data\":"
            + data
            + "}";
    return text.getBytes(StandardCharsets.UTF_8);
  }

  private static void aRejectedMessageIsAcknowledgedAndReported() throws Exception {
    MemoryTransport transport = new MemoryTransport();
    List<String> rejected = new CopyOnWriteArrayList<>();
    List<String> events = new CopyOnWriteArrayList<>();
    List<String> called = new CopyOnWriteArrayList<>();
    LedgerSubscriber.subscribe(
        transport,
        new LedgerEventHandlers() {
          @Override
          public void recordPayment(BookingPaid event, Events.EventMeta meta) {
            called.add("record");
          }

          @Override
          public void auditPayment(BookingPaid event, Events.EventMeta meta) {}
        },
        new Events.ConsumerOptions()
            .onEventRejected(
                e -> {
                  rejected.add(e.reason().text());
                  events.add(String.valueOf(e.event()));
                }));
    String digest = "0018ccc5f17d6c76";
    String id = A.toString();
    transport.publish(
        "shop.bookings.v1",
        null,
        message("{\"bookingId\":\"" + id + "\",\"amount\":\"0.00\",\"paidAt\":\"2026-06-10T11:59:58Z\"}", digest));
    transport.publish("shop.bookings.v1", null, message("{\"amount\":\"1.00\"}", digest));
    transport.publish("shop.bookings.v1", null, "{".getBytes(StandardCharsets.UTF_8));
    transport.idle();
    same(List.of("rule", "shape", "malformed"), rejected, "the reasons, in order");
    same("BookingPaid", events.get(0), "the event of a rule rejection");
    same("null", events.get(2), "a malformed message names no event");
    check(called.isEmpty(), "no handler was called");
    same(0, transport.deadLetters().size(), "acknowledged, not dead-lettered");
  }

  public static void main(String[] args) throws Exception {
    twoGroupsEachGetTheMessage();
    aHandlerThatThrowsGetsTheMessageAgain();
    aMessageThatNeverSucceedsIsDeadLettered();
    orderPerKeyHolds();
    competingConsumersOfOneGroupShareTheMessages();
    aTopicNobodySubscribedToDropsTheMessage();
    anInvalidEventNeverLeavesThePublisher();
    aRejectedMessageIsAcknowledgedAndReported();
    System.out.println("events process ok");
  }
}
