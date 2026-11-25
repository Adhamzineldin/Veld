//! `veld test` and the `E0953` finding of `veld check`, run through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::net::TcpListener;
use std::path::Path;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn init(dir: &Path) {
    veld(dir)
        .args(["init", "--name", "demo"])
        .assert()
        .success();
}

fn bookings() -> String {
    fs::read_to_string(repo_root().join("tests/rules/endpoints/bookings.veld")).unwrap()
}

/// A project with the bookings contract and its 20 examples.
fn project_with(contract: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    write(tmp.path(), "contracts/bookings.veld", contract);
    tmp
}

const UNFILLABLE: &str = r#"model Code {
  value: string @pattern("^[A-Z]+$")
}

service Codes at "/codes" {
  post "/" make(body: Code) -> Code
}

example "a code with nothing written" {
  make(body: {}) -> ok
}
"#;

#[test]
fn passing_examples_are_listed_and_exit_zero() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "a full room is rejected with its capacity",
        ))
        .stdout(predicate::str::contains("ok  "))
        .stdout(predicate::str::contains("20 passed"))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn a_failing_example_shows_expected_and_actual_and_exits_one() {
    let wrong = bookings().replace("RoomFull { capacity: 2 }", "RoomFull { capacity: 9 }");
    let tmp = project_with(&wrong);
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL a full room is rejected with its capacity",
        ))
        .stdout(predicate::str::contains(
            "expected: RoomFull {\"capacity\":9}",
        ))
        .stdout(predicate::str::contains(
            "actual:   RoomFull 409 {\"capacity\":2}",
        ))
        .stdout(predicate::str::contains(
            "at $.capacity: expected 9, actual 2",
        ))
        .stderr(predicate::str::contains("1 failed"));
}

#[test]
fn quiet_still_prints_failures() {
    let wrong = bookings().replace("RoomFull { capacity: 2 }", "RoomFull { capacity: 9 }");
    let tmp = project_with(&wrong);
    veld(tmp.path())
        .args(["--quiet", "test"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL a full room"))
        .stdout(predicate::str::contains("ok  ").not());
}

#[test]
fn filter_runs_only_the_matching_examples() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .args(["test", "--filter", "full room"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 passed"))
        .stdout(predicate::str::contains("a missing room").not());
    veld(tmp.path())
        .args(["test", "--filter", "nothing like this"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no examples"));
}

#[test]
fn a_project_without_examples_says_so_and_exits_zero() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("no examples"));
}

#[test]
fn dry_run_needs_live() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .args(["test", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--live"));
}

#[test]
fn live_against_nothing_names_the_url_and_exits_one() {
    let tmp = project_with(&bookings());
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    veld(tmp.path())
        .args(["test", "--live", &format!("http://{addr}")])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(addr.to_string()));
}

#[test]
fn an_example_that_cannot_be_filled_in_fails_the_run_and_the_check() {
    let tmp = project_with(UNFILLABLE);
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL a code with nothing written"))
        .stdout(predicate::str::contains("E0953"));
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[E0953]"))
        .stderr(predicate::str::contains("bookings.veld:9:1"))
        .stderr(predicate::str::contains("1 example error"));
}

#[test]
fn check_is_clean_when_every_example_can_be_filled_in() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("E0953").not());
}

// --- event examples ------------------------------------------------------------------------

const EVENTS: &str = r#"
event PaymentMade {
  paymentId: uuid
  amount: decimal
  rule positive: amount > 0
}

channel PaymentEvents {
  carries PaymentMade
  key paymentId
}

service Billing { publishes PaymentEvents }

service Receipts {
  subscribes PaymentEvents as "receipts" {
    on PaymentMade -> sendReceipt
  }
}

example "a payment reaches the receipt handler" {
  given event PaymentMade { amount: 5.00 }
  expect Receipts.sendReceipt called
}

example "a payment of nothing is rejected" {
  given event PaymentMade { amount: 0.00 }
  expect rejected
}
"#;

fn with_events(example_fix: impl Fn(&str) -> String) -> tempfile::TempDir {
    project_with(&format!("{}\n{}", bookings(), example_fix(EVENTS)))
}

#[test]
fn event_examples_run_next_to_call_examples_and_print_the_same_way() {
    let tmp = with_events(str::to_owned);
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "a payment reaches the receipt handler",
        ))
        .stdout(predicate::str::contains(
            "a full room is rejected with its capacity",
        ))
        .stdout(predicate::str::contains("22 passed"))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn a_failing_event_example_shows_expected_and_actual_and_exits_one() {
    let tmp = with_events(|t| {
        t.replace(
            "Receipts.sendReceipt called",
            "Receipts.sendReceipt not called",
        )
    });
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL a payment reaches the receipt handler",
        ))
        .stdout(predicate::str::contains(
            "expected: Receipts.sendReceipt not called",
        ))
        .stdout(predicate::str::contains(
            "actual:   called Receipts.sendReceipt",
        ))
        .stdout(predicate::str::contains(
            "at Receipts.sendReceipt: expected not called, actual called",
        ))
        .stderr(predicate::str::contains("1 failed"));
}

#[test]
fn an_event_example_that_expects_a_rejection_but_is_handled_fails() {
    let tmp = with_events(|t| t.replace("amount: 0.00", "amount: 3.00"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL a payment of nothing is rejected",
        ))
        .stdout(predicate::str::contains("expected: rejected"))
        .stdout(predicate::str::contains(
            "actual:   called Receipts.sendReceipt",
        ));
}

#[test]
fn a_rejection_is_reported_with_its_reason() {
    let tmp = with_events(|t| t.replace("amount: 5.00", "amount: 0.00"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "actual:   rejected (rule: rule positive)",
        ));
}

#[test]
fn live_skips_event_examples_and_sends_nothing_for_them() {
    let tmp = with_events(str::to_owned);
    let fake = AlwaysPass::start();
    let url = fake.url();
    veld(tmp.path())
        .args(["test", "--live", url.as_str(), "--dry-run", "--filter", "payment"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "skip a payment reaches the receipt handler (live event examples arrive with a later phase)",
        ))
        .stdout(predicate::str::contains("2 skipped"))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn check_reports_an_event_example_that_cannot_be_filled_in() {
    let contract = r#"event Code { value: string @pattern("^[A-Z]+$") }
channel Codes { carries Code }
service Maker { publishes Codes }
service Taker { subscribes Codes as "t" { on Code -> take } }
example "a code with nothing written" {
  given event Code { }
  expect Taker.take called
}
"#;
    let tmp = project_with(contract);
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[E0953]"));
}

// --- --fuzz ------------------------------------------------------------------------------

/// A test handler that answers every request with a dry-run pass. It stops when dropped.
struct AlwaysPass {
    addr: std::net::SocketAddr,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn read_target(stream: &mut std::net::TcpStream) -> Option<String> {
    use std::io::Read;
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
    Some(head.lines().next()?.split(' ').nth(1)?.to_owned())
}

impl AlwaysPass {
    fn start() -> AlwaysPass {
        use std::io::Write;
        use std::sync::atomic::Ordering;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop2 = stop.clone();
        // The probe of a test handler of the `Bookings` service (`spec/rules.md` section 12.4).
        let body = "{\"veldTestHandler\":1,\"digests\":{\"Bookings\":null}}";
        let probe = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop2.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut stream) = stream else { continue };
                let reply = if read_target(&mut stream).as_deref() == Some("/__veld/test") {
                    probe.as_str()
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
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Runs `veld test --live <fake> --dry-run <extra>` and returns stdout; the run must exit 1.
///
/// Only one example runs, `a valid booking passes every check`, and the fake passes it in a dry
/// run: the exit code 1 can only come from the fuzz.
fn fuzz_stdout(tmp: &Path, extra: &[&str]) -> String {
    let fake = AlwaysPass::start();
    let url = fake.url();
    let filter = "a valid booking passes every check";
    let mut args = vec![
        "test",
        "--live",
        url.as_str(),
        "--dry-run",
        "--filter",
        filter,
    ];
    args.extend_from_slice(extra);
    let out = veld(tmp).args(&args).assert().code(1).get_output().clone();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("1 passed"), "the example passes: {stdout}");
    assert!(!stdout.contains("FAIL a "), "no example fails: {stdout}");
    stdout
}

/// What a run prints, without the line that names the port of the fake.
fn without_port(text: &str) -> String {
    text.lines()
        .filter(|l| !l.contains("127.0.0.1"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn fuzz_needs_live_and_seed_needs_fuzz() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .args(["test", "--fuzz", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--live"));
    veld(tmp.path())
        .args(["test", "--live", "http://127.0.0.1:1", "--seed", "3"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--fuzz"));
}

/// `--fuzz 0` used to print a green tick for a run that sent nothing, and a count near the
/// largest integer overflowed. Both are refused as usage errors, before anything is sent.
#[test]
fn a_fuzz_count_of_zero_or_beyond_the_limit_is_a_usage_error() {
    let tmp = project_with(&bookings());
    for count in ["0", "100001", "18446744073709551615"] {
        veld(tmp.path())
            .args(["test", "--live", "http://127.0.0.1:1", "--fuzz", count])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("--fuzz"));
    }
}

#[test]
fn a_fuzz_run_against_a_server_that_always_passes_fails_with_a_seed_and_an_example() {
    let tmp = project_with(&bookings());
    let stdout = fuzz_stdout(tmp.path(), &["--fuzz", "60", "--seed", "4242"]);
    assert!(stdout.contains("seed 4242"), "{stdout}");
    assert!(stdout.contains("repeat with --seed 4242"), "{stdout}");
    assert!(
        stdout.contains("FAIL Bookings.createBooking (seed 4242"),
        "{stdout}"
    );
    assert!(
        stdout.contains("example \"fuzz seed 4242 Bookings.createBooking request"),
        "{stdout}"
    );
    assert!(stdout.contains("given room = "), "{stdout}");
    assert!(stdout.contains("createBooking(body: {"), "{stdout}");
    assert!(
        stdout.contains(" sent ("),
        "the tally per endpoint: {stdout}"
    );
}

#[test]
fn the_same_seed_repeats_a_fuzz_run_and_another_seed_does_not() {
    let tmp = project_with(&bookings());
    let first = without_port(&fuzz_stdout(tmp.path(), &["--fuzz", "60", "--seed", "99"]));
    let again = without_port(&fuzz_stdout(tmp.path(), &["--fuzz", "60", "--seed", "99"]));
    assert_eq!(first, again);
    let other = without_port(&fuzz_stdout(tmp.path(), &["--fuzz", "60", "--seed", "100"]));
    assert_ne!(first, other);
}

#[test]
fn a_fuzz_run_without_a_seed_prints_the_one_it_chose() {
    let tmp = project_with(&bookings());
    let stdout = fuzz_stdout(tmp.path(), &["--fuzz", "5"]);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("fuzz: 5 requests"))
        .unwrap();
    let seed: u64 = line
        .split("--seed ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .parse()
        .unwrap();
    assert!(stdout.contains(&format!("seed {seed}")), "{stdout}");
}

// --- `@rollout(audit)` (spec/rules.md section 17.1) ----------------------------------------

fn rollout() -> String {
    fs::read_to_string(repo_root().join("tests/rules/endpoints/rollout.veld")).unwrap()
}

#[test]
fn an_audited_rule_is_listed_under_the_example_that_violates_it_and_does_not_fail_it() {
    let tmp = project_with(&rollout());
    let out =
        veld(tmp.path()).arg("test").assert().success().stdout(
            predicate::str::contains("11 passed").and(predicate::str::contains("FAIL").not()),
        );
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    let at = lines
        .iter()
        .position(|l| {
            l.contains("an audited invariant that is violated does not reject the request")
        })
        .unwrap();
    assert!(
        lines[at + 1].contains("audited smallParty") && lines[at + 1].starts_with("     "),
        "{stdout}"
    );
    // An example that violates no audited rule has nothing listed under it.
    let at = lines
        .iter()
        .position(|l| l.contains("an audited rule that holds changes nothing"))
        .unwrap();
    assert!(lines[at + 1].starts_with("ok "), "{stdout}");
}

#[test]
fn the_output_of_a_contract_without_audited_rules_has_no_audited_lines() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("audited").not());
}
