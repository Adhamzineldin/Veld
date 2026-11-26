//! A flagged invariant of an event (`spec/platform.md` section 11), from
//! `tests/toolchain/contract/platform_p2/flags_events.veld` compiled as `t.veld` of the package
//! `bookings`: the publisher and the consumer evaluate it with a context that has no principal when
//! the provider of their options says the flag is on; without a provider it is off; a failing
//! provider counts it as on and the failure goes to `on_flag_error`; the provider is asked once per
//! message. Prints `flags events ok`.
use std::collections::BTreeSet;
use std::future::Future;
use std::sync::{Arc, Mutex};

use events_server::bookings::desk_events::create_desk_publisher;
use events_server::bookings::floor_events::{FloorEventHandlers, subscribe_floor_events};
use events_server::bookings::t::Booked;
use events_server::veld::events::{ConsumerOptions, EventMeta, EventRejected, MemoryTransport, PublisherOptions, Reason, Transport};
use events_server::veld::flags::{FlagContext, FlagProvider};
use events_server::veld::{Error, block_on};

const ID: &str = "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21";

/// A provider that records every question and can be told to fail.
#[derive(Default)]
struct Provider {
    on: BTreeSet<String>,
    failing: bool,
    asked: Mutex<Vec<(String, String, Option<String>)>>,
}

impl Provider {
    fn on(names: &[&'static str]) -> Arc<Provider> {
        Arc::new(Provider { on: names.iter().map(|n| (*n).to_string()).collect(), ..Provider::default() })
    }

    fn down() -> Arc<Provider> {
        Arc::new(Provider { failing: true, ..Provider::default() })
    }

    fn asked(&self) -> Vec<(String, String, Option<String>)> {
        self.asked.lock().unwrap().clone()
    }
}

impl FlagProvider for Provider {
    async fn is_on<'a>(&'a self, name: &'a str, context: &'a FlagContext) -> Result<bool, Error> {
        self.asked.lock().unwrap().push((name.to_string(), context.operation.clone(), context.tenant.clone()));
        if self.failing {
            return Err(Error::other("the provider failed"));
        }
        Ok(self.on.contains(name))
    }
}

type Log<T> = Arc<Mutex<Vec<T>>>;

struct Floor {
    seated: Log<i32>,
}

impl FloorEventHandlers for Floor {
    fn seat(&self, event: Booked, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        self.seated.lock().unwrap().push(event.guests);
        std::future::ready(Ok(()))
    }
}

fn booked(guests: i32) -> Booked {
    Booked { id: ID.to_string(), guests }
}

fn refused(result: Result<events_server::veld::events::PublishedEvent, Error>) -> usize {
    match result {
        Err(Error::Problem(p)) => {
            assert_eq!(p.title, "ValidationError");
            p.violations().len()
        }
        other => panic!("expected the ValidationError, got {other:?}"),
    }
}

/// The publisher: no provider (off), a provider that says on (refused, nothing sent), one that says
/// off, one that fails (counts as on, `on_flag_error` hears it).
async fn publisher() {
    let memory = Arc::new(MemoryTransport::new());
    let seated: Log<i32> = Default::default();
    let transport: Arc<dyn Transport> = memory.clone();
    subscribe_floor_events(transport.clone(), Arc::new(Floor { seated: seated.clone() }), ConsumerOptions::new()).await.unwrap();

    let plain = create_desk_publisher(transport.clone(), PublisherOptions::new());
    plain.bookings().booked(&booked(9)).await.unwrap();
    memory.idle().await;
    assert_eq!(*seated.lock().unwrap(), vec![9], "without a provider the flagged invariant is off");

    let on = Provider::on(&["smallGroups"]);
    let strict = create_desk_publisher(transport.clone(), PublisherOptions::new().flags(on.clone()));
    assert_eq!(refused(strict.bookings().booked(&booked(9)).await), 1, "the flag is on: the invariant applies");
    memory.idle().await;
    assert_eq!(*seated.lock().unwrap(), vec![9], "nothing was sent");
    let asked = on.asked();
    assert_eq!(asked.len(), 1, "one reading per event: {asked:?}");
    assert_eq!(asked[0].0, "smallGroups");
    assert!(asked[0].1.starts_with("veld:bookings/Booked@"), "the operation is the signature of the event: {asked:?}");
    assert_eq!(asked[0].2, None, "no tenant");
    strict.bookings().booked(&booked(3)).await.unwrap();
    memory.idle().await;
    assert_eq!(*seated.lock().unwrap(), vec![9, 3], "a value that holds goes through");
    assert_eq!(on.asked().len(), 2);

    let off = create_desk_publisher(transport.clone(), PublisherOptions::new().flags(Provider::on(&[])));
    off.bookings().booked(&booked(9)).await.unwrap();

    let errors: Log<String> = Default::default();
    let heard = errors.clone();
    let broken = create_desk_publisher(
        transport,
        PublisherOptions::new().flags(Provider::down()).on_flag_error(move |e| heard.lock().unwrap().push(e.to_string())),
    );
    assert_eq!(refused(broken.bookings().booked(&booked(9)).await), 1, "a failing provider counts the invariant as on");
    assert_eq!(errors.lock().unwrap().len(), 1, "the failure was heard once");
}

/// The consumer, three times: no provider (handled), a provider that says on (rejected with the
/// reason `rule`), a provider that fails (rejected, the failure heard).
async fn consumer() {
    for round in 0..3 {
        let memory = Arc::new(MemoryTransport::new());
        let transport: Arc<dyn Transport> = memory.clone();
        let seated: Log<i32> = Default::default();
        let rejected: Log<Reason> = Default::default();
        let errors: Log<String> = Default::default();
        let (r, e) = (rejected.clone(), errors.clone());
        let provider = match round {
            1 => Some(Provider::on(&["smallGroups"])),
            2 => Some(Provider::down()),
            _ => None,
        };
        let mut options = ConsumerOptions::new().on_event_rejected(move |x: &EventRejected| r.lock().unwrap().push(x.reason));
        if let Some(p) = &provider {
            options = options.flags(p.clone()).on_flag_error(move |x| e.lock().unwrap().push(x.to_string()));
        }
        subscribe_floor_events(transport.clone(), Arc::new(Floor { seated: seated.clone() }), options).await.unwrap();
        create_desk_publisher(transport, PublisherOptions::new()).bookings().booked(&booked(9)).await.unwrap();
        memory.idle().await;
        if round == 0 {
            assert_eq!(*seated.lock().unwrap(), vec![9], "no provider: the event is handled");
            assert!(rejected.lock().unwrap().is_empty());
            continue;
        }
        assert!(seated.lock().unwrap().is_empty(), "round {round}: the invariant applies");
        assert_eq!(*rejected.lock().unwrap(), vec![Reason::Rule], "round {round}");
        let asked = provider.map(|p| p.asked()).unwrap_or_default();
        assert_eq!(asked.len(), 1, "round {round}: one reading per message: {asked:?}");
        if round == 2 {
            assert_eq!(errors.lock().unwrap().len(), 1, "the failure was heard");
        }
    }
}

fn main() {
    block_on(async {
        publisher().await;
        consumer().await;
    });
    println!("flags events ok");
}
