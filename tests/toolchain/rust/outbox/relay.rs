//! The `relay` and `written` goldens of `tests/wire/events/outbox.json` (copied next to the program
//! as `outbox.json`) through the GENERATED relay `run_bookings_outbox_relay` and `check_written`
//! (`spec/events.md` section 11, `tests/wire/events/README.md`). Expects the server generated from
//! `tests/events/outbox.veld.pending` as the package `bookings`. Prints `outbox relay ok`.
#[path = "../common.rs"]
mod common;

use std::sync::{Arc, Mutex};

use common::*;
use events_server::bookings::bookings_events::run_bookings_outbox_relay;
use events_server::veld::events::{
    BoxFuture, ChannelOptions, Deliver, MemoryOutbox, Outbox, OutboxEntry, RelayOptions, Transport, TransportSubscription, check_written,
};
use events_server::veld::{Error, block_on};
use serde_json::Value;

const TOPIC: &str = "shop.bookings.v1";
const CRASH: &str = "the process ended";

/// What a script says happens to the entry the run is looking at.
#[derive(Clone)]
enum Step {
    Accepted,
    PublishFails(String),
    DoneFails(String),
    CrashAfterPublish,
}

fn step_of(v: &Value) -> Step {
    match v {
        Value::String(s) if s == "accepted" => Step::Accepted,
        Value::String(s) if s == "crashAfterPublish" => Step::CrashAfterPublish,
        Value::Object(o) if o.contains_key("publishFails") => Step::PublishFails(o["publishFails"].as_str().unwrap().to_string()),
        Value::Object(o) => Step::DoneFails(o["doneFails"].as_str().unwrap().to_string()),
        other => panic!("a step of the script: {other}"),
    }
}

/// One run: the script, the entry it is at, and what the transport accepted and `done` succeeded for.
struct Run {
    script: Vec<Step>,
    seen: Mutex<isize>,
    published: Mutex<Vec<String>>,
    done: Mutex<Vec<String>>,
}

impl Run {
    fn step(&self) -> Step {
        let i = usize::try_from(*self.seen.lock().unwrap()).unwrap_or(0);
        self.script.get(i).cloned().unwrap_or(Step::Accepted)
    }
}

struct Scripted(Arc<Run>);

impl Transport for Scripted {
    fn publish<'a>(&'a self, _topic: &'a str, _key: Option<&'a str>, bytes: &'a [u8], _options: ChannelOptions) -> BoxFuture<'a, Result<(), Error>> {
        *self.0.seen.lock().unwrap() += 1;
        let out = match self.0.step() {
            Step::PublishFails(text) => Err(Error::other(text)),
            _ => {
                self.0.published.lock().unwrap().push(String::from_utf8_lossy(bytes).into_owned());
                Ok(())
            }
        };
        Box::pin(std::future::ready(out))
    }

    fn subscribe<'a>(&'a self, _t: &'a str, _g: &'a str, _d: Deliver, _o: ChannelOptions) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        Box::pin(std::future::ready(Err(Error::other("not used"))))
    }
}

/// The memory outbox, with the `done` of the script.
struct Scripts {
    memory: Arc<MemoryOutbox>,
    run: Arc<Run>,
}

impl Outbox for Scripts {
    fn save<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>> {
        self.memory.save(topic, key, bytes)
    }

    fn pending(&self, limit: usize) -> BoxFuture<'_, Result<Vec<OutboxEntry>, Error>> {
        self.memory.pending(limit)
    }

    fn done<'a>(&'a self, entry: &'a OutboxEntry) -> BoxFuture<'a, Result<(), Error>> {
        let out = match self.run.step() {
            Step::CrashAfterPublish => Err(Error::other(CRASH)),
            Step::DoneFails(text) => Err(Error::other(text)),
            _ => {
                self.run.done.lock().unwrap().push(entry.id.clone());
                return self.memory.done(entry);
            }
        };
        Box::pin(std::future::ready(out))
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

fn pending(memory: &MemoryOutbox) -> Vec<String> {
    memory.entries().into_iter().map(|e| e.id).collect()
}

async fn run_case(case: &Value) {
    let memory = Arc::new(MemoryOutbox::new());
    // Entry `n` is saved n-th: the memory outbox names it "n", and its bytes say so.
    for i in 1..=case["saves"].as_u64().unwrap() {
        memory.save(TOPIC, None, i.to_string().as_bytes()).await.unwrap();
    }
    for (n, run) in case["runs"].as_array().unwrap().iter().enumerate() {
        let what = format!("{} / run {}", case["name"].as_str().unwrap(), n + 1);
        let state = Arc::new(Run {
            script: run["script"].as_array().unwrap().iter().map(step_of).collect(),
            seen: Mutex::new(-1),
            published: Mutex::default(),
            done: Mutex::default(),
        });
        let failure: Arc<Mutex<Option<String>>> = Arc::default();
        let f = failure.clone();
        let options = RelayOptions::new().limit(usize::try_from(run["limit"].as_u64().unwrap()).unwrap()).on_error(move |error, entry| {
            // A crash reports nothing: the process is gone.
            if error.to_string() != CRASH {
                *f.lock().unwrap() = Some(format!("{}/{error}", entry.map_or("-", |e| e.id.as_str())));
            }
        });
        let outbox = Scripts { memory: memory.clone(), run: state.clone() };
        let sent = run_bookings_outbox_relay(&outbox, &Scripted(state.clone()), &options).await;
        let expect = &run["expect"];
        assert_eq!(*state.published.lock().unwrap(), strings(&expect["published"]), "{what}: published");
        assert_eq!(*state.done.lock().unwrap(), strings(&expect["done"]), "{what}: done");
        assert_eq!(sent as u64, expect["sent"].as_u64().unwrap(), "{what}: sent");
        let wanted = expect["failure"].as_object().map(|f| format!("{}/{}", f["entry"].as_str().unwrap(), f["error"].as_str().unwrap()));
        assert_eq!(*failure.lock().unwrap(), wanted, "{what}: failure");
        assert_eq!(pending(&memory), strings(&run["pendingAfter"]), "{what}: pending after");
    }
}

fn main() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string("outbox.json").unwrap()).unwrap();
    assert!(!golden["relay"].as_array().unwrap().is_empty() && !golden["written"].as_array().unwrap().is_empty());
    block_on(async {
        for case in golden["relay"].as_array().unwrap() {
            run_case(case).await;
        }
    });
    // `written`: what the server checks after the handler of a move that emits.
    for case in golden["written"].as_array().unwrap() {
        let (expected, written) = (strings(&case["expected"]), strings(&case["written"]));
        let (e, w): (Vec<&str>, Vec<&str>) = (expected.iter().map(String::as_str).collect(), written.iter().map(String::as_str).collect());
        assert_eq!(check_written(&e, &w), case["result"].as_str().unwrap(), "{}", case["name"]);
    }
    // The relay hands the transport the options of the channel of the topic (a topic of no channel: none).
    block_on(async {
        let (memory, recorder) = (MemoryOutbox::new(), Recorder::new());
        memory.save(TOPIC, Some("k"), b"x").await.unwrap();
        memory.save("somewhere.else", None, b"y").await.unwrap();
        run_bookings_outbox_relay(&memory, &*recorder, &RelayOptions::new()).await;
        assert_eq!(*recorder.options.lock().unwrap(), vec![ChannelOptions::NONE, ChannelOptions::NONE]);
        assert_eq!(recorder.messages().len(), 2);
    });
    println!("outbox relay ok");
}
