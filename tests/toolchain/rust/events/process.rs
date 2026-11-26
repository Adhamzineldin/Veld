//! Two generated services in one process over the memory transport: `Bookings` publishes,
//! `Notifications` and `Ledger` subscribe (tests/events/bookings.veld). Redelivery when a handler
//! fails, two groups that each get the message, competing consumers of one group, ordering per
//! key, and what an invalid event does. Prints `events process ok`. No async runtime: every future
//! is driven by `veld::block_on`.
#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use common::*;
use events_server::bookings::bookings_events::create_bookings_publisher;
use events_server::bookings::ledger_events::{LedgerEventHandlers, subscribe_ledger_events};
use events_server::bookings::notifications_events::{
    NotificationsEventHandlers, subscribe_notifications_events,
};
use events_server::bookings::t::{BookingCancelled, BookingPaid, Reason};
use events_server::veld::events::{
    BoxFuture, ConsumerOptions, EventMeta, MemoryTransport, PublisherOptions, Transport,
};
use events_server::veld::{Error, block_on};

const A: &str = "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21";
const B: &str = "7e6f0a52-3d84-4a4b-9c27-0f8e2b1d6a90";

fn paid(id: &str, amount: &str, nights: i32) -> BookingPaid {
    BookingPaid {
        booking_id: id.to_string(),
        amount: amount.to_string(),
        paid_at: "2026-06-10T11:59:58Z".to_string(),
        nights,
    }
}

type Done = BoxFuture<'static, Result<(), Error>>;
type PaidFn = Box<dyn Fn(BookingPaid, EventMeta) -> Done + Send + Sync>;
type CancelledFn = Box<dyn Fn(BookingCancelled, EventMeta) -> Done + Send + Sync>;

fn ok() -> Done {
    Box::pin(std::future::ready(Ok(())))
}

struct Notify {
    receipt: PaidFn,
    apology: CancelledFn,
}

impl NotificationsEventHandlers for Notify {
    fn send_receipt(&self, event: BookingPaid, meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        (self.receipt)(event, meta)
    }

    fn send_apology(&self, event: BookingCancelled, meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        (self.apology)(event, meta)
    }
}

struct Ledger {
    record: PaidFn,
    audit: PaidFn,
}

impl LedgerEventHandlers for Ledger {
    fn record_payment(&self, event: BookingPaid, meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        (self.record)(event, meta)
    }

    fn audit_payment(&self, event: BookingPaid, meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        (self.audit)(event, meta)
    }
}

fn notify(receipt: PaidFn) -> Arc<Notify> {
    Arc::new(Notify { receipt, apology: Box::new(|_, _| ok()) })
}

fn ledger(record: PaidFn) -> Arc<Ledger> {
    Arc::new(Ledger { record, audit: Box::new(|_, _| ok()) })
}

type Log<T> = Arc<Mutex<Vec<T>>>;

fn log<T>() -> Log<T> {
    Arc::new(Mutex::new(Vec::new()))
}

fn options() -> PublisherOptions {
    PublisherOptions::new().new_id(counter()).clock(|| time_from("2026-06-10T12:00:00Z"))
}

fn two_groups_each_get_the_message() {
    let transport = Arc::new(MemoryTransport::new());
    let heard: Log<String> = log();
    let metas: Log<EventMeta> = log();
    let (h, m) = (heard.clone(), metas.clone());
    let notifications = Arc::new(Notify {
        receipt: Box::new(move |e, meta| {
            h.lock().unwrap().push(format!("receipt {}", e.booking_id));
            m.lock().unwrap().push(meta);
            ok()
        }),
        apology: Box::new(|_, _| ok()),
    });
    let (h, h2) = (heard.clone(), heard.clone());
    let ledger = Arc::new(Ledger {
        record: Box::new(move |e, _| {
            h.lock().unwrap().push(format!("ledger {}", e.booking_id));
            ok()
        }),
        audit: Box::new(move |_, _| {
            h2.lock().unwrap().push("audit".to_string());
            ok()
        }),
    });
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_notifications_events(t.clone(), notifications, ConsumerOptions::new()).await.unwrap();
        subscribe_ledger_events(t.clone(), ledger, ConsumerOptions::new()).await.unwrap();
        let bookings = create_bookings_publisher(t, options());
        let sent = bookings.booking_events().booking_paid(&paid(A, "120.00", 1)).await.unwrap();
        // Publishing only queues: nothing was handled inside the call.
        assert!(heard.lock().unwrap().is_empty(), "handlers never run inside publish");
        transport.idle().await;
        let mut got = heard.lock().unwrap().clone();
        got.sort();
        assert_eq!(got, vec![format!("ledger {A}"), format!("receipt {A}")]);
        assert_eq!(sent.id, "id-1");
        assert_eq!(sent.subject.as_deref(), Some(A));
        let metas = metas.lock().unwrap();
        assert_eq!(metas[0].id, "id-1");
        assert_eq!(metas[0].source, "veld:bookings/Bookings");
        assert_eq!(metas[0].r#type, sent.r#type);
        assert_eq!(metas[0].subject.as_deref(), Some(A));
        assert_eq!(metas[0].time.as_deref(), Some("2026-06-10T12:00:00Z"));
        assert_eq!(metas[0].attempt, 1);
        // The other channel is another topic and another group.
        bookings.audit().booking_paid(&paid(B, "1.00", 1)).await.unwrap();
        transport.idle().await;
        let mut got = heard.lock().unwrap().clone();
        got.sort();
        assert_eq!(got, vec!["audit".to_string(), format!("ledger {A}"), format!("receipt {A}")]);
        assert!(transport.dead_letters().is_empty());
    });
}

fn a_failing_handler_gets_the_message_again() {
    let transport = Arc::new(MemoryTransport::new());
    let attempts: Log<u32> = log();
    let failures: Log<String> = log();
    let receipts: Log<String> = log();
    let a = attempts.clone();
    let r = receipts.clone();
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_notifications_events(
            t.clone(),
            notify(Box::new(move |e, _| {
                r.lock().unwrap().push(e.booking_id);
                ok()
            })),
            ConsumerOptions::new(),
        )
        .await
        .unwrap();
        let f = failures.clone();
        subscribe_ledger_events(
            t.clone(),
            ledger(Box::new(move |_, meta| {
                a.lock().unwrap().push(meta.attempt);
                let attempt = meta.attempt;
                Box::pin(async move {
                    if attempt < 3 { Err(Error::other(format!("not yet ({attempt})"))) } else { Ok(()) }
                })
            })),
            ConsumerOptions::new().on_handler_error(move |failure| f.lock().unwrap().push(failure.error.to_string())),
        )
        .await
        .unwrap();
        create_bookings_publisher(t, PublisherOptions::new()).booking_events().booking_paid(&paid(A, "1.00", 1)).await.unwrap();
        transport.idle().await;
        assert_eq!(*attempts.lock().unwrap(), vec![1, 2, 3], "delivered again at once, with the attempt counted");
        assert_eq!(failures.lock().unwrap().len(), 2);
        assert_eq!(*receipts.lock().unwrap(), vec![A.to_string()], "the other group is not delivered again");
        assert!(transport.dead_letters().is_empty());
    });
}

fn a_message_that_never_succeeds_is_dead_lettered() {
    let transport = Arc::new(MemoryTransport::with_max_attempts(2));
    let calls: Log<u32> = log();
    let c = calls.clone();
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_ledger_events(
            t.clone(),
            ledger(Box::new(move |_, meta| {
                c.lock().unwrap().push(meta.attempt);
                Box::pin(async { Err(Error::other("always")) })
            })),
            ConsumerOptions::new(),
        )
        .await
        .unwrap();
        create_bookings_publisher(t, PublisherOptions::new()).booking_events().booking_paid(&paid(A, "1.00", 1)).await.unwrap();
        transport.idle().await;
        assert_eq!(calls.lock().unwrap().len(), 2);
        let dead = transport.dead_letters();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].group, "ledger");
        assert_eq!(dead[0].topic, "shop.bookings.v1");
        assert_eq!(dead[0].attempts, 2);
        assert_eq!(dead[0].key.as_deref(), Some(A));
    });
}

fn a_panicking_handler_is_a_failed_handler() {
    let transport = Arc::new(MemoryTransport::with_max_attempts(2));
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_ledger_events(t.clone(), ledger(Box::new(|_, _| panic!("boom"))), ConsumerOptions::new()).await.unwrap();
        create_bookings_publisher(t, PublisherOptions::new()).booking_events().booking_paid(&paid(A, "1.00", 1)).await.unwrap();
        transport.idle().await;
        assert_eq!(transport.dead_letters().len(), 1);
    });
}

fn order_per_key_holds() {
    let transport = Arc::new(MemoryTransport::new());
    let seen: Log<String> = log();
    let s = seen.clone();
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_ledger_events(
            t.clone(),
            ledger(Box::new(move |e, _| {
                let s = s.clone();
                Box::pin(async move {
                    yield_now().await;
                    s.lock().unwrap().push(format!("{}:{}", &e.booking_id[..4], e.nights));
                    Ok(())
                })
            })),
            ConsumerOptions::new(),
        )
        .await
        .unwrap();
        let bookings = create_bookings_publisher(t, PublisherOptions::new());
        for n in 1..=6 {
            let id = if n % 2 == 0 { A } else { B };
            bookings.booking_events().booking_paid(&paid(id, "1.00", n)).await.unwrap();
        }
        transport.idle().await;
        let seen = seen.lock().unwrap();
        let of = |id: &str| seen.iter().filter(|x| x.starts_with(&id[..4])).cloned().collect::<Vec<_>>();
        assert_eq!(of(A), vec!["5d2b:2", "5d2b:4", "5d2b:6"]);
        assert_eq!(of(B), vec!["7e6f:1", "7e6f:3", "7e6f:5"]);
        assert_eq!(seen.len(), 6);
    });
}

fn competing_consumers_share_the_messages() {
    let transport = Arc::new(MemoryTransport::new());
    let (one, two): (Log<String>, Log<String>) = (log(), log());
    let (o, w) = (one.clone(), two.clone());
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        let first = subscribe_notifications_events(
            t.clone(),
            notify(Box::new(move |e, _| {
                o.lock().unwrap().push(e.booking_id);
                ok()
            })),
            ConsumerOptions::new(),
        )
        .await
        .unwrap();
        subscribe_notifications_events(
            t.clone(),
            notify(Box::new(move |e, _| {
                w.lock().unwrap().push(e.booking_id);
                ok()
            })),
            ConsumerOptions::new(),
        )
        .await
        .unwrap();
        let bookings = create_bookings_publisher(t, PublisherOptions::new());
        for n in 1..=4 {
            bookings.booking_events().booking_paid(&paid(A, "1.00", n)).await.unwrap();
        }
        transport.idle().await;
        let (a, b) = (one.lock().unwrap().len(), two.lock().unwrap().len());
        assert_eq!(a + b, 4, "each message is handled by one copy of the service");
        assert!(a > 0 && b > 0, "both copies get some");
        first.close().await;
        bookings.booking_events().booking_paid(&paid(A, "1.00", 1)).await.unwrap();
        transport.idle().await;
        assert_eq!(one.lock().unwrap().len() + two.lock().unwrap().len(), 5);
        assert_eq!(one.lock().unwrap().len(), a);
    });
}

fn a_topic_nobody_subscribed_to_drops_the_message() {
    let transport = Arc::new(MemoryTransport::new());
    let seen: Log<String> = log();
    let s = seen.clone();
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        let bookings = create_bookings_publisher(t.clone(), PublisherOptions::new());
        bookings.booking_events().booking_paid(&paid(A, "1.00", 1)).await.unwrap();
        transport.idle().await;
        let handle = subscribe_ledger_events(
            t,
            ledger(Box::new(move |e, _| {
                s.lock().unwrap().push(e.booking_id);
                ok()
            })),
            ConsumerOptions::new(),
        )
        .await
        .unwrap();
        transport.idle().await;
        assert!(seen.lock().unwrap().is_empty(), "no retention and no replay");
        handle.close().await;
        bookings.booking_events().booking_paid(&paid(A, "1.00", 1)).await.unwrap();
        transport.idle().await;
        assert!(seen.lock().unwrap().is_empty(), "a closed subscription hears nothing");
    });
}

fn an_invalid_event_never_leaves_the_publisher() {
    let recorder = Recorder::new();
    let heard: Log<(String, String, String)> = log();
    let h = heard.clone();
    let bookings = create_bookings_publisher(
        recorder.clone(),
        PublisherOptions::new().on_rule_violation(move |e| {
            h.lock().unwrap().push((format!("{:?}", e.outcome), format!("{:?}", e.position), e.operation.clone()));
        }),
    );
    block_on(async {
        // An invariant: amount > 0.
        let err = bookings.booking_events().booking_paid(&paid(A, "0.00", 1)).await.unwrap_err();
        assert_eq!(err.problem().unwrap().title, "ValidationError");
        assert!(err.is_local());
        assert_eq!(err.violations().len(), 1);
        assert_eq!(err.violations()[0].rule, BookingPaid::RULE_POSITIVE);
        assert_eq!(err.violations()[0].path, "$");
        assert_eq!(
            *heard.lock().unwrap(),
            vec![("Rejected".to_string(), "Output".to_string(), BookingPaid::VELD_SIG.to_string())]
        );
        // The shape: a field of the wrong type.
        let mut bad = paid(A, "1.00", 1);
        bad.paid_at = "yesterday".to_string();
        let shape = bookings.booking_events().booking_paid(&bad).await.unwrap_err();
        assert_eq!(shape.problem().unwrap().title, "ValidationError");
        // The invariant of an optional field.
        let note = BookingCancelled { booking_id: A.to_string(), reason: Reason::Guest, note: Some(String::new()) };
        assert!(bookings.booking_events().booking_cancelled(&note).await.is_err());
        assert!(recorder.messages().is_empty(), "nothing was sent");
        let fine = BookingCancelled { note: Some("x".to_string()), ..note };
        bookings.booking_events().booking_cancelled(&fine).await.unwrap();
        assert_eq!(recorder.messages().len(), 1);
    });
}

fn a_rejected_message_is_acknowledged_and_reported() {
    let transport = Arc::new(MemoryTransport::new());
    let rejected: Log<(String, Option<String>)> = log();
    let called: Log<String> = log();
    let (r, c) = (rejected.clone(), called.clone());
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_ledger_events(
            t.clone(),
            ledger(Box::new(move |_, _| {
                c.lock().unwrap().push("record".to_string());
                ok()
            })),
            ConsumerOptions::new().on_event_rejected(move |e| r.lock().unwrap().push((e.reason.as_str().to_string(), e.event.clone()))),
        )
        .await
        .unwrap();
        let message = |data: &str| {
            format!(
                "{{\"specversion\":\"1.0\",\"id\":\"x\",\"source\":\"s\",\"type\":\"{}\",\"veldrules\":\"0018ccc5f17d6c76\",\"data\":{data}}}",
                BookingPaid::VELD_SIG
            )
        };
        let topic = "shop.bookings.v1";
        let broken = message(&format!("{{\"bookingId\":\"{A}\",\"amount\":\"0.00\",\"paidAt\":\"2026-06-10T11:59:58Z\"}}"));
        t.publish(topic, None, broken.as_bytes()).await.unwrap();
        t.publish(topic, None, message("{\"amount\":\"1.00\"}").as_bytes()).await.unwrap();
        t.publish(topic, None, b"{").await.unwrap();
        transport.idle().await;
        let got = rejected.lock().unwrap().clone();
        assert_eq!(
            got,
            vec![
                ("rule".to_string(), Some("BookingPaid".to_string())),
                ("shape".to_string(), Some("BookingPaid".to_string())),
                ("malformed".to_string(), None),
            ]
        );
        assert!(called.lock().unwrap().is_empty());
        assert!(transport.dead_letters().is_empty(), "acknowledged, not dead-lettered");
    });
}

fn main() {
    two_groups_each_get_the_message();
    a_failing_handler_gets_the_message_again();
    a_message_that_never_succeeds_is_dead_lettered();
    a_panicking_handler_is_a_failed_handler();
    order_per_key_holds();
    competing_consumers_share_the_messages();
    a_topic_nobody_subscribed_to_drops_the_message();
    an_invalid_event_never_leaves_the_publisher();
    a_rejected_message_is_acknowledged_and_reported();
    println!("events process ok");
}
