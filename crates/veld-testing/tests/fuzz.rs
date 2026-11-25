//! The request generator and the fuzz loop. The loop is tested against a fake test handler that
//! answers every request with a dry-run pass; the generated Node server has its own test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Endpoint, Ir, PackageKind, Service};
use veld_rules::helpers::instant::parse_datetime;
use veld_rules::{Outcome, Pipeline, fill_example};
use veld_testing::{EndpointReport, FuzzOptions, FuzzReport, Live, fuzz, generate, render_example};

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let Some(ir) = out.ir.clone() else {
        panic!("does not compile:\n{}", out.render());
    };
    ir
}

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    std::fs::read_to_string(path).unwrap()
}

/// A small hotel with the 30 day rule: `createBooking` has rules and `getBooking` has none.
/// It is a fixed copy of what `examples/bookings` was before it grew a state machine and
/// policies, so that the tests below, which name its endpoints, do not move with the example.
fn hotel() -> (String, Ir) {
    let text = read("crates/veld-testing/tests/fixtures/hotel.veld");
    let ir = compile_text(&text);
    (text, ir)
}

/// The contract of `examples/bookings` as it is today. It changes with the example, so a test
/// may only assert about it what holds for every contract.
fn bookings_example() -> (String, Ir) {
    let text = read("examples/bookings/contracts/bookings.veld");
    let ir = compile_text(&text);
    (text, ir)
}

/// `tests/rules/endpoints/bookings.veld`: coupons, an optional fact, `payBooking`.
fn endpoints() -> (String, Ir) {
    let text = read("tests/rules/endpoints/bookings.veld");
    let ir = compile_text(&text);
    (text, ir)
}

fn endpoint<'a>(ir: &'a Ir, name: &str) -> (&'a Service, &'a Endpoint) {
    for s in &ir.services {
        if let Some(e) = s.endpoints.iter().find(|e| e.name == name) {
            return (s, e);
        }
    }
    panic!("no endpoint `{name}`");
}

fn label(o: &Outcome) -> String {
    match o {
        Outcome::Passes => "ok".to_owned(),
        Outcome::Error(a) => a.name.clone(),
    }
}

fn outcomes(ir: &Ir, name: &str, seed: u64, count: usize) -> BTreeMap<String, usize> {
    let (s, e) = endpoint(ir, name);
    let pipeline = Pipeline::new(ir, s, name).unwrap();
    let mut seen = BTreeMap::new();
    for req in generate(ir, s, e, seed, count).unwrap().requests {
        *seen.entry(label(&pipeline.run(&req).unwrap())).or_default() += 1;
    }
    seen
}

// --- the generator ---------------------------------------------------------------------------

#[test]
fn the_same_seed_gives_the_same_requests() {
    let (_, ir) = hotel();
    let (s, e) = endpoint(&ir, "createBooking");
    let a = generate(&ir, s, e, 42, 300).unwrap();
    let b = generate(&ir, s, e, 42, 300).unwrap();
    assert_eq!(a.requests, b.requests);
    assert_eq!(a.boundaries, b.boundaries);
    let c = generate(&ir, s, e, 43, 300).unwrap();
    assert_ne!(a.requests, c.requests);
}

/// The example has endpoints without a parameter, which have only a few different requests, so
/// no number of requests is asserted here.
#[test]
fn no_generated_request_of_the_bookings_example_is_an_input_error() {
    let (_, ir) = bookings_example();
    let mut endpoints = 0;
    for s in &ir.services {
        for e in &s.endpoints {
            let pipeline = Pipeline::new(&ir, s, &e.name).unwrap();
            let generated = generate(&ir, s, e, 7, 600).unwrap();
            assert!(!generated.requests.is_empty(), "{}", e.name);
            for req in &generated.requests {
                pipeline
                    .run(req)
                    .unwrap_or_else(|err| panic!("{}: {err}\n{req:?}", e.name));
            }
            endpoints += 1;
        }
    }
    assert!(endpoints > 0);
}

#[test]
fn no_generated_request_is_an_input_error() {
    for (_, ir) in [hotel(), endpoints(), policies(), transitions()] {
        for s in &ir.services {
            for e in &s.endpoints {
                let pipeline = Pipeline::new(&ir, s, &e.name).unwrap();
                let generated = generate(&ir, s, e, 7, 2500).unwrap();
                assert!(generated.requests.len() > 100, "{}", e.name);
                for req in &generated.requests {
                    pipeline
                        .run(req)
                        .unwrap_or_else(|err| panic!("{}: {err}\n{req:?}", e.name));
                }
            }
        }
    }
}

#[test]
fn boundaries_come_first_and_never_exceed_the_count() {
    let (_, ir) = hotel();
    let (s, e) = endpoint(&ir, "createBooking");
    let g = generate(&ir, s, e, 1, 500).unwrap();
    assert!(
        g.boundaries >= 12,
        "only {} boundary requests",
        g.boundaries
    );
    assert_eq!(g.requests.len(), 500);
    let few = generate(&ir, s, e, 1, 5).unwrap();
    assert_eq!(few.requests.len(), 5);
    assert_eq!(few.boundaries, 5);
    assert_eq!(few.requests[..], g.requests[..5]);
}

fn stay_ms(req: &veld_rules::Request) -> Option<i64> {
    let body = req.params.get("body")?;
    let at = |k: &str| parse_datetime(body.get(k)?.as_str()?);
    Some(at("end")? - at("start")?)
}

#[test]
fn the_boundaries_of_the_hotel_rules_are_requested() {
    let (_, ir) = hotel();
    let (s, e) = endpoint(&ir, "createBooking");
    let g = generate(&ir, s, e, 5, 400).unwrap();
    let first = &g.requests[..g.boundaries];
    let guests_vs_capacity: BTreeSet<i64> = first
        .iter()
        .filter_map(|r| {
            let guests = r.params.get("body")?.get("guests")?.as_i64()?;
            let capacity = r.facts.get("room")?.as_ref()?.get("capacity")?.as_i64()?;
            Some(guests - capacity)
        })
        .collect();
    for d in [-1, 0, 1] {
        assert!(guests_vs_capacity.contains(&d), "{guests_vs_capacity:?}");
    }
    let day = 86_400_000;
    let stays: BTreeSet<i64> = first.iter().filter_map(stay_ms).collect();
    for want in [30 * day - 1, 30 * day, 30 * day + 1, -1, 0, 1] {
        assert!(stays.contains(&want), "no stay of {want} ms in {stays:?}");
    }
    // The start is also put at the clock and a millisecond either side of it.
    let from_clock: BTreeSet<i64> = first
        .iter()
        .filter_map(|r| {
            let start = parse_datetime(r.params.get("body")?.get("start")?.as_str()?)?;
            Some(start - r.now)
        })
        .collect();
    for d in [-1, 0, 1] {
        assert!(from_clock.contains(&d), "{from_clock:?}");
    }
}

#[test]
fn the_outcomes_seen_reach_every_error_of_the_endpoint() {
    let (_, ir) = hotel();
    let seen = outcomes(&ir, "createBooking", 11, 600);
    for kind in [
        "ok",
        "ValidationError",
        "NotFound",
        "RoomFull",
        "RoomClosed",
    ] {
        assert!(
            seen.get(kind).copied().unwrap_or(0) > 0,
            "{kind} in {seen:?}"
        );
    }
    let (_, ir) = endpoints();
    let seen = outcomes(&ir, "payBooking", 11, 400);
    for kind in ["ok", "NotFound", "AmountMismatch"] {
        assert!(
            seen.get(kind).copied().unwrap_or(0) > 0,
            "{kind} in {seen:?}"
        );
    }
    let seen = outcomes(&ir, "createBooking", 11, 600);
    for kind in [
        "ok",
        "ValidationError",
        "RoomFull",
        "RoomClosed",
        "NotFound",
    ] {
        assert!(
            seen.get(kind).copied().unwrap_or(0) > 0,
            "{kind} in {seen:?}"
        );
    }
}

#[test]
fn a_coupon_is_put_at_its_limit_with_the_key_that_loads_it() {
    let (_, ir) = endpoints();
    let (s, e) = endpoint(&ir, "createBooking");
    let g = generate(&ir, s, e, 3, 300).unwrap();
    let percents: BTreeSet<i64> = g.requests[..g.boundaries]
        .iter()
        .filter_map(|r| {
            let coupon = r.facts.get("coupon")?.as_ref()?;
            r.params.get("body")?.get("couponCode")?;
            coupon.get("percent")?.as_i64()
        })
        .collect();
    for p in [49, 50, 51] {
        assert!(percents.contains(&p), "{percents:?}");
    }
}

#[test]
fn a_string_with_a_pattern_and_no_example_skips_the_endpoint_with_a_reason() {
    let ir = compile_text(
        "model Code { value: string @pattern(\"^[A-Z]+$\") }\n\
         service S at \"/s\" { post \"/\" make(body: Code) -> Code }\n",
    );
    let (s, e) = endpoint(&ir, "make");
    let err = generate(&ir, s, e, 1, 10).unwrap_err();
    assert!(err.to_string().contains("@pattern"), "{err}");
    let ir = compile_text(
        "model Code { value: string @pattern(\"^[A-Z]+$\") @example(\"ABC\") }\n\
         service S at \"/s\" { post \"/\" make(body: Code) -> Code }\n",
    );
    let (s, e) = endpoint(&ir, "make");
    let g = generate(&ir, s, e, 1, 10).unwrap();
    assert!(
        g.requests
            .iter()
            .all(|r| r.params["body"]["value"] == "ABC")
    );
}

#[test]
fn every_kind_of_type_can_be_generated() {
    let ir = compile_text(
        "enum Color { red, green }\n\
         model Leaf { n: long, d: decimal @min(1) @max(2), f: float, b: bool, c: Color, u: uuid, \
           t: time, day: date, moment: datetime, raw: bytes, j: json }\n\
         model Cat { name: string }\n\
         model Dog { bark?: string }\n\
         union Pet { cat: Cat, dog: Dog }\n\
         model Zoo { leaf: Leaf, leaves: Leaf[] @length(min: 1, max: 3), named: Map<string, Leaf>, \
           pet: Pet, maybe?: Leaf, tags: string[] @length(max: 2) }\n\
         service S at \"/s\" { post \"/\" make(body: Zoo) -> Zoo }\n",
    );
    let (s, e) = endpoint(&ir, "make");
    let g = generate(&ir, s, e, 9, 200).unwrap();
    let pipeline = Pipeline::new(&ir, s, "make").unwrap();
    for r in &g.requests {
        assert!(matches!(pipeline.run(r).unwrap(), Outcome::Passes));
        let leaves = r.params["body"]["leaves"].as_array().unwrap().len();
        assert!((1..=3).contains(&leaves));
        let d: f64 = r.params["body"]["leaf"]["d"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1.0..=2.0).contains(&d), "{d}");
    }
    let some_absent = g
        .requests
        .iter()
        .any(|r| r.params["body"].get("maybe").is_none());
    let some_present = g
        .requests
        .iter()
        .any(|r| r.params["body"].get("maybe").is_some());
    assert!(some_absent && some_present);
}

// --- example blocks ----------------------------------------------------------------------------

/// Writing a request as an `example` block and filling the block in gives the request back.
#[test]
fn an_example_block_gives_the_request_back() {
    let (text, ir) = endpoints();
    block_round_trip(&text, &ir, &[("createBooking", 150), ("payBooking", 100)]);
}

/// The same for requests with a principal (`as { ... }`) and with every state of a machine.
#[test]
fn a_block_with_a_principal_and_a_state_gives_the_request_back() {
    let (text, ir) = policies();
    block_round_trip(&text, &ir, &[("payBooking", 150), ("getBooking", 100)]);
    let (text, ir) = transitions();
    let all = [("payBooking", 80), ("cancelBooking", 80), ("checkIn", 80)];
    block_round_trip(&text, &ir, &all);
    let ir = compile_text(ORDER);
    block_round_trip(ORDER, &ir, &[("pay", 200)]);
}

/// Prints the blocks of generated requests, appends them to the contract and checks that each
/// fills back to its request, and that it passes offline.
fn block_round_trip(text: &str, ir: &Ir, endpoints: &[(&str, usize)]) {
    let mut blocks = Vec::new();
    let mut requests = Vec::new();
    for &(name, count) in endpoints {
        let (s, e) = endpoint(ir, name);
        let pipeline = Pipeline::new(ir, s, name).unwrap();
        for (i, req) in generate(ir, s, e, 21, count)
            .unwrap()
            .requests
            .into_iter()
            .enumerate()
        {
            let outcome = pipeline.run(&req).unwrap();
            let title = format!("fuzz {name} {i}");
            blocks.push(render_example(ir, s, e, &req, &outcome, &title).unwrap());
            requests.push((title, req, outcome));
        }
    }
    let with = compile_text(&format!("{text}\n{}\n", blocks.join("\n\n")));
    for (title, req, outcome) in requests {
        let ex = with.examples.iter().find(|e| e.name == title).unwrap();
        let filled = fill_example(&with, ex).unwrap_or_else(|e| panic!("{title}: {e}"));
        assert_eq!(filled.request, req, "{title}");
        // The example expects what the reference pipeline says: offline it passes.
        let (s, _) = endpoint(&with, &ex.endpoint);
        let again = Pipeline::new(&with, s, &ex.endpoint)
            .unwrap()
            .run(&filled.request)
            .unwrap();
        assert_eq!(label(&again), label(&outcome), "{title}");
        let report = veld_rules::run_example(&with, ex).unwrap();
        assert!(report.passed(), "{title}: {report}");
    }
}

// --- the fuzz loop against a fake test handler -----------------------------------------------------

/// A test handler for the services of a contract that answers every request with a dry-run
/// pass. It speaks the protocol otherwise: the probe names its services, and every answer carries
/// the rules digest. With `dies_after`, it stops answering after that many requests.
struct AlwaysPass {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AlwaysPass {
    fn start(ir: &Ir) -> AlwaysPass {
        AlwaysPass::serving(ir, |_| true, None)
    }

    /// The handler of the services of `ir` that `serves` accepts.
    fn serving(ir: &Ir, serves: impl Fn(&str) -> bool, dies_after: Option<usize>) -> AlwaysPass {
        let digests: serde_json::Map<String, serde_json::Value> = ir
            .services
            .iter()
            .filter(|s| serves(&s.name))
            .map(|s| (s.name.clone(), serde_json::json!(s.rules_digest)))
            .collect();
        let probe = serde_json::json!({ "veldTestHandler": 1, "digests": digests }).to_string();
        let probe = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{probe}",
            probe.len()
        );
        // One service per contract in these tests, so one digest serves every answer.
        let digest = ir.services.iter().find_map(|s| s.rules_digest.clone());
        let rules = digest.map_or_else(String::new, |d| format!("Veld-Rules: {d}\r\n"));
        let pass = format!(
            "HTTP/1.1 204 No Content\r\nVeld-Test-Passed: 1\r\n{rules}Connection: close\r\n\r\n"
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut answered = 0usize;
            for stream in listener.incoming() {
                if stop2.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut stream) = stream else { continue };
                let target = read_target(&mut stream);
                if target.as_deref() == Some("/__veld/test") {
                    let _ = stream.write_all(probe.as_bytes());
                    continue;
                }
                if dies_after.is_some_and(|n| answered >= n) {
                    // Dropping the listener refuses every later connection: the server is gone.
                    return;
                }
                answered += 1;
                let _ = stream.write_all(pass.as_bytes());
            }
        });
        AlwaysPass {
            addr,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for AlwaysPass {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Reads one request (head and body) and returns its target.
fn read_target(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let target = head.lines().next()?.split(' ').nth(1)?.to_owned();
    let len: usize = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    while buf.len() < head_end + len {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Some(target)
}

#[test]
fn a_server_that_always_passes_is_caught_with_a_seed_and_a_block_that_compiles() {
    let (text, ir) = hotel();
    let fake = AlwaysPass::start(&ir);
    let live = Live::connect(&fake.url(), true).unwrap();
    let opts = FuzzOptions {
        count: 200,
        seed: 1234,
    };
    let report = fuzz(&ir, &live, &opts).unwrap();
    assert_eq!(report.seed, 1234);
    assert!(!report.passed());
    let create = report
        .endpoints
        .iter()
        .find(|e| e.endpoint == "createBooking")
        .unwrap();
    let m = create.mismatch.as_ref().unwrap();
    assert!(m.actual.contains("204"), "{}", m.actual);
    assert!(!m.differences.is_empty());
    // Stops at the first mismatch: the requests sent are the ones up to it.
    assert_eq!(create.sent, m.index + 1);
    let block = m.example.as_ref().unwrap();
    assert!(
        block.contains("fuzz seed 1234 Bookings.createBooking"),
        "{block}"
    );
    let with = compile_text(&format!("{text}\n{block}\n"));
    assert_eq!(with.examples.len(), ir.examples.len() + 1);
    // The block is the request that failed.
    let (s, e) = endpoint(&ir, "createBooking");
    let req = &generate(&ir, s, e, 1234, 200).unwrap().requests[m.index];
    let ex = with
        .examples
        .iter()
        .find(|e| block.contains(&e.name))
        .unwrap();
    assert_eq!(&fill_example(&with, ex).unwrap().request, req);
    // getBooking has no rule that can fail, so an always-pass server agrees with it.
    let get = report
        .endpoints
        .iter()
        .find(|e| e.endpoint == "getBooking")
        .unwrap();
    assert!(get.mismatch.is_none());
    assert_eq!(get.sent, 200);
    assert_eq!(get.outcomes.get("ok"), Some(&200));
}

#[test]
fn a_fuzz_run_is_repeatable_from_its_seed() {
    let (_, ir) = hotel();
    let fake = AlwaysPass::start(&ir);
    let live = Live::connect(&fake.url(), true).unwrap();
    let run = |seed| {
        let report = fuzz(&ir, &live, &FuzzOptions { count: 60, seed }).unwrap();
        report
            .endpoints
            .iter()
            .map(|e| {
                (
                    e.sent,
                    e.outcomes.clone(),
                    e.mismatch.as_ref().map(|m| m.example.clone()),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6));
}

#[test]
fn a_value_that_needs_escaping_is_written_so_that_it_compiles() {
    // The block of a request whose values need escaping still compiles.
    let ir = compile_text(
        "model Note { text: string @example(\"say \\\"hi\\\"\"), n: int @min(1) }\n\
         service S at \"/s\" { post \"/\" make(body: Note) -> Note }\n",
    );
    let (s, e) = endpoint(&ir, "make");
    let req = &generate(&ir, s, e, 1, 3).unwrap().requests[0];
    let outcome = Pipeline::new(&ir, s, "make").unwrap().run(req).unwrap();
    let block = render_example(&ir, s, e, req, &outcome, "q").unwrap();
    let text = "model Note { text: string @example(\"say \\\"hi\\\"\"), n: int @min(1) }\n\
         service S at \"/s\" { post \"/\" make(body: Note) -> Note }\n";
    let with = compile_text(&format!("{text}\n{block}\n"));
    assert_eq!(with.examples.len(), 1);
}

#[test]
fn a_service_without_endpoint_rules_has_no_test_handler_and_is_skipped() {
    let ir = compile_text(MEASURES);
    let fake = AlwaysPass::start(&ir);
    let live = Live::connect(&fake.url(), true).unwrap();
    let report = fuzz(&ir, &live, &FuzzOptions { count: 20, seed: 1 }).unwrap();
    assert!(report.passed());
    let e = &report.endpoints[0];
    assert_eq!(e.sent, 0);
    assert!(e.skipped.as_ref().unwrap().contains("no test handler"));
    assert_eq!(report.skipped().len(), 1);
}

#[test]
fn nothing_empty_goes_in_the_path_the_query_string_or_a_header() {
    let ir = compile_text(
        "model Filter { tags: string[], word: string, note?: string }\n\
         service S at \"/s\" {\n\
           get \"/{id}\" find(id: string, query: Filter, @header(\"X-Trace\") trace: string, \
             @query names: string[]) -> Filter\n\
         }\n",
    );
    let (s, e) = endpoint(&ir, "find");
    for req in generate(&ir, s, e, 8, 300).unwrap().requests {
        assert!(!req.params["id"].as_str().unwrap().is_empty());
        assert!(!req.params["trace"].as_str().unwrap().is_empty());
        assert!(!req.params["names"].as_array().unwrap().is_empty());
        assert!(!req.params["query"]["tags"].as_array().unwrap().is_empty());
        assert!(!req.params["query"]["word"].as_str().unwrap().is_empty());
        if let Some(note) = req.params["query"].get("note") {
            assert!(!note.as_str().unwrap().is_empty());
        }
    }
}

// --- every kind of boundary ----------------------------------------------------------------------

const MEASURES: &str = "model Item {\n\
  tag: string\n\
  notes: string[]\n\
  price: decimal\n\
  ratio: float\n\
  day: date\n\
  due: date\n\
  rule tagShort: len(tag) <= 5\n\
  rule needNotes: len(notes) >= 2\n\
  rule cheap: price < 99.50\n\
  rule smallRatio: ratio <= 2.5\n\
  rule nonNegative: ratio >= 0\n\
  rule dueSoon: due - day <= days(7)\n\
}\n\
service Shop at \"/shop\" { post \"/\" make(body: Item) -> Item }\n";

fn field<'a>(r: &'a veld_rules::Request, name: &str) -> &'a serde_json::Value {
    &r.params["body"][name]
}

#[test]
fn lengths_decimals_floats_and_dates_get_their_boundaries() {
    let ir = compile_text(MEASURES);
    let (s, e) = endpoint(&ir, "make");
    let g = generate(&ir, s, e, 3, 300).unwrap();
    let first = &g.requests[..g.boundaries];
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let tags: BTreeSet<usize> = first
        .iter()
        .map(|r| field(r, "tag").as_str().unwrap().len())
        .collect();
    let notes: BTreeSet<usize> = first
        .iter()
        .map(|r| field(r, "notes").as_array().unwrap().len())
        .collect();
    assert!([4, 5, 6].iter().all(|n| tags.contains(n)), "{tags:?}");
    assert!([1, 2, 3].iter().all(|n| notes.contains(n)), "{notes:?}");
    let prices: Vec<f64> = first
        .iter()
        .map(|r| field(r, "price").as_str().unwrap().parse().unwrap())
        .collect();
    for want in [99.49, 99.50, 99.51] {
        assert!(
            prices.iter().any(|p| close(*p, want)),
            "{want} in {prices:?}"
        );
    }
    let ratios: Vec<f64> = first
        .iter()
        .map(|r| field(r, "ratio").as_f64().unwrap())
        .collect();
    assert!(ratios.contains(&2.5));
    assert!(ratios.contains(&2.5_f64.next_up()));
    assert!(ratios.contains(&2.5_f64.next_down()));
    assert!(ratios.contains(&0.0));
    assert!(
        ratios.contains(&0.0_f64.next_down()),
        "a float just below zero"
    );
    let days: BTreeSet<i64> = first
        .iter()
        .map(|r| {
            let at = |k: &str| {
                veld_rules::helpers::instant::parse_date(field(r, k).as_str().unwrap()).unwrap()
            };
            at("due") - at("day")
        })
        .collect();
    assert!([6, 7, 8].iter().all(|n| days.contains(n)), "{days:?}");
}

#[test]
fn the_blocks_of_those_boundaries_compile_and_give_the_requests_back() {
    let ir = compile_text(MEASURES);
    let (s, e) = endpoint(&ir, "make");
    let pipeline = Pipeline::new(&ir, s, "make").unwrap();
    let requests = generate(&ir, s, e, 3, 120).unwrap().requests;
    let mut blocks = Vec::new();
    for (i, req) in requests.iter().enumerate() {
        let outcome = pipeline.run(req).unwrap();
        let block = render_example(&ir, s, e, req, &outcome, &format!("m {i}"));
        blocks.push(block.unwrap_or_else(|| panic!("request {i} cannot be written: {req:?}")));
    }
    let with = compile_text(&format!("{MEASURES}\n{}\n", blocks.join("\n")));
    for (i, req) in requests.iter().enumerate() {
        let ex = with
            .examples
            .iter()
            .find(|x| x.name == format!("m {i}"))
            .unwrap();
        assert_eq!(&fill_example(&with, ex).unwrap().request, req, "m {i}");
    }
}

// --- what the independent check of the fuzzer found ------------------------------------------

/// A minimum length used to be cut at 64: the value then broke its own constraint, a correct
/// server answered 422 and the reference pipeline, which does not validate shape, said ok.
#[test]
fn a_minimum_length_is_never_cut_short() {
    let ir = compile_text(
        "model Big { s: string @length(min: 100), tags: string[] @length(min: 70), \
           m: Map<string, int> @length(min: 66), n: int @min(0)  rule small: n <= 5 }\n\
         service B at \"/b\" { post \"/\" go(body: Big) -> Big { require body.n < 3 } }\n",
    );
    let (s, e) = endpoint(&ir, "go");
    let generated = generate(&ir, s, e, 1, 60).unwrap();
    assert_eq!(generated.requests.len(), 60);
    assert!(generated.boundaries > 0, "no boundary request was made");
    for req in &generated.requests {
        let body = &req.params["body"];
        assert!(body["s"].as_str().unwrap().chars().count() >= 100, "{body}");
        assert!(body["tags"].as_array().unwrap().len() >= 70);
        assert!(body["m"].as_object().unwrap().len() >= 66);
    }
}

#[test]
fn a_minimum_length_beyond_the_generator_skips_the_endpoint_with_a_reason() {
    let ir = compile_text(
        "model Huge { s: string @length(min: 5000) }\n\
         service H at \"/h\" { post \"/\" go(body: Huge) -> Huge }\n",
    );
    let (s, e) = endpoint(&ir, "go");
    let err = generate(&ir, s, e, 1, 10).unwrap_err();
    assert!(err.to_string().contains("at least 5000"), "{err}");
}

/// A server that stops answering ends the run, and what was sent before is still reported.
#[test]
fn a_server_that_dies_mid_run_keeps_what_was_found() {
    let (_, ir) = hotel();
    let fake = AlwaysPass::serving(&ir, |_| true, Some(5));
    let live = Live::connect(&fake.url(), true).unwrap();
    let report = fuzz(&ir, &live, &FuzzOptions { count: 50, seed: 7 }).unwrap();
    let why = report
        .stopped
        .as_ref()
        .expect("the run must say it stopped");
    assert!(why.contains("cannot reach"), "{why}");
    assert!(!report.passed());
    let sent: usize = report.endpoints.iter().map(|e| e.sent).sum();
    assert_eq!(sent, 5, "{:?}", report.endpoints);
    // The always-pass handler disagrees on the first request that must be rejected: that
    // finding was made before the server died, and it is in the report.
    assert!(report.endpoints.iter().any(|e| e.mismatch.is_some()));
}

/// A workspace has one test handler per service. Requests for a service the handler does not
/// name in its probe are not sent to it.
#[test]
fn a_service_the_handler_does_not_serve_is_skipped() {
    let (_, ir) = hotel();
    let fake = AlwaysPass::serving(&ir, |_| false, None);
    let live = Live::connect(&fake.url(), true).unwrap();
    let report = fuzz(&ir, &live, &FuzzOptions { count: 20, seed: 1 }).unwrap();
    assert!(report.passed());
    assert!(!report.endpoints.is_empty());
    for e in &report.endpoints {
        assert_eq!(e.sent, 0);
        assert!(
            e.skipped.as_ref().unwrap().contains("does not serve"),
            "{e:?}"
        );
    }
}

#[test]
fn an_endpoint_that_only_ever_ends_one_way_is_named() {
    let endpoint = |name: &str, outcomes: &[(&str, usize)]| EndpointReport {
        service: "S".to_owned(),
        endpoint: name.to_owned(),
        sent: outcomes.iter().map(|(_, n)| n).sum(),
        boundaries: 0,
        outcomes: outcomes
            .iter()
            .map(|(k, n)| ((*k).to_owned(), *n))
            .collect(),
        skipped: None,
        mismatch: None,
    };
    let report = FuzzReport {
        seed: 1,
        endpoints: vec![
            endpoint("stuck", &[("ValidationError", 300)]),
            endpoint("fine", &[("ok", 200), ("ValidationError", 100)]),
            endpoint("allOk", &[("ok", 300)]),
            endpoint("once", &[("NotFound", 1)]),
        ],
        stopped: None,
    };
    assert_eq!(
        report.one_sided(),
        [("S.stuck".to_owned(), "ValidationError".to_owned())]
    );
}

// --- the principal, policies and state machines -----------------------------------------------

fn policies() -> (String, Ir) {
    let text = read("tests/rules/endpoints/policies.veld");
    let ir = compile_text(&text);
    (text, ir)
}

fn transitions() -> (String, Ir) {
    let text = read("tests/rules/endpoints/transitions.veld");
    let ir = compile_text(&text);
    (text, ir)
}

/// The `message` of every `Forbidden` the reference gives for the requests of `name`.
fn denial_messages(ir: &Ir, name: &str, seed: u64, count: usize) -> BTreeMap<String, usize> {
    let (s, e) = endpoint(ir, name);
    let pipeline = Pipeline::new(ir, s, name).unwrap();
    let mut seen = BTreeMap::new();
    for req in generate(ir, s, e, seed, count).unwrap().requests {
        if let Outcome::Error(a) = pipeline.run(&req).unwrap()
            && a.name == "Forbidden"
        {
            let message = a.data["message"].as_str().unwrap().to_owned();
            *seen.entry(message).or_default() += 1;
        }
    }
    seen
}

#[test]
fn a_principal_is_generated_for_an_endpoint_with_auth_and_only_for_it() {
    let (_, ir) = policies();
    for name in ["payBooking", "getBooking"] {
        let (s, e) = endpoint(&ir, name);
        let generated = generate(&ir, s, e, 3, 200).unwrap();
        for req in &generated.requests {
            let p = req.principal.as_ref().expect("a principal");
            assert!(p["id"].is_string() && p["roles"].is_array(), "{p}");
        }
    }
    // No principal in a package that declares none.
    let (_, ir) = endpoints();
    let (s, e) = endpoint(&ir, "payBooking");
    assert!(
        generate(&ir, s, e, 3, 50)
            .unwrap()
            .requests
            .iter()
            .all(|r| r.principal.is_none())
    );
}

#[test]
fn the_policies_are_permitted_and_denied_by_a_permit_and_by_a_forbid() {
    let (_, ir) = policies();
    for name in ["payBooking", "getBooking"] {
        let seen = outcomes(&ir, name, 11, 400);
        assert!(
            seen.get("ok").copied().unwrap_or(0) > 20,
            "{name}: {seen:?}"
        );
        assert!(
            seen.get("Forbidden").copied().unwrap_or(0) > 20,
            "{name}: {seen:?}"
        );
        let messages = denial_messages(&ir, name, 11, 400);
        let forbid = "A suspended account cannot manage bookings";
        assert!(
            messages.get(forbid).copied().unwrap_or(0) > 5,
            "{messages:?}"
        );
        assert!(
            messages.get("not permitted").copied().unwrap_or(0) > 5,
            "{messages:?}"
        );
    }
    // Every permit is reached: the guest's own booking and a caller with the role `support`.
    let (s, e) = endpoint(&ir, "payBooking");
    let pipeline = Pipeline::new(&ir, s, "payBooking").unwrap();
    let (mut own, mut support) = (0, 0);
    for req in generate(&ir, s, e, 11, 400).unwrap().requests {
        if !matches!(pipeline.run(&req).unwrap(), Outcome::Passes) {
            continue;
        }
        let user = req.principal.as_ref().unwrap();
        let guest = req.facts["booking"].as_ref().unwrap()["guestId"].clone();
        own += usize::from(user["id"] == guest);
        let roles = user["roles"].as_array().unwrap();
        support += usize::from(roles.iter().any(|r| r == "support"));
    }
    assert!(own > 5 && support > 5, "own {own}, support {support}");
}

#[test]
fn every_state_of_a_machine_is_requested_as_a_boundary() {
    let (_, ir) = transitions();
    for (name, legal) in [
        ("payBooking", vec!["pending"]),
        ("cancelBooking", vec!["pending", "paid"]),
        ("checkIn", vec!["paid"]),
    ] {
        let (s, e) = endpoint(&ir, name);
        let pipeline = Pipeline::new(&ir, s, name).unwrap();
        let generated = generate(&ir, s, e, 5, 300).unwrap();
        let mut states = BTreeMap::new();
        for req in &generated.requests[..generated.boundaries] {
            let booking = req.facts["booking"].as_ref().unwrap();
            let status = booking["status"].as_str().unwrap();
            states.insert(status.to_owned(), label(&pipeline.run(req).unwrap()));
        }
        for state in ["pending", "paid", "checkedIn", "cancelled", "refunded"] {
            let outcome = states
                .get(state)
                .unwrap_or_else(|| panic!("{name}: no {state}"));
            if legal.contains(&state) {
                assert_ne!(outcome, "InvalidTransition", "{name} {state}");
            } else {
                assert_eq!(outcome, "InvalidTransition", "{name} {state}");
            }
        }
        let seen = outcomes(&ir, name, 5, 300);
        assert!(
            seen.get("ok").copied().unwrap_or(0) > 20,
            "{name}: {seen:?}"
        );
        assert!(
            seen.get("InvalidTransition").copied().unwrap_or(0) > 20,
            "{name}: {seen:?}"
        );
    }
}

/// A contract where a request can be denied, in an illegal state and fail a `require` at once.
const ORDER: &str = r#"
enum Status {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled
}
model User { id: uuid }
principal User
model Booking { id: uuid, guestId: uuid, status: Status = .pending, total: decimal }
model Payment { amount: decimal }
policy Own(user: User, booking: Booking) { permit when booking.guestId == user.id }
@auth(bearer)
service Bookings at "/bookings" {
  post "/{id}/pay" pay(id: uuid, body: Payment) -> Booking {
    fact booking: Booking = id
    allow Own(user, booking)
    transition booking.status: pending -> paid
    require body.amount == booking.total
  }
}
"#;

#[test]
fn requests_where_two_checks_apply_end_in_the_one_that_comes_first() {
    let ir = compile_text(ORDER);
    let (s, e) = endpoint(&ir, "pay");
    let pipeline = Pipeline::new(&ir, s, "pay").unwrap();
    let mut tally = BTreeMap::new();
    for req in generate(&ir, s, e, 9, 600).unwrap().requests {
        let first = label(&pipeline.run(&req).unwrap());
        *tally.entry(first).or_insert(0) += 1;
    }
    for outcome in ["ok", "Forbidden", "InvalidTransition", "ValidationError"] {
        assert!(tally.contains_key(outcome), "{tally:?}");
    }
    // Requests that fail two checks: cure the first failure and the reference must then give the
    // second one. Counted by the reference, not by looking at the values.
    let pairs = two_checks(&ir, "pay", 9, 600, cure_order);
    let n = |a: &str, b: &str| {
        pairs
            .get(&(a.to_owned(), b.to_owned()))
            .copied()
            .unwrap_or(0)
    };
    assert!(n("Forbidden", "InvalidTransition") > 3, "{pairs:?}");
    assert!(n("Forbidden", "ValidationError") > 3, "{pairs:?}");
    assert!(n("InvalidTransition", "ValidationError") > 3, "{pairs:?}");
}

/// For each request of `name` that the reference refuses, `cure` removes that one refusal; the
/// result counts how often a refusal `first` was followed, after the cure, by the refusal `then`.
fn two_checks(
    ir: &Ir,
    name: &str,
    seed: u64,
    count: usize,
    cure: fn(&mut veld_rules::Request, &str),
) -> BTreeMap<(String, String), usize> {
    let (s, e) = endpoint(ir, name);
    let pipeline = Pipeline::new(ir, s, name).unwrap();
    let mut pairs = BTreeMap::new();
    for req in generate(ir, s, e, seed, count).unwrap().requests {
        let first = label(&pipeline.run(&req).unwrap());
        let mut cured = req.clone();
        cure(&mut cured, &first);
        if cured == req {
            continue;
        }
        let then = label(&pipeline.run(&cured).unwrap());
        if then != "ok" {
            *pairs.entry((first, then)).or_default() += 1;
        }
    }
    pairs
}

/// The contract `ORDER`: a denied caller becomes the guest, an illegal state becomes `pending`.
fn cure_order(req: &mut veld_rules::Request, refusal: &str) {
    match refusal {
        "Forbidden" => {
            let guest = req.facts["booking"].as_ref().unwrap()["guestId"].clone();
            req.principal.as_mut().unwrap()["id"] = guest;
        }
        "InvalidTransition" => {
            let booking = req.facts.get_mut("booking").unwrap().as_mut().unwrap();
            booking["status"] = "pending".into();
        }
        _ => {}
    }
}

/// `policies.veld`: a denied caller becomes the guest and is not suspended.
fn cure_policies(req: &mut veld_rules::Request, refusal: &str) {
    if refusal == "Forbidden" {
        cure_order(req, refusal);
        req.principal.as_mut().unwrap()["suspended"] = false.into();
    }
}

/// `transitions.veld`: an illegal state of the booking becomes `pending` (the source of
/// `payBooking`).
fn cure_transitions(req: &mut veld_rules::Request, refusal: &str) {
    if refusal == "InvalidTransition" {
        cure_order(req, refusal);
    }
}

/// On the two contracts of `tests/rules/endpoints` that have one of each, a request often fails
/// the earlier check and would fail the later one too: the denied caller who also sends the wrong
/// amount, the booking in an illegal state with the wrong amount.
#[test]
fn the_served_contracts_have_requests_that_fail_two_checks() {
    let (_, ir) = policies();
    let pairs = two_checks(&ir, "payBooking", 20_260_201, 300, cure_policies);
    let denied_wrong = pairs.get(&("Forbidden".to_owned(), "ValidationError".to_owned()));
    eprintln!("policies.payBooking, seed 20260201, 300: {pairs:?}");
    assert!(denied_wrong.copied().unwrap_or(0) > 3, "{pairs:?}");
    let (_, ir) = transitions();
    let pairs = two_checks(&ir, "payBooking", 20_260_201, 300, cure_transitions);
    let illegal_wrong = pairs.get(&("InvalidTransition".to_owned(), "AmountMismatch".to_owned()));
    eprintln!("transitions.payBooking, seed 20260201, 300: {pairs:?}");
    assert!(illegal_wrong.copied().unwrap_or(0) > 3, "{pairs:?}");
}
