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
use veld_testing::{FuzzOptions, Live, fuzz, generate, render_example};

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

/// The hotel of `examples/bookings`: it has the 30 day rule.
fn hotel() -> (String, Ir) {
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

#[test]
fn no_generated_request_is_an_input_error() {
    for (_, ir) in [hotel(), endpoints()] {
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
    let mut blocks = Vec::new();
    let mut requests = Vec::new();
    for (name, count) in [("createBooking", 150), ("payBooking", 100)] {
        let (s, e) = endpoint(&ir, name);
        let pipeline = Pipeline::new(&ir, s, name).unwrap();
        for (i, req) in generate(&ir, s, e, 21, count)
            .unwrap()
            .requests
            .into_iter()
            .enumerate()
        {
            let outcome = pipeline.run(&req).unwrap();
            let title = format!("fuzz {name} {i}");
            blocks.push(render_example(&ir, s, e, &req, &outcome, &title).unwrap());
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

/// A test handler that answers every request with a dry-run pass.
struct AlwaysPass {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AlwaysPass {
    fn start() -> AlwaysPass {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop2.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut stream) = stream else { continue };
                let target = read_target(&mut stream);
                let reply = if target.as_deref() == Some("/__veld/test") {
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                } else {
                    "HTTP/1.1 204 No Content\r\nVeld-Test-Passed: 1\r\nConnection: close\r\n\r\n"
                };
                let _ = stream.write_all(reply.as_bytes());
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
    let fake = AlwaysPass::start();
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
    let fake = AlwaysPass::start();
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
