//! Events and no rule at all: a publisher-only service and a subscriber-only service, a crate
//! without the rules runtime. Prints `events plain ok`.
#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use events_server::bookings::listener_events::{ListenerEventHandlers, subscribe_listener_events};
use events_server::bookings::pinger_events::create_pinger_publisher;
use events_server::bookings::t::Pinged;
use events_server::veld::events::{ConsumerOptions, EventMeta, MemoryTransport, PublisherOptions, Transport};
use events_server::veld::{Error, block_on};

struct Listener(Arc<Mutex<Vec<String>>>);

impl ListenerEventHandlers for Listener {
    fn heard(&self, event: Pinged, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        self.0.lock().unwrap().push(event.when);
        std::future::ready(Ok(()))
    }
}

fn main() {
    let transport = Arc::new(MemoryTransport::new());
    let seen = Arc::new(Mutex::new(Vec::new()));
    block_on(async {
        let t: Arc<dyn Transport> = transport.clone();
        subscribe_listener_events(t.clone(), Arc::new(Listener(seen.clone())), ConsumerOptions::new()).await.unwrap();
        let pinger = create_pinger_publisher(t, PublisherOptions::new());
        let sent = pinger.pings().pinged(&Pinged { when: "2026-06-10T13:59:58.250+02:00".to_string() }).await.unwrap();
        assert!(sent.subject.is_none(), "a channel without a key has no subject");
        let bad = pinger.pings().pinged(&Pinged { when: "yesterday".to_string() }).await.unwrap_err();
        assert_eq!(bad.problem().unwrap().title, "ValidationError");
        transport.idle().await;
    });
    assert_eq!(*seen.lock().unwrap(), vec!["2026-06-10T11:59:58.250Z".to_string()], "the datetime is canonical");
    println!("events plain ok");
}
